//! Fixed-capacity item transactions; no engine, renderer, or filesystem types.
use crate::{
    Block, EditBatch, World,
    interaction::{BuildBlock, EditAction, EditError, edit},
    physics::{PhysicsConfig, Player},
    raycast::{Ray, raycast},
};
pub const HOTBAR_SLOTS: usize = 9;
pub const MAIN_SLOTS: usize = 27;
pub const SLOT_COUNT: usize = HOTBAR_SLOTS + MAIN_SLOTS;
pub const MAX_STACK: u8 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Torch,
    /// Legacy Slice22 item 7. Inert until explicitly converted in the inventory.
    WaterSource,
    EmptyBucket,
    WaterBucket,
}
impl Item {
    pub fn id(self) -> u16 {
        match self {
            Self::Grass => 1,
            Self::Dirt => 2,
            Self::Stone => 3,
            Self::Wood => 4,
            Self::Leaves => 5,
            Self::Torch => 6,
            Self::WaterSource => 7,
            Self::EmptyBucket => 8,
            Self::WaterBucket => 9,
        }
    }
    pub fn from_id(id: u16) -> Option<Self> {
        Some(match id {
            1 => Self::Grass,
            2 => Self::Dirt,
            3 => Self::Stone,
            4 => Self::Wood,
            5 => Self::Leaves,
            6 => Self::Torch,
            7 => Self::WaterSource,
            8 => Self::EmptyBucket,
            9 => Self::WaterBucket,
            _ => return None,
        })
    }
    pub fn from_block(b: Block) -> Option<Self> {
        Some(match b {
            Block::Air => return None,
            Block::WaterSource => return None,
            Block::Grass => Self::Grass,
            Block::Dirt => Self::Dirt,
            Block::Stone => Self::Stone,
            Block::Wood => Self::Wood,
            Block::Leaves => Self::Leaves,
            Block::Torch
            | Block::TorchEast
            | Block::TorchWest
            | Block::TorchSouth
            | Block::TorchNorth => Self::Torch,
            _ => return None,
        })
    }
    pub const CREATIVE: [Self; 8] = [
        Self::Grass,
        Self::Dirt,
        Self::Stone,
        Self::Wood,
        Self::Leaves,
        Self::Torch,
        Self::EmptyBucket,
        Self::WaterBucket,
    ];
    pub fn max_stack(self) -> u8 {
        match self {
            Self::EmptyBucket => 16,
            Self::WaterBucket => 1,
            _ => MAX_STACK,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyBucket => "Empty Bucket",
            Self::WaterBucket => "Water Bucket",
            Self::WaterSource => "Legacy Water (inventory: CONVERT 7)",
            _ => self.build().unwrap().name(),
        }
    }
    pub fn build(self) -> Option<BuildBlock> {
        Some(match self {
            Self::Grass => BuildBlock::Grass,
            Self::Dirt => BuildBlock::Dirt,
            Self::Stone => BuildBlock::Stone,
            Self::Wood => BuildBlock::Wood,
            Self::Leaves => BuildBlock::Leaves,
            Self::Torch => BuildBlock::Torch,
            Self::WaterSource | Self::EmptyBucket | Self::WaterBucket => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stack {
    item: Item,
    count: u8,
}
impl Stack {
    pub fn new(item: Item, count: u8) -> Option<Self> {
        (1..=item.max_stack())
            .contains(&count)
            .then_some(Self { item, count })
    }
    pub fn item(self) -> Item {
        self.item
    }
    pub fn count(self) -> u8 {
        self.count
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Click {
    Left,
    Right,
    Quick,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inventory {
    slots: [Option<Stack>; SLOT_COUNT],
    cursor: Option<Stack>,
    selected: usize,
    revision: u64,
}
impl Default for Inventory {
    fn default() -> Self {
        Self::empty()
    }
}
impl Inventory {
    pub fn empty() -> Self {
        Self {
            slots: [None; SLOT_COUNT],
            cursor: None,
            selected: 0,
            revision: 0,
        }
    }
    pub fn starter() -> Self {
        let mut s = Self::empty();
        for (i, item) in [
            Item::Grass,
            Item::Dirt,
            Item::Stone,
            Item::Wood,
            Item::Leaves,
            Item::Torch,
            Item::WaterBucket,
            Item::EmptyBucket,
        ]
        .into_iter()
        .enumerate()
        {
            s.slots[i] = Stack::new(item, if i < 3 { 64 } else { item.max_stack().min(32) });
        }
        s
    }
    pub fn from_parts(
        slots: [Option<Stack>; SLOT_COUNT],
        cursor: Option<Stack>,
        selected: usize,
    ) -> Option<Self> {
        (selected < HOTBAR_SLOTS).then_some(Self {
            slots,
            cursor,
            selected,
            revision: 0,
        })
    }
    pub fn slots(&self) -> &[Option<Stack>; SLOT_COUNT] {
        &self.slots
    }
    pub fn cursor(&self) -> Option<Stack> {
        self.cursor
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn selected_stack(&self) -> Option<Stack> {
        self.slots[self.selected]
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn select(&mut self, index: usize) -> bool {
        if index >= HOTBAR_SLOTS {
            return false;
        }
        if self.selected != index {
            self.selected = index;
            self.changed();
        }
        true
    }
    pub fn scroll(&mut self, steps: i32) {
        self.select(
            (self.selected as i64 + i64::from(steps)).rem_euclid(HOTBAR_SLOTS as i64) as usize,
        );
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    // Merge existing stacks before using empty cells, preserving all overflow.
    fn distribute(&mut self, item: Item, mut count: u16, range: std::ops::Range<usize>) -> u16 {
        for i in range.clone() {
            if let Some(s) = self.slots[i].as_mut()
                && s.item == item
            {
                let n = count.min(u16::from(item.max_stack() - s.count));
                s.count += n as u8;
                count -= n;
            }
        }
        for i in range {
            if count == 0 {
                break;
            }
            if self.slots[i].is_none() {
                let n = count.min(u16::from(item.max_stack()));
                self.slots[i] = Stack::new(item, n as u8);
                count -= n;
            }
        }
        count
    }
    pub fn insert(&mut self, item: Item, count: u16) -> u16 {
        let rest = self.distribute(item, count, 0..SLOT_COUNT);
        if rest != count {
            self.changed();
        }
        rest
    }
    /// Add a catalog grant only when the entire stack fits; never overwrite slots.
    pub fn grant(&mut self, item: Item, count: u16) -> bool {
        let mut next = self.clone();
        if next.insert(item, count) != 0 {
            return false;
        }
        *self = next;
        true
    }
    /// Explicit one-for-one conversion; saved ID7 never changes meaning on load.
    pub fn convert_legacy_water(&mut self) -> bool {
        let Some(s) = self
            .selected_stack()
            .filter(|s| s.item == Item::WaterSource)
        else {
            return false;
        };
        let mut next = self.clone();
        next.slots[next.selected] = Stack::new(s.item, s.count - 1);
        if next.insert(Item::WaterBucket, 1) != 0 {
            return false;
        }
        next.changed();
        *self = next;
        true
    }
    fn exchange_selected(&mut self, result: Item) -> Result<(), InventoryEditError> {
        let s = self.selected_stack().ok_or(InventoryEditError::Empty)?;
        self.slots[self.selected] = Stack::new(s.item, s.count - 1);
        // Prefer the selected slot if it was emptied, then ordinary merge-first insertion.
        if self.slots[self.selected].is_none() {
            self.slots[self.selected] = Stack::new(result, 1);
        } else if self.insert(result, 1) != 0 {
            return Err(InventoryEditError::Full);
        }
        self.changed();
        Ok(())
    }
    pub fn click(&mut self, index: usize, click: Click) -> bool {
        if index >= SLOT_COUNT {
            return false;
        }
        let before = (self.slots, self.cursor);
        if click == Click::Quick {
            // A held stack has explicit place/swap semantics; shift never moves it.
            if self.cursor.is_some() {
                return false;
            }
            if let Some(s) = self.slots[index] {
                let range = if index < HOTBAR_SLOTS {
                    HOTBAR_SLOTS..SLOT_COUNT
                } else {
                    0..HOTBAR_SLOTS
                };
                let rest = self.distribute(s.item, u16::from(s.count), range);
                self.slots[index] = Stack::new(s.item, rest as u8);
            }
        } else if click == Click::Left {
            match (self.cursor, self.slots[index]) {
                (Some(mut hand), Some(mut slot)) if hand.item == slot.item => {
                    let n = hand.count.min(hand.item.max_stack() - slot.count);
                    slot.count += n;
                    hand.count -= n;
                    self.slots[index] = Some(slot);
                    self.cursor = Stack::new(hand.item, hand.count);
                }
                _ => std::mem::swap(&mut self.cursor, &mut self.slots[index]),
            }
        } else {
            match (self.cursor, self.slots[index]) {
                (None, Some(s)) => {
                    let n = s.count.div_ceil(2);
                    self.cursor = Stack::new(s.item, n);
                    self.slots[index] = Stack::new(s.item, s.count - n);
                }
                (Some(hand), None) => {
                    self.slots[index] = Stack::new(hand.item, 1);
                    self.cursor = Stack::new(hand.item, hand.count - 1);
                }
                (Some(hand), Some(mut slot))
                    if hand.item == slot.item && slot.count < slot.item.max_stack() =>
                {
                    slot.count += 1;
                    self.slots[index] = Some(slot);
                    self.cursor = Stack::new(hand.item, hand.count - 1);
                }
                _ => {}
            }
        }
        let changed = before != (self.slots, self.cursor);
        if changed {
            self.changed();
        }
        changed
    }
    /// Refuse closure if the held remainder cannot be returned. No world drops.
    pub fn return_cursor(&mut self) -> bool {
        let Some(s) = self.cursor else {
            return true;
        };
        let mut next = self.clone();
        if next.distribute(s.item, u16::from(s.count), 0..SLOT_COUNT) != 0 {
            return false;
        }
        next.cursor = None;
        next.changed();
        *self = next;
        true
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryEditError {
    World(EditError),
    Full,
    Empty,
}
/// Returns every changed cell in a bounded batch for mesh/light invalidation.
/// Both effects commit together in RAM. No inventory mutation is published if
/// validation/world mutation fails. Mesh jobs never collect or consume items.
pub fn edit_inventory(
    world: &mut World,
    inventory: &mut Inventory,
    ray: Ray,
    place: bool,
    player: &Player,
    config: &PhysicsConfig,
) -> Result<Option<EditBatch>, InventoryEditError> {
    let bucket = place
        && inventory
            .selected_stack()
            .is_some_and(|s| matches!(s.item, Item::EmptyBucket | Item::WaterBucket));
    if bucket {
        return use_bucket(world, inventory, ray);
    }
    let Some(hit) = raycast(world, ray) else {
        return Ok(None);
    };
    if !world.ready(hit.position) {
        return Err(InventoryEditError::World(EditError::NotLoaded));
    }
    let mut next = inventory.clone();
    let batch = if place {
        let stack = next.selected_stack().ok_or(InventoryEditError::Empty)?;
        let build = stack.item.build().ok_or(InventoryEditError::Empty)?;
        next.slots[next.selected] = Stack::new(stack.item, stack.count - 1);
        next.changed();
        edit(world, ray, EditAction::Place(build), player, config)
            .map_err(InventoryEditError::World)?
            .map(EditBatch::single)
    } else {
        let Some(batch) = world
            .plan_break(hit.position)
            .map_err(InventoryEditError::World)?
        else {
            return Ok(None);
        };
        for change in batch.changes() {
            let item = Item::from_block(change.before).ok_or(InventoryEditError::Empty)?;
            if next.insert(item, 1) != 0 {
                return Err(InventoryEditError::Full);
            }
        }
        world
            .commit_break(batch)
            .map_err(InventoryEditError::World)?;
        Some(batch)
    };
    if batch.is_some() {
        *inventory = next;
    }
    Ok(batch)
}

/// Inventory conversion is staged before the validated world edit. Every rejection
/// preserves the complete input inventory, including stacked empty buckets.
fn use_bucket(
    world: &mut World,
    inventory: &mut Inventory,
    ray: Ray,
) -> Result<Option<EditBatch>, InventoryEditError> {
    use crate::raycast::{TargetMode, raycast_with};
    let item = inventory
        .selected_stack()
        .ok_or(InventoryEditError::Empty)?
        .item;
    let mode = if item == Item::EmptyBucket {
        TargetMode::CollectSource
    } else {
        TargetMode::Blocks
    };
    let Some(hit) = raycast_with(world, ray, mode) else {
        return Ok(None);
    };
    if !world.ready(hit.position) {
        return Err(InventoryEditError::World(EditError::NotLoaded));
    }
    let (position, block, result) = if item == Item::EmptyBucket {
        if world.block(hit.position) != Block::WaterSource {
            return Ok(None);
        }
        (hit.position, Block::Air, Item::WaterBucket)
    } else {
        let position = if world.block(hit.position).fluid_drop().is_some() {
            hit.position
        } else {
            hit.adjacent
                .ok_or(InventoryEditError::World(EditError::NoPlacementFace))?
        };
        if !world.ready(position) {
            return Err(InventoryEditError::World(EditError::NotLoaded));
        }
        if !world.block(position).fluid_replaceable() || world.block(position) == Block::WaterSource
        {
            return Err(InventoryEditError::World(EditError::Occupied));
        }
        (position, Block::WaterSource, Item::EmptyBucket)
    };
    let mut next = inventory.clone();
    next.exchange_selected(result)?;
    let change = world
        .set_block(position, block)
        .map_err(InventoryEditError::World)?;
    if change.is_some() {
        *inventory = next;
    }
    Ok(change.map(EditBatch::single))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn total(i: &Inventory) -> [u32; 9] {
        let mut n = [0; 9];
        for s in i.slots.iter().copied().chain([i.cursor]).flatten() {
            n[s.item.id() as usize - 1] += u32::from(s.count);
        }
        n
    }
    #[test]
    fn capacity_selection_merge_overflow_and_validation() {
        assert!(Stack::new(Item::Dirt, 0).is_none());
        assert!(Stack::new(Item::Dirt, 65).is_none());
        let mut i = Inventory::empty();
        assert_eq!(i.insert(Item::Dirt, 36 * 64 + 9), 9);
        assert!(i.slots.iter().all(|s| s.unwrap().count() == 64));
        assert!(!i.select(9));
        i.scroll(-1);
        assert_eq!(i.selected(), 8);
        i.scroll(i32::MAX);
        assert!(i.selected() < 9);
        assert!(Inventory::from_parts([None; 36], None, 9).is_none());
    }
    #[test]
    fn pick_split_place_merge_swap_and_quick_transfer() {
        let mut i = Inventory::empty();
        i.insert(Item::Dirt, 5);
        i.click(0, Click::Right);
        assert_eq!(i.cursor().unwrap().count(), 3);
        assert_eq!(i.slots()[0].unwrap().count(), 2);
        i.click(1, Click::Right);
        assert_eq!(i.slots()[1].unwrap().count(), 1);
        i.click(1, Click::Left);
        assert!(i.cursor().is_none());
        assert_eq!(i.slots()[1].unwrap().count(), 3);
        i.click(1, Click::Quick);
        assert_eq!(i.slots()[9].unwrap().count(), 3);
        i.click(0, Click::Left);
        i.insert(Item::Wood, 3);
        i.click(0, Click::Left);
        assert_eq!(i.cursor().unwrap().item(), Item::Wood);
        assert_eq!(i.slots()[0].unwrap().item(), Item::Dirt);
        assert!(i.return_cursor());
        assert_eq!(total(&i), [0, 5, 0, 3, 0, 0, 0, 0, 0]);
    }
    #[test]
    fn randomized_clicks_conserve_every_item_and_keep_stack_bounds() {
        let mut i = Inventory::starter();
        i.insert(Item::Leaves, 1000);
        let expected = total(&i);
        let mut seed = 42u64;
        for _ in 0..20000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let slot = (seed >> 32) as usize % 40;
            let c = match seed % 3 {
                0 => Click::Left,
                1 => Click::Right,
                _ => Click::Quick,
            };
            i.click(slot, c);
            assert_eq!(total(&i), expected);
            assert!(
                i.slots
                    .iter()
                    .copied()
                    .chain([i.cursor])
                    .flatten()
                    .all(|s| (1..=64).contains(&s.count))
            );
        }
        assert!(i.return_cursor());
        assert_eq!(total(&i), expected);
    }
    #[test]
    fn held_stack_and_full_destination_preserve_remainders() {
        let mut slots = [Stack::new(Item::Stone, 64); 36];
        slots[0] = Stack::new(Item::Dirt, 63);
        let mut i = Inventory::from_parts(slots, Stack::new(Item::Dirt, 3), 0).unwrap();
        i.click(0, Click::Left);
        assert_eq!(i.cursor().unwrap().count(), 2);
        let before = i.clone();
        assert!(!i.return_cursor());
        assert_eq!(i, before);
        assert!(!i.click(4, Click::Quick));
        assert_eq!(i, before);
    }
    #[test]
    fn world_and_inventory_commit_together_and_failed_operations_are_atomic() {
        use crate::{BlockPos, Chunk, ChunkPos, LocalPos};
        let mut w = World::default();
        let mut c = Chunk::default();
        c.set(LocalPos::new([0, 1, 0]).unwrap(), Block::Stone);
        w.insert_chunk(ChunkPos([0; 3]), c);
        let ray = Ray {
            origin: [0.5, 1.5, 3.],
            direction: [0., 0., -1.],
            reach: 5.,
        };
        let body = Player::new([8., 1., 8.]);
        let cfg = PhysicsConfig::default();
        let mut inv = Inventory::empty();
        inv.insert(Item::Dirt, 36 * 64);
        let before = inv.clone();
        let revision = w.revision();
        assert_eq!(
            edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg),
            Err(InventoryEditError::Full)
        );
        assert_eq!(inv, before);
        assert_eq!(w.revision(), revision);
        inv = Inventory::empty();
        assert_eq!(
            edit_inventory(&mut w, &mut inv, ray, true, &body, &cfg),
            Err(InventoryEditError::Empty)
        );
        edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg)
            .unwrap()
            .unwrap();
        assert_eq!(w.block(BlockPos([0, 1, 0])), Block::Air);
        assert_eq!(inv.selected_stack().unwrap().item(), Item::Stone);
        assert!(
            edit_inventory(&mut w, &mut inv, ray, true, &body, &cfg)
                .unwrap()
                .is_none()
        );
        assert_eq!(inv.selected_stack().unwrap().count(), 1);
    }
}

#[cfg(test)]
mod transaction_regressions {
    use super::*;
    #[test]
    fn quick_transfer_merges_only_available_capacity_and_retains_the_rest() {
        let mut slots = [Stack::new(Item::Stone, 64); SLOT_COUNT];
        slots[0] = Stack::new(Item::Dirt, 60);
        slots[9] = Stack::new(Item::Dirt, 64);
        let mut i = Inventory::from_parts(slots, None, 0).unwrap();
        i.click(9, Click::Quick);
        assert_eq!(i.slots()[0].unwrap().count(), 64);
        assert_eq!(i.slots()[9].unwrap().count(), 60);
        let before = i.clone();
        assert!(!i.click(9, Click::Quick));
        assert_eq!(i, before);
    }
    #[test]
    fn failed_player_overlap_or_pending_placement_never_consumes_a_stack() {
        use crate::{BlockPos, Chunk, ChunkPos, LocalPos};
        let mut world = World::default();
        let mut c = Chunk::default();
        c.set(LocalPos::new([0, 1, 0]).unwrap(), Block::Stone);
        world.insert_chunk(ChunkPos([0; 3]), c);
        let ray = Ray {
            origin: [0.5, 1.5, 3.],
            direction: [0., 0., -1.],
            reach: 5.,
        };
        let mut inv = Inventory::starter();
        let before = inv.clone();
        let cfg = PhysicsConfig::default();
        assert_eq!(
            edit_inventory(
                &mut world,
                &mut inv,
                ray,
                true,
                &Player::new([0.5, 1., 1.5]),
                &cfg
            ),
            Err(InventoryEditError::World(EditError::IntersectsPlayer))
        );
        assert_eq!(inv, before);
        assert_eq!(world.block(BlockPos([0, 1, 1])), Block::Air);
        let mut pending = crate::streaming::new_large_world(42);
        assert_eq!(
            edit_inventory(
                &mut pending,
                &mut inv,
                ray,
                true,
                &Player::new([8., 1., 8.]),
                &cfg
            ),
            Err(InventoryEditError::World(EditError::NotLoaded))
        );
        assert_eq!(inv, before);
    }
}

#[cfg(test)]
mod torch_transactions {
    use super::*;
    use crate::{BlockPos, Chunk, ChunkPos};
    #[test]
    fn torch_full_inventory_failed_support_and_pending_edits_preserve_everything() {
        let mut w = World::default();
        let p = BlockPos([0, 1, 0]);
        w.set_block(BlockPos([0, 0, 0]), Block::Stone).unwrap();
        w.set_block(p, Block::Torch).unwrap();
        let ray = Ray {
            origin: [0.5, 1.5, 3.],
            direction: [0., 0., -1.],
            reach: 5.,
        };
        let body = Player::new([8., 1., 8.]);
        let cfg = PhysicsConfig::default();
        let mut inv = Inventory::empty();
        inv.insert(Item::Stone, 36 * 64);
        let before = inv.clone();
        let revision = w.revision();
        assert_eq!(
            edit_inventory(&mut w, &mut inv, ray, false, &body, &cfg),
            Err(InventoryEditError::Full)
        );
        assert_eq!(inv, before);
        assert_eq!(w.revision(), revision);
        assert_eq!(w.block(p), Block::Torch);
        let mut pending = crate::streaming::new_world(42);
        pending.insert_chunk(ChunkPos([0, 1, 0]), Chunk::default());
        assert_eq!(
            pending.set_block(BlockPos([0, 16, 0]), Block::Torch),
            Err(EditError::NotLoaded)
        );
        assert_eq!(pending.block(BlockPos([0, 16, 0])), Block::Air);
        assert_eq!(
            pending.set_block(BlockPos([0, 32, 0]), Block::Torch),
            Err(EditError::OutOfBounds)
        );
    }
}
