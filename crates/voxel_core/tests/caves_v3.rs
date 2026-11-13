// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator},
};
const SEEDS: [u64; 4] = [0, 42, 2026, u64::MAX];
#[test]
fn exact_merged_slice38_history_including_all_v13_geometry_and_water() {
    for line in include_str!("fixtures/history-v1-v13.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let version = v[0].parse().unwrap();
        let seed = v[1].parse().unwrap();
        let c = [v[2].parse().unwrap(), v[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        assert_eq!(
            audit::crc(&audit::column(&g, c)),
            v[4].parse::<u32>().unwrap(),
            "{line}"
        );
        assert_eq!(
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8)
                .map_or(255, |b| b as u8),
            v[5].parse::<u8>().unwrap()
        );
    }
}
#[test]
fn all_paths_halos_foundation_supported_ecology_quiet_water_and_signed_bounds() {
    for seed in SEEDS {
        for c in [
            [-1, -1],
            [-8, -8],
            [-4, -1],
            [-32, -200],
            [-1511, 1250],
            [1750, -1875],
            [-2048, 2047],
            [2047, -2048],
        ] {
            let r = audit::audit(seed, 14, c);
            assert!(r.findings.is_empty(), "{:#?}", r.findings);
            assert_eq!(r.underground_water, 0);
        }
        audit::parallel_parity(seed, 14, [-8, -8]);
    }
}
#[test]
fn surface_geography_climate_and_material_contracts_are_exactly_v13() {
    for seed in SEEDS {
        let a = Generator::new(GenerationConfig { seed, version: 13 }).unwrap();
        let b = Generator::new(GenerationConfig { seed, version: 14 }).unwrap();
        for c in [[-1, -1], [-32, -200], [1750, -1875], [-1511, 1250]] {
            let s = a.sampler(c);
            let t = b.sampler(c);
            for z in -1..=16 {
                for x in -1..=16 {
                    let xx = c[0] * 16 + x;
                    let zz = c[1] * 16 + z;
                    assert_eq!(s.biome_sample(xx, zz), t.biome_sample(xx, zz));
                }
            }
            for y in [-4, -3, -2, -1, 0, 1, 3, 6, 9, 11] {
                let cp = ChunkPos([c[0], y, c[1]]);
                let ch = t.chunk(cp).unwrap();
                for l in [[0, 0, 0], [15, 15, 15], [7, 8, 9]] {
                    let lp = LocalPos::new(l).unwrap();
                    let p = cp.block_position(lp).unwrap();
                    assert_eq!(ch.get(lp), t.block(p), "{seed} {p:?}");
                    assert_eq!(ch.get(lp), b.block(p), "point {seed} {p:?}");
                }
            }
        }
        for p in [
            BlockPos([i32::MIN, 0, 0]),
            BlockPos([32768, 0, 0]),
            BlockPos([0, 192, 0]),
            BlockPos([0, -65, 0]),
        ] {
            assert_eq!(b.block(p), Block::Air);
        }
    }
}
#[test]
fn v14_safe_spawn_and_default_fail_closed() {
    use voxel_core::{
        World,
        physics::{self, MovementInput, PhysicsConfig, Player},
        regions::RegionIdentity,
        streaming,
    };
    assert_eq!(voxel_core::worldgen::CURRENT_LOCAL_GENERATOR, 14);
    for v in [0, 15, u32::MAX] {
        assert!(
            Generator::new(GenerationConfig {
                seed: 0,
                version: v
            })
            .is_err()
        );
    }
    for seed in SEEDS {
        let mut w = World::regional(RegionIdentity {
            seed,
            generator: 14,
            nonce: [39; 16],
        });
        streaming::prepare_spawn(&mut w).unwrap();
        let cfg = PhysicsConfig::default();
        let pos = physics::spawn_position(&w, &cfg, [0.5, 40., 10.5]);
        let mut p = Player::new(pos);
        assert!(!p.in_water(&w, &cfg));
        for _ in 0..120 {
            p.tick(&w, MovementInput::default(), &cfg);
        }
        assert!((p.position[1] - pos[1]).abs() < 0.1);
    }
}

#[test]
fn real_v14_cave_collision_mining_placing_and_torch_light_across_chunk_seams() {
    use std::collections::BTreeSet;
    use voxel_core::{
        World,
        inventory::{self, Inventory},
        lighting::Skylight,
        physics::{PhysicsConfig, Player},
        raycast::Ray,
        regions::{RegionIdentity, RegionPos},
    };
    let g = Generator::new(GenerationConfig {
        seed: 42,
        version: 14,
    })
    .unwrap();
    let mut w = World::regional(RegionIdentity {
        seed: 42,
        generator: 14,
        nonce: [39; 16],
    });
    for z in 2..=4 {
        for x in 1..=3 {
            w.install_region(
                RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            let s = g.sampler([x, z]);
            for y in g.world_height().chunks() {
                let cp = ChunkPos([x, y, z]);
                w.insert_chunk(cp, s.chunk(cp).unwrap());
            }
        }
    }
    let floor = (-59..-30)
        .rev()
        .find(|&y| {
            w.block(BlockPos([36, y, 61])).is_solid()
                && w.block(BlockPos([36, y + 1, 61])) == Block::Air
                && w.block(BlockPos([36, y + 2, 61])) == Block::Air
        })
        .unwrap();
    let cfg = PhysicsConfig::default();
    let mut p = Player::new([36.5, f64::from(floor + 1), 61.5]);
    for _ in 0..120 {
        p.tick(&w, Default::default(), &cfg);
    }
    assert!(p.grounded);
    assert!((p.position[1] - f64::from(floor + 1)).abs() < 0.01);
    let mut inventory = Inventory::empty();
    let ray = Ray {
        origin: p.eye_position(&cfg),
        direction: [0., -1., 0.],
        reach: 5.,
    };
    assert!(
        inventory::edit_inventory(&mut w, &mut inventory, ray, false, &p, &cfg)
            .unwrap()
            .is_some()
    );
    assert_eq!(w.block(BlockPos([36, floor, 61])), Block::Air);
    assert!(
        inventory::edit_inventory(&mut w, &mut inventory, ray, true, &p, &cfg)
            .unwrap()
            .is_some()
    );
    assert_eq!(w.block(BlockPos([36, floor, 61])), Block::Stone);
    let torch = BlockPos([36, floor + 1, 61]);
    w.set_block(torch, Block::Torch).unwrap();
    let targets: BTreeSet<_> = w.chunks().map(|(cp, _)| cp).collect();
    let full = Skylight::for_chunks(&w, &targets);
    let left = BTreeSet::from([ChunkPos([2, -3, 3]), ChunkPos([2, -2, 3])]);
    let local = Skylight::for_chunks(&w, &left);
    assert_eq!(full.block_level(torch), 14);
    for y in -48..-16 {
        for z in 48..64 {
            for x in 32..48 {
                let p = BlockPos([x, y, z]);
                assert_eq!(
                    (full.level(p), full.block_level(p)),
                    (local.level(p), local.block_level(p)),
                    "{p:?}"
                );
            }
        }
    }
    let cp = ChunkPos([2, -3, 3]);
    let mesh = voxel_core::mesh_chunk_lit(&w, cp, &local);
    assert!(!mesh.indices.is_empty());
    assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
}

#[allow(dead_code)]
#[path = "../examples/support/cave_quality.rs"]
mod quality;
#[test]
fn connected_air_and_player_floor_graph_measure_real_multi_chunk_complexes() {
    let v = quality::Volume::new(u64::MAX, 14, [-256, -256], 128);
    let mask = v.underground();
    let count = mask.iter().filter(|v| **v != 0).count();
    let components = v.components(mask, false);
    assert!(
        count > 40000 && count < 300000,
        "bounded complex keeps substantial rock: {count}"
    );
    assert!(components[0].cells > count / 2);
    assert!(components[0].max[1] - components[0].min[1] > 48);
    let walk = v.components(v.standing(), true);
    assert!(walk[0].cells > 1000);
    assert!(
        walk[0].cells < components[0].cells,
        "air volume is not a navigation claim"
    );
    assert!(
        v.geodesic(v.standing(), true) > 48,
        "actual floor graph crosses several chunks"
    );
    let entrance = quality::Volume::new(2026, 14, [0, -256], 128);
    let components = entrance.components(entrance.underground(), false);
    assert!(
        components
            .iter()
            .any(|c| c.surface && c.min[1] < 54 && c.cells > 2000),
        "actual surface air must continue underground: {components:?}"
    );
}
