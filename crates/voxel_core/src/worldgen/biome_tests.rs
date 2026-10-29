use super::*;
use crate::physics::{PhysicsConfig, Player, spawn_position};
use biome::{Biome, sample_biome, sample_climate};
use std::collections::{BTreeMap, BTreeSet};

fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig { seed, version: 5 }).unwrap()
}
const SITES: [(Biome, i32, i32); 5] = [
    (Biome::Plains, -1568, -4096),
    (Biome::Forest, -4096, -4096),
    (Biome::Desert, -2176, -4096),
    (Biome::SnowyPlains, -832, -4096),
    (Biome::RockyHighlands, -2368, -4096),
];
#[test]
fn climate_is_broad_balanced_continuous_and_independent_of_request_order() {
    for seed in [0, 42, 2026, u64::MAX] {
        let mut counts = BTreeMap::new();
        let mut same = 0;
        let mut total = 0;
        for z in (-4096..4096).step_by(64) {
            for x in (-4096..4096).step_by(64) {
                let c = sample_climate(seed, x, z).unwrap();
                let b = sample_biome(seed, x, z).unwrap();
                assert_eq!(biome::classify(c), b);
                assert_eq!(sample_climate(seed, x, z), Some(c));
                assert!(c.temperature.abs() <= 1024 && c.moisture.abs() <= 1024);
                *counts.entry(b).or_insert(0) += 1;
                for [dx, dz] in [[16, 0], [0, 16]] {
                    total += 1;
                    same += usize::from(sample_biome(seed, x + dx, z + dz) == Some(b));
                    let n = sample_climate(seed, x + dx, z + dz).unwrap();
                    assert!((n.temperature - c.temperature).abs() < 150);
                    assert!((n.moisture - c.moisture).abs() < 180);
                }
            }
        }
        assert_eq!(counts.len(), 5);
        for count in counts.values() {
            assert!((650..8000).contains(count), "seed {seed} {counts:?}");
        }
        assert!(
            same * 100 > total * 90,
            "many-chunk regions: {seed} {same}/{total}"
        );
    }
    for [x, z] in [[i32::MIN, 0], [0, i32::MAX], [32768, 0], [-32769, 0]] {
        assert_eq!(sample_biome(42, x, z), None);
        assert_eq!(sample_climate(42, x, z), None);
    }
    assert_eq!(CURRENT_LOCAL_GENERATOR, 6);
    for version in 1..5 {
        assert_eq!(
            Generator::new(GenerationConfig { seed: 42, version })
                .unwrap()
                .biome(0, 0),
            None
        );
    }
    assert!(
        Generator::new(GenerationConfig {
            seed: 42,
            version: 7
        })
        .is_err()
    );
}

#[test]
fn biomes_have_connected_interiors_and_boundaries_do_not_follow_chunks() {
    for (b, x, z) in SITES {
        let mut cells = BTreeSet::new();
        for dz in (-48..=48).step_by(8) {
            for dx in (-48..=48).step_by(8) {
                assert_eq!(sample_biome(42, x + dx, z + dz), Some(b));
                cells.insert([dx, dz]);
            }
        }
        let mut todo = vec![[0, 0]];
        cells.remove(&[0, 0]);
        while let Some([x, z]) = todo.pop() {
            for n in [[x - 8, z], [x + 8, z], [x, z - 8], [x, z + 8]] {
                if cells.remove(&n) {
                    todo.push(n);
                }
            }
        }
        assert!(cells.is_empty());
    }
    let mut aligned = 0;
    let mut changes = 0;
    for z in (-3072..3072).step_by(193) {
        for x in -3072..3072 {
            if sample_biome(42, x, z) != sample_biome(42, x - 1, z) {
                changes += 1;
                aligned += usize::from(x.rem_euclid(16) == 0);
            }
        }
    }
    assert!(changes > 100);
    assert!(aligned * 100 < changes * 15, "{aligned}/{changes}");
}

