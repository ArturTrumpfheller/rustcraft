use super::*;
use crate::{
    player::{PlayerPlugin, SPAWN},
    scene::VoxelWorld,
};
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;
use voxel_core::physics::{PhysicsConfig, Player};

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(voxel_core::demo_world()))
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
