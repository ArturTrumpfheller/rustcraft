// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos, World,
    regions::RegionIdentity,
    worldgen::{GenerationConfig, Generator},
};

use voxel_core::worldgen::biome_v2;
const SEEDS: [u64; 4] = [0, 42, 2026, u64::MAX];
fn generator(seed: u64, version: u32) -> Generator {
    Generator::new(GenerationConfig { seed, version }).unwrap()
}
#[test]
fn v13_spawn_is_dry_supported_and_stable_under_gravity() {
    use voxel_core::{
        physics::{self, MovementInput, PhysicsConfig, Player},
        streaming,
    };
    for seed in [0, 42, 2026, u64::MAX] {
        let mut w = World::regional(RegionIdentity {
            seed,
            generator: 13,
            nonce: [36; 16],
        });
        streaming::prepare_spawn(&mut w).unwrap();
        let config = PhysicsConfig::default();
        let pos = physics::spawn_position(&w, &config, [0.5, 40., 10.5]);
        let mut player = Player::new(pos);
        assert!(
            w.block(BlockPos([
                pos[0].floor() as i32,
                pos[1].floor() as i32 - 1,
                pos[2].floor() as i32
            ]))
            .is_solid()
        );
        assert!(!player.in_water(&w, &config));
        for _ in 0..120 {
            player.tick(&w, MovementInput::default(), &config);
        }
        assert!((player.position[1] - pos[1]).abs() < 0.1);
    }
}
#[test]
fn natural_water_has_supported_beds_and_banks_after_density_composition() {
    let mut count = 0;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 13 }).unwrap();
        for c in [[-160, -256], [-28, 1], [6, 6], [-7, -30], [-40, -52]] {
            let s = g.sampler(c);
            for y in -2..=0 {
                let cp = ChunkPos([c[0], y, c[1]]);
                let chunk = s.chunk(cp).unwrap();
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(l).unwrap();
                    if chunk.get(l) != Block::WaterSource {
                        continue;
                    }
                    count += 1;
                    for d in [[0, -1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]] {
                        let n = p.offset(d).unwrap();
                        let b = s.block(n);
                        assert!(
                            b.is_solid() || b.fluid().is_some(),
                            "{seed} leak {p:?} to {n:?}: {b:?}"
                        );
                    }
                }
            }
        }
        assert!(g.terrain_admission_cache_entries() <= 128);
    }
    assert!(count > 1000);
}

#[test]
fn climate_and_biomes_match_point_halo_reverse_and_parallel_planning() {
    for seed in SEEDS {
        let g = generator(seed, 13);
        for c in [
            [-1, -1],
            [-1511, 1250],
            [1750, -1875],
            [-249, -197],
            [208, -204],
            [-2048, 2047],
        ] {
            let a = g.sampler(c);
            for dz in -1..=16 {
                for dx in -1..=16 {
                    let x = c[0] * 16 + dx;
                    let z = c[1] * 16 + dz;
                    if !(-32768..32768).contains(&x) || !(-32768..32768).contains(&z) {
                        continue;
                    }
                    let s = a.biome_sample(x, z).unwrap();
                    assert_eq!(Some(s), biome_v2::sample(&g, x, z), "sample {seed} {x} {z}");
                    assert_eq!(g.biome(x, z), Some(s.biome));
                    if dx == -1 || dx == 16 || dz == -1 || dz == 16 {
                        let other = g.sampler([x.div_euclid(16), z.div_euclid(16)]);
                        assert_eq!(Some(s), other.biome_sample(x, z));
                        for y in [s.height - 4, s.height, s.height + 1, s.height + 8] {
                            let p = BlockPos([x, y, z]);
                            assert_eq!(a.block(p), other.block(p), "halo {p:?}");
                        }
                    }
                }
            }
        }
        let coords = [[-1, 0], [0, 0], [-249, -197]];
        let columns = |c: [i32; 2]| {
            let s = g.sampler(c);
            g.world_height()
                .chunks()
                .map(|y| s.chunk(ChunkPos([c[0], y, c[1]])).unwrap())
                .collect::<Vec<_>>()
        };
        let expected: Vec<_> = coords.into_iter().map(columns).collect();
        for i in (0..coords.len()).rev() {
            assert_eq!(columns(coords[i]), expected[i]);
        }
        std::thread::scope(|scope| {
            let handles: Vec<_> = coords
                .into_iter()
                .map(|c| scope.spawn(move || columns(c)))
                .collect();
            for (h, e) in handles.into_iter().zip(expected) {
                assert_eq!(h.join().unwrap(), e);
            }
        });
        for [x, z] in [[i32::MIN, 0], [0, i32::MAX], [-32769, 0], [32768, 0]] {
            assert!(biome_v2::sample(&g, x, z).is_none());
            assert!(biome_v2::parameters(seed, x, z).is_none());
            assert!(g.biome(x, z).is_none());
        }
    }
}

