// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{
    Block, Chunk, StreamIdentity,
    worldgen::{GenerationConfig, Generator},
};

fn compare(world: &World, targets: &BTreeSet<ChunkPos>) {
    let actual = Skylight::for_chunks(world, targets);
    let expected = Skylight::full_reference(world, targets);
    for &cp in targets {
        assert_eq!(actual.chunk(cp), expected.chunk(cp), "{cp:?}");
    }
    // Also compare the full sampled horizontal halo, including trimmed planes.
    for z in (actual.min[1]..actual.min[1] + actual.size[1] as i32).step_by(3) {
        for x in (actual.min[0]..actual.min[0] + actual.size[0] as i32).step_by(3) {
            for y in world.world_height().min - 1..=world.world_height().max {
                let p = BlockPos([x, y, z]);
                assert_eq!(
                    [actual.level(p), actual.block_level(p)],
                    [expected.level(p), expected.block_level(p)],
                    "{p:?}"
                );
            }
        }
    }
}

#[test]
fn descriptor_collector_matches_frozen_full_height_after_edits() {
    for (version, seed, c) in [
        (3, 42, [-1, -1]),
        (6, 2026, [0, 0]),
        (9, 0, [-160, -256]),
        (9, 0, [-136, -256]),
        (9, 0, [-256, -256]),
        (9, 2026, [216, -248]),
        (9, 0, [-236, -256]),
        (9, 42, [-28, 1]),
        (9, 2026, [-7, -30]),
        (9, 42, [6, 6]),
        (9, 42, [-4, -9]),
        (9, 42, [3, -4]),
        (9, 0, [-7, -2]),
    ] {
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let mut world = World::streaming(StreamIdentity {
            seed,
            generator: version,
        });
        for z in c[1] - 1..=c[1] + 1 {
            for x in c[0] - 1..=c[0] + 1 {
                let sampler = g.sampler([x, z]);
                for y in g.world_height().chunks() {
                    let cp = ChunkPos([x, y, z]);
                    world.insert_mesh_chunk(cp, sampler.chunk(cp).unwrap());
                }
            }
        }
        let targets = g
            .world_height()
            .chunks()
            .map(|y| ChunkPos([c[0], y, c[1]]))
            .collect();
        compare(&world, &targets);
        for block in [
            Block::Air,
            Block::Stone,
            Block::WaterSource,
            Block::Fern,
            Block::Torch,
        ] {
            for [x, y, z] in [[0, 15, 0], [15, 16, 15], [-1, -1, 16]] {
                let p = BlockPos([c[0] * 16 + x, y, c[1] * 16 + z]);
                let (cp, local) = p.split();
                let mut chunk = world.chunk(cp).unwrap().clone();
                chunk.set(local, block);
                world.insert_mesh_chunk(cp, chunk);
            }
            compare(&world, &targets);
        }
    }
}

#[test]
fn uniform_emitter_and_pending_descriptors_match_reference() {
    let targets = BTreeSet::from([ChunkPos([-1, 0, -1]), ChunkPos([0, 1, 0])]);
    for block in [Block::Air, Block::Stone, Block::WaterSource, Block::Torch] {
        let mut world = World::streaming(StreamIdentity {
            seed: 42,
            generator: 9,
        });
        for &cp in &targets {
            world.insert_mesh_chunk(cp, Chunk::filled(block));
        }
        compare(&world, &targets);
    }
}
