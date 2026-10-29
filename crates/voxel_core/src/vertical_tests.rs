//! Slice 26 regressions. Historical golden fixtures live in their original modules.
use crate::{
    Block, BlockPos, Chunk, ChunkPos, LocalPos, World,
    bounds::WorldHeight,
    interaction::EditError,
    lighting::Skylight,
    regions::{RegionIdentity, RegionPos},
    streaming::{self, TerrainCache},
    worldgen::{GenerationConfig, Generator},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
fn stable_id(b: Block) -> u16 {
    match b {
        Block::Air => 0,
        Block::Grass => 1,
        Block::Dirt => 2,
        Block::Stone => 3,
        Block::Wood => 4,
        Block::Leaves => 5,
        Block::Sand => 20,
        Block::SnowyGround => 21,
        _ => panic!("no generated fluids or emitters"),
    }
}
fn world(version: u32) -> World {
    World::regional(RegionIdentity {
        seed: 42,
        generator: version,
        nonce: [26; 16],
    })
}
fn empty_area(w: &mut World, radius: i32) {
    for x in -radius..=radius {
        for z in -radius..=radius {
            w.install_region(
                RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Arc::new(BTreeMap::new()),
            );
            for y in w.world_height().chunks() {
                w.insert_chunk(ChunkPos([x, y, z]), Chunk::default());
            }
        }
    }
}
fn light(w: &World) -> Skylight {
    Skylight::for_chunks(w, &BTreeSet::from([ChunkPos([0, 0, 0])]))
}
#[test]
fn identities_define_exclusive_bounds_and_old_worlds_never_expand() {
    for version in 1..=7 {
        let h = WorldHeight::for_generator(version).unwrap();
        assert_eq!(
            h,
            if version >= 6 {
                WorldHeight::EXTENDED
            } else {
                WorldHeight::HISTORICAL
            }
        );
        assert_eq!(h.chunk_count(), if version >= 6 { 16 } else { 3 });
        assert!(h.contains(h.min));
        assert!(h.contains(h.max - 1));
        assert!(!h.contains(h.min - 1));
        assert!(!h.contains(h.max));
        let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        assert_eq!(g.world_height(), h);
        assert!(g.chunk(ChunkPos([0, h.chunks().start - 1, 0])).is_none());
        assert!(g.chunk(ChunkPos([0, h.chunks().end, 0])).is_none());
    }
    for version in 3..=8 {
        let w = world(version);
        assert_eq!(w.bounds().0[0], -32768);
        assert_eq!(w.bounds().1[2], 32768);
        assert_eq!(w.bounds().0[1], w.world_height().min);
        assert_eq!(w.bounds().1[1], w.world_height().max);
        assert_eq!(
            w.region_identity().unwrap().format_version(),
            if version == 3 { 4 } else { 5 }
        );
    }
    assert_eq!(
        streaming::new_large_world(42).identity().unwrap().generator,
        8
    );
    for v in [0, 9, u32::MAX] {
        assert!(WorldHeight::for_generator(v).is_err());
        assert!(
            Generator::new(GenerationConfig {
                seed: 0,
                version: v
            })
            .is_err()
        );
    }
}
#[test]
fn v6_matches_frozen_v5_surface_and_point_chunk_sampling_in_any_order() {
    for seed in [0, 42, u64::MAX] {
        let old = Generator::new(GenerationConfig { seed, version: 5 }).unwrap();
        let new = Generator::new(GenerationConfig { seed, version: 6 }).unwrap();
        for c in [[0, 0], [-1, -1], [-256, -256], [2047, -2048]] {
            let s = new.sampler(c);
            for cy in WorldHeight::EXTENDED.chunks().rev() {
                let cp = ChunkPos([c[0], cy, c[1]]);
                let chunk = s.chunk(cp).unwrap();
                assert_eq!(chunk, new.chunk(cp).unwrap());
                if WorldHeight::HISTORICAL.chunks().contains(&cy) {
                    assert_eq!(chunk, old.chunk(cp).unwrap());
                } else {
                    assert_eq!(chunk.allocated_bytes(), 0);
                    assert_eq!(
                        chunk.uniform(),
                        Some(if cy < -1 { Block::Stone } else { Block::Air })
                    );
                }
                for coords in [[0, 0, 0], [15, 15, 15], [0, 15, 0], [15, 0, 15], [7, 8, 9]] {
                    let lp = LocalPos::new(coords).unwrap();
                    let p = cp.block_position(lp).unwrap();
                    assert_eq!(chunk.get(lp), new.block(p), "{seed} {p:?}");
                    assert_eq!(chunk.get(lp), s.block(p));
                }
            }
        }
    }
}
#[test]
fn v6_independent_column_goldens() {
    let mut actual = Vec::new();
    for (seed, c) in [
        (0, [0, 0]),
        (42, [-1, 1]),
        (2026, [-2048, 2047]),
        (u64::MAX, [173, -261]),
    ] {
        let g = Generator::new(GenerationConfig { seed, version: 6 }).unwrap();
        let s = g.sampler(c);
        let mut hash = crc32fast::Hasher::new();
        for cy in g.world_height().chunks() {
            let chunk = s.chunk(ChunkPos([c[0], cy, c[1]])).unwrap();
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        hash.update(
                            &stable_id(chunk.get(LocalPos::new([x, y, z]).unwrap())).to_le_bytes(),
                        );
                    }
                }
            }
        }
        actual.push(hash.finalize());
    }
    assert_eq!(actual, vec![427164329, 4240216072, 4166722754, 988062637]);
}
#[test]
fn ready_air_materializes_and_last_removal_stays_ready_across_eviction() {
    let mut w = world(6);
    let p = BlockPos([1, 128, 1]);
    assert!(!w.ready(p));
    assert_eq!(w.block(p), Block::Stone);
    assert_eq!(w.set_block(p, Block::Wood), Err(EditError::NotLoaded));
    w.install_region(RegionPos([0, 0]), Arc::new(BTreeMap::new()));
    let g = streaming::prepare_spawn(&mut w).unwrap();
    assert!(w.ready(p));
    assert_eq!(w.block(p), Block::Air);
    let bytes = w.allocated_voxel_bytes();
    w.set_block(p, Block::Wood).unwrap();
    assert_eq!(w.allocated_voxel_bytes(), bytes + 4096);
    w.set_block(p, Block::Air).unwrap();
    assert!(w.ready(p));
    assert_eq!(w.allocated_voxel_bytes(), bytes);
    let deep = BlockPos([1, -48, 1]);
    w.set_block(deep, Block::Air).unwrap();
    w.evict_column([0, 0]);
    assert!(!w.ready(p));
    assert!(!w.ready(deep));
    let r = streaming::build_column(&g, w.overlay_snapshot(), [0, 0], w.revision());
    for (p, c, _) in r.chunks {
        w.insert_chunk(p, c);
    }
    assert_eq!(w.block(p), Block::Air);
    assert_eq!(w.block(deep), Block::Air);
    assert!(w.ready(p));
    assert!(w.ready(deep));
    assert_eq!(w.overlay_snapshot().get(&p), Some(&Block::Air));
    assert!(w.ready(BlockPos([1, 192, 1])));
    assert_eq!(w.block(BlockPos([1, 192, 1])), Block::Air);
    assert_eq!(
        w.set_block(BlockPos([1, 192, 1]), Block::Wood),
        Err(EditError::OutOfBounds)
    );
}
#[test]
fn sparse_sky_and_deep_slabs_do_not_multiply_surface_work_or_cache_bytes() {
    let mut results = Vec::new();
    for version in [5, 6] {
        let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        let mut cache = TerrainCache::default();
        let r = streaming::build_column_cached(
            &g,
            World::default(),
            Default::default(),
            [0, 0],
            0,
            &mut cache,
        );
        assert_eq!(cache.columns(), 9);
        assert!(
            r.chunks
                .iter()
                .filter(|(p, _, _)| p.0[1] >= 2)
                .all(|(_, c, m)| c.is_empty() && c.allocated_bytes() == 0 && m.indices.is_empty())
        );
        assert!(r.lights.keys().all(|p| p.0[1] < 2));
        results.push((
            r.lighting.cells,
            cache.allocated_voxel_bytes(),
            r.chunks
                .iter()
                .map(|(_, c, _)| c.allocated_bytes())
                .sum::<usize>(),
        ));
    }
    assert!(results[1].0 <= results[0].0 * 2, "{results:?}");
    assert_eq!(results[0].1, results[1].1);
    assert_eq!(results[0].2, results[1].2);
    let mut w = world(6);
    empty_area(&mut w, 1);
    let l = light(&w);
    assert_eq!(l.cells(), 0);
    for y in [-64, -48, 0, 32, 64, 128, 176, 191, 192] {
        assert_eq!(l.level(BlockPos([0, y, 0])), 15);
    }
    let pending = world(6);
    assert_eq!(light(&pending).level(BlockPos([0, 176, 0])), 0);
}
#[test]
fn deep_and_high_torches_occlusion_and_vertical_seams_relight_exactly() {
    for base in [-64, -48, -32, -16, 0, 16, 32, 64, 128, 176] {
        let mut w = world(6);
        empty_area(&mut w, 1);
        for x in 0..=4 {
            for z in 0..=4 {
                for y in base..(base + 5).min(192) {
                    if x == 0 || x == 4 || z == 0 || z == 4 || y == base || y == base + 4 {
                        w.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
                    }
                }
            }
        }
        let torch = BlockPos([2, base + 1, 2]);
        assert_eq!(light(&w).level(torch), 0);
        w.set_block(torch, Block::Torch).unwrap();
        let l = light(&w);
        assert_eq!(l.block_level(torch), 14);
        assert_eq!(l.block_level(BlockPos([2, base + 2, 2])), 13);
        w.set_block(torch, Block::Air).unwrap();
        assert_eq!(light(&w).block_level(torch), 0);
        w.set_block(BlockPos([2, base + 4, 2]), Block::Air).unwrap();
        assert_eq!(light(&w).level(torch), 15);
        let l = light(&w);
        // All chunk sample halos match the common field at every Y transition.
        for cy in [base.div_euclid(16), (base + 4).div_euclid(16)] {
            let cp = ChunkPos([0, cy, 0]);
            let samples = l.chunk(cp);
            for ly in -1..=16 {
                let y = cy * 16 + ly;
                assert_eq!(
                    samples.0[((ly + 1) * 18 * 18 + 3 * 18 + 3) as usize] & 15,
                    l.level(BlockPos([2, y, 2]))
                );
            }
        }
    }
}
#[test]
fn expanded_mesh_culls_vertical_faces_and_invalidates_neighbor_revisions() {
    for seam in [-48, -32, -16, 0, 16, 32, 64, 128, 176] {
        let mut w = world(6);
        empty_area(&mut w, 1);
        let a = BlockPos([8, seam - 1, 8]);
        let b = BlockPos([8, seam, 8]);
        w.set_block(a, Block::Stone).unwrap();
        w.set_block(b, Block::Stone).unwrap();
        for p in [a, b] {
            assert_eq!(crate::mesh_chunk(&w, p.split().0).indices.len(), 30);
        }
        let token = w.mesh_revision(a.split().0);
        w.set_block(b, Block::Air).unwrap();
        assert_ne!(w.mesh_revision(a.split().0), token);
        assert_eq!(crate::mesh_chunk(&w, a.split().0).indices.len(), 36);
        w.set_block(b, Block::WaterSource).unwrap();
        let m = crate::mesh_chunk_lit(&w, b.split().0, &light(&w));
        assert!(!m.water.indices.is_empty());
    }
}
#[test]
fn water_crosses_all_vertical_seams_stops_at_floor_and_recedes_after_removal() {
    let mut w = world(6);
    empty_area(&mut w, 1);
    // A narrow closed shaft keeps this a bounded vertical flow workload.
    for y in -64..192 {
        for [x, z] in [[7, 8], [9, 8], [8, 7], [8, 9]] {
            w.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
        }
    }
    let source = BlockPos([8, 191, 8]);
    w.set_block(source, Block::WaterSource).unwrap();
    let mut changes = Vec::new();
    for _ in 0..1500 {
        w.tick_fluids(Default::default(), &mut changes);
        if w.fluid_updates().pending() == 0 {
            break;
        }
    }
    for y in -64..191 {
        assert_eq!(w.block(BlockPos([8, y, 8])), Block::WaterFalling, "y={y}");
    }
    assert_eq!(w.fluid_updates().pending(), 0);
    w.set_block(source, Block::Air).unwrap();
    for _ in 0..1500 {
        w.tick_fluids(Default::default(), &mut changes);
        if w.fluid_updates().pending() == 0 {
            break;
        }
    }
    for y in -64..192 {
        assert_eq!(w.block(BlockPos([8, y, 8])), Block::Air);
    }
    assert_eq!(w.fluid_updates().pending(), 0);
}
#[test]
fn collision_jump_targeting_and_pending_barriers_work_at_extended_heights() {
    use crate::physics::{MovementInput, PhysicsConfig, Player};
    let cfg = PhysicsConfig::default();
    for y in [-63, -48, -32, 32, 64, 128, 176] {
        let mut w = world(6);
        empty_area(&mut w, 1);
        for x in 0..5 {
            for z in 0..5 {
                w.set_block(BlockPos([x, y - 1, z]), Block::Stone).unwrap();
            }
        }
        let mut player = Player::new([2.5, f64::from(y), 2.5]);
        for _ in 0..60 {
            player.tick(&w, MovementInput::default(), &cfg);
        }
        assert!((player.position[1] - f64::from(y)).abs() < 1e-6);
        assert!(player.grounded);
        player.tick(
            &w,
            MovementInput {
                jump: true,
                ..Default::default()
            },
            &cfg,
        );
        assert!(player.position[1] > f64::from(y));
        for _ in 0..120 {
            player.tick(&w, MovementInput::default(), &cfg);
        }
        assert!(player.grounded);
        let ray = crate::raycast::Ray {
            origin: [2.5, f64::from(y) + 2., 2.5],
            direction: [0., -1., 0.],
            reach: 5.,
        };
        assert_eq!(
            crate::raycast::raycast(&w, ray).unwrap().position,
            BlockPos([2, y - 1, 2])
        );
    }
    let mut pending = world(6);
    pending.insert_chunk(ChunkPos([0, -3, 0]), Chunk::default());
    let mut player = Player::new([2.5, -47., 2.5]);
    for _ in 0..120 {
        player.tick(&pending, MovementInput::default(), &cfg);
    }
    assert!(player.position[1] >= -48.); // pending lower slab is a solid barrier
    assert_eq!(WorldHeight::EXTENDED.recovery_y(), -80.);
}

