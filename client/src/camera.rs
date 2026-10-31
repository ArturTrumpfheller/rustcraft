//! Frame-rate input/view adapter. Player physics never reads a camera transform.

use crate::controls::{MovementBindings, MovementGestures};
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
            .init_resource::<MovementGestures>()
            .init_resource::<ViewAngles>()
            .init_resource::<RenderOrigin>()
            .init_resource::<ViewPose>()
            .add_systems(
                Startup,
                (spawn_camera.in_set(CameraSetup)).run_if(crate::session::has_world),
            )
            .add_systems(
                crate::session::SessionSetup,
                spawn_camera.in_set(CameraSetup),
            )
            .add_systems(
                RunFixedMainLoop,
                (manage_cursor, read_input)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .after(crate::inventory_ui::InventoryInput)
                    .after(crate::session::FlowInput)
                    .in_set(CameraInput)
                    .run_if(crate::session::has_world),
            )
            .add_systems(
                RunFixedMainLoop,
                (rebase_scene, sync_camera, sync_sprint_fov)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                    .in_set(ViewSync)
                    .run_if(crate::session::has_world),
            );
    }
}

#[derive(Component)]
pub(crate) struct FirstPersonCamera;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct ViewSync;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct CameraSetup;
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct CameraInput;

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

