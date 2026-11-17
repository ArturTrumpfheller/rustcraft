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
    for (seed, version) in SEEDS
        .into_iter()
        .flat_map(|seed| [14, 15, 16, 17].map(move |v| (seed, v)))
    {
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
            let r = audit::audit(seed, version, c);
            assert!(r.findings.is_empty(), "{:#?}", r.findings);
            assert_eq!(r.underground_water, 0);
        }
        audit::parallel_parity(seed, version, [-8, -8]);
    }
}
#[test]
fn surface_geography_climate_and_material_contracts_are_exactly_v13() {
    for (seed, version) in SEEDS
        .into_iter()
        .flat_map(|seed| [14, 15, 16].map(move |v| (seed, v)))
    {
        let a = Generator::new(GenerationConfig { seed, version: 13 }).unwrap();
        let b = Generator::new(GenerationConfig { seed, version }).unwrap();
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
    assert_eq!(voxel_core::worldgen::CURRENT_LOCAL_GENERATOR, 17);
    for v in [0, 18, u32::MAX] {
        assert!(
            Generator::new(GenerationConfig {
                seed: 0,
                version: v
            })
            .is_err()
        );
    }
    for (seed, version) in SEEDS
        .into_iter()
        .flat_map(|seed| [14, 15, 16, 17].map(move |v| (seed, v)))
    {
        let mut w = World::regional(RegionIdentity {
            seed,
            generator: version,
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

fn assert_cave_gameplay(version: u32) {
    use std::collections::BTreeSet;
    use voxel_core::{
        World,
        inventory::{self, Inventory},
        lighting::Skylight,
        physics::{PhysicsConfig, Player},
        raycast::Ray,
        regions::{RegionIdentity, RegionPos},
    };
    let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
    let mut w = World::regional(RegionIdentity {
        seed: 42,
        generator: version,
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
    // Choose the first supported player position in the fixed center chunk.
    // The candidate's geometry may change; collision/edit/light contracts may not.
    let (px, pz, floor) = (48..64)
        .flat_map(|z| (32..48).map(move |x| (x, z)))
        .find_map(|(x, z)| {
            (-47..-18)
                .rev()
                .find(|&y| {
                    w.block(BlockPos([x, y, z])).is_solid()
                        && w.block(BlockPos([x, y + 1, z])) == Block::Air
                        && w.block(BlockPos([x, y + 2, z])) == Block::Air
                })
                .map(|y| (x, z, y))
        })
        .expect("fixed neighborhood must contain a supported cave floor");
    let cfg = PhysicsConfig::default();
    let mut p = Player::new([
        f64::from(px) + 0.5,
        f64::from(floor + 1),
        f64::from(pz) + 0.5,
    ]);
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
    assert_eq!(w.block(BlockPos([px, floor, pz])), Block::Air);
    assert!(
        inventory::edit_inventory(&mut w, &mut inventory, ray, true, &p, &cfg)
            .unwrap()
            .is_some()
    );
    assert_eq!(w.block(BlockPos([px, floor, pz])), Block::Stone);
    let torch = BlockPos([px, floor + 1, pz]);
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

#[test]
fn v14_post_design_columns_pin_cross_platform_cavity_output_for_future_versions() {
    // Post-design snapshots, separate from the independent mathematical field oracle.
    for line in include_str!("fixtures/columns-v14.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let seed = v[1].parse().unwrap();
        let c = [v[2].parse().unwrap(), v[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version: 14 }).unwrap();
        assert_eq!(
            audit::crc(&audit::column(&g, c)),
            v[4].parse::<u32>().unwrap(),
            "{line}"
        );
        assert_eq!(
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8).unwrap() as u8,
            v[5].parse::<u8>().unwrap()
        );
    }
}

#[test]
fn real_v14_cave_collision_mining_placing_and_torch_light_across_chunk_seams() {
    assert_cave_gameplay(14);
}
#[test]
fn real_v15_cave_collision_mining_placing_and_torch_light_across_chunk_seams() {
    assert_cave_gameplay(15);
}
#[test]
fn v15_post_design_columns_pin_output_without_replacing_v14_fixtures() {
    for line in include_str!("fixtures/columns-v15.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let seed = v[1].parse().unwrap();
        let c = [v[2].parse().unwrap(), v[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version: 15 }).unwrap();
        assert_eq!(
            audit::crc(&audit::column(&g, c)),
            v[4].parse::<u32>().unwrap(),
            "{line}"
        );
        assert_eq!(
            g.biome(c[0] * 16 + 8, c[1] * 16 + 8).unwrap() as u8,
            v[5].parse::<u8>().unwrap()
        );
    }
}

fn assert_saved_cave_edits(version: u32) {
    use voxel_core::{World, persistence::SaveStore, regions::RegionIdentity, streaming};
    let path = std::env::temp_dir().join(format!(
        "rustcraft-slice42-cave-{version}-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let id = RegionIdentity {
        seed: 42,
        generator: version,
        nonce: [42; 16],
    };
    {
        let (store, mut world, _) =
            SaveStore::open_with(&path, || Ok(World::regional(id))).unwrap();
        let inv = store.load_player(&mut world).unwrap();
        store.prepare_regional_column(&mut world, [2, 3]).unwrap();
        streaming::prepare_column(&mut world, [2, 3]).unwrap();
        let g = Generator::new(GenerationConfig {
            seed: id.seed,
            version: id.generator,
        })
        .unwrap();
        let sampler = g.sampler([2, 3]);
        let floor = (48..64)
            .flat_map(|z| (32..48).map(move |x| (x, z)))
            .find_map(|(x, z)| {
                (-48..-18).find_map(|y| {
                    let p = BlockPos([x, y, z]);
                    (sampler.block(p) == Block::Stone
                        && sampler.block(BlockPos([x, y + 1, z])) == Block::Air
                        && sampler.block(BlockPos([x, y + 2, z])) == Block::Air)
                        .then_some(p)
                })
            })
            .expect("fixed region contains a real v15 supported underground floor");
        let torch = BlockPos([floor.0[0], floor.0[1] + 1, floor.0[2]]);
        world.set_block(torch, Block::Torch).unwrap();
        let mined = BlockPos([floor.0[0], floor.0[1] - 1, floor.0[2]]);
        assert_eq!(world.block(mined), Block::Stone);
        world.set_block(mined, Block::Air).unwrap();
        let placed = BlockPos([floor.0[0], floor.0[1] + 2, floor.0[2]]);
        world.set_block(placed, Block::BirchWood).unwrap();
        store.save_player_session(&world, &inv, true).unwrap();
        let manifest = std::fs::read(store.path()).unwrap();
        world.evict_column([2, 3]);
        streaming::prepare_column(&mut world, [2, 3]).unwrap();
        for (p, b) in [
            (torch, Block::Torch),
            (mined, Block::Air),
            (placed, Block::BirchWood),
        ] {
            assert_eq!(world.block(p), b);
        }
        drop(store);
        let (store, mut restored, _) =
            SaveStore::open_with(&path, || panic!("saved identity must be loaded")).unwrap();
        assert_eq!(restored.region_identity(), Some(id));
        let _ = store.load_player(&mut restored).unwrap();
        store
            .prepare_regional_column(&mut restored, [2, 3])
            .unwrap();
        streaming::prepare_column(&mut restored, [2, 3]).unwrap();
        for (p, b) in [
            (torch, Block::Torch),
            (mined, Block::Air),
            (placed, Block::BirchWood),
        ] {
            assert_eq!(restored.block(p), b);
        }
        assert_eq!(std::fs::read(store.path()).unwrap(), manifest);
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn refined_caves_keep_narrow_routes_broad_spaces_and_separate_air_and_floor_graphs() {
    for (seed, min) in [(42, [1184, 1136]), (u64::MAX, [-256, -256])] {
        let v = quality::Volume::new(seed, 15, min, 128);
        let mask = v.underground();
        let count = mask.iter().filter(|&&v| v != 0).count();
        assert!(
            (20_000..700_000).contains(&count),
            "retain substantial rock: {seed} {count}"
        );
        let air = v.components(mask, false);
        assert!(air[0].cells > 10_000);
        assert!(air[0].max[1] - air[0].min[1] >= 32);
        assert!(
            air.iter().skip(1).any(|c| c.cells < 2048),
            "not every cavity belongs to the main system"
        );
        let floors = v.components(v.standing(), true);
        assert!(floors[0].cells > 1000);
        assert!(
            floors[0].cells < air[0].cells,
            "air volume does not establish walkability"
        );
        assert!(v.geodesic(v.standing(), true) > 80);
        let (widths, heights) = v.widths();
        assert!(
            widths[..3].iter().sum::<usize>() > 100,
            "narrow passages remain"
        );
        assert!(
            widths[3..].iter().sum::<usize>() > 100,
            "broader spaces remain"
        );
        assert!(
            heights[3..].iter().sum::<usize>() > 100,
            "occasional tall spaces remain"
        );
    }
}

#[test]
fn v15_real_underground_edits_keep_saved_generator_identity() {
    assert_saved_cave_edits(15);
}
#[test]
fn v16_real_underground_edits_keep_saved_generator_identity() {
    assert_saved_cave_edits(16);
}
#[test]
fn real_v16_cave_collision_mining_placing_and_torch_light_across_chunk_seams() {
    assert_cave_gameplay(16);
}

#[test]
fn v17_real_cave_collision_mining_placing_and_torch_light_across_chunk_seams() {
    assert_cave_gameplay(17);
}
#[test]
fn v17_saved_underground_edits_remain_authoritative_after_evict_and_reopen() {
    assert_saved_cave_edits(17);
}
#[test]
fn v16_post_design_columns_pin_output_without_replacing_historical_fixtures() {
    for line in include_str!("fixtures/columns-v16.csv").lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let seed = v[1].parse().unwrap();
        let c = [v[2].parse().unwrap(), v[3].parse().unwrap()];
        let g = Generator::new(GenerationConfig { seed, version: 16 }).unwrap();
        assert_eq!(
            audit::crc(&audit::column(&g, c)),
            v[4].parse::<u32>().unwrap(),
            "{line}"
        );
    }
}

#[test]
fn v16_keeps_deep_voxels_and_only_reduces_cavities_in_the_surface_band() {
    for seed in [42, u64::MAX] {
        let old = Generator::new(GenerationConfig { seed, version: 15 }).unwrap();
        let new = Generator::new(GenerationConfig { seed, version: 16 }).unwrap();
        for c in [[-13, 5], [-12, -12], [74, 71], [-1511, 1250]] {
            let a = audit::column(&old, c);
            let b = audit::column(&new, c);
            let minimum_height = (-4..=20)
                .flat_map(|z| (-4..=20).map(move |x| (x, z)))
                .map(|(x, z)| old.height(c[0] * 16 + x, c[1] * 16 + z).unwrap())
                .min()
                .unwrap();
            for ((cp, before), (_, after)) in a.iter().zip(&b) {
                for i in 0..4096 {
                    let l = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(l).unwrap();
                    if p.0[1] <= minimum_height - 20 {
                        assert_eq!(before.get(l), after.get(l), "deep voxel {seed} {p:?}");
                    }
                    if before.get(l).is_solid() && after.get(l) == Block::Air {
                        assert!(
                            matches!(
                                before.get(l),
                                Block::Leaves | Block::BirchLeaves | Block::SpruceLeaves
                            ),
                            "new cavity {seed} {p:?}"
                        );
                    }
                }
            }
        }
    }
}
