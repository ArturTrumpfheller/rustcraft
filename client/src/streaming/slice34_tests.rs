// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;

fn bounds(app: &mut App) {
    assert_textured_residency(app);
    let state = app.world().resource::<StreamState>();
    let world = &app.world().resource::<VoxelWorld>().0;
    let height = world.world_height().chunk_count();
    assert_eq!(world.chunk_count(), state.desired.len() * height);
    assert!(world.allocated_voxel_bytes() <= state.desired.len() * height * 4096);
    let halo_columns = (2 * state.radius() + 3).pow(2) as usize;
    assert!(state.cache.columns() <= halo_columns);
    assert!(state.cache.allocated_voxel_bytes() <= halo_columns * height * 4096);
    assert!(state.lights.len() <= state.desired.len() * height);
    assert!(state.queued_at.is_empty() && state.dirty.is_empty());
    assert!(state.first_load_dispatch.is_empty() && state.first_mesh_dispatch.is_empty());
    assert!(state.job.is_none() && state.failures.is_empty());
    assert!(
        world
            .chunks()
            .all(|(p, _)| state.desired.contains(&[p.0[0], p.0[2]]))
    );
    let renders = app.world().resource::<ChunkRenders>();
    assert!(
        renders
            .0
            .keys()
            .chain(renders.1.keys())
            .chain(renders.2.keys())
            .all(|p| state.desired.contains(&[p.0[0], p.0[2]]))
    );
}

#[test]
fn radius_four_transitions_cancel_inflight_outer_work_without_revival_or_retention() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    app.add_systems(PostUpdate, crate::camera::rebase_scene);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    assert_eq!(
        app.world().resource::<VoxelWorld>().0.chunk_count(),
        81 * 16
    );
    let outer = ChunkPos([4, 1, 4]);
    let release = super::slice32::gated(&mut app, Work::Load([4, 4]));
    app.world_mut().resource_mut::<StreamState>().set_radius(1);
    app.update();
    assert!(
        !app.world()
            .resource::<VoxelWorld>()
            .0
            .ready(BlockPos([64, 25, 64]))
    );
    assert!(
        !app.world()
            .resource::<ChunkRenders>()
            .0
            .contains_key(&outer)
    );
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    app.update();
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
    drain(&mut app);
    assert!(app.world().resource::<StreamState>().metrics.cancelled > 0);
    for radius in [1, 4, 1, 4, 1] {
        app.world_mut()
            .resource_mut::<StreamState>()
            .set_radius(radius);
        drain(&mut app);
        bounds(&mut app);
        assert_eq!(
            app.world().resource::<VoxelWorld>().0.chunk_count(),
            (radius * 2 + 1).pow(2) as usize * 16
        );
    }
}

#[test]
fn radius_four_signed_travel_reversals_teleports_and_origin_shifts_converge_exactly() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    app.add_systems(PostUpdate, crate::camera::rebase_scene);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    let release = super::slice32::gated(&mut app, Work::Load([4, 4]));
    for center in [
        [-1, -1],
        [-2, 0],
        [1, -2],
        [-1990, 1990],
        [1990, -1990],
        [0, 0],
    ] {
        app.world_mut().resource_mut::<PlayerState>().body.position = [
            f64::from(center[0] * 16) + 0.5,
            24.,
            f64::from(center[1] * 16) + 0.5,
        ];
        app.update();
        let state = app.world().resource::<StreamState>();
        assert!(state.queued.len() <= 81 && state.dirty.len() <= 81 * 16);
    }
    release.send(()).unwrap();
    drain(&mut app);
    for center in [
        [-1990, 1990],
        [-1989, 1989],
        [-1990, 1990],
        [2047, -2048],
        [0, 0],
    ] {
        go(&mut app, center);
        bounds(&mut app);
        let world = &app.world().resource::<VoxelWorld>().0;
        let expected = streaming::desired_with_limit(center, 4, 2048);
        assert_eq!(world.chunk_count(), expected.len() * 16);
        let cp = ChunkPos([center[0], 1, center[1]]);
        assert_geometry(&app, &BTreeSet::from([cp]));
        let origin = app.world().resource::<RenderOrigin>().0;
        let renders = app.world().resource::<ChunkRenders>();
        for (p, r) in renders.0.iter().chain(&renders.1).chain(&renders.2) {
            let expected = Vec3::from_array(std::array::from_fn(|a| {
                (f64::from(p.0[a] * 16) - origin[a]) as f32
            }));
            assert_eq!(
                app.world().get::<Transform>(r.entity).unwrap().translation,
                expected
            );
        }
    }
}

