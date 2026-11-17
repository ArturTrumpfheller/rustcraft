// SPDX-License-Identifier: MIT OR Apache-2.0
//! V15-only terrain coupling and physical vertical cave wavelengths.
//! Shared primitives/lattice/carvers remain in caves_v3. Frozen v14 arithmetic
//! stays in density.rs; no historical configuration selects this composition.
use super::{Fields, Heights, Q, STEP};

impl Heights {
    pub(super) fn refined(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        Self::refined_for(16, seed, min, max)
    }
    pub(super) fn refined_for(version: u32, seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        // One extra lattice ring gives central differences even for point plans.
        // Every coordinate has identical terrain inputs independent of plan size.
        let padded = Self::for_terrain(version, seed, min.map(|v| v - STEP), max.map(|v| v + STEP));
        let size: [usize; 2] = std::array::from_fn(|i| {
            (max[i].div_euclid(STEP) + 2 - min[i].div_euclid(STEP)) as usize
        });
        let stride = size[0] + 2;
        let mut values = Vec::with_capacity(size.iter().product());
        let mut slopes = Vec::with_capacity(values.capacity());
        for z in 0..size[1] {
            for x in 0..size[0] {
                let i = x + 1 + (z + 1) * stride;
                values.push(padded.values[i]);
                let dx = (padded.values[i + 1] - padded.values[i - 1]) as f32 / (2 * STEP) as f32;
                let dz = (padded.values[i + stride] - padded.values[i - stride]) as f32
                    / (2 * STEP) as f32;
                slopes.push(dx.abs().max(dz.abs()));
            }
        }
        Self {
            values,
            slopes: Some(slopes),
            exposure: None,
        }
    }
}
impl Fields {
    pub(super) fn refined(&self, p: [i32; 3], height: i32, slope: f32) -> [i32; 5] {
        // Keep the noise wavelength in physical blocks. Compressing [-64,63]
        // into [-64,2] also compressed every layer, ridge and passage height.
        // The offset centers the reference's 2D elevation band in our geology;
        // bounds/sea-level and the foundation fade remain RustCraft coordinates.
        let y = f64::from(p[1] + 32);
        let depth = (height - p[1]) as f32;
        let upland = ((height - 40) as f32 / 64.).clamp(0., 1.);
        let exposure = slope / (1. + slope);
        let at = |index: usize, xz: f64, ys: f64| {
            self.0[index].get([f64::from(p[0]) * xz, y * ys, f64::from(p[2]) * xz])
        };
        let rough = (-0.05 * at(8, 1., 1.) - 0.05) * (at(7, 1., 1.).abs() - 0.4);
        let rarity = at(5, 2., 1.);
        let scale = if rarity < -0.5 {
            0.75
        } else if rarity < 0. {
            1.
        } else if rarity < 0.5 {
            1.5
        } else {
            2.
        };
        let spaghetti3 = (scale
            * at(3, 1. / f64::from(scale), 1. / f64::from(scale))
                .abs()
                .max(at(4, 1. / f64::from(scale), 1. / f64::from(scale)).abs())
            - 0.0765
            - 0.011500001 * at(6, 1., 1.))
        .clamp(-1., 1.);
        // Broad entrance density responds to existing terrain, rather than an
        // unconditional high-altitude opening field. Slopes expose side walls;
        // lowland openings need less broad-field bias than high plateaus.
        let ridges = rough + spaghetti3;
        let covered = ridges.max(0.08 * (1. - exposure) - depth * 0.02);
        let entrance = (at(2, 0.75, 0.5) + 0.31 + 0.16 * upland - 0.08 * exposure
            + 0.3 * ((-31. - f64::from(p[1])) / 40.).clamp(0., 1.) as f32)
            // Apply the extra cover to elevated terrain, grading it away on
            // slopes. Extending this rule to ordinary plains removes their thin
            // natural entrances, despite those regions already having too few.
            // Broad entrance noise and traditional walks remain independent.
            .min(ridges + upland * (covered - ridges));
        let sloped = depth / (8. * (1. + 0.5 * exposure));
        let mut main = if sloped < 1.5625 {
            sloped.min(5. * entrance)
        } else {
            let rarity2 = at(11, 2., 1.);
            let scale2 = if rarity2 < -0.75 {
                0.5
            } else if rarity2 < -0.5 {
                0.75
            } else if rarity2 < 0.5 {
                1.
            } else if rarity2 < 0.75 {
                2.
            } else {
                3.
            };
            let thick = -0.95 - 0.34999996 * at(12, 2., 1.);
            let spaghetti2 = (scale2 * at(9, 1. / f64::from(scale2), 1. / f64::from(scale2)).abs()
                + 0.083 * thick)
                .max(((8. * at(10, 1., 0.) + (8. - (y + 64.) / 8.) as f32).abs() + thick).powi(3))
                .clamp(-1., 1.);
            let layer = at(1, 1., 8.);
            let cheese = (at(0, 1., 2. / 3.) + 0.27).clamp(-1., 1.)
                + (1.5 - 0.64 * sloped).clamp(0., 0.5)
                + 4. * layer * layer
                // Extra mountain overburden must not automatically become an
                // equally porous copy of the lowland underground. Retain narrow
                // families and deep systems while grading broad high caverns.
                + 0.10 * upland * ((p[1] - 2) as f32 / 32.).clamp(0., 1.);
            let pillar = (2. * at(13, 25., 0.3) - at(14, 1., 1.) - 1.)
                * (0.55 * at(15, 1., 1.) + 0.55).powi(3);
            // Blend only near the intersection of two density boundaries. The
            // maximum change is 0.015 density units, leaving chamber interiors
            // and isolated far-field rock unchanged instead of widening all
            // tunnels to improve a connectivity counter.
            let a = cheese.min(entrance);
            let b = spaghetti2 + rough;
            let overlap = (0.06 - (a - b).abs()).max(0.);
            (a.min(b) - overlap * overlap / 0.24).max(if pillar >= 0.03 { pillar } else { -1e6 })
        };
        let bottom = ((p[1] + 60) as f32 / 12.).clamp(0., 1.);
        main = 0.1171875 + bottom * (main - 0.1171875);
        [
            main,
            at(16, 1., 1.),
            // Noodles are links underground, not independent pinholes through
            // the surface. A density fade leaves carvers/entrances responsible
            // for openings and has no effect below ten blocks of cover.
            (-0.075 - 0.025 * at(17, 1., 1.)).max((6. - depth) * 0.025),
            at(18, 8. / 3., 8. / 3.),
            at(19, 8. / 3., 8. / 3.),
        ]
        .map(|v| (v.clamp(-32., 32.) * Q as f32).round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Lattice;
    use super::*;

    #[test]
    fn refined_lattice_is_identical_in_point_halo_and_wide_negative_plans() {
        for seed in [0, 7, 42, 2026, u64::MAX] {
            let fields = Fields::new(seed);
            let large_heights = Heights::refined(seed, [-32, -32], [32, 32]);
            let halo_heights = Heights::refined(seed, [-1, -1], [16, 16]);
            for bottom in [-64, -16, 0, 16, 80, 176] {
                let a = Lattice::build(
                    &fields,
                    &large_heights,
                    [-32, -32],
                    [32, 32],
                    bottom,
                    bottom + 16,
                );
                let b = Lattice::build(
                    &fields,
                    &halo_heights,
                    [-1, -1],
                    [16, 16],
                    bottom,
                    bottom + 16,
                );
                for z in -1..=16 {
                    for x in -1..=16 {
                        for y in bottom..bottom + 16 {
                            let p = [x, y, z];
                            assert_eq!(a.sample(p), b.sample(p), "{seed} {p:?}");
                            assert_eq!(a.contains(p), b.contains(p), "{seed} {p:?}");
                            let v = a.sample(p);
                            let raw = v[0] < 0
                                || (v[1] >= 0
                                    && i64::from(v[2])
                                        + i64::from(v[3].abs().max(v[4].abs())) * 3 / 2
                                        < 0);
                            assert_eq!(
                                a.contains(p),
                                raw,
                                "interval rejected cavity: {seed} {p:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
