//! Default locomotion bindings, separate from simulation commands. A future
//! settings adapter can replace this resource without changing voxel physics.
use bevy::prelude::*;
use voxel_core::physics::MovementInput;

#[derive(Resource)]
pub(crate) struct MovementBindings {
    pub forward: KeyCode,
    pub backward: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub sprint: KeyCode,
    pub sneak: KeyCode,
    pub jump: KeyCode,
    pub respawn: KeyCode,
}

impl Default for MovementBindings {
    fn default() -> Self {
        Self {
            forward: KeyCode::KeyW,
            backward: KeyCode::KeyS,
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            sprint: KeyCode::ControlLeft,
            sneak: KeyCode::ShiftLeft,
            jump: KeyCode::Space,
            respawn: KeyCode::KeyR,
        }
    }
}

impl MovementBindings {
    pub fn command(&self, keys: &ButtonInput<KeyCode>, yaw: f64) -> MovementInput {
        let axis = |positive, negative| {
            f64::from(keys.pressed(positive)) - f64::from(keys.pressed(negative))
        };
        MovementInput {
            sneak: keys.pressed(self.sneak),
            ..MovementInput::from_yaw(
                [
                    axis(self.right, self.left),
                    axis(self.backward, self.forward),
                ],
                yaw,
                keys.pressed(self.sprint),
                false,
            )
        }
    }
}
