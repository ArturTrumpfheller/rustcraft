//! Default locomotion bindings, separate from simulation commands. A future
//! settings adapter can replace this resource without changing voxel physics.
use bevy::prelude::*;
use voxel_core::physics::MovementInput;

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MovementBindings {
    pub forward: KeyCode,
    pub backward: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub sprint: KeyCode,
    pub sneak: KeyCode,
    pub jump: KeyCode,
    pub respawn: KeyCode,
    pub inventory: KeyCode,
    pub menu: KeyCode,
    pub hotbar: [KeyCode; 9],
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
            inventory: KeyCode::KeyE,
            menu: KeyCode::Escape,
            hotbar: [
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
                KeyCode::Digit8,
                KeyCode::Digit9,
            ],
        }
    }
}

impl MovementBindings {
    pub fn keys(&self) -> [KeyCode; 19] {
        let mut out = [
            self.forward,
            self.backward,
            self.left,
            self.right,
            self.jump,
            self.sprint,
            self.sneak,
            self.respawn,
            self.inventory,
            self.menu,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        out[10..].copy_from_slice(&self.hotbar);
        out
    }
    pub fn from_keys(k: [KeyCode; 19]) -> Self {
        Self {
            forward: k[0],
            backward: k[1],
            left: k[2],
            right: k[3],
            jump: k[4],
            sprint: k[5],
            sneak: k[6],
            respawn: k[7],
            inventory: k[8],
            menu: k[9],
            hotbar: k[10..].try_into().unwrap(),
        }
    }
}

pub(crate) const DOUBLE_TAP_WINDOW: std::time::Duration = std::time::Duration::from_millis(250);

/// Frame-edge gestures only. Physics still owns support and legal jump cadence.
#[derive(Resource, Default)]
pub(crate) struct MovementGestures {
    bindings: Option<MovementBindings>,
    first_press: Option<std::time::Duration>,
    forward_down: bool,
    pub double_tap_sprint: bool,
    pub jump_ready: bool,
    // Keys held during a modal/capture transition need a release before reuse.
    blocked_movement: [bool; 6],
}

impl MovementGestures {
    pub fn block_held_movement(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        bindings: &MovementBindings,
    ) {
        self.blocked_movement = [
            bindings.forward,
            bindings.backward,
            bindings.left,
            bindings.right,
            bindings.sprint,
            bindings.sneak,
        ]
        .map(|k| keys.pressed(k));
    }

    pub fn command(
        &self,
        keys: &ButtonInput<KeyCode>,
        bindings: &MovementBindings,
        yaw: f64,
    ) -> MovementInput {
        let held = |i: usize, key| keys.pressed(key) && !self.blocked_movement[i];
        MovementInput {
            sneak: held(5, bindings.sneak),
            ..MovementInput::from_yaw(
                [
                    f64::from(held(3, bindings.right)) - f64::from(held(2, bindings.left)),
                    f64::from(held(1, bindings.backward)) - f64::from(held(0, bindings.forward)),
                ],
                yaw,
                held(4, bindings.sprint),
                false,
            )
        }
    }

