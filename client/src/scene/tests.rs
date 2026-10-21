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

fn edit_cell(app: &mut App, position: [i32; 3], block: voxel_core::Block) {
    let change = app
        .world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(voxel_core::BlockPos(position), block)
        .unwrap()
        .unwrap();
    app.world_mut()
        .resource_mut::<DirtyChunks>()
        .0
        .extend(change.affected_chunks());
    app.update();
}

fn terrain_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_plugins(VoxelScenePlugin);
    app.insert_resource(VoxelWorld(World::default()));
    app.update();
    app
}

#[test]
fn repeated_boundary_edits_update_neighbors_and_reuse_assets_even_when_empty() {
    let mut app = terrain_app();
    edit_cell(&mut app, [15, 1, 1], voxel_core::Block::Stone);
    edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Dirt);
    let renders = app.world().resource::<ChunkRenders>();
    let left = renders.0[&ChunkPos([0, 0, 0])].mesh.clone();
    let right = renders.0[&ChunkPos([1, 0, 0])].mesh.clone();
    let meshes = app.world().resource::<Assets<Mesh>>();
    assert_eq!(meshes.get(&left).unwrap().indices().unwrap().len(), 30);
    assert_eq!(meshes.get(&right).unwrap().indices().unwrap().len(), 30);
    for _ in 0..50 {
        edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Air);
        let world = app.world();
        assert_eq!(
            world
                .resource::<Assets<Mesh>>()
                .get(&left)
                .unwrap()
                .indices()
                .unwrap()
                .len(),
            36
        );
        assert!(
            world
                .resource::<Assets<Mesh>>()
                .get(&right)
                .unwrap()
                .indices()
                .unwrap()
                .is_empty()
        );
        let entity = world.resource::<ChunkRenders>().0[&ChunkPos([1, 0, 0])].entity;
        assert_eq!(
            *world.get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
        edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Stone);
    }
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
    assert_eq!(app.world().resource::<ChunkRenders>().0.len(), 2);
    assert_eq!(
        app.world().resource::<ChunkRenders>().0[&ChunkPos([1, 0, 0])].mesh,
        right
    );
}

#[test]
fn idle_frames_do_not_remesh_and_negative_seams_refresh_both_chunks() {
    let mut app = terrain_app();
    edit_cell(&mut app, [-1, 1, 1], voxel_core::Block::Stone);
    edit_cell(&mut app, [0, 1, 1], voxel_core::Block::Stone);
    let handle = app.world().resource::<ChunkRenders>().0[&ChunkPos([-1, 0, 0])]
        .mesh
        .clone();
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .indices()
            .unwrap()
            .len(),
        30
    );
    // A sentinel bound is preserved on idle frames, removed only on a rebuild.
    let entity = app.world().resource::<ChunkRenders>().0[&ChunkPos([-1, 0, 0])].entity;
    app.world_mut().entity_mut(entity).insert(Aabb::default());
    app.update();
    assert!(app.world().get::<Aabb>(entity).is_some());
    edit_cell(&mut app, [0, 1, 1], voxel_core::Block::Air);
    assert!(app.world().get::<Aabb>(entity).is_none());
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .indices()
            .unwrap()
            .len(),
        36
    );
}