#[test]
fn radius_four_seam_light_water_and_saved_air_survive_eviction_and_reload() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    // A supported torch and flowing water on both sides of x=0, in clear sky.
    for x in [-1, 0] {
        edit(&mut app, BlockPos([x, 127, 8]), Block::Stone);
        edit(&mut app, BlockPos([x, 128, 8]), Block::Torch);
        edit(&mut app, BlockPos([x, 128, 9]), Block::WaterFalling);
    }
    let deep = BlockPos([-1, -60, 8]);
    edit(&mut app, deep, Block::Air);
    drain(&mut app);
    let targets = BTreeSet::from([ChunkPos([-1, 8, 0]), ChunkPos([0, 8, 0])]);
    assert_geometry(&app, &targets);
    let release = super::slice32::gated(&mut app, Work::Mesh(targets.clone()));
    edit(&mut app, BlockPos([-1, 128, 8]), Block::Air);
    app.update();
    release.send(()).unwrap();
    drain(&mut app);
    assert_geometry(&app, &targets);
    save_large(&mut app);
    go(&mut app, [-100, 100]);
    bounds(&mut app);
    go(&mut app, [0, 0]);
    assert_geometry(&app, &targets);
    drop(app);
    let mut app = large_app(&dir.0);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    let world = &app.world().resource::<VoxelWorld>().0;
    assert_eq!(world.block(deep), Block::Air);
    assert_eq!(world.block(BlockPos([-1, 128, 8])), Block::Air);
    assert_eq!(world.block(BlockPos([0, 128, 8])), Block::Torch);
    assert_eq!(world.block(BlockPos([0, 128, 9])), Block::WaterFalling);
    assert_geometry(&app, &targets);
    bounds(&mut app);
}

#[test]
fn radius_four_pending_collision_is_safe_and_near_load_precedes_distant_view() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    let release = super::slice32::gated(&mut app, Work::Load([4, 4]));
    app.world_mut().resource_mut::<PlayerState>().body.position = [-1600.5, 128., 1600.5];
    app.update();
    let pending = BlockPos([-1601, 128, 1600]);
    assert!(!app.world().resource::<VoxelWorld>().0.ready(pending));
    assert!(
        app.world_mut()
            .resource_mut::<VoxelWorld>()
            .0
            .set_block(pending, Block::Stone)
            .is_err()
    );
    let before = app.world().resource::<PlayerState>().body.position;
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1. / 60.,
    )));
    app.world_mut()
        .resource_mut::<ControllerInput>()
        .simulation_active = true;
    app.update();
    assert_eq!(app.world().resource::<PlayerState>().body.position, before);
    app.world_mut()
        .resource_mut::<ControllerInput>()
        .simulation_active = false;
    release.send(()).unwrap();
    wait_finished(&app);
    app.update();
    assert!(matches!(
        app.world()
            .resource::<StreamState>()
            .job
            .as_ref()
            .map(|j| &j.work),
        Some(Work::Load([-101, 100]))
    ));
    drain(&mut app);
    assert!(app.world().resource::<VoxelWorld>().0.ready(pending));
    bounds(&mut app);
}

#[test]
fn radius_four_connected_edits_span_the_entire_light_domain_and_publish_together() {
    let dir = RegionalDir::new();
    let mut app = large_app(&dir.0);
    app.world_mut().resource_mut::<StreamState>().set_radius(4);
    drain(&mut app);
    // Overlapping light neighborhoods join the diagonal into one unit, spanning
    // nine columns on both axes without exceeding the existing dirty-region cap.
    let mut targets = BTreeSet::new();
    for c in -3..=3 {
        edit(
            &mut app,
            BlockPos([c * 16 + 8, 128, c * 16 + 8]),
            Block::Stone,
        );
        targets.insert(ChunkPos([c, 8, c]));
    }
    app.update();
    let state = app.world().resource::<StreamState>();
    let Some(Work::Mesh(batch)) = state.job.as_ref().map(|j| &j.work) else {
        panic!("connected mesh job expected");
    };
    for axis in [0, 2] {
        assert_eq!(batch.iter().map(|p| p.0[axis]).min(), Some(-4));
        assert_eq!(batch.iter().map(|p| p.0[axis]).max(), Some(4));
    }
    drain(&mut app);
    assert_geometry(&app, &targets);
    bounds(&mut app);
    let state = app.world().resource::<StreamState>();
    println!(
        "RADIUS4_CONNECTED max_worker_wall_ms={:.3} max_compute_wall_ms={:.3} max_install_ms={:.3} max_stream_update_ms={:.3} light_bytes={} mesh_bytes={}",
        state.metrics.execution.max.as_secs_f64() * 1000.,
        state.metrics.compute.max.as_secs_f64() * 1000.,
        state.max_install.as_secs_f64() * 1000.,
        state.max_update.as_secs_f64() * 1000.,
        state.lights.values().map(|l| l.0.len()).sum::<usize>(),
        state.mesh_bytes
    );
}
