// SPDX-License-Identifier: MIT OR Apache-2.0
// Shared deterministic room for screenshots and CPU measurements. No save I/O.
use voxel_core::{Block, BlockPos, World};

pub fn room(scene: &str) -> World {
    let mut w = World::default();
    for x in -20..=20 {
        for y in 0..=8 {
            for z in -8..=8 {
                if x == -20 || x == 20 || y == 0 || y == 8 || z == -8 || z == 8 {
                    w.set_block(
                        BlockPos([x, y, z]),
                        if y == 0 && x == 0 {
                            Block::Wood
                        } else {
                            Block::Stone
                        },
                    )
                    .unwrap();
                }
            }
        }
    }
    if scene == "removed" {
        w.set_block(BlockPos([-2, 1, -3]), Block::Torch).unwrap();
        w.set_block(BlockPos([-2, 1, -3]), Block::Air).unwrap();
    } else if scene != "dark" {
        w.set_block(
            BlockPos([if scene == "seam" { -1 } else { -2 }, 1, -3]),
            Block::Torch,
        )
        .unwrap();
    }
    if scene == "several" || scene == "many" {
        for p in [[6, 1, -3], [-8, 1, 2]] {
            w.set_block(BlockPos(p), Block::Torch).unwrap();
        }
    }
    if scene == "many" {
        for x in (-18..19).step_by(3) {
            for z in (-6..7).step_by(3) {
                w.set_block(BlockPos([x, 1, z]), Block::Torch).unwrap();
            }
        }
    }
    if scene == "mounts" {
        for (p, b, support) in [
            ([-6, 2, -4], Block::TorchEast, [-1, 0, 0]),
            ([-2, 2, -4], Block::TorchWest, [1, 0, 0]),
            ([2, 2, -4], Block::TorchSouth, [0, 0, -1]),
            ([6, 2, -4], Block::TorchNorth, [0, 0, 1]),
        ] {
            let p = BlockPos(p);
            // Both floor and side supports make an equivalent standing-torch
            // fallback possible in the original Slice20 benchmark baseline.
            w.set_block(p.offset([0, -1, 0]).unwrap(), Block::Stone)
                .unwrap();
            w.set_block(p.offset(support).unwrap(), Block::Stone)
                .unwrap();
            w.set_block(p, b).unwrap();
        }
    }
    if scene == "wall" {
        for y in 1..8 {
            for z in -7..8 {
                w.set_block(BlockPos([0, y, z]), Block::Stone).unwrap();
            }
        }
    }
    w
}
