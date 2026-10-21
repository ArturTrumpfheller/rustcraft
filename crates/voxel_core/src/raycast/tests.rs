use super::*;
use crate::Block;

fn world(blocks: &[[i32; 3]]) -> World {
    let mut world = World::default();
    for &block in blocks {
        world.set_block(BlockPos(block), Block::Stone).unwrap();
    }
    world
}
fn ray(origin: [f64; 3], direction: [f64; 3], reach: f64) -> Ray {
    Ray {
        origin,
        direction,
        reach,
    }
}

#[test]
fn axis_hits_report_face_distance_and_adjacent_cell_in_both_directions() {
    for axis in 0..3 {
        for sign in [-1, 1] {
            let mut block = [0; 3];
            block[axis] = sign * 3;
            let mut direction = [0.0; 3];
            direction[axis] = f64::from(sign) * 10.0;
            let hit = raycast(&world(&[block]), ray([0.5; 3], direction, 5.0)).unwrap();
            assert_eq!(hit.position, BlockPos(block));
            let mut normal = [0; 3];
            normal[axis] = -sign;
            assert_eq!(hit.normal, normal);
            assert!((hit.distance - 2.5).abs() < 1e-10);
            assert_eq!(hit.adjacent, BlockPos(block).offset(normal));
        }
    }
}

#[test]
fn misses_empty_world_and_skips_unrelated_solid_cells() {
    assert!(raycast(&World::default(), ray([0.5; 3], [0.0, 1.0, 0.0], 5.0)).is_none());
    assert!(
        raycast(
            &world(&[[1, 1, 0], [-1, 0, 0]]),
            ray([0.5; 3], [1.0, 0.0, 0.0], 5.0)
        )
        .is_none()
    );
}

#[test]
fn reach_is_inclusive_and_blocks_behind_it_are_ignored() {
    let world = world(&[[3, 0, 0]]);
    assert!(raycast(&world, ray([0.5; 3], [1.0, 0.0, 0.0], 2.5)).is_some());
    assert!(raycast(&world, ray([0.5; 3], [1.0, 0.0, 0.0], 2.499)).is_none());
    assert!(raycast(&world, ray([0.5; 3], [1.0, 0.0, 0.0], 0.0)).is_none());
}

#[test]
fn inside_solid_has_zero_distance_without_inventing_a_placement_face() {
    let hit = raycast(&world(&[[0, 0, 0]]), ray([0.5; 3], [1.0, 1.0, 1.0], 0.0)).unwrap();
    assert_eq!(hit.distance, 0.0);
    assert_eq!(hit.normal, [0; 3]);
    assert_eq!(hit.adjacent, None);
}

#[test]
fn negative_origins_and_positive_negative_chunk_seams_work_on_every_axis() {
    for axis in 0..3 {
        for seam in [-16, 0, 16] {
            for sign in [-1, 1] {
                let mut block = [0; 3];
                block[axis] = seam;
                let mut origin = [0.5; 3];
                origin[axis] = f64::from(seam) + 0.5 - f64::from(sign) * 2.0;
                let mut direction = [0.0; 3];
                direction[axis] = f64::from(sign);
                let hit = raycast(&world(&[block]), ray(origin, direction, 5.0)).unwrap();
                assert_eq!(hit.position, BlockPos(block));
                assert!((hit.distance - 1.5).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn exact_boundaries_enter_negative_cell_at_distance_zero_and_parallel_axes_stay_put() {
    let hit = raycast(
        &world(&[[-1, 0, 0]]),
        ray([0.0, 0.5, 0.5], [-1.0, 0.0, 0.0], 1.0),
    )
    .unwrap();
    assert_eq!(hit.distance, 0.0);
    assert_eq!(hit.normal, [1, 0, 0]);
    assert_eq!(hit.adjacent, Some(BlockPos([0, 0, 0])));
    assert!(
        raycast(
            &world(&[[0, -1, 0]]),
            ray([0.5, 0.0, 0.5], [1.0, 0.0, 0.0], 2.0)
        )
        .is_none()
    );
}

#[test]
fn diagonal_corner_ties_step_x_then_y_then_z_with_face_adjacent_placement() {
    let ray = ray([0.5; 3], [1.0; 3], 5.0);
    let hit = raycast(&world(&[[1, 0, 0], [0, 1, 0], [1, 1, 1]]), ray).unwrap();
    assert_eq!(hit.position, BlockPos([1, 0, 0]));
    assert_eq!(hit.normal, [-1, 0, 0]);
    let hit = raycast(&world(&[[1, 1, 1]]), ray).unwrap();
    assert_eq!(hit.position, BlockPos([1; 3]));
    assert_eq!(hit.normal, [0, 0, -1]);
    assert_eq!(hit.adjacent, Some(BlockPos([1, 1, 0])));
    assert!((hit.distance - 3.0_f64.sqrt() * 0.5).abs() < 1e-10);
}

#[test]
fn near_ties_and_negative_diagonals_have_repeatable_results() {
    let world = world(&[[-1, 0, 0], [-1, -1, -1]]);
    let ray = ray([0.5; 3], [-1.0, -1.0 - 1e-12, -1.0], 5.0);
    let hit = raycast(&world, ray).unwrap();
    assert_eq!(hit.position, BlockPos([-1, 0, 0]));
    for _ in 0..100 {
        assert_eq!(raycast(&world, ray), Some(hit));
    }
}

#[test]
fn invalid_inputs_and_coordinate_overflow_miss_without_unbounded_traversal() {
    let world = World::default();
    for direction in [[0.0; 3], [f64::NAN, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0]] {
        assert!(raycast(&world, ray([0.5; 3], direction, 5.0)).is_none());
    }
    for reach in [-1.0, f64::NAN, f64::INFINITY, MAX_REACH + 1.0] {
        assert!(raycast(&world, ray([0.5; 3], [1.0, 0.0, 0.0], reach)).is_none());
    }
    for origin in [
        [f64::NAN; 3],
        [f64::from(i32::MAX) + 1.0; 3],
        [f64::from(i32::MIN) - 1.0; 3],
    ] {
        assert!(raycast(&world, ray(origin, [1.0, 0.0, 0.0], 5.0)).is_none());
    }
    assert!(
        raycast(
            &world,
            ray([f64::from(i32::MAX) + 0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 5.0)
        )
        .is_none()
    );
}
