//! Small fluid registry and event-driven, synchronous local relaxation.
//! Unknown cells are closed boundaries. No terrain generation or IO occurs here.
use crate::{Block, BlockChange, BlockPos, ChunkPos, LocalPos, World};
use std::collections::{BTreeMap, HashSet, VecDeque};

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
/// Hard cap matches the maximum supported resident volume (49 extended columns).
/// A bit/set entry is never retained for an unknown or evicted cell.
pub const MAX_PENDING: usize = 49
    * ((crate::bounds::WorldHeight::EXTENDED.max - crate::bounds::WorldHeight::EXTENDED.min)
        as usize
        / 16)
    * 4096;
pub const NEIGHBORS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [-1, 0, 0],
    [1, 0, 0],
    [0, 0, -1],
    [0, 0, 1],
];
const SIDES: [[i32; 3]; 4] = [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]];
/// Four horizontal steps, additionally limited by the feeder's remaining levels.
/// A radius-four diamond has only 41 cells; scratch stays on the stack.
pub const ROUTE_DISTANCE: i32 = 4;
const ROUTE_WIDTH: usize = (2 * ROUTE_DISTANCE + 1) as usize;
const ROUTE_CELLS: usize = 1 + 2 * (ROUTE_DISTANCE * (ROUTE_DISTANCE + 1)) as usize;
// Low four bits are directions, bit four means computed (including mask zero).
// Lazily allocated only for queried resident chunks: at most 49 * crate::bounds::WorldHeight::EXTENDED.chunk_count() * 4096 bytes.
type RouteCache = BTreeMap<ChunkPos, Box<[u8; 4096]>>;
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
    routes: RouteCache,
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
        self.routes.retain(|c, _| [c.0[0], c.0[2]] != column);
        self.queue.retain(|p| {
            let c = p.split().0;
            [c.0[0], c.0[2]] != column
        });
        self.pending.retain(|p| {
            let c = p.split().0;
            [c.0[0], c.0[2]] != column
        });
    }
    pub(crate) fn invalidate_route(&mut self, p: BlockPos) {
        let (cp, lp) = p.split();
        if let Some(chunk) = self.routes.get_mut(&cp) {
            chunk[lp.index()] = 0;
        }
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
    /// Terrain/source edits can change a drop on this plane or the plane above.
    /// Wake wet feeders and their consumers, not an entire resident-world scan.
    pub(crate) fn wake_fluid_routes(&mut self, p: BlockPos) {
        for dy in 0..=1 {
            for dz in -ROUTE_DISTANCE..=ROUTE_DISTANCE {
                for dx in -ROUTE_DISTANCE..=ROUTE_DISTANCE {
                    if dx.abs() + dz.abs() > ROUTE_DISTANCE {
                        continue;
                    }
                    if let Some(n) = p.offset([dx, dy, dz])
                        && self.ready(n)
                        && self.block(n).fluid().is_some()
                    {
                        self.fluids.invalidate_route(n);
                        self.wake_fluid(n);
                    }
                }
            }
        }
    }
    pub(crate) fn wake_installed_fluids(&mut self, cp: ChunkPos) {
        // Only lifecycle events scan a chunk; skip dry chunks in constant time.
        // Neighbor water wakes across both faces even if the installed chunk is dry.
        // Include nearby interior/diagonal water whose bounded route can now see
        // into this chunk. Also used after eviction, with no missing data read.
        for d in (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| [x, y, z])))
        {
            let np = ChunkPos(std::array::from_fn(|a| cp.0[a] + d[a]));
            if self.chunk(np).is_none_or(|c| c.water_cells() == 0) {
                continue;
            }
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let lp = LocalPos::new([x, y, z]).unwrap();
                        let p = np.block_position(lp).unwrap();
                        let distance = |a: usize| {
                            let low = cp.0[a] * 16;
                            (low - p.0[a]).max(p.0[a] - (low + 15)).max(0)
                        };
                        // A dry consumer one cell beyond a wet feeder may have
                        // deferred because its next neighbor was pending. Wake
                        // that feeder too when the missing vertical slab arrives.
                        if distance(1) > 2 || distance(0) + distance(2) > ROUTE_DISTANCE {
                            continue;
                        }
                        let block = self.chunk(np).unwrap().get(lp);
                        let active = if block == Block::WaterSource {
                            // A generated source body is already stable. Sources
                            // cannot flow upward, into solids, or into sources.
                            // Wake only real resident consumers; pending boundaries
                            // will be reconsidered by the next lifecycle event.
                            NEIGHBORS.iter().filter(|d| **d != [0, 1, 0]).any(|&d| {
                                p.offset(d).is_some_and(|n| {
                                    self.in_bounds(n)
                                        && self.ready(n)
                                        && self.block(n) != Block::WaterSource
                                        && self.block(n).fluid_replaceable()
                                })
                            })
                        } else {
                            block.fluid().is_some()
                        };
                        if active {
                            self.fluids.invalidate_route(p);
                            self.wake_fluid(p);
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
        if count == 0 {
            self.fluids.changed = 0;
            return;
        }
        let mut regions = [None; crate::regions::MAX_DIRTY_REGIONS];
        let mut region_count = 0;
        let mut planned_drops = 0;
        let mut planned_records = BTreeMap::<crate::regions::RegionPos, usize>::new();
        let mut routes = std::mem::take(&mut self.fluids.routes);
        for _ in 0..count {
            let p = self.fluids.queue.pop_front().unwrap();
            self.fluids.pending.remove(&p);
            if !self.in_bounds(p) || !self.ready(p) {
                continue;
            }
            let before = self.block(p);
            if !before.fluid_replaceable() {
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
            let desired = desired(self, p, &mut routes);
            // A dry decoration stays intact until actual water arrives.
            let after = if desired == Block::Air && before.fluid_drop().is_some() {
                before
            } else {
                desired
            };
            let region = crate::regions::RegionPos::of(p);
            let drop_allowed = before.fluid_drop().is_none_or(|item| {
                after.fluid().is_none()
                    || (self.drops.can_add(p, item)
                        && (self.drops.contains(p)
                            || self.drops.len() + planned_drops < crate::drops::MAX_DROPS))
            });
            // Count earlier accepted candidates against the shared region budget.
            let allowed = self
                .fluid_record_room(p, planned_records.get(&region).copied().unwrap_or(0))
                && drop_allowed
                && self.fluid_edit_allowed(p)
                && (self.region_identity().is_none()
                    || self.fluid_region_dirty(region)
                    || regions.contains(&Some(region))
                    || self.dirty_regions() + region_count < crate::regions::MAX_DIRTY_REGIONS);
            if before != after && allowed {
                if self.fluid_new_record(p) {
                    *planned_records.entry(region).or_default() += 1;
                }
                if self.region_identity().is_some()
                    && !self.fluid_region_dirty(region)
                    && !regions.contains(&Some(region))
                {
                    regions[region_count] = Some(region);
                    region_count += 1;
                }
                if before.fluid_drop().is_some()
                    && after.fluid().is_some()
                    && !self.drops.contains(p)
                {
                    planned_drops += 1;
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
        // Restore before committing: the batch must invalidate changed levels
        // and geometry in this cache, not an empty temporary map.
        self.fluids.routes = routes;
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
    world.ready(below) && (b.is_collidable() || b == Block::WaterSource)
}
fn desired(world: &World, p: BlockPos, routes: &mut RouteCache) -> Block {
    let above = p.offset([0, 1, 0]).unwrap();
    if world.in_bounds(above) && world.block(above).fluid().is_some() {
        return Block::WaterFalling;
    }
    let mut level = 8;
    for (side, d) in SIDES.into_iter().enumerate() {
        let n = p.offset(d).unwrap();
        if !world.in_bounds(n) || !supported(world, n) {
            continue;
        }
        if let Some(f) = world.block(n).fluid() {
            let candidate = if f.is_falling() { 1 } else { f.state_id() + 1 };
            if candidate > 7 {
                continue;
            }
            let (cp, lp) = n.split();
            let cached = &mut routes.entry(cp).or_insert_with(|| Box::new([0; 4096]))[lp.index()];
            if *cached == 0 {
                *cached = route_directions(world, n, f) | 16;
            }
            if *cached & (1 << (side ^ 1)) != 0 {
                level = level.min(candidate);
            }
        }
    }
    Fluid::horizontal(level).map_or(Block::Air, Fluid::block)
}

/// Geometry (including permanent sources) decides routes, never transient flow
/// occupancy. Otherwise a wave could block its own route and oscillate.
fn route_directions(world: &World, origin: BlockPos, fluid: Fluid) -> u8 {
    let level = if fluid.is_falling() {
        0
    } else {
        fluid.state_id()
    };
    let limit = ROUTE_DISTANCE.min(i32::from(7 - level));
    if limit == 0 {
        return 0;
    }
    let index = |x: i32, z: i32| {
        ((z + ROUTE_DISTANCE) as usize) * ROUTE_WIDTH + (x + ROUTE_DISTANCE) as usize
    };
    let mut distances = [u8::MAX; ROUTE_WIDTH * ROUTE_WIDTH];
    let mut directions = [0u8; ROUTE_WIDTH * ROUTE_WIDTH];
    let mut drops = [false; ROUTE_WIDTH * ROUTE_WIDTH];
    let mut queue = [[0i32; 2]; ROUTE_CELLS];
    distances[index(0, 0)] = 0;
    let (mut head, mut tail) = (0, 1);
    let mut best = u8::MAX;
    let mut selected = 0;
    let mut eligible = 0;
    while head < tail {
        let [x, z] = queue[head];
        head += 1;
        let i = index(x, z);
        let distance = distances[i];
        if distance > best {
            break;
        }
        if distance != 0 && drops[i] {
            best = distance;
            selected |= directions[i];
            continue;
        }
        if i32::from(distance) == limit || distance == best {
            continue;
        }
        for (side, [dx, _, dz]) in SIDES.into_iter().enumerate() {
            let (nx, nz) = (x + dx, z + dz);
            let ni = index(nx, nz);
            if distances[ni] < distance + 1 {
                continue;
            }
            let n = origin.offset([nx, 0, nz]).unwrap();
            let below = n.offset([0, -1, 0]).unwrap();
            if !world.in_bounds(n) || !world.ready(n) || !world.ready(below) {
                distances[ni] = 0; // Closed cells need not be queried from each side.
                continue;
            }
            let block = world.block(n);
            if !block.fluid_replaceable() || block == Block::WaterSource {
                distances[ni] = 0;
                continue;
            }
            let mask = if distance == 0 {
                1 << side
            } else {
                directions[i]
            };
            if distance == 0 {
                eligible |= mask;
            }
            directions[ni] |= mask;
            if distances[ni] == u8::MAX {
                let floor = world.block(below);
                drops[ni] =
                    world.in_bounds(below) && !floor.is_collidable() && floor != Block::WaterSource;
                distances[ni] = distance + 1;
                queue[tail] = [nx, nz];
                tail += 1;
            }
        }
    }
    if selected != 0 { selected } else { eligible }
}

#[cfg(test)]
mod routing_tests;
#[cfg(test)]
mod tests;
