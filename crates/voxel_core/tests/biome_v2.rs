// SPDX-License-Identifier: MIT OR Apache-2.0
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos,
    worldgen::{Biome, GenerationConfig, Generator, biome_v2},
};
const SEEDS: [u64; 4] = [0, 42, 2026, u64::MAX];
fn generator(seed: u64, version: u32) -> Generator {
    Generator::new(GenerationConfig { seed, version }).unwrap()
}
#[test]
fn exact_merged_slice37_historical_columns_and_biomes() {
    for line in include_str!("fixtures/history-v1-v12.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let version = v[0].parse().unwrap();
        let seed = v[1].parse().unwrap();
        let c = [v[2].parse::<i32>().unwrap(), v[3].parse::<i32>().unwrap()];
        let g = generator(seed, version);
        let s = g.sampler(c);
        let mut crc = crc32fast::Hasher::new();
        for y in g.world_height().chunks() {
            let ch = s.chunk(ChunkPos([c[0], y, c[1]])).unwrap();
            for i in 0..4096 {
                let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                crc.update(&(ch.get(l) as u16).to_le_bytes());
            }
        }
        assert_eq!(crc.finalize(), v[4].parse::<u32>().unwrap(), "{line}");
        assert_eq!(
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8)
                .map_or(255, |b| b as u8),
            v[5].parse::<u8>().unwrap(),
            "{line}"
        );
    }
}
#[test]
fn v12_preserves_every_base_solid_air_cave_and_water_cell() {
    for seed in SEEDS {
        let old = generator(seed, 11);
        let new = generator(seed, 12);
        for c in [
            [-249, -197],
            [-32, -200],
            [-136, -256],
            [6, 6],
            [-1, 0],
            [-1511, 1250],
            [2047, -2048],
            [220, 108],
            [-101, -252],
        ] {
            let a = old.sampler(c);
            let b = new.sampler(c);
            for i in 0..256 {
                let x = c[0] * 16 + i % 16;
                let z = c[1] * 16 + i / 16;
                assert_eq!(old.height(x, z), new.height(x, z));
            }
            for y in new.world_height().chunks() {
                let cp = ChunkPos([c[0], y, c[1]]);
                let ca = a.chunk(cp).unwrap();
                let cb = b.chunk(cp).unwrap();
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(l).unwrap();
                    let sa = b.biome_sample(p.0[0], p.0[2]).unwrap();
                    let (ba, bb) = (ca.get(l), cb.get(l));
                    if p.0[1] <= sa.height {
                        assert_eq!(
                            (ba == Block::Air, ba.fluid()),
                            (bb == Block::Air, bb.fluid()),
                            "geometry {seed} {p:?}"
                        );
                        if p.0[1] < sa.height - 4 {
                            assert_eq!(ba, bb, "deep geology {seed} {p:?}");
                        }
                    }
                    assert_eq!(bb, b.block(p), "raster {seed} {p:?}");
                    if matches!(bb, Block::Leaves | Block::BirchLeaves | Block::SpruceLeaves) {
                        assert!(p.0[1] > sa.height, "buried crown {seed} {p:?}");
                    }
                    if bb.is_plant() {
                        assert!(
                            bb.supported_by(b.block(p.offset([0, -1, 0]).unwrap())),
                            "plant {seed} {p:?}"
                        );
                        assert_eq!(p.0[1], sa.height + 1);
                    }
                    if matches!(bb, Block::Wood | Block::BirchWood | Block::SpruceWood)
                        && b.block(p.offset([0, -1, 0]).unwrap()) != bb
                    {
                        assert_eq!(p.0[1], sa.height + 1, "root {seed} {p:?}");
                        assert!(matches!(
                            b.block(p.offset([0, -1, 0]).unwrap()),
                            Block::Grass | Block::SnowyGround
                        ));
                    }
                }
            }
        }
    }
}
#[test]
fn climate_and_biomes_match_point_halo_reverse_and_parallel_planning() {
    for seed in SEEDS {
        let g = generator(seed, 12);
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
fn climate_interpolation_is_broad_continuous_and_seed_dependent() {
    let mut signatures = std::collections::BTreeSet::new();
    for seed in SEEDS {
        let mut crc = crc32fast::Hasher::new();
        for z in (-31000..31000).step_by(997) {
            for x in (-31000..31000).step_by(887) {
                let c = biome_v2::parameters(seed, x, z).unwrap();
                crc.update(&c.temperature.to_le_bytes());
                crc.update(&c.humidity.to_le_bytes());
                for [dx, dz] in [[1, 0], [0, 1], [16, 0], [0, 16]] {
                    let n = biome_v2::parameters(seed, x + dx, z + dz).unwrap();
                    let limit = if dx + dz == 1 { 12 } else { 100 };
                    assert!(
                        (c.temperature - n.temperature).abs() < limit,
                        "temperature seam {seed} {x} {z}"
                    );
                    assert!(
                        (c.humidity - n.humidity).abs() < limit,
                        "humidity seam {seed} {x} {z}"
                    );
                }
            }
        }
        signatures.insert(crc.finalize());
    }
    assert_eq!(signatures.len(), 4);
}
#[test]
fn append_only_biome_ids_and_v12_fail_closed_identity() {
    for (i, b) in [
        Biome::Plains,
        Biome::Forest,
        Biome::Desert,
        Biome::SnowyPlains,
        Biome::RockyHighlands,
        Biome::Meadow,
        Biome::BirchForest,
        Biome::Taiga,
        Biome::Grove,
        Biome::SnowySlopes,
        Biome::StonyPeaks,
        Biome::Beach,
        Biome::Ocean,
        Biome::River,
        Biome::Lake,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(b as usize, i);
        assert!(!b.name().is_empty());
    }
    assert_eq!(voxel_core::worldgen::CURRENT_LOCAL_GENERATOR, 13);
    for version in [0, 14, u32::MAX] {
        assert!(Generator::new(GenerationConfig { seed: 0, version }).is_err());
    }
    assert!(biome_v2::sample(&generator(42, 11), 0, 0).is_none());
}

#[test]
fn independent_fixed_point_climate_and_snowline_fixtures_match() {
    for line in include_str!("fixtures/climate-v12.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let c = biome_v2::parameters(
            v[0].parse().unwrap(),
            v[1].parse().unwrap(),
            v[2].parse().unwrap(),
        )
        .unwrap();
        let expected: Vec<i32> = v[3..].iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(
            [
                c.temperature,
                c.humidity,
                c.continentalness,
                c.erosion,
                c.weirdness,
                c.snowline(),
                c.treeline(),
                c.elevation
            ],
            expected.as_slice(),
            "{line}"
        );
    }
}

#[test]
fn actual_ecosystems_have_distinct_species_flowers_and_dry_supported_roots() {
    fn patch(seed: u64, c: [i32; 2], target: Biome) -> [usize; 8] {
        let g = generator(seed, 12);
        let mut counts = [0; 8];
        for cz in c[1] - 1..=c[1] + 1 {
            for cx in c[0] - 1..=c[0] + 1 {
                let s = g.sampler([cx, cz]);
                for i in 0..256 {
                    let x = cx * 16 + i % 16;
                    let z = cz * 16 + i / 16;
                    let sample = s.biome_sample(x, z).unwrap();
                    if sample.biome != target {
                        continue;
                    }
                    counts[0] += 1;
                    let p = BlockPos([x, sample.height + 1, z]);
                    let b = s.block(p);
                    let index = match b {
                        Block::Wood => Some(1),
                        Block::BirchWood => Some(2),
                        Block::SpruceWood => Some(3),
                        Block::RedFlower | Block::YellowFlower => Some(4),
                        Block::Fern => Some(5),
                        Block::DeadBush => Some(6),
                        Block::TallGrass => Some(7),
                        _ => None,
                    };
                    if let Some(index) = index {
                        counts[index] += 1;
                        let below = s.block(p.offset([0, -1, 0]).unwrap());
                        if b.is_plant() {
                            assert!(b.supported_by(below));
                        } else {
                            assert!(matches!(below, Block::Grass | Block::SnowyGround));
                        }
                    }
                }
            }
        }
        counts
    }
    let forest = patch(0, [128, -252], Biome::Forest);
    assert!(forest[1] > forest[2] && forest[1] > 0, "{forest:?}");
    let birch = patch(0, [-120, -252], Biome::BirchForest);
    assert!(birch[2] > birch[1] * 2 && birch[2] > 0, "{birch:?}");
    for (seed, c, b) in [
        (0, [100, -252], Biome::Taiga),
        (u64::MAX, [-101, -252], Biome::Grove),
    ] {
        let p = patch(seed, c, b);
        assert!(p[3] > 0 && p[5] > 0, "{b:?}: {p:?}");
        assert_eq!((p[1], p[2]), (0, 0));
    }
    let meadow = patch(0, [208, -116], Biome::Meadow);
    let plains = patch(0, [-220, -252], Biome::Plains);
    assert!(
        meadow[4] * plains[0] > plains[4] * meadow[0] * 2,
        "flowers meadow={meadow:?} plains={plains:?}"
    );
    let desert = patch(0, [-168, -200], Biome::Desert);
    assert_eq!(desert[1..6], [0; 5]);
    assert!(desert[6] > 0);
    for (seed, c, b) in [
        (0, [-60, 156], Biome::SnowySlopes),
        (42, [48, -56], Biome::StonyPeaks),
        (42, [208, -204], Biome::Beach),
    ] {
        let p = patch(seed, c, b);
        assert!(p[0] > 0);
        assert_eq!(p[1..], [0; 7], "{b:?}: {p:?}");
    }
}