#[test]
fn terrain_transitions_surfaces_and_vertical_headroom() {
    let mut saw_stone = false;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        for z in [-4096, -3000, -1992, 0, 127, 4095] {
            let mut old = g.height(-4096, z).unwrap();
            for x in -4095..4096 {
                let h = g.height(x, z).unwrap();
                assert!((-1..=24).contains(&h));
                assert!((h - old).abs() <= 2, "cliff {seed} {x} {z} {old}/{h}");
                old = h;
            }
        }
    }
    for (b, x, z) in SITES {
        let c = biome::column(42, x, z);
        let expected = match b {
            Biome::Desert => Block::Sand,
            Biome::SnowyPlains => Block::SnowyGround,
            Biome::RockyHighlands => Block::Stone,
            _ => Block::Grass,
        };
        assert_eq!(c.top, expected, "{b:?}");
        let g = generator(42);
        for y in -16..32 {
            assert_eq!(
                g.block(BlockPos([x, y, z])),
                g.sampler([x.div_euclid(16), z.div_euclid(16)])
                    .block(BlockPos([x, y, z]))
            );
        }
        if b == Biome::Desert {
            for depth in 0..=4 {
                assert_eq!(g.block(BlockPos([x, c.height - depth, z])), Block::Sand);
            }
        }
        if b == Biome::RockyHighlands {
            saw_stone = c.top == Block::Stone;
        }
        assert_eq!(g.block(BlockPos([x, 31, z])), Block::Air);
    }
    assert!(saw_stone);
    // Dry temperate land stays plains; warm highlands do not become all snow.
    assert_eq!(
        biome::classify(biome::Climate {
            temperature: 0,
            moisture: -700,
            geography: 0
        }),
        Biome::Plains
    );
    assert_eq!(
        biome::classify(biome::Climate {
            temperature: 700,
            moisture: 0,
            geography: 500
        }),
        Biome::RockyHighlands
    );
}

#[test]
fn biome_trees_are_supported_sparse_or_dense_and_cross_chunk_order_independent() {
    let g = generator(42);
    let mut roots = BTreeMap::new();
    let mut cross = false;
    for (b, x, z) in SITES {
        let mut world = World::regional(crate::regions::RegionIdentity {
            seed: 42,
            generator: 5,
            nonce: [1; 16],
        });
        let center = [x.div_euclid(16), z.div_euclid(16)];
        for cz in center[1] - 1..=center[1] + 2 {
            for cx in center[0] - 1..=center[0] + 2 {
                let s = g.sampler([cx, cz]);
                for cy in -1..2 {
                    let p = ChunkPos([cx, cy, cz]);
                    world.insert_chunk(p, s.chunk(p).unwrap());
                }
            }
        }
        let cfg = PhysicsConfig::default();
        let feet = spawn_position(&world, &cfg, [f64::from(x) + 0.5, 28., f64::from(z) + 0.5]);
        assert!(Player::new(feet).fits_in_world(&world, &cfg));
        let foot = BlockPos([
            feet[0].floor() as i32,
            feet[1].floor() as i32 - 1,
            feet[2].floor() as i32,
        ]);
        assert!(matches!(
            world.block(foot),
            Block::Grass | Block::Dirt | Block::Stone | Block::Sand | Block::SnowyGround
        ));
        let mut count = 0;
        for wz in z - 12..z + 28 {
            for wx in x - 12..x + 28 {
                let h = g.height(wx, wz).unwrap();
                assert_eq!(world.block(BlockPos([wx, -16, wz])), Block::Stone);
                assert_eq!(world.block(BlockPos([wx, 31, wz])), Block::Air);
                if world.block(BlockPos([wx, h + 1, wz])) != Block::Wood {
                    continue;
                }
                count += 1;
                assert!(matches!(
                    world.block(BlockPos([wx, h, wz])),
                    Block::Grass | Block::SnowyGround
                ));
                let top = (h + 1..31)
                    .take_while(|&y| world.block(BlockPos([wx, y, wz])) == Block::Wood)
                    .last()
                    .unwrap();
                assert!(top < 30);
                for dx in -2..=2 {
                    for dz in -2..=2 {
                        let nh = g.height(wx + dx, wz + dz).unwrap();
                        assert!((nh - h).abs() <= 2 && nh < top - 1);
                        let p = BlockPos([wx + dx, top, wz + dz]);
                        assert_eq!(g.block(p), world.block(p));
                        if dx.abs() + dz.abs() <= 3
                            && p.split().0 != BlockPos([wx, top, wz]).split().0
                        {
                            cross = true;
                        }
                    }
                }
            }
        }
        roots.insert(b, count);
        // Reverse regeneration agrees with all installed chunks, including canopies.
        for (p, c) in world.chunks().collect::<Vec<_>>().into_iter().rev() {
            assert_eq!(g.chunk(p).unwrap(), *c);
        }
    }
    assert_eq!(roots[&Biome::Desert], 0);
    assert!(roots[&Biome::Forest] >= 8, "{roots:?}");
    assert!(
        roots[&Biome::Forest] > roots[&Biome::Plains] * 2,
        "{roots:?}"
    );
    assert!(
        roots[&Biome::Forest] > roots[&Biome::RockyHighlands] * 2,
        "{roots:?}"
    );
    assert!(cross);
}

