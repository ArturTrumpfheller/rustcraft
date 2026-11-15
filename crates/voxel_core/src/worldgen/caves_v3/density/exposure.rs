// SPDX-License-Identifier: MIT OR Apache-2.0
//! V16-only coherent surface exposure. V15 density and deep carvers are frozen.
use super::{Heights, STEP};
use crate::worldgen::terrain_v4::gradient2;
const Q: i32 = 1024;

#[derive(Clone, Copy)]
pub(in crate::worldgen::caves_v3) struct Column {
    height: i32,
    band: i32,
    closure: i32,
}
impl Column {
    pub(super) fn new(height: i32, slope: f32, field: i32) -> Self {
        let slope = (slope * Q as f32).round() as i32;
        let exposure = slope * Q / (Q + slope);
        let high = ((height - 40) * Q / 64).clamp(0, Q);
        // Quiet lowlands and exposed cliffs retain more openings than upland
        // plateaus. Independent, smoothly correlated fields avoid regular spacing.
        let threshold = -100 + high * 200 / Q - exposure * 100 / Q;
        let t = ((threshold + 80 - field) * Q / 160).clamp(0, Q);
        let closure = t * t / Q * (3 * Q - 2 * t) / Q;
        Self {
            height,
            band: 16 - exposure * 4 / Q,
            closure,
        }
    }
    pub(in crate::worldgen::caves_v3) fn amount(self, y: i32) -> i32 {
        let t = ((self.height - y).max(0) * Q / self.band).clamp(0, Q);
        let remaining = Q - t;
        // Cubic relief restores the original tunnel rapidly with cover. All
        // influence is exactly zero at/below the band, not a deep density shift.
        self.closure * remaining / Q * remaining / Q * remaining / Q
    }
}
impl Heights {
    pub(super) fn exposed(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        let mut result = Self::refined(seed, min, max);
        let start = min.map(|v| v.div_euclid(STEP));
        let size: [usize; 2] =
            std::array::from_fn(|i| (max[i].div_euclid(STEP) + 2 - start[i]) as usize);
        result.exposure = Some(
            (0..size[0] * size[1])
                .map(|i| {
                    let x = (start[0] + (i % size[0]) as i32) * STEP * 4096;
                    let z = (start[1] + (i / size[0]) as i32) * STEP * 4096;
                    (gradient2(seed, x, z, 144, 144, 0x421601) * 2
                        + gradient2(seed, x, z, 56, 56, 0x421602))
                        / 3
                })
                .collect(),
        );
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relief_is_bounded_monotone_and_exactly_absent_below_its_band() {
        for height in [-60, 0, 40, 104, 191] {
            for slope in [0., 0.5, 1., 4., 32.] {
                for field in [-1024, -100, 0, 100, 1024] {
                    let c = Column::new(height, slope, field);
                    let mut previous = 1024;
                    for cover in 0..=32 {
                        let amount = c.amount(height - cover);
                        assert!((0..=previous).contains(&amount));
                        if cover >= 16 {
                            assert_eq!(amount, 0);
                        }
                        previous = amount;
                    }
                    assert_eq!(c.amount(height - c.band), 0);
                    assert!(c.amount(height - 8) <= c.amount(height) / 8);
                }
            }
        }
    }
    #[test]
    fn exposed_cliffs_keep_more_surface_relief_than_flat_uplands() {
        let flat = Column::new(104, 0., 0);
        let cliff = Column::new(104, 4., 0);
        assert!(cliff.amount(104) < flat.amount(104));
        assert!(cliff.band < flat.band);
        assert!(Column::new(32, 0., 0).amount(32) < flat.amount(104));
    }
}
