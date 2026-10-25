//! Authoritative integer clock. No wall clock, engine, lighting or residency dependencies.
pub const UNITS_PER_DAY: u32 = 24_000;
pub const STEPS_PER_UNIT: u32 = 3;
pub const STEPS_PER_DAY: u32 = UNITS_PER_DAY * STEPS_PER_UNIT;

/// At 60 simulation steps/second, one day lasts exactly 20 minutes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldTime {
    day: u64,
    step: u32,
}
impl Default for WorldTime {
    fn default() -> Self {
        Self {
            day: 0,
            step: 1000 * STEPS_PER_UNIT,
        }
    }
}
impl WorldTime {
    pub fn from_parts(day: u64, step: u32) -> Option<Self> {
        (step < STEPS_PER_DAY).then_some(Self { day, step })
    }
    pub fn day(self) -> u64 {
        self.day
    }
    pub fn step(self) -> u32 {
        self.step
    }
    pub fn units(self) -> f64 {
        f64::from(self.step) / f64::from(STEPS_PER_UNIT)
    }
    pub fn advance(&mut self, steps: u32) {
        let total = u64::from(self.step) + u64::from(steps);
        let days = total / u64::from(STEPS_PER_DAY);
        if let Some(day) = self.day.checked_add(days) {
            self.day = day;
            self.step = (total % u64::from(STEPS_PER_DAY)) as u32;
        } else {
            self.day = u64::MAX;
            self.step = STEPS_PER_DAY - 1;
        }
    }
    /// Sunrise east (+X), noon above, sunset west (-X), midnight below.
    pub fn sun_direction(self) -> [f32; 3] {
        let angle = self.units() / f64::from(UNITS_PER_DAY) * std::f64::consts::TAU;
        [angle.cos() as f32, angle.sin() as f32, 0.]
    }
    pub fn moon_direction(self) -> [f32; 3] {
        self.sun_direction().map(|v| -v)
    }
    pub fn daylight(self) -> f32 {
        let t = ((self.sun_direction()[1] + 0.15) / 0.40).clamp(0., 1.);
        0.10 + 0.90 * t * t * (3. - 2. * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_twenty_minutes_rollover_and_fractional_resume() {
        let mut t = WorldTime::default();
        t.advance(60 * 20 * 60);
        assert_eq!(t, WorldTime::from_parts(1, 3000).unwrap());
        t.advance(STEPS_PER_DAY - 2999);
        assert_eq!(t, WorldTime::from_parts(2, 1).unwrap());
        assert_eq!(WorldTime::from_parts(0, STEPS_PER_DAY), None);
    }
    #[test]
    fn cardinal_arc_and_continuity() {
        for (units, expected) in [
            (0, [1., 0., 0.]),
            (6000, [0., 1., 0.]),
            (12000, [-1., 0., 0.]),
            (18000, [0., -1., 0.]),
        ] {
            let t = WorldTime::from_parts(0, units * 3).unwrap();
            for (a, b) in t.sun_direction().into_iter().zip(expected) {
                assert!((a - b).abs() < 0.0001);
            }
            assert_eq!(t.moon_direction(), t.sun_direction().map(|v| -v));
        }
        let first = WorldTime::from_parts(0, 0).unwrap();
        let last = WorldTime::from_parts(0, STEPS_PER_DAY - 1).unwrap();
        assert!((first.daylight() - last.daylight()).abs() < 0.001);
        assert_eq!(WorldTime::from_parts(0, 18000 * 3).unwrap().daylight(), 0.1);
        assert_eq!(WorldTime::from_parts(0, 6000 * 3).unwrap().daylight(), 1.);
    }
    #[test]
    fn clock_never_changes_block_revision_or_mesh_dependencies() {
        let mut w = crate::streaming::new_large_world(123);
        let token = w.mesh_revision(crate::ChunkPos([0; 3]));
        w.time.advance(STEPS_PER_DAY);
        assert_eq!(w.revision(), 0);
        assert_eq!(w.mesh_revision(crate::ChunkPos([0; 3])), token);
        assert_eq!(w.time, w.save_snapshot().time);
        let mut max = WorldTime::from_parts(u64::MAX, STEPS_PER_DAY - 1).unwrap();
        max.advance(u32::MAX);
        assert_eq!(
            max,
            WorldTime::from_parts(u64::MAX, STEPS_PER_DAY - 1).unwrap()
        );
    }
}
