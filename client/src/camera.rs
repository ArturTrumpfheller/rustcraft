//! Frame-rate input/view adapter. Player physics never reads a camera transform.

use crate::controls::MovementBindings;
use crate::player::{ControllerInput, PlayerSettings, PlayerState};
use crate::scene::{ChunkRenders, RenderOrigin};
use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::Tonemapping,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use std::f32::consts::FRAC_PI_2;

pub struct FirstPersonPlugin;

impl Plugin for FirstPersonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MovementBindings>()
            .init_resource::<ViewAngles>()
            .init_resource::<RenderOrigin>()
            .init_resource::<ViewPose>()
            .add_systems(Startup, spawn_camera.in_set(CameraSetup))
            .add_systems(
                RunFixedMainLoop,
                (manage_cursor, read_input)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .after(crate::inventory_ui::InventoryInput),
            )
            .add_systems(
                RunFixedMainLoop,
                (rebase_scene, sync_camera)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                    .in_set(ViewSync),
            );
    }
}

#[derive(Component)]
pub(crate) struct FirstPersonCamera;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ViewSync;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct CameraSetup;

#[derive(Resource, Default)]
pub(crate) struct ViewPose {
    pub eye: [f64; 3],
    pub direction: [f64; 3],
    pub feet: [f64; 3],
    pub crouched: bool,
}

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
    mut input: ResMut<ControllerInput>,
    inventory: Option<Res<crate::inventory_ui::GameInventory>>,
) {
    input.captured_this_frame = false;
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if let Some(inv) = inventory {
        if inv.open {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
            return;
        }
        if inv.transition {
            cursor.grab_mode = if inv.resume_capture {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            cursor.visible = !inv.resume_capture;
            input.captured_this_frame = true;
            return;
        }
    }
    if !window.focused || keyboard.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) {
        // winit falls back from Locked to Confined where native X11 needs it.
        input.captured_this_frame = cursor.grab_mode == CursorGrabMode::None;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

#[derive(SystemParam)]
struct KeyboardCommands<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    bindings: Res<'w, MovementBindings>,
}

fn read_input(
    commands: KeyboardCommands,
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut view: ResMut<ViewAngles>,
    mut input: ResMut<ControllerInput>,
    save: Option<Res<crate::persistence::SaveFeedback>>,
    inventory: Option<Res<crate::inventory_ui::GameInventory>>,
) {
    let keyboard = &commands.keys;
    let bindings = &commands.bindings;
    input.active = windows
        .single()
        .is_ok_and(|(window, cursor)| window.focused && cursor.grab_mode != CursorGrabMode::None);
    if inventory.is_some_and(|i| i.open || i.transition)
        || save.is_some_and(|status| status.closing)
    {
        input.active = false;
    }
    if !input.active {
        *input = ControllerInput::default();
        return;
    }
    // Do not apply pre-capture hover motion to the freshly captured view.
    if !input.captured_this_frame {
        view.pitch =
            (view.pitch - motion.delta.y * 0.002).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        view.yaw = (view.yaw - motion.delta.x * 0.002) % std::f32::consts::TAU;
    }
    if !input.captured_this_frame && keyboard.just_pressed(bindings.respawn) {
        *view = ViewAngles::default();
        input.reset_pending = true;
    }
    input.command = bindings.command(keyboard, f64::from(view.yaw));
    if !input.captured_this_frame {
        input.jump_pending |= keyboard.just_pressed(bindings.jump);
    }
}

#[derive(SystemParam)]
struct CameraSpace<'w> {
    settings: Res<'w, PlayerSettings>,
    origin: Res<'w, RenderOrigin>,
    pose: ResMut<'w, ViewPose>,
}
fn sync_camera(
    fixed_time: Res<Time<Fixed>>,
    player: Res<PlayerState>,
    mut space: CameraSpace,
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
    let mut previous_body = player.body;
    previous_body.crouched = player.previous_crouched;
    let eye_height = previous_body.eye_height(&space.settings.0) * (1.0 - alpha)
        + player.body.eye_height(&space.settings.0) * alpha;
    space.pose.feet = position;
    // Protect the union of the old/new posture during the single interpolated tick.
    space.pose.crouched = player.body.crouched && (player.previous_crouched || alpha == 1.0);
    space.pose.eye = [position[0], position[1] + eye_height, position[2]];
    let rotation = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
    space.pose.direction = (rotation * Vec3::NEG_Z).to_array().map(f64::from);
    for mut transform in &mut cameras {
        transform.translation = Vec3::new(
            (position[0] - space.origin.0[0]) as f32,
            (position[1] + eye_height - space.origin.0[1]) as f32,
            (position[2] - space.origin.0[2]) as f32,
        );
        transform.rotation = rotation;
    }
}

#[cfg(test)]
mod tests;

fn rebase_scene(
    player: Res<PlayerState>,
    mut origin: ResMut<RenderOrigin>,
    renders: Option<Res<ChunkRenders>>,
    mut transforms: Query<&mut Transform, With<Mesh3d>>,
) {
    let p = player.body.position;
    let next = [(p[0] / 16.).floor() * 16., 0., (p[2] / 16.).floor() * 16.];
    if origin.0 == next {
        return;
    }
    origin.0 = next;
    if let Some(renders) = renders {
        for (p, r) in &renders.0 {
            if let Ok(mut t) = transforms.get_mut(r.entity) {
                t.translation = Vec3::from_array(std::array::from_fn(|a| {
                    (f64::from(p.0[a]) * 16. - next[a]) as f32
                }));
            }
        }
    }
}
