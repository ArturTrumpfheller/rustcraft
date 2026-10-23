use super::*;
use crate::{Block, Chunk, ChunkPos};
use std::collections::BTreeMap;

fn world_with(blocks: impl IntoIterator<Item = [i32; 3]>) -> World {
    let mut chunks = BTreeMap::<ChunkPos, Chunk>::new();
    for position in blocks {
        let (chunk, local) = BlockPos(position).split();
        chunks.entry(chunk).or_default().set(local, Block::Stone);
    }
    let mut world = World::default();
    for (position, chunk) in chunks {
        world.insert_chunk(position, chunk);
    }
    world
}

fn floor() -> impl Iterator<Item = [i32; 3]> {
    (-24..=24).flat_map(|x| (-24..=24).map(move |z| [x, 0, z]))
}

fn run(player: &mut Player, world: &World, input: MovementInput, ticks: usize) {
    for _ in 0..ticks {
        player.tick(world, input, &PhysicsConfig::default());
    }
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-7, "{actual} != {expected}");
}

fn assert_clear(player: &Player, world: &World) {
    let (min, max) = player.bounds(&PhysicsConfig::default());
    for x in (min[0] + CONTACT_EPSILON).floor() as i32..=(max[0] - CONTACT_EPSILON).floor() as i32 {
        for y in
            (min[1] + CONTACT_EPSILON).floor() as i32..=(max[1] - CONTACT_EPSILON).floor() as i32
        {
            for z in (min[2] + CONTACT_EPSILON).floor() as i32
                ..=(max[2] - CONTACT_EPSILON).floor() as i32
            {
                assert!(
                    !world.block(BlockPos([x, y, z])).is_solid(),
                    "player {player:?} intersects {:?}",
                    [x, y, z]
                );
            }
        }
    }
}

#[test]
fn falls_and_stands_without_jitter_for_ten_seconds() {
    let world = world_with(floor());
    let mut player = Player::new([0.5, 8.0, 0.5]);
    run(&mut player, &world, MovementInput::default(), 120);
    assert!(player.grounded);
    near(player.position[1], 1.0);
    let settled = player;
    run(&mut player, &world, MovementInput::default(), 600);
    assert_eq!(player, settled);
    assert_clear(&player, &world);
}

#[test]
fn grounded_jump_clears_one_block_but_airborne_requests_do_not_rejump() {
    let world = world_with(floor());
    let mut player = Player::new([0.5, 1.0, 0.5]);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    player.tick(&world, jump, &PhysicsConfig::default());
    assert!(!player.grounded);
    assert!(player.velocity[1] > 0.0);
    let mut apex = player.position[1];
    for _ in 0..35 {
        let previous = player.velocity[1];
        player.tick(&world, jump, &PhysicsConfig::default());
        assert!(player.velocity[1] < previous);
        apex = apex.max(player.position[1]);
    }
    assert!((2.2..2.6).contains(&apex), "apex {apex}");
    run(&mut player, &world, MovementInput::default(), 90);
    assert!(player.grounded);
    near(player.position[1], 1.0);
}

#[test]
fn stale_grounded_state_cannot_jump_over_empty_space() {
    let mut player = Player::new([0.5, 1.0, 0.5]);
    player.grounded = true;
    player.tick(
        &World::default(),
        MovementInput {
            jump: true,
            ..Default::default()
        },
        &PhysicsConfig::default(),
    );
    assert!(player.velocity[1] < 0.0);
    assert!(!player.grounded);
}

#[test]
fn head_hits_ceiling_then_falls_without_becoming_grounded() {
    let world = world_with(floor().chain((-1..=1).flat_map(|x| (-1..=1).map(move |z| [x, 3, z]))));
    let mut player = Player::new([0.5, 1.0, 0.5]);
    player.tick(
        &world,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        &PhysicsConfig::default(),
    );
    player.tick(&world, MovementInput::default(), &PhysicsConfig::default());
    near(player.position[1], 1.2);
    near(player.velocity[1], 0.0);
    assert!(!player.grounded);
    run(&mut player, &world, MovementInput::default(), 60);
    near(player.position[1], 1.0);
    assert!(player.grounded);
}

