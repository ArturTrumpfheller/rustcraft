// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos, World,
    regions::RegionIdentity,
    worldgen::{GenerationConfig, Generator, terrain_v3},
};
#[test]
fn independently_computed_integer_height_goldens() {
    for line in include_str!("fixtures/terrain-v10.csv").lines().skip(1) {
        let values: Vec<_> = line.split(',').collect();
        let seed = values[0].parse().unwrap();
        let x = values[1].parse().unwrap();
        let z = values[2].parse().unwrap();
        assert_eq!(
            terrain_v3::height(seed, x, z),
            values[3].parse::<i32>().unwrap(),
            "{line}"
        );
    }
}
#[test]
fn macro_geography_has_highlands_valleys_buildable_land_and_headroom() {
    for seed in [0, 42, 2026, u64::MAX] {
        let mut peaks = 0;
        let mut flat = 0;
        let mut valleys = 0;
        let mut highest = 0;
        for z in (-4096..4096).step_by(64) {
            for x in (-4096..4096).step_by(64) {
                let h = terrain_v3::height(seed, x, z);
                highest = highest.max(h);
                peaks += usize::from(h >= 70);
                let patch: Vec<_> = [0, 16, 32]
                    .into_iter()
                    .flat_map(|dz| {
                        [0, 16, 32]
                            .into_iter()
                            .map(move |dx| terrain_v3::height(seed, x + dx, z + dz))
                    })
                    .collect();
                flat += usize::from(
                    h >= 4 && patch.iter().max().unwrap() - patch.iter().min().unwrap() <= 1,
                );
                valleys += usize::from(
                    terrain_v3::height(seed, x - 256, z).min(terrain_v3::height(seed, x + 256, z))
                        - h
                        >= 30,
                );
                assert!((-40..180).contains(&h));
                for [dx, dz] in [[1, 0], [0, 1]] {
                    assert!((terrain_v3::height(seed, x + dx, z + dz) - h).abs() <= 3);
                }
            }
        }
        assert!(
            highest >= 140 && peaks > 300 && flat > 2000 && valleys > 20,
            "seed={seed} peak={highest} mountain={peaks} flat={flat} valley={valleys}"
        );
    }
}
#[test]
fn full_height_surface_water_vegetation_caves_and_halos_agree() {
    for line in include_str!("fixtures/columns-v10.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let seed = v[0].parse().unwrap();
        let c = [v[1].parse().unwrap(), v[2].parse().unwrap()];
        let expected_crc = v[3].parse::<u32>().unwrap();
        let r = audit::audit(seed, 10, c);
        assert!(r.findings.is_empty(), "{:#?}", r.findings);
        assert_eq!(r.crc, expected_crc, "seed={seed} column={c:?}");
        if c == [-7, -2] {
            assert!(r.underground_water > 0 && r.cave_air > 0);
        }
        audit::parallel_parity(seed, 10, c);
    }
}
#[test]
fn water_beds_are_supported_and_do_not_leak_into_surface_caves() {
    let mut water = 0;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 10 }).unwrap();
        for c in [[-160, -256], [-28, 1], [6, 6], [-7, -30]] {
            let s = g.sampler(c);
            for y in -2..=0 {
                let cp = ChunkPos([c[0], y, c[1]]);
                let chunk = s.chunk(cp).unwrap();
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(l).unwrap();
                    if chunk.get(l) != Block::WaterSource || p.0[1] > 2 {
                        continue;
                    }
                    water += 1;
                    for d in [[0, -1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]] {
                        let n = p.offset(d).unwrap();
                        let b = s.block(n);
                        assert!(
                            b.is_solid() || b.fluid().is_some(),
                            "seed={seed} leak {p:?} to {n:?} {b:?}"
                        );
                    }
                }
            }
        }
    }
    assert!(water > 1000);
}
#[test]
fn protected_spawn_is_dry_supported_and_collision_safe_for_all_seeds() {
    use voxel_core::{
        physics::{self, MovementInput, PhysicsConfig, Player},
        streaming,
    };
    for seed in [0, 42, 2026, u64::MAX] {
        let mut w = World::regional(RegionIdentity {
            seed,
            generator: 10,
            nonce: [35; 16],
        });
        streaming::prepare_spawn(&mut w).unwrap();
        let config = PhysicsConfig::default();
        let pos = physics::spawn_position(&w, &config, [0.5, 40., 10.5]);
        let mut player = Player::new(pos);
        let ground = BlockPos([
            pos[0].floor() as i32,
            pos[1].floor() as i32 - 1,
            pos[2].floor() as i32,
        ]);
        assert!(w.block(ground).is_solid());
        assert!(!player.in_water(&w, &config));
        for _ in 0..120 {
            player.tick(&w, MovementInput::default(), &config);
        }
        assert!((player.position[1] - pos[1]).abs() < 0.1);
    }
}

#[test]
fn bounded_admission_cache_eviction_preserves_uncached_geography() {
    use voxel_core::worldgen::hydrology;
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 10 }).unwrap();
        for i in 0..160 {
            let x = i * 128 - 10240;
            let z = (i * 509) % 16384 - 8192;
            assert_eq!(
                g.height(x, z).unwrap(),
                hydrology::surface_v10(seed, x, z).bed
            );
            assert!(g.terrain_admission_cache_entries() <= 128);
        }
        assert_eq!(g.terrain_admission_cache_entries(), 128);
        assert_eq!(
            g.height(-10240, -8192).unwrap(),
            hydrology::surface_v10(seed, -10240, -8192).bed
        );
    }
}

#[test]
fn widened_compatibility_banks_remove_observed_steep_rims() {
    for (seed, x, z) in [(0, -3776, -3648), (u64::MAX, 960, -704)] {
        let g = Generator::new(GenerationConfig { seed, version: 10 }).unwrap();
        let h = g.height(x, z).unwrap();
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            assert!(
                (g.height(x + dx, z + dz).unwrap() - h).abs() <= 3,
                "{seed} {x} {z}"
            );
        }
    }
}
