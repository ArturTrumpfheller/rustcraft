//! Bounded mining transactions; test snapshots may clone worlds, gameplay does not.
use crate::{
    Block, BlockPos, EditBatch, World,
    interaction::EditError,
    inventory::{Inventory, InventoryEditError, Item, Stack, edit_inventory},
    physics::{PhysicsConfig, Player},
    raycast::Ray,
};
const ATTACHMENTS: [[i32; 3]; 5] = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
fn fixture(p: BlockPos, attachments: &[usize]) -> World {
    let mut w = World::default();
    w.set_block(p, Block::Stone).unwrap();
    for &i in attachments {
        let d = ATTACHMENTS[i];
        w.set_block(p.offset(d).unwrap(), Block::torch_on_face(d).unwrap())
            .unwrap();
    }
    w
}
pub(crate) fn mine(
    w: &mut World,
    inv: &mut Inventory,
    p: BlockPos,
) -> Result<Option<EditBatch>, InventoryEditError> {
    edit_inventory(
        w,
        inv,
        Ray {
            origin: p.0.map(|v| f64::from(v) + 0.5),
            direction: [0., -1., 0.],
            reach: 0.,
        },
        false,
        &Player::new([100., 30., 100.]),
        &PhysicsConfig::default(),
    )
}
pub(crate) fn count(inv: &Inventory, item: Item) -> u16 {
    inv.slots()
        .iter()
        .copied()
        .chain([inv.cursor()])
        .flatten()
        .filter(|s| s.item() == item)
        .map(|s| u16::from(s.count()))
        .sum()
}
#[test]
fn standing_wall_multiple_and_five_dependents_collect_all_items_in_one_revision() {
    let p = BlockPos([0, 3, 0]);
    for attachments in [
        &[][..],
        &[0],
        &[1],
        &[2],
        &[3],
        &[4],
        &[1, 2, 3],
        &[1, 2, 3, 4],
        &[0, 1, 2, 3, 4],
    ] {
        let mut w = fixture(p, attachments);
        let mut inv = Inventory::empty();
        let before = w.revision();
        let batch = mine(&mut w, &mut inv, p).unwrap().unwrap();
        assert_eq!(batch.changes().len(), 1 + attachments.len());
        assert_eq!(w.revision(), before + 1);
        assert_eq!(count(&inv, Item::Stone), 1);
        assert_eq!(count(&inv, Item::Torch), attachments.len() as u16);
        for c in batch.changes() {
            assert_eq!(w.block(c.position), Block::Air);
        }
        let before = inv.clone();
        assert!(mine(&mut w, &mut inv, p).unwrap().is_none());
        assert_eq!(inv, before);
    }
}
#[test]
fn inventory_capacity_is_preflighted_for_the_support_and_every_torch() {
    let p = BlockPos([0, 3, 0]);
    for (stone, torches, success) in [(63, 60, false), (64, 59, false), (63, 59, true)] {
        let mut slots = [Stack::new(Item::Dirt, 64); 36];
        slots[0] = Stack::new(Item::Stone, stone);
        slots[1] = Stack::new(Item::Torch, torches);
        let mut inv = Inventory::from_parts(slots, None, 0).unwrap();
        let mut w = fixture(p, &[0, 1, 2, 3, 4]);
        let before = w.clone();
        let items = inv.clone();
        let result = mine(&mut w, &mut inv, p);
        if success {
            assert_eq!(result.unwrap().unwrap().changes().len(), 6);
            assert_eq!(count(&inv, Item::Stone), 64);
            assert_eq!(count(&inv, Item::Torch), 64);
        } else {
            assert_eq!(result, Err(InventoryEditError::Full));
            assert_eq!(w, before);
            assert_eq!(inv, items);
        }
    }
}
#[test]
fn rapid_batches_and_stale_plans_do_not_duplicate_items_across_signed_chunk_seams() {
    for p in [BlockPos([15, 15, 15]), BlockPos([-16, 15, -16])] {
        let mut inv = Inventory::empty();
        let mut w = World::default();
        for n in 1..=60 {
            w.set_block(p, Block::Stone).unwrap();
            for d in ATTACHMENTS {
                w.set_block(p.offset(d).unwrap(), Block::torch_on_face(d).unwrap())
                    .unwrap();
            }
            let stale = w.plan_break(p).unwrap().unwrap();
            let batch = mine(&mut w, &mut inv, p).unwrap().unwrap();
            assert_eq!(batch, stale);
            let before = w.clone();
            assert_eq!(w.commit_break(stale), Err(EditError::StaleEdit));
            assert_eq!(w, before);
            assert_eq!(count(&inv, Item::Stone), n);
            assert_eq!(count(&inv, Item::Torch), n * 5);
        }
    }
}
#[test]
fn removing_several_emitters_retains_unrelated_light_and_preserves_sky() {
    use crate::lighting::Skylight;
    let p = BlockPos([0, 3, 0]);
    let mut w = fixture(p, &[0, 1, 2, 3, 4]);
    for x in -8..=8 {
        for y in 0..=8 {
            for z in -8..=8 {
                if x == -8 || x == 8 || y == 0 || y == 8 || z == -8 || z == 8 {
                    w.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
                }
            }
        }
    }
    let other = BlockPos([5, 1, 0]);
    w.set_block(other, Block::Torch).unwrap();
    let targets = w.chunks().map(|(p, _)| p).collect();
    let before = Skylight::for_chunks(&w, &targets);
    assert_eq!(before.block_level(BlockPos([-1, 3, 0])), 14);
    let mut inv = Inventory::empty();
    mine(&mut w, &mut inv, p).unwrap().unwrap();
    let after = Skylight::for_chunks(&w, &targets);
    assert_eq!(after.block_level(BlockPos([-1, 3, 0])), 6);
    assert_eq!(after.level(p), 0);
    assert_eq!(after.block_level(other), 14);
    mine(&mut w, &mut inv, other).unwrap().unwrap();
    let dark = Skylight::for_chunks(&w, &targets);
    assert_eq!(dark.block_level(p), 0);
    assert_eq!(dark.level(p), 0);
}
fn prepare(w: &mut World, column: [i32; 2]) {
    use crate::{ChunkPos, regions::needed_regions};
    for r in needed_regions((-1..2).map(|y| crate::ChunkPos([column[0], y, column[1]]))) {
        w.install_region(r, Default::default());
    }
    crate::streaming::prepare_column(w, column).unwrap();
    assert!(w.chunk(ChunkPos([column[0], 1, column[1]])).is_some());
}
#[test]
fn aggregate_dirty_region_limits_and_pending_dependents_reject_everything() {
    use crate::regions::{MAX_DIRTY_REGIONS, RegionIdentity};
    let p = BlockPos([63, 25, 63]);
    for already_dirty in [5, 6, 7] {
        let mut w = World::regional(RegionIdentity {
            seed: 42,
            generator: 3,
            nonce: [3; 16],
        });
        for c in [[3, 3], [4, 3], [3, 4]] {
            prepare(&mut w, c);
        }
        w.set_block(p, Block::Stone).unwrap();
        for d in ATTACHMENTS {
            w.set_block(p.offset(d).unwrap(), Block::torch_on_face(d).unwrap())
                .unwrap();
        }
        w.mark_regions_saved(w.revision());
        for n in 0..already_dirty {
            let c = [40 + n * 4, 40];
            prepare(&mut w, c);
            w.set_block(BlockPos([c[0] * 16 + 8, 25, c[1] * 16 + 8]), Block::Wood)
                .unwrap();
        }
        let mut inv = Inventory::empty();
        let before = w.clone();
        let items = inv.clone();
        let result = mine(&mut w, &mut inv, p);
        if already_dirty == 5 {
            assert_eq!(result.unwrap().unwrap().changes().len(), 6);
            assert_eq!(w.dirty_regions(), MAX_DIRTY_REGIONS);
        } else {
            assert_eq!(
                result,
                Err(InventoryEditError::World(EditError::SaveRequired))
            );
            assert_eq!(w, before);
            assert_eq!(inv, items);
        }
    }
    let mut w = World::regional(RegionIdentity {
        seed: 42,
        generator: 3,
        nonce: [3; 16],
    });
    for c in [[3, 3], [4, 3], [3, 4]] {
        prepare(&mut w, c);
    }
    w.set_block(p, Block::Stone).unwrap();
    w.set_block(p.offset([1, 0, 0]).unwrap(), Block::TorchEast)
        .unwrap();
    w.evict_column([4, 3]);
    let before = w.clone();
    let mut inv = Inventory::empty();
    assert_eq!(
        mine(&mut w, &mut inv, p),
        Err(InventoryEditError::World(EditError::NotLoaded))
    );
    assert_eq!(w, before);
    assert_eq!(count(&inv, Item::Stone), 0);
}
