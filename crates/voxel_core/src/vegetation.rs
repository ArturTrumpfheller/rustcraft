//! Static decorative plant policy, shared by generation, gameplay and rendering.
use crate::{Block, shape::SelectionBox};

impl Block {
    pub fn is_plant(self) -> bool {
        matches!(
            self,
            Self::TallGrass | Self::Fern | Self::RedFlower | Self::YellowFlower | Self::DeadBush
        )
    }
    pub fn plant_height(self) -> f32 {
        match self {
            Self::TallGrass => 0.875,
            Self::Fern => 0.625,
            Self::RedFlower => 0.6875,
            Self::YellowFlower => 0.5625,
            Self::DeadBush => 0.75,
            _ => 0.,
        }
    }
    /// Offset to an attachment's support. No multi-block plants or growth state.
    pub fn support_offset(self) -> Option<[i32; 3]> {
        if self.is_plant() {
            Some([0, -1, 0])
        } else {
            self.torch_support()
        }
    }
    pub fn supported_by(self, ground: Self) -> bool {
        match self {
            Self::DeadBush => matches!(ground, Self::Sand | Self::Dirt | Self::Stone),
            Self::Fern => matches!(ground, Self::Grass | Self::Dirt | Self::SnowyGround),
            b if b.is_plant() => matches!(ground, Self::Grass | Self::Dirt),
            _ => ground.is_cube(),
        }
    }
    pub fn plant_selection(self) -> &'static [SelectionBox] {
        const TALL: [SelectionBox; 1] = [SelectionBox {
            min: [0.15, 0., 0.15],
            max: [0.85, 0.875, 0.85],
        }];
        const LOW: [SelectionBox; 1] = [SelectionBox {
            min: [0.2, 0., 0.2],
            max: [0.8, 0.625, 0.8],
        }];
        const FLOWER: [SelectionBox; 1] = [SelectionBox {
            min: [0.25, 0., 0.25],
            max: [0.75, 0.6875, 0.75],
        }];
        const YELLOW: [SelectionBox; 1] = [SelectionBox {
            min: [0.25, 0., 0.25],
            max: [0.75, 0.5625, 0.75],
        }];
        const BUSH: [SelectionBox; 1] = [SelectionBox {
            min: [0.15, 0., 0.15],
            max: [0.85, 0.75, 0.85],
        }];
        match self {
            Self::TallGrass => &TALL,
            Self::Fern => &LOW,
            Self::RedFlower => &FLOWER,
            Self::YellowFlower => &YELLOW,
            Self::DeadBush => &BUSH,
            _ => &[],
        }
    }
}