#[test]
fn movement_accelerates_brakes_and_normalizes_diagonals_and_sprint() {
    let world = world_with(floor());
    for sprint in [false, true] {
        let mut straight = Player::new([0.5, 1.0, 0.5]);
        let mut diagonal = straight;
        run(
            &mut straight,
            &world,
            MovementInput {
                direction: [1.0, 0.0],
                sprint,
                ..Default::default()
            },
            60,
        );
        run(
            &mut diagonal,
            &world,
            MovementInput {
                direction: [1.0, 1.0],
                sprint,
                ..Default::default()
            },
            60,
        );
        near(straight.velocity[0], if sprint { 8.0 } else { 5.0 });
        near(
            diagonal.velocity[0].hypot(diagonal.velocity[2]),
            straight.velocity[0],
        );
        near(
            (diagonal.position[0] - 0.5).hypot(diagonal.position[2] - 0.5),
            straight.position[0] - 0.5,
        );
        run(&mut straight, &world, MovementInput::default(), 10);
        near(straight.velocity[0], 0.0);
    }
}

#[test]
fn yaw_rotates_forward_without_changing_speed() {
    let input = MovementInput::from_yaw([0.0, -1.0], std::f64::consts::FRAC_PI_2, false, false);
    near(input.direction[0], -1.0);
    near(input.direction[1], 0.0);
}

#[test]
fn slides_along_wall_and_cannot_push_through_corner() {
    let walls = (-20..=20).flat_map(|z| (1..=4).map(move |y| [2, y, z]));
    let world = world_with(floor().chain(walls));
    let mut player = Player::new([0.5, 1.0, 0.5]);
    run(
        &mut player,
        &world,
        MovementInput {
            direction: [1.0, 1.0],
            ..Default::default()
        },
        120,
    );
    near(player.position[0], 1.7);
    assert!(player.position[2] > 5.0);
    assert_clear(&player, &world);
    let corner = world_with(
        floor().chain((-20..=20).flat_map(|n| (1..=4).flat_map(move |y| [[2, y, n], [n, y, 2]]))),
    );
    let mut player = Player::new([0.5, 1.0, 0.5]);
    run(
        &mut player,
        &corner,
        MovementInput {
            direction: [1.0, 1.0],
            ..Default::default()
        },
        600,
    );
    near(player.position[0], 1.7);
    near(player.position[2], 1.7);
    assert_clear(&player, &corner);
}

#[test]
fn leaving_a_ledge_clears_support_and_starts_falling() {
    let world = world_with([[0, 0, 0]]);
    let mut player = Player::new([0.5, 1.0, 0.5]);
    run(
        &mut player,
        &world,
        MovementInput {
            direction: [1.0, 0.0],
            ..Default::default()
        },
        40,
    );
    assert!(!player.grounded);
    assert!(player.position[0] > 1.3);
    assert!(player.position[1] < 1.0);
    assert!(player.velocity[1] < 0.0);
}

