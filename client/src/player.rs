//! Adapter between Bevy's fixed schedule and the engine-independent controller.

use bevy::prelude::*;
use voxel_core::physics::{MovementInput, PHYSICS_HZ, PhysicsConfig, Player};

use crate::scene::VoxelWorld;

// Clear ground at (0, 10), above the low terrace and facing the island's centre.
pub(crate) const SPAWN: [f64; 3] = [0.5, 4.0, 10.5];
pub(crate) const RECOVERY_Y: f64 = -32.0;

pub struct PlayerPlugin;
#[derive(Resource)]
pub(crate) struct InitialSpawn(pub [f64; 3]);
pub(crate) fn start_column(
    value: Option<std::ffi::OsString>,
    limit: i32,
) -> Result<Option<[i32; 2]>, std::io::Error> {
    let Some(value) = value else {
        return Ok(None);
    };
    let result = value
        .to_str()
        .and_then(|s| s.split_once(','))
        .and_then(|(x, z)| Some([x.parse::<i32>().ok()?, z.parse::<i32>().ok()?]));
    result
        .filter(|c| c.iter().all(|v| (-limit..limit).contains(v)))
        .map(Some)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "RUSTCRAFT_START_CHUNK must be x,z within this world's chunk bounds",
            )
        })
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
            .init_resource::<PlayerSettings>()
            .init_resource::<PlayerState>()
            .init_resource::<FluidRuntime>()
            .init_resource::<crate::scene::DirtyChunks>()
            .init_resource::<ControllerInput>()
            .add_systems(
                Startup,
                initialize_player
                    .before(crate::camera::CameraSetup)
                    .run_if(crate::session::has_world),
            )
            .add_systems(
                crate::session::SessionSetup,
                initialize_player.before(crate::camera::CameraSetup),
            )
            .add_systems(
                FixedUpdate,
                (
                    simulate_fluids.before(simulate_player),
                    simulate_player,
                    advance_world_time,
                )
                    .run_if(crate::session::has_world),
            );
    }
}

// Shares the pre-fixed-loop physics gate: inventory runs; menus and focus pause.
fn advance_world_time(input: Res<ControllerInput>, mut world: ResMut<VoxelWorld>) {
    if input.simulation_active {
        world.0.time.advance(1);
    }
}

#[derive(Resource, Default)]
pub(crate) struct PlayerSettings(pub PhysicsConfig);

#[derive(Resource)]
pub(crate) struct PlayerState {
    pub body: Player,
    pub previous_position: [f64; 3],
    pub previous_crouched: bool,
    pub awaiting_spawn: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            body: Player::new(SPAWN),
            previous_position: SPAWN,
            previous_crouched: false,
            awaiting_spawn: false,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct ControllerInput {
    pub command: MovementInput,
    /// Whether gameplay commands are accepted (cursor captured, no modal).
    pub active: bool,
    /// Whether fixed physics advances, independently of gameplay commands.
    pub simulation_active: bool,
    pub captured_this_frame: bool,
    // Edges survive frames with no fixed tick, and are consumed only once even
    // when a slow render frame runs several physics ticks.
    pub jump_pending: bool,
    pub reset_pending: bool,
}

fn simulate_player(
    world: Res<VoxelWorld>,
    config: Res<PlayerSettings>,
    mut input: ResMut<ControllerInput>,
    mut player: ResMut<PlayerState>,
) {
    player.previous_position = player.body.position;
    player.previous_crouched = player.body.crouched;
    if !input.simulation_active {
        return;
    }
    let reset = std::mem::take(&mut input.reset_pending);
    if (input.active && reset) || player.body.position[1] < RECOVERY_Y {
        *player = safe_spawn(&world.0, &config.0);
        input.jump_pending = false;
        return;
    }
    if player.awaiting_spawn {
        // The requested position drives streaming back to the starting area.
        // Never search remote resident ground or simulate pending spawn cells.
        if !world.0.ready(voxel_core::BlockPos([0, 0, 10])) {
            input.jump_pending = false;
            return;
        }
        *player = safe_spawn(&world.0, &config.0);
        input.jump_pending = false;
        return;
    }
    if world.0.identity().is_some() {
        let ready = player.body.cache_ready(&world.0, &config.0);
        if !ready {
            input.jump_pending = false;
            return;
        }
        // Recovery after a teleport into a column that finished loading.
        if !player.body.fits_in_world(&world.0, &config.0) {
            let p = voxel_core::physics::spawn_position(&world.0, &config.0, player.body.position);
            player.body = Player::new(p);
            player.previous_position = p;
            player.previous_crouched = false;
            input.jump_pending = false;
        }
    }
    let mut command = if input.active {
        input.command
    } else {
        MovementInput::default()
    };
    let jump = std::mem::take(&mut input.jump_pending);
    command.jump |= input.active && jump;
    player.body.tick(&world.0, command, &config.0);
}

fn safe_spawn(world: &voxel_core::World, config: &PhysicsConfig) -> PlayerState {
    let awaiting_spawn = !world.ready(voxel_core::BlockPos([0, 0, 10]));
    let body = Player::new(if awaiting_spawn {
        [SPAWN[0], f64::from(world.bounds().1[1]), SPAWN[2]]
    } else {
        voxel_core::physics::spawn_position(world, config, SPAWN)
    });
    PlayerState {
        previous_position: body.position,
        previous_crouched: false,
        body,
        awaiting_spawn,
    }
}
fn initialize_player(
    world: Res<VoxelWorld>,
    settings: Res<PlayerSettings>,
    mut player: ResMut<PlayerState>,
    start: Option<Res<InitialSpawn>>,
) {
    if let Some(start) = start {
        let p = voxel_core::physics::spawn_position(&world.0, &settings.0, start.0);
        *player = PlayerState {
            body: Player::new(p),
            previous_position: p,
            previous_crouched: false,
            awaiting_spawn: false,
        };
    } else {
        *player = safe_spawn(&world.0, &settings.0);
    }
}

#[derive(Resource)]
pub(crate) struct FluidRuntime {
    pub config: voxel_core::fluid::FluidConfig,
    phase: u8,
    changes: Vec<voxel_core::BlockChange>,
    pub milliseconds: f64,
}
impl Default for FluidRuntime {
    fn default() -> Self {
        Self {
            config: voxel_core::fluid::FluidConfig::default(),
            phase: 0,
            changes: Vec::with_capacity(256),
            milliseconds: 0.,
        }
    }
}
fn simulate_fluids(
    input: Res<ControllerInput>,
    mut world: ResMut<VoxelWorld>,
    mut runtime: ResMut<FluidRuntime>,
    mut dirty: ResMut<crate::scene::DirtyChunks>,
) {
    if !input.simulation_active {
        runtime.phase = 0;
        return;
    }
    runtime.phase += 1;
    if runtime.phase < 12 {
        return;
    }
    runtime.phase = 0;
    let start = std::time::Instant::now();
    let config = runtime.config;
    world.0.tick_fluids(config, &mut runtime.changes);
    // All fluid cells transmit exactly the same light as Air. Only geometry
    // changes; actual solid/torch edits still invalidate independent light fields.
    for change in &runtime.changes {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(p) = change.position.offset([dx, dy, dz]) {
                        dirty.2.insert(p.split().0);
                    }
                }
            }
        }
    }
    runtime.milliseconds = start.elapsed().as_secs_f64() * 1000.;
}
