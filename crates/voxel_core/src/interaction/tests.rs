use super::*;
use crate::raycast::DEFAULT_REACH;
use crate::{BlockPos, ChunkPos, EDIT_MAX, EDIT_MIN, mesh_chunk};

fn ray() -> Ray {
    Ray {
        origin: [0.5, 2.5, 0.5],
        direction: [0.0, 0.0, -1.0],
        reach: DEFAULT_REACH,
    }
}
fn far_player() -> Player {
    Player::new([10.5, 3.0, 10.5])
}
fn config() -> PhysicsConfig {
    PhysicsConfig::default()
}

#[test]
fn mutation_reports_changes_noops_and_bounds_without_allocating_empty_chunks() {
    let mut world = World::default();
    let p = BlockPos([-1, 0, 16]);
    assert_eq!(world.set_block(p, Block::Air), Ok(None));
    assert_eq!(world.chunks().count(), 0);
    let change = world.set_block(p, Block::Stone).unwrap().unwrap();
    assert_eq!(change.before, Block::Air);
    assert_eq!(change.after, Block::Stone);
    assert_eq!(world.block(p), Block::Stone);
    assert_eq!(world.chunks().count(), 1);
    assert_eq!(world.set_block(p, Block::Stone), Ok(None));
    world.set_block(p, Block::Air).unwrap();
    assert_eq!(world.block(p), Block::Air);
    for axis in 0..3 {
        for outside in [EDIT_MIN[axis] - 1, EDIT_MAX[axis]] {
            let mut position = [0; 3];
            position[axis] = outside;
            assert_eq!(
                world.set_block(BlockPos(position), Block::Stone),
                Err(EditError::OutOfBounds)
            );
        }
    }
    assert_eq!(world.chunks().count(), 1);
}

#[test]
fn selection_slots_only_offer_solid_blocks() {
    for slot in 1..=3 {
        let choice = BuildBlock::from_slot(slot).unwrap();
        assert!(choice.block().is_solid());
        assert_eq!(choice.slot(), slot);
        assert!(!choice.name().is_empty());
    }
    assert_eq!(BuildBlock::from_slot(0), None);
    assert_eq!(BuildBlock::from_slot(4), None);
}

#[test]
fn no_target_and_repeated_break_clicks_are_safe() {
    let mut world = World::default();
    assert_eq!(
        edit(
            &mut world,
            ray(),
            EditAction::Break,
            &far_player(),
            &config()
        ),
        Ok(None)
    );
    world.set_block(BlockPos([0, 2, -2]), Block::Stone).unwrap();
    assert!(
        edit(
            &mut world,
            ray(),
            EditAction::Break,
            &far_player(),
            &config()
        )
        .unwrap()
        .is_some()
    );
    assert_eq!(
        edit(
            &mut world,
            ray(),
            EditAction::Break,
            &far_player(),
            &config()
        ),
        Ok(None)
    );
    assert_eq!(world.block(BlockPos([0, 2, -2])), Block::Air);
}