#[test]
fn negative_chunk_seams_do_not_snag_and_walls_work_on_both_sides() {
    let world = world_with(floor());
    for axis in [0, 2] {
        for boundary in [-16.0, 0.0, 16.0] {
            for sign in [-1.0, 1.0] {
                let mut player = Player::new([0.5, 1.0, 0.5]);
                player.position[axis] = boundary - sign * 2.0;
                let mut direction = [0.0; 2];
                direction[axis / 2] = sign;
                run(
                    &mut player,
                    &world,
                    MovementInput {
                        direction,
                        ..Default::default()
                    },
                    60,
                );
                assert!((player.position[axis] - boundary) * sign > 1.5);
                assert!(player.grounded);
                near(player.position[1], 1.0);
            }
        }
    }
    for axis in [0, 2] {
        for boundary in [-16, 0, 16] {
            let wall = (-2..=2).flat_map(|n| {
                (1..=4).map(move |y| {
                    if axis == 0 {
                        [boundary, y, n]
                    } else {
                        [n, y, boundary]
                    }
                })
            });
            let world = world_with(floor().chain(wall));
            for sign in [-1.0, 1.0] {
                let mut player = Player::new([0.5, 1.0, 0.5]);
                player.position[axis] = f64::from(boundary) + if sign > 0.0 { -2.0 } else { 3.0 };
                let mut direction = [0.0; 2];
                direction[axis / 2] = sign;
                run(
                    &mut player,
                    &world,
                    MovementInput {
                        direction,
                        ..Default::default()
                    },
                    60,
                );
                near(
                    player.position[axis],
                    f64::from(boundary) + if sign > 0.0 { -0.3 } else { 1.3 },
                );
                assert_clear(&player, &world);
            }
        }
    }
}

#[test]
fn swept_collision_blocks_high_speed_floor_wall_and_ceiling_tunneling() {
    let config = PhysicsConfig {
        terminal_speed: 300.0,
        ..Default::default()
    };
    let world = world_with(floor().chain((1..=6).map(|y| [2, y, 0])));
    let mut player = Player::new([0.5, 4.0, 0.5]);
    player.velocity = [600.0, -250.0, 0.0];
    player.tick(&world, MovementInput::default(), &config);
    near(player.position[0], 1.7);
    near(player.position[1], 1.0);
    assert!(player.grounded);
    assert_clear(&player, &world);
    let world = world_with([[0, 3, 0]]);
    let mut player = Player::new([0.5, 1.0, 0.5]);
    player.velocity[1] = 250.0;
    player.tick(&world, MovementInput::default(), &config);
    near(player.position[1], 1.2);
    near(player.velocity[1], 0.0);
    assert!(!player.grounded);
}

#[test]
fn supported_terminal_fall_hits_single_block_and_air_does_not_create_ground() {
    let world = world_with([[0, -16, 0]]);
    let mut player = Player::new([0.5, 3.0, 0.5]);
    player.velocity[1] = -55.0;
    run(&mut player, &world, MovementInput::default(), 120);
    near(player.position[1], -15.0);
    assert!(player.grounded);
    run(
        &mut player,
        &World::default(),
        MovementInput::default(),
        180,
    );
    assert!(!player.grounded);
    assert!(player.position[1] < -50.0);
    near(player.velocity[1], -55.0);
}

#[test]
fn fixed_command_replay_is_repeatable_and_never_intersects_demo_blocks() {
    let world = crate::demo_world();
    let mut a = Player::new([0.5, 4.0, 10.5]);
    let mut b = a;
    let mut seed = 731_u64;
    for tick in 0..6000 {
        // Hold each command long enough to traverse terrain, not just jitter in place.
        if tick % 37 == 0 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        let yaw = ((seed >> 32) % 628) as f64 / 100.0;
        let input = MovementInput::from_yaw([0.0, -1.0], yaw, tick % 100 < 50, tick % 49 == 0);
        a.tick(&world, input, &PhysicsConfig::default());
        b.tick(&world, input, &PhysicsConfig::default());
        assert_eq!(a, b);
        assert_clear(&a, &world);
        if a.position[1] < -32.0 {
            a = Player::new([0.5, 4.0, 10.5]);
            b = a;
        }
    }
}

#[test]
fn jumping_climbs_a_one_block_terrace_but_walking_cannot_auto_step() {
    let world = world_with(floor().chain((1..=12).flat_map(|x| (-2..=2).map(move |z| [x, 1, z]))));
    let input = MovementInput {
        direction: [1.0, 0.0],
        ..Default::default()
    };
    let mut walking = Player::new([0.5, 1.0, 0.5]);
    run(&mut walking, &world, input, 120);
    near(walking.position[0], 0.7);
    near(walking.position[1], 1.0);
    walking.tick(
        &world,
        MovementInput {
            jump: true,
            ..input
        },
        &PhysicsConfig::default(),
    );
    for _ in 0..55 {
        walking.tick(&world, input, &PhysicsConfig::default());
        assert_clear(&walking, &world);
    }
    assert!(walking.position[0] > 3.0);
    near(walking.position[1], 2.0);
    assert!(walking.grounded);
}

