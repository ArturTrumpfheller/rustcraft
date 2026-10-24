use crate::terrain_material::{AtlasRepeat, TerrainSurface};
use bevy::{
    asset::RenderAssetUsages, camera::primitives::Aabb, ecs::system::SystemParam, mesh::Indices,
    prelude::*, render::render_resource::PrimitiveTopology,
};
use std::collections::{BTreeMap, BTreeSet};
use voxel_core::{CHUNK_SIZE, ChunkPos, World, demo_world};

pub struct VoxelScenePlugin;
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct SceneSetup;
/// World-space offset used only by presentation. Physics/rays remain f64 world coordinates.
#[derive(Resource, Default)]
pub(crate) struct RenderOrigin(pub [f64; 3]);

#[derive(Resource)]
pub(crate) struct VoxelWorld(pub World);
impl Default for VoxelWorld {
    fn default() -> Self {
        Self(demo_world())
    }
}

#[derive(Resource, Default)]
pub(crate) struct DirtyChunks(pub BTreeSet<ChunkPos>, pub BTreeSet<ChunkPos>);
impl DirtyChunks {
    pub fn mark(&mut self, change: voxel_core::BlockChange) {
        self.0.extend(change.affected_chunks());
        self.1
            .extend(voxel_core::lighting::affected(change.position));
    }
}

#[derive(Resource, Default)]
pub(crate) struct ChunkRenders(pub BTreeMap<ChunkPos, ChunkRender>);
pub(crate) struct ChunkRender {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}
#[derive(Resource)]
pub(crate) struct TerrainMaterial(pub Handle<TerrainSurface>, pub Handle<Image>);

impl Plugin for VoxelScenePlugin {
    fn build(&self, app: &mut App) {
        if !app
            .world()
            .contains_resource::<crate::session::ClientFlow>()
        {
            app.init_resource::<VoxelWorld>();
        }
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<TerrainSurface>>()
            .init_resource::<Assets<crate::atmosphere::SkyMaterial>>()
            .init_resource::<RenderOrigin>()
            .init_resource::<DirtyChunks>()
            .init_resource::<LegacyMeshing>()
            .init_resource::<ChunkRenders>()
            .add_systems(
                Startup,
                (spawn_scene.in_set(SceneSetup)).run_if(crate::session::has_world),
            )
            .add_systems(crate::session::SessionSetup, spawn_scene.in_set(SceneSetup))
            .add_systems(
                Update,
                rebuild_dirty_chunks.run_if(crate::session::has_world),
            )
            .add_systems(
                Startup,
                spawn_atmosphere
                    .after(SceneSetup)
                    .after(crate::camera::CameraSetup)
                    .run_if(crate::session::has_world),
            )
            .add_systems(
                crate::session::SessionSetup,
                spawn_atmosphere
                    .after(SceneSetup)
                    .after(crate::camera::CameraSetup),
            )
            .add_systems(
                PostUpdate,
                update_atmosphere.run_if(crate::session::has_world),
            );
    }
}

fn convert_mesh(surface: voxel_core::ChunkMesh) -> (Mesh, Visibility) {
    let visibility = if surface.indices.is_empty() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    // Retain CPU assets so edits can replace buffers under the same handle.
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, surface.atlas_uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors)
    .with_inserted_indices(Indices::U32(surface.indices));
    (mesh, visibility)
}

