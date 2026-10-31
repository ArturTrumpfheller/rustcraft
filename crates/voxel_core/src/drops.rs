//! Bounded stationary world items, indexed by chunk for local pickup/rendering.
//! Ownership lives in World; entities are disposable presentation only.
use crate::{
    Block, BlockPos, ChunkPos, World,
    inventory::{Inventory, Item, Stack},
};
use std::collections::BTreeMap;

pub const MAX_DROPS: usize = 4096;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DroppedItems {
    chunks: BTreeMap<ChunkPos, BTreeMap<BlockPos, Stack>>,
    len: usize,
}
impl Block {
    /// Explicit decoration policy; ordinary solids are never fluid-replaceable.
    pub fn fluid_drop(self) -> Option<Item> {
        if self.is_torch() {
            Some(Item::Torch)
        } else if self.is_plant() {
            Item::from_block(self)
        } else {
            None
        }
    }
    pub fn fluid_replaceable(self) -> bool {
        self == Self::Air || self.fluid().is_some() || self.fluid_drop().is_some()
    }
}
impl DroppedItems {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = (BlockPos, Stack)> + '_ {
        self.chunks
            .values()
            .flat_map(|c| c.iter().map(|(&p, &s)| (p, s)))
    }
    pub fn in_chunk(&self, p: ChunkPos) -> impl Iterator<Item = (BlockPos, Stack)> + '_ {
        self.chunks
            .get(&p)
            .into_iter()
            .flat_map(|c| c.iter().map(|(&p, &s)| (p, s)))
    }
    pub(crate) fn contains(&self, p: BlockPos) -> bool {
        self.chunks
            .get(&p.split().0)
            .is_some_and(|c| c.contains_key(&p))
    }
    pub(crate) fn can_add(&self, p: BlockPos, item: Item) -> bool {
        match self.chunks.get(&p.split().0).and_then(|c| c.get(&p)) {
            Some(s) => s.item() == item && s.count() < item.max_stack(),
            None => self.len < MAX_DROPS,
        }
    }
    pub(crate) fn add(&mut self, p: BlockPos, stack: Stack) -> bool {
        if self.len >= MAX_DROPS
            || self
                .chunks
                .get(&p.split().0)
                .is_some_and(|c| c.contains_key(&p))
        {
            return false;
        }
        self.chunks.entry(p.split().0).or_default().insert(p, stack);
        self.len += 1;
        true
    }
    pub(crate) fn spawn_one(&mut self, p: BlockPos, item: Item) {
        assert!(self.can_add(p, item));
        let c = self.chunks.entry(p.split().0).or_default();
        if let Some(s) = c.get_mut(&p) {
            *s = Stack::new(item, s.count() + 1).unwrap();
        } else {
            c.insert(p, Stack::new(item, 1).unwrap());
            self.len += 1;
        }
    }
}
impl World {
    pub fn dropped_items(&self) -> &DroppedItems {
        &self.drops
    }
    /// At most 27 indexed chunk lookups. Unknown/unloaded cells cannot be picked up.
    pub fn collect_items(&mut self, feet: [f64; 3], inventory: &mut Inventory) -> u16 {
        if !feet.iter().all(|v| v.is_finite() && v.abs() < 1e9) || self.drops.is_empty() {
            return 0;
        }
        let center = BlockPos(feet.map(|v| v.floor() as i32)).split().0;
        let mut collected = 0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cp = ChunkPos([center.0[0] + dx, center.0[1] + dy, center.0[2] + dz]);
                    if self.chunk(cp).is_none() {
                        continue;
                    }
                    if let Some(c) = self.drops.chunks.get_mut(&cp) {
                        c.retain(|p, s| {
                            let distance = (feet[0] - f64::from(p.0[0]) - 0.5).powi(2)
                                + (feet[1] + 0.8 - f64::from(p.0[1]) - 0.4).powi(2)
                                + (feet[2] - f64::from(p.0[2]) - 0.5).powi(2);
                            if distance > 1.5f64.powi(2) {
                                return true;
                            }
                            let rest = inventory.insert(s.item(), u16::from(s.count()));
                            collected += u16::from(s.count()) - rest;
                            if rest == 0 {
                                self.drops.len -= 1;
                                false
                            } else {
                                *s = Stack::new(s.item(), rest as u8).unwrap();
                                true
                            }
                        });
                        if c.is_empty() {
                            self.drops.chunks.remove(&cp);
                        }
                    }
                }
            }
        }
        if collected != 0 {
            self.revision_items_changed();
        }
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fluid::FluidConfig, lighting::Skylight};
    use std::collections::BTreeSet;
    #[test]
    fn fluid_washes_all_five_torches_relights_and_continues_without_duplicate_drops() {
        for torch in [
            Block::Torch,
            Block::TorchEast,
            Block::TorchWest,
            Block::TorchSouth,
            Block::TorchNorth,
        ] {
            let mut w = World::default();
            let p = BlockPos([4, 2, 4]);
            w.set_block(
                p.offset(torch.torch_support().unwrap()).unwrap(),
                Block::Stone,
            )
            .unwrap();
            w.set_block(p, torch).unwrap();
            let targets = BTreeSet::from([p.split().0]);
            assert_eq!(Skylight::for_chunks(&w, &targets).block_level(p), 14);
            w.set_block(p.offset([0, 1, 0]).unwrap(), Block::WaterSource)
                .unwrap();
            let mut changes = Vec::new();
            let mut removals = 0;
            for _ in 0..200 {
                w.tick_fluids(FluidConfig::default(), &mut changes);
                removals += changes
                    .iter()
                    .filter(|c| c.before == torch && c.position == p)
                    .count();
            }
            assert_eq!(removals, 1);
            assert!(w.block(p).fluid().is_some());
            assert_eq!(w.block(p).emission(), 0);
            assert_eq!(Skylight::for_chunks(&w, &targets).block_level(p), 0);
            assert_eq!(w.drops.len(), 1);
            assert_eq!(w.drops.iter().next().unwrap().1.count(), 1);
            assert!(w.water_cells() > 2);
            assert_eq!(w.fluid_updates().pending(), 0);
        }
    }
    #[test]
    fn full_and_partial_pickups_conserve_ownership_and_ignore_distant_chunks() {
        let mut w = World::default();
        let p = BlockPos([0, 1, 0]);
        w.set_block(p.offset([0, -1, 0]).unwrap(), Block::Stone)
            .unwrap();
        w.set_block(p, Block::Torch).unwrap();
        w.set_block(p, Block::WaterSource).unwrap();
        assert!(
            w.drops
                .add(BlockPos([20, 1, 0]), Stack::new(Item::Torch, 3).unwrap())
        );
        let mut inv = Inventory::empty();
        inv.insert(Item::Stone, 36 * 64);
        assert_eq!(w.collect_items([0.5, 1., 0.5], &mut inv), 0);
        assert_eq!(w.drops.len(), 2);
        let mut slots = [Stack::new(Item::Stone, 64); 36];
        slots[0] = Stack::new(Item::Torch, 63);
        inv = Inventory::from_parts(slots, None, 0).unwrap();
        assert_eq!(w.collect_items([0.5, 1., 0.5], &mut inv), 1);
        assert_eq!(w.drops.len(), 1);
        assert_eq!(w.collect_items([0.5, 1., 0.5], &mut inv), 0);
        assert_eq!(inv.slots()[0].unwrap().count(), 64);
        // No resident chunk at the distant record yet: presentation/pickup wait.
        assert_eq!(w.collect_items([20.5, 1., 0.5], &mut Inventory::empty()), 0);
        w.insert_chunk(ChunkPos([1, 0, 0]), crate::Chunk::default());
        let mut slots = [Stack::new(Item::Stone, 64); 36];
        slots[0] = Stack::new(Item::Torch, 63);
        inv = Inventory::from_parts(slots, None, 0).unwrap();
        assert_eq!(w.collect_items([20.5, 1., 0.5], &mut inv), 1);
        assert_eq!(w.drops.iter().next().unwrap().1.count(), 2);
    }
    #[test]
    fn drop_cap_defers_displacement_and_resumes_when_capacity_is_released() {
        let mut w = World::default();
        for n in 0..MAX_DROPS {
            assert!(w.drops.add(
                BlockPos([n as i32, 1, 20]),
                Stack::new(Item::Torch, 1).unwrap()
            ));
        }
        let p = BlockPos([0, 1, 0]);
        w.set_block(BlockPos([0, 0, 0]), Block::Stone).unwrap();
        w.set_block(p, Block::Torch).unwrap();
        assert_eq!(
            w.set_block(p, Block::WaterSource),
            Err(crate::interaction::EditError::DropCapacity)
        );
        w.set_block(BlockPos([0, 2, 0]), Block::WaterSource)
            .unwrap();
        let mut changes = Vec::new();
        for _ in 0..10 {
            w.tick_fluids(FluidConfig::default(), &mut changes);
        }
        assert_eq!(w.block(p), Block::Torch);
        assert_eq!(w.drops.len(), MAX_DROPS);
        w.insert_chunk(ChunkPos([0, 0, 1]), crate::Chunk::default());
        assert!(w.collect_items([0.5, 1., 20.5], &mut Inventory::empty()) > 0);
        for _ in 0..10 {
            w.tick_fluids(FluidConfig::default(), &mut changes);
        }
        assert!(w.block(p).fluid().is_some());
        assert!(w.drops.len() <= MAX_DROPS);
    }
}
