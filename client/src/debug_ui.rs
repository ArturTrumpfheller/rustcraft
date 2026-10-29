//! Bounded, retained F3 diagnostics using RustCraft's original pixel glyphs.
//! Never reads save paths, render-relative positions, or scans voxel/chunk contents.
use crate::{menu::Canvas, player::PlayerState, scene::VoxelWorld, session::ClientFlow};
use bevy::{
    camera::{
        ScalingMode,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    prelude::*,
    render::view::Tonemapping,
    window::PrimaryWindow,
};
use std::fmt::Write;

const LAYER: usize = 3;
const REFRESH_SECONDS: f64 = 0.2;

pub struct DebugUiPlugin;
impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlay>()
            .add_systems(Startup, setup)
            .add_systems(PostUpdate, update);
    }
}

#[derive(Resource, Default)]
pub(crate) struct DebugOverlay {
    pub enabled: bool,
    was_visible: bool,
    seconds_since_refresh: f64,
    frame_seconds: Option<f64>,
    text: String,
}
impl DebugOverlay {
    fn sample_frame(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            // Ignore zero/invalid startup samples. Bound stalls and extreme small
            // deltas so reciprocal FPS stays finite. EMA has a 0.5s response.
            let seconds = seconds.clamp(1e-6, 1.0);
            let blend = 1.0 - (-seconds / 0.5).exp();
            self.frame_seconds = Some(
                self.frame_seconds
                    .map_or(seconds, |old| old + (seconds - old) * blend),
            );
        }
    }
}
#[derive(Component)]
struct DebugCamera;
#[derive(Component)]
struct DebugSurface;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DebugCamera,
        Camera3d::default(),
        Camera {
            order: 9,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 270.,
            },
            ..OrthographicProjection::default_3d()
        }),
        RenderLayers::layer(LAYER),
        Tonemapping::None,
        bevy::render::view::DebandDither::Disabled,
    ));
    commands.spawn((
        DebugSurface,
        Mesh3d(meshes.add(Canvas::default().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        RenderLayers::layer(LAYER),
        NoFrustumCulling,
        Transform::from_xyz(0., 0., -10.),
        Visibility::Hidden,
    ));
}

/// Integer feet block and X/Z column/local block coordinates, including negatives.
fn coordinates(position: [f64; 3]) -> ([i32; 3], [i32; 2], [i32; 3]) {
    let block = position.map(|v| v.floor() as i32);
    let chunk = [block[0].div_euclid(16), block[2].div_euclid(16)];
    (block, chunk, block.map(|v| v.rem_euclid(16)))
}
fn facing(direction: [f64; 3]) -> (&'static str, f64, f64) {
    let [x, y, z] = direction;
    let yaw = (-x).atan2(-z).to_degrees();
    let pitch = y.clamp(-1., 1.).asin().to_degrees();
    let cardinal = if z.abs() >= x.abs() {
        if z <= 0. { "NORTH -Z" } else { "SOUTH +Z" }
    } else if x >= 0. {
        "EAST +X"
    } else {
        "WEST -X"
    };
    (cardinal, yaw, pitch)
}

#[derive(bevy::ecs::system::SystemParam)]
struct Diagnostics<'w> {
    flow: Res<'w, ClientFlow>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    world: Option<Res<'w, VoxelWorld>>,
    player: Res<'w, PlayerState>,
    pose: Res<'w, crate::camera::ViewPose>,
    stream: Option<Res<'w, crate::streaming::StreamState>>,
    inventory: Res<'w, crate::inventory_ui::GameInventory>,
    input: Res<'w, crate::player::ControllerInput>,
    renders: Option<Res<'w, crate::scene::ChunkRenders>>,
    fluids: Option<Res<'w, crate::player::FluidRuntime>>,
}
fn update(
    mut overlay: ResMut<DebugOverlay>,
    data: Diagnostics,
    time: Res<Time<Real>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &mut Projection), With<DebugCamera>>,
    mut surfaces: Query<(&Mesh3d, &mut Visibility), With<DebugSurface>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(window), Ok((mut camera, mut projection)), Ok((handle, mut visibility))) = (
        windows.single(),
        cameras.single_mut(),
        surfaces.single_mut(),
    ) else {
        return;
    };
    if data.world.is_none() {
        overlay.enabled = false;
    }
    let eligible =
        data.world.is_some() && data.flow.playing() && window.focused && !data.inventory.open;
    if eligible && data.keys.just_pressed(KeyCode::F3) {
        overlay.enabled = !overlay.enabled;
    }
    let visible = eligible && overlay.enabled;
    camera.is_active = visible;
    *visibility = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !visible {
        overlay.was_visible = false;
        return;
    }
    overlay.sample_frame(time.delta_secs_f64());
    overlay.seconds_since_refresh += time.delta_secs_f64().min(1.);
    let size = Vec2::new(window.width(), window.height());
    if overlay.was_visible && overlay.seconds_since_refresh < REFRESH_SECONDS {
        return;
    }
    overlay.was_visible = true;
    overlay.seconds_since_refresh = 0.;
    let scale = (window.width() / 480.)
        .min(window.height() / 270.)
        .max(0.01);
    let half = size / (2. * scale);
    if let Projection::Orthographic(p) = &mut *projection {
        p.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: half.y * 2.,
        };
    }
    let p = &data.player.body;
    let (block, chunk, local) = coordinates(p.position);
    let (cardinal, yaw, pitch) = facing(data.pose.direction);
    // Reuse the text buffer. Formatting and glyph geometry run at most 5Hz while
    // visible, plus immediate toggle. Resize is also bounded. Asset handles never change.
    overlay.text.clear();
    let _ = writeln!(
        overlay.text,
        "RUSTCRAFT {} / DEV SLICE 25",
        env!("CARGO_PKG_VERSION")
    );
    if let Some(seconds) = overlay.frame_seconds {
        let ms = seconds * 1000.;
        let fps = 1. / seconds;
        let _ = writeln!(overlay.text, "FPS: {fps:.0} / FRAME: {ms:.2} MS (SMOOTHED)");
    } else {
        let _ = writeln!(overlay.text, "FPS: -- / FRAME: -- MS (WARMING UP)");
    }
    let _ = writeln!(
        overlay.text,
        "XYZ: {:.3} / {:.3} / {:.3}",
        p.position[0], p.position[1], p.position[2]
    );
    let _ = writeln!(
        overlay.text,
        "BLOCK: {} / {} / {}",
        block[0], block[1], block[2]
    );
    let _ = writeln!(overlay.text, "CHUNK XZ: {} / {}", chunk[0], chunk[1]);
    let _ = writeln!(
        overlay.text,
        "LOCAL XYZ: {} / {} / {}",
        local[0], local[1], local[2]
    );
    let _ = writeln!(
        overlay.text,
        "FACING: {cardinal} / YAW: {yaw:.1} / PITCH: {pitch:.1}"
    );
    if let Some(stream) = &data.stream {
        let _ = writeln!(overlay.text, "RENDER DISTANCE: {} CHUNKS", stream.radius());
    } else {
        let _ = writeln!(overlay.text, "RENDER DISTANCE: UNAVAILABLE");
    }
    let _ = writeln!(
        overlay.text,
        "CHUNKS LOADED: {} / MESH SLOTS: {}",
        data.world.as_ref().unwrap().0.chunk_count(),
        data.renders.as_ref().map_or(0, |r| r.0.len())
    );
    let world = &data.world.as_ref().unwrap().0;
    let _ = writeln!(
        overlay.text,
        "WATER: {} / PENDING: {} / TICK: {} / {:.3} MS",
        world.water_cells(),
        world.fluid_updates().pending(),
        world.fluid_updates().processed,
        data.fluids.as_ref().map_or(0., |f| f.milliseconds)
    );
    if let Some(id) = world.identity() {
        if id.generator == 5 {
            if let Some(biome) = voxel_core::worldgen::sample_biome(id.seed, block[0], block[2]) {
                let _ = writeln!(
                    overlay.text,
                    "BIOME: {} / GENERATOR: 5",
                    biome.name().to_ascii_uppercase()
                );
            }
        } else {
            let _ = writeln!(overlay.text, "BIOME: LEGACY / GENERATOR: {}", id.generator);
        }
    }
    let clock = world.time;
    let _ = writeln!(
        overlay.text,
        "DAY: {} / TIME: {:.0} / DAYLIGHT: {:.2}",
        clock.day(),
        clock.units().floor(),
        clock.daylight()
    );
    let _ = writeln!(
        overlay.text,
        "SPEED XZ: {:.3} BLOCKS/S / GROUNDED: {}",
        p.velocity[0].hypot(p.velocity[2]),
        if p.grounded { "YES" } else { "NO" }
    );
    let _ = write!(
        overlay.text,
        "SNEAKING: {} / SPRINT INPUT: {}",
        if p.crouched { "YES" } else { "NO" },
        if data.input.active && data.input.command.sprint {
            "YES"
        } else {
            "NO"
        }
    );
    if let Some(mut mesh) = meshes.get_mut(&handle.0) {
        *mesh = canvas(&overlay.text, half).mesh();
    }
}
fn canvas(text: &str, half: Vec2) -> Canvas {
    let mut canvas = Canvas::default();
    let x = -half.x + 6.;
    let top = half.y - 38.; // Leave the existing save feedback unobstructed.
    for (i, line) in text.lines().enumerate() {
        let y = top - i as f32 * 10.;
        let size = 0.8;
        canvas.rect(
            x - 2.,
            y - 2.,
            line.len() as f32 * 6. * size + 4.,
            10.,
            [0.008, 0.012, 0.02, 0.78],
        );
        canvas.text(x, y, line, size, [0.94, 0.96, 1., 1.]);
    }
    canvas
}
pub(crate) fn reset(world: &mut World) {
    if world.contains_resource::<DebugOverlay>() {
        world.insert_resource(DebugOverlay::default());
        for mut camera in world
            .query_filtered::<&mut Camera, With<DebugCamera>>()
            .iter_mut(world)
        {
            camera.is_active = false;
        }
        for mut visibility in world
            .query_filtered::<&mut Visibility, With<DebugSurface>>()
            .iter_mut(world)
        {
            *visibility = Visibility::Hidden;
        }
    }
}
#[cfg(test)]
mod tests;
