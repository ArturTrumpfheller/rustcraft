use super::*;
use crate::{
    interaction::{EditAction, EditError, edit},
    persistence::{decode, encode},
    physics::{MovementInput, PhysicsConfig, Player},
    raycast::Ray,
};
fn gen2(seed: u64) -> Arc<Generator> {
    Arc::new(Generator::new(GenerationConfig { seed, version: 2 }).unwrap())
}
fn install(world: &mut World, result: ColumnResult) {
    for (p, c, _) in result.chunks {
        world.insert_chunk(p, c);
    }
}

#[test]
fn radius_policy_caps_pending_and_handles_extreme_fast_travel_with_spawn_pin() {
    for r in 1..=3 {
        for x in -4..4 {
            for z in -4..4 {
                let wanted = desired([x, z], r);
                assert!(wanted.contains(&[0, 0]));
                assert!(wanted.contains(&[x, z]));
                assert!(wanted.len() <= ((2 * r + 1).pow(2) + 1) as usize);
                assert!(wanted.iter().all(|p| p.iter().all(|v| (-4..4).contains(v))));
                assert_eq!(nearest(&wanted, [x, z]), Some([x, z]));
            }
        }
    }
    for center in [[i32::MAX; 2], [i32::MIN; 2]] {
        let wanted = desired(center, 99);
        assert!(wanted.len() <= 50);
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
