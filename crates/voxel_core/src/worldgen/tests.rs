use super::*;
use crate::{
    EDIT_MAX, EDIT_MIN,
    persistence::encode,
    physics::{MovementInput, PhysicsConfig, Player, spawn_position},
};
fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig {
        seed,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn deterministic_seeds_chunk_order_and_golden_worlds() {
    for (seed, golden) in [
        (0, 0xc45927a4),
        (1, 0xbd348afc),
        (42, 0xf80a9287),
        (2026, 0xd14aedaf),
        (u64::MAX, 0xea69a8c0),
    ] {
        let g = generator(seed);
        let world = g.generate();
        let bytes = encode(&world).unwrap();
        assert_eq!(bytes, encode(&generator(seed).generate()).unwrap());
        let mut reverse = World::default();
        for x in (-2..2).rev() {
            for z in (-2..2).rev() {
                for y in (-1..2).rev() {
                    let pos = ChunkPos([x, y, z]);
                    let chunk = g.chunk(pos).unwrap();
                    reverse.insert_chunk(pos, chunk);
                }
            }
        }
        assert_eq!(bytes, encode(&reverse).unwrap());
        assert_eq!(
            crc32fast::hash(&bytes),
            golden,
            "generation contract changed for {seed}"
        );
    }
    assert_ne!(
        encode(&generator(1).generate()).unwrap(),
        encode(&generator(2).generate()).unwrap()
    );
    assert!(
        Generator::new(GenerationConfig {
            seed: 0,
            version: 999
        })
        .is_err()
    );
}

#[test]
fn terrain_has_coherent_varied_heights_layers_bounded_chunks_and_firm_foundation() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let world = g.generate();
        let min = *g.heights.iter().min().unwrap();
        let max = *g.heights.iter().max().unwrap();
        assert!(max - min >= 6, "unvaried seed {seed}: {min}..{max}");
        assert!(min >= -8 && max <= 18);
        for z in -32..32 {
            for x in -32..32 {
                let h = g.height(x, z).unwrap();
                for y in [-16, -15] {
                    assert_eq!(world.block(BlockPos([x, y, z])), Block::Stone);
                }
                for y in -14..=h {
                    let b = world.block(BlockPos([x, y, z]));
                    if b != Block::Air {
                        assert_eq!(
                            b,
                            if y == h {
                                Block::Grass
                            } else if y >= h - 3 {
                                Block::Dirt
                            } else {
                                Block::Stone
                            }
                        );
                    }
                }
                if x < 31 {
                    assert!((h - g.height(x + 1, z).unwrap()).abs() <= 3);
                }
                if z < 31 {
                    assert!((h - g.height(x, z + 1).unwrap()).abs() <= 3);
                }
            }
        }
        assert!(world.chunks().count() <= 48);
        for (pos, _) in world.chunks() {
            let p = pos
                .block_position(LocalPos::new([0; 3]).unwrap())
                .unwrap()
                .0;
            for axis in 0..3 {
                assert!(p[axis] >= EDIT_MIN[axis] && p[axis] + 15 < EDIT_MAX[axis]);
            }
        }
        for p in [
            ChunkPos([-3, 0, 0]),
            ChunkPos([0, -2, 0]),
            ChunkPos([0, 0, 2]),
            ChunkPos([i32::MAX; 3]),
        ] {
            assert!(g.chunk(p).is_none());
        }
        assert_eq!(g.block(BlockPos([i32::MIN; 3])), Block::Air);
    }
}

#[test]
fn cave_network_has_a_continuous_headroom_ramp_and_accessible_caverns() {
    for seed in [0, 1, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let mut last = g.entrance_height;
        for z in (-12..=20).rev() {
            let floor = g.entrance_height
                - ((20 - z) * (g.entrance_height + 6) / 32).min(g.entrance_height + 6);
            assert!((floor - last).abs() <= 1, "unclimbable ramp");
            last = floor;
            for y in floor..floor + 4 {
                assert_eq!(
                    g.block(BlockPos([-10, y, z])),
                    Block::Air,
                    "blocked entrance at {z}/{y}"
                );
            }
        }
        for x in -32..32 {
            let center = -10 + noise(seed, x, 0, 16, 53) * 3 / 1024;
            assert_eq!(g.block(BlockPos([x, -4, center])), Block::Air);
            assert_eq!(g.block(BlockPos([x, -3, center])), Block::Air);
        }
        assert!(g.entrance_height > g.height(-10, 20).unwrap());
    }
}

