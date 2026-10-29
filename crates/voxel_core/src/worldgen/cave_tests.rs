// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::{
    lighting::Skylight,
    regions::{RegionIdentity, RegionPos},
    streaming,
};
use std::collections::BTreeSet;
fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig { seed, version: 7 }).unwrap()
}
fn area(seed: u64, center: [i32; 2], radius: i32) -> World {
    let g = generator(seed);
    let mut w = World::regional(RegionIdentity {
        seed,
        generator: 7,
        nonce: [27; 16],
    });
    for cz in center[1] - radius..=center[1] + radius {
        for cx in center[0] - radius..=center[0] + radius {
            w.install_region(
                RegionPos::of(BlockPos([cx * 16, 0, cz * 16])),
                Default::default(),
            );
            let s = g.sampler([cx, cz]);
            for y in g.world_height().chunks() {
                let p = ChunkPos([cx, y, cz]);
                w.insert_chunk(p, s.chunk(p).unwrap());
            }
        }
    }
    w
}
fn crc(g: &Generator, c: [i32; 2]) -> u32 {
    let s = g.sampler(c);
    let mut crc = crc32fast::Hasher::new();
    for cy in g.world_height().chunks() {
        let chunk = s.chunk(ChunkPos([c[0], cy, c[1]])).unwrap();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let id: u16 = match chunk.get(LocalPos::new([x, y, z]).unwrap()) {
                        Block::Air => 0,
                        Block::Grass => 1,
                        Block::Dirt => 2,
                        Block::Stone => 3,
                        Block::Wood => 4,
                        Block::Leaves => 5,
                        Block::Sand => 20,
                        Block::SnowyGround => 21,
                        _ => panic!("no natural water or emitters"),
                    };
                    crc.update(&id.to_le_bytes());
                }
            }
        }
    }
    crc.finalize()
}
#[test]
fn v7_signed_extreme_point_chunk_order_thread_and_regeneration_agree() {
    for (seed, c, golden) in [
        (0, [0, 0], 392822372),
        (42, [3, -5], 4061003562),
        (2026, [-1, -1], 3418224074),
        (u64::MAX, [-2048, 2047], 3382729508),
        (42, [2047, -2048], 1524942302),
    ] {
        let g = generator(seed);
        let s = g.sampler(c);
        let first = crc(&g, c);
        assert_eq!(first, golden, "v7 frozen fixture seed={seed} column={c:?}");
        for cy in g.world_height().chunks().rev() {
            let cp = ChunkPos([c[0], cy, c[1]]);
            let chunk = s.chunk(cp).unwrap();
            assert_eq!(chunk, g.chunk(cp).unwrap());
            // Includes every face, local boundary and several internal offsets.
            for y in [0, 7, 15] {
                for z in [0, 8, 15] {
                    for x in [0, 8, 15] {
                        let l = LocalPos::new([x, y, z]).unwrap();
                        let p = cp.block_position(l).unwrap();
                        assert_eq!(chunk.get(l), g.block(p), "{seed} {p:?}");
                        assert_eq!(chunk.get(l), s.block(p));
                    }
                }
            }
        }
        assert_eq!(
            first,
            std::thread::scope(|scope| scope.spawn(|| crc(&g, c)).join().unwrap())
        );
        assert_eq!(first, crc(&generator(seed), c));
        // Querying another column through an existing sampler uses the same rules.
        let other = ChunkPos([c[0].saturating_add(1).min(2047), -2, c[1]]);
        assert_eq!(s.chunk(other), g.chunk(other));
    }
    let g = generator(42);
    for p in [
        [i32::MIN, 0, 0],
        [0, i32::MAX, 0],
        [32768, -30, 0],
        [-32769, -30, 0],
        [0, -65, 0],
        [0, 192, 0],
    ] {
        assert_eq!(g.block(BlockPos(p)), Block::Air);
    }
    assert_ne!(crc(&g, [3, -5]), crc(&generator(2026), [3, -5]));
}
#[test]
fn foundation_spawn_surface_and_sparse_solid_chunks_remain_safe() {
    let mut mixed = 0;
    let mut solid = 0;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let old = Generator::new(GenerationConfig { seed, version: 6 }).unwrap();
        for z in (-256..256).step_by(31) {
            for x in (-256..256).step_by(31) {
                assert_eq!(g.height(x, z), old.height(x, z));
                assert_eq!(g.biome(x, z), old.biome(x, z));
                for y in -64..-60 {
                    assert_eq!(g.block(BlockPos([x, y, z])), Block::Stone);
                }
            }
        }
        for c in [[0, 0], [3, -5], [-8, 7], [32, -31]] {
            let s = g.sampler(c);
            for cy in -4..0 {
                let chunk = s.chunk(ChunkPos([c[0], cy, c[1]])).unwrap();
                mixed += usize::from(chunk.uniform().is_none());
                solid += usize::from(chunk.uniform() == Some(Block::Stone));
                if chunk.uniform() == Some(Block::Stone) {
                    assert_eq!(chunk.allocated_bytes(), 0);
                }
            }
            assert_eq!(
                s.chunk(ChunkPos([c[0], 11, c[1]]))
                    .unwrap()
                    .allocated_bytes(),
                0
            );
        }
        let mut w = area(seed, [0, 0], 1);
        let cfg = crate::physics::PhysicsConfig::default();
        let p = crate::physics::spawn_position(&w, &cfg, [0.5, 30., 10.5]);
        let ground = BlockPos([
            p[0].floor() as i32,
            p[1].floor() as i32 - 1,
            p[2].floor() as i32,
        ]);
        assert!(w.block(ground).is_collidable());
        for y in ground.0[1] - 12..=ground.0[1] {
            assert!(w.block(BlockPos([ground.0[0], y, ground.0[2]])).is_opaque());
        }
        let mut player = crate::physics::Player::new(p);
        for _ in 0..60 {
            player.tick(&w, Default::default(), &cfg);
        }
        assert!(player.grounded);
        w.evict_column([0, 0]);
        streaming::prepare_column(&mut w, [0, 0]).unwrap();
        assert!(w.block(ground).is_solid());
    }
    assert!(mixed > 5 && solid > 5, "mixed={mixed} solid={solid}");
}
#[test]
fn procedural_chamber_light_matches_full_reference_and_torch_crosses_vertical_seam() {
    let mut w = area(42, [3, -5], 2);
    let p = BlockPos([51, -30, -66]);
    assert_eq!(w.block(p), Block::Air);
    let targets = BTreeSet::from([ChunkPos([3, -2, -5]), ChunkPos([3, -3, -5])]);
    let dark = Skylight::for_chunks(&w, &targets);
    assert_eq!(dark.level(p), 0);
    let floor = (-59..-30)
        .rev()
        .find(|&y| w.block(BlockPos([51, y, -66])).is_opaque())
        .unwrap();
    let torch = BlockPos([51, floor + 1, -66]);
    w.set_block(torch, Block::Torch).unwrap();
    let light = Skylight::for_chunks(&w, &targets);
    assert_eq!(light.block_level(torch), 14);
    for y in floor + 2..=-30 {
        let p = BlockPos([51, y, -66]);
        if y - floor < 15 {
            assert!(light.block_level(p) > 0);
        }
    }
    let full = Skylight::full_reference(&w, &targets);
    for x in 32..80 {
        for z in -96..-48 {
            for y in -64..48 {
                let p = BlockPos([x, y, z]);
                assert_eq!(light.level(p), full.level(p));
                assert_eq!(light.block_level(p), full.block_level(p));
                if w.block(p).is_opaque() {
                    assert_eq!(light.block_level(p), 0);
                }
            }
        }
    }
    // Existing greedy geometry uses the same exact light field on each slab.
    for cp in targets {
        let mesh = crate::mesh_chunk_lit(&w, cp, &light);
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.positions.len(), mesh.normals.len());
    }
}
#[test]
fn natural_entrance_reaches_deep_passages_without_an_isolated_surface_pit() {
    let w = area(2026, [2, -3], 4);
    let g = generator(2026);
    let mut todo = Vec::new();
    let mut seen = BTreeSet::new();
    // Find the real aperture, rather than carving a fixture opening.
    for x in 28..48 {
        for z in -54..-30 {
            let y = g.height(x, z).unwrap();
            let p = BlockPos([x, y, z]);
            if w.block(p) == Block::Air {
                todo.push(p);
                seen.insert(p);
            }
        }
    }
    assert!(!todo.is_empty());
    let targets = todo.iter().map(|p| p.split().0).collect();
    let light = Skylight::for_chunks(&w, &targets);
    assert!(todo.iter().any(|p| light.level(*p) > 0));
    let mut low = 100;
    let mut i = 0;
    while i < todo.len() {
        let p = todo[i];
        i += 1;
        low = low.min(p.0[1]);
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let n = p.offset(d).unwrap();
            if (0..96).contains(&n.0[0])
                && (-96..16).contains(&n.0[2])
                && (-60..=g.height(n.0[0], n.0[2]).unwrap()).contains(&n.0[1])
                && w.block(n) == Block::Air
                && seen.insert(n)
            {
                todo.push(n);
            }
        }
    }
    assert!(
        low < -40 && seen.len() > 2000,
        "depth={low} volume={}",
        seen.len()
    );
}
#[test]
fn real_cave_mining_placement_collision_and_fluid_recession_use_existing_gameplay() {
    use crate::{
        inventory::{self, Inventory, Item},
        physics::{PhysicsConfig, Player},
        raycast::Ray,
    };
    let mut w = area(42, [3, -5], 2);
    let cfg = PhysicsConfig::default();
    let floor = (-59..-30)
        .rev()
        .find(|&y| w.block(BlockPos([51, y, -66])).is_opaque())
        .unwrap();
    let mut player = Player::new([51.5, f64::from(floor + 1), -65.5]);
    for _ in 0..120 {
        player.tick(&w, Default::default(), &cfg);
    }
    assert!(player.grounded);
    let mut inv = Inventory::empty();
    let ray = Ray {
        origin: [51.5, f64::from(floor) + 2.6, -65.5],
        direction: [0., -1., 0.],
        reach: 5.,
    };
    let before = w.block(BlockPos([51, floor, -66]));
    assert_eq!(before, Block::Stone);
    assert!(
        inventory::edit_inventory(&mut w, &mut inv, ray, false, &player, &cfg)
            .unwrap()
            .is_some()
    );
    assert_eq!(w.block(BlockPos([51, floor, -66])), Block::Air);
    assert_eq!(inv.slots()[0].unwrap().item(), Item::Stone);
    assert!(
        inventory::edit_inventory(&mut w, &mut inv, ray, true, &player, &cfg)
            .unwrap()
            .is_some()
    );
    assert_eq!(w.block(BlockPos([51, floor, -66])), Block::Stone);
    assert!(inv.slots()[0].is_none());
    assert!(
        inventory::edit_inventory(&mut w, &mut inv, ray, false, &player, &cfg)
            .unwrap()
            .is_some()
    );
    // Placement beside the player stays transactional and cannot overlap the body.
    let target = BlockPos([54, floor + 1, -66]);
    w.set_block(target, Block::Wood).unwrap();
    let water = BlockPos([51, floor + 1, -66]);
    w.set_block(water, Block::WaterSource).unwrap();
    let mut changes = Vec::new();
    let mut flowed = false;
    for _ in 0..100 {
        w.tick_fluids(Default::default(), &mut changes);
        flowed |= changes.iter().any(|c| c.after.fluid().is_some());
    }
    assert!(flowed);
    assert!(w.block(BlockPos([51, floor, -66])).fluid().is_some());
    w.set_block(water, Block::Air).unwrap();
    for _ in 0..1000 {
        w.tick_fluids(Default::default(), &mut changes);
        if w.fluid_updates().pending() == 0 {
            break;
        }
    }
    assert_eq!(w.block(BlockPos([51, floor, -66])), Block::Air);
    assert_eq!(w.block(target), Block::Wood);
}

#[test]
fn benchmark_same_content_scene_and_entire_halo_match_frozen_v6() {
    let old = Generator::new(GenerationConfig {
        seed: 42,
        version: 6,
    })
    .unwrap();
    let new = generator(42);
    for z in 64..=66 {
        for x in 64..=66 {
            let a = old.sampler([x, z]);
            let b = new.sampler([x, z]);
            for y in -4..12 {
                let p = ChunkPos([x, y, z]);
                assert_eq!(a.chunk(p), b.chunk(p), "{p:?}");
            }
        }
    }
}