pub(crate) fn upsert_stream_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    position: ChunkPos,
    surface: voxel_core::ChunkMesh,
    material: &Handle<TerrainSurface>,
    origin: &RenderOrigin,
) {
    if surface.indices.is_empty() {
        if let Some(render) = renders.0.remove(&position) {
            commands.entity(render.entity).despawn();
            meshes.remove(&render.mesh);
        }
    } else {
        upsert_chunk(
            commands, meshes, renders, position, surface, material, origin,
        );
    }
}
pub(crate) fn upsert_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    position: ChunkPos,
    surface: voxel_core::ChunkMesh,
    material: &Handle<TerrainSurface>,
    origin: &RenderOrigin,
) {
    let (mesh, visibility) = convert_mesh(surface);
    if let Some(render) = renders.0.get(&position) {
        *meshes.get_mut(&render.mesh).expect("retained terrain mesh") = mesh;
        commands
            .entity(render.entity)
            .insert(visibility)
            .remove::<Aabb>();
        return;
    }
    let handle = meshes.add(mesh);
    let entity = commands
        .spawn((
            crate::session::SessionEntity,
            Name::new(format!("Voxel chunk {:?}", position.0)),
            Mesh3d(handle.clone()),
            MeshMaterial3d(material.clone()),
            visibility,
            Transform::from_translation(Vec3::from_array(std::array::from_fn(|axis| {
                (f64::from(position.0[axis]) * CHUNK_SIZE as f64 - origin.0[axis]) as f32
            }))),
        ))
        .id();
    renders.0.insert(
        position,
        ChunkRender {
            entity,
            mesh: handle,
        },
    );
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainSurface>>,
    mut images: ResMut<Assets<Image>>,
    world: Res<VoxelWorld>,
    mut renders: ResMut<ChunkRenders>,
    origin: Res<RenderOrigin>,
) {
    let started = std::time::Instant::now();
    let atlas = images.add(crate::terrain_texture::atlas_image());
    let material = materials.add(TerrainSurface {
        base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(atlas.clone()),
            perceptual_roughness: 1.0,
            unlit: true,
            ..default()
        },
        extension: AtlasRepeat {
            atlas: atlas.clone(),
            atmosphere: crate::atmosphere::AtmosphereUniform::for_radius(2),
        },
    });
    let initial_targets: BTreeSet<_> = world.0.chunks().map(|(p, _)| p).collect();
    let initial_light = if world.0.identity().is_none() && !initial_targets.is_empty() {
        Some(voxel_core::lighting::Skylight::for_chunks(
            &world.0,
            &initial_targets,
        ))
    } else {
        None
    };
    for (position, _) in world.0.chunks() {
        // Streamed startup is published only after an authoritative lit halo is
        // available from the worker, avoiding a temporary pending-data light pass.
        let Some(light) = &initial_light else {
            continue;
        };
        let surface = voxel_core::mesh_chunk_lit(&world.0, position, light);
        upsert_chunk(
            &mut commands,
            &mut meshes,
            &mut renders,
            position,
            surface,
            &material,
            &origin,
        );
    }

    commands.insert_resource(TerrainMaterial(material, atlas));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 0.0,
        ..default()
    });
    info!(
        "RustCraft slice 10: {} prepared chunks, {} initial meshes in {:?}",
        world.0.chunks().count(),
        renders.0.len(),
        started.elapsed()
    );
}

struct LegacyJob {
    revision: u64,
    handle: std::thread::JoinHandle<Vec<(ChunkPos, voxel_core::ChunkMesh)>>,
}
#[derive(Resource, Default)]
pub(crate) struct LegacyMeshing {
    job: Option<LegacyJob>,
}
impl LegacyMeshing {
    pub(crate) fn drain(&mut self) -> bool {
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let _ = self.job.take().unwrap().handle.join();
        }
        self.job.is_none()
    }
    #[cfg(test)]
    pub(crate) fn pending(&self) -> bool {
        self.job.is_some()
    }
}
impl Drop for LegacyMeshing {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.handle.join();
        }
    }
}
#[derive(SystemParam)]
struct TerrainWrites<'w, 's> {
    commands: Commands<'w, 's>,
    renders: ResMut<'w, ChunkRenders>,
    meshes: ResMut<'w, Assets<Mesh>>,
    material: Res<'w, TerrainMaterial>,
    origin: Res<'w, RenderOrigin>,
}
fn rebuild_dirty_chunks(
    world: Res<VoxelWorld>,
    mut dirty: ResMut<DirtyChunks>,
    mut scene: TerrainWrites,
    mut legacy: ResMut<LegacyMeshing>,
    flow: Option<Res<crate::session::ClientFlow>>,
) {
    if flow.as_ref().is_some_and(|f| f.leaving()) {
        return;
    }
    if world.0.identity().is_some() {
        return;
    }
    if legacy.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
        let job = legacy.job.take().unwrap();
        let revision = job.revision;
        match job.handle.join() {
            Ok(batch) if world.0.revision() == revision => {
                for (p, m) in batch {
                    upsert_chunk(
                        &mut scene.commands,
                        &mut scene.meshes,
                        &mut scene.renders,
                        p,
                        m,
                        &scene.material.0,
                        &scene.origin,
                    );
                }
                dirty.0.clear();
                dirty.1.clear();
            }
            Ok(_) => {}
            Err(_) => warn!("Legacy lighting worker stopped; retaining dirty meshes for retry"),
        }
    }
    if legacy.job.is_none() && (!dirty.0.is_empty() || !dirty.1.is_empty()) {
        let targets: BTreeSet<_> = dirty
            .0
            .union(&dirty.1)
            .filter(|p| world.0.chunk(**p).is_some())
            .copied()
            .collect();
        if targets.is_empty() {
            dirty.0.clear();
            dirty.1.clear();
            return;
        }
        let snapshot = world.0.clone();
        let revision = world.0.revision();
        match std::thread::Builder::new()
            .name("rustcraft-legacy-light".into())
            .spawn(move || {
                let light = voxel_core::lighting::Skylight::for_chunks(&snapshot, &targets);
                targets
                    .into_iter()
                    .map(|p| (p, voxel_core::mesh_chunk_lit(&snapshot, p, &light)))
                    .collect()
            }) {
            Ok(job) => {
                legacy.job = Some(LegacyJob {
                    revision,
                    handle: job,
                })
            }
            Err(e) => warn!("Could not start legacy lighting worker: {e}"),
        }
    }
}