#[test]
fn trees_have_supported_trunks_clear_canopies_spacing_and_cross_chunk_foliage() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        assert!(g.trees.len() >= 8);
        let mut crosses = false;
        for tree in &g.trees {
            assert_eq!(
                g.block(BlockPos([tree.x, tree.ground, tree.z])),
                Block::Grass
            );
            for y in 1..=tree.trunk {
                assert_eq!(
                    g.block(BlockPos([tree.x, tree.ground + y, tree.z])),
                    Block::Wood
                );
            }
            assert_eq!(
                g.block(BlockPos([tree.x, tree.ground + tree.trunk + 1, tree.z])),
                Block::Leaves
            );
            for other in &g.trees {
                if tree.x != other.x || tree.z != other.z {
                    assert!((tree.x - other.x).abs().max((tree.z - other.z).abs()) >= 6);
                }
            }
            for dx in -2..=2 {
                for dz in -2..=2 {
                    if g.block(BlockPos([
                        tree.x + dx,
                        tree.ground + tree.trunk,
                        tree.z + dz,
                    ])) == Block::Leaves
                    {
                        crosses |= (tree.x + dx).div_euclid(16) != tree.x.div_euclid(16)
                            || (tree.z + dz).div_euclid(16) != tree.z.div_euclid(16);
                    }
                }
            }
        }
        assert!(crosses, "missing cross-boundary tree coverage");
    }
}

#[test]
fn spawn_and_respawn_are_supported_clear_and_physically_stable_for_many_seeds() {
    let config = PhysicsConfig::default();
    for seed in 0..24 {
        let world = generator(seed).generate();
        let spawn = spawn_position(&world, &config, [0.5, 4., 10.5]);
        let mut player = Player::new(spawn);
        assert!(player.fits_in_world(&world, &config));
        for _ in 0..120 {
            player.tick(&world, MovementInput::default(), &config);
        }
        assert!(player.grounded, "seed {seed}");
        assert!((player.position[1] - spawn[1]).abs() <= 1.001);
        assert_eq!(spawn, spawn_position(&world, &config, [0.5, 4., 10.5]));
        assert!(!matches!(
            world.block(BlockPos([
                spawn[0].floor() as i32,
                player.position[1] as i32 - 1,
                spawn[2].floor() as i32
            ])),
            Block::Wood | Block::Leaves
        ));
    }
}

#[test]
fn new_materials_are_solid_and_cull_faces_across_positive_and_negative_seams() {
    use crate::mesh_chunk;
    for x in [-17, -1, 15] {
        let mut world = World::default();
        world.set_block(BlockPos([x, 1, 0]), Block::Wood).unwrap();
        world
            .set_block(BlockPos([x + 1, 1, 0]), Block::Leaves)
            .unwrap();
        let a = BlockPos([x, 1, 0]).split().0;
        let b = BlockPos([x + 1, 1, 0]).split().0;
        assert_ne!(a, b);
        assert_eq!(mesh_chunk(&world, a).indices.len(), 30);
        assert_eq!(mesh_chunk(&world, b).indices.len(), 30);
        let config = PhysicsConfig::default();
        let mut player = Player::new([f64::from(x) + 1.5, 4., 0.5]);
        for _ in 0..120 {
            player.tick(&world, MovementInput::default(), &config);
        }
        assert!(player.grounded);
        assert!((player.position[1] - 2.).abs() < 1e-8);
    }
}

