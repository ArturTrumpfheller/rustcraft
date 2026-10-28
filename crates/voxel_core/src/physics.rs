//! Fixed-tick kinematic player physics, independent of Bevy and rendering.
//!
//! Y is up. Positions are feet-center coordinates in block units (metres).
//! Call `Player::tick` once per simulated tick with an input command. Missing
//! chunks follow `World::block` (pending streamed cells are solid barriers).
//! No mesh queries or allocations
//! are needed in the simulation. A non-intersecting initial position is required.

use crate::{BlockPos, World};

pub const PHYSICS_HZ: f64 = 60.0;
pub const PHYSICS_DT: f64 = 1.0 / PHYSICS_HZ;
const CONTACT_EPSILON: f64 = 1.0e-8;
const SUPPORT_DISTANCE: f64 = 1.0e-6;
const WATER_EXIT_REACH: f64 = 0.35;
const WATER_EXIT_SPEED: f64 = 7.8;

/// Controller tuning in block units and seconds. Values must be finite and
/// positive (eye height must be less than body height). Not a Minecraft replica.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsConfig {
    pub half_width: f64,
    pub height: f64,
    pub eye_height: f64,
    pub crouch_height: f64,
    pub crouch_eye_height: f64,
    pub sneak_speed: f64,
    pub walk_speed: f64,
    pub sprint_speed: f64,
    pub sprint_jump_boost: f64,
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
            crouch_height: 1.5,
            crouch_eye_height: 1.32,
            sneak_speed: 1.295,
            walk_speed: 4.317,
            sprint_speed: 5.612,
            // Preserve the prior 15% launch headroom at the slower sprint pace.
            sprint_jump_boost: 0.8418,
            ground_acceleration: 65.0,
            ground_braking: 75.0,
            air_acceleration: 12.0,
            gravity: 24.0,
            jump_speed: 8.5,
            terminal_speed: 55.0,
        }
    }
}

/// One tick's world-space X/Z movement command. Jump may remain requested while
/// held: only actual support permits a launch, never a second airborne jump.
#[derive(Clone, Copy, Debug, Default)]
pub struct MovementInput {
    pub direction: [f64; 2],
    pub sprint: bool,
    pub sneak: bool,
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
            sneak: false,
            jump,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub grounded: bool,
    pub crouched: bool,
    // One ballistic exit attempt until landing or returning below its launch.
    // Transient controller state; never part of world/player persistence.
    water_exit_origin: Option<f64>,
}

impl Player {
    pub fn new(position: [f64; 3]) -> Self {
        Self {
            position,
            velocity: [0.0; 3],
            grounded: false,
            crouched: false,
            water_exit_origin: None,
        }
    }

    pub fn eye_position(&self, config: &PhysicsConfig) -> [f64; 3] {
        let [x, y, z] = self.position;
        [x, y + self.eye_height(config), z]
    }

    pub fn eye_height(&self, config: &PhysicsConfig) -> f64 {
        if self.crouched {
            config.crouch_eye_height
        } else {
            config.eye_height
        }
    }

    pub fn height(&self, config: &PhysicsConfig) -> f64 {
        if self.crouched {
            config.crouch_height
        } else {
            config.height
        }
    }

    /// Strict positive-volume overlap: face contact alone is allowed. This
    /// shares the exact dimensions used by collision, including feet/head.
    pub fn intersects_block(&self, position: BlockPos, config: &PhysicsConfig) -> bool {
        let (min, max) = self.bounds(config);
        (0..3).all(|axis| {
            min[axis] < f64::from(position.0[axis]) + 1.0 && max[axis] > f64::from(position.0[axis])
        })
    }

