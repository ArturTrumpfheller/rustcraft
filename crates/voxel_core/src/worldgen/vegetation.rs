// SPDX-License-Identifier: MIT OR Apache-2.0
//! V9 vegetation only. Fixed candidate neighborhood; no loaded-world dependency.
use super::{Biome, Climate, Generator, biome, caves, hash, hydrology, noise};
use crate::{Block, BlockPos, Chunk, ChunkPos};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Species {
    Oak,
    Birch,
    Spruce,
}
#[derive(Clone, Copy, Debug)]
struct Tree {
    x: i32,
    z: i32,
    ground: i32,
    height: i32,
    species: Species,
    variant: u64,
}
impl Tree {
    fn wood(self) -> Block {
        match self.species {
            Species::Oak => Block::Wood,
            Species::Birch => Block::BirchWood,
            Species::Spruce => Block::SpruceWood,
        }
    }
    fn leaves(self) -> Block {
        match self.species {
            Species::Oak => Block::Leaves,
            Species::Birch => Block::BirchLeaves,
            Species::Spruce => Block::SpruceLeaves,
        }
    }
    fn trunk(self, p: BlockPos) -> bool {
        p.0[0] == self.x && p.0[2] == self.z && (1..=self.height).contains(&(p.0[1] - self.ground))
    }
    fn crown(self, p: BlockPos) -> bool {
        let dx = (p.0[0] - self.x).abs();
        let dz = (p.0[2] - self.z).abs();
        let dy = p.0[1] - self.ground;
        match self.species {
            Species::Oak => {
                let top = dy - self.height;
                (-1..=1).contains(&top)
                    && dx <= 2
                    && dz <= 2
                    && dx + dz
                        <= if top == 1 {
                            1 + (self.variant % 2) as i32
                        } else {
                            3
                        }
            }
            Species::Birch => {
                let top = dy - self.height;
                (-2..=1).contains(&top)
                    && dx <= 2
                    && dz <= 2
                    && dx + dz
                        <= if top == -1 || top == 0 {
                            2 + (self.variant % 2) as i32
                        } else {
                            1
                        }
            }
            Species::Spruce => {
                if !(3..=self.height + 1).contains(&dy) {
                    return false;
                }
                let below = self.height + 1 - dy;
                let mut radius = if below == 0 {
                    0
                } else if below < 4 {
                    1
                } else {
                    2
                };
                // Broad lower tiers taper toward the tip; every third tier is
                // inset, with the central trunk connecting the foliage layers.
                if below >= 3 && below % 3 == 0 {
                    radius = (radius - 1).max(1);
                }
                dx <= radius && dz <= radius && dx + dz <= radius + i32::from(radius == 2)
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Plan {
    trees: Vec<Tree>,
    plants: Vec<(BlockPos, Block)>,
}
/// Shared dry-ground admission includes the final basin bed, shore material and
/// real cave opening. Sampling is cached per candidate footprint during planning.
struct Roots {
    geography: hydrology::Geography,
    caves: caves::Plan,
    water: BTreeMap<[i32; 2], hydrology::Surface>,
}
impl Roots {
    fn new(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        Self {
            geography: hydrology::Geography::new(g.config.seed, min, max),
            caves: caves::Plan::new(g.config.seed, min, max),
            water: BTreeMap::new(),
        }
    }
    fn surface(&mut self, g: &Generator, x: i32, z: i32) -> hydrology::Surface {
        *self.water.entry([x, z]).or_insert_with(|| {
            self.geography
                .column(g.config.seed, x, z, biome::height(g.config.seed, x, z))
        })
    }
    fn ground(&mut self, g: &Generator, x: i32, z: i32) -> (i32, Block) {
        let water = self.surface(g, x, z);
        let h = water.bed;
        if h <= hydrology::SEA_LEVEL {
            return (h, Block::Air);
        }
        if self.caves.contains(BlockPos([x, h, z]), h) {
            let sealed = [[-1, 0], [1, 0], [0, -1], [0, 1]]
                .into_iter()
                .any(|[dx, dz]| self.surface(g, x + dx, z + dz).depth() > 0);
            if !sealed {
                return (h, Block::Air);
            }
        }
        let c = biome::column(g.config.seed, x, z);
        let top = if h <= hydrology::SEA_LEVEL + 2
            && water.coastal
            && !matches!(c.top, Block::SnowyGround | Block::Stone)
        {
            Block::Sand
        } else {
            c.top
        };
        (h, top)
    }
}
#[cfg(test)]
fn root(g: &Generator, x: i32, z: i32) -> (i32, Block) {
    Roots::new(g, [x - 1, z - 1], [x + 1, z + 1]).ground(g, x, z)
}
fn reserved(x: i32, z: i32) -> bool {
    x.abs() <= 5 && (z - 10).abs() <= 5
}
fn density(seed: u64, x: i32, z: i32, c: Climate) -> u64 {
    // Continuous moisture/cold/highland weights plus broad groves and clearings.
    let forest = ((c.moisture + 40) * 1024 / 240).clamp(0, 1024);
    let cold = ((-c.temperature - 180) * 1024 / 160).clamp(0, 1024);
    let high = ((c.geography - 100) * 1024 / 280).clamp(0, 1024);
    let base = (75 + forest * 700 / 1024) * (1024 - cold * 4 / 5) / 1024;
    let grove = (600 + noise(seed, x, z, 48, 0x9010) * 750 / 1024).clamp(0, 1024);
    (base * (1024 - high * 9 / 10) / 1024 * grove / 1024) as u64
}
impl Plan {
    pub(super) fn trees(g: &Generator, min: [i32; 2], max: [i32; 2]) -> Self {
        let mut result = Self::default();
        let mut ground_cache = BTreeMap::new();
        let mut roots: Option<Roots> = None;
        for gz in (min[1] - 9).div_euclid(8)..=(max[1] + 2).div_euclid(8) {
            for gx in (min[0] - 9).div_euclid(8)..=(max[0] + 2).div_euclid(8) {
                let h = hash(g.config.seed, gx, gz, 0x9001);
                let x = gx * 8 + (h % 8) as i32;
                let z = gz * 8 + ((h >> 8) % 8) as i32;
                if x < min[0] - 2
                    || x > max[0] + 2
                    || z < min[1] - 2
                    || z > max[1] + 2
                    || x < -g.extent + 3
                    || x >= g.extent - 3
                    || z < -g.extent + 3
                    || z >= g.extent - 3
                    || reserved(x, z)
                {
                    continue;
                }
                let c = biome::sample_climate(g.config.seed, x, z).unwrap();
                if biome::classify(c) == Biome::Desert
                    || (h >> 24) % 1000 >= density(g.config.seed, x, z, c)
                {
                    continue;
                }
                let species = if c.temperature < -180 || c.geography > 200 {
                    Species::Spruce
                } else if (h >> 36) % 100 < (42 + c.moisture / 40).clamp(20, 60) as u64 {
                    Species::Birch
                } else {
                    Species::Oak
                };
                let height = match species {
                    Species::Oak => 4 + (h >> 42) % 3,
                    Species::Birch => 5 + (h >> 42) % 3,
                    Species::Spruce => 7 + (h >> 42) % 4,
                } as i32;
                let roots = roots
                    .get_or_insert_with(|| Roots::new(g, min.map(|v| v - 5), max.map(|v| v + 5)));
                let (ground, top) = *ground_cache
                    .entry([x, z])
                    .or_insert_with(|| roots.ground(g, x, z));
                if !matches!(top, Block::Grass | Block::SnowyGround)
                    || ground + height + 1 >= g.world_height().max
                {
                    continue;
                }
                // Full root/crown footprint rejects steep slopes, water banks and
                // cave mouths; every admitted leaf stays above the final terrain.
                let clear = (-2..=2).all(|dx| {
                    (-2..=2).all(|dz| {
                        let (other, material) = *ground_cache
                            .entry([x + dx, z + dz])
                            .or_insert_with(|| roots.ground(g, x + dx, z + dz));
                        (other - ground).abs() <= 1 && material != Block::Air
                    })
                });
                if clear {
                    result.trees.push(Tree {
                        x,
                        z,
                        ground,
                        height,
                        species,
                        variant: h >> 48,
                    });
                }
            }
        }
        result
    }
    pub(super) fn plants(
        &mut self,
        seed: u64,
        min: [i32; 2],
        heights: &[i32],
        columns: &[biome::Column],
        support: impl Fn(BlockPos) -> Block,
    ) {
        for z in 0..18 {
            for x in 0..18 {
                let i = z * 18 + x;
                let px = min[0] + x as i32;
                let pz = min[1] + z as i32;
                if !(-32768..32768).contains(&px)
                    || !(-32768..32768).contains(&pz)
                    || reserved(px, pz)
                    || heights[i] <= hydrology::SEA_LEVEL
                {
                    continue;
                }
                let p = BlockPos([px, heights[i] + 1, pz]);
                if self.block(p).is_some() {
                    continue;
                }
                let c = columns[i].climate;
                let patch = noise(seed, px, pz, 18, 0x9020);
                let flower = noise(seed, px, pz, 28, 0x9021);
                let h = hash(seed, px, pz, 0x9022);
                let b = match biome::classify(c) {
                    Biome::Desert => (patch > 80 && h % 1000 < 32).then_some(Block::DeadBush),
                    Biome::SnowyPlains => (patch > 180 && h % 1000 < 20).then_some(Block::Fern),
                    Biome::RockyHighlands => (patch > 220 && h % 1000 < 24).then_some(Block::Fern),
                    Biome::Plains | Biome::Forest => {
                        let chance =
                            (65 + patch / 4 + c.moisture.clamp(-200, 400) / 8).clamp(0, 270) as u64;
                        if h % 1000 >= chance {
                            None
                        } else if flower > 240 {
                            Some(if noise(seed, px, pz, 48, 0x9023) > 0 {
                                Block::RedFlower
                            } else {
                                Block::YellowFlower
                            })
                        } else {
                            Some(if c.moisture > 80 && (h >> 16) % 100 < 55 {
                                Block::Fern
                            } else {
                                Block::TallGrass
                            })
                        }
                    }
                };
                if let Some(b) = b
                    && b.supported_by(support(BlockPos([px, heights[i], pz])))
                    && support(p) == Block::Air
                {
                    self.plants.push((p, b));
                }
            }
        }
    }
    pub(super) fn block(&self, p: BlockPos) -> Option<Block> {
        self.trees
            .iter()
            .find(|t| t.trunk(p))
            .map(|t| t.wood())
            .or_else(|| self.trees.iter().find(|t| t.crown(p)).map(|t| t.leaves()))
            .or_else(|| self.plants.iter().find(|(at, _)| *at == p).map(|(_, b)| *b))
    }
    pub(super) fn apply(&self, chunk: &mut Chunk, p: ChunkPos) {
        // Rasterize bounded tree intersections, rather than discover/replan per voxel.
        for wood in [false, true] {
            for t in &self.trees {
                let min = [t.x - 2, t.ground + 1, t.z - 2];
                let max = [t.x + 2, t.ground + t.height + 1, t.z + 2];
                let lo: [i32; 3] = std::array::from_fn(|i| min[i].max(p.0[i] * 16));
                let hi: [i32; 3] = std::array::from_fn(|i| max[i].min(p.0[i] * 16 + 15));
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        for x in lo[0]..=hi[0] {
                            let bp = BlockPos([x, y, z]);
                            let (_, lp) = bp.split();
                            if wood && t.trunk(bp) {
                                chunk.set(lp, t.wood());
                            } else if !wood && t.crown(bp) && chunk.get(lp) == Block::Air {
                                chunk.set(lp, t.leaves());
                            }
                        }
                    }
                }
            }
        }
        for &(bp, b) in &self.plants {
            let (cp, lp) = bp.split();
            if cp == p && chunk.get(lp) == Block::Air {
                chunk.set(lp, b);
            }
        }
    }
    pub(super) fn max_y(&self) -> i32 {
        self.trees
            .iter()
            .map(|t| t.ground + t.height + 1)
            .chain(self.plants.iter().map(|(p, _)| p.0[1]))
            .max()
            .unwrap_or(i32::MIN)
    }
    pub(super) fn bytes(&self) -> usize {
        self.trees.capacity() * std::mem::size_of::<Tree>()
            + self.plants.capacity() * std::mem::size_of::<(BlockPos, Block)>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::GenerationConfig;
    #[test]
    fn species_have_connected_distinct_crowns_and_vary_heights() {
        use std::collections::BTreeSet;
        let mut shapes = Vec::new();
        for species in [Species::Oak, Species::Birch, Species::Spruce] {
            let t = Tree {
                x: -16,
                z: -1,
                ground: 10,
                height: match species {
                    Species::Oak => 5,
                    Species::Birch => 6,
                    Species::Spruce => 9,
                },
                species,
                variant: 3,
            };
            let mut cells = BTreeSet::new();
            for y in 11..=21 {
                for z in -3..=1 {
                    for x in -18..=-14 {
                        let p = BlockPos([x, y, z]);
                        if t.trunk(p) || t.crown(p) {
                            cells.insert(p);
                        }
                    }
                }
            }
            let mut seen = BTreeSet::from([BlockPos([t.x, 11, t.z])]);
            let mut todo = vec![BlockPos([t.x, 11, t.z])];
            while let Some(p) = todo.pop() {
                for d in crate::fluid::NEIGHBORS {
                    let n = p.offset(d).unwrap();
                    if cells.contains(&n) && seen.insert(n) {
                        todo.push(n);
                    }
                }
            }
            assert_eq!(seen, cells, "floating canopy {species:?}");
            shapes.push(cells);
        }
        assert_ne!(shapes[0], shapes[1]);
        assert_ne!(shapes[1], shapes[2]);
        let g = Generator::new(GenerationConfig {
            seed: 0,
            version: 9,
        })
        .unwrap();
        let mut heights = [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        for x in (-2304..-256).step_by(16) {
            let plan = Plan::trees(&g, [x, -4096], [x + 15, -4081]);
            for t in &plan.trees {
                let (ground, b) = root(&g, t.x, t.z);
                assert_eq!(ground, t.ground);
                assert!(matches!(b, Block::Grass | Block::SnowyGround));
                assert!(t.ground + t.height + 1 < 192);
                heights[match t.species {
                    Species::Oak => 0,
                    Species::Birch => 1,
                    Species::Spruce => 2,
                }]
                .insert(t.height);
            }
        }
        for h in heights {
            assert!(h.len() >= 2, "height variation: {h:?}");
        }
    }
    #[test]
    fn v9_preserves_v8_geology_and_plants_have_final_dry_surface_support() {
        let mut kinds = std::collections::BTreeSet::new();
        for (seed, x, z) in [
            (0, -2432_i32, -4096_i32),
            (0, -2304, -4096),
            (0, -2176, -4096),
            (0, -1536, -4032),
            (42, -4096, -4096),
            (2026, 0, 0),
            (42, -434, 31),
            (0, -4096, -4096),
        ] {
            let old = Generator::new(GenerationConfig { seed, version: 8 }).unwrap();
            let new = Generator::new(GenerationConfig { seed, version: 9 }).unwrap();
            let c = [x.div_euclid(16), z.div_euclid(16)];
            let a = old.sampler(c);
            let b = new.sampler(c);
            let surface_heights: Vec<_> = (0..256)
                .map(|i| new.height(c[0] * 16 + i % 16, c[1] * 16 + i / 16).unwrap())
                .collect();
            for y in new.world_height().chunks() {
                let cp = ChunkPos([c[0], y, c[1]]);
                let ca = a.chunk(cp).unwrap();
                let cb = b.chunk(cp).unwrap();
                for i in 0..4096 {
                    let lp = crate::LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(lp).unwrap();
                    let block = cb.get(lp);
                    if p.0[1] <= surface_heights[i % 256] {
                        assert_eq!(ca.get(lp), block, "geology {p:?}");
                    }
                    if block.is_plant() {
                        kinds.insert(block as u8);
                        match biome::sample_biome(seed, p.0[0], p.0[2]).unwrap() {
                            Biome::Desert => assert_eq!(block, Block::DeadBush),
                            Biome::SnowyPlains | Biome::RockyHighlands => {
                                assert_eq!(block, Block::Fern)
                            }
                            Biome::Plains | Biome::Forest => assert_ne!(block, Block::DeadBush),
                        }
                        let below = p.offset([0, -1, 0]).unwrap();
                        assert!(block.supported_by(b.block(below)));
                        assert_eq!(p.0[1], new.height(p.0[0], p.0[2]).unwrap() + 1);
                        assert_eq!(a.block(p), Block::Air);
                    }
                    if i % 997 == 0 {
                        assert_eq!(b.block(p), block, "sampler/chunk {p:?}");
                    }
                }
            }
            for [dx, dz] in [[0, 0], [15, 15], [0, 15], [15, 0]] {
                let px = c[0] * 16 + dx;
                let pz = c[1] * 16 + dz;
                assert_eq!(old.height(px, pz), new.height(px, pz));
                let p = BlockPos([px, new.height(px, pz).unwrap() + 1, pz]);
                assert_eq!(new.block(p), b.block(p));
            }
        }
        assert_eq!(kinds.len(), 5, "missing plant variety {kinds:?}");
    }
    #[test]
    fn signed_seams_reverse_parallel_order_and_sampler_halos_agree() {
        let g = Generator::new(GenerationConfig {
            seed: 0,
            version: 9,
        })
        .unwrap();
        let coords = [[-137, -257], [-136, -257], [-137, -256], [-136, -256]];
        let chunks = |c: [i32; 2]| {
            let s = g.sampler(c);
            (0..3)
                .map(|y| s.chunk(ChunkPos([c[0], y, c[1]])).unwrap())
                .collect::<Vec<_>>()
        };
        let expected: Vec<_> = coords.into_iter().map(chunks).collect();
        for i in (0..coords.len()).rev() {
            assert_eq!(chunks(coords[i]), expected[i]);
        }
        std::thread::scope(|scope| {
            let handles: Vec<_> = coords
                .into_iter()
                .map(|c| scope.spawn(move || chunks(c)))
                .collect();
            for (h, e) in handles.into_iter().zip(&expected) {
                assert_eq!(&h.join().unwrap(), e);
            }
        });
        for c in coords {
            let s = g.sampler(c);
            for dx in [-1, 16] {
                for z in [0, 7, 15] {
                    let x = c[0] * 16 + dx;
                    let z = c[1] * 16 + z;
                    let own = g.sampler([x.div_euclid(16), z.div_euclid(16)]);
                    for y in 3..32 {
                        let p = BlockPos([x, y, z]);
                        assert_eq!(s.block(p), own.block(p), "halo {p:?}");
                    }
                }
            }
        }
        let spawn = g.sampler([0, 0]);
        for z in 5..16 {
            for x in -5..=5 {
                let p = BlockPos([x, g.height(x, z).unwrap() + 1, z]);
                assert!(!spawn.block(p).is_plant());
            }
        }
    }
}