#[test]
fn placement_uses_selected_material_then_break_retargets_the_updated_world() {
    let mut world = World::default();
    world.set_block(BlockPos([0, 2, -2]), Block::Stone).unwrap();
    let change = edit(
        &mut world,
        ray(),
        EditAction::Place(BuildBlock::Dirt),
        &far_player(),
        &config(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(change.position, BlockPos([0, 2, -1]));
    assert_eq!(world.block(change.position), Block::Dirt);
    edit(
        &mut world,
        ray(),
        EditAction::Break,
        &far_player(),
        &config(),
    )
    .unwrap();
    assert_eq!(world.block(change.position), Block::Air);
    assert_eq!(world.block(BlockPos([0, 2, -2])), Block::Stone);
}

#[test]
fn player_overlap_covers_feet_head_corners_and_allows_face_contact() {
    let player = Player::new([0.5, 1.0, 0.5]);
    for block in [[0, 1, 0], [0, 2, 0]] {
        assert!(player.intersects_block(BlockPos(block), &config()));
    }
    for block in [[0, 0, 0], [1, 1, 0], [0, 3, 0]] {
        assert!(!player.intersects_block(BlockPos(block), &config()));
    }
    let corner = Player::new([1.29, 1.999, 1.29]);
    assert!(corner.intersects_block(BlockPos([0, 1, 0]), &config()));
    let touching = Player::new([1.3, 1.0, 0.5]);
    assert!(!touching.intersects_block(BlockPos([0, 1, 0]), &config()));
    let mut world = World::default();
    world.set_block(BlockPos([0, 2, -1]), Block::Stone).unwrap();
    let before = world.clone();
    assert_eq!(
        edit(
            &mut world,
            ray(),
            EditAction::Place(BuildBlock::Grass),
            &player,
            &config()
        ),
        Err(EditError::IntersectsPlayer)
    );
    assert_eq!(world, before);
}

#[test]
fn starting_inside_solid_cannot_replace_it_or_invent_a_placement_face() {
    let mut world = World::default();
    world.set_block(BlockPos([0, 2, 0]), Block::Stone).unwrap();
    let before = world.clone();
    assert_eq!(
        edit(
            &mut world,
            ray(),
            EditAction::Place(BuildBlock::Dirt),
            &far_player(),
            &config()
        ),
        Err(EditError::NoPlacementFace)
    );
    assert_eq!(world, before);
}

#[test]
fn placement_creates_missing_neighbor_storage_and_updates_boundary_faces() {
    let mut world = World::default();
    world.set_block(BlockPos([15, 2, 0]), Block::Stone).unwrap();
    let ray = Ray {
        origin: [17.5, 2.5, 0.5],
        direction: [-1.0, 0.0, 0.0],
        reach: 5.0,
    };
    let change = edit(
        &mut world,
        ray,
        EditAction::Place(BuildBlock::Grass),
        &far_player(),
        &config(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(change.position, BlockPos([16, 2, 0]));
    assert!(change.affected_chunks().contains(&ChunkPos([0, 0, 0])));
    assert!(change.affected_chunks().contains(&ChunkPos([1, 0, 0])));
    let faces: usize = world
        .chunks()
        .map(|(p, _)| mesh_chunk(&world, p).indices.len() / 6)
        .sum();
    assert_eq!(faces, 10);
}

#[test]
fn dirty_neighbors_include_faces_but_not_diagonals_on_negative_seams() {
    let mut world = World::default();
    for (p, expected) in [
        ([1, 1, 1], 1),
        ([0, 1, 1], 2),
        ([-1, 1, 1], 2),
        ([-16, 0, 0], 4),
        ([15, 15, 15], 4),
    ] {
        let change = world.set_block(BlockPos(p), Block::Stone).unwrap().unwrap();
        assert_eq!(change.affected_chunks().len(), expected);
        let (owner, _) = BlockPos(p).split();
        for neighbor in change.affected_chunks() {
            let distance: i32 = (0..3)
                .map(|axis| (owner.0[axis] - neighbor.0[axis]).abs())
                .sum();
            assert!(distance <= 1);
        }
    }
}

#[test]
fn edits_change_physics_collision_immediately() {
    let mut world = World::default();
    world.set_block(BlockPos([0, 0, 0]), Block::Stone).unwrap();
    let mut player = Player::new([0.5, 1.0, 0.5]);
    player.tick(&world, Default::default(), &config());
    assert!(player.grounded);
    world.set_block(BlockPos([0, 0, 0]), Block::Air).unwrap();
    player.tick(&world, Default::default(), &config());
    assert!(!player.grounded);
    assert!(player.velocity[1] < 0.0);
    // Reintroducing a floor below the feet restores support in the same query.
    player = Player::new([0.5, 1.0, 0.5]);
    world.set_block(BlockPos([0, 0, 0]), Block::Stone).unwrap();
    player.tick(&world, Default::default(), &config());
    assert!(player.grounded);
}

#[test]
fn placing_past_the_editable_box_is_rejected_without_allocating_storage() {
    let mut world = World::default();
    world.set_block(BlockPos([31, 1, 0]), Block::Stone).unwrap();
    let before = world.clone();
    let ray = Ray {
        origin: [33.5, 1.5, 0.5],
        direction: [-1.0, 0.0, 0.0],
        reach: 5.0,
    };
    assert_eq!(
        edit(
            &mut world,
            ray,
            EditAction::Place(BuildBlock::Stone),
            &far_player(),
            &config()
        ),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(world, before);
}
