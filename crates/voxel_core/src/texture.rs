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
    pub const fn uv(self, local: [u8; 2]) -> [f32; 2] {
        let i = self as u32;
        [
            ((i % 4) * TILE_PITCH + TILE_PADDING + local[0] as u32 * TILE_SIZE) as f32
                / ATLAS_SIZE[0] as f32,
            ((i / 4) * TILE_PITCH + TILE_PADDING + local[1] as u32 * TILE_SIZE) as f32
                / ATLAS_SIZE[1] as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_faces_select_distinct_top_side_bottom_without_storage_ids() {
        for n in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            assert_eq!(face_tile(Block::Grass, n), Some(Tile::GrassSide));
            assert_eq!(face_tile(Block::Wood, n), Some(Tile::Bark));
        }
        assert_eq!(face_tile(Block::Grass, [0, 1, 0]), Some(Tile::GrassTop));
        assert_eq!(face_tile(Block::Grass, [0, -1, 0]), Some(Tile::Dirt));
        for n in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            assert_eq!(face_tile(Block::Air, n), None);
            assert_eq!(face_tile(Block::Dirt, n), Some(Tile::Dirt));
            assert_eq!(face_tile(Block::Stone, n), Some(Tile::Stone));
            assert_eq!(face_tile(Block::Leaves, n), Some(Tile::Leaves));
        }
        assert_eq!(face_tile(Block::Wood, [0, 1, 0]), Some(Tile::EndGrain));
        assert_eq!(face_tile(Block::Wood, [0, -1, 0]), Some(Tile::EndGrain));
    }

    #[test]
    fn atlas_corners_cover_exactly_sixteen_texels_with_guard_bands() {
        for (i, tile) in [
            Tile::GrassTop,
            Tile::GrassSide,
            Tile::Dirt,
            Tile::Stone,
            Tile::Bark,
            Tile::EndGrain,
            Tile::Leaves,
        ]
        .into_iter()
        .enumerate()
        {
            let min = tile.uv([0, 0]);
            let max = tile.uv([1, 1]);
            assert_eq!(
                min,
                [
                    ((i % 4) * 32 + 8) as f32 / 128.,
                    ((i / 4) * 32 + 8) as f32 / 64.
                ]
            );
            for a in 0..2 {
                assert!(min[a] > 0. && max[a] < 1.);
                assert_eq!((max[a] - min[a]) * ATLAS_SIZE[a] as f32, 16.);
            }
            assert_eq!(tile.uv([0, 1]), [min[0], max[1]]);
            assert_eq!(tile.uv([1, 0]), [max[0], min[1]]);
        }
    }
}
