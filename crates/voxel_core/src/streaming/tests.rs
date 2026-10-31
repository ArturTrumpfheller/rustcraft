use super::*;
use crate::Block;
use crate::{
    interaction::{EditAction, EditError, edit},
    persistence::{decode, encode},
    physics::{MovementInput, PhysicsConfig, Player},
    raycast::Ray,
};
use std::collections::BTreeMap;

#[test]
fn generated_border_volume_metrics_follow_the_saved_world_height() {
    for version in [2, 9] {
        let generator = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        let targets = BTreeSet::from([ChunkPos([0, 0, 0])]);
        let batch = remesh_cached(
            &generator,
            World::default(),
            EditSnapshot::default(),
            targets.clone(),
        );
        let expected = 9 * generator.world_height().chunk_count() * 4096;
        assert_eq!(batch.sampled_border_cells, expected);
        assert_eq!(batch.lighting.generated_columns, 9);
        let water = remesh_water(
            &generator,
            World::default(),
            EditSnapshot::default(),
            &targets,
            &mut TerrainCache::default(),
            &batch.lights,
        );
        assert_eq!(water.sampled_border_cells, expected);
        assert_eq!(water.lighting.generated_columns, 9);
        if version == 9 {
            let sky = BTreeSet::from([ChunkPos([0, 10, 0])]);
            let mut cache = TerrainCache::default();
            let first = remesh_lit(
                &generator,
                World::default(),
                EditSnapshot::default(),
                &sky,
                &mut cache,
                &BTreeMap::new(),
                &sky,
            );
            assert!(first.lights.is_empty());
            assert_eq!(first.sampled_border_cells, expected);
            let warm = remesh_lit(
                &generator,
                World::default(),
                EditSnapshot::default(),
                &sky,
                &mut cache,
                &BTreeMap::new(),
                &sky,
            );
            assert_eq!(warm.sampled_border_cells, 0);
            assert_eq!(warm.lighting.generated_columns, 0);
        }
    }
}
fn gen2(seed: u64) -> Arc<Generator> {
    Arc::new(Generator::new(GenerationConfig { seed, version: 2 }).unwrap())
}
fn install(world: &mut World, result: ColumnResult) {
    for (p, c, _) in result.chunks {
        world.insert_chunk(p, c);
    }
}

#[test]
fn radius_policy_caps_pending_and_handles_extreme_fast_travel_without_spawn_pin() {
    for r in 1..=3 {
        for x in -4..4 {
            for z in -4..4 {
                let wanted = desired([x, z], r);
                assert_eq!(wanted.contains(&[0, 0]), x.abs().max(z.abs()) <= r);
                assert!(wanted.contains(&[x, z]));
                assert!(wanted.len() <= (2 * r + 1).pow(2) as usize);
                assert!(wanted.iter().all(|p| p.iter().all(|v| (-4..4).contains(v))));
                assert_eq!(nearest(&wanted, [x, z]), Some([x, z]));
            }
        }
    }
    for center in [[i32::MAX; 2], [i32::MIN; 2]] {
        let wanted = desired(center, 99);
        assert!(wanted.len() <= 49);
        assert!(nearest(&wanted, center).is_some());
    }
}

#[test]
fn expanded_generation_is_order_independent_and_halo_meshes_match_full_neighbors() {
    let generator = gen2(42);
    let mut reference = World::default();
    for x in -3..=3 {
        for z in -3..=3 {
            for y in -1..2 {
                let p = ChunkPos([x, y, z]);
                reference.insert_chunk(p, generator.chunk(p).unwrap());
            }
        }
    }
    let mut edits = BTreeMap::new();
    edits.insert(BlockPos([-17, 3, 16]), Block::Air);
    edits.insert(BlockPos([-16, 20, 16]), Block::Leaves);
    edits.insert(BlockPos([32, 19, -17]), Block::Wood);
    for (&p, &b) in &edits {
        reference.apply_cached_edit(p, b);
    }
    let edits = Arc::new(edits);
    for column in [[2, -2], [-2, 1], [0, 0], [-1, 1], [2, -2]] {
        let a = build_column(&generator, Arc::clone(&edits), column, 7);
        assert_eq!(a.revision, 7);
        assert_eq!(a.chunks.len(), 3);
        for (p, c, m) in a.chunks {
            assert_eq!(&c, reference.chunk(p).unwrap());
            assert_eq!(m, mesh_chunk(&reference, p), "halo seam {p:?}");
        }
    }
    assert!(
        build_column(&generator, edits, [i32::MAX; 2], 0)
            .chunks
            .is_empty()
    );
}

