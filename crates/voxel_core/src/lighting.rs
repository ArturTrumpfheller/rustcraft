//! Derived bounded sky and artificial light; independent integer channels.
//! Direct sky is 15; torch emission is 14. Six-connected scattering costs one.
use crate::{BlockPos, ChunkPos, LocalPos, World};
use std::collections::{BTreeSet, VecDeque};
pub const SKY: u8 = 15;
pub const HALO: i32 = SKY as i32;
/// Historical fixture limits. Runtime calculations use World::world_height.
pub const MIN_Y: i32 = crate::bounds::WorldHeight::HISTORICAL.min;
pub const MAX_Y: i32 = crate::bounds::WorldHeight::HISTORICAL.max;

/// Exact disposable cache: low nibble skylight, high nibble artificial light.
/// Equality cannot hide a hash collision or suppress an independent channel change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLight(pub Box<[u8]>);
#[derive(Debug)]
pub struct Skylight {
    min: [i32; 2],
    size: [usize; 2],
    min_y: i32,
    max_y: i32,
    world_height: crate::bounds::WorldHeight,
    levels: Vec<u8>,
    block_levels: Vec<u8>,
    pub propagated: usize,
    pub block_propagated: usize,
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
    affected_in(position, crate::bounds::WorldHeight::HISTORICAL)
}
pub fn affected_in(position: BlockPos, height: crate::bounds::WorldHeight) -> BTreeSet<ChunkPos> {
    let [x, _, z] = position.0;
    let mut out = BTreeSet::new();
    for cx in (x.saturating_sub(HALO)).div_euclid(16)..=(x.saturating_add(HALO)).div_euclid(16) {
        for cz in (z.saturating_sub(HALO)).div_euclid(16)..=(z.saturating_add(HALO)).div_euclid(16)
        {
            for cy in height.chunks() {
                out.insert(ChunkPos([cx, cy, cz]));
            }
        }
    }
    out
}
impl Skylight {
    pub fn for_chunks(world: &World, targets: &BTreeSet<ChunkPos>) -> Self {
        Self::calculate(world, targets, active_span(world, targets))
    }
    #[cfg(test)]
    pub(crate) fn full_reference(world: &World, targets: &BTreeSet<ChunkPos>) -> Self {
        let h = world.world_height();
        reference::calculate(world, targets, (h.min, h.max))
    }
    fn calculate(world: &World, targets: &BTreeSet<ChunkPos>, (min_y, max_y): (i32, i32)) -> Self {
        assert!(!targets.is_empty());
        let min = [0, 2].map(|a| targets.iter().map(|p| p.0[a]).min().unwrap() * 16 - HALO);
        let max = [0, 2].map(|a| (targets.iter().map(|p| p.0[a]).max().unwrap() + 1) * 16 + HALO);
        let size = [(max[0] - min[0]) as usize, (max[1] - min[1]) as usize];
        // Callers bound targets to resident radius (maximum 7x7 columns).
        assert!(size[0] <= 7 * 16 + 2 * HALO as usize && size[1] <= 7 * 16 + 2 * HALO as usize);
        let world_height = world.world_height();
        let height = (max_y - min_y) as usize;
        let count = size[0] * size[1] * height;
        let mut solid = vec![false; count];
        let mut levels = vec![0; count];
        let mut queue = VecDeque::<u32>::new();
        let mut sources = Vec::new();
        let mut floors = vec![0usize; size[0] * size[1]];
        // Resolve each vertical descriptor stack once, instead of looking up
        // the same BTreeMap entry for every X/Z cell. The supported world is at
        // most sixteen slabs tall; these borrowed descriptors live on the stack.
        let first_y = min_y.div_euclid(16);
        let end_y = (max_y + 15).div_euclid(16);
        assert!(end_y - first_y <= 16);
        for cz in min[1].div_euclid(16)..=(max[1] - 1).div_euclid(16) {
            for cx in min[0].div_euclid(16)..=(max[0] - 1).div_euclid(16) {
                let slabs: [_; 16] = std::array::from_fn(|i| {
                    let cy = first_y + i as i32;
                    let cp = ChunkPos([cx, cy, cz]);
                    let chunk = (cy < end_y).then(|| world.chunk(cp)).flatten();
                    let pending = cy < end_y
                        && chunk.is_none()
                        && !world.ready(BlockPos([cx * 16, cy * 16, cz * 16]));
                    (chunk, pending)
                });
                for wz in min[1].max(cz * 16)..max[1].min((cz + 1) * 16) {
                    for wx in min[0].max(cx * 16)..max[0].min((cx + 1) * 16) {
                        let column = (wz - min[1]) as usize * size[0] + (wx - min[0]) as usize;
                        let base = column * height;
                        let mut sky = true;
                        for cy in (first_y..end_y).rev() {
                            let (chunk, pending) = slabs[(cy - first_y) as usize];
                            let low = (cy * 16).max(min_y);
                            let high = ((cy + 1) * 16).min(max_y);
                            let range =
                                base + (low - min_y) as usize..base + (high - min_y) as usize;
                            let uniform = if pending {
                                Some(crate::Block::Stone)
                            } else {
                                chunk.map_or(Some(crate::Block::Air), crate::Chunk::uniform)
                            };
                            if let Some(block) = uniform.filter(|b| b.emission() == 0) {
                                let opaque = block.is_opaque();
                                solid[range.clone()].fill(opaque);
                                if sky && opaque {
                                    floors[column] = (high - min_y) as usize;
                                    sky = false;
                                }
                                if sky {
                                    levels[range].fill(SKY);
                                }
                                continue;
                            }
                            for y in (low..high).rev() {
                                let i = base + (y - min_y) as usize;
                                let block = chunk.unwrap().get(
                                    LocalPos::new([
                                        wx.rem_euclid(16) as usize,
                                        y.rem_euclid(16) as usize,
                                        wz.rem_euclid(16) as usize,
                                    ])
                                    .unwrap(),
                                );
                                let opaque = block.is_opaque();
                                if block.emission() > 0 {
                                    sources.push((i as u32, block.emission()));
                                }
                                solid[i] = opaque;
                                if sky && opaque {
                                    floors[column] = (y - min_y + 1) as usize;
                                }
                                sky &= !opaque;
                                if sky {
                                    levels[i] = SKY;
                                }
                            }
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
                        let n = hi * height + y;
                        if !solid[n] && levels[n] < SKY - 1 {
                            levels[n] = SKY - 1;
                            queue.push_back(n as u32);
                            propagated += 1;
                        }
                    }
                }
            }
        }
        propagated += flood(&solid, &mut levels, &mut queue, size, height);
        // Avoid the second volume allocation and flood entirely without sources.
        // Reuse the drained skylight queue for the independent artificial channel.
        let mut block_levels = if sources.is_empty() {
            Vec::new()
        } else {
            vec![0; count]
        };
        for (i, emission) in sources {
            block_levels[i as usize] = emission;
            queue.push_back(i);
        }
        let block_propagated = flood(&solid, &mut block_levels, &mut queue, size, height);
        Self {
            min,
            size,
            min_y,
            max_y,
            world_height,
            levels,
            propagated,
            block_levels,
            block_propagated,
        }
    }
    pub fn cells(&self) -> usize {
        self.levels.len()
    }
    pub fn level(&self, p: BlockPos) -> u8 {
        let [x, y, z] = p.0;
        if y >= self.world_height.max {
            return SKY;
        }
        if y < self.world_height.min {
            return 0;
        }
        let x = x - self.min[0];
        let z = z - self.min[1];
        if x < 0 || z < 0 || x as usize >= self.size[0] || z as usize >= self.size[1] {
            return 0;
        }
        // Omitted bottom planes are opaque everywhere; omitted upper planes
        // are proven open sky. Neither inference is made from pending chunks.
        if y >= self.max_y {
            return SKY;
        }
        if y < self.min_y {
            return 0;
        }
        self.levels[(z as usize * self.size[0] + x as usize) * (self.max_y - self.min_y) as usize
            + (y - self.min_y) as usize]
    }
    /// Artificial light has no implicit sky/outside-world sources.
    pub fn block_level(&self, p: BlockPos) -> u8 {
        let [x, y, z] = p.0;
        let x = x - self.min[0];
        let z = z - self.min[1];
        if self.block_levels.is_empty()
            || !(self.min_y..self.max_y).contains(&y)
            || x < 0
            || z < 0
            || x as usize >= self.size[0]
            || z as usize >= self.size[1]
        {
            return 0;
        }
        self.block_levels[(z as usize * self.size[0] + x as usize)
            * (self.max_y - self.min_y) as usize
            + (y - self.min_y) as usize]
    }
    pub fn combined_level(&self, p: BlockPos) -> u8 {
        self.level(p).max(self.block_level(p))
    }
    /// Logical resident channel payload, excluding allocation/queue overhead.
    pub fn bytes(&self) -> usize {
        self.levels.len() + self.block_levels.len()
    }
    pub fn chunk(&self, p: ChunkPos) -> ChunkLight {
        let mut b = Vec::with_capacity(18 * 18 * 18);
        for y in -1..=16 {
            for z in -1..=16 {
                for x in -1..=16 {
                    let sample = BlockPos([p.0[0] * 16 + x, p.0[1] * 16 + y, p.0[2] * 16 + z]);
                    b.push(self.level(sample) | (self.block_level(sample) << 4));
                }
            }
        }
        ChunkLight(b.into_boxed_slice())
    }
}
/// Trim only planes whose values are proven from complete source descriptors.
/// No dense allocation or per-cell scan of empty upper sky or solid deep slabs.
/// Nonuniform/edited data anywhere in the height range participates normally.
fn active_span(world: &World, targets: &BTreeSet<ChunkPos>) -> (i32, i32) {
    let h = world.world_height();
    if h == crate::bounds::WorldHeight::HISTORICAL {
        return (h.min, h.max);
    }
    let columns = source_columns(targets);
    let classification = |c: [i32; 2], cy: i32| {
        let cp = ChunkPos([c[0], cy, c[1]]);
        match world.chunk(cp) {
            Some(chunk) => chunk.uniform(),
            None if world.ready(BlockPos([c[0] * 16, cy * 16, c[1] * 16])) => {
                Some(crate::Block::Air)
            }
            None => Some(crate::Block::Stone), // unknown is opaque, never implicit sky
        }
    };
    let top = h.chunks().rev().find(|&cy| {
        columns
            .iter()
            .any(|&c| classification(c, cy) != Some(crate::Block::Air))
    });
    let Some(top) = top else {
        return (h.min, h.min);
    };
    let bottom = (h.chunks().start..=top).find(|&cy| {
        columns
            .iter()
            .any(|&c| !classification(c, cy).is_some_and(|b| b.is_opaque()))
    });
    let min = bottom.map_or((top + 1) * 16, |cy| cy * 16);
    let max = ((top + 1) * 16 + HALO).min(h.max);
    (min, max.max(min))
}
fn flood(
    solid: &[bool],
    levels: &mut [u8],
    queue: &mut VecDeque<u32>,
    size: [usize; 2],
    height: usize,
) -> usize {
    let mut propagated = 0;
    while let Some(i) = queue.pop_front() {
        let i = i as usize;
        let value = levels[i].saturating_sub(1);
        if value == 0 {
            continue;
        }
        let y = i % height;
        let column = i / height;
        let x = column % size[0];
        let z = column / size[0];
        let neighbors = [
            (y > 0, i.wrapping_sub(1)),
            (y + 1 < height, i + 1),
            (x > 0, i.wrapping_sub(height)),
            (x + 1 < size[0], i + height),
            (z > 0, i.wrapping_sub(height * size[0])),
            (z + 1 < size[1], i + height * size[0]),
        ];
        for (valid, n) in neighbors {
            if valid && !solid[n] && levels[n] < value {
                levels[n] = value;
                queue.push_back(n as u32);
                propagated += 1;
            }
        }
    }
    propagated
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

#[cfg(test)]
mod reference;

#[cfg(test)]
mod optimization_tests;