#[test]
fn air_control_preserves_momentum_without_input_and_gravity_does_not_exceed_terminal_speed() {
    let mut player = Player::new([0.5, 1000.0, 0.5]);
    player.velocity = [3.0, 0.0, 2.0];
    run(
        &mut player,
        &World::default(),
        MovementInput::default(),
        300,
    );
    near(player.velocity[0], 3.0);
    near(player.velocity[2], 2.0);
    near(player.velocity[1], -55.0);
    assert!(!player.grounded);
}

#[test]
fn diagonal_edge_contacts_are_clear_and_monotonic_at_signed_chunk_seams_and_heights() {
    let config = PhysicsConfig::default();
    for base in [-16, 0, 2, 16] {
        for height in [1, 7, 17] {
            for sx in [-1, 1] {
                for sz in [-1, 1] {
                    let cell = |v: i32, sign| base + if sign < 0 { -v - 1 } else { v };
                    let ground = (-4..=4)
                        .flat_map(|x| (-4..=4).map(move |z| [base + x, height - 1, base + z]));
                    let world = world_with(ground.chain([
                        [cell(-1, sx), height, cell(-1, sz)],
                        [cell(0, sx), height, cell(0, sz)],
                    ]));
                    let mut p = Player::new([
                        f64::from(base) + f64::from(sx) * 1.5,
                        f64::from(height),
                        f64::from(base) - f64::from(sz) * 0.5,
                    ]);
                    let input = MovementInput {
                        direction: [-f64::from(sx), f64::from(sz)],
                        ..Default::default()
                    };
                    for tick in 0..180 {
                        let old = p.position;
                        p.tick(&world, input, &config);
                        assert!(
                            p.fits_in_world(&world, &config),
                            "base {base}, height {height}, signs {sx}/{sz}, tick {tick}: {p:?}"
                        );
                        assert!((p.position[0] - old[0]) * f64::from(sx) <= CONTACT_EPSILON);
                        assert!((p.position[2] - old[2]) * f64::from(sz) >= -CONTACT_EPSILON);
                        assert_clear(&p, &world);
                    }
                    near(p.position[0], f64::from(base) + f64::from(sx) * 0.3);
                    near(p.position[2], f64::from(base) - f64::from(sz) * 0.3);
                    assert!(p.grounded);
                    let contact = p.position;
                    run(&mut p, &world, input, 300);
                    assert_eq!(p.position, contact);
                    p.tick(
                        &world,
                        MovementInput {
                            jump: true,
                            ..input
                        },
                        &config,
                    );
                    assert!(p.position[1] > f64::from(height) && p.velocity[1] > 0.);
                }
            }
        }
    }
}

#[test]
fn clearance_tolerance_permits_roundoff_but_rejects_real_penetration() {
    let world = world_with([[1, 1, 0]]);
    let config = PhysicsConfig::default();
    assert!(Player::new([2.3, 1., 0.5]).fits_in_world(&world, &config));
    assert!(!Player::new([2.3 - 1e-5, 1., 0.5]).fits_in_world(&world, &config));
}

