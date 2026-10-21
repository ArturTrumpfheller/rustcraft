//! Adapter between Bevy's fixed schedule and the engine-independent controller.

use bevy::prelude::*;
use voxel_core::physics::{MovementInput, PHYSICS_HZ, PhysicsConfig, Player};

use crate::scene::VoxelWorld;

// Clear ground at (0, 10), above the low terrace and facing the island's centre.
pub(crate) const SPAWN: [f64; 3] = [0.5, 4.0, 10.5];
const RECOVERY_Y: f64 = -32.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
            .init_resource::<PlayerSettings>()
            .init_resource::<PlayerState>()
            .init_resource::<ControllerInput>()
            .add_systems(FixedUpdate, simulate_player);
    }
}

#[derive(Resource, Default)]
pub(crate) struct PlayerSettings(pub PhysicsConfig);

#[derive(Resource)]
pub(crate) struct PlayerState {
    pub body: Player,
    pub previous_position: [f64; 3],
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            body: Player::new(SPAWN),
            previous_position: SPAWN,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct ControllerInput {
    pub command: MovementInput,
    pub active: bool,
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
    if !input.active {
        return;
    } // Local single-player pause while cursor is free.
    if std::mem::take(&mut input.reset_pending) || player.body.position[1] < RECOVERY_Y {
        // Building at the old spawn must not make R/void recovery embed the
        // body in solids. Search upward; above EDIT_MAX all editable cells are air.
        let mut body = Player::new(SPAWN);
        while !body.fits_in_world(&world.0, &config.0)
            && body.position[1] < f64::from(voxel_core::EDIT_MAX[1])
        {
            body.position[1] += 1.0;
        }
        *player = PlayerState {
            previous_position: body.position,
            body,
        };
        input.jump_pending = false;
        return;
    }
    let mut command = input.command;
    command.jump = std::mem::take(&mut input.jump_pending);
    player.body.tick(&world.0, command, &config.0);
}
