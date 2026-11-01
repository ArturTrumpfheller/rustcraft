// SPDX-License-Identifier: MIT OR Apache-2.0
#[path = "../examples/support/worldgen_audit.rs"]
mod audit;

#[test]
fn bounded_multi_seed_v9_integration_audit() {
    let mut biomes = std::collections::BTreeSet::new();
    let (mut plants, mut water, mut caves, mut pools) = (0, 0, 0, 0);
    for seed in audit::SEEDS {
        for (_, c) in audit::SITES {
            let r = audit::audit(seed, 9, c);
            assert!(r.findings.is_empty(), "{:#?}", r.findings);
            // Captured with this harness against exact merged Slice29, before
            // changing any generator: see raw/audit-slice29.csv and its method.
            let golden = match (seed, c) {
                (0, [-136, -256]) => Some(3_390_200_405),
                (42, [-28, 1]) => Some(2_101_020_021),
                (2026, [216, -248]) => Some(3_540_832_644),
                _ => None,
            };
            if let Some(expected) = golden {
                assert_eq!(r.crc, expected);
            }
            assert!(r.voxel_bytes <= 9 * 16 * 4096 * std::mem::size_of::<voxel_core::Block>());
            biomes.insert(r.biome);
            plants += r.plants;
            water += r.water;
            caves += r.cave_air;
            pools += r.underground_water;
        }
        audit::parallel_parity(seed, 9, [-4, -1]);
    }
    assert_eq!(biomes.len(), 5);
    assert!(plants > 0 && water > 0 && caves > 0 && pools > 0);
}

#[test]
fn all_released_generators_keep_parallel_signed_seams_and_eviction_parity() {
    for version in 1..=9 {
        for seed in audit::SEEDS {
            audit::parallel_parity(seed, version, [-1, 0]);
        }
    }
}
