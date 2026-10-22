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
        if state.queued.is_empty() && state.dirty.is_empty() && state.job.is_none() {
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

fn blocked_column(app: &mut App, column: Column) -> std::sync::mpsc::Sender<()> {
    let world = &app.world().resource::<VoxelWorld>().0;
    let edits = world.overlay_snapshot();
    let revision = world.revision();
    let work = Work::Load(column);
    let versions = work
        .targets()
        .into_iter()
        .map(|p| (p, world.mesh_revision(p)))
        .collect();
    let generator = app
        .world()
        .resource::<StreamState>()
        .generator
        .clone()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        rx.recv().unwrap();
        Ok(JobResult {
            result: WorkResult::Load(streaming::build_column(
                &generator,
                edits.clone(),
                column,
                revision,
            )),
            overlays: edits,
        })
    });
    app.world_mut().resource_mut::<StreamState>().job = Some(StreamJob {
        handle,
        versions,
        work,
    });
    tx
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
        assert!(count <= 27);
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
    assert!(state.peak_chunks <= 27);
    assert!(state.completed > 30);
}
#[test]
fn obsolete_or_stale_worker_results_never_overwrite_edits_or_recreate_evicted_gpu_assets() {
    let mut app = app(1);
    drain(&mut app);
    let tx = blocked_column(&mut app, [0, 0]);
    edit(&mut app, BlockPos([0, 25, 0]), Block::Leaves);
    app.world_mut().resource_mut::<PlayerState>().body.position = [48.5, 24., 48.5];
    app.update();
    tx.send(()).unwrap();
    drain(&mut app);
    assert!(app.world().resource::<StreamState>().discarded >= 1);
    assert_eq!(
        app.world().resource::<VoxelWorld>().0.edits()[&BlockPos([0, 25, 0])],
        Block::Leaves
    );
    assert!(app.world().resource::<ChunkRenders>().0.len() <= 30);
    // The spawn pin is gone along with ancient center-neighbor columns.
    assert!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .chunk(ChunkPos([-1, 0, -1]))
            .is_none()
    );
}
#[test]
fn pending_teleport_freezes_then_recovers_clearance_and_void_reset_reloads_spawn() {
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
    assert!(app.world().resource::<PlayerState>().awaiting_spawn);
    let frozen = app.world().resource::<PlayerState>().body;
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<PlayerState>().body, frozen);
    app.update();
    drain(&mut app);
    app.world_mut().run_schedule(FixedUpdate);
    let player = app.world().resource::<PlayerState>();
    assert!(!player.awaiting_spawn);
    assert_eq!(player.previous_position, player.body.position);
    assert!(player.body.fits_in_world(
        &app.world().resource::<VoxelWorld>().0,
        &voxel_core::physics::PhysicsConfig::default()
    ));
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
    let tx = blocked_column(&mut app, [-1, -1]);
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

#[test]
fn repeated_seam_mining_workload() {
    let mut app = app(1);
    drain(&mut app);
    let before = app.world().resource::<StreamState>().completed;
    let generated = app.world().resource::<StreamState>().generated_columns;
    let remeshed = app.world().resource::<StreamState>().remeshed_chunks;
    let sampled = app.world().resource::<StreamState>().sampled_border_cells;
    let initial_assets = app.world().resource::<Assets<Mesh>>().len();
    let initial_mesh_bytes = app.world().resource::<StreamState>().mesh_bytes;
    let start = Instant::now();
    let mut max_update = Duration::ZERO;
    for _ in 0..16 {
        for block in [Block::Wood, Block::Air] {
            for x in 14..18 {
                edit(&mut app, BlockPos([x, 25, 8]), block);
            }
            let frame = Instant::now();
            app.update();
            max_update = max_update.max(frame.elapsed());
            drain(&mut app);
        }
    }
    let elapsed = start.elapsed();
    assert_geometry(
        &app,
        &BTreeSet::from([ChunkPos([0, 1, 0]), ChunkPos([1, 1, 0])]),
    );
    let state = app.world().resource::<StreamState>();
    assert_eq!(
        state.generated_columns, generated,
        "mining cached blocks must not regenerate columns"
    );
    assert_eq!(state.remeshed_chunks - remeshed, 64);
    assert_eq!(
        state.sampled_border_cells - sampled,
        32 * 256,
        "only one pending outer face per batch needs sampling"
    );
    assert_eq!(state.mesh_bytes, initial_mesh_bytes);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), initial_assets);
    println!(
        "SEAM_MINING elapsed={elapsed:?} installed={} discarded={} max_worker={:?} max_dispatch_frame={max_update:?} meshes={} chunks={}",
        state.completed - before,
        state.discarded,
        state.max_job,
        app.world().resource::<ChunkRenders>().0.len(),
        app.world().resource::<VoxelWorld>().0.chunks().count()
    );
}

