use crate::interaction::EditError;
use crate::regions::{
    EditSnapshot, LARGE_EXTENT, MAX_DIRTY_REGIONS, RegionData, RegionEdits, RegionIdentity,
    RegionPos,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{Block, BlockPos, CHUNK_SIZE, Chunk, ChunkPos, LocalPos};

/// Legacy full-snapshot/edit bounds, minimum inclusive and maximum exclusive.
/// Streamed worlds expose their expanded limits through World::bounds.
pub const EDIT_MIN: [i32; 3] = [-32, -16, -32];
pub const EDIT_MAX: [i32; 3] = [32, 32, 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockChange {
    pub position: BlockPos,
    pub before: Block,
    pub after: Block,
}

impl BlockChange {
    /// One-cell geometry/AO footprint, including edge- and corner-sharing chunks.
    /// An interior edit affects one chunk; an edge/corner edit at most eight.
    pub fn affected_chunks(self) -> BTreeSet<ChunkPos> {
        let mut result = BTreeSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(p) = self.position.offset([dx, dy, dz]) {
                        result.insert(p.split().0);
                    }
                }
            }
        }
        result
    }
}

/// Versioned procedural identity, persisted only by v3. Generator v1 worlds
/// are legacy snapshots; streamed worlds currently select generator v2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamIdentity {
    pub seed: u64,
    pub generator: u32,
}

/// Authoritative identity/edits plus a finite resident cache. Legacy missing
/// chunks are Air; streamed in-volume missing chunks are pending solid barriers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct World {
    chunks: BTreeMap<ChunkPos, Chunk>,
    revision: u64,
    // Transient output dependencies, not persisted and retained across eviction.
    // Geometry plus bounded full-height skylight dependencies advance on edits.
    mesh_revisions: BTreeMap<ChunkPos, u64>,
    identity: Option<StreamIdentity>,
    regional: Option<RegionIdentity>,
    regions: BTreeMap<RegionPos, RegionData>,
    edits: Arc<BTreeMap<BlockPos, Block>>,
}