    pub fn update(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        bindings: &MovementBindings,
        now: std::time::Duration,
        enabled: bool,
    ) {
        let forward = keys.pressed(bindings.forward);
        if !enabled || self.bindings != Some(*bindings) {
            *self = Self {
                bindings: Some(*bindings),
                forward_down: forward,
                jump_ready: !keys.pressed(bindings.jump),
                blocked_movement: self.blocked_movement,
                ..Default::default()
            };
            return;
        }
        for (blocked, key) in self.blocked_movement.iter_mut().zip([
            bindings.forward,
            bindings.backward,
            bindings.left,
            bindings.right,
            bindings.sprint,
            bindings.sneak,
        ]) {
            *blocked &= keys.pressed(key);
        }
        if !keys.pressed(bindings.jump) {
            self.jump_ready = true;
        }
        if keys.pressed(bindings.sneak) || keys.pressed(bindings.backward) {
            self.first_press = None;
            self.double_tap_sprint = false;
        } else {
            if self
                .first_press
                .is_some_and(|t| now.saturating_sub(t) > DOUBLE_TAP_WINDOW)
            {
                self.first_press = None;
            }
            if keys.just_pressed(bindings.forward)
                && (!self.forward_down || keys.just_released(bindings.forward))
            {
                if self.first_press.take().is_some() {
                    self.double_tap_sprint = true;
                } else {
                    self.first_press = Some(now);
                }
            }
        }
        if !forward {
            self.double_tap_sprint = false;
        }
        self.forward_down = forward;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(
        g: &mut MovementGestures,
        keys: &mut ButtonInput<KeyCode>,
        b: &MovementBindings,
        ms: u64,
        enabled: bool,
    ) {
        g.update(keys, b, Duration::from_millis(ms), enabled);
        keys.clear();
    }

    #[test]
    fn double_forward_requires_distinct_edges_within_250ms_and_ends_on_release() {
        for (ms, expected) in [(249, true), (250, true), (251, false)] {
            let b = MovementBindings::default();
            let mut k = ButtonInput::default();
            let mut g = MovementGestures::default();
            sample(&mut g, &mut k, &b, 0, false);
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 10, true);
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 20, true);
            assert!(!g.double_tap_sprint, "held/repeat is not a second press");
            k.release(b.forward);
            sample(&mut g, &mut k, &b, 50, true);
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 10 + ms, true);
            assert_eq!(g.double_tap_sprint, expected);
            sample(&mut g, &mut k, &b, 1000, true);
            assert_eq!(
                g.double_tap_sprint, expected,
                "second hold persists beyond window"
            );
            k.release(b.forward);
            sample(&mut g, &mut k, &b, 1001, true);
            assert!(!g.double_tap_sprint);
        }
    }

    #[test]
    fn brief_forward_taps_and_release_repress_in_one_frame_are_recognized() {
        let b = MovementBindings::default();
        let mut k = ButtonInput::default();
        let mut g = MovementGestures::default();
        sample(&mut g, &mut k, &b, 0, false);
        k.press(b.forward);
        k.release(b.forward);
        sample(&mut g, &mut k, &b, 10, true);
        k.press(b.forward);
        sample(&mut g, &mut k, &b, 100, true);
        assert!(g.double_tap_sprint);
        k.release(b.forward);
        sample(&mut g, &mut k, &b, 110, true);
        k.press(b.forward);
        sample(&mut g, &mut k, &b, 120, true);
        k.release(b.forward);
        k.press(b.forward);
        sample(&mut g, &mut k, &b, 130, true);
        assert!(g.double_tap_sprint);
    }

    #[test]
    fn conflicting_keys_modal_resets_and_binding_changes_disarm_gestures() {
        for cancel in [Some(KeyCode::KeyS), Some(KeyCode::ShiftLeft), None] {
            let mut b = MovementBindings::default();
            let mut k = ButtonInput::default();
            let mut g = MovementGestures::default();
            sample(&mut g, &mut k, &b, 0, false);
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 10, true);
            k.release(b.forward);
            if let Some(key) = cancel {
                k.press(key);
            }
            sample(&mut g, &mut k, &b, 20, cancel.is_some());
            if let Some(key) = cancel {
                k.release(key);
            }
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 30, true);
            assert!(!g.double_tap_sprint);
            b.forward = KeyCode::ArrowUp;
            k.press(b.forward);
            sample(&mut g, &mut k, &b, 40, true);
            assert!(!g.double_tap_sprint);
            k.press(b.jump);
            sample(&mut g, &mut k, &b, 50, false);
            sample(&mut g, &mut k, &b, 60, true);
            assert!(!g.jump_ready);
            k.release(b.jump);
            sample(&mut g, &mut k, &b, 70, true);
            assert!(g.jump_ready);
        }
    }
}
