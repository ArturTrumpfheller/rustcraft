use super::*;
use crate::{
    player::{ControllerInput, PlayerPlugin},
    scene::VoxelScenePlugin,
};
use bevy::time::TimeUpdateStrategy;
use std::time::Instant;
use voxel_core::{Block, BlockPos, ChunkPos};
fn app(radius: i32) -> App {
    let mut world = streaming::new_world(42);
    let generator = streaming::prepare_spawn(&mut world).unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .insert_resource(StreamState::new(generator, radius))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_plugins((VoxelScenePlugin, PlayerPlugin, StreamingPlugin));
    app.update();
    app
}
fn drain(app: &mut App) {
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        app.update();
        let state = app.world().resource::<StreamState>();
        if state.queued.is_empty() && state.job.is_none() {
            break;
        }
        assert!(Instant::now() < end, "bounded streaming workload timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn go(app: &mut App, center: Column) {
    app.world_mut().resource_mut::<PlayerState>().body.position = [
        f64::from(center[0] * 16) + 0.5,
        24.,
        f64::from(center[1] * 16) + 0.5,
    ];
    app.update();
    drain(app);
}
fn edit(app: &mut App, p: BlockPos, b: Block) {
    let change = app
        .world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(p, b)
        .unwrap()
        .unwrap();
    app.world_mut()
        .resource_mut::<DirtyChunks>()
        .0
        .extend(change.affected_chunks());
}
#[test]
fn moving_radius_releases_entities_assets_and_cpu_chunks_but_keeps_unsaved_air_edits() {
    let start = Instant::now();
    let mut app = app(1);
    drain(&mut app);
    let p = BlockPos([0, -15, 0]);
    edit(&mut app, p, Block::Air);
    drain(&mut app);
    go(&mut app, [-3, -3]);
    let remote = BlockPos([-40, 25, -40]);
    edit(&mut app, remote, Block::Wood);
    drain(&mut app);
    let stable = app.world().resource::<VoxelWorld>().0.revision();
    for center in [[3, 3], [-3, -3], [3, 3], [0, 0]] {
        go(&mut app, center);
        let world = app.world_mut();
        let count = world.resource::<VoxelWorld>().0.chunks().count();
        assert!(count <= 30);
        let rendered = world.resource::<ChunkRenders>().0.len();
        assert!(rendered <= count);
        assert_eq!(world.resource::<Assets<Mesh>>().len(), rendered);
        assert_eq!(world.query::<&Mesh3d>().iter(world).count(), rendered);
        assert_eq!(world.resource::<VoxelWorld>().0.revision(), stable);
    }
    assert_eq!(app.world().resource::<VoxelWorld>().0.block(p), Block::Air);
    assert!(!app.world().resource::<VoxelWorld>().0.ready(remote));
    go(&mut app, [-3, -3]);
    assert_eq!(
        app.world().resource::<VoxelWorld>().0.block(remote),
        Block::Wood
    );
    let state = app.world().resource::<StreamState>();
    println!(
        "STREAM_WORKLOAD elapsed={:?} peak_chunks={} max_worker={:?} terrain_buffer_bytes={} installed={} discarded={}",
        start.elapsed(),
        state.peak_chunks,
        state.max_job,
        state.mesh_bytes,
        state.completed,
        state.discarded
    );
    assert!(state.peak_chunks <= 30);
    assert!(state.completed > 30);
}
#[test]
fn obsolete_or_stale_worker_results_never_overwrite_edits_or_recreate_evicted_gpu_assets() {
    let mut app = app(1);
    drain(&mut app);
    let generator = app
        .world()
        .resource::<StreamState>()
        .generator
        .clone()
        .unwrap();
    let revision = app.world().resource::<VoxelWorld>().0.revision();
    let edits = app.world().resource::<VoxelWorld>().0.edits();
    let (tx, rx) = std::sync::mpsc::channel();
    app.world_mut().resource_mut::<StreamState>().job = Some(thread::spawn(move || {
        rx.recv().unwrap();
        streaming::build_column(&generator, edits, [0, 0], revision)
    }));
    edit(&mut app, BlockPos([0, 25, 0]), Block::Leaves);
    app.world_mut().resource_mut::<PlayerState>().body.position = [48.5, 24., 48.5];
    app.update();
    tx.send(()).unwrap();
    drain(&mut app);
    assert!(app.world().resource::<StreamState>().discarded >= 1);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([0, 25, 0])),
        Block::Leaves
    );
    assert!(app.world().resource::<ChunkRenders>().0.len() <= 30);
    // Pin still exists but ancient center-neighbor columns have been evicted.
    assert!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .chunk(ChunkPos([-1, 0, -1]))
            .is_none()
    );
}
#[test]
fn pending_teleport_freezes_then_recovers_clearance_and_void_reset_uses_pinned_spawn() {
    let mut app = app(1);
    drain(&mut app);
    app.world_mut().resource_mut::<ControllerInput>().active = true;
    app.world_mut().resource_mut::<PlayerState>().body.position = [48.5, -10., 48.5];
    let before = app.world().resource::<PlayerState>().body;
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<PlayerState>().body, before);
    go(&mut app, [3, 3]); // Load destination; then teleport into its rock.
    app.world_mut().resource_mut::<PlayerState>().body.position = [48.5, -10., 48.5];
    app.world_mut().run_schedule(FixedUpdate);
    let world = app.world();
    let body = world.resource::<PlayerState>().body;
    assert!(body.position[1] > -10.);
    assert!(body.fits_in_world(
        &world.resource::<VoxelWorld>().0,
        &voxel_core::physics::PhysicsConfig::default()
    ));
    app.world_mut().resource_mut::<PlayerState>().body.position[1] = -33.;
    app.world_mut().run_schedule(FixedUpdate);
    assert!(app.world().resource::<PlayerState>().body.position[0].abs() < 2.);
}
#[test]
fn legacy_snapshots_do_not_stream_and_radius_configuration_is_bounded() {
    let mut app = App::new();
    app.insert_resource(VoxelWorld(voxel_core::demo_world()))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<PlayerState>()
        .add_plugins((VoxelScenePlugin, StreamingPlugin));
    app.update();
    assert_eq!(app.world().resource::<VoxelWorld>().0.chunks().count(), 4);
    assert!(app.world().resource::<StreamState>().job.is_none());
    assert_eq!(radius_from(None).unwrap(), 2);
    for r in ["1", "2", "3"] {
        assert!(radius_from(Some(r.into())).is_ok());
    }
    for r in ["0", "4", "-1", "abc", ""] {
        assert!(radius_from(Some(r.into())).is_err());
    }
}