#[test]
fn full_height_caves_water_plants_roots_crowns_and_halos_agree() {
    for seed in SEEDS {
        for c in [
            [-1, -1],
            [-249, -197],
            [-32, -200],
            [-1511, 1250],
            [1750, -1875],
            [208, -204],
            [-2048, 2047],
            [2047, -2048],
        ] {
            let report = audit::audit(seed, 13, c);
            assert!(report.findings.is_empty(), "{:#?}", report.findings);
        }
        audit::parallel_parity(seed, 13, [-32, -200]);
    }
}

#[test]
fn final_terrain_retains_all_fifteen_ecosystems_and_supported_alpine_rules() {
    use voxel_core::worldgen::Biome;
    let mut seen = std::collections::BTreeSet::new();
    for (seed, x, z, expected) in [
        (0, -3584_i32, -3904_i32, Biome::Plains),
        (0, 1984, -3904, Biome::Forest),
        (0, 3072, -1984, Biome::Desert),
        (0, -1792, -832, Biome::SnowyPlains),
        (2026, 28256, -28592, Biome::RockyHighlands),
        (0, -3008, -640, Biome::Meadow),
        (0, 512, 1152, Biome::BirchForest),
        (0, 1984, 256, Biome::Taiga),
        (0, 2688, 4032, Biome::Grove),
        (42, -768, -640, Biome::SnowySlopes),
        (2026, -23296, 20896, Biome::StonyPeaks),
        (0, -2688, -2304, Biome::Beach),
        (0, -128, -4032, Biome::Ocean),
        (2026, -960, -1728, Biome::River),
        (0, 1216, 1856, Biome::Lake),
    ] {
        let g = generator(seed, 13);
        let s = g.sampler([x.div_euclid(16), z.div_euclid(16)]);
        let sample = s.biome_sample(x, z).unwrap();
        assert_eq!(sample.biome, expected, "{seed} {x} {z}");
        assert_eq!(Some(sample.height), g.height(x, z));
        assert_eq!(Some(sample), biome_v2::sample(&g, x, z));
        let old_climate = biome_v2::parameters(seed, x, z).unwrap();
        assert_eq!(
            (sample.climate.temperature, sample.climate.humidity),
            (old_climate.temperature, old_climate.humidity)
        );
        let p = BlockPos([x, sample.height, z]);
        assert!(s.block(p).is_solid());
        if matches!(expected, Biome::Ocean | Biome::River | Biome::Lake) {
            assert_eq!(s.block(BlockPos([x, 2, z])), Block::WaterSource);
        }
        if expected == Biome::StonyPeaks {
            assert_eq!(sample.top, Block::Stone);
        }
        if expected == Biome::SnowySlopes {
            assert_eq!(sample.top, Block::SnowyGround);
            assert!(sample.height >= sample.climate.treeline());
        }
        seen.insert(expected);
    }
    assert_eq!(seen.len(), 15);
}