#[test]
fn tuned_speeds_acceleration_braking_and_turning_are_bounded() {
    let world = world_with(floor());
    let cfg = PhysicsConfig::default();
    for (sprint, sneak, speed) in [(false, false, 5.), (true, false, 8.), (true, true, 1.8)] {
        let mut p = Player::new([0.5, 1., 0.5]);
        let command = MovementInput {
            direction: [1., 0.],
            sprint,
            sneak,
            jump: false,
        };
        p.tick(&world, command, &cfg);
        near(p.velocity[0], cfg.ground_acceleration * PHYSICS_DT);
        for _ in 0..20 {
            p.tick(&world, command, &cfg);
        }
        near(p.velocity[0], speed);
        let start = p.position;
        for _ in 0..60 {
            p.tick(&world, command, &cfg);
        }
        near(p.position[0] - start[0], speed);
        let old = p.velocity;
        p.tick(
            &world,
            MovementInput {
                direction: [0., 1.],
                ..command
            },
            &cfg,
        );
        assert!(
            (p.velocity[0] - old[0]).hypot(p.velocity[2] - old[2])
                <= cfg.ground_acceleration * PHYSICS_DT + 1e-8
        );
        p.tick(
            &world,
            MovementInput {
                sneak,
                ..Default::default()
            },
            &cfg,
        );
        assert!(p.velocity[0].hypot(p.velocity[2]) < speed);
        run(
            &mut p,
            &world,
            MovementInput {
                sneak,
                ..Default::default()
            },
            10,
        );
        near(p.velocity[0].hypot(p.velocity[2]), 0.);
    }
}

#[test]
fn sneak_guards_cardinal_and_diagonal_ledges_with_partial_support_at_signed_seams() {
    let cfg = PhysicsConfig::default();
    for base in [-17, -16, -1, 0, 15, 16, 15999] {
        for direction in [
            [1., 0.],
            [-1., 0.],
            [0., 1.],
            [0., -1.],
            [1., 1.],
            [-1., 1.],
            [1., -1.],
            [-1., -1.],
        ] {
            let world = world_with([[base, 0, base]]);
            let mut p = Player::new([f64::from(base) + 0.5, 1., f64::from(base) + 0.5]);
            let command = MovementInput {
                direction,
                sneak: true,
                sprint: true,
                ..Default::default()
            };
            for _ in 0..150 {
                p.tick(&world, command, &cfg);
                assert!(p.grounded, "{base} {direction:?}: {p:?}");
                near(p.position[1], 1.);
                assert_clear(&p, &world);
            }
            assert!(p.position[0] > f64::from(base) - 0.3 && p.position[0] < f64::from(base) + 1.3);
            assert!(p.position[2] > f64::from(base) - 0.3 && p.position[2] < f64::from(base) + 1.3);
            let contact = p.position;
            run(&mut p, &world, command, 180);
            assert_eq!(p.position, contact, "no edge jitter");
            run(
                &mut p,
                &world,
                MovementInput {
                    sneak: false,
                    ..command
                },
                60,
            );
            assert!(!p.grounded && p.position[1] < 1.);
        }
    }
}

#[test]
fn sneak_slides_along_a_ledge_and_traverses_chunk_seams_without_snagging() {
    let world = world_with((-24..=24).map(|z| [0, 0, z]));
    let mut p = Player::new([0.5, 1., -18.5]);
    let command = MovementInput {
        direction: [1., 1.],
        sneak: true,
        ..Default::default()
    };
    run(&mut p, &world, command, 1200);
    assert!(p.grounded && p.position[2] > 3. && p.position[0] < 1.3);
    near(p.position[1], 1.);
}

#[test]
fn crouch_changes_collision_and_eye_and_only_uncrouches_with_headroom() {
    let world = world_with(floor().chain([[0, 3, 0]]));
    let cfg = PhysicsConfig::default();
    let mut p = Player::new([0.5, 1.49, 0.5]);
    p.crouched = true;
    assert!(p.fits_in_world(&world, &cfg));
    near(p.height(&cfg), 1.5);
    near(p.eye_position(&cfg)[1], 2.81);
    assert!(!p.intersects_block(BlockPos([0, 3, 0]), &cfg));
    let standing = Player {
        crouched: false,
        ..p
    };
    assert!(!standing.fits_in_world(&world, &cfg));
    assert!(standing.intersects_block(BlockPos([0, 3, 0]), &cfg));
    for _ in 0..5 {
        p.tick(&world, MovementInput::default(), &cfg);
        assert!(p.crouched && p.fits_in_world(&world, &cfg));
    }
    run(&mut p, &world, MovementInput::default(), 60);
    assert!(!p.crouched && p.grounded);
    near(p.eye_position(&cfg)[1], 2.62);
}

