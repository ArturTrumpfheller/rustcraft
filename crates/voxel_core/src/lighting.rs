//! Derived bounded skylight. Direct vertical sky is 15, every scattered step
//! costs one; all current solids (including leaves) are opaque. No engine types.
use crate::{BlockPos, ChunkPos, LocalPos, World};
use std::collections::{BTreeSet, VecDeque};
pub const SKY: u8 = 15;
pub const HALO: i32 = SKY as i32;
pub const MIN_Y: i32 = -16;
pub const MAX_Y: i32 = 32;
const HEIGHT: usize = (MAX_Y - MIN_Y) as usize;
/// Exact disposable sample cache, not a hash: equality cannot hide a collision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLight(pub Box<[u8]>);
#[derive(Debug)]
pub struct Skylight {
    min: [i32; 2],
    size: [usize; 2],
    levels: Vec<u8>,
    pub propagated: usize,
}
/// Full-height columns needed by targets, including the face sample and maximum
/// positive lateral contribution (14 steps). Never depends on world area.
pub fn source_columns(targets: &BTreeSet<ChunkPos>) -> BTreeSet<[i32; 2]> {
    let mut out = BTreeSet::new();
    if targets.is_empty() {
        return out;
    }
    let min = [0, 2].map(|a| targets.iter().map(|p| p.0[a]).min().unwrap() - 1);
    let max = [0, 2].map(|a| targets.iter().map(|p| p.0[a]).max().unwrap() + 1);
    for x in min[0]..=max[0] {
        for z in min[1]..=max[1] {
            out.insert([x, z]);
        }
    }
    out
}
pub fn affected(position: BlockPos) -> BTreeSet<ChunkPos> {
    let [x, _, z] = position.0;
    let mut out = BTreeSet::new();
    for cx in (x.saturating_sub(HALO)).div_euclid(16)..=(x.saturating_add(HALO)).div_euclid(16) {
        for cz in (z.saturating_sub(HALO)).div_euclid(16)..=(z.saturating_add(HALO)).div_euclid(16)
        {
            for cy in -1..2 {
                out.insert(ChunkPos([cx, cy, cz]));
            }
        }
    }
    out
}
impl Skylight {
    pub fn for_chunks(world: &World, targets: &BTreeSet<ChunkPos>) -> Self {
        assert!(!targets.is_empty());
        let min = [0, 2].map(|a| targets.iter().map(|p| p.0[a]).min().unwrap() * 16 - HALO);
        let max = [0, 2].map(|a| (targets.iter().map(|p| p.0[a]).max().unwrap() + 1) * 16 + HALO);
        let size = [(max[0] - min[0]) as usize, (max[1] - min[1]) as usize];
        // Callers bound targets to resident radius (maximum 7x7 columns).
        assert!(size[0] <= 7 * 16 + 2 * HALO as usize && size[1] <= 7 * 16 + 2 * HALO as usize);
        let count = size[0] * size[1] * HEIGHT;
        let mut solid = vec![false; count];
        let mut levels = vec![0; count];
        let mut queue = VecDeque::<u32>::new();
        let mut floors = vec![0usize; size[0] * size[1]];
        for z in 0..size[1] {
            for x in 0..size[0] {
                let wx = min[0] + x as i32;
                let wz = min[1] + z as i32;
                let base = (z * size[0] + x) * HEIGHT;
                let mut sky = true;
                for cy in (-1..2).rev() {
                    let cp = ChunkPos([wx.div_euclid(16), cy, wz.div_euclid(16)]);
                    let chunk = world.chunk(cp);
                    let pending = chunk.is_none() && !world.ready(BlockPos([wx, cy * 16, wz]));
                    for ly in (0..16).rev() {
                        let y = cy * 16 + ly as i32;
                        let i = base + (y - MIN_Y) as usize;
                        let opaque = pending
                            || chunk.is_some_and(|c| {
                                c.get(
                                    LocalPos::new([
                                        wx.rem_euclid(16) as usize,
                                        ly,
                                        wz.rem_euclid(16) as usize,
                                    ])
                                    .unwrap(),
                                )
                                .is_solid()
                            });
                        solid[i] = opaque;
                        if sky && opaque {
                            floors[z * size[0] + x] = (y - MIN_Y + 1) as usize;
                        }
                        sky &= !opaque;
                        if sky {
                            levels[i] = SKY;
                        }
                    }
                }
            }
        }
        // Direct sky ends on an opaque cell: only horizontal borders can seed
        // indirect light. Compare column roof heights instead of queueing every
        // sky cell (most of the open volume needs no flood work at all).
        let mut propagated = 0;
        for z in 0..size[1] {
            for x in 0..size[0] {
                let a = z * size[0] + x;
                for (valid, b) in [(x + 1 < size[0], a + 1), (z + 1 < size[1], a + size[0])] {
                    if !valid || floors[a] == floors[b] {
                        continue;
                    }
                    let (lo, hi) = if floors[a] < floors[b] {
                        (a, b)
                    } else {
                        (b, a)
                    };
                    for y in floors[lo]..floors[hi] {
                        let n = hi * HEIGHT + y;
                        if !solid[n] && levels[n] < SKY - 1 {
                            levels[n] = SKY - 1;
                            queue.push_back(n as u32);
                            propagated += 1;
                        }
                    }
                }
            }
        }
        while let Some(i) = queue.pop_front() {
            let i = i as usize;
            let value = levels[i].saturating_sub(1);
            if value == 0 {
                continue;
            }
            let y = i % HEIGHT;
            let column = i / HEIGHT;
            let x = column % size[0];
            let z = column / size[0];
            let neighbors = [
                (y > 0, i.wrapping_sub(1)),
                (y + 1 < HEIGHT, i + 1),
                (x > 0, i.wrapping_sub(HEIGHT)),
                (x + 1 < size[0], i + HEIGHT),
                (z > 0, i.wrapping_sub(HEIGHT * size[0])),
                (z + 1 < size[1], i + HEIGHT * size[0]),
            ];
            for (valid, n) in neighbors {
                if valid && !solid[n] && levels[n] < value {
                    levels[n] = value;
                    queue.push_back(n as u32);
                    propagated += 1;
                }
            }
        }
        Self {
            min,
            size,
            levels,
            propagated,
        }
    }
    pub fn cells(&self) -> usize {
        self.levels.len()
    }
    pub fn level(&self, p: BlockPos) -> u8 {
        let [x, y, z] = p.0;
        if y >= MAX_Y {
            return SKY;
        }
        if y < MIN_Y {
            return 0;
        }
        let x = x - self.min[0];
        let z = z - self.min[1];
        if x < 0 || z < 0 || x as usize >= self.size[0] || z as usize >= self.size[1] {
            return 0;
        }
        self.levels[(z as usize * self.size[0] + x as usize) * HEIGHT + (y - MIN_Y) as usize]
    }
    pub fn chunk(&self, p: ChunkPos) -> ChunkLight {
        let mut b = Vec::with_capacity(18 * 18 * 18);
        for y in -1..=16 {
            for z in -1..=16 {
                for x in -1..=16 {
                    b.push(self.level(BlockPos([
                        p.0[0] * 16 + x,
                        p.0[1] * 16 + y,
                        p.0[2] * 16 + z,
                    ])));
                }
            }
        }
        ChunkLight(b.into_boxed_slice())
    }
}
/// Linear texture modulation with a small cave floor, and fixed retro face
/// orientation. Flat per face, no AO, dynamic lights, shadow maps or smooth leaks.
pub fn face_color(level: u8, normal: [i32; 3]) -> [f32; 4] {
    let brightness = 0.025 + 0.975 * 0.8f32.powi(i32::from(SKY - level.min(SKY)));
    let direction = match normal {
        [0, 1, 0] => 1.,
        [0, -1, 0] => 0.5,
        [1, 0, 0] | [-1, 0, 0] => 0.75,
        _ => 0.85,
    };
    let v = brightness * direction;
    [v, v, v, 1.]
}
#[cfg(test)]
mod tests;