    /// Clearance query with the same open contact intervals as collision.
    /// A clipped face may reconstruct a few ULPs inside a block in f64; this
    /// must not trigger teleport recovery while the sweep permits contact.
    pub fn fits_in_world(&self, world: &World, config: &PhysicsConfig) -> bool {
        let (min, max) = self.bounds(config);
        for x in
            (min[0] + CONTACT_EPSILON).floor() as i32..=(max[0] - CONTACT_EPSILON).floor() as i32
        {
            for y in (min[1] + CONTACT_EPSILON).floor() as i32
                ..=(max[1] - CONTACT_EPSILON).floor() as i32
            {
                for z in (min[2] + CONTACT_EPSILON).floor() as i32
                    ..=(max[2] - CONTACT_EPSILON).floor() as i32
                {
                    if world.block(BlockPos([x, y, z])).is_collidable() {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn tick(&mut self, world: &World, input: MovementInput, config: &PhysicsConfig) {
        if input.sneak {
            self.crouched = true;
        } else if self.crouched {
            let standing = Self {
                crouched: false,
                ..*self
            };
            if standing.fits_in_world(world, config) {
                self.crouched = false;
            }
        }
        // Never trust last tick's support after walking off a ledge or a world edit.
        self.grounded = self.velocity[1] <= 0.0 && self.supported(world, config);
        let in_water = self.in_water(world, config);
        if self.grounded
            || (in_water
                && self.velocity[1] <= 0.
                && self
                    .water_exit_origin
                    .is_some_and(|y| self.position[1] <= y))
        {
            self.water_exit_origin = None;
        }
        if in_water {
            // Exponential drag and bounded acceleration, at the same 60-Hz step.
            let drag = (-5.0 * PHYSICS_DT).exp();
            self.velocity[0] *= drag;
            self.velocity[2] *= drag;
            let mut water_config = *config;
            water_config.walk_speed = 2.;
            water_config.sprint_speed = 2.;
            water_config.sneak_speed = 1.;
            water_config.sprint_jump_boost = 0.;
            water_config.air_acceleration = 10.;
            self.accelerate(input, &water_config);
        } else {
            self.accelerate(input, config);
        }
        if self.crouched && self.grounded {
            let speed = self.velocity[0].hypot(self.velocity[2]);
            if speed > config.sneak_speed {
                self.velocity[0] *= config.sneak_speed / speed;
                self.velocity[2] *= config.sneak_speed / speed;
            }
        }
        if input.jump && self.grounded && !in_water {
            self.boost_sprint_jump(world, input, config);
            self.velocity[1] = config.jump_speed;
            self.grounded = false;
        }
        let guard_ledge = self.crouched && self.grounded && !in_water;
        if in_water
            && input.jump
            && !self.crouched
            && self.water_exit_origin.is_none()
            && self.water_exit_clear(world, input, config)
        {
            self.water_exit_origin = Some(self.position[1]);
            self.velocity[1] = WATER_EXIT_SPEED;
            self.grounded = false;
        }
        self.velocity[1] = if in_water && self.water_exit_origin.is_none() {
            (self.velocity[1] * (-4.0 * PHYSICS_DT).exp()
                + if input.jump { 12. } else { -4. } * PHYSICS_DT)
                .clamp(-2., 2.5)
        } else {
            (self.velocity[1] - config.gravity * PHYSICS_DT).max(-config.terminal_speed)
        };

        // Y first settles floors/ceilings; independent X/Z clipping gives wall
        // sliding. The fixed axis order is deliberate and repeatable.
        for axis in [1, 0, 2] {
            let requested = self.velocity[axis] * PHYSICS_DT;
            let mut allowed = self.sweep_axis(world, config, axis, requested);
            if guard_ledge && axis != 1 {
                allowed = self.guard_ledge(world, config, axis, allowed);
            }
            self.position[axis] += allowed;
            if allowed != requested {
                self.velocity[axis] = 0.0;
            }
        }
        self.grounded = self.velocity[1] <= 0.0 && self.supported(world, config);
    }

    /// Check a nearby one-block ledge before launching. This probe never moves
    /// the player: all actual motion still uses the ordinary Y/X/Z sweeps.
    fn water_exit_clear(
        &self,
        world: &World,
        input: MovementInput,
        config: &PhysicsConfig,
    ) -> bool {
        let [x, z] = input.direction;
        let length = x.hypot(z);
        if !length.is_finite() || length < CONTACT_EPSILON {
            return false;
        }
        let delta = [x / length * WATER_EXIT_REACH, z / length * WATER_EXIT_REACH];
        let blocked = [(0, delta[0]), (2, delta[1])]
            .into_iter()
            .any(|(a, d)| (self.sweep_axis(world, config, a, d) - d).abs() > CONTACT_EPSILON);
        if !blocked {
            return false;
        }
        let rise = self.position[1].floor() + 1. - self.position[1];
        if self.sweep_axis(world, config, 1, rise) < rise - CONTACT_EPSILON {
            return false;
        }
        let mut landing = *self;
        landing.position[1] += rise;
        for (a, d) in [(0, delta[0]), (2, delta[1])] {
            if (landing.sweep_axis(world, config, a, d) - d).abs() > CONTACT_EPSILON {
                return false;
            }
            landing.position[a] += d;
        }
        let mut support = landing;
        support.position[1] -= SUPPORT_DISTANCE;
        landing.cache_ready(world, config)
            && support.cache_ready(world, config)
            && landing.fits_in_world(world, config)
            && landing.supported(world, config)
    }

    /// Body overlap with actual sloped surface heights, not just water cell IDs.
    pub fn in_water(&self, world: &World, config: &PhysicsConfig) -> bool {
        let (min, max) = self.bounds(config);
        for x in min[0].floor() as i32..=(max[0] - CONTACT_EPSILON).floor() as i32 {
            for y in min[1].floor() as i32..=(max[1] - CONTACT_EPSILON).floor() as i32 {
                for z in min[2].floor() as i32..=(max[2] - CONTACT_EPSILON).floor() as i32 {
                    if submerged(
                        world,
                        [
                            f64::from(x) + 0.5,
                            min[1].max(f64::from(y)) + CONTACT_EPSILON,
                            f64::from(z) + 0.5,
                        ],
                    ) {
                        return true;
                    }
                }
            }
        }
        false
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
        let speed = if self.crouched {
            config.sneak_speed
        } else if input.sprint {
            if self.grounded {
                config.sprint_speed
            } else {
                // Retain earned launch momentum through bounded vector air
                // control; turning cannot accumulate speed above this target.
                self.velocity[0]
                    .hypot(self.velocity[2])
                    .max(config.sprint_speed)
                    .min(config.sprint_speed + config.sprint_jump_boost)
            }
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

    fn boost_sprint_jump(&mut self, world: &World, input: MovementInput, config: &PhysicsConfig) {
        let speed = self.velocity[0].hypot(self.velocity[2]);
        let alignment =
            self.velocity[0] * input.direction[0] + self.velocity[2] * input.direction[1];
        // A run-up and real vertical clearance prevent standing/ceiling or
        // wall-pushing launches from earning horizontal speed.
        if !input.sprint
            || self.crouched
            || speed < config.walk_speed
            || !alignment.is_finite()
            || alignment <= 0.0
            || self.sweep_axis(world, config, 1, 0.5) < 0.5
        {
            return;
        }
        let boosted =
            (speed + config.sprint_jump_boost).min(config.sprint_speed + config.sprint_jump_boost);
        let velocity = [
            self.velocity[0] * boosted / speed,
            self.velocity[2] * boosted / speed,
        ];
        let mut probe = *self;
        for (axis, v) in [(0, velocity[0]), (2, velocity[1])] {
            let delta = v * PHYSICS_DT;
            let allowed = probe.sweep_axis(world, config, axis, delta);
            if (allowed - delta).abs() > CONTACT_EPSILON {
                return;
            }
            probe.position[axis] += allowed;
        }
        self.velocity[0] = velocity[0];
        self.velocity[2] = velocity[1];
    }

    fn bounds(&self, config: &PhysicsConfig) -> ([f64; 3], [f64; 3]) {
        let [x, y, z] = self.position;
        (
            [x - config.half_width, y, z - config.half_width],
            [
                x + config.half_width,
                y + self.height(config),
                z + config.half_width,
            ],
        )
    }

    /// Pending streaming cells must not be entered by a teleported body.
    pub fn cache_ready(&self, world: &World, config: &PhysicsConfig) -> bool {
        let (min, max) = self.bounds(config);
        for y in
            (min[1] + CONTACT_EPSILON).floor() as i32..=(max[1] - CONTACT_EPSILON).floor() as i32
        {
            for z in (min[2] + CONTACT_EPSILON).floor() as i32
                ..=(max[2] - CONTACT_EPSILON).floor() as i32
            {
                for x in (min[0] + CONTACT_EPSILON).floor() as i32
                    ..=(max[0] - CONTACT_EPSILON).floor() as i32
                {
                    if !world.ready(BlockPos([x, y, z])) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Only called for supported, non-jumping movement. Probe the resolved
    /// X/Z path separately to preserve ledge sliding and diagonal partial support.
    /// Never snap down, fabricate support, or cancel vertical momentum.
    fn guard_ledge(&self, world: &World, config: &PhysicsConfig, axis: usize, delta: f64) -> f64 {
        if delta == 0.0 {
            return delta;
        }
        let supported_at = |distance| {
            let mut candidate = *self;
            candidate.position[axis] += distance;
            candidate.supported(world, config)
        };
        // Scan a bounded normal tick path before bisection so even externally
        // supplied large velocities cannot skip a gap and land on another ledge.
        let steps = (delta.abs() / 0.025).ceil().clamp(1.0, 64.0) as u32;
        let mut safe = 0.0;
        for step in 1..=steps {
            let mut unsafe_distance = delta * f64::from(step) / f64::from(steps);
            if supported_at(unsafe_distance) {
                safe = unsafe_distance;
                continue;
            }
            for _ in 0..20 {
                let middle = (safe + unsafe_distance) * 0.5;
                if supported_at(middle) {
                    safe = middle;
                } else {
                    unsafe_distance = middle;
                }
            }
            return safe;
        }
        delta
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
                    if !world.block(BlockPos(block)).is_collidable() {
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

/// Finds sky-exposed solid ground near a preferred column without changing the
/// world. Tree tops are skipped. Used at startup and respawn, never each tick.
/// An intentionally empty world has no supported spawn; return clear space.
pub fn spawn_position(world: &World, config: &PhysicsConfig, preferred: [f64; 3]) -> [f64; 3] {
    let (world_min, world_max) = world.bounds();
    let px = (preferred[0].floor() as i32).clamp(world_min[0], world_max[0] - 1);
    let pz = (preferred[2].floor() as i32).clamp(world_min[2], world_max[2] - 1);
    let extent = (world_max[0] - world_min[0]).max(world_max[2] - world_min[2]);
    for radius in 0_i32..extent.min(64) {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                let x = px + dx;
                let z = pz + dz;
                if !(world_min[0]..world_max[0]).contains(&x)
                    || !(world_min[2]..world_max[2]).contains(&z)
                {
                    continue;
                }
                if !world.ready(BlockPos([x, 0, z])) {
                    continue;
                }
                for y in (world_min[1]..world_max[1]).rev() {
                    let ground = world.block(BlockPos([x, y, z]));
                    if !ground.is_collidable() {
                        continue;
                    }
                    if matches!(ground, crate::Block::Wood | crate::Block::Leaves) {
                        break;
                    }
                    let surface = f64::from(y + 1);
                    let feet = if (surface..=surface + 1.0).contains(&preferred[1]) {
                        preferred[1]
                    } else {
                        surface
                    };
                    let p = [f64::from(x) + 0.5, feet, f64::from(z) + 0.5];
                    if Player::new(p).fits_in_world(world, config) {
                        return p;
                    }
                    break;
                }
            }
        }
    }
    [
        f64::from(px) + 0.5,
        f64::from(world_max[1]),
        f64::from(pz) + 0.5,
    ]
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod water_tests;

/// Matches the two surface triangles and their shared diagonal exactly.
pub fn submerged(world: &World, point: [f64; 3]) -> bool {
    let p = BlockPos(point.map(|v| v.floor() as i32));
    if world.block(p).fluid().is_none() {
        return false;
    }
    let x = (point[0] - f64::from(p.0[0])) as f32;
    let z = (point[2] - f64::from(p.0[2])) as f32;
    let h = [[0, 0], [1, 0], [0, 1], [1, 1]]
        .map(|[cx, cz]| crate::water_mesh::corner_height(world, p, cx, cz));
    let height = if x + z <= 1. {
        h[0] + (h[1] - h[0]) * x + (h[2] - h[0]) * z
    } else {
        h[3] + (h[2] - h[3]) * (1. - x) + (h[1] - h[3]) * (1. - z)
    };
    point[1] < f64::from(p.0[1]) + f64::from(height)
}
