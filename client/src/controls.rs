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

pub(crate) const DOUBLE_TAP_WINDOW: std::time::Duration = std::time::Duration::from_millis(250);

/// Frame-edge gestures only. Physics still owns support and legal jump cadence.
#[derive(Resource, Default)]
pub(crate) struct MovementGestures {
    bindings: Option<MovementBindings>,
    first_press: Option<std::time::Duration>,
    forward_down: bool,
    pub double_tap_sprint: bool,
    pub jump_ready: bool,
}

impl MovementGestures {
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
                ..Default::default()
            };
            return;
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
