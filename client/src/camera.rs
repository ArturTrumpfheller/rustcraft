//! Frame-rate input/view adapter. Player physics never reads a camera transform.

use crate::player::{ControllerInput, PlayerSettings, PlayerState};
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::Tonemapping,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use std::f32::consts::FRAC_PI_2;
use voxel_core::physics::MovementInput;

pub struct FirstPersonPlugin;

impl Plugin for FirstPersonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewAngles>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                RunFixedMainLoop,
                (manage_cursor, read_input)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                RunFixedMainLoop,
                sync_camera.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

#[derive(Component)]
struct FirstPersonCamera;

#[derive(Resource)]
struct ViewAngles {
    yaw: f32,
    pitch: f32,
}

impl Default for ViewAngles {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.12,
        }
    }
}

fn spawn_camera(mut commands: Commands, player: Res<PlayerState>, settings: Res<PlayerSettings>) {
    commands.spawn((
        FirstPersonCamera,
        Camera3d::default(),
        Tonemapping::Reinhard,
        Transform::from_translation(Vec3::from_array(
            player.body.eye_position(&settings.0).map(|v| v as f32),
        )),
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
        // winit falls back from Locked to Confined where native X11 needs it.
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn read_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut view: ResMut<ViewAngles>,
    mut input: ResMut<ControllerInput>,
) {
    input.active = windows
        .single()
        .is_ok_and(|(window, cursor)| window.focused && cursor.grab_mode != CursorGrabMode::None);
    if !input.active {
        *input = ControllerInput::default();
        return;
    }
    // Do not apply pre-capture hover motion to the freshly captured view.
    if !mouse.just_pressed(MouseButton::Left) {
        view.pitch =
            (view.pitch - motion.delta.y * 0.002).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        view.yaw = (view.yaw - motion.delta.x * 0.002) % std::f32::consts::TAU;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        *view = ViewAngles::default();
        input.reset_pending = true;
    }
    let axis = |positive, negative| {
        f64::from(keyboard.pressed(positive)) - f64::from(keyboard.pressed(negative))
    };
    input.command = MovementInput::from_yaw(
        [
            axis(KeyCode::KeyD, KeyCode::KeyA),
            axis(KeyCode::KeyS, KeyCode::KeyW),
        ],
        f64::from(view.yaw),
        keyboard.pressed(KeyCode::ShiftLeft),
        false,
    );
    input.jump_pending |= keyboard.just_pressed(KeyCode::Space);
}

fn sync_camera(
    fixed_time: Res<Time<Fixed>>,
    player: Res<PlayerState>,
    settings: Res<PlayerSettings>,
    input: Res<ControllerInput>,
    view: Res<ViewAngles>,
    mut cameras: Query<&mut Transform, With<FirstPersonCamera>>,
) {
    // A single fixed tick of interpolation latency smooths translation even at
    // render rates above/below 60 Hz; mouse rotation remains immediate.
    let alpha = if input.active {
        f64::from(fixed_time.overstep_fraction())
    } else {
        1.0
    };
    let position: [f64; 3] = std::array::from_fn(|axis| {
        player.previous_position[axis] * (1.0 - alpha) + player.body.position[axis] * alpha
    });
    for mut transform in &mut cameras {
        transform.translation = Vec3::new(
            position[0] as f32,
            (position[1] + settings.0.eye_height) as f32,
            position[2] as f32,
        );
        transform.rotation = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
    }
}

#[cfg(test)]
mod tests;