fn assert_geometry(app: &App, targets: &BTreeSet<ChunkPos>) {
    use bevy::mesh::VertexAttributeValues;
    let state = app.world().resource::<StreamState>();
    let world = &app.world().resource::<VoxelWorld>().0;
    let renders = app.world().resource::<ChunkRenders>();
    let assets = app.world().resource::<Assets<Mesh>>();
    for column in targets
        .iter()
        .map(|p| [p.0[0], p.0[2]])
        .collect::<BTreeSet<_>>()
    {
        let reference = streaming::build_column(
            state.generator.as_ref().unwrap(),
            world.overlay_snapshot(),
            column,
            world.revision(),
        );
        for (p, _, mesh) in reference
            .chunks
            .into_iter()
            .filter(|(p, _, _)| targets.contains(p))
        {
            if mesh.indices.is_empty() {
                assert!(!renders.0.contains_key(&p));
            } else {
                let actual = assets.get(&renders.0[&p].mesh).unwrap();
                assert_eq!(
                    actual.attribute(Mesh::ATTRIBUTE_POSITION),
                    Some(&VertexAttributeValues::Float32x3(mesh.positions)),
                    "positions {p:?}"
                );
                assert_eq!(
                    actual.attribute(Mesh::ATTRIBUTE_NORMAL),
                    Some(&VertexAttributeValues::Float32x3(mesh.normals)),
                    "normals {p:?}"
                );
                assert_eq!(
                    actual.attribute(Mesh::ATTRIBUTE_COLOR),
                    Some(&VertexAttributeValues::Float32x4(mesh.colors)),
                    "colors {p:?}"
                );
                assert_eq!(
                    actual.indices().unwrap().iter().collect::<Vec<_>>(),
                    mesh.indices
                        .into_iter()
                        .map(|i| i as usize)
                        .collect::<Vec<_>>(),
                    "indices {p:?}"
                );
            }
        }
    }
}

fn wait_finished(app: &App) {
    let end = Instant::now() + Duration::from_secs(30);
    while !app
        .world()
        .resource::<StreamState>()
        .job
        .as_ref()
        .unwrap()
        .handle
        .is_finished()
    {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn newer_seam_edits_reject_the_entire_mesh_batch_then_publish_both_current_faces() {
    let mut app = app(1);
    drain(&mut app);
    let targets = BTreeSet::from([ChunkPos([0, 1, 0]), ChunkPos([1, 1, 0])]);
    edit(&mut app, BlockPos([15, 25, 8]), Block::Wood);
    edit(&mut app, BlockPos([16, 25, 8]), Block::Wood);
    let world = &app.world().resource::<VoxelWorld>().0;
    let snapshot = streaming::mesh_snapshot(world, &targets);
    let edits = world.overlay_snapshot();
    let versions = targets
        .iter()
        .map(|&p| (p, world.mesh_revision(p)))
        .collect();
    let generator = app
        .world()
        .resource::<StreamState>()
        .generator
        .clone()
        .unwrap();
    let work = Work::Mesh(targets.clone());
    let output_targets = targets.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        rx.recv().unwrap();
        Ok(JobResult {
            overlays: EditSnapshot::default(),
            result: WorkResult::Mesh(streaming::remesh_cached(
                &generator,
                snapshot,
                edits,
                output_targets,
            )),
        })
    });
    app.world_mut().resource_mut::<StreamState>().job = Some(StreamJob {
        handle,
        versions,
        work,
    });
    app.update();
    let bytes = app.world().resource::<StreamState>().mesh_bytes;
    let discarded = app.world().resource::<StreamState>().discarded;
    edit(&mut app, BlockPos([15, 25, 8]), Block::Air);
    edit(&mut app, BlockPos([16, 25, 8]), Block::Leaves);
    app.update();
    tx.send(()).unwrap();
    wait_finished(&app);
    app.update();
    let state = app.world().resource::<StreamState>();
    assert_eq!(state.discarded, discarded + 1);
    assert_eq!(
        state.mesh_bytes, bytes,
        "a stale partial batch must never publish"
    );
    assert_eq!(
        state.dirty.len(),
        2,
        "newer invalidations must survive rejection"
    );
    drain(&mut app);
    assert_geometry(&app, &targets);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([15, 25, 8])),
        Block::Air
    );
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([16, 25, 8])),
        Block::Leaves
    );
}

