use super::*;
use bevy::camera::primitives::Aabb;

#[test]
fn scene_plugin_builds_four_renderable_chunk_entities_with_one_embedded_atlas() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<TerrainSurface>>()
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
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(!mesh.indices().unwrap().is_empty());
    }
    assert_eq!(world.resource::<Assets<TerrainSurface>>().len(), 2);
    assert_eq!(world.query::<&DirectionalLight>().iter(world).count(), 0);
}

fn edit_cell(app: &mut App, position: [i32; 3], block: voxel_core::Block) {
    let change = app
        .world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(voxel_core::BlockPos(position), block)
        .unwrap()
        .unwrap();
    app.world_mut().resource_mut::<DirtyChunks>().mark(change);
    app.update();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.world().resource::<LegacyMeshing>().job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
        app.update();
    }
}

fn terrain_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<TerrainSurface>>()
        .add_plugins(VoxelScenePlugin);
    app.insert_resource(VoxelWorld(World::default()));
    app.update();
    app
}

#[test]
#[ignore = "CPU-only item presentation measurement; run alone with --ignored --nocapture"]
fn item_presentation_cpu_measurements() {
    for count in [0, 12] {
        let mut world = World::default();
        for x in 0..12 {
            let p = voxel_core::BlockPos([x, 1, 0]);
            world
                .set_block(voxel_core::BlockPos([x, 0, 0]), voxel_core::Block::Stone)
                .unwrap();
            if x < count {
                world.set_block(p, voxel_core::Block::Torch).unwrap();
                world.set_block(p, voxel_core::Block::WaterSource).unwrap();
            }
        }
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<TerrainSurface>>()
            .insert_resource(VoxelWorld(world))
            .add_plugins(VoxelScenePlugin);
        for _ in 0..60 {
            app.update();
        }
        let mut samples = Vec::new();
        for _ in 0..2000 {
            let start = std::time::Instant::now();
            app.update();
            samples.push(start.elapsed().as_secs_f64() * 1e6);
        }
        samples.sort_by(f64::total_cmp);
        let meshes = app.world().resource::<Assets<Mesh>>();
        println!(
            "ITEM_PRESENTATION count={count} update_p50_us={:.3} update_p95_us={:.3} meshes={} vertices={} entities={}",
            samples[1000],
            samples[1900],
            meshes.len(),
            meshes
                .iter()
                .map(|(_, m)| m.count_vertices())
                .sum::<usize>(),
            app.world_mut().query::<&Mesh3d>().iter(app.world()).count()
        );
    }
}

#[test]
fn repeated_boundary_edits_update_neighbors_and_release_empty_surfaces() {
    let mut app = terrain_app();
    edit_cell(&mut app, [15, 1, 1], voxel_core::Block::Stone);
    edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Dirt);
    let left = app.world().resource::<ChunkRenders>().0[&ChunkPos([0, 0, 0])]
        .mesh
        .clone();
    for _ in 0..50 {
        let right = app.world().resource::<ChunkRenders>().0[&ChunkPos([1, 0, 0])]
            .mesh
            .clone();
        let entity = app.world().resource::<ChunkRenders>().0[&ChunkPos([1, 0, 0])].entity;
        edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Air);
        assert!(
            !app.world()
                .resource::<ChunkRenders>()
                .0
                .contains_key(&ChunkPos([1, 0, 0]))
        );
        assert!(app.world().resource::<Assets<Mesh>>().get(&right).is_none());
        assert!(app.world().get_entity(entity).is_err());
        assert_eq!(
            app.world().resource::<ChunkRenders>().0[&ChunkPos([0, 0, 0])].mesh,
            left
        );
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&left)
                .unwrap()
                .indices()
                .unwrap()
                .len(),
            36
        );
        edit_cell(&mut app, [16, 1, 1], voxel_core::Block::Stone);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&left)
                .unwrap()
                .indices()
                .unwrap()
                .len(),
            30
        );
    }
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
    assert_eq!(app.world().resource::<Assets<TerrainSurface>>().len(), 2);
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
    // Retained bounds stay on the entity; the real visibility pipeline below
    // separately verifies their AssetChanged-driven refresh.
    let entity = app.world().resource::<ChunkRenders>().0[&ChunkPos([-1, 0, 0])].entity;
    app.world_mut().entity_mut(entity).insert(Aabb::default());
    app.update();
    assert!(app.world().get::<Aabb>(entity).is_some());
    edit_cell(&mut app, [0, 1, 1], voxel_core::Block::Air);
    assert!(app.world().get::<Aabb>(entity).is_some());
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

