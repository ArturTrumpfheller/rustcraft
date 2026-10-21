//! Fixed-tick kinematic player physics, independent of Bevy and rendering.
//!
//! Y is up. Positions are feet-center coordinates in block units (metres).
//! Call `Player::tick` once per simulated tick with an input command. Missing
//! chunks are air, exactly as in `World::block`. No mesh queries or allocations
//! are needed in the simulation. A non-intersecting initial position is required.

use crate::{BlockPos, World};

pub const PHYSICS_HZ: f64 = 60.0;
pub const PHYSICS_DT: f64 = 1.0 / PHYSICS_HZ;
const CONTACT_EPSILON: f64 = 1.0e-8;
const SUPPORT_DISTANCE: f64 = 1.0e-6;

/// Controller tuning in block units and seconds. Values must be finite and
/// positive (eye height must be less than body height). Not a Minecraft replica.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsConfig {
    pub half_width: f64,
    pub height: f64,
    pub eye_height: f64,
    pub walk_speed: f64,
    pub sprint_speed: f64,
    pub ground_acceleration: f64,
    pub ground_braking: f64,
    pub air_acceleration: f64,
    pub gravity: f64,
    pub jump_speed: f64,
    pub terminal_speed: f64,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            half_width: 0.3,
            height: 1.8,
            eye_height: 1.62,
            walk_speed: 4.3,
            sprint_speed: 5.6,
            ground_acceleration: 55.0,
            ground_braking: 60.0,
            air_acceleration: 12.0,
            gravity: 24.0,
            jump_speed: 8.5,
            terminal_speed: 55.0,
        }
    }
}

/// One tick's world-space X/Z movement command. Jump is an edge/request, not a
/// held key: callers must not repeat it unless the player actually presses again.
#[derive(Clone, Copy, Debug, Default)]
pub struct MovementInput {
    pub direction: [f64; 2],
    pub sprint: bool,
    pub jump: bool,
}

impl MovementInput {
    /// Local X is strafe-right; local Z is backwards (forward is -Z).
    pub fn from_yaw(local: [f64; 2], yaw: f64, sprint: bool, jump: bool) -> Self {
        let (sin, cos) = yaw.sin_cos();
        Self {
            direction: [
                cos * local[0] + sin * local[1],
                -sin * local[0] + cos * local[1],
            ],
            sprint,
            jump,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub grounded: bool,
}

impl Player {
    pub fn new(position: [f64; 3]) -> Self {
        Self {
            position,
            velocity: [0.0; 3],
            grounded: false,
        }
    }

    pub fn eye_position(&self, config: &PhysicsConfig) -> [f64; 3] {
        let [x, y, z] = self.position;
        [x, y + config.eye_height, z]
    }

    pub fn tick(&mut self, world: &World, input: MovementInput, config: &PhysicsConfig) {
        // Never trust last tick's support after walking off a ledge or a world edit.
        self.grounded = self.velocity[1] <= 0.0 && self.supported(world, config);
        self.accelerate(input, config);
        if input.jump && self.grounded {
            self.velocity[1] = config.jump_speed;
            self.grounded = false;
        }
        self.velocity[1] =
            (self.velocity[1] - config.gravity * PHYSICS_DT).max(-config.terminal_speed);

        // Y first settles floors/ceilings; independent X/Z clipping gives wall
        // sliding. The fixed axis order is deliberate and repeatable.
        for axis in [1, 0, 2] {
            let requested = self.velocity[axis] * PHYSICS_DT;
            let allowed = self.sweep_axis(world, config, axis, requested);
            self.position[axis] += allowed;
            if allowed != requested {
                self.velocity[axis] = 0.0;
            }
        }
        self.grounded = self.velocity[1] <= 0.0 && self.supported(world, config);
    }

    fn accelerate(&mut self, input: MovementInput, config: &PhysicsConfig) {
        let [mut x, mut z] = input.direction;
        if !x.is_finite() || !z.is_finite() {
            x = 0.0;
            z = 0.0;
        }
        let length = x.hypot(z);
        if length > 1.0 {
            x /= length;
            z /= length;
        }
        // In the air, releasing movement preserves momentum. Directional input
        // still supplies bounded air control; there is no instant mid-air brake.
        if !self.grounded && length == 0.0 {
            return;
        }
        let speed = if input.sprint {
            config.sprint_speed
        } else {
            config.walk_speed
        };
        let target = [x * speed, z * speed];
        let delta = [target[0] - self.velocity[0], target[1] - self.velocity[2]];
        let acceleration = if !self.grounded {
            config.air_acceleration
        } else if length == 0.0 {
            config.ground_braking
        } else {
            config.ground_acceleration
        };
        let magnitude = delta[0].hypot(delta[1]);
        if magnitude > 0.0 {
            let factor = (acceleration * PHYSICS_DT / magnitude).min(1.0);
            self.velocity[0] += delta[0] * factor;
            self.velocity[2] += delta[1] * factor;
        }
    }

    fn bounds(&self, config: &PhysicsConfig) -> ([f64; 3], [f64; 3]) {
        let [x, y, z] = self.position;
        (
            [x - config.half_width, y, z - config.half_width],
            [
                x + config.half_width,
                y + config.height,
                z + config.half_width,
            ],
        )
    }

    fn supported(&self, world: &World, config: &PhysicsConfig) -> bool {
        self.sweep_axis(world, config, 1, -SUPPORT_DISTANCE) > -SUPPORT_DISTANCE
    }

    /// Clip displacement to the nearest solid face across the entire swept
    /// slab, not just at its destination. Other axes use open intervals so a
    /// touching floor/wall does not snag movement parallel to that surface.
    fn sweep_axis(&self, world: &World, config: &PhysicsConfig, axis: usize, delta: f64) -> f64 {
        if delta == 0.0 {
            return 0.0;
        }
        let (min, max) = self.bounds(config);
        let mut first = [0; 3];
        let mut last = [0; 3];
        for dimension in 0..3 {
            if dimension == axis {
                first[dimension] = (min[dimension] + delta.min(0.0)).floor() as i32;
                last[dimension] = (max[dimension] + delta.max(0.0)).floor() as i32;
            } else {
                first[dimension] = (min[dimension] + CONTACT_EPSILON).floor() as i32;
                last[dimension] = (max[dimension] - CONTACT_EPSILON).floor() as i32;
            }
        }
        let mut allowed = delta;
        for y in first[1]..=last[1] {
            for z in first[2]..=last[2] {
                for x in first[0]..=last[0] {
                    let block = [x, y, z];
                    if !world.block(BlockPos(block)).is_solid() {
                        continue;
                    }
                    let face = f64::from(block[axis]);
                    if delta > 0.0 && max[axis] <= face + CONTACT_EPSILON {
                        allowed = allowed.min((face - max[axis]).max(0.0));
                    } else if delta < 0.0 && min[axis] >= face + 1.0 - CONTACT_EPSILON {
                        allowed = allowed.max((face + 1.0 - min[axis]).min(0.0));
                    }
                }
            }
        }
        allowed
    }
}

#[cfg(test)]
mod tests;
