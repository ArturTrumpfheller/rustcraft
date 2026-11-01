#[path = "dropped_items.rs"]
mod dropped_items;
pub(crate) fn cleanup_drops(world: &mut bevy::prelude::World) {
    dropped_items::cleanup(world);
}
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
pub(crate) struct DirtyChunks(
    pub BTreeSet<ChunkPos>,
    pub BTreeSet<ChunkPos>,
    pub BTreeSet<ChunkPos>,
);
impl DirtyChunks {
    #[cfg(test)]
    pub fn mark(&mut self, change: voxel_core::BlockChange) {
        self.mark_in(change, voxel_core::bounds::WorldHeight::HISTORICAL);
    }
    pub fn mark_in(
        &mut self,
        change: voxel_core::BlockChange,
        height: voxel_core::bounds::WorldHeight,
    ) {
        let relight = change.before.is_opaque() != change.after.is_opaque()
            || change.before.emission() != change.after.emission();
        if !relight
            && !change.before.is_plant()
            && !change.after.is_plant()
            && (change.before.fluid().is_some() || change.after.fluid().is_some())
        {
            self.2.extend(change.affected_chunks());
        } else {
            self.0.extend(change.affected_chunks());
        }
        if relight {
            self.1
                .extend(voxel_core::lighting::affected_in(change.position, height));
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct ChunkRenders(
    pub BTreeMap<ChunkPos, ChunkRender>,
    pub BTreeMap<ChunkPos, ChunkRender>,
    pub BTreeMap<ChunkPos, ChunkRender>,
);
pub(crate) struct ChunkRender {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}
#[derive(Resource)]
pub(crate) struct TerrainMaterial(
    pub Handle<TerrainSurface>,
    pub Handle<Image>,
    pub Handle<crate::terrain_material::WaterSurface>,
    pub Handle<TerrainSurface>,
);

impl Plugin for VoxelScenePlugin {
    fn build(&self, app: &mut App) {
        dropped_items::register(app);
        if !app
            .world()
            .contains_resource::<crate::session::ClientFlow>()
        {
            app.init_resource::<VoxelWorld>();
        }
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<TerrainSurface>>()
            .init_resource::<Assets<crate::terrain_material::WaterSurface>>()
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
                (
                    update_atmosphere,
                    update_underwater.after(crate::camera::ViewSync),
                )
                    .run_if(crate::session::has_world),
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
    mut surface: voxel_core::ChunkMesh,
    material: &TerrainMaterial,
    origin: &RenderOrigin,
) {
    if surface.water_only || surface.indices.is_empty() {
        if !surface.water_only {
            upsert_plants(
                commands,
                meshes,
                renders,
                position,
                std::mem::take(&mut surface.plants),
                material,
                origin,
            );
        }
        upsert_water(
            commands,
            meshes,
            renders,
            position,
            std::mem::take(&mut surface.water),
            &material.2,
            origin,
        );
        if surface.water_only {
            return;
        }
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
    mut surface: voxel_core::ChunkMesh,
    material: &TerrainMaterial,
    origin: &RenderOrigin,
) {
    if surface.indices.is_empty() {
        // Legacy and streamed publication share the empty-surface lifecycle.
        upsert_stream_mesh(
            commands, meshes, renders, position, surface, material, origin,
        );
        return;
    }
    // Full legacy/streamed publication owns all three chunk surfaces.
    upsert_plants(
        commands,
        meshes,
        renders,
        position,
        std::mem::take(&mut surface.plants),
        material,
        origin,
    );
    if !surface.water.indices.is_empty() || renders.1.contains_key(&position) {
        upsert_water(
            commands,
            meshes,
            renders,
            position,
            std::mem::take(&mut surface.water),
            &material.2,
            origin,
        );
    }
    let (mesh, visibility) = convert_mesh(surface);
    if let Some(render) = renders.0.get(&position) {
        *meshes.get_mut(&render.mesh).expect("retained terrain mesh") = mesh;
        // Reconstruct through Bevy's serial missing-bounds path before culling.
        // Retaining Aabb selected its parallel changed-bounds path and worsened
        // publication tails in the matched probe; keep the measured baseline.
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
            MeshMaterial3d(material.0.clone()),
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

#[allow(clippy::too_many_arguments)] // ECS setup owns both retained material asset stores.
fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainSurface>>,
    mut water_materials: ResMut<Assets<crate::terrain_material::WaterSurface>>,
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
    let water_texture = images.add(crate::terrain_texture::water_image());
    let water = water_materials.add(crate::terrain_material::WaterSurface {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: crate::terrain_material::WaterEffect {
            texture: water_texture,
            atmosphere: crate::atmosphere::AtmosphereUniform::for_radius(2),
            effect: Vec4::ZERO,
        },
    });
    let plants = materials.add(TerrainSurface {
        base: StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: AtlasRepeat {
            atlas: atlas.clone(),
            atmosphere: crate::atmosphere::AtmosphereUniform::for_radius(2),
        },
    });
    let material = TerrainMaterial(material, atlas, water, plants);
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

    commands.insert_resource(material);
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
                        &scene.material,
                        &scene.origin,
                    );
                }
                dirty.0.clear();
                dirty.1.clear();
                dirty.2.clear();
            }
            Ok(_) => {}
            Err(_) => warn!("Legacy lighting worker stopped; retaining dirty meshes for retry"),
        }
    }
    if legacy.job.is_none() && (!dirty.0.is_empty() || !dirty.1.is_empty() || !dirty.2.is_empty()) {
        let targets: BTreeSet<_> = dirty
            .0
            .union(&dirty.1)
            .chain(dirty.2.iter())
            .filter(|p| world.0.chunk(**p).is_some())
            .copied()
            .collect();
        if targets.is_empty() {
            dirty.0.clear();
            dirty.1.clear();
            dirty.2.clear();
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
    world: Option<Res<'w, VoxelWorld>>,
    stream: Option<Res<'w, crate::streaming::StreamState>>,
    preferences: Option<Res<'w, crate::settings::Preferences>>,
}
impl AtmosphereSettings<'_> {
    fn parameters(&self) -> crate::atmosphere::AtmosphereUniform {
        crate::atmosphere::AtmosphereUniform::at_time(
            atmosphere_radius(self.stream.as_deref(), self.preferences.as_deref()),
            self.world
                .as_ref()
                .map_or_else(Default::default, |w| w.0.time),
        )
    }
}
#[allow(clippy::too_many_arguments)] // ECS setup includes the lightweight camera tint.
fn spawn_atmosphere(
    mut commands: Commands,
    camera: Query<Entity, With<crate::camera::FirstPersonCamera>>,
    standard: Option<ResMut<Assets<StandardMaterial>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::atmosphere::SkyMaterial>>,
    mut terrain: ResMut<Assets<TerrainSurface>>,
    handle: Res<TerrainMaterial>,
    settings: AtmosphereSettings,
) {
    if camera.is_empty() {
        return;
    }
    if let Some(mut standard) = standard {
        let tint = commands
            .spawn((
                crate::session::SessionEntity,
                UnderwaterTint,
                Mesh3d(meshes.add(Rectangle::new(2., 2.))),
                MeshMaterial3d(standard.add(StandardMaterial {
                    base_color: Color::srgba(0.04, 0.22, 0.65, 0.16),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                bevy::camera::visibility::RenderLayers::layer(crate::viewmodel::OVERLAY_LAYER),
                Transform::from_xyz(0., 0., -0.3),
                Visibility::Hidden,
            ))
            .id();
        if let Ok(camera) = camera.single() {
            commands.entity(camera).add_child(tint);
        }
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
    mut water: ResMut<Assets<crate::terrain_material::WaterSurface>>,
    renders: Res<ChunkRenders>,
) {
    let Some(handle) = handle else {
        return;
    };
    let params = settings.parameters();
    let animation = settings
        .world
        .as_ref()
        .map_or(0., |w| (w.0.time.units() / 20.) as f32);
    if !renders.1.is_empty()
        && water
            .get(&handle.2)
            .is_some_and(|m| m.extension.atmosphere != params || m.extension.effect.x != animation)
        && let Some(mut material) = water.get_mut(&handle.2)
    {
        material.extension.atmosphere = params;
        material.extension.effect.x = animation;
    }

    if terrain
        .get(&handle.3)
        .is_some_and(|m| m.extension.atmosphere != params)
        && let Some(mut m) = terrain.get_mut(&handle.3)
    {
        m.extension.atmosphere = params;
    }
    crate::atmosphere::update_materials(
        params,
        &handle.0,
        sky.as_ref().map(|s| &s.material),
        &mut terrain,
        &mut materials,
    );
}

fn upsert_water(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    position: ChunkPos,
    surface: voxel_core::water_mesh::WaterMesh,
    material: &Handle<crate::terrain_material::WaterSurface>,
    origin: &RenderOrigin,
) {
    if surface.indices.is_empty() {
        if let Some(r) = renders.1.remove(&position) {
            commands.entity(r.entity).despawn();
            meshes.remove(&r.mesh);
        }
        return;
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors)
    .with_inserted_indices(Indices::U32(surface.indices));
    if let Some(r) = renders.1.get(&position) {
        *meshes.get_mut(&r.mesh).expect("retained water mesh") = mesh;
        commands.entity(r.entity).remove::<Aabb>();
        return;
    }
    let mesh = meshes.add(mesh);
    let entity = commands
        .spawn((
            crate::session::SessionEntity,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::from_array(std::array::from_fn(|a| {
                (f64::from(position.0[a]) * 16. - origin.0[a]) as f32
            }))),
        ))
        .id();
    renders.1.insert(position, ChunkRender { entity, mesh });
}

#[derive(Component)]
struct UnderwaterTint;
fn update_underwater(
    world: Res<VoxelWorld>,
    view: Option<Res<crate::camera::ViewPose>>,
    camera: Query<&Projection, With<crate::camera::FirstPersonCamera>>,
    mut tint: Query<(&mut Transform, &mut Visibility), With<UnderwaterTint>>,
) {
    let Some(view) = view else {
        return;
    };
    let Ok(Projection::Perspective(p)) = camera.single() else {
        return;
    };
    for (mut t, mut visibility) in &mut tint {
        let submerged = voxel_core::physics::submerged(&world.0, view.eye);
        *visibility = if submerged {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let h = 0.3 * (p.fov * 0.5).tan();
        t.scale = Vec3::new(h * p.aspect_ratio * 1.02, h * 1.02, 1.);
    }
}

fn upsert_plants(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    position: ChunkPos,
    surface: voxel_core::plant_mesh::PlantMesh,
    material: &TerrainMaterial,
    origin: &RenderOrigin,
) {
    if surface.indices.is_empty() {
        if let Some(r) = renders.2.remove(&position) {
            commands.entity(r.entity).despawn();
            meshes.remove(&r.mesh);
        }
        return;
    }
    let (mesh, visibility) = convert_mesh(surface.into_surface());
    if let Some(r) = renders.2.get(&position) {
        *meshes.get_mut(&r.mesh).expect("retained plant mesh") = mesh;
        commands
            .entity(r.entity)
            .insert(visibility)
            .remove::<Aabb>();
    } else {
        let mesh = meshes.add(mesh);
        let entity = commands
            .spawn((
                crate::session::SessionEntity,
                Name::new("Chunk plants"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.3.clone()),
                visibility,
                Transform::from_translation(Vec3::from_array(std::array::from_fn(|a| {
                    (f64::from(position.0[a]) * 16. - origin.0[a]) as f32
                }))),
            ))
            .id();
        renders.2.insert(position, ChunkRender { entity, mesh });
    }
}
