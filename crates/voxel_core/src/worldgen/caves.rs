// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generator 7 only. Bounded local systems, Q4 swept passages and lobed chambers.
//! No resident neighbors, float noise, global maps or random-stream state.
use super::{hash, noise};
use crate::{Block, BlockPos, Chunk, ChunkPos, LocalPos};

const CELL: i32 = 96;
pub(super) const MIN_CAVE_Y: i32 = -60; // Four uninterrupted Stone foundation layers.
const DIRECTIONS: [[i32; 2]; 8] = [
    [16, 0],
    [11, 11],
    [0, 16],
    [-11, 11],
    [-16, 0],
    [-11, -11],
    [0, -16],
    [11, -11],
];

#[derive(Clone, Debug)]
struct Shape {
    a: [i32; 3], // Q4, centered on voxel centers
    #[cfg(test)]
    b: [i32; 3],
    #[cfg(test)]
    radius: [i32; 3], // Q4; retained for geometry regression inspection
    min: [i32; 3], // conservative inclusive block bounds
    max: [i32; 3],
    entrance: bool,
    v: [i64; 3],
    weights: [i64; 3],
    length: i64,
    limit: i64,
}
impl Shape {
    fn new(a: [i32; 3], b: [i32; 3], radius: [i32; 3], entrance: bool) -> Self {
        let min = std::array::from_fn(|i| (a[i].min(b[i]) - radius[i]).div_euclid(16));
        let max = std::array::from_fn(|i| (a[i].max(b[i]) + radius[i]).div_euclid(16));
        let r = radius.map(i64::from);
        let v = std::array::from_fn(|i| i64::from(b[i] - a[i]));
        let weights = [
            r[1] * r[1] * r[2] * r[2],
            r[0] * r[0] * r[2] * r[2],
            r[0] * r[0] * r[1] * r[1],
        ];
        let length = (0..3).map(|i| v[i] * v[i] * weights[i]).sum();
        let limit = r[0] * r[0] * r[1] * r[1] * r[2] * r[2];
        Self {
            v,
            weights,
            length,
            limit,
            a,
            #[cfg(test)]
            b,
            #[cfg(test)]
            radius,
            min,
            max,
            entrance,
        }
    }
    fn contains(&self, p: [i32; 3]) -> bool {
        if (0..3).any(|i| p[i] < self.min[i] || p[i] > self.max[i]) {
            return false;
        }
        let d: [i64; 3] = std::array::from_fn(|i| i64::from(p[i] * 16 + 8 - self.a[i]));
        // Local Q4 deltas and axes <=432 keep every product below i64::MAX.
        // Absolute coordinates are subtracted before any products.
        let dot = |u: [i64; 3], w: [i64; 3]| -> i64 {
            (0..3).map(|i| u[i] * w[i] * self.weights[i]).sum()
        };
        let len = self.length;
        let t = if len == 0 {
            0
        } else {
            (dot(d, self.v) * 1024 / len).clamp(0, 1024)
        };
        let q = std::array::from_fn(|i| d[i] - self.v[i] * t / 1024);
        dot(q, q) <= self.limit
    }
}

