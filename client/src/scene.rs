use bevy::{
    asset::RenderAssetUsages, camera::primitives::Aabb, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::{BTreeMap, BTreeSet};
use voxel_core::{CHUNK_SIZE, ChunkPos, World, demo_world, mesh_chunk};

pub struct VoxelScenePlugin;

#[derive(Resource)]
pub(crate) struct VoxelWorld(pub World);

#[derive(Resource, Default)]
pub(crate) struct DirtyChunks(pub BTreeSet<ChunkPos>);

#[derive(Resource, Default)]
struct ChunkRenders(BTreeMap<ChunkPos, ChunkRender>);
struct ChunkRender {
    entity: Entity,
    mesh: Handle<Mesh>,
}
#[derive(Resource)]
struct TerrainMaterial(Handle<StandardMaterial>);

impl Plugin for VoxelScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoxelWorld(demo_world()))
            .init_resource::<DirtyChunks>()
            .init_resource::<ChunkRenders>()
            .add_systems(Startup, spawn_scene)
            .add_systems(Update, rebuild_dirty_chunks);
    }
}

fn render_mesh(world: &World, position: ChunkPos) -> (Mesh, Visibility) {
    let surface = mesh_chunk(world, position);
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
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors)
    .with_inserted_indices(Indices::U32(surface.indices));
    (mesh, visibility)
}

fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    renders: &mut ChunkRenders,
    world: &World,
    position: ChunkPos,
    material: &Handle<StandardMaterial>,
) {
    let (mesh, visibility) = render_mesh(world, position);
    let handle = meshes.add(mesh);
    let entity = commands
        .spawn((
            Name::new(format!("Voxel chunk {:?}", position.0)),
            Mesh3d(handle.clone()),
            MeshMaterial3d(material.clone()),
            visibility,
            Transform::from_translation(Vec3::from_array(
                position.0.map(|axis| (axis * CHUNK_SIZE as i32) as f32),
            )),
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
    world: Res<VoxelWorld>,
    mut renders: ResMut<ChunkRenders>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        ..default()
    });
    for (position, _) in world.0.chunks() {
        spawn_chunk(
            &mut commands,
            &mut meshes,
            &mut renders,
            &world.0,
            position,
            &material,
        );
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
    info!("RustCraft slice 3: in-memory block edits; changes are lost when the game closes.");
}

fn rebuild_dirty_chunks(
    mut commands: Commands,
    world: Res<VoxelWorld>,
    mut dirty: ResMut<DirtyChunks>,
    mut renders: ResMut<ChunkRenders>,
    mut meshes: ResMut<Assets<Mesh>>,
    material: Res<TerrainMaterial>,
) {
    while let Some(position) = dirty.0.pop_first() {
        if world.0.chunk(position).is_none() {
            continue;
        }
        if let Some(render) = renders.0.get(&position) {
            let (mesh, visibility) = render_mesh(&world.0, position);
            *meshes
                .get_mut(&render.mesh)
                .expect("chunk mesh retained in MAIN_WORLD") = mesh;
            // Bevy computes a fresh bound after geometry changes. Otherwise a
            // newly expanded chunk could retain its old frustum-culling box.
            commands
                .entity(render.entity)
                .insert(visibility)
                .remove::<Aabb>();
        } else {
            spawn_chunk(
                &mut commands,
                &mut meshes,
                &mut renders,
                &world.0,
                position,
                &material.0,
            );
        }
    }
}

#[cfg(test)]
mod tests;
