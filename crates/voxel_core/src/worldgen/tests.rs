use super::*;
use crate::{
    EDIT_MAX, EDIT_MIN,
    persistence::encode,
    physics::{MovementInput, PhysicsConfig, Player, spawn_position},
};
fn generator(seed: u64) -> Generator {
    Generator::new(GenerationConfig {
        seed,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn deterministic_seeds_chunk_order_and_golden_worlds() {
    for (seed, golden) in [
        (0, 0xc45927a4),
        (1, 0xbd348afc),
        (42, 0xf80a9287),
        (2026, 0xd14aedaf),
        (u64::MAX, 0xea69a8c0),
    ] {
        let g = generator(seed);
        let world = g.generate();
        let bytes = encode(&world).unwrap();
        assert_eq!(bytes, encode(&generator(seed).generate()).unwrap());
        let mut reverse = World::default();
        for x in (-2..2).rev() {
            for z in (-2..2).rev() {
                for y in (-1..2).rev() {
                    let pos = ChunkPos([x, y, z]);
                    let chunk = g.chunk(pos).unwrap();
                    reverse.insert_chunk(pos, chunk);
                }
            }
        }
        assert_eq!(bytes, encode(&reverse).unwrap());
        assert_eq!(
            crc32fast::hash(&bytes),
            golden,
            "generation contract changed for {seed}"
        );
    }
    assert_ne!(
        encode(&generator(1).generate()).unwrap(),
        encode(&generator(2).generate()).unwrap()
    );
    assert!(
        Generator::new(GenerationConfig {
            seed: 0,
            version: 999
        })
        .is_err()
    );
}

#[test]
fn terrain_has_coherent_varied_heights_layers_bounded_chunks_and_firm_foundation() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let world = g.generate();
        let min = *g.heights.iter().min().unwrap();
        let max = *g.heights.iter().max().unwrap();
        assert!(max - min >= 6, "unvaried seed {seed}: {min}..{max}");
        assert!(min >= -8 && max <= 18);
        for z in -32..32 {
            for x in -32..32 {
                let h = g.height(x, z).unwrap();
                for y in [-16, -15] {
                    assert_eq!(world.block(BlockPos([x, y, z])), Block::Stone);
                }
                for y in -14..=h {
                    let b = world.block(BlockPos([x, y, z]));
                    if b != Block::Air {
                        assert_eq!(
                            b,
                            if y == h {
                                Block::Grass
                            } else if y >= h - 3 {
                                Block::Dirt
                            } else {
                                Block::Stone
                            }
                        );
                    }
                }
                if x < 31 {
                    assert!((h - g.height(x + 1, z).unwrap()).abs() <= 3);
                }
                if z < 31 {
                    assert!((h - g.height(x, z + 1).unwrap()).abs() <= 3);
                }
            }
        }
        assert!(world.chunks().count() <= 48);
        for (pos, _) in world.chunks() {
            let p = pos
                .block_position(LocalPos::new([0; 3]).unwrap())
                .unwrap()
                .0;
            for axis in 0..3 {
                assert!(p[axis] >= EDIT_MIN[axis] && p[axis] + 15 < EDIT_MAX[axis]);
            }
        }
        for p in [
            ChunkPos([-3, 0, 0]),
            ChunkPos([0, -2, 0]),
            ChunkPos([0, 0, 2]),
            ChunkPos([i32::MAX; 3]),
        ] {
            assert!(g.chunk(p).is_none());
        }
        assert_eq!(g.block(BlockPos([i32::MIN; 3])), Block::Air);
    }
}

#[test]
fn cave_network_has_a_continuous_headroom_ramp_and_accessible_caverns() {
    for seed in [0, 1, 42, 2026, u64::MAX] {
        let g = generator(seed);
        let mut last = g.entrance_height;
        for z in (-12..=20).rev() {
            let floor = g.entrance_height
                - ((20 - z) * (g.entrance_height + 6) / 32).min(g.entrance_height + 6);
            assert!((floor - last).abs() <= 1, "unclimbable ramp");
            last = floor;
            for y in floor..floor + 4 {
                assert_eq!(
                    g.block(BlockPos([-10, y, z])),
                    Block::Air,
                    "blocked entrance at {z}/{y}"
                );
            }
        }
        for x in -32..32 {
            let center = -10 + noise(seed, x, 0, 16, 53) * 3 / 1024;
            assert_eq!(g.block(BlockPos([x, -4, center])), Block::Air);
            assert_eq!(g.block(BlockPos([x, -3, center])), Block::Air);
        }
        assert!(g.entrance_height > g.height(-10, 20).unwrap());
    }
}

#[test]
fn trees_have_supported_trunks_clear_canopies_spacing_and_cross_chunk_foliage() {
    for seed in [0, 42, 2026, u64::MAX] {
        let g = generator(seed);
        assert!(g.trees.len() >= 8);
        let mut crosses = false;
        for tree in &g.trees {
            assert_eq!(
                g.block(BlockPos([tree.x, tree.ground, tree.z])),
                Block::Grass
            );
            for y in 1..=tree.trunk {
                assert_eq!(
                    g.block(BlockPos([tree.x, tree.ground + y, tree.z])),
                    Block::Wood
                );
            }
            assert_eq!(
                g.block(BlockPos([tree.x, tree.ground + tree.trunk + 1, tree.z])),
                Block::Leaves
            );
            for other in &g.trees {
                if tree.x != other.x || tree.z != other.z {
                    assert!((tree.x - other.x).abs().max((tree.z - other.z).abs()) >= 6);
                }
            }
            for dx in -2..=2 {
                for dz in -2..=2 {
                    if g.block(BlockPos([
                        tree.x + dx,
                        tree.ground + tree.trunk,
                        tree.z + dz,
                    ])) == Block::Leaves
                    {
                        crosses |= (tree.x + dx).div_euclid(16) != tree.x.div_euclid(16)
                            || (tree.z + dz).div_euclid(16) != tree.z.div_euclid(16);
                    }
                }
            }
        }
        assert!(crosses, "missing cross-boundary tree coverage");
    }
}

#[test]
fn spawn_and_respawn_are_supported_clear_and_physically_stable_for_many_seeds() {
    let config = PhysicsConfig::default();
    for seed in 0..24 {
        let world = generator(seed).generate();
        let spawn = spawn_position(&world, &config, [0.5, 4., 10.5]);
        let mut player = Player::new(spawn);
        assert!(player.fits_in_world(&world, &config));
        for _ in 0..120 {
            player.tick(&world, MovementInput::default(), &config);
        }
        assert!(player.grounded, "seed {seed}");
        assert!((player.position[1] - spawn[1]).abs() <= 1.001);
        assert_eq!(spawn, spawn_position(&world, &config, [0.5, 4., 10.5]));
        assert!(!matches!(
            world.block(BlockPos([
                spawn[0].floor() as i32,
                player.position[1] as i32 - 1,
                spawn[2].floor() as i32
            ])),
            Block::Wood | Block::Leaves
        ));
    }
}

#[test]
fn new_materials_are_solid_and_cull_faces_across_positive_and_negative_seams() {
    use crate::mesh_chunk;
    for x in [-17, -1, 15] {
        let mut world = World::default();
        world.set_block(BlockPos([x, 1, 0]), Block::Wood).unwrap();
        world
            .set_block(BlockPos([x + 1, 1, 0]), Block::Leaves)
            .unwrap();
        let a = BlockPos([x, 1, 0]).split().0;
        let b = BlockPos([x + 1, 1, 0]).split().0;
        assert_ne!(a, b);
        assert_eq!(mesh_chunk(&world, a).indices.len(), 30);
        assert_eq!(mesh_chunk(&world, b).indices.len(), 30);
        let config = PhysicsConfig::default();
        let mut player = Player::new([f64::from(x) + 1.5, 4., 0.5]);
        for _ in 0..120 {
            player.tick(&world, MovementInput::default(), &config);
        }
        assert!(player.grounded);
        assert!((player.position[1] - 2.).abs() < 1e-8);
    }
}