#[test]
fn spawn_caves_and_signed_extreme_columns_remain_safe() {
    for seed in [0, 1, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let mut world = World::default();
        let s = g.sampler([0, 0]);
        for y in -1..2 {
            let p = ChunkPos([0, y, 0]);
            world.insert_chunk(p, s.chunk(p).unwrap());
        }
        let cfg = PhysicsConfig::default();
        let feet = spawn_position(&world, &cfg, [0.5, 28., 10.5]);
        assert!(Player::new(feet).fits_in_world(&world, &cfg));
        for z in 8..=13 {
            for x in -3..=3 {
                let h = g.height(x, z).unwrap();
                assert_ne!(g.block(BlockPos([x, h + 1, z])), Block::Wood);
            }
        }
        let entrance = g.height(-10, 20).unwrap() + 1;
        for z in -16..=22 {
            let floor = entrance - ((20 - z).clamp(0, 36) * (entrance + 6) / 32).min(entrance + 6);
            for y in floor..floor + 4 {
                assert_eq!(g.block(BlockPos([-10, y, z])), Block::Air);
            }
        }
        assert_eq!(g.block(BlockPos([-10, -4, -10])), Block::Air);
        for column in [[-2048, -2048], [2047, 2047], [-1, 3], [4, -4]] {
            let s = g.sampler(column);
            for y in -1..2 {
                let p = ChunkPos([column[0], y, column[1]]);
                let c = s.chunk(p).unwrap();
                for z in [0, 15] {
                    for x in [0, 15] {
                        for ly in [0, 8, 15] {
                            let l = LocalPos::new([x, ly, z]).unwrap();
                            assert_eq!(c.get(l), g.block(p.block_position(l).unwrap()));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn v5_goldens_pin_new_world_output_across_signed_distant_columns_and_threads() {
    for (seed, expected) in [
        (0, 0x661347a7),
        (42, 0xfb613b69),
        (2026, 0xe3267b21),
        (u64::MAX, 0xe6f50f5e),
    ] {
        let g = generator(seed);
        let columns = [
            [0, 0],
            [-1, 3],
            [4, -4],
            [120, -120],
            [1000, -1000],
            [2047, -2048],
        ];
        let mut hash = crc32fast::Hasher::new();
        for c in columns {
            let sampler = g.sampler(c);
            for cy in -1..2 {
                let p = ChunkPos([c[0], cy, c[1]]);
                let chunk = sampler.chunk(p).unwrap();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            hash.update(&[match chunk.get(LocalPos::new([x, y, z]).unwrap()) {
                                Block::Air => 0,
                                Block::Grass => 1,
                                Block::Dirt => 2,
                                Block::Stone => 3,
                                Block::Wood => 4,
                                Block::Leaves => 5,
                                Block::Sand => 20,
                                Block::SnowyGround => 21,
                                _ => panic!("unexpected gameplay block"),
                            }]);
                        }
                    }
                }
            }
        }
        assert_eq!(hash.finalize(), expected, "v5 seed {seed}");
        assert_eq!(g.planning_bytes(), 0);
        std::thread::scope(|scope| {
            for c in columns.into_iter().rev() {
                let g = &g;
                scope.spawn(move || {
                    let sampler = g.sampler(c);
                    for y in -1..2 {
                        let p = ChunkPos([c[0], y, c[1]]);
                        assert_eq!(g.chunk(p), sampler.chunk(p));
                    }
                });
            }
        });
    }
}

#[test]
fn v5_tree_cave_and_edit_meshes_and_light_match_full_neighbors_at_region_seams() {
    use std::{collections::BTreeMap, sync::Arc};
    let g = Generator::new(GenerationConfig {
        seed: 2026,
        version: 5,
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