#[test]
fn empty_streamed_geometry_releases_assets_and_refilling_does_not_leak_handles() {
    let mut app = app(1);
    drain(&mut app);
    let before = app.world().resource::<Assets<Mesh>>().len();
    let renders = app.world().resource::<ChunkRenders>();
    let chunk = app
        .world()
        .resource::<VoxelWorld>()
        .0
        .chunks()
        .find(|(p, _)| p.0[1] == 1 && !renders.0.contains_key(p))
        .unwrap()
        .0;
    let point = BlockPos([chunk.0[0] * 16 + 8, 31, chunk.0[2] * 16 + 8]);
    for _ in 0..8 {
        edit(&mut app, point, Block::Leaves);
        drain(&mut app);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), before + 1);
        edit(&mut app, point, Block::Air);
        drain(&mut app);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), before);
    }
    assert!(app.world().resource::<VoxelWorld>().0.ready(point));
}

#[test]
fn no_longer_desired_job_is_discarded_even_when_edit_revision_is_current() {
    let mut app = app(1);
    drain(&mut app);
    let generator = app
        .world()
        .resource::<StreamState>()
        .generator
        .clone()
        .unwrap();
    let edits = app.world().resource::<VoxelWorld>().0.edits();
    let (tx, rx) = std::sync::mpsc::channel();
    app.world_mut().resource_mut::<StreamState>().job = Some(thread::spawn(move || {
        rx.recv().unwrap();
        streaming::build_column(&generator, edits, [-1, -1], 0)
    }));
    app.world_mut().resource_mut::<PlayerState>().body.position = [48.5, 24., 48.5];
    app.update();
    tx.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.world().resource::<VoxelWorld>().0.revision(), 0);
    assert!(app.world().resource::<StreamState>().discarded >= 1);
    assert!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .chunk(ChunkPos([-1, 0, -1]))
            .is_none()
    );
    assert!(
        !app.world()
            .resource::<ChunkRenders>()
            .0
            .contains_key(&ChunkPos([-1, 0, -1]))
    );
}