impl World {
    pub fn streaming(identity: StreamIdentity) -> Self {
        Self {
            identity: Some(identity),
            ..Self::default()
        }
    }
    pub fn regional(identity: RegionIdentity) -> Self {
        assert_ne!(identity.nonce, [0; 16]);
        Self {
            regional: Some(identity),
            identity: Some(StreamIdentity {
                seed: identity.seed,
                generator: 3,
            }),
            ..Self::default()
        }
    }
    pub fn region_identity(&self) -> Option<RegionIdentity> {
        self.regional
    }
    pub fn overlay_snapshot(&self) -> EditSnapshot {
        let mut s = EditSnapshot::from(Arc::clone(&self.edits));
        for (&p, r) in &self.regions {
            s.insert_region(p, Arc::clone(&r.edits));
        }
        s
    }
    pub fn install_region(&mut self, p: RegionPos, edits: RegionEdits) {
        assert!(self.regional.is_some() && p.valid());
        self.regions.entry(p).or_insert(RegionData {
            edits,
            revision: 0,
            dirty: false,
        });
    }
    pub(crate) fn restore_dirty_region(
        &mut self,
        p: RegionPos,
        edits: crate::regions::RegionEdits,
    ) {
        self.regions.insert(
            p,
            RegionData {
                edits,
                revision: self.revision,
                dirty: true,
            },
        );
    }
    pub fn cached_regions(&self) -> usize {
        self.regions.len()
    }
    pub fn edit_count(&self) -> usize {
        self.edits.len() + self.regions.values().map(|r| r.edits.len()).sum::<usize>()
    }
    pub fn dirty_regions(&self) -> usize {
        self.regions.values().filter(|r| r.dirty).count()
    }
    pub fn save_pressure(&self) -> bool {
        self.dirty_regions() >= MAX_DIRTY_REGIONS / 2
    }
    pub fn mark_regions_saved(&mut self, revision: u64) {
        for r in self.regions.values_mut() {
            if r.revision <= revision {
                r.dirty = false;
            }
        }
    }
    pub fn retain_regions(&mut self, keep: &BTreeSet<RegionPos>) {
        self.regions.retain(|p, r| r.dirty || keep.contains(p));
        self.mesh_revisions.retain(|p, _| {
            keep.contains(&RegionPos([
                p.0[0].div_euclid(crate::regions::REGION_CHUNKS),
                p.0[2].div_euclid(crate::regions::REGION_CHUNKS),
            ]))
        });
    }
    pub(crate) fn dirty_region_snapshots(
        &self,
    ) -> impl Iterator<Item = (RegionPos, RegionEdits)> + '_ {
        self.regions
            .iter()
            .filter(|(_, r)| r.dirty)
            .map(|(&p, r)| (p, Arc::clone(&r.edits)))
    }
    pub fn identity(&self) -> Option<StreamIdentity> {
        self.identity
    }
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        if self.regional.is_some() {
            (
                [-LARGE_EXTENT, -16, -LARGE_EXTENT],
                [LARGE_EXTENT, 32, LARGE_EXTENT],
            )
        } else if self.identity.is_some() {
            ([-64, -16, -64], [64, 32, 64])
        } else {
            (EDIT_MIN, EDIT_MAX)
        }
    }
    pub fn in_bounds(&self, p: BlockPos) -> bool {
        let (min, max) = self.bounds();
        (0..3).all(|a| p.0[a] >= min[a] && p.0[a] < max[a])
    }
    /// Outside is known Air. Inside a streaming volume missing means pending,
    /// distinct from a resident all-Air chunk.
    pub fn ready(&self, p: BlockPos) -> bool {
        self.identity.is_none() || !self.in_bounds(p) || self.chunks.contains_key(&p.split().0)
    }
    pub fn edits(&self) -> Arc<BTreeMap<BlockPos, Block>> {
        assert!(
            self.regional.is_none(),
            "use overlay_snapshot for regional worlds"
        );
        Arc::clone(&self.edits)
    }
    pub(crate) fn restore_edit(&mut self, p: BlockPos, block: Block) {
        // Validated persistence import; does not materialize chunks or dirty state.
        Arc::make_mut(&mut self.edits).insert(p, block);
    }
    pub(crate) fn apply_cached_edit(&mut self, pos: BlockPos, block: Block) {
        let (cp, lp) = pos.split();
        if let Some(chunk) = self.chunks.get_mut(&cp) {
            chunk.set(lp, block);
        }
    }
    pub fn evict_column(&mut self, column: [i32; 2]) {
        assert!(
            self.identity.is_some(),
            "legacy snapshots must not be evicted"
        );
        self.chunks.retain(|p, _| [p.0[0], p.0[2]] != column);
    }
    pub fn save_snapshot(&self) -> Self {
        if self.identity.is_none() {
            return self.clone();
        }
        Self {
            chunks: BTreeMap::new(),
            revision: self.revision,
            mesh_revisions: BTreeMap::new(),
            identity: self.identity,
            regional: self.regional,
            regions: self
                .regions
                .iter()
                .filter(|(_, r)| r.dirty)
                .map(|(&p, r)| (p, r.clone()))
                .collect(),
            edits: Arc::clone(&self.edits),
        }
    }
    /// Process-local change generation; never part of the save format.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Streamed-world mesh dependency token. Unrelated edits and residency do
    /// not invalidate a worker; save revisions remain global and unchanged.
    pub fn mesh_revision(&self, position: ChunkPos) -> u64 {
        self.mesh_revisions.get(&position).copied().unwrap_or(0)
    }

    /// Raw bounded mutation for trusted world-edit operations. Placement rules
    /// (solid replacement/player overlap) live in interaction::edit.
    pub fn set_block(
        &mut self,
        position: BlockPos,
        block: Block,
    ) -> Result<Option<BlockChange>, EditError> {
        if !self.in_bounds(position) {
            return Err(EditError::OutOfBounds);
        }
        if !self.ready(position) {
            return Err(EditError::NotLoaded);
        }
        let before = self.block(position);
        if before == block {
            return Ok(None);
        }
        if self.regional.is_some() {
            let r = self
                .regions
                .get(&RegionPos::of(position))
                .ok_or(EditError::NotLoaded)?;
            if !r.dirty && self.dirty_regions() >= MAX_DIRTY_REGIONS {
                return Err(EditError::SaveRequired);
            }
        }
        let (chunk, local) = position.split();
        self.chunks.entry(chunk).or_default().set(local, block);
        if self.regional.is_some() {
            let r = self.regions.get_mut(&RegionPos::of(position)).unwrap();
            Arc::make_mut(&mut r.edits).insert(position, block);
            r.dirty = true;
            r.revision = self.revision.wrapping_add(1);
        } else if self.identity.is_some() {
            Arc::make_mut(&mut self.edits).insert(position, block);
        }
        self.revision = self.revision.wrapping_add(1);
        let change = BlockChange {
            position,
            before,
            after: block,
        };
        if self.identity.is_some() {
            for p in crate::lighting::affected(position)
                .union(&change.affected_chunks())
                .copied()
            {
                self.mesh_revisions.insert(p, self.revision);
            }
        }
        Ok(Some(change))
    }

    /// Trusted import/cache install, not an edit API. Stream workers must apply
    /// the overlay and validate local dependencies before installing this data.
    pub fn insert_chunk(&mut self, position: ChunkPos, chunk: Chunk) {
        if self.chunks.get(&position) != Some(&chunk) {
            self.chunks.insert(position, chunk);
            if self.identity.is_none() {
                self.revision = self.revision.wrapping_add(1);
            }
        }
    }

    pub fn chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&position)
    }

    /// Resident voxel chunks, including empty chunks; constant-time diagnostics.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunks(&self) -> impl Iterator<Item = (ChunkPos, &Chunk)> {
        self.chunks
            .iter()
            .map(|(&position, chunk)| (position, chunk))
    }

    pub fn block(&self, position: BlockPos) -> Block {
        let (chunk, local) = position.split();
        self.chunks.get(&chunk).map_or_else(
            || {
                if self.identity.is_some() && self.in_bounds(position) {
                    Block::Stone
                } else {
                    Block::Air
                }
            },
            |chunk| chunk.get(local),
        )
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
