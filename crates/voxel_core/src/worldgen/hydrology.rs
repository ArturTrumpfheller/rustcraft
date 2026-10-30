// SPDX-License-Identifier: MIT OR Apache-2.0
//! V8 only: bounded, coordinate-based surface basins and verified cave pools.
use super::{Generator, biome, caves, hash, noise};
use crate::{Block, BlockPos, Chunk, ChunkPos};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

/// Top occupied source-cell Y. Its visible surface is Y + 8/9, per Fluid::SOURCE.
pub const SEA_LEVEL: i32 = 2;
const POOL_LEVEL: i32 = -52;
const MAX_POOLS: usize = 64;
const MAX_POOL_CELLS: usize = 8192;
type Pool = Arc<Vec<BlockPos>>;
#[derive(Default)]
pub(super) struct PoolCache(Mutex<CachedPools>);
#[derive(Default)]
struct CachedPools {
    entries: BTreeMap<[i32; 2], Pool>,
    order: VecDeque<[i32; 2]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterBody {
    Dry,
    Pond,
    Lake,
    River,
    Ocean,
}
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub bed: i32,
    pub body: WaterBody,
    /// Ocean shelf and its dry beach margin; biome materials still decide sand.
    pub coastal: bool,
}
impl Surface {
    pub fn depth(self) -> i32 {
        (SEA_LEVEL - self.bed).max(0)
    }
}
#[derive(Clone, Copy)]
struct Basin {
    center: [i32; 2],
    radii: [i32; 2],
    depth: i32,
}
#[derive(Clone, Copy)]
struct Segment {
    a: [i32; 2],
    b: [i32; 2],
    width: i32,
    depth: i32,
}
struct Geography {
    basins: Vec<Basin>,
    rivers: Vec<Segment>,
}
fn overlaps(min: [i32; 2], max: [i32; 2], a: [i32; 2], b: [i32; 2], r: i32) -> bool {
    (0..2).all(|i| a[i].min(b[i]) - r <= max[i] && a[i].max(b[i]) + r >= min[i])
}
impl Geography {
    fn new(seed: u64, min: [i32; 2], max: [i32; 2]) -> Self {
        let mut result = Self {
            basins: Vec::new(),
            rivers: Vec::new(),
        };
        for cz in min[1].div_euclid(128) - 1..=max[1].div_euclid(128) + 1 {
            for cx in min[0].div_euclid(128) - 1..=max[0].div_euclid(128) + 1 {
                let h = hash(seed, cx, cz, 0x8001);
                let center = [
                    cx * 128 + 32 + (h % 64) as i32,
                    cz * 128 + 32 + ((h >> 8) % 64) as i32,
                ];
                let c = biome::sample_climate(
                    seed,
                    center[0].clamp(-32768, 32767),
                    center[1].clamp(-32768, 32767),
                )
                .unwrap();
                let chance = if c.geography > 240 {
                    90
                } else if c.moisture < -100 && c.temperature > 180 {
                    140
                } else {
                    480
                };
                if (h >> 24) % 1000 >= chance {
                    continue;
                }
                let radius = match (h >> 32) % 10 {
                    0 => 34,
                    1..=4 => 15,
                    _ => 6,
                } + ((h >> 40) % 6) as i32;
                let basin = Basin {
                    center,
                    radii: [radius, radius * (70 + ((h >> 48) % 60) as i32) / 100],
                    depth: 2 + radius / 5,
                };
                result.add_basin(basin, min, max);
            }
        }
        // Finite gently curved channels terminate in real endpoint basins. The
        // common still-water datum never asks this first hydrology model to flow uphill.
        for cz in min[1].div_euclid(384) - 1..=max[1].div_euclid(384) + 1 {
            for cx in min[0].div_euclid(384) - 1..=max[0].div_euclid(384) + 1 {
                let h = hash(seed, cx, cz, 0x8100);
                if h.is_multiple_of(3) {
                    continue;
                }
                let flip = (h >> 8) & 1 != 0;
                let origin = [cx * 384, cz * 384];
                let transform = |x, z| {
                    if flip {
                        [origin[0] + z, origin[1] + x]
                    } else {
                        [origin[0] + x, origin[1] + z]
                    }
                };
                let a = transform(52, 100 + ((h >> 16) % 130) as i32);
                let b = transform(332, 100 + ((h >> 24) % 130) as i32);
                let controls = [
                    a,
                    transform(140, 64 + ((h >> 32) % 256) as i32),
                    transform(244, 64 + ((h >> 40) % 256) as i32),
                    b,
                ];
                // Reject steep routes as a whole; no channel clipping into disconnected patches.
                if controls
                    .iter()
                    .any(|p| biome::height(seed, p[0], p[1]) > 12)
                    && !(h >> 56).is_multiple_of(8)
                {
                    continue;
                }
                result.add_basin(
                    Basin {
                        center: a,
                        radii: [12, 10],
                        depth: 4,
                    },
                    min,
                    max,
                );
                result.add_basin(
                    Basin {
                        center: b,
                        radii: [30, 24],
                        depth: 8,
                    },
                    min,
                    max,
                );
                let at = |t: i32| {
                    std::array::from_fn(|k| {
                        ((24 - t).pow(3) * controls[0][k]
                            + 3 * (24 - t).pow(2) * t * controls[1][k]
                            + 3 * (24 - t) * t * t * controls[2][k]
                            + t.pow(3) * controls[3][k])
                            / 24_i32.pow(3)
                    })
                };
                let mut prev = a;
                for t in 1..=24 {
                    let next = at(t);
                    let width = 3 + (noise(seed, next[0], next[1], 64, 0x8101) + 1024) * 3 / 2048;
                    if overlaps(min, max, prev, next, width + 8) {
                        result.rivers.push(Segment {
                            a: prev,
                            b: next,
                            width,
                            depth: 3 + width / 2,
                        });
                    }
                    prev = next;
                }
            }
        }
        result
    }
    fn add_basin(&mut self, b: Basin, min: [i32; 2], max: [i32; 2]) {
        if overlaps(
            min,
            max,
            b.center,
            b.center,
            (b.radii[0].max(b.radii[1]) * 8 + 4) / 5 + 4,
        ) {
            self.basins.push(b);
        }
    }
    fn column(&self, seed: u64, x: i32, z: i32, base: i32) -> Surface {
        let wx = x + noise(seed, x, z, 256, 0x8201) * 44 / 1024;
        let wz = z + noise(seed, x, z, 256, 0x8202) * 44 / 1024;
        let continental = noise(seed, wx, wz, 768, 0x8203) + noise(seed, x, z, 96, 0x8204) / 6;
        let lowering = (-continental - 80).max(0);
        let mut bed = (base.max(SEA_LEVEL + 1) - lowering * lowering / 6000).max(-28);
        let coastal = continental < -80 && bed <= SEA_LEVEL + 2;
        let mut body = if bed < SEA_LEVEL {
            WaterBody::Ocean
        } else {
            WaterBody::Dry
        };
        for b in &self.basins {
            let dx = x - b.center[0] + noise(seed, x, z, 24, 0x8301) * 3 / 1024;
            let dz = z - b.center[1] + noise(seed, x, z, 28, 0x8302) * 3 / 1024;
            let d = dx * dx * 1024 / (b.radii[0] * b.radii[0])
                + dz * dz * 1024 / (b.radii[1] * b.radii[1]);
            if d < 2560 {
                let target = if d <= 1024 {
                    SEA_LEVEL - b.depth + (b.depth + 3) * d / 1024
                } else {
                    SEA_LEVEL + 3 + (base.max(SEA_LEVEL + 1) - SEA_LEVEL - 3) * (d - 1024) / 1536
                };
                if target < bed {
                    bed = target;
                    if bed < SEA_LEVEL && body != WaterBody::Ocean {
                        body = if b.radii[0] < 14 {
                            WaterBody::Pond
                        } else {
                            WaterBody::Lake
                        };
                    }
                }
            }
        }
        for s in &self.rivers {
            let v = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
            let d = [x - s.a[0], z - s.a[1]];
            let len = v[0] * v[0] + v[1] * v[1];
            let t = ((d[0] * v[0] + d[1] * v[1]) * 1024 / len.max(1)).clamp(0, 1024);
            let q = [d[0] - v[0] * t / 1024, d[1] - v[1] * t / 1024];
            let distance = q[0] * q[0] + q[1] * q[1];
            if distance < (s.width + 7).pow(2) {
                let target = if distance <= s.width * s.width {
                    SEA_LEVEL - s.depth + (s.depth + 3) * distance / (s.width * s.width)
                } else {
                    SEA_LEVEL
                        + 3
                        + (base.max(SEA_LEVEL + 1) - SEA_LEVEL - 3) * (distance - s.width * s.width)
                            / ((s.width + 7).pow(2) - s.width * s.width)
                };
                if target < bed {
                    bed = target;
                    if bed < SEA_LEVEL && body != WaterBody::Ocean {
                        body = WaterBody::River;
                    }
                }
            }
        }
        // Retain a supported starting shore even when the origin lies offshore.
        let spawn_distance = x.abs().max((z - 10).abs());
        if spawn_distance < 40 {
            let weight = (spawn_distance - 12).clamp(0, 28);
            bed = base.max(SEA_LEVEL + 1) + (bed - base.max(SEA_LEVEL + 1)) * weight / 28;
        }
        Surface {
            bed,
            coastal,
            body: if bed < SEA_LEVEL {
                body
            } else {
                WaterBody::Dry
            },
        }
    }
}
pub fn surface(seed: u64, x: i32, z: i32) -> Surface {
    Geography::new(seed, [x, z], [x, z]).column(seed, x, z, biome::height(seed, x, z))
}

