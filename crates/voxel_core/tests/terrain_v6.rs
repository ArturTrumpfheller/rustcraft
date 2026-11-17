// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator, biome_v2, terrain_v5, terrain_v6},
};

#[test]
fn v17_full_height_columns_pin_the_new_version_without_rewriting_old_fixtures() {
    for line in include_str!("fixtures/columns-v17.csv").lines().skip(1) {
        let r: Vec<_> = line.split(',').collect();
        let seed = r[1].parse().unwrap();
        let c = [r[2].parse().unwrap(), r[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version: 17 }).unwrap();
        assert_eq!(
            audit::crc(&audit::column(&g, c)),
            r[4].parse::<u32>().unwrap(),
            "{line}"
        );
        assert_eq!(
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8)
                .map_or(255, |b| b as u8),
            r[5].parse::<u8>().unwrap(),
            "{line}"
        );
    }
}

#[test]
fn all_fifteen_v17_ecosystems_have_supported_vegetation_and_water_geometry() {
    let g = Generator::new(GenerationConfig {
        seed: 0,
        version: 17,
    })
    .unwrap();
    // Fixed sites from the final terrain-only64-grid survey, not new biome IDs.
    for (expected, x, z) in [
        (0, -3456, -4096),
        (1, -1344, -4096),
        (2, -2880, -4096),
        (3, 960, -4096),
        (4, 0, -4096),
        (5, 2240, -3968),
        (6, -2240, -4096),
        (7, -4096, -4096),
        (8, -1984, -3776),
        (9, 192, -4032),
        (10, -2688, -2304),
        (11, 1920, -4096),
        (12, -3584, -4096),
        (13, 4032, -4032),
        (14, 576, -3776),
    ] {
        assert_eq!(g.biome(x, z).unwrap() as u8, expected);
        let report = audit::audit(0, 17, [x.div_euclid(16), z.div_euclid(16)]);
        assert!(report.findings.is_empty(), "{:#?}", report.findings);
    }
}

#[test]
fn v17_height_ecology_and_hydrology_share_the_same_versioned_geography() {
    for seed in [0, 42, 2026, u64::MAX, 123, 777] {
        let g = Generator::new(GenerationConfig { seed, version: 17 }).unwrap();
        for c in [
            [-1, -1],
            [-1511, 1250],
            [1750, -1875],
            [-2048, 2047],
            [2047, -2048],
            [-32, -200],
        ] {
            let s = g.sampler(c);
            for (dx, dz) in [(0, 0), (15, 15), (-1, 8), (16, 8), (8, -1), (8, 16)] {
                let (x, z) = (c[0] * 16 + dx, c[1] * 16 + dz);
                if !(-32768..32768).contains(&x) || !(-32768..32768).contains(&z) {
                    continue;
                }
                let b = s.biome_sample(x, z).unwrap();
                assert_eq!(Some(b), biome_v2::sample(&g, x, z));
                assert_eq!(Some(b.height), g.height(x, z));
                let old = biome_v2::parameters(seed, x, z).unwrap();
                assert_eq!(
                    (b.climate.temperature, b.climate.humidity),
                    (old.temperature, old.humidity)
                );
                let p = terrain_v6::parameters(seed, x, z);
                assert_eq!(b.climate.continentalness, p.continentalness / 4);
                assert_eq!(b.climate.erosion, p.erosion / 4);
                assert_eq!(b.climate.weirdness, p.weirdness / 4);
                assert_eq!(
                    terrain_v5::height_for(17, seed, x, z),
                    terrain_v6::height(seed, x, z)
                );
            }
        }
    }
}

#[test]
fn v17_sources_remain_contained_and_tree_roots_never_displace_water() {
    let mut water = 0;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 17 }).unwrap();
        for c in [[-160, -256], [-28, 1], [6, 6], [-7, -30], [-40, -52]] {
            let s = g.sampler(c);
            for y in -2..=0 {
                let cp = ChunkPos([c[0], y, c[1]]);
                let ch = s.chunk(cp).unwrap();
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    if ch.get(l) != Block::WaterSource {
                        continue;
                    }
                    water += 1;
                    let p = cp.block_position(l).unwrap();
                    for d in [[0, -1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]] {
                        let b = s.block(p.offset(d).unwrap());
                        assert!(
                            b.is_solid() || b.fluid().is_some(),
                            "{seed} leak {p:?} {d:?}: {b:?}"
                        );
                    }
                }
            }
        }
        assert!(g.terrain_admission_cache_entries() <= 128);
        for p in [
            BlockPos([i32::MIN, 0, 0]),
            BlockPos([0, 192, 0]),
            BlockPos([32768, 0, 0]),
            BlockPos([0, -65, 0]),
        ] {
            assert_eq!(g.block(p), Block::Air);
        }
    }
    assert!(water > 1000);
}
