use crate::{
    inventory::*,
    physics::{PhysicsConfig, Player},
    raycast::*,
    *,
};
fn world() -> World {
    let mut w = World::default();
    for x in -10..=10 {
        for z in -10..=10 {
            w.set_block(BlockPos([x, 0, z]), Block::Stone).unwrap();
        }
    }
    w
}
fn ray() -> Ray {
    Ray {
        origin: [0.5, 4., 0.5],
        direction: [0., -1., 0.],
        reach: 5.,
    }
}
fn use_item(w: &mut World, i: &mut Inventory) -> Result<Option<EditBatch>, InventoryEditError> {
    edit_inventory(
        w,
        i,
        ray(),
        true,
        &Player::new([8., 1., 8.]),
        &PhysicsConfig::default(),
    )
}
fn inventory(item: Item, count: u8) -> Inventory {
    let mut slots = [None; SLOT_COUNT];
    slots[0] = Stack::new(item, count);
    Inventory::from_parts(slots, None, 0).unwrap()
}
#[test]
fn buckets_exchange_sources_and_wake_recession_without_mining_water() {
    let mut w = world();
    let mut i = inventory(Item::WaterBucket, 1);
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::WaterSource);
    assert_eq!(i.selected_stack().unwrap().item(), Item::EmptyBucket);
    let mut changes = Vec::new();
    for _ in 0..30 {
        w.tick_fluids(fluid::FluidConfig::default(), &mut changes);
    }
    assert!(w.water_cells() > 1);
    assert_eq!(raycast(&w, ray()).unwrap().position, BlockPos([0, 0, 0]));
    assert_eq!(
        raycast_with(&w, ray(), TargetMode::CollectSource)
            .unwrap()
            .position,
        BlockPos([0, 1, 0])
    );
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(i.selected_stack().unwrap().item(), Item::WaterBucket);
    for _ in 0..100 {
        w.tick_fluids(fluid::FluidConfig::default(), &mut changes);
    }
    assert_eq!(w.water_cells(), 0);
    assert_eq!(w.fluid_updates().pending(), 0);
}
#[test]
fn collection_skips_flowing_water_and_stops_at_occluding_blocks() {
    let mut w = world();
    let mut i = inventory(Item::EmptyBucket, 1);
    w.set_block(BlockPos([0, 1, 0]), Block::Water3).unwrap();
    let before = i.clone();
    assert!(use_item(&mut w, &mut i).unwrap().is_none());
    assert_eq!(i, before);
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    w.set_block(BlockPos([0, 2, 0]), Block::WaterFalling)
        .unwrap();
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(w.block(BlockPos([0, 2, 0])), Block::WaterFalling);
    i = inventory(Item::EmptyBucket, 1);
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    w.set_block(BlockPos([0, 2, 0]), Block::Stone).unwrap();
    assert!(use_item(&mut w, &mut i).unwrap().is_none());
    assert_eq!(i.selected_stack().unwrap().item(), Item::EmptyBucket);
}

