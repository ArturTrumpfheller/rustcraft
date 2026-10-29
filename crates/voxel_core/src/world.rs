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

/// Bounded authoritative result of one player action: support plus five torches.
/// Fixed stack storage; all changed positions must be invalidated by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditBatch {
    changes: [BlockChange; 6],
    len: u8,
}
impl EditBatch {
    pub fn single(change: BlockChange) -> Self {
        Self {
            changes: [change; 6],
            len: 1,
        }
    }
    pub fn changes(&self) -> &[BlockChange] {
        &self.changes[..usize::from(self.len)]
    }
    fn push(&mut self, change: BlockChange) {
        self.changes[usize::from(self.len)] = change;
        self.len += 1;
    }
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
    pub time: crate::time::WorldTime,
    pub(crate) drops: crate::drops::DroppedItems,
    chunks: BTreeMap<ChunkPos, Chunk>,
    pub(crate) fluids: crate::fluid::FluidUpdates,
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
        identity
            .validate_generator()
            .expect("supported regional generator");
        Self {
            regional: Some(identity),
            identity: Some(StreamIdentity {
                seed: identity.seed,
                generator: identity.generator,
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
    /// Validated full-snapshot import. Gameplay support checks must not depend
    /// on record order (a wall support can sort after its torch).
    pub(crate) fn restore_snapshot_block(&mut self, p: BlockPos, block: Block) {
        debug_assert!(self.identity.is_none() && self.in_bounds(p));
        let (cp, lp) = p.split();
        if self.chunks.entry(cp).or_default().set(lp, block) {
            if block.fluid().is_some() {
                self.wake_fluid(p);
            }
            self.revision = self.revision.wrapping_add(1);
        }
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
        self.fluids.evict(column);
        for y in -1..=1 {
            self.wake_installed_fluids(ChunkPos([column[0], y, column[1]]));
        }
    }
    pub fn save_snapshot(&self) -> Self {
        if self.identity.is_none() {
            return self.clone();
        }
        Self {
            chunks: BTreeMap::new(),
            fluids: Default::default(),
            time: self.time,
            drops: self.drops.clone(),
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
    pub(crate) fn revision_items_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
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
        // Single-cell support policy: reject support removal until its torch
        // is collected. No implicit multi-cell edits or unjournaled item drops.
        if let Some(offset) = block.torch_support() {
            let support = position.offset(offset).ok_or(EditError::Unsupported)?;
            if !self.in_bounds(support) {
                return Err(EditError::Unsupported);
            }
            if !self.ready(support) {
                return Err(EditError::NotLoaded);
            }
            if !self.block(support).is_collidable() {
                return Err(EditError::Unsupported);
            }
        }
        if before.is_collidable()
            && !block.is_collidable()
            && self
                .dependent_torches(position)?
                .iter()
                .any(Option::is_some)
        {
            return Err(EditError::SupportsTorch);
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
        if block.fluid().is_some()
            && before
                .fluid_drop()
                .is_some_and(|item| !self.drops.can_add(position, item))
        {
            return Err(EditError::DropCapacity);
        }
        let change = BlockChange {
            position,
            before,
            after: block,
        };
        self.apply_batch(EditBatch::single(change));
        Ok(Some(change))
    }

    /// Inspect at most five attachments. Known overlays can prove pending
    /// neighbors contain no torch; an actual dependent must be loaded to edit.
    fn dependent_torches(&self, position: BlockPos) -> Result<[Option<BlockPos>; 5], EditError> {
        let mut result = [None; 5];
        let mut count = 0;
        for delta in [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let Some(neighbor) = position.offset(delta).filter(|p| self.in_bounds(*p)) else {
                continue;
            };
            let candidate = if self.ready(neighbor) {
                self.block(neighbor)
            } else if self.regional.is_some() {
                // Frozen generators contain no torches; full overlays are authoritative.
                self.regions
                    .get(&RegionPos::of(neighbor))
                    .ok_or(EditError::NotLoaded)?
                    .edits
                    .get(&neighbor)
                    .copied()
                    .unwrap_or(Block::Air)
            } else {
                self.edits.get(&neighbor).copied().unwrap_or(Block::Air)
            };
            if candidate.torch_support() == Some(delta.map(|v| -v)) {
                result[count] = Some(neighbor);
                count += 1;
            }
        }
        Ok(result)
    }

    /// Preflight a deterministic bounded removal. No authoritative state changes.
    pub(crate) fn plan_break(&self, position: BlockPos) -> Result<Option<EditBatch>, EditError> {
        if !self.in_bounds(position) {
            return Err(EditError::OutOfBounds);
        }
        if !self.ready(position) {
            return Err(EditError::NotLoaded);
        }
        let before = self.block(position);
        if before == Block::Air {
            return Ok(None);
        }
        let mut batch = EditBatch::single(BlockChange {
            position,
            before,
            after: Block::Air,
        });
        if before.is_collidable() {
            for p in self.dependent_torches(position)?.into_iter().flatten() {
                if !self.ready(p) {
                    return Err(EditError::NotLoaded);
                }
                batch.push(BlockChange {
                    position: p,
                    before: self.block(p),
                    after: Block::Air,
                });
            }
        }
        if self.regional.is_some() {
            let mut touched = [None; 6];
            let mut clean = 0;
            for change in batch.changes() {
                let p = RegionPos::of(change.position);
                let region = self.regions.get(&p).ok_or(EditError::NotLoaded)?;
                if !region.dirty && !touched.contains(&Some(p)) {
                    touched[clean] = Some(p);
                    clean += 1;
                }
            }
            if self.dirty_regions() + clean > MAX_DIRTY_REGIONS {
                return Err(EditError::SaveRequired);
            }
        }
        Ok(Some(batch))
    }

    /// Revalidate before a no-fail commit. No callbacks, disk I/O or independent
    /// set_block commits occur between validation and publishing the whole batch.
    pub(crate) fn commit_break(&mut self, batch: EditBatch) -> Result<(), EditError> {
        if self.plan_break(batch.changes()[0].position)? != Some(batch) {
            return Err(EditError::StaleEdit);
        }
        self.apply_batch(batch);
        Ok(())
    }

    fn apply_batch(&mut self, batch: EditBatch) {
        self.apply_changes(batch.changes());
    }

    pub(crate) fn fluid_region_dirty(&self, p: RegionPos) -> bool {
        self.regions.get(&p).is_some_and(|r| r.dirty)
    }

    pub(crate) fn fluid_edit_allowed(&self, p: BlockPos) -> bool {
        self.regional.is_none()
            || self
                .regions
                .get(&RegionPos::of(p))
                .is_some_and(|r| r.dirty || self.dirty_regions() < MAX_DIRTY_REGIONS)
    }

    pub(crate) fn apply_changes(&mut self, changes: &[BlockChange]) {
        let revision = self.revision.wrapping_add(1);
        for change in changes {
            if change.after.fluid().is_some()
                && let Some(item) = change.before.fluid_drop()
            {
                self.drops.spawn_one(change.position, item);
            }
            let (chunk, local) = change.position.split();
            self.chunks
                .entry(chunk)
                .or_default()
                .set(local, change.after);
            if self.regional.is_some() {
                let r = self
                    .regions
                    .get_mut(&RegionPos::of(change.position))
                    .expect("validated region");
                Arc::make_mut(&mut r.edits).insert(change.position, change.after);
                r.dirty = true;
                r.revision = revision;
            } else if self.identity.is_some() {
                Arc::make_mut(&mut self.edits).insert(change.position, change.after);
            }
        }
        for change in changes {
            self.fluids.invalidate_route(change.position);
            if change.before.fluid_replaceable() != change.after.fluid_replaceable()
                || (change.before == Block::WaterSource) != (change.after == Block::WaterSource)
            {
                self.wake_fluid_routes(change.position);
            }
            if change.before.fluid().is_some()
                || change.after.fluid().is_some()
                || crate::fluid::NEIGHBORS
                    .iter()
                    .filter_map(|&d| change.position.offset(d))
                    .any(|p| self.ready(p) && self.block(p).fluid().is_some())
            {
                self.wake_fluid(change.position);
            }
        }
        self.revision = revision;
        if self.identity.is_some() {
            let mut affected = BTreeSet::new();
            for change in changes {
                if change.before.is_opaque() != change.after.is_opaque()
                    || change.before.emission() != change.after.emission()
                {
                    affected.extend(crate::lighting::affected(change.position));
                }
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if let Some(p) = change.position.offset([dx, dy, dz]) {
                                affected.insert(p.split().0);
                            }
                        }
                    }
                }
            }
            for p in affected {
                self.mesh_revisions.insert(p, revision);
            }
        }
    }

    /// Trusted import/cache install, not an edit API. Stream workers must apply
    /// the overlay and validate local dependencies before installing this data.
    pub(crate) fn insert_mesh_chunk(&mut self, position: ChunkPos, chunk: Chunk) {
        self.chunks.insert(position, chunk);
    }

    pub fn insert_chunk(&mut self, position: ChunkPos, chunk: Chunk) {
        if self.chunks.get(&position) != Some(&chunk) {
            self.chunks.insert(position, chunk);
            self.wake_installed_fluids(position);
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
