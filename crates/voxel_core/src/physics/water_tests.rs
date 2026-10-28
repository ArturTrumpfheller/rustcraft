// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::Block;

fn shore(water: Block, wall_height: i32, ceiling: bool) -> World {
    let mut world = World::default();
    for x in -4..=5 {
        for z in -3..=3 {
            world.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
            if x <= 0 {
                world.set_block(BlockPos([x, 1, z]), water).unwrap();
            } else {
                for y in 1..=wall_height {
                    world.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
                }
            }
            if ceiling {
                world.set_block(BlockPos([x, 3, z]), Block::Stone).unwrap();
            }
        }
    }
    world
}

#[test]
fn held_jump_exits_every_flow_level_and_source_onto_one_block_ledge() {
    let config = PhysicsConfig::default();
    for water in (0..=8).map(|s| crate::fluid::Fluid::from_state_id(s).unwrap().block()) {
        let world = shore(water, 1, false);
        let mut player = Player::new([0.5, 1., 0.5]);
        for _ in 0..90 {
            player.tick(
                &world,
                MovementInput {
                    direction: [1., 0.],
                    jump: true,
                    ..Default::default()
                },
                &config,
            );
            assert!(player.fits_in_world(&world, &config), "{water:?}");
        }
        assert!(
            player.position[0] > 1.3 && player.position[1] >= 2.,
            "{water:?}: {player:?}"
        );
    }
}

#[test]
fn shallow_contact_still_uses_the_rendered_surface() {
    let world = shore(Block::Water7, 0, false);
    let config = PhysicsConfig::default();
    assert!(Player::new([-1.5, 1.05, 0.5]).in_water(&world, &config));
    assert!(!Player::new([-1.5, 1.2, 0.5]).in_water(&world, &config));
}

#[test]
fn source_flow_and_falling_columns_allow_ordinary_upward_swimming() {
    let config = PhysicsConfig::default();
    for state in 0..=8 {
        let water = crate::fluid::Fluid::from_state_id(state).unwrap().block();
        let mut world = shore(water, 0, false);
        for y in 2..=4 {
            for x in -3..=0 {
                for z in -2..=2 {
                    world.set_block(BlockPos([x, y, z]), water).unwrap();
                }
            }
        }
        let mut player = Player::new([-1.5, 1., 0.5]);
        for _ in 0..60 {
            player.tick(
                &world,
                MovementInput {
                    jump: true,
                    ..Default::default()
                },
                &config,
            );
        }
        assert!(player.position[1] > 2. && player.velocity[1] > 0.);
        assert!(player.velocity[1] <= 2.5);
        assert_eq!(player.water_exit_origin, None);
    }
}

#[test]
fn two_block_walls_and_low_ceilings_prevent_exit_assistance() {
    let config = PhysicsConfig::default();
    for water in [
        Block::WaterSource,
        Block::Water1,
        Block::Water4,
        Block::Water7,
        Block::WaterFalling,
    ] {
        for (height, ceiling) in [(2, false), (1, true)] {
            let world = shore(water, height, ceiling);
            let mut player = Player::new([0.5, 1., 0.5]);
            for _ in 0..240 {
                player.tick(
                    &world,
                    MovementInput {
                        direction: [1., 0.],
                        jump: true,
                        ..Default::default()
                    },
                    &config,
                );
                assert!(player.fits_in_world(&world, &config));
                assert!(player.position[0] <= 0.7 + CONTACT_EPSILON);
                assert_eq!(player.water_exit_origin, None);
                if ceiling {
                    assert!(player.position[1] <= 1.2 + CONTACT_EPSILON);
                }
            }
        }
    }
}