#[test]
fn sneak_cannot_hover_after_support_is_mined_or_while_falling_and_jump_bypasses_guard() {
    let cfg = PhysicsConfig::default();
    let sneak = MovementInput {
        direction: [1., 0.],
        sneak: true,
        ..Default::default()
    };
    let mut world = world_with([[0, 0, 0]]);
    let mut p = Player::new([0.5, 1., 0.5]);
    p.tick(&world, sneak, &cfg);
    world.set_block(BlockPos([0, 0, 0]), Block::Air).unwrap();
    let old_x = p.position[0];
    p.tick(&world, sneak, &cfg);
    assert!(!p.grounded && p.velocity[1] < 0. && p.position[0] > old_x);
    let mut falling = Player::new([0.5, 8., 0.5]);
    falling.velocity = [3., -4., 0.];
    run(
        &mut falling,
        &World::default(),
        MovementInput {
            sneak: true,
            ..Default::default()
        },
        30,
    );
    near(falling.velocity[0], 3.); // no midair snap brake
    assert!(falling.position[0] > 1.9 && falling.position[1] < 8.);
    let world = world_with([[0, 0, 0]]);
    let mut jumping = Player::new([1.29, 1., 0.5]);
    jumping.tick(
        &world,
        MovementInput {
            jump: true,
            ..sneak
        },
        &cfg,
    );
    assert!(!jumping.grounded && jumping.velocity[1] > 0. && jumping.position[0] > 1.3);
}

#[test]
fn sprint_to_sneak_stops_at_ledge_and_missing_streamed_chunks_remain_barriers() {
    let cfg = PhysicsConfig::default();
    let mut p = Player::new([1.29, 1., 0.5]);
    p.velocity[0] = cfg.sprint_speed;
    let world = world_with([[0, 0, 0]]);
    p.tick(
        &world,
        MovementInput {
            sneak: true,
            direction: [1., 0.],
            ..Default::default()
        },
        &cfg,
    );
    assert!(p.grounded && p.position[0] < 1.3);
    let mut world = crate::streaming::new_world(42);
    for y in -1..=1 {
        world.insert_chunk(ChunkPos([0, y, 0]), Chunk::default());
    }
    for x in 0..16 {
        for z in 0..16 {
            world.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
        }
    }
    let mut p = Player::new([14.5, 1., 8.5]);
    run(
        &mut p,
        &world,
        MovementInput {
            direction: [1., 0.],
            sneak: true,
            ..Default::default()
        },
        180,
    );
    near(p.position[0], 15.7);
    assert!(p.cache_ready(&world, &cfg) && p.grounded);
}

#[test]
fn twenty_block_movement_profile_records_speed_and_time_comparisons() {
    let world = world_with(floor());
    let old = PhysicsConfig {
        walk_speed: 4.3,
        sprint_speed: 5.6,
        ground_acceleration: 55.,
        ground_braking: 60.,
        ..Default::default()
    };
    for (label, config, sprint, sneak, expected_ticks) in [
        ("Slice10 walk", old, false, false, 281),
        ("Slice10 sprint", old, true, false, 217),
        ("Slice11 walk", PhysicsConfig::default(), false, false, 242),
        ("Slice11 sprint", PhysicsConfig::default(), true, false, 154),
        ("Slice11 sneak", PhysicsConfig::default(), false, true, 668),
    ] {
        let mut p = Player::new([0.5, 1., 0.5]);
        let command = MovementInput {
            direction: [1., 0.],
            sprint,
            sneak,
            jump: false,
        };
        let mut ticks = 0;
        while p.position[0] < 20.5 && ticks < 1000 {
            p.tick(&world, command, &config);
            ticks += 1;
        }
        println!(
            "{label}: {ticks} ticks, {:.3}s, {:.3} blocks/s",
            f64::from(ticks) * PHYSICS_DT,
            p.velocity[0]
        );
        assert_eq!(ticks, expected_ticks, "{label}");
        assert!(p.grounded);
    }
}

