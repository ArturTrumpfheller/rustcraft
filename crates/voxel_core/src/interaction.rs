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
        }
    }
    pub fn block(self) -> Block {
        match self {
            Self::Grass => Block::Grass,
            Self::Dirt => Block::Dirt,
            Self::Stone => Block::Stone,
            Self::Wood => Block::Wood,
            Self::Leaves => Block::Leaves,
            Self::Torch => Block::Torch,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Dirt => "Dirt",
            Self::Stone => "Stone",
            Self::Wood => "Wood",
            Self::Leaves => "Leaves",
            Self::Torch => "Torch",
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
    InventoryFull,
    EmptySlot,
    Occupied,
    IntersectsPlayer,
    NoPlacementFace,
    Unsupported,
    SupportsTorch,
    StaleEdit,
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
            let position = hit.adjacent.ok_or(EditError::NoPlacementFace)?;
            if !world.ready(position) {
                return Err(EditError::NotLoaded);
            }
            if world.block(position) != Block::Air {
                return Err(EditError::Occupied);
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
