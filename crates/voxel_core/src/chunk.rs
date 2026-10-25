/// Blocks per chunk axis. Y is up; blocks occupy [x, x+1] on each axis.
pub const CHUNK_SIZE: usize = 16;
const VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Block {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Torch,
    TorchEast,
    TorchWest,
    TorchSouth,
    TorchNorth,
}

impl Block {
    pub fn is_solid(self) -> bool {
        self.is_collidable()
    }

    /// Physical collision is independent of rendering, targeting and lighting.
    pub fn is_collidable(self) -> bool {
        self != Self::Air && !self.is_torch()
    }
    pub fn is_opaque(self) -> bool {
        self != Self::Air && !self.is_torch()
    }
    pub fn is_cube(self) -> bool {
        self != Self::Air && !self.is_torch()
    }
    pub fn is_torch(self) -> bool {
        matches!(
            self,
            Self::Torch | Self::TorchEast | Self::TorchWest | Self::TorchSouth | Self::TorchNorth
        )
    }
    /// Offset from torch cell to its supporting full cube.
    pub fn torch_support(self) -> Option<[i32; 3]> {
        Some(match self {
            Self::Torch => [0, -1, 0],
            Self::TorchEast => [-1, 0, 0],
            Self::TorchWest => [1, 0, 0],
            Self::TorchSouth => [0, 0, -1],
            Self::TorchNorth => [0, 0, 1],
            _ => return None,
        })
    }
    /// Face points outward from the support toward the new torch cell.
    pub fn torch_on_face(normal: [i32; 3]) -> Option<Self> {
        Some(match normal {
            [0, 1, 0] => Self::Torch,
            [1, 0, 0] => Self::TorchEast,
            [-1, 0, 0] => Self::TorchWest,
            [0, 0, 1] => Self::TorchSouth,
            [0, 0, -1] => Self::TorchNorth,
            _ => return None,
        })
    }
    pub fn is_targetable(self) -> bool {
        self != Self::Air
    }
    pub fn emission(self) -> u8 {
        if self.is_torch() { 14 } else { 0 }
    }
    /// Original linear RGBA vertex colors, independent of any texture assets.
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Air => [0.0; 4],
            Self::Grass => [0.18, 0.42, 0.07, 1.0],
            Self::Dirt => [0.28, 0.13, 0.06, 1.0],
            Self::Stone => [0.32, 0.35, 0.38, 1.0],
            Self::Wood => [0.24, 0.12, 0.035, 1.0],
            Self::Leaves => [0.075, 0.29, 0.045, 1.0],
            Self::Torch
            | Self::TorchEast
            | Self::TorchWest
            | Self::TorchSouth
            | Self::TorchNorth => [1.0, 0.65, 0.12, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BlockPos(pub [i32; 3]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ChunkPos(pub [i32; 3]);

/// A validated coordinate within a chunk; invalid array indexing is unrepresentable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalPos([usize; 3]);

impl LocalPos {
    pub fn new(coordinates: [usize; 3]) -> Option<Self> {
        coordinates
            .iter()
            .all(|&axis| axis < CHUNK_SIZE)
            .then_some(Self(coordinates))
    }

    pub fn coordinates(self) -> [usize; 3] {
        self.0
    }

    /// X varies fastest, then Z, then Y.
    pub fn index(self) -> usize {
        let [x, y, z] = self.0;
        x + CHUNK_SIZE * (z + CHUNK_SIZE * y)
    }
}

impl BlockPos {
    pub fn split(self) -> (ChunkPos, LocalPos) {
        let chunk = self.0.map(|axis| axis.div_euclid(CHUNK_SIZE as i32));
        let local = self
            .0
            .map(|axis| axis.rem_euclid(CHUNK_SIZE as i32) as usize);
        (ChunkPos(chunk), LocalPos(local))
    }

    pub fn offset(self, delta: [i32; 3]) -> Option<Self> {
        let [x, y, z] = self.0;
        Some(Self([
            x.checked_add(delta[0])?,
            y.checked_add(delta[1])?,
            z.checked_add(delta[2])?,
        ]))
    }
}

impl ChunkPos {
    /// Returns None if this chunk cannot be represented in i32 block coordinates.
    pub fn block_position(self, local: LocalPos) -> Option<BlockPos> {
        let mut result = [0; 3];
        for (axis, value) in result.iter_mut().enumerate() {
            *value = self.0[axis]
                .checked_mul(CHUNK_SIZE as i32)?
                .checked_add(local.0[axis] as i32)?;
        }
        Some(BlockPos(result))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    blocks: Vec<Block>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            blocks: vec![Block::Air; VOLUME],
        }
    }
}

impl Chunk {
    pub fn get(&self, position: LocalPos) -> Block {
        self.blocks[position.index()]
    }

    pub fn set(&mut self, position: LocalPos, block: Block) -> bool {
        let cell = &mut self.blocks[position.index()];
        if *cell == block {
            return false;
        }
        *cell = block;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_local_coordinate_has_a_unique_index_and_round_trips_storage() {
        let mut seen = vec![false; VOLUME];
        let mut chunk = Chunk::default();
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let local = LocalPos::new([x, y, z]).unwrap();
                    assert!(!seen[local.index()]);
                    seen[local.index()] = true;
                    assert_eq!(chunk.get(local), Block::Air);
                    chunk.set(local, Block::Stone);
                    assert_eq!(chunk.get(local), Block::Stone);
                }
            }
        }
        assert!(seen.into_iter().all(|value| value));
    }

    #[test]
    fn rejects_invalid_local_coordinates_on_every_axis() {
        for coordinates in [[16, 0, 0], [0, 16, 0], [0, 0, 16], [usize::MAX; 3]] {
            assert_eq!(LocalPos::new(coordinates), None);
        }
    }

    #[test]
    fn negative_boundaries_use_euclidean_coordinates() {
        for coordinate in [
            i32::MIN,
            -33,
            -32,
            -17,
            -16,
            -15,
            -1,
            0,
            1,
            15,
            16,
            17,
            i32::MAX,
        ] {
            let position = BlockPos([coordinate; 3]);
            let (chunk, local) = position.split();
            assert_eq!(chunk.block_position(local), Some(position));
        }
        let (chunk, local) = BlockPos([-1, -16, -17]).split();
        assert_eq!(chunk, ChunkPos([-1, -1, -2]));
        assert_eq!(local.coordinates(), [15, 0, 15]);
    }

    #[test]
    fn coordinate_overflow_is_reported() {
        assert_eq!(BlockPos([i32::MAX, 0, 0]).offset([1, 0, 0]), None);
        assert_eq!(BlockPos([i32::MIN, 0, 0]).offset([-1, 0, 0]), None);
        assert_eq!(
            ChunkPos([i32::MAX, 0, 0]).block_position(LocalPos::new([0; 3]).unwrap()),
            None
        );
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;
    #[test]
    fn chunk_set_reports_actual_changes_and_noops() {
        let mut chunk = Chunk::default();
        let local = LocalPos::new([15, 0, 15]).unwrap();
        assert!(!chunk.set(local, Block::Air));
        assert!(chunk.set(local, Block::Stone));
        assert!(!chunk.set(local, Block::Stone));
        assert!(chunk.set(local, Block::Dirt));
        assert_eq!(chunk.get(local), Block::Dirt);
    }
}