#[test]
fn sparse_light_matches_full_256_plane_reference_including_shadows_and_pending() {
    let mut w = world(6);
    let g = Generator::new(GenerationConfig {
        seed: 42,
        version: 6,
    })
    .unwrap();
    for c in streaming::desired_with_limit([0, 0], 1, g.chunk_limit()) {
        w.install_region(
            RegionPos::of(BlockPos([c[0] * 16, 0, c[1] * 16])),
            Arc::new(BTreeMap::new()),
        );
        let s = g.sampler(c);
        for y in g.world_height().chunks() {
            let p = ChunkPos([c[0], y, c[1]]);
            w.insert_chunk(p, s.chunk(p).unwrap());
        }
    }
    for y in -60..15 {
        w.set_block(BlockPos([8, y, 8]), Block::Air).unwrap();
    }
    for x in 4..12 {
        for z in 4..12 {
            w.set_block(BlockPos([x, 128, z]), Block::Stone).unwrap();
        }
    }
    w.set_block(BlockPos([8, -60, 8]), Block::Stone).unwrap();
    w.set_block(BlockPos([8, -59, 8]), Block::Torch).unwrap();
    w.set_block(BlockPos([8, 129, 8]), Block::Torch).unwrap();
    w.set_block(BlockPos([9, 100, 9]), Block::WaterSource)
        .unwrap();
    let targets = BTreeSet::from([ChunkPos([0, 0, 0])]);
    for pending in [false, true] {
        if pending {
            w.evict_column([-1, 0]);
        }
        let sparse = Skylight::for_chunks(&w, &targets);
        let full = Skylight::full_reference(&w, &targets);
        for x in -14..30 {
            for z in -14..30 {
                for y in -65..193 {
                    let p = BlockPos([x, y, z]);
                    assert_eq!(sparse.level(p), full.level(p), "sky {p:?}");
                    assert_eq!(sparse.block_level(p), full.block_level(p), "torch {p:?}");
                }
            }
        }
    }
}

