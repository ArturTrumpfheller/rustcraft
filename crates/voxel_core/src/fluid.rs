//! Small fluid registry and event-driven, synchronous local relaxation.
//! Unknown cells are closed boundaries. No terrain generation or IO occurs here.
use crate::{Block, BlockChange, BlockPos, ChunkPos, LocalPos, World};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FluidKind {
    Water,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fluid {
    pub kind: FluidKind,
    state: u8,
}
impl Fluid {
    pub const SOURCE: Self = Self {
        kind: FluidKind::Water,
        state: 0,
    };
    pub const FALLING: Self = Self {
        kind: FluidKind::Water,
        state: 8,
    };
    pub const fn horizontal(level: u8) -> Option<Self> {
        if level >= 1 && level <= 7 {
            Some(Self {
                kind: FluidKind::Water,
                state: level,
            })
        } else {
            None
        }
    }
    pub const fn from_state_id(state: u8) -> Option<Self> {
        if state <= 8 {
            Some(Self {
                kind: FluidKind::Water,
                state,
            })
        } else {
            None
        }
    }
    pub const fn state_id(self) -> u8 {
        self.state
    }
    pub const fn is_source(self) -> bool {
        self.state == 0
    }
    pub const fn is_falling(self) -> bool {
        self.state == 8
    }
    pub fn height(self) -> f32 {
        if self.is_falling() {
            1.
        } else {
            (8 - self.state) as f32 / 9.
        }
    }
    pub fn block(self) -> Block {
        match self.state {
            0 => Block::WaterSource,
            1 => Block::Water1,
            2 => Block::Water2,
            3 => Block::Water3,
            4 => Block::Water4,
            5 => Block::Water5,
            6 => Block::Water6,
            7 => Block::Water7,
            8 => Block::WaterFalling,
            _ => unreachable!("validated fluid state"),
        }
    }
}
impl Block {
    pub fn fluid(self) -> Option<Fluid> {
        let state = match self {
            Self::WaterSource => 0,
            Self::Water1 => 1,
            Self::Water2 => 2,
            Self::Water3 => 3,
            Self::Water4 => 4,
            Self::Water5 => 5,
            Self::Water6 => 6,
            Self::Water7 => 7,
            Self::WaterFalling => 8,
            _ => return None,
        };
        Fluid::from_state_id(state)
    }
}
/// Hard cap matches maximum supported radius (147 resident chunks).
/// A bit/set entry is never retained for an unknown or evicted cell.
pub const MAX_PENDING: usize = 147 * 4096;
pub const NEIGHBORS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [-1, 0, 0],
    [1, 0, 0],
    [0, 0, -1],
    [0, 0, 1],
];
const SIDES: [[i32; 3]; 4] = [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]];
#[derive(Clone, Copy, Debug)]
pub struct FluidConfig {
    pub budget: usize,
}
impl Default for FluidConfig {
    fn default() -> Self {
        Self { budget: 256 }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FluidUpdates {
    queue: VecDeque<BlockPos>,
    pending: HashSet<BlockPos>,
    pub processed: usize,
    pub changed: usize,
    pub total_processed: u64,
}
impl FluidUpdates {
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
    fn enqueue(&mut self, p: BlockPos) {
        // Supported resident volumes fit the cap; never silently drop an event.
        if self.pending.insert(p) {
            assert!(
                self.queue.len() < MAX_PENDING,
                "unsupported resident fluid volume"
            );
            self.queue.push_back(p);
        }
    }
    pub(crate) fn evict(&mut self, column: [i32; 2]) {
        self.queue.retain(|p| {
            let c = p.split().0;
            [c.0[0], c.0[2]] != column
        });
        self.pending.retain(|p| {
            let c = p.split().0;
            [c.0[0], c.0[2]] != column
        });
    }
}
impl World {
    pub fn fluid_updates(&self) -> &FluidUpdates {
        &self.fluids
    }
    pub fn water_cells(&self) -> usize {
        self.chunks().map(|(_, c)| c.water_cells()).sum()
    }
    /// Wake one cell and its six dependencies. Missing data waits for install.
    pub fn wake_fluid(&mut self, p: BlockPos) {
        for d in [[0; 3]].into_iter().chain(NEIGHBORS) {
            if let Some(n) = p.offset(d).filter(|n| self.in_bounds(*n) && self.ready(*n)) {
                self.fluids.enqueue(n);
            }
        }
    }
    pub(crate) fn wake_installed_fluids(&mut self, cp: ChunkPos) {
        // Only lifecycle events scan a chunk; skip dry chunks in constant time.
        // Neighbor water wakes across both faces even if the installed chunk is dry.
        for d in [[0; 3]].into_iter().chain(NEIGHBORS) {
            let np = ChunkPos(std::array::from_fn(|a| cp.0[a] + d[a]));
            if self.chunk(np).is_none_or(|c| c.water_cells() == 0) {
                continue;
            }
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let lp = LocalPos::new([x, y, z]).unwrap();
                        if d != [0; 3]
                            && !(0..3)
                                .any(|a| d[a] != 0 && [x, y, z][a] == if d[a] > 0 { 0 } else { 15 })
                        {
                            continue;
                        }
                        if self.chunk(np).unwrap().get(lp).fluid().is_some() {
                            self.wake_fluid(np.block_position(lp).unwrap());
                        }
                    }
                }
            }
        }
    }
    /// One controlled fluid tick. Read the same pre-tick world for all candidates,
    /// then commit one batch. Callers retain/reuse `changes` and invalidate meshes once.
    pub fn tick_fluids(&mut self, config: FluidConfig, changes: &mut Vec<BlockChange>) {
        changes.clear();
        let count = self.fluids.queue.len().min(config.budget);
        self.fluids.processed = count;
        self.fluids.total_processed += count as u64;
        let mut regions = [None; crate::regions::MAX_DIRTY_REGIONS];
        let mut region_count = 0;
        for _ in 0..count {
            let p = self.fluids.queue.pop_front().unwrap();
            self.fluids.pending.remove(&p);
            if !self.in_bounds(p) || !self.ready(p) {
                continue;
            }
            let before = self.block(p);
            if before != Block::Air && before.fluid().is_none() {
                continue;
            }
            // Sources are permanent; no relaxation can manufacture a new one.
            if before == Block::WaterSource {
                continue;
            }
            // Do not retract a boundary cell using incomplete neighbor data.
            // Reconcile when its neighbor is installed; no perpetual retry loop.
            if NEIGHBORS
                .iter()
                .filter_map(|&d| p.offset(d))
                .any(|n| self.in_bounds(n) && !self.ready(n))
            {
                continue;
            }
            let after = desired(self, p);
            let region = crate::regions::RegionPos::of(p);
            let allowed = self.fluid_edit_allowed(p)
                && (self.region_identity().is_none()
                    || self.fluid_region_dirty(region)
                    || regions.contains(&Some(region))
                    || self.dirty_regions() + region_count < crate::regions::MAX_DIRTY_REGIONS);
            if before != after && allowed {
                if self.region_identity().is_some()
                    && !self.fluid_region_dirty(region)
                    && !regions.contains(&Some(region))
                {
                    regions[region_count] = Some(region);
                    region_count += 1;
                }
                changes.push(BlockChange {
                    position: p,
                    before,
                    after,
                });
            } else if before != after {
                // Save backpressure retains a bounded request, retried next tick.
                self.fluids.enqueue(p);
            }
        }
        self.fluids.changed = changes.len();
        if !changes.is_empty() {
            self.apply_changes(changes);
        }
    }
}
fn supported(world: &World, p: BlockPos) -> bool {
    let below = p.offset([0, -1, 0]).unwrap();
    if !world.in_bounds(below) {
        return true;
    }
    let b = world.block(below);
    world.ready(below) && (b.is_collidable() || b.is_torch() || b == Block::WaterSource)
}
fn desired(world: &World, p: BlockPos) -> Block {
    let above = p.offset([0, 1, 0]).unwrap();
    if world.in_bounds(above) && world.block(above).fluid().is_some() {
        return Block::WaterFalling;
    }
    let mut level = 8;
    for d in SIDES {
        let n = p.offset(d).unwrap();
        if !world.in_bounds(n) || !supported(world, n) {
            continue;
        }
        if let Some(f) = world.block(n).fluid() {
            let candidate = if f.is_falling() { 1 } else { f.state_id() + 1 };
            level = level.min(candidate);
        }
    }
    Fluid::horizontal(level).map_or(Block::Air, Fluid::block)
}

#[cfg(test)]
mod tests;
