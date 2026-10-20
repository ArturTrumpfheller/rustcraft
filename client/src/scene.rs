use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use voxel_core::{CHUNK_SIZE, World, demo_world, mesh_chunk};

pub struct VoxelScenePlugin;

#[derive(Resource)]
struct VoxelWorld(World);

impl Plugin for VoxelScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoxelWorld(demo_world()))
            .add_systems(Startup, spawn_scene);
    }
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world: Res<VoxelWorld>,
) {
    // Bounded, one-time construction: four 16³ chunks. No per-frame world work.
    let world = &world.0;
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        ..default()
    });
    for (position, _) in world.chunks() {
        let surface = mesh_chunk(world, position);
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface.colors)
        .with_inserted_indices(Indices::U32(surface.indices));
        commands.spawn((
            Name::new(format!("Voxel chunk {:?}", position.0)),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::from_array(
                position.0.map(|axis| (axis * CHUNK_SIZE as i32) as f32),
            )),
        ));
    }
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
    info!("RustCraft slice 1: four chunks; free-flight camera, no collision or world editing yet.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_plugin_builds_four_renderable_chunk_entities_without_external_assets() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_plugins(VoxelScenePlugin);
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<VoxelWorld>().0.chunks().count(), 4);
        let handles: Vec<_> = world
            .query::<&Mesh3d>()
            .iter(world)
            .map(|mesh| mesh.0.clone())
            .collect();
        assert_eq!(handles.len(), 4);
        let meshes = world.resource::<Assets<Mesh>>();
        for handle in handles {
            let mesh = meshes.get(&handle).unwrap();
            assert!(mesh.count_vertices() > 0);
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
            assert!(!mesh.indices().unwrap().is_empty());
        }
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 1);
        assert_eq!(world.query::<&DirectionalLight>().iter(world).count(), 1);
    }
}