#[test]
fn assistance_requires_jump_direction_and_a_nearby_ledge() {
    let config = PhysicsConfig::default();
    let world = shore(Block::Water7, 1, false);
    for input in [
        MovementInput {
            direction: [1., 0.],
            ..Default::default()
        },
        MovementInput {
            jump: true,
            ..Default::default()
        },
        MovementInput {
            direction: [-1., 0.],
            jump: true,
            ..Default::default()
        },
        MovementInput {
            direction: [1., 0.],
            jump: true,
            sneak: true,
            ..Default::default()
        },
    ] {
        let mut player = Player::new([0.5, 1., 0.5]);
        for _ in 0..30 {
            player.tick(&world, input, &config);
            assert_eq!(player.water_exit_origin, None);
        }
    }
    let mut remote = Player::new([-1.5, 1., 0.5]);
    remote.tick(
        &world,
        MovementInput {
            direction: [1., 0.],
            jump: true,
            ..Default::default()
        },
        &config,
    );
    assert_eq!(remote.water_exit_origin, None);
}

#[test]
fn exit_is_one_ballistic_impulse_and_leaving_water_restores_exact_gravity() {
    let world = shore(Block::Water7, 1, false);
    let config = PhysicsConfig::default();
    let input = MovementInput {
        direction: [1., 0.],
        jump: true,
        ..Default::default()
    };
    let mut player = Player::new([0.5, 1., 0.5]);
    player.tick(&world, input, &config);
    assert!(player.water_exit_origin.is_some());
    assert_eq!(
        player.velocity[1],
        WATER_EXIT_SPEED - config.gravity * PHYSICS_DT
    );
    // Take the horizontal input away, keeping held Jump. Neither submerged
    // nor dry rising ticks may grant another launch, even after release/repress.
    for tick in 0..12 {
        let before = player.velocity[1];
        player.tick(
            &world,
            MovementInput {
                jump: tick != 3,
                ..Default::default()
            },
            &config,
        );
        assert!((player.velocity[1] - (before - config.gravity * PHYSICS_DT)).abs() < 1e-12);
    }
    assert!(!player.in_water(&world, &config));
    assert!(player.position[1] < 2.4);
    let mut dry = Player::new([-2., 5., 0.5]);
    dry.velocity[1] = 1.;
    dry.tick(
        &world,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        &config,
    );
    assert_eq!(dry.velocity[1], 1. - config.gravity * PHYSICS_DT);
}

#[test]
fn shore_exit_is_identical_with_30_60_and_144_render_frames() {
    let config = PhysicsConfig::default();
    let world = shore(Block::Water4, 1, false);
    let input = MovementInput {
        direction: [1., 0.],
        jump: true,
        ..Default::default()
    };
    let mut results = Vec::new();
    for fps in [30, 60, 144] {
        let mut player = Player::new([0.5, 1., 0.5]);
        let mut accumulator = 0.;
        let mut ticks = 0;
        for _ in 0..fps {
            accumulator += PHYSICS_HZ / f64::from(fps);
            while accumulator >= 1. - 1e-9 {
                player.tick(&world, input, &config);
                ticks += 1;
                accumulator -= 1.;
            }
        }
        assert_eq!(ticks, 60);
        results.push(player);
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[1], results[2]);
}

#[test]
fn dry_jump_sprint_and_airborne_requests_retain_existing_calibration() {
    let config = PhysicsConfig::default();
    let world = shore(Block::Air, 0, false);
    for sprint in [false, true] {
        let input = MovementInput {
            direction: [1., 0.],
            sprint,
            ..Default::default()
        };
        let mut player = Player::new([-2.5, 1., 0.5]);
        for _ in 0..30 {
            player.tick(&world, input, &config);
        }
        let speed = if sprint {
            config.sprint_speed
        } else {
            config.walk_speed
        };
        assert!((player.velocity[0] - speed).abs() < 1e-10);
        player.tick(
            &world,
            MovementInput {
                jump: true,
                ..input
            },
            &config,
        );
        assert_eq!(
            player.velocity[1],
            config.jump_speed - config.gravity * PHYSICS_DT
        );
        let before = player.velocity[1];
        player.tick(
            &world,
            MovementInput {
                jump: true,
                ..input
            },
            &config,
        );
        assert_eq!(player.velocity[1], before - config.gravity * PHYSICS_DT);
        assert_eq!(player.water_exit_origin, None);
    }
}