#[test]
fn all_materials_share_one_opaque_texture_and_mesh_layout_is_compact() {
    let mut app = terrain_app();
    for (i, b) in [
        voxel_core::Block::Grass,
        voxel_core::Block::Dirt,
        voxel_core::Block::Stone,
        voxel_core::Block::Wood,
        voxel_core::Block::Leaves,
    ]
    .into_iter()
    .enumerate()
    {
        edit_cell(&mut app, [i as i32 * 3, 1, 1], b);
    }
    let world = app.world_mut();
    assert_eq!(world.resource::<Assets<Image>>().len(), 2);
    assert_eq!(world.resource::<Assets<TerrainSurface>>().len(), 2);
    let terrain = &world.resource::<TerrainMaterial>().0;
    let material = world
        .resource::<Assets<TerrainSurface>>()
        .get(terrain)
        .unwrap();
    assert_eq!(material.base.base_color, Color::WHITE);
    assert_eq!(material.base.alpha_mode, AlphaMode::Opaque);
    assert!(
        world
            .resource::<Assets<Image>>()
            .get(material.base.base_color_texture.as_ref().unwrap())
            .is_some()
    );
    let h = world.resource::<ChunkRenders>().0[&ChunkPos([0, 0, 0])]
        .mesh
        .clone();
    let m = world.resource::<Assets<Mesh>>().get(&h).unwrap();
    assert_eq!(
        m.attribute(Mesh::ATTRIBUTE_COLOR).unwrap().len(),
        m.count_vertices()
    );
    assert!(material.base.unlit);
    assert_eq!(m.count_vertices(), 120);
    assert_eq!(m.attribute(Mesh::ATTRIBUTE_UV_0).unwrap().len(), 120);
    assert_eq!(
        m.get_mesh_vertex_buffer_layout(&mut bevy::mesh::MeshVertexBufferLayouts::default())
            .0
            .layout()
            .array_stride,
        56
    );
}

#[test]
fn stale_legacy_worker_retains_complete_old_mesh_until_current_batch_is_ready() {
    let mut app = terrain_app();
    edit_cell(&mut app, [1, 1, 1], voxel_core::Block::Stone);
    let snapshot = app.world().resource::<VoxelWorld>().0.clone();
    let revision = snapshot.revision();
    let (tx, rx) = std::sync::mpsc::channel();
    let job = std::thread::spawn(move || {
        rx.recv().unwrap();
        vec![(
            ChunkPos([0; 3]),
            voxel_core::mesh_chunk(&snapshot, ChunkPos([0; 3])),
        )]
    });
    app.world_mut().resource_mut::<LegacyMeshing>().job = Some(LegacyJob {
        revision,
        handle: job,
    });
    let change = app
        .world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(voxel_core::BlockPos([2, 1, 1]), voxel_core::Block::Wood)
        .unwrap()
        .unwrap();
    app.world_mut().resource_mut::<DirtyChunks>().mark(change);
    app.update();
    let handle = app.world().resource::<ChunkRenders>().0[&ChunkPos([0; 3])]
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
        36
    );
    tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !app
        .world()
        .resource::<LegacyMeshing>()
        .job
        .as_ref()
        .unwrap()
        .handle
        .is_finished()
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    app.update();
    assert!(!app.world().resource::<DirtyChunks>().0.is_empty());
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
    while app.world().resource::<LegacyMeshing>().pending() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .indices()
            .unwrap()
            .len(),
        60
    );
}