#[test]
fn dirty_components_coalesce_and_take_priority_over_loads_without_global_invalidation() {
    let mut app = app(1);
    drain(&mut app);
    let generated = app.world().resource::<StreamState>().generated_columns;
    let discarded = app.world().resource::<StreamState>().discarded;
    let remeshed = app.world().resource::<StreamState>().remeshed_chunks;
    let tx = blocked_column(&mut app, [-1, -1]);
    for n in 0..64 {
        let b = if n % 2 == 0 { Block::Wood } else { Block::Air };
        for x in 14..18 {
            edit(&mut app, BlockPos([x, 25, 8]), b);
        }
        app.update();
        assert_eq!(app.world().resource::<StreamState>().dirty.len(), 2);
    }
    tx.send(()).unwrap();
    wait_finished(&app);
    app.update();
    assert!(
        matches!(&app.world().resource::<StreamState>().job.as_ref().unwrap().work, Work::Mesh(targets) if targets.len() == 2)
    );
    drain(&mut app);
    let state = app.world().resource::<StreamState>();
    assert_eq!(state.generated_columns, generated);
    assert_eq!(
        state.remeshed_chunks - remeshed,
        2,
        "256 coalesced changes need one two-chunk mesh batch"
    );
    assert_eq!(state.discarded, discarded);
    assert_geometry(
        &app,
        &BTreeSet::from([ChunkPos([0, 1, 0]), ChunkPos([1, 1, 0])]),
    );

    let dirty = BTreeMap::from([
        (ChunkPos([-3, 1, -3]), Instant::now()),
        (ChunkPos([0, 1, 0]), Instant::now()),
        (ChunkPos([1, 1, 0]), Instant::now()),
        (ChunkPos([1, 0, 0]), Instant::now()),
    ]);
    assert_eq!(
        nearest_dirty(&dirty, [0., 24., 0.]),
        BTreeSet::from([
            ChunkPos([0, 1, 0]),
            ChunkPos([1, 1, 0]),
            ChunkPos([1, 0, 0])
        ])
    );

    // While new columns are queued, a current load is allowed to finish, then
    // nearby visible edits are dispatched before any other generation.
    let tx = blocked_column(&mut app, [2, 0]);
    app.world_mut().resource_mut::<PlayerState>().body.position = [16.5, 24., 0.5];
    edit(&mut app, BlockPos([8, 25, 8]), Block::Leaves);
    app.update();
    assert!(!app.world().resource::<StreamState>().queued.is_empty());
    tx.send(()).unwrap();
    wait_finished(&app);
    app.update();
    assert!(
        matches!(&app.world().resource::<StreamState>().job.as_ref().unwrap().work, Work::Mesh(targets) if targets.contains(&ChunkPos([0, 1, 0])))
    );
    drain(&mut app);
}