#[test]
fn edits_survive_unsaved_eviction_reload_and_v3_restart_without_dense_storage() {
    let generator = gen2(2026);
    let mut world = new_world(2026);
    let result = build_column(&generator, world.edits(), [-3, 2], 0);
    install(&mut world, result);
    let removed = BlockPos([-40, -15, 40]);
    let wood = BlockPos([-40, 24, 40]);
    world.set_block(removed, Block::Air).unwrap();
    world.set_block(wood, Block::Wood).unwrap();
    let rev = world.revision();
    let snapshot = world.save_snapshot();
    assert_eq!(snapshot.chunks().count(), 0);
    world.evict_column([-3, 2]);
    assert_eq!(world.chunks().count(), 0);
    assert_eq!(world.revision(), rev);
    assert_eq!(world.edits().len(), 2);
    assert!(!world.ready(wood));
    let result = build_column(&generator, world.edits(), [-3, 2], rev);
    install(&mut world, result);
    assert_eq!(world.block(removed), Block::Air);
    assert_eq!(world.block(wood), Block::Wood);
    let bytes = encode(&world).unwrap();
    assert_eq!(&bytes[8..12], &3_u32.to_le_bytes());
    assert_eq!(bytes.len(), 40 + 2 * 14);
    assert_eq!(encode(&snapshot).unwrap(), bytes);
    let mut loaded = decode(&bytes).unwrap();
    assert_eq!(loaded.identity(), world.identity());
    assert_eq!(loaded.chunks().count(), 0);
    let result = build_column(&generator, loaded.edits(), [-3, 2], 0);
    install(&mut loaded, result);
    assert_eq!(loaded.block(removed), Block::Air);
    assert_eq!(loaded.block(wood), Block::Wood);
}

#[test]
fn pending_cells_are_solid_not_editable_and_cannot_cause_floor_or_wall_tunneling() {
    let generator = gen2(0);
    let mut world = new_world(0);
    let result = build_column(&generator, world.edits(), [0, 0], 0);
    install(&mut world, result);
    // Use a high clear platform to isolate pending-boundary behavior from terrain.
    for x in 13..16 {
        world.set_block(BlockPos([x, 20, 8]), Block::Stone).unwrap();
    }
    let config = PhysicsConfig::default();
    let mut player = Player::new([14.5, 21., 8.5]);
    for _ in 0..120 {
        player.tick(
            &world,
            MovementInput {
                direction: [1., 0.],
                ..Default::default()
            },
            &config,
        );
    }
    assert!(player.position[0] <= 15.7 + 1e-8);
    assert!(player.grounded);
    assert!(
        player.cache_ready(&world, &config),
        "mere wall contact is not entering pending terrain"
    );
    assert!(!Player::new([16.5, 21., 8.5]).cache_ready(&world, &config));
    let before = world.revision();
    assert_eq!(
        world.set_block(BlockPos([16, 21, 8]), Block::Air),
        Err(EditError::NotLoaded)
    );
    assert_eq!(
        edit(
            &mut world,
            Ray {
                origin: [15.5, 22., 8.5],
                direction: [1., 0., 0.],
                reach: 5.
            },
            EditAction::Break,
            &player,
            &config
        ),
        Err(EditError::NotLoaded)
    );
    assert_eq!(world.revision(), before);
    assert_eq!(world.block(BlockPos([64, 0, 0])), Block::Air); // Finite edge is known.
    assert_eq!(
        world.set_block(BlockPos([64, 0, 0]), Block::Wood),
        Err(EditError::OutOfBounds)
    );
}

#[test]
fn immutable_overlay_snapshots_and_residency_do_not_change_dirty_revisions() {
    let mut world = new_world(1);
    let g = prepare_spawn(&mut world).unwrap();
    assert_eq!(world.revision(), 0);
    assert_eq!(world.chunks().count(), 3);
    let old = world.save_snapshot();
    let edits = world.edits();
    world
        .set_block(BlockPos([0, 25, 0]), Block::Leaves)
        .unwrap();
    assert!(edits.is_empty());
    assert!(old.edits().is_empty());
    let revision = world.revision();
    let result = build_column(&g, world.edits(), [1, 0], revision);
    install(&mut world, result);
    world.evict_column([1, 0]);
    assert_eq!(world.revision(), revision);
    assert_eq!(
        decode(&encode(&old).unwrap()).unwrap().identity(),
        Some(StreamIdentity {
            seed: 1,
            generator: 2
        })
    );
}

#[test]
fn generator_v2_column_golden_contract_includes_expanded_negative_and_positive_terrain() {
    for (seed, golden) in [(0, 0x67e85f54), (42, 0xfbf0f1d5), (u64::MAX, 0x3395e07a)] {
        let generator = gen2(seed);
        let mut hash = crc32fast::Hasher::new();
        for column in [[-4, -4], [0, 0], [3, 3], [-3, 2]] {
            for y in -1..2 {
                let chunk = generator
                    .chunk(ChunkPos([column[0], y, column[1]]))
                    .unwrap();
                for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let block = chunk.get(crate::LocalPos::new([lx, ly, lz]).unwrap());
                            let id = match block {
                                Block::Air => 0,
                                Block::Grass => 1,
                                Block::Dirt => 2,
                                Block::Stone => 3,
                                Block::Wood => 4,
                                Block::Leaves => 5,
                                Block::Torch => 6,
                                Block::TorchEast => 7,
                                Block::TorchWest => 8,
                                Block::TorchSouth => 9,
                                Block::TorchNorth => 10,
                                _ => panic!("frozen generator must not emit fluids"),
                            };
                            hash.update(&[id]);
                        }
                    }
                }
            }
        }
        assert_eq!(
            hash.finalize(),
            golden,
            "generator v2 contract changed for seed {seed}"
        );
    }
}