fn spawn_camera(
    mut commands: Commands,
    player: Res<PlayerState>,
    settings: Res<PlayerSettings>,
    preferences: Option<Res<crate::settings::Preferences>>,
) {
    commands.spawn((
        crate::session::SessionEntity,
        FirstPersonCamera,
        SprintFov::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: preferences.map_or(45., |p| p.fov as f32).to_radians(),
            ..default()
        }),
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
    mut flow: Option<ResMut<crate::session::ClientFlow>>,
) {
    input.captured_this_frame = false;
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if flow.as_ref().is_some_and(|f| !f.playing()) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if !window.focused || inventory.as_ref().is_some_and(|i| i.open) {
        if let Some(flow) = &mut flow {
            flow.resume_frame = false;
        }
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if flow.as_ref().is_some_and(|f| f.resume_frame) {
        flow.as_mut().unwrap().resume_frame = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        input.captured_this_frame = true;
        return;
    }
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
    gestures: ResMut<'w, MovementGestures>,
    time: Res<'w, Time<Real>>,
    player: Res<'w, PlayerState>,
    voxel_world: Res<'w, crate::scene::VoxelWorld>,
    flow: Option<Res<'w, crate::session::ClientFlow>>,
    preferences: Option<Res<'w, crate::settings::Preferences>>,
}

fn read_input(
    mut commands: KeyboardCommands,
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut view: ResMut<ViewAngles>,
    mut input: ResMut<ControllerInput>,
    save: Option<Res<crate::persistence::SaveFeedback>>,
    inventory: Option<Res<crate::inventory_ui::GameInventory>>,
) {
    let keyboard = &commands.keys;
    let bindings = &commands.bindings;
    let focused = windows.single().is_ok_and(|(window, _)| window.focused);
    let captured = windows
        .single()
        .is_ok_and(|(_, cursor)| cursor.grab_mode != CursorGrabMode::None);
    let inventory_modal = inventory.as_ref().is_some_and(|i| i.open || i.transition);
    let session_running = commands.flow.as_ref().is_none_or(|f| f.playing())
        && !save.is_some_and(|status| status.closing);
    // A free cursor alone still freezes the standalone prototype. Inventory is
    // explicitly non-pausing; focus and lifecycle barriers always take priority.
    input.simulation_active = focused && session_running && (captured || inventory_modal);
    input.active = focused && session_running && captured && !inventory_modal;
    if !input.active {
        commands.gestures.block_held_movement(keyboard, bindings);
        commands
            .gestures
            .update(keyboard, bindings, commands.time.elapsed(), false);
        let simulation_active = input.simulation_active;
        *input = ControllerInput {
            simulation_active,
            ..default()
        };
        return;
    }
    // Do not apply pre-capture hover motion to the freshly captured view.
    if !input.captured_this_frame {
        let sensitivity = commands
            .preferences
            .as_ref()
            .map_or(0.002, |p| p.sensitivity as f32 * 0.00002);
        let invert = commands.preferences.as_ref().is_some_and(|p| p.invert_y);
        view.pitch = (view.pitch - motion.delta.y * sensitivity * if invert { -1. } else { 1. })
            .clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        view.yaw = (view.yaw - motion.delta.x * sensitivity) % std::f32::consts::TAU;
    }
    if !input.captured_this_frame && keyboard.just_pressed(bindings.respawn) {
        *view = ViewAngles::default();
        input.reset_pending = true;
    }
    if input.captured_this_frame {
        commands.gestures.block_held_movement(keyboard, bindings);
    }
    let enabled = !input.captured_this_frame
        && !input.reset_pending
        && !commands.player.awaiting_spawn
        && commands.player.body.position[1] >= commands.voxel_world.0.world_height().recovery_y();
    commands
        .gestures
        .update(keyboard, bindings, commands.time.elapsed(), enabled);
    input.command = commands
        .gestures
        .command(keyboard, bindings, f64::from(view.yaw));
    input.command.sprint |= commands.gestures.double_tap_sprint;
    input.command.jump = enabled && commands.gestures.jump_ready && keyboard.pressed(bindings.jump);
    if enabled && commands.gestures.jump_ready {
        input.jump_pending |= keyboard.just_pressed(bindings.jump);
    } else {
        input.jump_pending = false;
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
    let alpha = if input.simulation_active {
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

// Relative to the perspective projection's base, including future settings edits.
const SPRINT_FOV_GAIN: f32 = 0.09;
const SPRINT_FOV_RESPONSE: f32 = 10.0;
#[derive(Component, Default)]
struct SprintFov {
    base: Option<f32>,
    amount: f32,
    applied: Option<f32>,
}

fn sync_sprint_fov(
    time: Res<Time<Real>>,
    input: Res<ControllerInput>,
    player: Res<PlayerState>,
    settings: Res<PlayerSettings>,
    mut cameras: Query<(&mut Projection, &mut SprintFov), With<FirstPersonCamera>>,
) {
    let sprinting = input.active
        && !input.reset_pending
        && !player.awaiting_spawn
        && input.command.sprint
        && !input.command.sneak
        && !player.body.crouched
        && player.body.velocity[0].hypot(player.body.velocity[2]) > settings.0.walk_speed + 0.2
        && input.command.direction[0].hypot(input.command.direction[1]) > 0.0;
    let target = if sprinting { 1.0 } else { 0.0 };
    // Smooth return even in menus; limit the first step after a long OS stall.
    let blend = 1.0 - (-SPRINT_FOV_RESPONSE * time.delta_secs().min(0.05)).exp();
    for (mut projection, mut fov) in &mut cameras {
        let Projection::Perspective(p) = &*projection else {
            continue;
        };
        if fov.applied != Some(p.fov) {
            fov.base = Some(p.fov);
        }
        fov.amount += (target - fov.amount) * blend;
        if (target - fov.amount).abs() < 1e-5 {
            fov.amount = target;
        }
        let next = fov.base.unwrap_or(p.fov) * (1.0 + SPRINT_FOV_GAIN * fov.amount);
        fov.applied = Some(next);
        if next != p.fov
            && let Projection::Perspective(p) = &mut *projection
        {
            p.fov = next;
        }
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
        for (p, r) in renders.0.iter().chain(&renders.1).chain(&renders.2) {
            if let Ok(mut t) = transforms.get_mut(r.entity) {
                t.translation = Vec3::from_array(std::array::from_fn(|a| {
                    (f64::from(p.0[a]) * 16. - next[a]) as f32
                }));
            }
        }
    }
}

pub(crate) fn reset(world: &mut World) {
    world.insert_resource(ViewAngles::default());
    world.insert_resource(ViewPose::default());
}

pub(crate) fn set_fov(world: &mut World, degrees: u16) {
    for (mut projection, mut sprint) in world
        .query_filtered::<(&mut Projection, &mut SprintFov), With<FirstPersonCamera>>()
        .iter_mut(world)
    {
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = (degrees as f32).to_radians();
        }
        *sprint = SprintFov::default();
    }
}
