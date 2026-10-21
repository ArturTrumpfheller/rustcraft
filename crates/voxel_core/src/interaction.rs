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
}

impl BuildBlock {
    pub fn from_slot(slot: u8) -> Option<Self> {
        match slot {
            1 => Some(Self::Grass),
            2 => Some(Self::Dirt),
            3 => Some(Self::Stone),
            4 => Some(Self::Wood),
            5 => Some(Self::Leaves),
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
        }
    }
    pub fn block(self) -> Block {
        match self {
            Self::Grass => Block::Grass,
            Self::Dirt => Block::Dirt,
            Self::Stone => Block::Stone,
            Self::Wood => Block::Wood,
            Self::Leaves => Block::Leaves,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Dirt => "Dirt",
            Self::Stone => "Stone",
            Self::Wood => "Wood",
            Self::Leaves => "Leaves",
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
    Occupied,
    IntersectsPlayer,
    NoPlacementFace,
}

/// Empty/no-target clicks return Ok(None); rejected operations leave the world
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
    match action {
        EditAction::Break => world.set_block(hit.position, Block::Air),
        EditAction::Place(selection) => {
            let position = hit.adjacent.ok_or(EditError::NoPlacementFace)?;
            if world.block(position).is_solid() {
                return Err(EditError::Occupied);
            }
            if player.intersects_block(position, config) {
                return Err(EditError::IntersectsPlayer);
            }
            world.set_block(position, selection.block())
        }
    }
}

#[cfg(test)]
mod tests;