#[test]
fn crouched_diagonal_contacts_stay_clear_and_never_backtrack() {
    let cfg = PhysicsConfig::default();
    for base in [-16, 0, 16, 16000] {
        let ground = (-4..=4).flat_map(|x| (-4..=4).map(move |z| [base + x, 0, base + z]));
        let world = world_with(ground.chain([[base + 1, 1, base], [base + 2, 1, base + 1]]));
        let mut p = Player::new([f64::from(base) + 3.5, 1., f64::from(base) + 0.5]);
        let command = MovementInput {
            direction: [-1., 1.],
            sneak: true,
            ..Default::default()
        };
        for _ in 0..300 {
            let old = p.position;
            p.tick(&world, command, &cfg);
            assert!(p.fits_in_world(&world, &cfg) && p.grounded);
            assert!(
                p.position[0] <= old[0] + CONTACT_EPSILON
                    && p.position[2] >= old[2] - CONTACT_EPSILON
            );
        }
        near(p.position[0], f64::from(base) + 2.3);
        near(p.position[2], f64::from(base) + 0.7);
        p.tick(
            &world,
            MovementInput {
                jump: true,
                ..command
            },
            &cfg,
        );
        assert!(!p.grounded && p.velocity[1] > 0.);
    }
}

#[test]
fn pending_headroom_prevents_uncrouching_until_the_upper_chunk_is_loaded() {
    let cfg = PhysicsConfig::default();
    let mut world = crate::streaming::new_world(42);
    world.insert_chunk(ChunkPos([0, 0, 0]), Chunk::default());
    let mut p = Player::new([0.5, 14.5, 0.5]);
    p.crouched = true;
    assert!(p.cache_ready(&world, &cfg));
    p.tick(&world, MovementInput::default(), &cfg);
    assert!(p.crouched && p.fits_in_world(&world, &cfg));
    world.insert_chunk(ChunkPos([0, 1, 0]), Chunk::default());
    p.tick(&world, MovementInput::default(), &cfg);
    assert!(!p.crouched && p.cache_ready(&world, &cfg));
}

#[test]
fn held_jumps_launch_only_after_landing_and_stop_after_release() {
    let world = world_with(floor());
    let cfg = PhysicsConfig::default();
    let mut p = Player::new([0.5, 1., 0.5]);
    let mut launches = 0;
    for _ in 0..600 {
        let was_supported = p.grounded || p.position[1] == 1.;
        let old_vy = p.velocity[1];
        p.tick(
            &world,
            MovementInput {
                jump: true,
                ..Default::default()
            },
            &cfg,
        );
        if p.velocity[1] > 0.0 && p.velocity[1] > old_vy {
            launches += 1;
            assert!(was_supported && old_vy <= 0.);
        }
        assert_clear(&p, &world);
    }
    assert_eq!(launches, 15);
    run(&mut p, &world, MovementInput::default(), 120);
    assert!(p.grounded);
    near(p.position[1], 1.);
    let mut blocked = Player::new([0.5, 1., 0.5]);
    let roof = world_with(floor().chain([[0, 3, 0]]));
    let tall = PhysicsConfig { height: 2., ..cfg };
    for _ in 0..120 {
        blocked.tick(
            &roof,
            MovementInput {
                jump: true,
                sprint: true,
                ..Default::default()
            },
            &tall,
        );
        near(blocked.position[1], 1.);
        near(blocked.velocity[1], 0.);
        near(blocked.velocity[0].hypot(blocked.velocity[2]), 0.);
        assert!(blocked.grounded);
    }
    let mut unsupported = Player::new([0.5, 1., 0.5]);
    unsupported.grounded = true;
    unsupported.tick(
        &World::default(),
        MovementInput {
            jump: true,
            ..Default::default()
        },
        &cfg,
    );
    assert!(unsupported.velocity[1] < 0.);
}