#[test]
fn source_targeting_respects_fluid_height_and_interaction_reach() {
    let mut w = world();
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    w.set_block(BlockPos([2, 1, 0]), Block::Stone).unwrap();
    let mut ray = Ray {
        origin: [-1., 1.95, 0.5],
        direction: [1., 0., 0.],
        reach: 5.,
    };
    assert_eq!(
        raycast_with(&w, ray, TargetMode::CollectSource)
            .unwrap()
            .position,
        BlockPos([2, 1, 0])
    );
    ray.origin[1] = 1.5;
    assert_eq!(
        raycast_with(&w, ray, TargetMode::CollectSource)
            .unwrap()
            .position,
        BlockPos([0, 1, 0])
    );
    assert_eq!(raycast(&w, ray).unwrap().position, BlockPos([2, 1, 0]));
    ray.reach = 0.99;
    assert!(raycast_with(&w, ray, TargetMode::CollectSource).is_none());
}
#[test]
fn full_inventory_stack_exchange_is_atomic_and_can_reuse_the_last_bucket_slot() {
    let mut w = world();
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    let mut slots = [Stack::new(Item::Stone, 64); SLOT_COUNT];
    slots[0] = Stack::new(Item::EmptyBucket, 2);
    let mut i = Inventory::from_parts(slots, None, 0).unwrap();
    let before = i.clone();
    let revision = w.revision();
    assert_eq!(use_item(&mut w, &mut i), Err(InventoryEditError::Full));
    assert_eq!(i, before);
    assert_eq!(w.revision(), revision);
    slots[0] = Stack::new(Item::EmptyBucket, 1);
    i = Inventory::from_parts(slots, None, 0).unwrap();
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(i.selected_stack().unwrap().item(), Item::WaterBucket);
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(i.selected_stack().unwrap().item(), Item::EmptyBucket);
}
#[test]
fn stack_limits_catalog_grants_and_explicit_legacy_conversion_preserve_ids() {
    for id in 1..=9 {
        assert_eq!(Item::from_id(id).unwrap().id(), id);
    }
    assert!(Stack::new(Item::WaterBucket, 2).is_none());
    assert!(Stack::new(Item::EmptyBucket, 17).is_none());
    let mut i = inventory(Item::WaterSource, 2);
    let initial = i.clone();
    assert!(i.selected_stack().unwrap().item().build().is_none());
    let mut w = world();
    assert_eq!(use_item(&mut w, &mut i), Err(InventoryEditError::Empty));
    assert_eq!(i, initial);
    assert!(i.convert_legacy_water());
    assert_eq!(i.slots()[0].unwrap().count(), 1);
    assert_eq!(i.slots()[1].unwrap().item(), Item::WaterBucket);
    let mut full = inventory(Item::Stone, 64);
    full.insert(Item::Stone, 35 * 64);
    let initial = full.clone();
    assert!(!full.grant(Item::EmptyBucket, 16));
    assert_eq!(full, initial);
    assert!(Item::CREATIVE.contains(&Item::Torch) && Item::CREATIVE.contains(&Item::WaterBucket));
}
#[test]
fn ordinary_mining_and_placement_reach_solids_through_sources_and_keep_body_protection() {
    let mut w = world();
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    let mut i = inventory(Item::Dirt, 2);
    let player = Player::new([0.5, 1., 0.5]);
    let cfg = PhysicsConfig::default();
    let before = i.clone();
    assert_eq!(
        edit_inventory(&mut w, &mut i, ray(), true, &player, &cfg),
        Err(InventoryEditError::World(
            interaction::EditError::IntersectsPlayer
        ))
    );
    assert_eq!(i, before);
    use_item(&mut w, &mut i).unwrap().unwrap();
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::Dirt);
    w.set_block(BlockPos([0, 1, 0]), Block::WaterSource)
        .unwrap();
    edit_inventory(
        &mut w,
        &mut i,
        ray(),
        false,
        &Player::new([8., 1., 8.]),
        &cfg,
    )
    .unwrap()
    .unwrap();
    assert_eq!(w.block(BlockPos([0, 0, 0])), Block::Air);
    assert_eq!(w.block(BlockPos([0, 1, 0])), Block::WaterSource);
    w.set_block(BlockPos([0, 1, 0]), Block::Water1).unwrap();
    assert!(raycast(&w, ray()).is_none());
}
#[test]
fn rejected_boundary_and_pending_bucket_placements_do_not_consume_items() {
    let mut w = world();
    w.set_block(BlockPos([0, 31, 0]), Block::Stone).unwrap();
    let r = Ray {
        origin: [0.5, 34., 0.5],
        ..ray()
    };
    let mut i = inventory(Item::WaterBucket, 1);
    let before = i.clone();
    assert_eq!(
        edit_inventory(
            &mut w,
            &mut i,
            r,
            true,
            &Player::new([8., 1., 8.]),
            &PhysicsConfig::default()
        ),
        Err(InventoryEditError::World(
            interaction::EditError::OutOfBounds
        ))
    );
    assert_eq!(i, before);
    let mut pending = streaming::new_large_world(42);
    assert_eq!(
        use_item(&mut pending, &mut i),
        Err(InventoryEditError::World(interaction::EditError::NotLoaded))
    );
    assert_eq!(i, before);
}
#[test]
fn bucket_displaces_every_torch_orientation_once_and_never_arbitrary_solids() {
    for torch in [
        Block::Torch,
        Block::TorchEast,
        Block::TorchWest,
        Block::TorchSouth,
        Block::TorchNorth,
    ] {
        let mut w = world();
        let p = BlockPos([0, 1, 0]);
        w.set_block(
            p.offset(torch.torch_support().unwrap()).unwrap(),
            Block::Stone,
        )
        .unwrap();
        w.set_block(p, torch).unwrap();
        let point = torch.shape_transform().point([0.5, 0.35, 0.5]);
        let r = Ray {
            origin: [point[0], 3., point[2]],
            direction: [0., -1., 0.],
            reach: 5.,
        };
        assert_eq!(raycast(&w, r).unwrap().position, p);
        let mut i = inventory(Item::WaterBucket, 1);
        let batch = edit_inventory(
            &mut w,
            &mut i,
            r,
            true,
            &Player::new([8., 1., 8.]),
            &PhysicsConfig::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(batch.changes()[0].before, torch);
        assert_eq!(w.block(p), Block::WaterSource);
        assert_eq!(w.dropped_items().len(), 1);
        assert_eq!(
            w.dropped_items().iter().next().unwrap().1.item(),
            Item::Torch
        );
        let mut changes = Vec::new();
        for _ in 0..40 {
            w.tick_fluids(fluid::FluidConfig::default(), &mut changes);
        }
        assert_eq!(w.dropped_items().len(), 1);
    }
}
