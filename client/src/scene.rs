use bevy::{
    asset::RenderAssetUsages, camera::primitives::Aabb, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::{BTreeMap, BTreeSet};
use voxel_core::{CHUNK_SIZE, ChunkPos, World, demo_world, mesh_chunk};

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
pub(crate) struct DirtyChunks(pub BTreeSet<ChunkPos>);

#[derive(Resource, Default)]
pub(crate) struct ChunkRenders(pub BTreeMap<ChunkPos, ChunkRender>);
pub(crate) struct ChunkRender {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}
#[derive(Resource)]
pub(crate) struct TerrainMaterial(pub Handle<StandardMaterial>);

impl Plugin for VoxelScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelWorld>()
            .init_resource::<Assets<Image>>()
            .init_resource::<RenderOrigin>()
            .init_resource::<DirtyChunks>()
            .init_resource::<ChunkRenders>()
            .add_systems(Startup, spawn_scene.in_set(SceneSetup))
            .add_systems(Update, rebuild_dirty_chunks);
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
    .with_inserted_indices(Indices::U32(surface.indices));
    (mesh, visibility)
}

pub(crate) fn upsert_stream_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    position: ChunkPos,
    surface: voxel_core::ChunkMesh,
    material: &Handle<StandardMaterial>,
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
    material: &Handle<StandardMaterial>,
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    world: Res<VoxelWorld>,
    mut renders: ResMut<ChunkRenders>,
    origin: Res<RenderOrigin>,
) {
    let started = std::time::Instant::now();
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(crate::terrain_texture::atlas_image())),
        perceptual_roughness: 1.0,
        ..default()
    });
    for (position, _) in world.0.chunks() {
        let surface = mesh_chunk(&world.0, position);
        if world.0.identity().is_some() {
            upsert_stream_mesh(
                &mut commands,
                &mut meshes,
                &mut renders,
                position,
                surface,
                &material,
                &origin,
            );
        } else {
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
    }
    commands.insert_resource(TerrainMaterial(material));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_xyz(-12.0, 24.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    info!(
        "RustCraft slice 8: {} restored/generated chunk meshes built in {:?}",
        world.0.chunks().count(),
        started.elapsed()
    );
}

fn rebuild_dirty_chunks(
    mut commands: Commands,
    world: Res<VoxelWorld>,
    mut dirty: ResMut<DirtyChunks>,
    mut renders: ResMut<ChunkRenders>,
    mut meshes: ResMut<Assets<Mesh>>,
    material: Res<TerrainMaterial>,
    origin: Res<RenderOrigin>,
) {
    if world.0.identity().is_some() {
        return;
    } // Stream worker owns generation/meshing.
    while let Some(position) = dirty.0.pop_first() {
        if world.0.chunk(position).is_some() {
            upsert_chunk(
                &mut commands,
                &mut meshes,
                &mut renders,
                position,
                mesh_chunk(&world.0, position),
                &material.0,
                &origin,
            );
        }
    }
}

#[cfg(test)]
mod tests;