#[test]
fn explicit_respawn_reloads_edited_spawn_and_preserves_edits_without_interpolated_teleport() {
    let mut app = app(1);
    drain(&mut app);
    let start = app.world().resource::<PlayerState>().body.position;
    let p = BlockPos([
        start[0].floor() as i32,
        start[1].floor() as i32,
        start[2].floor() as i32,
    ]);
    for y in 0..3 {
        edit(
            &mut app,
            BlockPos([p.0[0], p.0[1] + y, p.0[2]]),
            Block::Stone,
        );
    }
    drain(&mut app);
    let bytes = voxel_core::persistence::encode(&app.world().resource::<VoxelWorld>().0).unwrap();
    for void_reset in [false, true] {
        go(&mut app, [3, 3]);
        assert!(!app.world().resource::<VoxelWorld>().0.ready(p));
        let mut input = app.world_mut().resource_mut::<ControllerInput>();
        input.active = true;
        input.reset_pending = !void_reset;
        input.jump_pending = true;
        if void_reset {
            app.world_mut().resource_mut::<PlayerState>().body.position[1] = -33.;
        }
        app.world_mut().run_schedule(FixedUpdate);
        let waiting = app.world().resource::<PlayerState>();
        assert!(waiting.awaiting_spawn);
        assert_eq!(waiting.previous_position, waiting.body.position);
        assert_eq!(waiting.body.velocity, [0.; 3]);
        assert!(waiting.body.position[1] >= 32.);
        app.world_mut().run_schedule(FixedUpdate);
        app.update();
        drain(&mut app);
        app.world_mut().run_schedule(FixedUpdate);
        let player = app.world().resource::<PlayerState>();
        assert!(!player.awaiting_spawn);
        assert_eq!(player.previous_position, player.body.position);
        assert_eq!(player.body.position[1], f64::from(p.0[1] + 3));
        assert_eq!(
            voxel_core::persistence::encode(&app.world().resource::<VoxelWorld>().0).unwrap(),
            bytes
        );
    }
}

#[test]
fn remote_spawn_is_evicted_with_its_meshes_and_assets() {
    let mut app = app(1);
    drain(&mut app);
    go(&mut app, [3, 3]);
    let world = app.world();
    assert!(
        world
            .resource::<VoxelWorld>()
            .0
            .chunk(ChunkPos([0, 0, 0]))
            .is_none()
    );
    assert!(
        world
            .resource::<ChunkRenders>()
            .0
            .keys()
            .all(|p| p.0[0] != 0 || p.0[2] != 0)
    );
    assert_eq!(
        world.resource::<Assets<Mesh>>().len(),
        world.resource::<ChunkRenders>().0.len()
    );
}

#[test]
fn unrelated_edit_does_not_discard_an_in_flight_column() {
    let mut app = app(1);
    drain(&mut app);
    let state = app.world().resource::<StreamState>();
    let discarded = state.discarded;
    let tx = blocked_column(&mut app, [-1, -1]);
    edit(&mut app, BlockPos([24, 25, 24]), Block::Leaves);
    app.update();
    tx.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.world().resource::<StreamState>().discarded, discarded);
}

