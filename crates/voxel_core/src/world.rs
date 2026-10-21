use crate::interaction::EditError;
use std::collections::{BTreeMap, BTreeSet};

use crate::{Block, BlockPos, CHUNK_SIZE, Chunk, ChunkPos, LocalPos};

/// Editable prototype bounds, minimum inclusive and maximum exclusive.
/// Reads/meshing retain the existing missing-chunk-is-air semantics.
pub const EDIT_MIN: [i32; 3] = [-32, -16, -32];
pub const EDIT_MAX: [i32; 3] = [32, 32, 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockChange {
    pub position: BlockPos,
    pub before: Block,
    pub after: Block,
}

impl BlockChange {
    /// Only face-sharing neighbor chunks need remeshing, never diagonals.
    pub fn affected_chunks(self) -> BTreeSet<ChunkPos> {
        let (chunk, local) = self.position.split();
        let mut result = BTreeSet::from([chunk]);
        for (axis, coordinate) in local.coordinates().into_iter().enumerate() {
            let delta = if coordinate == 0 {
                -1
            } else if coordinate == CHUNK_SIZE - 1 {
                1
            } else {
                continue;
            };
            let mut neighbor = chunk.0;
            if let Some(value) = neighbor[axis].checked_add(delta) {
                neighbor[axis] = value;
                result.insert(ChunkPos(neighbor));
            }
        }
        result
    }
}

/// Finite world. Missing chunks are air; this rule also applies at mesh boundaries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct World {
    chunks: BTreeMap<ChunkPos, Chunk>,
}

impl World {
    /// Raw bounded mutation for trusted world-edit operations. Placement rules
    /// (solid replacement/player overlap) live in interaction::edit.
    pub fn set_block(
        &mut self,
        position: BlockPos,
        block: Block,
    ) -> Result<Option<BlockChange>, EditError> {
        if !(0..3)
            .all(|axis| position.0[axis] >= EDIT_MIN[axis] && position.0[axis] < EDIT_MAX[axis])
        {
            return Err(EditError::OutOfBounds);
        }
        let before = self.block(position);
        if before == block {
            return Ok(None);
        }
        let (chunk, local) = position.split();
        self.chunks.entry(chunk).or_default().set(local, block);
        Ok(Some(BlockChange {
            position,
            before,
            after: block,
        }))
    }

    pub fn insert_chunk(&mut self, position: ChunkPos, chunk: Chunk) {
        self.chunks.insert(position, chunk);
    }

    pub fn chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&position)
    }

    pub fn chunks(&self) -> impl Iterator<Item = (ChunkPos, &Chunk)> {
        self.chunks
            .iter()
            .map(|(&position, chunk)| (position, chunk))
    }

    pub fn block(&self, position: BlockPos) -> Block {
        let (chunk, local) = position.split();
        self.chunks
            .get(&chunk)
            .map_or(Block::Air, |chunk| chunk.get(local))
    }
}

/// Four adjacent chunks with fixed terraces and an original stone landmark.
/// This is a bounded test scene, not a seeded world generator.
pub fn demo_world() -> World {
    let mut world = World::default();
    for chunk_x in -1..=0 {
        for chunk_z in -1..=0 {
            let position = ChunkPos([chunk_x, 0, chunk_z]);
            let mut chunk = Chunk::default();
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let world_x = chunk_x * CHUNK_SIZE as i32 + x as i32;
                    let world_z = chunk_z * CHUNK_SIZE as i32 + z as i32;
                    let distance = world_x.abs().max(world_z.abs());
                    let height = 3 + (12 - distance).max(0) / 4;
                    for y in 0..height as usize {
                        let block = if y + 1 == height as usize {
                            Block::Grass
                        } else if y < 2 {
                            Block::Stone
                        } else {
                            Block::Dirt
                        };
                        chunk.set(LocalPos::new([x, y, z]).unwrap(), block);
                    }
                    if (3..=5).contains(&world_x) && (-3..=0).contains(&world_z) {
                        for y in height as usize..height as usize + 3 {
                            chunk.set(LocalPos::new([x, y, z]).unwrap(), Block::Stone);
                        }
                    }
                }
            }
            world.insert_chunk(position, chunk);
        }
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_resolves_negative_chunks_and_missing_space() {
        let mut chunk = Chunk::default();
        chunk.set(LocalPos::new([15, 0, 15]).unwrap(), Block::Dirt);
        let mut world = World::default();
        world.insert_chunk(ChunkPos([-1, 0, -1]), chunk);
        assert_eq!(world.block(BlockPos([-1, 0, -1])), Block::Dirt);
        assert_eq!(world.block(BlockPos([0, 0, 0])), Block::Air);
        assert_eq!(world.block(BlockPos([-2, 0, -1])), Block::Air);
    }

    #[test]
    fn demo_is_repeatable_and_contains_contiguous_terrain_and_landmark() {
        let world = demo_world();
        assert_eq!(world, demo_world());
        assert_eq!(world.chunks().count(), 4);
        assert_eq!(world.block(BlockPos([-1, 0, -1])), Block::Stone);
        assert_eq!(world.block(BlockPos([0, 0, 0])), Block::Stone);
        assert_eq!(world.block(BlockPos([0, 5, 0])), Block::Grass);
        assert_eq!(world.block(BlockPos([4, 7, -1])), Block::Stone);
        assert_eq!(world.block(BlockPos([16, 0, 0])), Block::Air);
    }
}
