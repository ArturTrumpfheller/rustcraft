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
                jump: false,
            },
            60,
        );
        run(
            &mut diagonal,
            &world,
            MovementInput {
                direction: [1.0, 1.0],
                sprint,
                jump: false,
            },
            60,
        );
        near(straight.velocity[0], if sprint { 5.6 } else { 4.3 });
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
