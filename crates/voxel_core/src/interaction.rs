//! Validated in-memory block edits. Ray hits are recomputed against the current
//! authoritative world; placement never trusts a caller-provided hit record.
use crate::{
    Block, BlockChange, World,
    physics::{PhysicsConfig, Player},
    raycast::{Ray, raycast},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BuildBlock {
    #[default]
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Torch,
    WaterSource,
    Sand,
    SnowyGround,
    BirchWood,
    BirchLeaves,
    SpruceWood,
    SpruceLeaves,
    TallGrass,
    Fern,
    RedFlower,
    YellowFlower,
    DeadBush,
}

impl BuildBlock {
    pub fn from_slot(slot: u8) -> Option<Self> {
        match slot {
            1 => Some(Self::Grass),
            2 => Some(Self::Dirt),
            3 => Some(Self::Stone),
            4 => Some(Self::Wood),
            5 => Some(Self::Leaves),
            6 => Some(Self::Torch),
            7 => Some(Self::WaterSource),
            10 => Some(Self::Sand),
            11 => Some(Self::SnowyGround),
            12 => Some(Self::BirchWood),
            13 => Some(Self::BirchLeaves),
            14 => Some(Self::SpruceWood),
            15 => Some(Self::SpruceLeaves),
            16 => Some(Self::TallGrass),
            17 => Some(Self::Fern),
            18 => Some(Self::RedFlower),
            19 => Some(Self::YellowFlower),
            20 => Some(Self::DeadBush),

            _ => None,
        }
    }
    pub fn slot(self) -> u8 {
        match self {
            Self::Grass => 1,
            Self::Dirt => 2,
            Self::Stone => 3,
            Self::Wood => 4,
            Self::Leaves => 5,
            Self::Torch => 6,
            Self::WaterSource => 7,
            Self::Sand => 10,
            Self::SnowyGround => 11,
            Self::BirchWood => 12,
            Self::BirchLeaves => 13,
            Self::SpruceWood => 14,
            Self::SpruceLeaves => 15,
            Self::TallGrass => 16,
            Self::Fern => 17,
            Self::RedFlower => 18,
            Self::YellowFlower => 19,
            Self::DeadBush => 20,
        }
    }
    pub fn block(self) -> Block {
        match self {
            Self::Grass => Block::Grass,
            Self::Sand => Block::Sand,
            Self::SnowyGround => Block::SnowyGround,
            Self::BirchWood => Block::BirchWood,
            Self::BirchLeaves => Block::BirchLeaves,
            Self::SpruceWood => Block::SpruceWood,
            Self::SpruceLeaves => Block::SpruceLeaves,
            Self::TallGrass => Block::TallGrass,
            Self::Fern => Block::Fern,
            Self::RedFlower => Block::RedFlower,
            Self::YellowFlower => Block::YellowFlower,
            Self::DeadBush => Block::DeadBush,

            Self::Dirt => Block::Dirt,
            Self::Stone => Block::Stone,
            Self::Wood => Block::Wood,
            Self::Leaves => Block::Leaves,
            Self::Torch => Block::Torch,
            Self::WaterSource => Block::WaterSource,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Sand => "Sand",
            Self::SnowyGround => "Snowy Ground",
            Self::BirchWood => "Birch Wood",
            Self::BirchLeaves => "Birch Leaves",
            Self::SpruceWood => "Spruce Wood",
            Self::SpruceLeaves => "Spruce Leaves",
            Self::TallGrass => "Tall Grass",
            Self::Fern => "Fern",
            Self::RedFlower => "Red Flower",
            Self::YellowFlower => "Yellow Flower",
            Self::DeadBush => "Dead Bush",

            Self::Dirt => "Dirt",
            Self::Stone => "Stone",
            Self::Wood => "Wood",
            Self::Leaves => "Leaves",
            Self::Torch => "Torch",
            Self::WaterSource => "Water Source",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EditAction {
    Break,
    Place(BuildBlock),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditError {
    OutOfBounds,
    NotLoaded,
    SaveRequired,
    RegionCapacity,
    InventoryFull,
    EmptySlot,
    Occupied,
    IntersectsPlayer,
    NoPlacementFace,
    Unsupported,
    SupportsTorch,
    SupportsPlant,
    StaleEdit,
    DropCapacity,
}

/// Single-cell primitive; inventory-backed gameplay uses `inventory::edit_inventory`
/// for support-removal batches. Empty/no-target clicks return Ok(None); rejected operations leave the world
/// untouched. The caller supplies the displayed eye ray and authoritative body.
pub fn edit(
    world: &mut World,
    ray: Ray,
    action: EditAction,
    player: &Player,
    config: &PhysicsConfig,
) -> Result<Option<BlockChange>, EditError> {
    let Some(hit) = raycast(world, ray) else {
        return Ok(None);
    };
    if !world.ready(hit.position) {
        return Err(EditError::NotLoaded);
    }
    match action {
        EditAction::Break => world.set_block(hit.position, Block::Air),
        EditAction::Place(selection) => {
            let position = placement_position(world, hit).ok_or(EditError::NoPlacementFace)?;
            if !world.ready(position) {
                return Err(EditError::NotLoaded);
            }
            if world.block(position) != Block::Air && world.block(position).fluid().is_none() {
                return Err(EditError::Occupied);
            }
            if selection.block().is_plant() && world.block(position).fluid().is_some() {
                return Err(EditError::Unsupported);
            }
            if selection.block().is_collidable() && player.intersects_block(position, config) {
                return Err(EditError::IntersectsPlayer);
            }
            let block = if selection == BuildBlock::Torch {
                Block::torch_on_face(hit.normal).ok_or(EditError::Unsupported)?
            } else {
                selection.block()
            };
            world.set_block(position, block)
        }
    }
}

#[cfg(test)]
mod tests;

/// Normal targeting skips water; adjacent placement may displace it.
pub fn placement_position(world: &World, hit: crate::raycast::RayHit) -> Option<crate::BlockPos> {
    if world.block(hit.position).fluid().is_some() {
        Some(hit.position)
    } else {
        hit.adjacent
    }
}
