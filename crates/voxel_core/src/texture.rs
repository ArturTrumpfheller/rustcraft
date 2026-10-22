//! Terrain art layout only: no engine, image loading, or stored block IDs.
use crate::Block;

pub const TILE_SIZE: u32 = 16;
pub const TILE_PITCH: u32 = 32;
pub const TILE_PADDING: u32 = 8;
pub const ATLAS_SIZE: [u32; 2] = [128, 64];

/// Atlas slots, unrelated to persistence IDs. Row zero is the image's top row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Tile {
    GrassTop,
    GrassSide,
    Dirt,
    Stone,
    Bark,
    EndGrain,
    Leaves,
}

pub fn face_tile(block: Block, normal: [i32; 3]) -> Option<Tile> {
    Some(match block {
        Block::Air => return None,
        Block::Grass if normal == [0, 1, 0] => Tile::GrassTop,
        Block::Grass if normal == [0, -1, 0] => Tile::Dirt,
        Block::Grass => Tile::GrassSide,
        Block::Dirt => Tile::Dirt,
        Block::Stone => Tile::Stone,
        Block::Wood if normal[1] != 0 => Tile::EndGrain,
        Block::Wood => Tile::Bark,
        Block::Leaves => Tile::Leaves,
    })
}

impl Tile {
    /// Full 16-texel face, not a half-texel inset (which would stretch edge pixels).
    /// Eight extruded texels surround it; nearest LOD0 sampling cannot mix tiles.
    pub fn uv(self, local: [u8; 2]) -> [f32; 2] {
        let i = self as u32;
        let start = [
            (i % 4) * TILE_PITCH + TILE_PADDING,
            (i / 4) * TILE_PITCH + TILE_PADDING,
        ];
        std::array::from_fn(|a| {
            (start[a] + u32::from(local[a]) * TILE_SIZE) as f32 / ATLAS_SIZE[a] as f32
        })
    }
}

/// Image V points down. Bark/grass sides stay upright on all four walls.
/// Top: U=+X,V=+Z; bottom: U=+X,V=-Z. Faces remain right-handed outside.
pub(crate) fn face_uv(normal: [i32; 3], corner: [u8; 3]) -> [u8; 2] {
    let [x, y, z] = corner;
    match normal {
        [1, 0, 0] => [1 - z, 1 - y],
        [-1, 0, 0] => [z, 1 - y],
        [0, 1, 0] => [x, z],
        [0, -1, 0] => [x, 1 - z],
        [0, 0, 1] => [x, 1 - y],
        [0, 0, -1] => [1 - x, 1 - y],
        _ => unreachable!("axis-aligned face"),
    }
}
