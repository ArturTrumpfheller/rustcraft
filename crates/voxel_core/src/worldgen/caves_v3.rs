// SPDX-License-Identifier: MIT OR Apache-2.0
//! V14: signed multi-octave cave density and bounded branching random walks.
//! Historical v1–13 are dispatched unchanged. All classifications use global
//! coordinates; lattice samples are quantized before integer interpolation.
use super::{Generator, caves, hash};
use crate::{Block, BlockPos, Chunk, ChunkPos, LocalPos};
mod carvers;
mod density;
mod horizon;
pub(super) type NoiseCache = std::sync::OnceLock<density::Cache>;
const MIN_Y: i32 = -60;
const STEP: i32 = 4;
/// The historical implementation is intentionally kept behind an explicit gate.
pub(super) enum Versioned {
    Historical(caves::Plan),
    V14(Plan),
}
impl Versioned {
    pub(super) fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        if g.config.version == 14 {
            Self::V14(Plan::new(g, min, max))
        } else {
            Self::Historical(caves::Plan::new(g.config.seed, min, max))
        }
    }
    pub(super) fn contains(&self, p: BlockPos, height: i32) -> bool {
        match self {
            Self::Historical(c) => c.contains(p, height),
            Self::V14(c) => c.contains(p, height),
        }
    }
    pub(super) fn carve(&self, c: &mut Chunk, p: ChunkPos, height: impl Fn(i32, i32) -> i32) {
        match self {
            Self::Historical(plan) => plan.carve(c, p, height),
            Self::V14(plan) => plan.carve_if(c, p, height, |_, _| true),
        }
    }
    pub(super) fn carve_if(
        &self,
        c: &mut Chunk,
        p: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        match self {
            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),
            Self::V14(plan) => plan.carve_if(c, p, height, permitted),
        }
    }
}

fn mix(a: i32, b: i32, t: i32, q: i32) -> i32 {
    a + ((i64::from(b - a) * i64::from(t)) / i64::from(q)) as i32
}
pub(super) struct Plan {
    shapes: Vec<carvers::Ellipsoid>,
    network: density::Network,
}
impl Plan {
    pub(super) fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        Self {
            shapes: carvers::build(g.config.seed, min, max),
            network: density::Network::new(g, min, max),
        }
    }
    fn allowed(p: [i32; 3], h: i32, entrance: bool) -> bool {
        p[1] >= MIN_Y
            && p[1] <= h
            && p[1] < 192
            && !(p[0].abs() <= 12 && (p[2] - 10).abs() <= 12 && p[1] > h - 20)
            && (entrance || p[1] <= h - 8)
    }
    pub(super) fn contains(&self, p: BlockPos, h: i32) -> bool {
        Self::allowed(p.0, h, true)
            && (self.shapes.iter().any(|s| s.contains(p.0)) || self.network.contains(p.0))
    }
    fn carve_if(
        &self,
        c: &mut Chunk,
        cp: ChunkPos,
        height: impl Fn(i32, i32) -> i32,
        permitted: impl Fn(BlockPos, i32) -> bool,
    ) {
        let o = cp.0.map(|v| v * 16);
        if o[1] + 15 < MIN_Y || o[1] >= 192 {
            return;
        }
        for s in &self.shapes {
            let min: [i32; 3] = std::array::from_fn(|i| s.min[i].max(o[i]));
            let max: [i32; 3] = std::array::from_fn(|i| s.max[i].min(o[i] + 15));
            if (0..3).any(|i| min[i] > max[i]) {
                continue;
            }
            for z in min[2]..=max[2] {
                for x in min[0]..=max[0] {
                    let h = height(x, z);
                    for y in min[1].max(MIN_Y)..=max[1].min(h) {
                        let p = BlockPos([x, y, z]);
                        let l = LocalPos::new([
                            (x - o[0]) as usize,
                            (y - o[1]) as usize,
                            (z - o[2]) as usize,
                        ])
                        .unwrap();
                        if c.get(l) != Block::Air
                            && Self::allowed(p.0, h, true)
                            && s.contains(p.0)
                            && permitted(p, h)
                        {
                            c.set(l, Block::Air);
                        }
                    }
                }
            }
        }
        self.network.carve_if(c, cp, height, permitted);
        c.compact();
    }
}
/// Read-only candidate centers of traditional carver ellipsoids. These are
/// proposed geometry, not guaranteed final air or a player-clearance assertion.
pub fn landmarks(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Vec<(&'static str, [i32; 3])> {
    if g.config.version != 14
        || min
            .iter()
            .chain(max.iter())
            .any(|v| !(-32000..32000).contains(v))
        || (0..2).any(|i| max[i] < min[i] || max[i] - min[i] > 512)
    {
        return vec![];
    }
    carvers::build(g.config.seed, min, max)
        .iter()
        .filter_map(|s| {
            let p = s.center.map(|v| v.div_euclid(256));
            ((min[0]..=max[0]).contains(&p[0]) && (min[1]..=max[1]).contains(&p[2]))
                .then_some(("carver", p))
        })
        .collect()
}