#[test]
fn local_v3_plans_are_area_independent_and_extreme_queries_are_safe() {
    let g = Generator::new(GenerationConfig {
        seed: 42,
        version: 3,
    })
    .unwrap();
    assert_eq!(g.planning_bytes(), 0);
    assert_eq!(g.chunk_limit(), 2048);
    for [x, z] in [[0, 0], [16000, -16000], [-32768, 32767], [32767, -32768]] {
        assert!(g.height(x, z).is_some());
    }
    for p in [
        ChunkPos([i32::MIN, 0, 0]),
        ChunkPos([i32::MAX, 0, 0]),
        ChunkPos([2048, 0, 0]),
    ] {
        assert!(g.chunk(p).is_none());
    }
    assert_eq!(g.block(BlockPos([i32::MIN, 0, i32::MAX])), Block::Air);
    assert_eq!(g.planning_bytes(), 0);
}
#[test]
fn local_v3_far_golden_columns_are_order_independent() {
    for seed in [0, 42, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 3 }).unwrap();
        let columns = [[0, 0], [1000, -1000], [-2048, 2047], [2047, -2048]];
        let mut hash = crc32fast::Hasher::new();
        for c in columns {
            let a = crate::streaming::build_column(
                &g,
                std::sync::Arc::new(std::collections::BTreeMap::new()),
                c,
                0,
            );
            let b = crate::streaming::build_column(
                &g,
                std::sync::Arc::new(std::collections::BTreeMap::new()),
                c,
                0,
            );
            for ((p, chunk, mesh), (q, other, othermesh)) in a.chunks.into_iter().zip(b.chunks) {
                assert_eq!(p, q);
                assert_eq!(chunk, other);
                assert_eq!(mesh, othermesh);
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let b = chunk.get(LocalPos::new([x, y, z]).unwrap());
                            hash.update(&[match b {
                                Block::Air => 0,
                                Block::Grass => 1,
                                Block::Dirt => 2,
                                Block::Stone => 3,
                                Block::Wood => 4,
                                Block::Leaves => 5,
                            }]);
                        }
                    }
                }
            }
        }
        let expected = match seed {
            0 => 0xb369fd23,
            42 => 0x1e775a10,
            _ => 0xa9276b8b,
        };
        assert_eq!(hash.finalize(), expected, "generator v3 seed {seed}");
        for c in columns.into_iter().rev() {
            assert_eq!(
                g.chunk(ChunkPos([c[0], 0, c[1]])),
                g.sampler(c).chunk(ChunkPos([c[0], 0, c[1]]))
            );
        }
    }
}
#[test]
fn local_v3_tree_cave_and_edit_halos_match_full_neighbors_at_distant_region_seams() {
    use std::{collections::BTreeMap, sync::Arc};
    let g = Generator::new(GenerationConfig {
        seed: 2026,
        version: 3,
    })
    .unwrap();
    for center in [[1000, -1000], [-1001, 1003], [3, 4]] {
        let mut reference = World::default();
        for x in center[0] - 1..=center[0] + 1 {
            for z in center[1] - 1..=center[1] + 1 {
                for y in -1..2 {
                    let p = ChunkPos([x, y, z]);
                    reference.insert_chunk(p, g.chunk(p).unwrap());
                }
            }
        }
        let mut edits = BTreeMap::new();
        for p in [
            BlockPos([center[0] * 16, -15, center[1] * 16]),
            BlockPos([center[0] * 16 - 1, 25, center[1] * 16]),
            BlockPos([center[0] * 16 + 15, 15, center[1] * 16 + 15]),
        ] {
            let b = if p.0[1] < 0 { Block::Air } else { Block::Wood };
            edits.insert(p, b);
            reference.apply_cached_edit(p, b);
        }
        for (p, c, m) in crate::streaming::build_column(&g, Arc::new(edits), center, 0).chunks {
            assert_eq!(&c, reference.chunk(p).unwrap());
            assert_eq!(m, crate::mesh_chunk(&reference, p));
        }
        for p in [
            BlockPos([center[0] * 16, 4, center[1] * 16]),
            BlockPos([center[0] * 16 + 15, 20, center[1] * 16 + 15]),
        ] {
            assert_eq!(g.block(p), g.sampler(center).block(p));
        }
    }
}

#[test]
fn local_generation_benchmark_reports_fixed_planning_and_column_latency() {
    use std::{collections::BTreeMap, sync::Arc, time::Instant};
    for version in [2, 3] {
        let t = Instant::now();
        let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        let setup = t.elapsed();
        let mut times = Vec::new();
        let mut bytes = 0;
        for _ in 0..3 {
            for c in [[0, 0], [1, 1], [-2, -2], [2, -1]] {
                let r = crate::streaming::build_column(&g, Arc::new(BTreeMap::new()), c, 0);
                times.push(r.elapsed);
                bytes += r
                    .chunks
                    .iter()
                    .map(|(_, _, m)| m.positions.len() * 32 + m.indices.len() * 4)
                    .sum::<usize>();
            }
        }
        times.sort_unstable();
        println!(
            "GEN_BENCH version={version} setup={setup:?} plan_bytes={} column_p50={:?} p95={:?} p99={:?} output_bytes={bytes}",
            g.planning_bytes(),
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times[times.len() * 99 / 100]
        );
        if version == 3 {
            assert_eq!(g.planning_bytes(), 0);
        }
    }
}

#[test]
fn local_v3_generates_trees_and_underground_caves_far_from_spawn() {
    let g = Generator::new(GenerationConfig {
        seed: 42,
        version: 3,
    })
    .unwrap();
    let mut wood = 0;
    let mut leaves = 0;
    let mut caves = 0;
    for x in 1022..=1026 {
        for z in 1022..=1026 {
            let s = g.sampler([x, z]);
            for y in -1..2 {
                let p = ChunkPos([x, y, z]);
                let c = s.chunk(p).unwrap();
                for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let local = LocalPos::new([lx, ly, lz]).unwrap();
                            let b = c.get(local);
                            let pos = p.block_position(local).unwrap();
                            wood += usize::from(b == Block::Wood);
                            leaves += usize::from(b == Block::Leaves);
                            caves += usize::from(
                                b == Block::Air
                                    && pos.0[1] > -14
                                    && pos.0[1] < g.height(pos.0[0], pos.0[2]).unwrap() - 3,
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(
        wood > 0 && leaves > 0 && caves > 0,
        "wood {wood}, leaves {leaves}, caves {caves}"
    );
}
