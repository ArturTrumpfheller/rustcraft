//! Equipped-pose profiles. Motion/bob/swing are shared and contain no item checks.
use bevy::prelude::*;
use voxel_core::inventory::Item;
#[derive(Clone, Copy)]
pub(super) struct Presentation {
    pub position: Vec3,
    pub angles: Vec3,
    pub scale: f32,
    pub origin: Vec3,
    pub pivot: Vec3,
    pub held: bool,
}
impl Presentation {
    pub fn for_item(item: Option<Item>) -> Self {
        match item {
            None => ARM,
            Some(Item::Torch) => SLENDER,
            Some(_) => BLOCK,
        }
    }
}
pub(super) const BLOCK: Presentation = Presentation {
    position: Vec3::new(0.56, -0.52, -0.72),
    angles: Vec3::new(0., 45., 0.),
    scale: 0.40,
    origin: Vec3::ZERO,
    pivot: Vec3::new(0.56, -0.52, -0.72),
    held: true,
};
pub(super) const SLENDER: Presentation = Presentation {
    position: Vec3::new(0.56, -0.46, -0.72),
    angles: Vec3::new(0., 35., 12.),
    scale: 0.40,
    origin: Vec3::new(0.5, 0.35, 0.5),
    pivot: Vec3::new(0.56, -0.46, -0.72),
    held: true,
};
pub(super) const ARM: Presentation = Presentation {
    position: Vec3::new(0.609, -0.641, -0.80),
    angles: Vec3::new(-46.5, 19.4, -23.9),
    scale: 1.,
    origin: Vec3::ZERO,
    pivot: Vec3::new(0.65, -0.90, -0.45),
    held: false,
};