pub(super) struct Plan {
    pub columns: Vec<Surface>,
    min: [i32; 2],
    pools: Vec<Pool>,
    sealed: Vec<bool>,
}
impl Plan {
    pub(super) fn new(
        g: &Generator,
        min: [i32; 2],
        max: [i32; 2],
        materials: &[biome::Column],
    ) -> Self {
        // One additional ring proves whether each 18x18 sample touches water.
        // Dry cave entrances outside that immediate bank retain v7 behavior.
        let geography = Geography::new(g.config.seed, min.map(|v| v - 1), max.map(|v| v + 1));
        let mut grid = Vec::with_capacity(400);
        for z in -1..=18 {
            for x in -1..=18 {
                let base = if (0..18).contains(&x) && (0..18).contains(&z) {
                    materials[(z * 18 + x) as usize].height
                } else {
                    biome::height(g.config.seed, min[0] + x, min[1] + z)
                };
                grid.push(geography.column(g.config.seed, min[0] + x, min[1] + z, base));
            }
        }
        let mut columns = Vec::with_capacity(324);
        let mut sealed = Vec::with_capacity(324);
        for z in 1..19 {
            for x in 1..19 {
                let i = z * 20 + x;
                columns.push(grid[i]);
                sealed.push(
                    [i, i - 1, i + 1, i - 20, i + 20]
                        .iter()
                        .any(|j| grid[*j].depth() > 0),
                );
            }
        }
        let mut pools = Vec::new();
        for cz in (min[1] - 48).div_euclid(96)..=(max[1] + 48).div_euclid(96) {
            for cx in (min[0] - 48).div_euclid(96)..=(max[0] + 48).div_euclid(96) {
                let pool = g.pool([cx, cz]);
                if !pool.is_empty() {
                    pools.push(pool);
                }
            }
        }
        Self {
            columns,
            min,
            pools,
            sealed,
        }
    }
    pub(super) fn fill_pools(&self, c: &mut Chunk, cp: ChunkPos) {
        for pool in &self.pools {
            for &p in pool.iter() {
                let (chunk, local) = p.split();
                if chunk == cp && c.get(local) == Block::Air {
                    c.set(local, Block::WaterSource);
                }
            }
        }
    }
    pub(super) fn cave_water(&self, p: BlockPos) -> bool {
        self.pools.iter().any(|pool| pool.binary_search(&p).is_ok())
    }
    pub(super) fn sealed(&self, x: i32, z: i32) -> bool {
        self.sealed[((z - self.min[1]) * 18 + x - self.min[0]) as usize]
    }
    pub(super) fn column(&self, x: i32, z: i32) -> Surface {
        self.columns[((z - self.min[1]) * 18 + x - self.min[0]) as usize]
    }
}
/// Read-only diagnostic of one bounded cave-water ownership cell.
#[derive(Clone, Debug)]
pub struct PoolSummary {
    pub cells: usize,
    pub surface_cells: usize,
    pub sample: BlockPos,
    pub min: [i32; 3],
    pub max: [i32; 3],
}
impl Generator {
    pub fn natural_pool_summary(&self, cell: [i32; 2]) -> Option<PoolSummary> {
        if self.config.version != 8 || cell.iter().any(|v| !(-341..341).contains(v)) {
            return None;
        }
        let pool = self.pool(cell);
        let &sample = pool.first()?;
        Some(PoolSummary {
            cells: pool.len(),
            surface_cells: pool.iter().filter(|p| p.0[1] == POOL_LEVEL).count(),
            sample,
            min: std::array::from_fn(|i| pool.iter().map(|p| p.0[i]).min().unwrap()),
            max: std::array::from_fn(|i| pool.iter().map(|p| p.0[i]).max().unwrap()),
        })
    }
    fn pool(&self, cell: [i32; 2]) -> Pool {
        if let Some(p) = self.water_cache.0.lock().unwrap().entries.get(&cell) {
            return Arc::clone(p);
        }
        let pool = Arc::new(cave_pool(self.config.seed, cell));
        let mut cache = self.water_cache.0.lock().unwrap();
        if let Some(p) = cache.entries.get(&cell) {
            return Arc::clone(p);
        }
        if cache.entries.len() >= MAX_POOLS {
            let key = cache.order.pop_front().unwrap();
            cache.entries.remove(&key);
        }
        cache.order.push_back(cell);
        cache.entries.insert(cell, Arc::clone(&pool));
        pool
    }
    pub fn water_planning_bytes(&self) -> usize {
        self.water_cache
            .0
            .lock()
            .unwrap()
            .entries
            .values()
            .map(|p| p.capacity() * std::mem::size_of::<BlockPos>())
            .sum()
    }
}
fn cave_pool(seed: u64, cell: [i32; 2]) -> Vec<BlockPos> {
    let Some(center) = caves::pool_center(seed, cell) else {
        return Vec::new();
    };
    let min = [center[0] - 40, center[2] - 40];
    let max = [center[0] + 40, center[2] + 40];
    let plan = caves::Plan::new(seed, min, max);
    let wet = |p: BlockPos| plan.contains(p, biome::height(seed, p.0[0], p.0[2]));
    let Some(start) = (-59..=POOL_LEVEL)
        .map(|y| BlockPos([center[0], y, center[2]]))
        .find(|p| wet(*p))
    else {
        return Vec::new();
    };
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        if seen.len() > MAX_POOL_CELLS {
            return Vec::new();
        }
        if p.0[0] <= min[0] || p.0[0] >= max[0] || p.0[2] <= min[1] || p.0[2] >= max[1] {
            return Vec::new();
        }
        for d in crate::fluid::NEIGHBORS {
            let n = p.offset(d).unwrap();
            if n.0[1] > POOL_LEVEL || seen.contains(&n) {
                continue;
            }
            if wet(n) {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    // Reject almost-empty puddles and require real air above the surface. All
    // lower side/floor neighbors were proved closed by the bounded flood.
    if seen.len() < 12
        || !seen
            .iter()
            .any(|p| p.0[1] == POOL_LEVEL && wet(p.offset([0, 1, 0]).unwrap()))
    {
        return Vec::new();
    }
    seen.into_iter().collect()
}

/// Read-only 16x16 surface survey. Runtime generation uses the same evaluator
/// with its 18x18 halo and existing cached biome material columns.
pub fn surface_tile(seed: u64, column: [i32; 2]) -> Vec<Surface> {
    let min = column.map(|v| v * 16);
    let geo = Geography::new(seed, min, min.map(|v| v + 15));
    (0..256)
        .map(|i| {
            let x = min[0] + i % 16;
            let z = min[1] + i / 16;
            geo.column(seed, x, z, biome::height(seed, x, z))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_river_centerlines_cross_chunks_without_dry_gaps() {
        let seed = 2026;
        let geo = Geography::new(seed, [0, -384], [384, 0]);
        assert!(geo.rivers.len() >= 24);
        let mut crossed = false;
        for segment in &geo.rivers {
            for t in 0..=16 {
                let p = std::array::from_fn::<_, 2, _>(|i| {
                    segment.a[i] + (segment.b[i] - segment.a[i]) * t / 16
                });
                let s = geo.column(seed, p[0], p[1], biome::height(seed, p[0], p[1]));
                assert!(s.depth() > 0, "dry river center at {p:?}");
            }
            crossed |= (0..2).any(|i| segment.a[i].div_euclid(16) != segment.b[i].div_euclid(16));
        }
        assert!(crossed);
    }
    #[test]
    fn closed_pools_fit_discovery_and_only_fill_real_v7_cave_space() {
        let mut pools = 0;
        for seed in [0, 42, 2026, u64::MAX] {
            let g = Generator::new(super::super::GenerationConfig { seed, version: 8 }).unwrap();
            for cz in -3..=3 {
                for cx in -3..=3 {
                    let pool = g.pool([cx, cz]);
                    if pool.is_empty() {
                        continue;
                    }
                    pools += 1;
                    assert!(pool.len() <= MAX_POOL_CELLS);
                    let cells: BTreeSet<_> = pool.iter().copied().collect();
                    let mut samplers = BTreeMap::new();
                    for &p in pool.iter() {
                        assert!(p.0[0] >= cx * 96 - 48 && p.0[0] < (cx + 1) * 96 + 48);
                        assert!(p.0[2] >= cz * 96 - 48 && p.0[2] < (cz + 1) * 96 + 48);
                        assert!(p.0[1] <= POOL_LEVEL && p.0[1] >= caves::MIN_CAVE_Y);
                        assert!(caves::point(seed, p, biome::height(seed, p.0[0], p.0[2])));
                        for d in crate::fluid::NEIGHBORS {
                            let n = p.offset(d).unwrap();
                            if n.0[1] <= POOL_LEVEL && !cells.contains(&n) {
                                assert!(!caves::point(
                                    seed,
                                    n,
                                    biome::height(seed, n.0[0], n.0[2])
                                ));
                            }
                        }
                        let c = [p.0[0].div_euclid(16), p.0[2].div_euclid(16)];
                        let s = samplers.entry(c).or_insert_with(|| g.sampler(c));
                        assert_eq!(s.block(p), Block::WaterSource);
                    }
                }
            }
        }
        assert!(pools >= 4);
    }
}
