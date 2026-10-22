use super::*;
use crate::{
    player::{PlayerPlugin, SPAWN},
    scene::VoxelWorld,
};
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;
use voxel_core::physics::{PhysicsConfig, Player};

fn test_app() -> App {
    test_app_with_world(voxel_core::demo_world())
}
fn test_app_with_world(world: voxel_core::World) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .add_plugins((PlayerPlugin, FirstPersonPlugin));
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        CursorOptions::default(),
        PrimaryWindow,
    ));
    frame(&mut app, Duration::ZERO);
    app
}

fn frame(app: &mut App, duration: Duration) {
    *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
        TimeUpdateStrategy::ManualDuration(duration);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
}

fn tick(app: &mut App) {
    let duration = app.world().resource::<Time<Fixed>>().timestep();
    frame(app, duration);
}

fn capture(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(app, Duration::ZERO);
}

fn key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
}

fn camera(app: &mut App) -> Transform {
    *app.world_mut()
        .query_filtered::<&Transform, With<FirstPersonCamera>>()
        .single(app.world())
        .unwrap()
}

fn body(app: &App) -> Player {
    app.world().resource::<PlayerState>().body
}

fn settle(app: &mut App) {
    capture(app);
    for _ in 0..120 {
        tick(app);
    }
    assert!(body(app).grounded);
}

