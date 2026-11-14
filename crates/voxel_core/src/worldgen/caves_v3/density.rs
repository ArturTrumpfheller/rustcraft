// SPDX-License-Identifier: MIT OR Apache-2.0
//! Horizon-informed signed cave composition on a globally aligned Q20 lattice.
//! Negative density is cavity. Terrain remains v13: only its local depth enters
//! the sloped-cheese adapter. The reference's sea/foundation span is mapped to
//! RustCraft's shorter underground; horizontal scale is unchanged.
use super::{Generator, MIN_Y, Plan, STEP, horizon::Noise, mix};
use crate::{Block, BlockPos, Chunk, ChunkPos, LocalPos};
use std::sync::{Arc, OnceLock};
const Q: i32 = 1 << 20;
const NAMES: [(&str, i32, f64, &[f64]); 20] = [
    (
        "cave_cheese",
        -8,
        0.8361300524356068,
        &[0.5, 1., 2., 1., 2., 1., 0., 2., 0.],
    ),
    ("cave_layer", -8, 0.955388882960065, &[1.]),
    ("cave_entrance", -7, 0.8500634887071167, &[0.4, 0.5, 1.]),
    ("spaghetti_3d_1", -7, 0.955388882960065, &[1.]),
    ("spaghetti_3d_2", -7, 0.955388882960065, &[1.]),
    ("spaghetti_3d_rarity", -11, 0.955388882960065, &[1.]),
    ("spaghetti_3d_thickness", -8, 0.955388882960065, &[1.]),
    ("spaghetti_roughness", -5, 0.955388882960065, &[1.]),
    (
        "spaghetti_roughness_modulator",
        -8,
        0.955388882960065,
        &[1.],
    ),
    ("spaghetti_2d", -7, 0.955388882960065, &[1.]),
    ("spaghetti_2d_elevation", -8, 0.955388882960065, &[1.]),
    ("spaghetti_2d_modulator", -11, 0.955388882960065, &[1.]),
    ("spaghetti_2d_thickness", -11, 0.955388882960065, &[1.]),
    ("pillar", -7, 0.9494731054427981, &[1., 1.]),
    ("pillar_rareness", -8, 0.955388882960065, &[1.]),
    ("pillar_thickness", -8, 0.955388882960065, &[1.]),
    ("noodle", -8, 0.955388882960065, &[1.]),
    ("noodle_thickness", -8, 0.955388882960065, &[1.]),
    ("noodle_ridge_a", -7, 0.955388882960065, &[1.]),
    ("noodle_ridge_b", -7, 0.955388882960065, &[1.]),
];
pub(in crate::worldgen) struct Fields([Noise; 20]);
impl Fields {
    fn new(seed: u64) -> Self {
        Self(NAMES.map(|(name, base, amplitude, modifiers)| {
            Noise::new(seed, name, base, amplitude, modifiers)
        }))
    }
    fn values(&self, p: [i32; 3], height: i32) -> [i32; 5] {
        let y = if p[1] <= 2 {
            -64. + f64::from(p[1] + 64) * 127. / 66.
        } else {
            f64::from(p[1] + 61)
        };
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
        let entrance = (at(2, 0.75, 0.5) + 0.37 + 0.3 * ((30. - y) / 40.).clamp(0., 1.) as f32)
            .min(rough + spaghetti3);
        let sloped = (height - p[1]) as f32 / 8.;
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
                + 4. * layer * layer;
            let pillar = (2. * at(13, 25., 0.3) - at(14, 1., 1.) - 1.)
                * (0.55 * at(15, 1., 1.) + 0.55).powi(3);
            cheese
                .min(entrance)
                .min(spaghetti2 + rough)
                .max(if pillar >= 0.03 { pillar } else { -1e6 })
        };
        let bottom = ((y + 64.) / 24.).clamp(0., 1.) as f32;
        main = 0.1171875 + bottom * (main - 0.1171875);
        [
            main,
            at(16, 1., 1.),
            -0.075 - 0.025 * at(17, 1., 1.),
            at(18, 8. / 3., 8. / 3.),
            at(19, 8. / 3., 8. / 3.),
        ]
        .map(|v| (v.clamp(-32., 32.) * Q as f32).round() as i32)
    }
}
pub(super) type Cache = Arc<OnceLock<Fields>>;
pub(super) struct Network {
    fields: Cache,
    seed: u64,
    min: [i32; 2],
    max: [i32; 2],
    bands: OnceLock<Box<[OnceLock<Lattice>; 16]>>,
    heights: OnceLock<Heights>,
}
// Terrain profiles are independent of Y. Compute their exact top once for
// the plan's aligned X/Z vertices, and share it across all lazy vertical bands.
struct Heights {
    values: Vec<i32>,
}
impl Heights {
    fn new(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        let min = min.map(|v| v.div_euclid(STEP));
        let size: [usize; 2] =
            std::array::from_fn(|i| (max[i].div_euclid(STEP) + 2 - min[i]) as usize);
        let values = (0..size[0] * size[1])
            .map(|i| {
                super::super::terrain_v5::height(
                    seed,
                    (min[0] + (i % size[0]) as i32) * STEP,
                    (min[1] + (i / size[0]) as i32) * STEP,
                )
            })
            .collect();
        Self { values }
    }
}
struct Lattice {
    min: [i32; 3],
    size: [usize; 3],
    values: Vec<[i32; 5]>,
    active: Vec<bool>,
}
impl Network {
    pub(super) fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        Self {
            fields: g.cave_noise.get_or_init(Default::default).clone(),
            seed: g.config.seed,
            min,
            max,
            bands: OnceLock::new(),
            heights: OnceLock::new(),
        }
    }
    fn lattice(&self, y: i32) -> &Lattice {
        let band = (y + 64).div_euclid(16) as usize;
        self.bands
            .get_or_init(|| Box::new(std::array::from_fn(|_| OnceLock::new())))[band]
            .get_or_init(|| {
                let fields = self.fields.get_or_init(|| Fields::new(self.seed));
                Lattice::build(
                    fields,
                    self.heights
                        .get_or_init(|| Heights::new(self.seed, self.min, self.max)),
                    self.min,
                    self.max,
                    band as i32 * 16 - 64,
                    band as i32 * 16 - 48,
                )
            })
    }
    pub(super) fn contains(&self, p: [i32; 3]) -> bool {
        self.lattice(p[1]).contains(p)
    }
    pub(super) fn carve_if(
        &self,
        chunk: &mut Chunk,
        cp: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        if chunk.uniform() == Some(Block::Air) {
            return;
        }
        let o = cp.0.map(|v| v * 16);
        if o[1] + 15 < MIN_Y || o[1] >= 192 {
            return;
        }
        let lattice = self.lattice(o[1].max(MIN_Y));
        for z in (0..16).step_by(4) {
            for x in (0..16).step_by(4) {
                for y in (0..16).step_by(4) {
                    let corner = [o[0] + x as i32, o[1] + y as i32, o[2] + z as i32];
                    if !lattice.cell_active(corner) {
                        continue;
                    }
                    for zz in z..z + 4 {
                        for xx in x..x + 4 {
                            let h = height(o[0] + xx as i32, o[2] + zz as i32);
                            for yy in y..y + 4 {
                                let p = BlockPos([
                                    o[0] + xx as i32,
                                    o[1] + yy as i32,
                                    o[2] + zz as i32,
                                ]);
                                let l = LocalPos::new([xx, yy, zz]).unwrap();
                                if Plan::allowed(p.0, h, true)
                                    && chunk.get(l) != Block::Air
                                    && lattice.contains(p.0)
                                    && permitted(p, h)
                                {
                                    chunk.set(l, Block::Air);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
impl Lattice {
    fn build(
        fields: &Fields,
        heights: &Heights,
        min: [i32; 2],
        max: [i32; 2],
        bottom: i32,
        top: i32,
    ) -> Self {
        let min = [
            min[0].div_euclid(STEP),
            bottom.div_euclid(STEP),
            min[1].div_euclid(STEP),
        ];
        let size = [
            (max[0].div_euclid(STEP) + 2 - min[0]) as usize,
            (top.div_euclid(STEP) + 1 - min[1]) as usize,
            (max[1].div_euclid(STEP) + 2 - min[2]) as usize,
        ];
        let mut values = Vec::with_capacity(size.iter().product());
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let p = [
                        (min[0] + x as i32) * STEP,
                        (min[1] + y as i32) * STEP,
                        (min[2] + z as i32) * STEP,
                    ];
                    let h = heights.values[x + z * size[0]];
                    values.push(fields.values(p, h));
                }
            }
        }
        let mut result = Self {
            min,
            size,
            values,
            active: vec![false; size.iter().product()],
        };
        for z in 0..size[2] - 1 {
            for y in 0..size[1] - 1 {
                for x in 0..size[0] - 1 {
                    let v = result.corners([x, y, z]);
                    // The main field is multilinear. A noodle can be negative only if
                    // gate >= 0 and both ridges can approach zero inside this cell.
                    let abs_min = |k: usize| {
                        let lo = v.iter().map(|v| v[k]).min().unwrap();
                        let hi = v.iter().map(|v| v[k]).max().unwrap();
                        if lo > 0 {
                            lo
                        } else if hi < 0 {
                            -hi
                        } else {
                            0
                        }
                    };
                    result.active[x + (y + z * size[1]) * size[0]] = v.iter().any(|v| v[0] < 0)
                        || (v.iter().any(|v| v[1] >= 0)
                            && i64::from(v.iter().map(|v| v[2]).min().unwrap())
                                + i64::from(abs_min(3).max(abs_min(4))) * 3 / 2
                                < 0);
                }
            }
        }
        result
    }
    fn cell(&self, p: [i32; 3]) -> [usize; 3] {
        std::array::from_fn(|i| (p[i].div_euclid(STEP) - self.min[i]) as usize)
    }
    fn cell_active(&self, p: [i32; 3]) -> bool {
        let c = self.cell(p);
        self.active[c[0] + (c[1] + c[2] * self.size[1]) * self.size[0]]
    }
    fn corners(&self, c: [usize; 3]) -> [[i32; 5]; 8] {
        std::array::from_fn(|i| {
            self.values[c[0]
                + (i & 1)
                + (c[1] + ((i >> 1) & 1) + (c[2] + ((i >> 2) & 1)) * self.size[1]) * self.size[0]]
        })
    }
    fn sample(&self, p: [i32; 3]) -> [i32; 5] {
        let v = self.corners(self.cell(p));
        let f = p.map(|v| v.rem_euclid(STEP));
        std::array::from_fn(|k| {
            mix(
                mix(
                    mix(v[0][k], v[1][k], f[0], STEP),
                    mix(v[2][k], v[3][k], f[0], STEP),
                    f[1],
                    STEP,
                ),
                mix(
                    mix(v[4][k], v[5][k], f[0], STEP),
                    mix(v[6][k], v[7][k], f[0], STEP),
                    f[1],
                    STEP,
                ),
                f[2],
                STEP,
            )
        })
    }
    fn contains(&self, p: [i32; 3]) -> bool {
        if !self.cell_active(p) {
            return false;
        }
        let v = self.sample(p);
        v[0] < 0
            || (v[1] >= 0 && i64::from(v[2]) + i64::from(v[3].abs().max(v[4].abs())) * 3 / 2 < 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapted_noise_matches_executed_horizon_reference() {
        let mut previous = None;
        let mut fields = None;
        for line in include_str!("../../../tests/fixtures/horizon-noise.csv")
            .lines()
            .skip(1)
        {
            let r: Vec<_> = line.split(',').collect();
            let seed: u64 = r[0].parse().unwrap();
            if previous != Some(seed) {
                fields = Some(Fields::new(seed));
                previous = Some(seed);
            }
            let index: usize = r[1].parse().unwrap();
            let p = std::array::from_fn(|i| r[i + 2].parse().unwrap());
            let expected = f32::from_bits(r[5].parse().unwrap());
            let actual = fields.as_ref().unwrap().0[index].get(p);
            assert!(
                (actual - expected).abs() <= 0.000002,
                "{line}: {actual} vs {expected}"
            );
        }
    }
    #[test]
    fn global_density_seams_and_conservative_rejection() {
        let fields = Fields::new(42);
        let a = Lattice::build(
            &fields,
            &Heights::new(42, [-32, -32], [32, 32]),
            [-32, -32],
            [32, 32],
            -32,
            -16,
        );
        let b = Lattice::build(
            &fields,
            &Heights::new(42, [-1, -1], [16, 16]),
            [-1, -1],
            [16, 16],
            -32,
            -16,
        );
        for x in -1..=16 {
            for z in -1..=16 {
                for y in -32..-16 {
                    let p = [x, y, z];
                    assert_eq!(a.sample(p), b.sample(p));
                    let v = a.sample(p);
                    let raw = v[0] < 0
                        || (v[1] >= 0
                            && i64::from(v[2]) + i64::from(v[3].abs().max(v[4].abs())) * 3 / 2 < 0);
                    assert_eq!(a.contains(p), raw, "interval rejected cavity {p:?}");
                }
            }
        }
    }
}
