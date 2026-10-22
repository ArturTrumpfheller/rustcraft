//! Finite generation v1. All choices use fixed integer arithmetic and world
//! coordinates, never a shared RNG or chunk traversal order. No Bevy types.
use crate::{Block, BlockPos, CHUNK_SIZE, Chunk, ChunkPos, LocalPos, World};

pub const GENERATOR_VERSION: u32 = 1;
pub const DEFAULT_SEED: u64 = 2026;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationConfig {
    pub seed: u64,
    pub version: u32,
}
impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            version: GENERATOR_VERSION,
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct UnsupportedGenerator(pub u32);
impl std::fmt::Display for UnsupportedGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported generator version {}", self.0)
    }
}
impl std::error::Error for UnsupportedGenerator {}

#[derive(Clone, Debug)]
struct Tree {
    x: i32,
    z: i32,
    ground: i32,
    trunk: i32,
}
/// Small finite planning cache: 4096 heights and a few dozen tree anchors, no
/// duplicate voxel volume. Chunk requests can be evaluated in any order.
pub struct Generator {
    config: GenerationConfig,
    heights: Vec<i32>,
    trees: Vec<Tree>,
    entrance_height: i32,
    extent: i32,
}
fn hash(seed: u64, x: i32, z: i32, salt: u64) -> u64 {
    let mut v = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z as i64 as u64).wrapping_mul(0xd1b54a32d192ed03)
        ^ salt;
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}
// Q10 smoothstep value noise. Euclidean cells make negative seams continuous.
fn noise(seed: u64, x: i32, z: i32, scale: i32, salt: u64) -> i32 {
    let cx = x.div_euclid(scale);
    let cz = z.div_euclid(scale);
    let fade = |v: i32| {
        let t = v * 1024 / scale;
        t * t / 1024 * (3072 - 2 * t) / 1024
    };
    let tx = fade(x.rem_euclid(scale));
    let tz = fade(z.rem_euclid(scale));
    let at = |dx, dz| (hash(seed, cx + dx, cz + dz, salt) % 2049) as i32 - 1024;
    let lerp = |a, b, t| a + (b - a) * t / 1024;
    lerp(
        lerp(at(0, 0), at(1, 0), tx),
        lerp(at(0, 1), at(1, 1), tx),
        tz,
    )
}
impl Generator {
    pub fn new(config: GenerationConfig) -> Result<Self, UnsupportedGenerator> {
        if ![GENERATOR_VERSION, 2].contains(&config.version) {
            return Err(UnsupportedGenerator(config.version));
        }
        let extent = if config.version == 1 { 32 } else { 64 };
        let mut heights = Vec::with_capacity((extent * extent * 4) as usize);
        for z in -extent..extent {
            for x in -extent..extent {
                let broad = noise(config.seed, x, z, 32, 11);
                let hills = noise(config.seed, x, z, 16, 23);
                let detail = noise(config.seed, x, z, 8, 37);
                heights.push(5 + (broad * 7 + hills * 4 + detail * 2) / 1024);
            }
        }
        let mut generator = Self {
            config,
            heights,
            trees: Vec::new(),
            entrance_height: 0,
            extent,
        };
        generator.entrance_height = generator.height(-10, 20).unwrap() + 1;
        let trees = extent / 8 - 1;
        for gz in -trees..=trees {
            for gx in -trees..=trees {
                let h = hash(config.seed, gx, gz, 71);
                if h.is_multiple_of(5) {
                    continue;
                }
                let x = gx * 8 + (h % 3) as i32 - 1;
                let z = gz * 8 + ((h >> 8) % 3) as i32 - 1;
                // Reserve open ground near preferred spawn and the visible entrance.
                if (x.abs() <= 4 && (z - 10).abs() <= 4)
                    || ((x + 10).abs() <= 4 && (-19..=24).contains(&z))
                {
                    continue;
                }
                let ground = generator.height(x, z).unwrap();
                let trunk = 4 + ((h >> 16) % 2) as i32;
                if generator.terrain(x, ground, z) != Block::Grass {
                    continue;
                }
                let clear = (-2..=2).all(|dx| {
                    (-2..=2).all(|dz| {
                        generator
                            .height(x + dx, z + dz)
                            .is_some_and(|v| v < ground + trunk - 1)
                    })
                });
                if clear {
                    generator.trees.push(Tree {
                        x,
                        z,
                        ground,
                        trunk,
                    });
                }
            }
        }
        Ok(generator)
    }
    /// Surface block Y before caves/trees; None outside the finite plan.
    pub fn height(&self, x: i32, z: i32) -> Option<i32> {
        if !(-self.extent..self.extent).contains(&x) || !(-self.extent..self.extent).contains(&z) {
            return None;
        }
        Some(self.heights[((z + self.extent) * self.extent * 2 + x + self.extent) as usize])
    }
    fn cave(&self, x: i32, y: i32, z: i32) -> bool {
        if y <= -14 || (x.abs() <= 3 && (8..=13).contains(&z)) {
            return false;
        }
        let center = -10 + noise(self.config.seed, x, 0, 16, 53) * 3 / 1024;
        // A continuous walkable east/west tunnel, with wider caverns at nodes.
        let radius = if x.rem_euclid(19) < 5 { 5 } else { 3 };
        let main =
            (z - center).pow(2) * 9 + (y + 4).pow(2) * radius * radius <= radius * radius * 9;
        // Stair-stepped ramp joins the tunnel and opens through the surface.
        let floor = self.entrance_height
            - ((20 - z).clamp(0, 36) * (self.entrance_height + 6) / 32)
                .min(self.entrance_height + 6);
        let entrance =
            (x + 10).abs() <= 1 && (-16..=22).contains(&z) && y >= floor && y < floor + 4;
        main || entrance
    }
    fn terrain(&self, x: i32, y: i32, z: i32) -> Block {
        let Some(height) = self.height(x, z) else {
            return Block::Air;
        };
        if !(-16..32).contains(&y) || y > height || self.cave(x, y, z) {
            return Block::Air;
        }
        if y == height {
            Block::Grass
        } else if y >= height - 3 {
            Block::Dirt
        } else {
            Block::Stone
        }
    }
    /// Samples world-space cells. Trees are planned once and cross chunk seams;
    /// no neighboring chunk must already exist for a correct result.
    pub fn block(&self, BlockPos([x, y, z]): BlockPos) -> Block {
        let terrain = self.terrain(x, y, z);
        if terrain != Block::Air || !(-16..32).contains(&y) {
            return terrain;
        }
        // Tree clearance guarantees foliage is above the local terrain. Avoid
        // scanning anchors for carved underground air or out-of-bounds cells.
        if self.height(x, z).is_none_or(|height| y <= height) {
            return Block::Air;
        }
        for tree in &self.trees {
            let dy = y - tree.ground;
            if x == tree.x && z == tree.z && (1..=tree.trunk).contains(&dy) {
                return Block::Wood;
            }
        }
        for tree in &self.trees {
            let dx = (x - tree.x).abs();
            let dz = (z - tree.z).abs();
            let dy = y - tree.ground - tree.trunk;
            if (-1..=1).contains(&dy)
                && dx <= 2
                && dz <= 2
                && dx + dz <= if dy == 1 { 2 } else { 3 }
            {
                return Block::Leaves;
            }
        }
        Block::Air
    }
    pub fn chunk(&self, pos: ChunkPos) -> Option<Chunk> {
        if !(-self.extent / 16..self.extent / 16).contains(&pos.0[0])
            || !(-1..2).contains(&pos.0[1])
            || !(-self.extent / 16..self.extent / 16).contains(&pos.0[2])
        {
            return None;
        }
        let mut chunk = Chunk::default();
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = LocalPos::new([x, y, z]).unwrap();
                    chunk.set(local, self.block(pos.block_position(local).unwrap()));
                }
            }
        }
        Some(chunk)
    }
    pub fn generate(&self) -> World {
        let mut world = if self.config.version == 1 {
            World::default()
        } else {
            World::streaming(crate::StreamIdentity {
                seed: self.config.seed,
                generator: 2,
            })
        };
        for y in -1..2 {
            for z in -self.extent / 16..self.extent / 16 {
                for x in -self.extent / 16..self.extent / 16 {
                    let pos = ChunkPos([x, y, z]);
                    let chunk = self.chunk(pos).unwrap();
                    if (0..16).any(|ly| {
                        (0..16).any(|lz| {
                            (0..16).any(|lx| {
                                chunk.get(LocalPos::new([lx, ly, lz]).unwrap()).is_solid()
                            })
                        })
                    }) {
                        world.insert_chunk(pos, chunk);
                    }
                }
            }
        }
        world
    }
}

#[cfg(test)]
mod tests;