#[test]
fn deferred_vertical_flow_restarts_when_next_pending_slab_arrives() {
    let mut w = world(6);
    w.install_region(RegionPos([0, 0]), Default::default());
    w.insert_chunk(ChunkPos([0, 8, 0]), Chunk::default());
    w.set_block(BlockPos([8, 129, 8]), Block::WaterSource)
        .unwrap();
    let mut changes = Vec::new();
    for _ in 0..8 {
        w.tick_fluids(Default::default(), &mut changes);
    }
    assert_eq!(w.block(BlockPos([8, 128, 8])), Block::Air);
    w.insert_chunk(ChunkPos([0, 7, 0]), Chunk::default());
    for _ in 0..8 {
        w.tick_fluids(Default::default(), &mut changes);
    }
    assert_eq!(w.block(BlockPos([8, 128, 8])), Block::WaterFalling);
    assert_eq!(w.block(BlockPos([8, 127, 8])), Block::WaterFalling);
}

#[test]
fn swimming_and_torch_washing_work_across_high_and_deep_chunk_boundaries() {
    use crate::physics::{MovementInput, PhysicsConfig, Player};
    for seam in [-48, 32, 128, 176] {
        let mut w = world(6);
        empty_area(&mut w, 1);
        for y in seam - 3..seam + 6 {
            for x in 6..=10 {
                for z in 6..=10 {
                    w.set_block(BlockPos([x, y, z]), Block::WaterFalling)
                        .unwrap();
                }
            }
        }
        let config = PhysicsConfig::default();
        let mut p = Player::new([8.5, f64::from(seam) - 0.1, 8.5]);
        assert!(p.in_water(&w, &config));
        for _ in 0..60 {
            p.tick(
                &w,
                MovementInput {
                    jump: true,
                    ..Default::default()
                },
                &config,
            );
        }
        assert!(p.position[1] > f64::from(seam) + 1.);
        // Side-mounted emitter is washed away with exactly one retained item.
        let torch = BlockPos([8, seam, 8]);
        w.set_block(torch, Block::Air).unwrap();
        w.set_block(BlockPos([7, seam, 8]), Block::Stone).unwrap();
        w.set_block(torch, Block::TorchEast).unwrap();
        w.set_block(BlockPos([8, seam + 1, 8]), Block::WaterSource)
            .unwrap();
        let mut changes = Vec::new();
        for _ in 0..4 {
            w.tick_fluids(Default::default(), &mut changes);
        }
        assert!(w.block(torch).fluid().is_some());
        assert_eq!(w.dropped_items().len(), 1);
        assert_eq!(
            w.dropped_items().iter().next().unwrap().1.item(),
            crate::inventory::Item::Torch
        );
        assert_eq!(light(&w).block_level(torch), 0);
    }
}
