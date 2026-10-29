//! Immutable generator-dependent height contract. Min inclusive, max exclusive.
use crate::worldgen::UnsupportedGenerator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldHeight {
    pub min: i32,
    pub max: i32,
}
impl WorldHeight {
    pub const HISTORICAL: Self = Self { min: -16, max: 32 };
    pub const EXTENDED: Self = Self { min: -64, max: 192 };
    pub fn for_generator(version: u32) -> Result<Self, UnsupportedGenerator> {
        match version {
            1..=5 => Ok(Self::HISTORICAL),
            6 | 7 => Ok(Self::EXTENDED),
            v => Err(UnsupportedGenerator(v)),
        }
    }
    pub fn contains(self, y: i32) -> bool {
        (self.min..self.max).contains(&y)
    }
    pub fn chunks(self) -> std::ops::Range<i32> {
        self.min / crate::CHUNK_SIZE as i32..self.max / crate::CHUNK_SIZE as i32
    }
    pub fn chunk_count(self) -> usize {
        self.chunks().len()
    }
    pub fn recovery_y(self) -> f64 {
        f64::from(self.min - crate::CHUNK_SIZE as i32)
    }
}