#[test]
fn live_fog_radius_changes_update_one_shared_material_without_remeshing() {
    use crate::atmosphere::{AtmosphereUniform, SkyAssets, SkyMaterial};
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<TerrainSurface>>()
        .init_resource::<crate::settings::Preferences>()
        .add_plugins(VoxelScenePlugin);
    app.world_mut().spawn(crate::camera::FirstPersonCamera);
    app.update();
    let terrain = app.world().resource::<TerrainMaterial>().0.clone();
    let sky = app.world().resource::<SkyAssets>().material.clone();
    let handles: Vec<_> = app
        .world()
        .resource::<ChunkRenders>()
        .0
        .values()
        .map(|r| r.mesh.clone())
        .collect();
    let count = app.world().resource::<Assets<Mesh>>().len();
    let revision = app.world().resource::<VoxelWorld>().0.revision();
    for radius in [1, 3, 2, 1, 3, 1] {
        app.world_mut()
            .resource_mut::<crate::settings::Preferences>()
            .radius = radius;
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<TerrainSurface>>()
                .get(&terrain)
                .unwrap()
                .extension
                .atmosphere,
            AtmosphereUniform::for_radius(radius)
        );
        assert_eq!(
            app.world()
                .resource::<Assets<SkyMaterial>>()
                .get(&sky)
                .unwrap()
                .atmosphere,
            AtmosphereUniform::for_radius(radius)
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
        assert_eq!(app.world().resource::<VoxelWorld>().0.revision(), revision);
        assert!(app.world().resource::<DirtyChunks>().0.is_empty());
        assert!(app.world().resource::<DirtyChunks>().1.is_empty());
        assert!(!app.world().resource::<LegacyMeshing>().pending());
        assert!(
            handles
                .iter()
                .all(|h| app.world().resource::<Assets<Mesh>>().get(h).is_some())
        );
    }
    // The actual active streaming radius wins over a saved preference, as it
    // does when RUSTCRAFT_LOAD_RADIUS overrides settings at session creation.
    let mut state = crate::streaming::StreamState::default();
    state.set_radius(3);
    app.insert_resource(state);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<TerrainSurface>>()
            .get(&terrain)
            .unwrap()
            .extension
            .atmosphere
            .fog
            .y,
        48.
    );
}

#[test]
fn water_uses_retained_separate_blended_meshes_and_releases_empty_surfaces() {
    let mut app = terrain_app();
    let cp = ChunkPos([0; 3]);
    edit_cell(&mut app, [2, 2, 2], voxel_core::Block::WaterSource);
    let water = app.world().resource::<ChunkRenders>().1[&cp].mesh.clone();
    let material = app.world().resource::<TerrainMaterial>().2.clone();
    assert_eq!(
        app.world()
            .resource::<Assets<crate::terrain_material::WaterSurface>>()
            .get(&material)
            .unwrap()
            .base
            .alpha_mode,
        AlphaMode::Blend
    );
    for _ in 0..20 {
        edit_cell(&mut app, [3, 2, 2], voxel_core::Block::Water1);
        assert_eq!(app.world().resource::<ChunkRenders>().1[&cp].mesh, water);
        edit_cell(&mut app, [3, 2, 2], voxel_core::Block::Air);
    }
    edit_cell(&mut app, [2, 2, 2], voxel_core::Block::Air);
    assert!(app.world().resource::<ChunkRenders>().1.is_empty());
    assert!(app.world().resource::<Assets<Mesh>>().get(&water).is_none());
    assert_eq!(
        app.world()
            .resource::<Assets<crate::terrain_material::WaterSurface>>()
            .len(),
        1
    );
}

#[test]
fn plants_use_one_cutout_surface_per_chunk_and_release_on_removal() {
    use voxel_core::Block;
    let mut app = terrain_app();
    let cp = ChunkPos([0, 0, 0]);
    edit_cell(&mut app, [1, 0, 1], Block::Grass);
    edit_cell(&mut app, [1, 1, 1], Block::RedFlower);
    edit_cell(&mut app, [2, 0, 1], Block::Grass);
    edit_cell(&mut app, [2, 1, 1], Block::TallGrass);
    let h = app.world().resource::<ChunkRenders>().2[&cp].mesh.clone();
    let w = app.world();
    let meshes = w.resource::<Assets<Mesh>>();
    let mesh = meshes.get(&h).unwrap();
    assert_eq!(mesh.count_vertices(), 16);
    assert_eq!(mesh.indices().unwrap().len(), 24);
    let handle = w.resource::<TerrainMaterial>();
    let plants = w
        .resource::<Assets<TerrainSurface>>()
        .get(&handle.3)
        .unwrap();
    assert_eq!(plants.base.alpha_mode, AlphaMode::Mask(0.5));
    assert_eq!(plants.base.cull_mode, None);
    assert!(plants.base.double_sided);
    assert_eq!(plants.extension.atlas, handle.1);
    assert_eq!(w.resource::<ChunkRenders>().2.len(), 1);
    edit_cell(&mut app, [1, 1, 1], Block::WaterSource);
    assert_eq!(app.world().resource::<ChunkRenders>().2[&cp].mesh, h);
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&h)
            .unwrap()
            .count_vertices(),
        8
    );
    edit_cell(&mut app, [2, 1, 1], Block::Air);
    assert!(app.world().resource::<ChunkRenders>().2.is_empty());
    assert!(app.world().resource::<Assets<Mesh>>().get(&h).is_none());
    assert!(!app.world().resource::<ChunkRenders>().1.is_empty());
}

#[path = "visibility_tests.rs"]
mod visibility_tests;