#[test]
fn surface_seam_mining_metrics() {
    let mut app = app(1);
    drain(&mut app);
    let generator = app
        .world()
        .resource::<StreamState>()
        .generator
        .clone()
        .unwrap();
    let cells: Vec<_> = (14..18)
        .map(|x| {
            let p = BlockPos([x, generator.height(x, 8).unwrap(), 8]);
            let b = app.world().resource::<VoxelWorld>().0.block(p);
            assert!(b.is_solid());
            (p, b)
        })
        .collect();
    let before = app.world().resource::<StreamState>().completed;
    let discarded = app.world().resource::<StreamState>().discarded;
    app.world_mut().resource_mut::<StreamState>().max_job = Duration::ZERO;
    let mut frames = Vec::new();
    let start = Instant::now();
    for _ in 0..16 {
        for restore in [false, true] {
            for &(p, b) in &cells {
                edit(&mut app, p, if restore { b } else { Block::Air });
            }
            loop {
                let frame = Instant::now();
                app.update();
                frames.push(frame.elapsed());
                let s = app.world().resource::<StreamState>();
                if s.queued.is_empty() && s.job.is_none() {
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(60));
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
    let elapsed = start.elapsed();
    frames.sort_unstable();
    let s = app.world().resource::<StreamState>();
    println!(
        "SURFACE_MINING elapsed={elapsed:?} installed={} discarded={} max_worker={:?} frame_p50={:?} frame_p95={:?} frame_p99={:?} frame_max={:?} samples={} mesh_bytes={}",
        s.completed - before,
        s.discarded - discarded,
        s.max_job,
        frames[frames.len() / 2],
        frames[frames.len() * 95 / 100],
        frames[frames.len() * 99 / 100],
        frames.last().unwrap(),
        frames.len(),
        s.mesh_bytes
    );
    for (p, b) in cells {
        assert_eq!(app.world().resource::<VoxelWorld>().0.block(p), b);
    }
}

#[test]
fn moving_center_during_a_load_does_not_generate_the_same_column_twice() {
    let mut app = app(1);
    drain(&mut app);
    let tx = blocked_column(&mut app, [2, 0]);
    for x in [16.5, 32.5] {
        app.world_mut().resource_mut::<PlayerState>().body.position = [x, 24., 0.5];
        app.update();
    }
    assert!(
        app.world()
            .resource::<StreamState>()
            .queued
            .contains(&[2, 0])
    );
    tx.send(()).unwrap();
    wait_finished(&app);
    app.update();
    let state = app.world().resource::<StreamState>();
    assert!(!state.queued.contains(&[2, 0]));
    assert!(!matches!(
        &state.job.as_ref().unwrap().work,
        Work::Load([2, 0])
    ));
    drain(&mut app);
}

struct RegionalDir(std::path::PathBuf);
impl RegionalDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rustcraft-large-ecs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for RegionalDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn large_app(path: &std::path::Path) -> App {
    let (store, mut world, source) =
        SaveStore::open_with(path, || Ok(streaming::new_large_world(42))).unwrap();
    store.prepare_regional_spawn(&mut world).unwrap();
    let generator = streaming::prepare_spawn(&mut world).unwrap();
    let session = crate::persistence::SaveSession::new(store, &world, source);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .insert_resource(session)
        .insert_resource(StreamState::new(generator, 1))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins((
            VoxelScenePlugin,
            PlayerPlugin,
            StreamingPlugin,
            crate::persistence::PersistencePlugin,
        ));
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        PrimaryWindow,
    ));
    app.update();
    drain(&mut app);
    app
}
fn save_large(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        if app.world().resource::<VoxelWorld>().0.dirty_regions() == 0
            && app
                .world()
                .resource::<crate::persistence::SaveFeedback>()
                .label
                == "Saved"
        {
            break;
        }
        assert!(Instant::now() < end, "save timed out");
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn large_world_travel_bounds_cpu_gpu_overlays_and_keeps_far_edits_across_restart() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    save_large(&mut app);
    let start = Instant::now();
    let mut timings = Vec::new();
    for (center, p) in [
        ([1000, -1000], BlockPos([16000, -15, -16000])),
        ([-1000, 1000], BlockPos([-16000, 25, 16000])),
    ] {
        go(&mut app, center);
        edit(
            &mut app,
            p,
            if p.0[1] < 0 { Block::Air } else { Block::Wood },
        );
        drain(&mut app);
    }
    for center in [
        [2047, 2047],
        [-2048, -2048],
        [1500, -1500],
        [-1500, 1500],
        [0, 0],
        [1000, -1000],
        [-1000, 1000],
    ] {
        let t = Instant::now();
        go(&mut app, center);
        timings.push(t.elapsed());
        let w = app.world_mut();
        let chunks = w.resource::<VoxelWorld>().0.chunks().count();
        let renders = w.resource::<ChunkRenders>().0.len();
        assert!(chunks <= 27);
        assert!(w.resource::<VoxelWorld>().0.cached_regions() <= 16);
        assert_eq!(w.resource::<Assets<Mesh>>().len(), renders);
        assert_eq!(w.query::<&Mesh3d>().iter(w).count(), renders);
        assert!(w.resource::<StreamState>().queued.len() <= 9);
    }
    assert_eq!(app.world().resource::<VoxelWorld>().0.dirty_regions(), 2);
    save_large(&mut app);
    go(&mut app, [0, 0]);
    assert_eq!(
        app.world().resource::<VoxelWorld>().0.edit_count(),
        0,
        "clean far overlays must be evicted"
    );
    timings.sort_unstable();
    let s = app.world().resource::<StreamState>();
    println!(
        "LARGE_TRAVEL elapsed={:?} visit_p50={:?} visit_p95={:?} peak_chunks={} worker_max={:?} worker_total={:?} queue_peak={} overlays={} buffers={}",
        start.elapsed(),
        timings[timings.len() / 2],
        timings[timings.len() * 95 / 100],
        s.peak_chunks,
        s.max_job,
        s.worker_time,
        s.peak_pending,
        app.world().resource::<VoxelWorld>().0.cached_regions(),
        s.mesh_bytes
    );
    drop(app);
    let mut app = large_app(&dir.0);
    go(&mut app, [1000, -1000]);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([16000, -15, -16000])),
        Block::Air
    );
    go(&mut app, [-1000, 1000]);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([-16000, 25, 16000])),
        Block::Wood
    );
}
#[test]
fn regional_fast_seam_edits_coalesce_without_regeneration_and_recover_failed_saves() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    save_large(&mut app);
    go(&mut app, [3, 0]);
    let before = app.world().resource::<StreamState>().generated_columns;
    let targets = BTreeSet::from([ChunkPos([3, 1, 0]), ChunkPos([4, 1, 0])]);
    for _ in 0..16 {
        for b in [Block::Wood, Block::Air] {
            for x in 62..66 {
                edit(&mut app, BlockPos([x, 25, 8]), b);
            }
            drain(&mut app);
        }
    }
    assert_eq!(
        app.world().resource::<StreamState>().generated_columns,
        before
    );
    assert_geometry(&app, &targets);
    save_large(&mut app);
    let store = Arc::clone(
        &app.world()
            .resource::<crate::persistence::SaveSession>()
            .store,
    );
    let path = store.region_path(voxel_core::regions::RegionPos([1, 0]));
    let prior = std::fs::read(&path).unwrap();
    let backup = path.with_extension("rcw.bak");
    std::fs::create_dir(&backup).unwrap();
    edit(&mut app, BlockPos([64, 25, 8]), Block::Leaves);
    drain(&mut app);
    go(&mut app, [1000, 1000]);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let end = Instant::now() + Duration::from_secs(30);
    while app
        .world()
        .resource::<crate::persistence::SaveFeedback>()
        .label
        != "ERROR / DIRTY"
    {
        app.update();
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert_eq!(app.world().resource::<VoxelWorld>().0.dirty_regions(), 1);
    std::fs::remove_dir(&backup).unwrap();
    save_large(&mut app);
    go(&mut app, [3, 0]);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([64, 25, 8])),
        Block::Leaves
    );
    assert_geometry(&app, &targets);
}
#[test]
fn corrupt_far_region_is_a_visible_safe_barrier_and_explicit_retry_recovers() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    save_large(&mut app);
    let store = Arc::clone(
        &app.world()
            .resource::<crate::persistence::SaveSession>()
            .store,
    );
    let r = voxel_core::regions::RegionPos([25, -25]);
    let path = store.region_path(r);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"corrupt").unwrap();
    app.world_mut().resource_mut::<PlayerState>().body.position = [1600.5, 24., -1599.5];
    let end = Instant::now() + Duration::from_secs(30);
    while app.world().resource::<StreamFailure>().0.is_none() {
        app.update();
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<crate::persistence::SaveFeedback>()
            .label,
        "ERROR / LOAD"
    );
    let w = &app.world().resource::<VoxelWorld>().0;
    let p = BlockPos([1600, 24, -1600]);
    assert!(!w.ready(p));
    assert_eq!(w.block(p), Block::Stone);
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
    // This deliberately-created corrupt disposable file had no saved edits.
    // Remove it explicitly to retry as a never-edited region.
    std::fs::remove_file(&path).unwrap();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    drain(&mut app);
    app.update();
    assert!(app.world().resource::<StreamFailure>().0.is_none());
    assert!(app.world().resource::<VoxelWorld>().0.ready(p));
}

#[test]
fn unrelated_regional_edit_survives_accepted_worker_overlay_and_voxel_install() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    save_large(&mut app);
    go(&mut app, [3, 0]);
    let before = app.world().resource::<StreamState>().discarded;
    let tx = blocked_column(&mut app, [2, 0]);
    let p = BlockPos([72, 25, 8]);
    edit(&mut app, p, Block::Leaves);
    app.update();
    tx.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.world().resource::<StreamState>().discarded, before);
    let w = &app.world().resource::<VoxelWorld>().0;
    assert_eq!(w.block(p), Block::Leaves);
    assert_eq!(w.overlay_snapshot().get(&p), Some(&Block::Leaves));
    save_large(&mut app);
    go(&mut app, [1000, 1000]);
    go(&mut app, [3, 0]);
    assert_eq!(
        app.world().resource::<VoxelWorld>().0.block(p),
        Block::Leaves
    );
}