#[test]
fn click_escape_and_focus_transitions_keep_the_native_cursor_contract() {
    let mut app = test_app();
    let window = window(&mut app);
    capture(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    assert!(!app.world().get::<CursorOptions>(window).unwrap().visible);
    key(&mut app, KeyCode::Escape);
    tick(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::None
    );
    assert!(app.world().get::<CursorOptions>(window).unwrap().visible);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    capture(&mut app);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    tick(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::None
    );
    assert!(app.world().get::<CursorOptions>(window).unwrap().visible);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    tick(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::None
    );
}

#[test]
fn released_cursor_pauses_simulation_and_clears_pending_edges() {
    let mut app = test_app();
    settle(&mut app);
    key(&mut app, KeyCode::Space);
    frame(&mut app, Duration::ZERO);
    assert!(app.world().resource::<ControllerInput>().jump_pending);
    key(&mut app, KeyCode::Escape);
    key(&mut app, KeyCode::KeyW);
    let original = body(&app);
    for _ in 0..60 {
        tick(&mut app);
    }
    assert_eq!(body(&app), original);
    assert!(!app.world().resource::<ControllerInput>().jump_pending);
}

#[test]
fn jump_edge_survives_a_frame_without_physics_and_is_consumed_once() {
    let mut app = test_app();
    settle(&mut app);
    key(&mut app, KeyCode::Space);
    frame(&mut app, Duration::ZERO);
    assert!(app.world().resource::<ControllerInput>().jump_pending);
    let step = app.world().resource::<Time<Fixed>>().timestep();
    frame(&mut app, step * 3);
    assert!((body(&app).velocity[1] - 7.3).abs() < 1e-8);
    assert!(!app.world().resource::<ControllerInput>().jump_pending);
    for _ in 0..120 {
        tick(&mut app);
    }
    // Space stayed pressed the entire time: landing must not trigger auto-jump.
    assert!(body(&app).grounded);
    assert_eq!(body(&app).velocity[1], 0.0);
}

#[test]
fn fixed_simulation_matches_at_30_60_and_144_render_frames_per_second() {
    let mut results = Vec::new();
    for fps in [30, 60, 144] {
        let mut app = test_app();
        capture(&mut app);
        key(&mut app, KeyCode::KeyD);
        for frame_number in 1..=fps * 2 {
            let end = Duration::from_secs_f64(f64::from(frame_number) / f64::from(fps));
            let start = Duration::from_secs_f64(f64::from(frame_number - 1) / f64::from(fps));
            frame(&mut app, end - start);
        }
        let player = body(&app);
        assert!(player.position[0] > 8.0);
        let eye = player.eye_position(&PhysicsConfig::default());
        let rendered_eye = camera(&mut app).translation;
        assert!(rendered_eye.distance(Vec3::from_array(eye.map(|v| v as f32))) < 0.1);
        results.push(player);
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[1], results[2]);
}

#[test]
fn view_is_frame_rate_independent_pitch_clamped_and_cannot_move_the_body() {
    let mut rotations = Vec::new();
    for duration in [Duration::ZERO, Duration::from_millis(100)] {
        let mut app = test_app();
        settle(&mut app);
        let original = body(&app);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(20.0, -100_000.0);
        frame(&mut app, duration);
        let transform = camera(&mut app);
        assert!(transform.forward().y > 0.999 && transform.forward().y < 1.0);
        assert_eq!(body(&app).position, original.position);
        rotations.push(transform.rotation);
    }
    assert_eq!(rotations[0], rotations[1]);
}

#[test]
fn pre_capture_hover_motion_does_not_rotate_the_view() {
    let mut app = test_app();
    let original = camera(&mut app).rotation;
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::splat(1000.0);
    capture(&mut app);
    assert_eq!(camera(&mut app).rotation, original);
}

#[test]
fn reset_and_void_recovery_teleport_player_and_interpolation_together() {
    let mut app = test_app();
    settle(&mut app);
    app.world_mut().resource_mut::<PlayerState>().body.position = [12.0, 10.0, 12.0];
    key(&mut app, KeyCode::KeyR);
    tick(&mut app);
    assert_eq!(body(&app), Player::new(SPAWN));
    assert_eq!(
        app.world().resource::<PlayerState>().previous_position,
        SPAWN
    );
    let spawn_eye = Vec3::from_array(
        body(&app)
            .eye_position(&PhysicsConfig::default())
            .map(|v| v as f32),
    );
    assert_eq!(camera(&mut app).translation, spawn_eye);
    app.world_mut().resource_mut::<PlayerState>().body.position[1] = -40.0;
    tick(&mut app);
    assert_eq!(body(&app), Player::new(SPAWN));
    assert_eq!(camera(&mut app).translation, spawn_eye);
}

#[test]
fn reset_uses_the_reset_yaw_for_movement_in_the_same_render_frame() {
    let mut app = test_app();
    settle(&mut app);
    app.world_mut().resource_mut::<ViewAngles>().yaw = std::f32::consts::FRAC_PI_2;
    key(&mut app, KeyCode::KeyW);
    key(&mut app, KeyCode::KeyR);
    let step = app.world().resource::<Time<Fixed>>().timestep();
    frame(&mut app, step * 3);
    assert_eq!(body(&app).position[0], SPAWN[0]);
    assert!(body(&app).position[2] < SPAWN[2]);
}

#[test]
fn respawn_searches_above_blocks_built_at_the_old_spawn() {
    let mut app = test_app();
    settle(&mut app);
    for y in 4..=6 {
        app.world_mut()
            .resource_mut::<VoxelWorld>()
            .0
            .set_block(voxel_core::BlockPos([0, y, 10]), voxel_core::Block::Stone)
            .unwrap();
    }
    key(&mut app, KeyCode::KeyR);
    tick(&mut app);
    assert_eq!(body(&app).position, [0.5, 7.0, 10.5]);
    assert!(body(&app).fits_in_world(
        &app.world().resource::<VoxelWorld>().0,
        &PhysicsConfig::default()
    ));
    assert_eq!(
        app.world().resource::<PlayerState>().previous_position,
        body(&app).position
    );
}

#[test]
fn mining_clicks_do_not_suppress_mouse_look_after_capture() {
    let mut app = test_app();
    settle(&mut app);
    let previous = camera(&mut app).rotation;
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta
        .x = 20.0;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(&mut app, Duration::ZERO);
    assert_ne!(camera(&mut app).rotation, previous);
    assert!(
        !app.world()
            .resource::<ControllerInput>()
            .captured_this_frame
    );
}

#[test]
fn final_save_pauses_controls_and_a_canceled_close_can_resume() {
    let mut app = test_app();
    settle(&mut app);
    let original = body(&app);
    app.insert_resource(crate::persistence::SaveFeedback {
        label: "Saving".into(),
        closing: true,
    });
    key(&mut app, KeyCode::KeyW);
    key(&mut app, KeyCode::Space);
    tick(&mut app);
    assert_eq!(body(&app), original);
    assert!(!app.world().resource::<ControllerInput>().active);
    assert!(!app.world().resource::<ControllerInput>().jump_pending);
    app.world_mut()
        .resource_mut::<crate::persistence::SaveFeedback>()
        .closing = false;
    tick(&mut app);
    assert!(app.world().resource::<ControllerInput>().active);
    assert_ne!(body(&app).position, original.position);
    assert!(
        body(&app).grounded,
        "the stale jump edge must not survive a paused save"
    );
}

#[test]
fn generated_startup_reset_and_void_recovery_use_actual_ground_not_legacy_spawn_height() {
    let world =
        voxel_core::worldgen::Generator::new(voxel_core::worldgen::GenerationConfig::default())
            .unwrap()
            .generate();
    let expected = voxel_core::physics::spawn_position(&world, &PhysicsConfig::default(), SPAWN);
    assert_ne!(expected[1], SPAWN[1]);
    let mut app = test_app_with_world(world);
    assert_eq!(body(&app).position, expected);
    capture(&mut app);
    app.world_mut().resource_mut::<PlayerState>().body.position = [20., -33., 20.];
    tick(&mut app);
    assert_eq!(body(&app).position, expected);
    key(&mut app, KeyCode::KeyR);
    tick(&mut app);
    assert_eq!(body(&app).position, expected);
    assert_eq!(
        app.world().resource::<PlayerState>().previous_position,
        expected
    );
    assert!(body(&app).fits_in_world(
        &app.world().resource::<VoxelWorld>().0,
        &PhysicsConfig::default()
    ));
}

#[test]
fn diagonal_contact_does_not_trigger_clearance_recovery_or_camera_backtracking() {
    use voxel_core::{Block, BlockPos, Chunk, ChunkPos};
    let mut world = voxel_core::streaming::new_world(42);
    for x in -1..=1 {
        for z in -1..=1 {
            for y in -1..=1 {
                world.insert_chunk(ChunkPos([x, y, z]), Chunk::default());
            }
        }
    }
    for x in -8..=8 {
        for z in -8..=8 {
            world.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
        }
    }
    // The two blocks share only an edge. Clipping leaves x=2.3, whose
    // reconstructed minimum is 1.9999999999999998 in f64.
    for p in [[1, 1, 0], [2, 1, 1]] {
        world.set_block(BlockPos(p), Block::Stone).unwrap();
    }
    let mut app = test_app_with_world(world);
    let p = [3.5, 1., 0.5];
    *app.world_mut().resource_mut::<PlayerState>() = PlayerState {
        body: Player::new(p),
        previous_position: p,
        awaiting_spawn: false,
    };
    capture(&mut app);
    key(&mut app, KeyCode::KeyA);
    key(&mut app, KeyCode::KeyS);
    let mut previous = p;
    let mut eye = camera(&mut app).translation;
    for n in 0..180 {
        tick(&mut app);
        let position = body(&app).position;
        assert!(
            position[0] <= previous[0] + 1e-8,
            "tick {n}: {previous:?} -> {position:?}"
        );
        assert!(
            position[2] >= previous[2] - 1e-8,
            "tick {n}: {previous:?} -> {position:?}"
        );
        let next_eye = camera(&mut app).translation;
        assert!(next_eye.x <= eye.x + 1e-6 && next_eye.z >= eye.z - 1e-6);
        previous = position;
        eye = next_eye;
    }
    assert!((previous[0] - 2.3).abs() < 1e-8);
    assert!((previous[2] - 0.7).abs() < 1e-8);
}

#[test]
fn floating_origin_keeps_far_camera_and_chunk_transforms_precise_without_changing_world_pose() {
    use voxel_core::{Block, Chunk, ChunkPos, LocalPos, World};
    let mut world = World::default();
    let mut chunk = Chunk::default();
    chunk.set(LocalPos::new([0, 0, 0]).unwrap(), Block::Stone);
    world.insert_chunk(ChunkPos([1000, 0, -1000]), chunk);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .add_plugins((
            crate::scene::VoxelScenePlugin,
            PlayerPlugin,
            FirstPersonPlugin,
        ));
    app.update();
    for position in [
        [16000.5, 2., -15999.5],
        [16016.5, 2., -15983.5],
        [0.5, 2., 10.5],
    ] {
        *app.world_mut().resource_mut::<PlayerState>() = PlayerState {
            body: Player::new(position),
            previous_position: position,
            awaiting_spawn: false,
        };
        app.update();
        assert_eq!(body(&app).position, position);
        let origin = app.world().resource::<crate::scene::RenderOrigin>().0;
        let transform = camera(&mut app);
        assert!(transform.translation.x.abs() < 16. && transform.translation.z.abs() < 16.);
        let pose = app.world().resource::<ViewPose>();
        assert!((f64::from(transform.translation.x) + origin[0] - pose.eye[0]).abs() < 1e-6);
        let render =
            &app.world().resource::<crate::scene::ChunkRenders>().0[&ChunkPos([1000, 0, -1000])];
        let terrain = app.world().get::<Transform>(render.entity).unwrap();
        assert_eq!(f64::from(terrain.translation.x) + origin[0], 16000.);
        assert_eq!(f64::from(terrain.translation.z) + origin[2], -16000.);
    }
}
#[test]
fn far_start_configuration_is_bounded_and_does_not_accept_invalid_coordinates() {
    use crate::player::start_column;
    assert_eq!(start_column(None, 2048).unwrap(), None);
    assert_eq!(
        start_column(Some("1000,-1000".into()), 2048).unwrap(),
        Some([1000, -1000])
    );
    for s in ["", "0", "1,2,3", "2048,0", "-2049,0", "NaN,1"] {
        assert!(start_column(Some(s.into()), 2048).is_err());
    }
}
