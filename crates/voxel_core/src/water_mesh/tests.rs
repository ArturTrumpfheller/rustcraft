// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{Block, Chunk, StreamIdentity};

fn compare(world: &World, cp: ChunkPos) {
    let light = Skylight::for_chunks(world, &std::collections::BTreeSet::from([cp]));
    let expected = reference::mesh_water(world, cp, &light);
    assert_eq!(mesh_water(world, cp, &light), expected);
    let cached = light.chunk(cp);
    assert_eq!(mesh_water_cached(world, cp, &cached), expected);
    assert_eq!(reference::mesh_water_cached(world, cp, &cached), expected);
}

#[test]
fn exact_reference_geometry_for_uniform_mixed_pending_and_edited_water() {
    for uniform in [
        Block::Air,
        Block::Stone,
        Block::WaterSource,
        Block::WaterFalling,
    ] {
        let mut world = World::streaming(StreamIdentity {
            seed: 42,
            generator: 9,
        });
        let cp = ChunkPos([-1, 0, -1]);
        for y in -1..=1 {
            for z in -2..=0 {
                for x in -2..=0 {
                    // Leave a diagonal pending to exercise World fallback.
                    if [x, y, z] != [0, 1, 0] {
                        world.insert_mesh_chunk(ChunkPos([x, y, z]), Chunk::filled(uniform));
                    }
                }
            }
        }
        compare(&world, cp);
        let states = [
            Block::Air,
            Block::Stone,
            Block::WaterSource,
            Block::Water1,
            Block::Water3,
            Block::Water7,
            Block::WaterFalling,
            Block::TallGrass,
            Block::Torch,
            Block::Leaves,
        ];
        // Deterministic islands, waterfalls and above-water samples at signed
        // horizontal/vertical seams. This changes the actual input, then checks
        // the unchanged complete positions/normals/UV/colors/indices contract.
        for (p, chunk) in world
            .chunks()
            .map(|(p, c)| (p, c.clone()))
            .collect::<Vec<_>>()
        {
            let mut chunk = chunk;
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let i = x * 17 + z * 23 + y * 31 + p.0[0].unsigned_abs() as usize;
                        if i.is_multiple_of(5) {
                            chunk.set(LocalPos::new([x, y, z]).unwrap(), states[i % states.len()]);
                        }
                    }
                }
            }
            world.insert_mesh_chunk(p, chunk);
        }
        compare(&world, cp);
        for p in [BlockPos([-16, 0, -16]), BlockPos([-1, 15, -1])] {
            for state in [Block::WaterSource, Block::Water7, Block::Air, Block::Stone] {
                let (chunk_pos, local) = p.split();
                let mut chunk = world.chunk(chunk_pos).unwrap().clone();
                chunk.set(local, state);
                world.insert_mesh_chunk(chunk_pos, chunk);
                for cx in 0..=1 {
                    for cz in 0..=1 {
                        assert_eq!(
                            corner_height(&world, p, cx, cz).to_bits(),
                            reference::corner_height(&world, p, cx, cz).to_bits()
                        );
                    }
                }
                compare(&world, cp);
            }
        }
    }
}