#[cfg(test)]
mod tests;

/// Test oracle for topology-independent comparisons after light splits/merges.
#[cfg(test)]
pub(crate) fn unit_faces(mesh: &Mesh) -> BTreeSet<([i32; 3], [i32; 3])> {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions")
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("normals")
    };
    let mut out = BTreeSet::new();
    for (q, p) in positions.as_chunks::<4>().0.iter().enumerate() {
        let n = normals[q * 4].map(|v| v as i32);
        let axis = n.iter().position(|&v| v != 0).unwrap();
        let mut lo: [i32; 3] =
            std::array::from_fn(|a| p.iter().map(|p| p[a] as i32).min().unwrap());
        let mut hi: [i32; 3] =
            std::array::from_fn(|a| p.iter().map(|p| p[a] as i32).max().unwrap());
        if n[axis] > 0 {
            lo[axis] -= 1;
        }
        hi[axis] = lo[axis] + 1;
        for x in lo[0]..hi[0] {
            for y in lo[1]..hi[1] {
                for z in lo[2]..hi[2] {
                    assert!(out.insert(([x, y, z], n)));
                }
            }
        }
    }
    out
}

fn atmosphere_radius(
    stream: Option<&crate::streaming::StreamState>,
    preferences: Option<&crate::settings::Preferences>,
) -> i32 {
    stream
        .map(|s| s.radius())
        .filter(|r| (1..=voxel_core::streaming::MAX_RADIUS).contains(r))
        .unwrap_or_else(|| preferences.map_or(2, |p| p.radius))
}
#[derive(SystemParam)]
struct AtmosphereSettings<'w> {
    stream: Option<Res<'w, crate::streaming::StreamState>>,
    preferences: Option<Res<'w, crate::settings::Preferences>>,
}
impl AtmosphereSettings<'_> {
    fn parameters(&self) -> crate::atmosphere::AtmosphereUniform {
        crate::atmosphere::AtmosphereUniform::for_radius(atmosphere_radius(
            self.stream.as_deref(),
            self.preferences.as_deref(),
        ))
    }
}
fn spawn_atmosphere(
    mut commands: Commands,
    camera: Query<(), With<crate::camera::FirstPersonCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::atmosphere::SkyMaterial>>,
    mut terrain: ResMut<Assets<TerrainSurface>>,
    handle: Res<TerrainMaterial>,
    settings: AtmosphereSettings,
) {
    if camera.is_empty() {
        return;
    }
    let params = settings.parameters();
    if let Some(mut m) = terrain.get_mut(&handle.0) {
        m.extension.atmosphere = params;
    }
    let sky = crate::atmosphere::spawn_sky(&mut commands, &mut meshes, &mut materials, params);
    commands
        .entity(sky.entity)
        .insert(crate::session::SessionEntity);
    commands.insert_resource(sky);
}
fn update_atmosphere(
    settings: AtmosphereSettings,
    handle: Option<Res<TerrainMaterial>>,
    sky: Option<Res<crate::atmosphere::SkyAssets>>,
    mut terrain: ResMut<Assets<TerrainSurface>>,
    mut materials: ResMut<Assets<crate::atmosphere::SkyMaterial>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let params = settings.parameters();
    crate::atmosphere::update_materials(
        params,
        &handle.0,
        sky.as_ref().map(|s| &s.material),
        &mut terrain,
        &mut materials,
    );
}
