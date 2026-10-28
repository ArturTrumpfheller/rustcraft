// SPDX-License-Identifier: MIT OR Apache-2.0
use voxel_core::{Block, BlockPos, Chunk, ChunkPos, LocalPos, World};
pub fn fixture(scene: &str) -> World {
    let mut world = World::default();
    for cz in -3..=3 {
        for cx in -3..=3 {
            for cy in -1..=1 {
                let cp = ChunkPos([cx, cy, cz]);
                let mut chunk = Chunk::default();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let lp = LocalPos::new([x, y, z]).unwrap();
                            let [wx, wy, wz] = cp.block_position(lp).unwrap().0;
                            let b = if wy < 0 {
                                Block::Dirt
                            } else if wy == 0 {
                                Block::Grass
                            } else if (scene == "pool"
                                && wy <= 2
                                && ((wx.abs() == 2 && wz.abs() <= 2)
                                    || (wz.abs() == 2 && wx.abs() <= 2)))
                                || (scene == "waterfall" && wy <= 8 && wx == 0 && wz == 0)
                                || (scene == "channel"
                                    && (0..=9).contains(&wx)
                                    && wz == 0
                                    && wy == 1)
                            {
                                Block::Stone
                            } else {
                                Block::Air
                            };
                            chunk.set(lp, b);
                        }
                    }
                }
                world.insert_chunk(cp, chunk);
            }
        }
    }
    if scene == "channel" {
        for x in 1..=10 {
            world.set_block(BlockPos([x, 0, 2]), Block::Air).unwrap();
            world.set_block(BlockPos([x, -1, 2]), Block::Air).unwrap();
        }
    }
    for p in [[-3, 1, -3], [5, 1, -3], [10, 1, 4]] {
        world.set_block(BlockPos(p), Block::Torch).unwrap();
    }
    world
}
// SLICE22_BEGIN
pub fn source_positions(scene: &str) -> &'static [[i32; 3]] {
    match scene {
        "dry" => &[],
        "pool" => &[[0, 1, 0]],
        "waterfall" => &[[0, 9, 0]],
        "several" => &[[-5, 1, -5], [5, 1, -5], [-5, 1, 5], [5, 1, 5]],
        "boundary" => &[[15, 1, 0]],
        "channel" => &[[0, 1, 2]],
        _ => &[[0, 1, 0]],
    }
}
pub fn add_water(world: &mut World, scene: &str) {
    for &p in source_positions(scene) {
        world.set_block(BlockPos(p), Block::WaterSource).unwrap();
    }
}
// SLICE22_END