#[test]
fn local_mesh_dependencies_cover_lighting_but_not_unrelated_edits_or_residency() {
    let mut world = new_world(42);
    prepare_spawn(&mut world).unwrap();
    world.set_block(BlockPos([8, 25, 8]), Block::Wood).unwrap();
    let owner = ChunkPos([0, 1, 0]);
    let revision = world.mesh_revision(owner);
    assert!(revision > 0);
    assert_eq!(world.mesh_revision(ChunkPos([1, 1, 0])), revision);
    assert_eq!(world.mesh_revision(ChunkPos([2, 1, 0])), 0);
    world
        .set_block(BlockPos([15, 31, 15]), Block::Leaves)
        .unwrap();
    assert_eq!(world.mesh_revision(ChunkPos([1, 1, 0])), world.revision());
    assert_eq!(world.mesh_revision(ChunkPos([0, 1, 1])), world.revision());
    assert_eq!(world.mesh_revision(ChunkPos([1, 1, 1])), world.revision());
    assert_eq!(world.mesh_revision(ChunkPos([2, 1, 1])), 0);
    let latest = world.mesh_revision(owner);
    assert!(latest > revision);
    world
        .set_block(BlockPos([15, 31, 15]), Block::Leaves)
        .unwrap();
    assert_eq!(world.mesh_revision(owner), latest);
    let bytes = encode(&world).unwrap();
    world.evict_column([0, 0]);
    assert_eq!(world.mesh_revision(owner), latest);
    assert_eq!(encode(&world).unwrap(), bytes);
    assert_eq!(decode(&bytes).unwrap().mesh_revision(owner), 0);
}

#[test]
fn cached_remeshing_matches_full_neighbors_at_pending_faces_and_all_axis_seams() {
    let generator = gen2(42);
    let mut reference = World::default();
    for x in -4..4 {
        for z in -4..4 {
            for y in -1..2 {
                let p = ChunkPos([x, y, z]);
                reference.insert_chunk(p, generator.chunk(p).unwrap());
            }
        }
    }
    let edits = Arc::new(BTreeMap::from([
        (BlockPos([-16, 0, -16]), Block::Air),
        (BlockPos([-17, 0, -16]), Block::Wood),
        (BlockPos([-16, -1, -16]), Block::Air),
        (BlockPos([32, 16, 32]), Block::Leaves),
        (BlockPos([31, 16, 32]), Block::Wood),
    ]));
    for (&p, &b) in edits.iter() {
        reference.apply_cached_edit(p, b);
    }
    let targets = BTreeSet::from([
        ChunkPos([-1, 0, -1]),
        ChunkPos([-2, 0, -1]),
        ChunkPos([-1, -1, -1]),
        ChunkPos([2, 1, 2]),
    ]);
    let mut cached = new_world(42);
    for &p in &targets {
        cached.insert_chunk(p, reference.chunk(p).unwrap().clone());
    }
    let snapshot = mesh_snapshot(&cached, &targets);
    assert_eq!(snapshot.chunks().count(), targets.len());
    let batch = remesh_cached(&generator, snapshot, Arc::clone(&edits), targets.clone());
    assert!(batch.sampled_border_cells > 0);
    for (p, mesh) in batch.meshes {
        assert_eq!(mesh, mesh_chunk(&reference, p), "pending halo {p:?}");
    }
    let full = mesh_snapshot(&reference, &targets);
    let batch = remesh_cached(&generator, full, edits, targets);
    assert_eq!(
        batch.sampled_border_cells, 0,
        "resident neighbors must not invoke worldgen"
    );
    for (p, mesh) in batch.meshes {
        assert_eq!(mesh, mesh_chunk(&reference, p));
    }
}

#[test]
fn motion_priority_prefers_forward_inside_near_ring_without_skipping_nearby_chunks() {
    let set = BTreeSet::from([[999, -1000], [1001, -1000], [1002, -1000]]);
    assert_eq!(
        nearest_ahead(&set, [1000, -1000], [1., 0.]),
        Some([1001, -1000])
    );
    assert_eq!(
        nearest_ahead(&set, [1000, -1000], [-1., 0.]),
        Some([999, -1000])
    );
    let set = BTreeSet::from([[999, -1000], [1002, -1000]]);
    assert_eq!(
        nearest_ahead(&set, [1000, -1000], [1., 0.]),
        Some([999, -1000])
    );
    assert!(nearest_ahead(&set, [1000, -1000], [f64::NAN, f64::INFINITY]).is_some());
}
