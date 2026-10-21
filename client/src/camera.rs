use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::Tonemapping,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use std::f32::consts::FRAC_PI_2;

pub struct FirstPersonPlugin;

impl Plugin for FirstPersonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, (manage_cursor, move_camera).chain());
    }
}

#[derive(Component)]
struct FirstPersonCamera;

fn initial_transform() -> Transform {
    Transform::from_xyz(12.0, 12.0, 22.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y)
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        FirstPersonCamera,
        Camera3d::default(),
        Tonemapping::Reinhard,
        initial_transform(),
    ));
}

fn manage_cursor(
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if !window.focused || keyboard.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) {
        // Bevy/winit supplies native platform fallbacks (confined on X11,
        // locked relative pointer on Wayland and supported Windows behavior).
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn move_camera(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut cameras: Query<&mut Transform, With<FirstPersonCamera>>,
) {
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    if !window.focused || cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    for mut transform in &mut cameras {
        if keyboard.just_pressed(KeyCode::KeyR) {
            *transform = initial_transform();
            continue;
        }
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        let pitch = (pitch - motion.delta.y * 0.002).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        let yaw = yaw - motion.delta.x * 0.002;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        let axis = |positive, negative| {
            f32::from(keyboard.pressed(positive)) - f32::from(keyboard.pressed(negative))
        };
        let local = Vec3::new(
            axis(KeyCode::KeyD, KeyCode::KeyA),
            axis(KeyCode::Space, KeyCode::ControlLeft),
            axis(KeyCode::KeyS, KeyCode::KeyW),
        );
        // Horizontal movement follows yaw, so looking up does not change speed.
        let direction = Quat::from_rotation_y(yaw) * local.normalize_or_zero();
        let speed = if keyboard.pressed(KeyCode::ShiftLeft) {
            18.0
        } else {
            6.0
        };
        // Limit the distance after pauses; physics will use a fixed tick later.
        transform.translation += direction * speed * time.delta_secs().min(0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .add_plugins(FirstPersonPlugin);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            CursorOptions::default(),
            PrimaryWindow,
        ));
        app.update();
        app
    }

    fn window_entity(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap()
    }

    fn camera_entity(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<FirstPersonCamera>>()
            .single(app.world())
            .unwrap()
    }

    fn capture(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
    }

    #[test]
    fn click_captures_escape_releases_and_focus_loss_releases() {
        let mut app = test_app();
        let window = window_entity(&mut app);
        capture(&mut app);
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::Locked
        );
        assert!(!app.world().get::<CursorOptions>(window).unwrap().visible);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
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
        app.update();
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
        assert!(app.world().get::<CursorOptions>(window).unwrap().visible);
        // Regaining focus alone must not capture the pointer again.
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.update();
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
    }

    #[test]
    fn uncaptured_or_unfocused_camera_does_not_move() {
        let mut app = test_app();
        let camera = camera_entity(&mut app);
        let window = window_entity(&mut app);
        let original = app.world().get::<Transform>(camera).unwrap().translation;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            original
        );
        app.world_mut()
            .get_mut::<CursorOptions>(window)
            .unwrap()
            .grab_mode = CursorGrabMode::Locked;
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            original
        );
    }

    #[test]
    fn diagonal_flight_is_normalized_and_reset_restores_the_spawn_pose() {
        let mut app = test_app();
        capture(&mut app);
        let camera = camera_entity(&mut app);
        let original = app.world().get::<Transform>(camera).unwrap().translation;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        for key in [KeyCode::KeyW, KeyCode::KeyD, KeyCode::Space] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.update();
        let distance = app
            .world()
            .get::<Transform>(camera)
            .unwrap()
            .translation
            .distance(original);
        assert!((distance - 0.6).abs() < 0.0001);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        let transform = app.world().get::<Transform>(camera).unwrap();
        assert_eq!(transform.translation, initial_transform().translation);
        assert_eq!(transform.rotation, initial_transform().rotation);
    }

    #[test]
    fn mouse_look_is_independent_of_frame_time_and_clamps_pitch() {
        let mut rotations = Vec::new();
        for millis in [10, 100] {
            let mut app = test_app();
            capture(&mut app);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(millis));
            app.world_mut()
                .resource_mut::<AccumulatedMouseMotion>()
                .delta = Vec2::new(20.0, -100_000.0);
            app.update();
            let camera = camera_entity(&mut app);
            let transform = app.world().get::<Transform>(camera).unwrap();
            assert!(transform.forward().y > 0.999);
            assert!(transform.forward().y < 1.0);
            rotations.push(transform.rotation);
        }
        assert_eq!(rotations[0], rotations[1]);
    }
}