pub(super) struct Plan {
    shapes: Vec<Shape>,
}
struct Builder<F: FnMut(Shape)> {
    emit: F,
    min: [i32; 2],
    max: [i32; 2],
    entrances_only: bool,
}
impl Plan {
    pub(super) fn new(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        let mut shapes = Vec::new();
        Builder {
            entrances_only: false,
            emit: |s| shapes.push(s),
            min,
            max,
        }
        .generate(seed);
        Self { shapes }
    }
}
impl<F: FnMut(Shape)> Builder<F> {
    fn generate(&mut self, seed: u64) {
        // All geometry lies within its cell expanded by <96 blocks.
        for cz in self.min[1].div_euclid(CELL) - 1..=self.max[1].div_euclid(CELL) + 1 {
            for cx in self.min[0].div_euclid(CELL) - 1..=self.max[0].div_euclid(CELL) + 1 {
                self.system(seed, cx, cz);
            }
        }
    }
    fn add(&mut self, a: [i32; 3], b: [i32; 3], r: [i32; 3], entrance: bool) {
        let s = Shape::new(a, b, r, entrance);
        if s.min[0] <= self.max[0]
            && s.max[0] >= self.min[0]
            && s.min[2] <= self.max[1]
            && s.max[2] >= self.min[1]
        {
            (self.emit)(s);
        }
    }
    fn chamber(&mut self, seed: u64, center: [i32; 3], large: bool) {
        let h = hash(seed, center[0], center[2], 0x7021);
        let r = if large {
            [
                18 + (h % 10) as i32,
                11 + ((h >> 8) % 7) as i32,
                16 + ((h >> 16) % 10) as i32,
            ]
        } else {
            [
                7 + (h % 6) as i32,
                5 + ((h >> 8) % 4) as i32,
                7 + ((h >> 16) % 6) as i32,
            ]
        };
        let c = center.map(|v| v * 16 + 8);
        self.add(c, c, r.map(|v| v * 16), false);
        // Off-axis overlapping lobes create ledges, overhangs, uneven floors
        // and non-spherical walls without per-voxel noise evaluation.
        for n in 0..3 {
            let salt = hash(seed, center[0], center[2], 0x7030 + n);
            let offset = [
                ((salt % 17) as i32 - 8) * r[0] / 12,
                (((salt >> 8) % 9) as i32 - 4) * r[1] / 12,
                (((salt >> 16) % 17) as i32 - 8) * r[2] / 12,
            ];
            let lobe = std::array::from_fn(|i| c[i] + offset[i] * 16);
            self.add(lobe, lobe, r.map(|v| v * 11), false);
        }
    }
    fn tube(&mut self, a: [i32; 3], b: [i32; 3], radius: i32, entrance: bool) {
        self.add(a, b, [radius, radius + 12, radius], entrance);
    }
    fn route(&mut self, seed: u64, root: [i32; 3], direction: usize, salt: u64) {
        if root[0] + 76 < self.min[0]
            || root[0] - 76 > self.max[0]
            || root[2] + 76 < self.min[1]
            || root[2] - 76 > self.max[1]
        {
            return;
        }
        let mut prev = root.map(|v| v * 16 + 8);
        let mut heading = direction as i32;
        // Seven bounded ten-block legs, quadratic midpoint smoothing. Each
        // independently salted branch has bends, elevation changes and an end.
        let mut points = [prev; 8];
        let mut radius = [0; 7];
        for leg in 0..7 {
            let h = hash(seed, root[0], root[2], salt + leg);
            heading = (heading + (h % 3) as i32 - 1).rem_euclid(8);
            let dir = DIRECTIONS[heading as usize];
            let next = [
                prev[0] + dir[0] * 10,
                (prev[1] + (((h >> 8) % 9) as i32 - 4) * 16).clamp(-54 * 16, -8 * 16),
                prev[2] + dir[1] * 10,
            ];
            points[leg as usize + 1] = next;
            radius[leg as usize] = 40 + ((h >> 16) % 29) as i32;
            prev = next;
        }
        for i in 1..points.len() - 1 {
            let a = std::array::from_fn(|k| (points[i - 1][k] + points[i][k]) / 2);
            let c: [i32; 3] = std::array::from_fn(|k| (points[i][k] + points[i + 1][k]) / 2);
            let mut last = a;
            for t in 1..=4 {
                let next = std::array::from_fn(|k| {
                    ((4 - t) * (4 - t) * a[k] + 2 * (4 - t) * t * points[i][k] + t * t * c[k]) / 16
                });
                self.tube(last, next, radius[i - 1], false);
                last = next;
            }
        }
        self.tube(
            points[0],
            std::array::from_fn(|k| (points[0][k] + points[1][k]) / 2),
            radius[0],
            false,
        );
        let n = points.len() - 1;
        self.tube(
            std::array::from_fn(|k| (points[n - 1][k] + points[n][k]) / 2),
            points[n],
            radius[n - 1],
            false,
        );
    }
    fn system(&mut self, seed: u64, cx: i32, cz: i32) {
        let h = hash(seed, cx, cz, 0x7001);
        let x = cx * CELL + 24 + (h % 48) as i32;
        let z = cz * CELL + 24 + ((h >> 8) % 48) as i32;
        let density = 760 + noise(seed, x, z, 512, 0x7002) * 350 / 1024;
        if ((h >> 24) % 1000) as i32 >= density {
            return;
        }
        let root = [x, -29 + ((h >> 16) % 13) as i32 - 6, z];
        if !self.entrances_only {
            self.chamber(seed, root, (h >> 40).is_multiple_of(5));
            let branches = 4 + ((h >> 32) % 4) as usize;
            for branch in 0..branches {
                self.route(
                    seed,
                    root,
                    (branch * 8 / branches + (h >> 48) as usize) % 8,
                    0x7100 + branch as u64 * 32,
                );
            }
            // A second, displaced deep room and a curved descending connector.
            let dir = DIRECTIONS[((h >> 52) % 8) as usize];
            let deep = [x + dir[0] * 2, -49 + ((h >> 44) % 7) as i32, z + dir[1] * 2];
            self.chamber(seed ^ 0x7041, deep, false);
            let mid = [
                x + dir[0] - dir[1] / 2,
                (root[1] + deep[1]) / 2,
                z + dir[1] + dir[0] / 2,
            ];
            for (a, b) in [(root, mid), (mid, deep)] {
                self.tube(a.map(|v| v * 16 + 8), b.map(|v| v * 16 + 8), 42, false);
            }
            self.route(seed, deep, ((h >> 56) % 8) as usize, 0x7300);
        }
        // A shallow passage feeds one uncommon gently rising, real entrance.
        // Its mouth is at most 48 blocks from the system root (same 3x3 proof).
        if (h >> 36).is_multiple_of(4) {
            let dir = DIRECTIONS[((h >> 60) % 8) as usize];
            let mouth = [x + dir[0] * 3, z + dir[1] * 3];
            if mouth[0].abs() < 28 && (mouth[1] - 10).abs() < 28 {
                return;
            }
            let mut prev = root.map(|v| v * 16 + 8);
            let top = super::biome::height(seed, mouth[0], mouth[1]) + 2;
            let curvature = noise(seed, x, z, 64, 0x7400);
            for step in 1..=12 {
                let bend = curvature * 3 * step * (12 - step) / 36 / 1024;
                let px = x + (mouth[0] - x) * step / 12 - dir[1] * bend / 16;
                let pz = z + (mouth[1] - z) * step / 12 + dir[0] * bend / 16;
                let y = root[1] + (top - root[1]) * step / 12;
                let next = [px * 16 + 8, y * 16 + 8, pz * 16 + 8];
                self.tube(prev, next, 34, true);
                prev = next;
            }
        }
    }
}
impl Plan {
    fn allowed(p: [i32; 3], height: i32, entrance: bool) -> bool {
        p[1] >= MIN_CAVE_Y
            && p[1] <= height + 5
            && !(p[0].abs() <= 10 && (p[2] - 10).abs() <= 10 && p[1] > height - 18)
            && (entrance || p[1] <= height - 5)
    }
    pub(super) fn contains(&self, p: BlockPos, height: i32) -> bool {
        self.shapes
            .iter()
            .any(|s| Self::allowed(p.0, height, s.entrance) && s.contains(p.0))
    }
    pub(super) fn carve(&self, chunk: &mut Chunk, pos: ChunkPos, height: impl Fn(i32, i32) -> i32) {
        self.carve_if(chunk, pos, height, |_, _| true);
    }
    pub(super) fn carve_if(
        &self,
        chunk: &mut Chunk,
        pos: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        let origin = pos.0.map(|v| v * 16);
        for s in &self.shapes {
            let min: [i32; 3] = std::array::from_fn(|i| s.min[i].max(origin[i]));
            let max: [i32; 3] = std::array::from_fn(|i| s.max[i].min(origin[i] + 15));
            if (0..3).any(|i| min[i] > max[i]) {
                continue;
            }
            for z in min[2]..=max[2] {
                for x in min[0]..=max[0] {
                    let h = height(x, z);
                    for y in min[1].max(MIN_CAVE_Y)..=max[1].min(h) {
                        let local = LocalPos::new([
                            (x - origin[0]) as usize,
                            (y - origin[1]) as usize,
                            (z - origin[2]) as usize,
                        ])
                        .unwrap();
                        if chunk.get(local) != Block::Air
                            && permitted(BlockPos([x, y, z]), h)
                            && Self::allowed([x, y, z], h, s.entrance)
                            && s.contains([x, y, z])
                        {
                            chunk.set(local, Block::Air);
                        }
                    }
                }
            }
        }
        chunk.compact();
    }
}
pub(super) fn point(seed: u64, p: BlockPos, height: i32) -> bool {
    if p.0[1] < MIN_CAVE_Y || p.0[1] > height {
        return false;
    }
    let mut carved = false;
    Builder {
        entrances_only: p.0[1] > height - 5,
        emit: |s: Shape| {
            carved |= Plan::allowed(p.0, height, s.entrance) && s.contains(p.0);
        },
        min: [p.0[0], p.0[2]],
        max: [p.0[0], p.0[2]],
    }
    .generate(seed);
    carved
}

