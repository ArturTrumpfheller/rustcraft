// SPDX-License-Identifier: MIT OR Apache-2.0
//! Continuous v14 cavities between locally owned features. Intersecting signed
//! fields make winding routes; a separate broad field makes irregular voids.
//! Lattice intervals conservatively reject whole cells before voxel evaluation.
use super::{MIN_Y, Plan, STEP, mix, noise3};
use crate::{Block, BlockPos, Chunk, ChunkPos, LocalPos};
use std::sync::OnceLock;

pub(super) struct Network {
    seed: u64,
    min: [i32; 2],
    max: [i32; 2],
    // Surface-only vegetation/root queries never construct underground fields.
    bands: OnceLock<Box<[OnceLock<Lattice>; 16]>>,
}
struct Lattice {
    min: [i32; 3],
    size: [usize; 3],
    values: Vec<[i16; 3]>,
    threshold: Vec<i16>,
    active: Vec<bool>,
}
impl Network {
    pub(super) fn new(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        Self {
            seed,
            min,
            max,
            bands: OnceLock::new(),
        }
    }
    fn lattice(&self, y: i32) -> &Lattice {
        let band = (y + 64).div_euclid(16) as usize;
        let bands = self
            .bands
            .get_or_init(|| Box::new(std::array::from_fn(|_| OnceLock::new())));
        bands[band].get_or_init(|| {
            Lattice::build(
                self.seed,
                self.min,
                self.max,
                (band as i32 * 16 - 64).max(MIN_Y),
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
                    if corner[1] < MIN_Y || !lattice.cell_active(corner) {
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
                                let local = LocalPos::new([xx, yy, zz]).unwrap();
                                if Plan::allowed(p.0, h, false)
                                    && chunk.get(local) != Block::Air
                                    && permitted(p, h)
                                    && lattice.contains(p.0)
                                {
                                    chunk.set(local, Block::Air);
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
    fn build(seed: u64, min: [i32; 2], max: [i32; 2], bottom: i32, top: i32) -> Self {
        let min = [
            min[0].div_euclid(STEP),
            bottom / STEP,
            min[1].div_euclid(STEP),
        ];
        let size = [
            (max[0].div_euclid(STEP) + 2 - min[0]) as usize,
            (top / STEP + 1 - min[1]) as usize,
            (max[1].div_euclid(STEP) + 2 - min[2]) as usize,
        ];
        let mut values = Vec::with_capacity(size.iter().product());
        let mut threshold = Vec::with_capacity(size[0] * size[2]);
        for z in 0..size[2] {
            for x in 0..size[0] {
                let xx = (min[0] + x as i32) * STEP;
                let zz = (min[2] + z as i32) * STEP;
                let region = super::super::noise(seed, xx, zz, 512, 0xe40a);
                threshold.push((560 - region * 160 / 1024) as i16);
            }
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let p = [
                        (min[0] + x as i32) * STEP,
                        (min[1] + y as i32) * STEP,
                        (min[2] + z as i32) * STEP,
                    ];
                    values.push([
                        noise3(seed, p, [64, 40, 64], 0xe401) as i16,
                        noise3(seed, p, [80, 52, 80], 0xe402) as i16,
                        noise3(seed, p, [104, 60, 104], 0xe403) as i16,
                    ]);
                }
            }
        }
        let mut result = Self {
            min,
            size,
            values,
            threshold,
            active: vec![false; size.iter().product()],
        };
        for z in 0..size[2] - 1 {
            for y in 0..size[1] - 1 {
                for x in 0..size[0] - 1 {
                    let c = [x, y, z];
                    let corners = result.corners(c);
                    let lo: [i32; 3] =
                        std::array::from_fn(|k| corners.iter().map(|v| v[k]).min().unwrap());
                    let hi: [i32; 3] =
                        std::array::from_fn(|k| corners.iter().map(|v| v[k]).max().unwrap());
                    let min_abs = |k: usize| {
                        if lo[k] > 0 {
                            lo[k]
                        } else if hi[k] < 0 {
                            -hi[k]
                        } else {
                            0
                        }
                    };
                    let width = 22 + (hi[2] + 400).clamp(0, 800) / 25;
                    let t = [
                        result.threshold[x + z * size[0]],
                        result.threshold[x + 1 + z * size[0]],
                        result.threshold[x + (z + 1) * size[0]],
                        result.threshold[x + 1 + (z + 1) * size[0]],
                    ];
                    result.active[x + (y + z * size[1]) * size[0]] = hi[2]
                        > i32::from(*t.iter().min().unwrap())
                        || (min_abs(0) < width && min_abs(1) < width * 5 / 4);
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
    fn corners(&self, c: [usize; 3]) -> [[i32; 3]; 8] {
        std::array::from_fn(|i| {
            self.values[c[0]
                + (i & 1)
                + (c[1] + ((i >> 1) & 1) + (c[2] + ((i >> 2) & 1)) * self.size[1]) * self.size[0]]
                .map(i32::from)
        })
    }
    fn sample(&self, p: [i32; 3]) -> ([i32; 3], i32) {
        let c = self.cell(p);
        let f = p.map(|v| v.rem_euclid(STEP));
        let v = self.corners(c);
        let fields = std::array::from_fn(|k| {
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
        });
        let i = c[0] + c[2] * self.size[0];
        let t = mix(
            mix(
                i32::from(self.threshold[i]),
                i32::from(self.threshold[i + 1]),
                f[0],
                STEP,
            ),
            mix(
                i32::from(self.threshold[i + self.size[0]]),
                i32::from(self.threshold[i + self.size[0] + 1]),
                f[0],
                STEP,
            ),
            f[2],
            STEP,
        );
        (fields, t)
    }
    fn contains(&self, p: [i32; 3]) -> bool {
        if !self.cell_active(p) {
            return false;
        }
        let ([a, b, c], threshold) = self.sample(p);
        let width = 22 + (c + 400).clamp(0, 800) / 25;
        c - a.abs() / 5 > threshold || (a.abs() < width && b.abs() < width * 5 / 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_signed_continuous_field_fixture() {
        for line in include_str!("../../../tests/fixtures/caves-v14-network.csv")
            .lines()
            .skip(1)
        {
            let v: Vec<_> = line.split(',').collect();
            let seed = v[0].parse().unwrap();
            let p: [i32; 3] = std::array::from_fn(|i| v[i + 1].parse().unwrap());
            let n = Network::new(seed, [p[0], p[2]], [p[0], p[2]]);
            let ([a, b, c], t) = n.lattice(p[1]).sample(p);
            let expected: Vec<i32> = v[4..].iter().map(|s| s.parse().unwrap()).collect();
            assert_eq!(
                [a, b, c, t, i32::from(n.contains(p))],
                expected.as_slice(),
                "{line}"
            );
        }
    }
    #[test]
    fn continuous_field_seams_and_cell_rejection_match_unculled_predicate() {
        for seed in [0, 42, 2026, u64::MAX] {
            let a = Lattice::build(seed, [-17, -17], [16, 16], MIN_Y, 192);
            let b = Network::new(seed, [-5, -5], [4, 4]);
            for x in -5..=4 {
                for z in -5..=4 {
                    for y in MIN_Y..192 {
                        let p = [x, y, z];
                        let (v, t) = a.sample(p);
                        assert_eq!((v, t), b.lattice(y).sample(p));
                        let [aa, bb, cc] = v;
                        let w = 22 + (cc + 400).clamp(0, 800) / 25;
                        let exact = cc - aa.abs() / 5 > t || (aa.abs() < w && bb.abs() < w * 5 / 4);
                        assert_eq!(a.contains(p), exact, "{seed} {p:?}");
                    }
                }
            }
        }
    }
}
