//! Bounded regional overlay cache data, independent of engine and filesystem.
use crate::{Block, BlockPos, ChunkPos};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const LARGE_EXTENT: i32 = 32768;
pub const REGION_SIZE: i32 = 64;
pub const REGION_CHUNKS: i32 = REGION_SIZE / crate::CHUNK_SIZE as i32;
pub const MAX_DIRTY_REGIONS: usize = 8;
pub const MAX_REGION_RECORDS: usize = 64 * 48 * 64;
pub type EditMap = BTreeMap<BlockPos, Block>;
pub type RegionEdits = Arc<EditMap>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionIdentity {
    pub seed: u64,
    pub generator: u32,
    pub nonce: [u8; 16],
}
impl RegionIdentity {
    /// v4 is forever generator 3. v5 explicitly selects a supported local generator.
    pub fn format_version(self) -> u32 {
        if self.generator == 3 { 4 } else { 5 }
    }
    pub fn world_height(self) -> crate::bounds::WorldHeight {
        crate::bounds::WorldHeight::for_generator(self.generator)
            .expect("validated regional identity")
    }
    /// Keep the historical record budget: eight full maps fit the 32 MiB journal.
    /// V6 permits these records anywhere in its taller volume, never truncates.
    pub fn max_region_records(self) -> usize {
        MAX_REGION_RECORDS
    }
    pub fn validate_generator(self) -> Result<(), crate::worldgen::UnsupportedGenerator> {
        match self.generator {
            3..=7 => Ok(()),
            v => Err(crate::worldgen::UnsupportedGenerator(v)),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RegionPos(pub [i32; 2]);
impl RegionPos {
    pub fn of(p: BlockPos) -> Self {
        Self([
            p.0[0].div_euclid(REGION_SIZE),
            p.0[2].div_euclid(REGION_SIZE),
        ])
    }
    pub fn valid(self) -> bool {
        self.0
            .into_iter()
            .all(|v| (-LARGE_EXTENT / REGION_SIZE..LARGE_EXTENT / REGION_SIZE).contains(&v))
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegionData {
    pub edits: RegionEdits,
    pub revision: u64,
    pub dirty: bool,
}
/// Immutable per-region COW snapshot. Legacy v3 still uses its exact one-map
/// overlay; new worlds never clone a map proportional to all explored edits.
#[derive(Clone, Debug, Default)]
pub struct EditSnapshot {
    pub(crate) legacy: RegionEdits,
    pub(crate) regions: BTreeMap<RegionPos, RegionEdits>,
}
impl From<RegionEdits> for EditSnapshot {
    fn from(legacy: RegionEdits) -> Self {
        Self {
            legacy,
            ..Self::default()
        }
    }
}
impl EditSnapshot {
    pub fn get(&self, p: &BlockPos) -> Option<&Block> {
        self.legacy
            .get(p)
            .or_else(|| self.regions.get(&RegionPos::of(*p)).and_then(|m| m.get(p)))
    }
    pub fn range_x(
        &self,
        min: BlockPos,
        max: BlockPos,
    ) -> impl Iterator<Item = (&BlockPos, &Block)> {
        self.legacy
            .range(min..max)
            .chain(self.regions.values().flat_map(move |m| m.range(min..max)))
    }
    pub fn contains_region(&self, p: RegionPos) -> bool {
        self.regions.contains_key(&p)
    }
    pub fn insert_region(&mut self, p: RegionPos, edits: RegionEdits) {
        self.regions.insert(p, edits);
    }
    pub fn retain_regions(&mut self, needed: &BTreeSet<RegionPos>) {
        self.regions.retain(|p, _| needed.contains(p));
    }
    pub fn regions(&self) -> impl Iterator<Item = (RegionPos, RegionEdits)> + '_ {
        self.regions.iter().map(|(&p, m)| (p, Arc::clone(m)))
    }
}
/// Regions containing targets or any face neighbor. The set is independent of
/// total world area and also covers pending faces used by cached remeshing.
pub fn needed_regions(targets: impl IntoIterator<Item = ChunkPos>) -> BTreeSet<RegionPos> {
    let mut result = BTreeSet::new();
    for p in targets {
        if !(-LARGE_EXTENT / 16..LARGE_EXTENT / 16).contains(&p.0[0])
            || !(-LARGE_EXTENT / 16..LARGE_EXTENT / 16).contains(&p.0[2])
        {
            continue;
        }
        for [dx, dz] in [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] {
            let r = RegionPos([
                (p.0[0] + dx).div_euclid(REGION_CHUNKS),
                (p.0[2] + dz).div_euclid(REGION_CHUNKS),
            ]);
            if r.valid() {
                result.insert(r);
            }
        }
    }
    result
}