/// The existing deep chamber anchor, for v8's separate closed-basin validation.
/// This metadata does not alter v7 geometry or admission.
pub(super) fn pool_center(seed: u64, [cx, cz]: [i32; 2]) -> Option<[i32; 3]> {
    let h = hash(seed, cx, cz, 0x7001);
    if !(h >> 20).is_multiple_of(3) {
        return None;
    }
    let x = cx * CELL + 24 + (h % 48) as i32;
    let z = cz * CELL + 24 + ((h >> 8) % 48) as i32;
    let density = 760 + noise(seed, x, z, 512, 0x7002) * 350 / 1024;
    if ((h >> 24) % 1000) as i32 >= density {
        return None;
    }
    let dir = DIRECTIONS[((h >> 52) % 8) as usize];
    Some([x + dir[0] * 2, -49 + ((h >> 44) % 7) as i32, z + dir[1] * 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_features_fit_the_proven_discovery_neighborhood() {
        for seed in [0, 42, 2026, u64::MAX] {
            let mut large = 0;
            let mut entrances = 0;
            for cz in -3..=3 {
                for cx in -3..=3 {
                    Builder {
                        entrances_only: false,
                        emit: |s: Shape| {
                            assert!(s.min[0] >= cx * CELL - CELL && s.max[0] < (cx + 2) * CELL);
                            assert!(s.min[2] >= cz * CELL - CELL && s.max[2] < (cz + 2) * CELL);
                            large += usize::from(s.radius[0] >= 18 * 16 && s.a == s.b);
                            entrances += usize::from(s.entrance);
                        },
                        min: [-1000; 2],
                        max: [1000; 2],
                    }
                    .system(seed, cx, cz);
                }
            }
            assert!(large > 0 && entrances > 0);
        }
    }
    #[test]
    fn distinct_chambers_curves_branch_roots_and_horizontal_vertical_seams_exist() {
        use std::collections::{BTreeMap, BTreeSet};
        for seed in [0, 42, 2026, u64::MAX] {
            let mut rooms = BTreeSet::new();
            let mut roots = BTreeMap::<_, usize>::new();
            let (mut medium, mut large, mut slopes, mut seams) = (0, 0, 0, 0);
            Builder {
                entrances_only: false,
                emit: |s: Shape| {
                    if s.a == s.b {
                        rooms.insert(s.a);
                        medium += usize::from((7 * 16..18 * 16).contains(&s.radius[0]));
                        large += usize::from(
                            s.radius[0] >= 18 * 16
                                && s.max[1].div_euclid(16) - s.min[1].div_euclid(16) >= 1
                                && s.max[0] - s.min[0] >= 36,
                        );
                    } else {
                        *roots.entry(s.a).or_default() += 1;
                        slopes +=
                            usize::from(s.a[1] != s.b[1] && s.a[0] != s.b[0] && s.a[2] != s.b[2]);
                        seams += usize::from(
                            (0..3).any(|i| s.a[i].div_euclid(256) != s.b[i].div_euclid(256)),
                        );
                    }
                },
                min: [-256; 2],
                max: [256; 2],
            }
            .generate(seed);
            assert!(medium > 0 && large > 0 && slopes > 10 && seams > 10);
            assert!(
                roots
                    .iter()
                    .any(|(p, count)| *count >= 4 && rooms.contains(p)),
                "branches connect inside chambers"
            );
        }
    }
}
