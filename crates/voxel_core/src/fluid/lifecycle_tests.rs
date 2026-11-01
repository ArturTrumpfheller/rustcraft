// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{Chunk, StreamIdentity};

#[test]
fn clipped_lifecycle_matches_original_queue_order_and_route_invalidation() {
    for streamed in [false, true] {
        for base in [Block::Air, Block::WaterSource, Block::Stone] {
            let mut world = if streamed {
                World::streaming(StreamIdentity {
                    seed: 42,
                    generator: 9,
                })
            } else {
                World::default()
            };
            for y in -1..=1 {
                for z in -2..=0 {
                    for x in -2..=0 {
                        let cp = ChunkPos([x, y, z]);
                        if [x, y, z] == [0, 1, 0] {
                            continue;
                        }
                        let mut chunk = Chunk::filled(base);
                        for ly in 0..16 {
                            for lz in 0..16 {
                                for lx in 0..16 {
                                    let i = lx * 13 + ly * 17 + lz * 31;
                                    if i % 13 == 0 {
                                        let b = [
                                            Block::WaterSource,
                                            Block::Water1,
                                            Block::Water7,
                                            Block::WaterFalling,
                                            Block::Torch,
                                            Block::Fern,
                                            Block::Air,
                                            Block::Stone,
                                        ][i % 8];
                                        chunk.set(LocalPos::new([lx, ly, lz]).unwrap(), b);
                                    }
                                }
                            }
                        }
                        world.insert_mesh_chunk(cp, chunk);
                        world.fluids.routes.insert(cp, Box::new([31; 4096]));
                    }
                }
            }
            for cp in [
                ChunkPos([-1, 0, -1]),
                ChunkPos([0, 1, 0]),
                ChunkPos([-2, -1, -2]),
            ] {
                let mut actual = world.clone();
                let mut expected = world.clone();
                actual.wake_installed_fluids(cp);
                expected.wake_installed_fluids_reference(cp);
                assert_eq!(actual.fluids, expected.fluids, "{streamed}/{base:?}/{cp:?}");
                // Retrying a lifecycle event must preserve deduplication/order.
                actual.wake_installed_fluids(cp);
                expected.wake_installed_fluids_reference(cp);
                assert_eq!(actual.fluids, expected.fluids);
            }
        }
    }
}

#[test]
fn uniform_source_body_stays_quiet_with_pending_neighbors() {
    let mut world = World::streaming(StreamIdentity {
        seed: 0,
        generator: 9,
    });
    for y in -1..=1 {
        for z in -1..=1 {
            for x in -1..=1 {
                world.insert_chunk(ChunkPos([x, y, z]), Chunk::filled(Block::WaterSource));
            }
        }
    }
    assert_eq!(world.fluid_updates().pending(), 0);
    assert_eq!(world.allocated_voxel_bytes(), 0);
}
