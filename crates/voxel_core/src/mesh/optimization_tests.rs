// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{Block, Chunk, lighting::Skylight};
use std::collections::BTreeSet;

#[test]
fn row_visibility_matches_scalar_halo_for_every_cell_and_face() {
    let (world, cp) = fixture(3, [-1, 0, -1]);
    let halo = ao::Occlusion::new(&world, cp);
    for y in 0..16 {
        for z in 0..16 {
            let masks = halo.visible_row(y, z);
            for x in 0..16 {
                let p = [x, y, z];
                let cube = world
                    .chunk(cp)
                    .unwrap()
                    .get(LocalPos::new(p).unwrap())
                    .is_cube();
                for (f, face) in FACES.iter().enumerate() {
                    assert_eq!(
                        masks[f] & (1 << x) != 0,
                        cube && !halo.solid(ao::Occlusion::index(p), ao::normal_offset(face))
                    );
                }
            }
        }
    }
}

fn fixture(kind: usize, coordinates: [i32; 3]) -> (World, ChunkPos) {
    let cp = ChunkPos(coordinates);
    let mut world = World::default();
    let palette = [
        Block::Air,
        Block::Grass,
        Block::Dirt,
        Block::Stone,
        Block::Wood,
        Block::Leaves,
        Block::Torch,
        Block::TorchEast,
        Block::TorchWest,
        Block::TorchSouth,
        Block::TorchNorth,
        Block::WaterSource,
        Block::Water1,
        Block::Water2,
        Block::Water3,
        Block::Water4,
        Block::Water5,
        Block::Water6,
        Block::Water7,
        Block::WaterFalling,
        Block::Sand,
        Block::SnowyGround,
        Block::BirchWood,
        Block::BirchLeaves,
        Block::SpruceWood,
        Block::SpruceLeaves,
        Block::TallGrass,
        Block::Fern,
        Block::RedFlower,
        Block::YellowFlower,
        Block::DeadBush,
    ];
    for block in palette {
        assert_eq!(block.is_cube(), block.is_opaque());
    }
    let mut random = 0xabc123u32;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = ChunkPos([cp.0[0] + dx, cp.0[1] + dy, cp.0[2] + dz]);
                let mut chunk = Chunk::default();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                            let b = match kind {
                                0 => Block::Air,
                                1 => Block::Stone,
                                2 => {
                                    if (4..12).contains(&x)
                                        && (4..12).contains(&y)
                                        && (4..12).contains(&z)
                                    {
                                        Block::Air
                                    } else {
                                        Block::Stone
                                    }
                                }
                                3 => palette[(random >> 16) as usize % palette.len()],
                                _ => Block::WaterSource,
                            };
                            chunk.set(LocalPos::new([x, y, z]).unwrap(), b);
                        }
                    }
                }
                chunk.compact();
                world.insert_chunk(p, chunk);
            }
        }
    }
    (world, cp)
}

#[test]
fn complete_buffers_equal_frozen_slice32_across_uniform_cavity_mixed_and_signed_seams() {
    for coordinates in [[0, 0, 0], [-1, -1, -1], [1, 1, 1]] {
        for kind in 0..5 {
            let (world, cp) = fixture(kind, coordinates);
            let light = Skylight::for_chunks(&world, &BTreeSet::from([cp]));
            assert_eq!(
                mesh_chunk_lit(&world, cp, &light),
                reference::mesh_chunk_lit(&world, cp, &light),
                "{coordinates:?} kind={kind}"
            );
        }
    }
}

#[test]
fn complete_buffers_equal_reference_with_pending_and_missing_neighbors() {
    for regional in [false, true] {
        let mut world = if regional {
            World::regional(crate::regions::RegionIdentity {
                seed: 42,
                generator: 9,
                nonce: [33; 16],
            })
        } else {
            World::default()
        };
        let cp = ChunkPos([-1, -1, 0]);
        world.insert_chunk(cp, Chunk::filled(Block::Stone));
        let light = Skylight::for_chunks(&world, &BTreeSet::from([cp]));
        assert_eq!(
            mesh_chunk_lit(&world, cp, &light),
            reference::mesh_chunk_lit(&world, cp, &light)
        );
    }
}
