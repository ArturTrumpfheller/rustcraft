// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;
use voxel_core::{
    Block, BlockPos, ChunkPos, LocalPos, World,
    regions::RegionIdentity,
    worldgen::{GenerationConfig, Generator},
};
#[test]
fn full_height_surface_density_water_caves_vegetation_and_halos_agree() {
    for line in include_str!("fixtures/columns-v11.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let seed = v[0].parse().unwrap();
        let c = [v[1].parse().unwrap(), v[2].parse().unwrap()];
        let report = audit::audit(seed, 11, c);
        assert!(report.findings.is_empty(), "{:#?}", report.findings);
        assert_eq!(report.crc, v[3].parse::<u32>().unwrap(), "{line}");
    }
    for seed in [0, 42, 2026, u64::MAX] {
        audit::parallel_parity(seed, 11, [-32, -200]);
    }
}
#[test]
fn v11_spawn_is_dry_supported_and_stable_under_gravity() {
    use voxel_core::{
        physics::{self, MovementInput, PhysicsConfig, Player},
        streaming,
    };
    for seed in [0, 42, 2026, u64::MAX] {
        let mut w = World::regional(RegionIdentity {
            seed,
            generator: 11,
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
        let g = Generator::new(GenerationConfig { seed, version: 11 }).unwrap();
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
fn every_density_band_cell_matches_the_independent_point_path_and_compact_slab_proof() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 11 }).unwrap();
        for c in [[-202, -54], [-32, -200]] {
            let s = g.sampler(c);
            for y in 0..12 {
                let cp = ChunkPos([c[0], y, c[1]]);
                let chunk = s.chunk(cp).unwrap();
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(l).unwrap();
                    assert_eq!(chunk.get(l), s.block(p), "seed={seed} p={p:?}");
                }
            }
        }
    }
}