#[test]
fn sprint_jump_bonus_is_capped_normalized_and_has_no_standing_sneak_wall_or_roof_boost() {
    let cfg = PhysicsConfig::default();
    let ground = || (-4..=128).flat_map(|x| (-4..=128).map(move |z| [x, 0, z]));
    let world = world_with(ground());
    for direction in [[1., 0.], [1., 1.]] {
        let mut p = Player::new([0.5, 1., 0.5]);
        let command = MovementInput {
            direction,
            sprint: true,
            jump: true,
            ..Default::default()
        };
        let mut maximum: f64 = 0.;
        for _ in 0..600 {
            p.tick(&world, command, &cfg);
            let speed = p.velocity[0].hypot(p.velocity[2]);
            maximum = maximum.max(speed);
            assert!(speed <= 9.2 + 1e-8);
            assert_clear(&p, &world);
        }
        near(maximum, 9.2);
    }
    let mut standing = Player::new([0.5, 1., 0.5]);
    standing.tick(
        &world,
        MovementInput {
            sprint: true,
            jump: true,
            ..Default::default()
        },
        &cfg,
    );
    near(standing.velocity[0].hypot(standing.velocity[2]), 0.);
    for sneak in [false, true] {
        let mut p = Player::new([0.5, 1., 0.5]);
        p.velocity[0] = 8.;
        let roof = world_with(ground().chain([[0, 3, 0]]));
        p.tick(
            &roof,
            MovementInput {
                direction: [1., 0.],
                sprint: true,
                jump: true,
                sneak,
            },
            &cfg,
        );
        assert!(p.velocity[0] <= if sneak { 1.8 + 1e-8 } else { 8. });
    }
    for pending in [false, true] {
        let mut world = if pending {
            crate::streaming::new_world(42)
        } else {
            world_with(ground())
        };
        if pending {
            for y in -1..=1 {
                world.insert_chunk(ChunkPos([0, y, 0]), Chunk::default());
            }
            for x in 0..16 {
                for z in 0..16 {
                    world.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
                }
            }
        } else {
            for y in 1..=4 {
                world.set_block(BlockPos([16, y, 0]), Block::Stone).unwrap();
            }
        }
        let mut p = Player::new([15.7, 1., 0.5]);
        p.velocity[0] = 8.;
        p.tick(
            &world,
            MovementInput {
                direction: [1., 0.],
                sprint: true,
                jump: true,
                ..Default::default()
            },
            &cfg,
        );
        near(p.position[0], 15.7);
        near(p.velocity[0], 0.);
    }
}

#[test]
fn flat_course_sprint_jumping_has_a_modest_measured_advantage() {
    let world = world_with((-4..=128).flat_map(|x| (-2..=2).map(move |z| [x, 0, z])));
    let cfg = PhysicsConfig::default();
    let mut times = Vec::new();
    for (label, sprint, jump) in [
        ("walk", false, false),
        ("Ctrl or double-W sprint", true, false),
        ("sprint + held jump", true, true),
    ] {
        let mut p = Player::new([0.5, 1., 0.5]);
        let command = MovementInput {
            direction: [1., 0.],
            sprint,
            jump,
            ..Default::default()
        };
        let mut ticks = 0;
        let mut at_twenty = 0;
        while p.position[0] < 100.5 && ticks < 1500 {
            p.tick(&world, command, &cfg);
            ticks += 1;
            if at_twenty == 0 && p.position[0] >= 20.5 {
                at_twenty = ticks;
            }
        }
        assert!(ticks < 1500);
        println!(
            "{label}: 20 blocks {:.3}s, 100 blocks {:.3}s, average {:.3} blocks/s",
            f64::from(at_twenty) * PHYSICS_DT,
            f64::from(ticks) * PHYSICS_DT,
            100. / (f64::from(ticks) * PHYSICS_DT)
        );
        times.push(ticks);
    }
    assert!(times[2] < times[1] && f64::from(times[1]) / f64::from(times[2]) < 1.16);
}
