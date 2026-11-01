use super::*;
fn gated(app: &mut App, work: Work) -> std::sync::mpsc::Sender<()> {
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let cache = std::mem::take(&mut app.world_mut().resource_mut::<StreamState>().cache);
    let world = &app.world().resource::<VoxelWorld>().0;
    let state = app.world().resource::<StreamState>();
    let job = StreamJob::spawn(
        work,
        world,
        state.generator.clone().unwrap(),
        app.world()
            .get_resource::<crate::persistence::SaveSession>()
            .map(|s| s.store.clone()),
        cache,
        state.lights.clone(),
        BTreeSet::new(),
        false,
        Some((entered_tx, release_rx)),
    )
    .unwrap();
    app.world_mut().resource_mut::<StreamState>().job = Some(job);
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    release_tx
}
fn join_cancelled(app: &mut App) -> JobResult {
    let job = app
        .world_mut()
        .resource_mut::<StreamState>()
        .job
        .take()
        .unwrap();
    assert!(job.control.is_cancelled());
    job.handle.join().unwrap().unwrap()
}
#[test]
fn cancellation_before_regional_reads_returns_cache_and_never_installs_evicted_assets() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    let cached = app.world().resource::<StreamState>().cache.columns();
    assert!(cached > 0);
    let release = gated(&mut app, Work::Load([0, 0]));
    app.world_mut().resource_mut::<PlayerState>().body.position = [1600.5, 24., -1600.5];
    app.update();
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    release.send(()).unwrap();
    let payload = join_cancelled(&mut app);
    assert!(matches!(
        payload.result,
        Err(JobError::Cancelled(streaming::Cancelled(
            streaming::WorkStage::Regions
        )))
    ));
    assert_eq!(payload.cache.columns(), cached);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
    assert!(
        !app.world()
            .resource::<VoxelWorld>()
            .0
            .ready(BlockPos([0, 0, 0]))
    );
    app.world_mut().resource_mut::<StreamState>().cache = payload.cache;
    drain(&mut app);
    assert_textured_residency(&mut app);
}
#[test]
fn burst_edits_cancel_old_work_preserve_first_age_and_publish_current_connected_meshes() {
    let mut app = app(1);
    drain(&mut app);
    let targets = BTreeSet::from([ChunkPos([0, 1, 0]), ChunkPos([1, 1, 0])]);
    edit(&mut app, BlockPos([15, 25, 8]), Block::Wood);
    edit(&mut app, BlockPos([16, 25, 8]), Block::Wood);
    app.update();
    drain(&mut app);
    let release = gated(&mut app, Work::Mesh(targets.clone()));
    for block in [Block::Air, Block::Leaves, Block::Wood] {
        edit(&mut app, BlockPos([15, 25, 8]), block);
        edit(&mut app, BlockPos([16, 25, 8]), block);
        app.update();
    }
    let first = app.world().resource::<StreamState>().dirty[&ChunkPos([0, 1, 0])];
    let count = app.world().resource::<StreamState>().dirty.len();
    release.send(()).unwrap();
    let payload = join_cancelled(&mut app);
    assert!(matches!(payload.result, Err(JobError::Cancelled(_))));
    assert_eq!(app.world().resource::<StreamState>().dirty.len(), count);
    assert_eq!(
        app.world().resource::<StreamState>().dirty[&ChunkPos([0, 1, 0])],
        first
    );
    app.world_mut().resource_mut::<StreamState>().cache = payload.cache;
    drain(&mut app);
    assert_geometry(&app, &targets);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([16, 25, 8])),
        Block::Wood
    );
}
#[test]
fn drain_requests_stop_then_joins_and_recovers_owned_cache_without_detaching() {
    let mut app = app(1);
    drain(&mut app);
    let cached = app.world().resource::<StreamState>().cache.columns();
    let release = gated(&mut app, Work::Load([0, 0]));
    assert!(!app.world_mut().resource_mut::<StreamState>().drain());
    assert!(
        app.world()
            .resource::<StreamState>()
            .job
            .as_ref()
            .unwrap()
            .control
            .is_cancelled()
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.world_mut().resource_mut::<StreamState>().drain() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let state = app.world().resource::<StreamState>();
    assert!(state.job.is_none());
    assert_eq!(state.cache.columns(), cached);
    assert_eq!(state.metrics.cancelled, 1);
}
#[test]
fn continuous_signed_motion_reversals_diagonals_and_radius_changes_keep_bounded_residency() {
    let mut app = app(1);
    drain(&mut app);
    for (i, center) in [
        [-1, 0],
        [-2, 0],
        [-2, -1],
        [-3, -2],
        [-2, -1],
        [-1, 0],
        [2, 2],
        [-3, -3],
        [0, 0],
    ]
    .into_iter()
    .enumerate()
    {
        app.world_mut()
            .resource_mut::<StreamState>()
            .set_radius((i % 3 + 1) as i32);
        app.world_mut().resource_mut::<PlayerState>().body.velocity =
            [if i % 2 == 0 { 4.317 } else { -5.612 }, 0., 4.317];
        go(&mut app, center);
        let s = app.world().resource::<StreamState>();
        let w = &app.world().resource::<VoxelWorld>().0;
        assert!(w.chunk_count() <= 49 * 3);
        assert!(s.cache.columns() <= 81);
        assert!(s.queued.is_empty() && s.dirty.is_empty() && s.job.is_none());
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            app.world().resource::<ChunkRenders>().0.len()
        );
        assert!(s.first_load_dispatch.is_empty() && s.first_mesh_dispatch.is_empty());
    }
}
