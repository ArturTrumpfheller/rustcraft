// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded real v9 bucket/edit/fluid/save/reload workload in disposable directories.
use std::time::Instant;
use voxel_core::{
    Block, BlockPos, World,
    fluid::FluidConfig,
    inventory::{Inventory, Item, SLOT_COUNT, Stack, edit_inventory},
    persistence::SaveStore,
    physics::{PhysicsConfig, Player},
    raycast::Ray,
    regions::RegionIdentity,
    streaming,
};
fn load(store: &SaveStore, world: &mut World) {
    for c in [[6, -16], [7, -16]] {
        store.prepare_regional_column(world, c).unwrap();
        streaming::prepare_column(world, c).unwrap();
    }
}
fn main() {
    println!(
        "sample,bucket_us,edit_and_32_fluid_ticks_us,save_us,reload_us,evict_reload_us,processed,pending,edits,drops"
    );
    for trial in 0..24 {
        let dir = std::env::temp_dir().join(format!(
            "rustcraft-slice30-water-{}-{trial}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let id = RegionIdentity {
            seed: 2026,
            generator: 9,
            nonce: [30; 16],
        };
        let (store, mut world, _) = SaveStore::open_with(&dir, || Ok(World::regional(id))).unwrap();
        let _ = store.load_player(&mut world).unwrap();
        load(&store, &mut world);
        let mut slots = [None; SLOT_COUNT];
        slots[0] = Stack::new(Item::EmptyBucket, 1);
        let mut inv = Inventory::from_parts(slots, None, 0).unwrap();
        let source = BlockPos([107, 2, -243]);
        assert_eq!(world.block(source), Block::WaterSource);
        let start = Instant::now();
        edit_inventory(
            &mut world,
            &mut inv,
            Ray {
                origin: [107.5, 4., -242.5],
                direction: [0., -1., 0.],
                reach: 5.,
            },
            true,
            &Player::new([115., 4., -235.]),
            &PhysicsConfig::default(),
        )
        .unwrap()
        .unwrap();
        let bucket = start.elapsed().as_secs_f64() * 1e6;
        assert_eq!(world.block(source), Block::Air);
        assert_eq!(inv.selected_stack().unwrap().item(), Item::WaterBucket);
        let start = Instant::now();
        let obstruction = BlockPos([108, 2, -243]);
        world.set_block(obstruction, Block::Stone).unwrap();
        let pocket = BlockPos([107, -1, -243]);
        world.set_block(pocket, Block::Air).unwrap();
        let mut changes = Vec::new();
        for tick in 0..32 {
            if tick == 16 {
                world.set_block(obstruction, Block::Air).unwrap();
            }
            world.tick_fluids(FluidConfig::default(), &mut changes);
            assert!(world.fluid_updates().processed <= 256);
        }
        let fluid = start.elapsed().as_secs_f64() * 1e6;
        let expected = world.overlay_snapshot();
        let processed = world.fluid_updates().total_processed;
        let pending = world.fluid_updates().pending();
        let edits = world.edit_count();
        assert!(edits < 4096);
        assert!(world.dropped_items().is_empty());
        let start = Instant::now();
        store.save_player_session(&world, &inv, true).unwrap();
        let save = start.elapsed().as_secs_f64() * 1e6;
        drop(store);
        let start = Instant::now();
        let (store, mut restored, _) = SaveStore::open_with(&dir, || panic!("must load")).unwrap();
        let actual_inv = store.load_player(&mut restored).unwrap();
        load(&store, &mut restored);
        let reload = start.elapsed().as_secs_f64() * 1e6;
        assert_eq!(actual_inv.slots(), inv.slots());
        assert_eq!(restored.region_identity(), Some(id));
        for (_, region) in expected.regions() {
            for (&p, &b) in region.iter() {
                assert_eq!(restored.block(p), b);
            }
        }
        let start = Instant::now();
        for c in [[6, -16], [7, -16]] {
            restored.evict_column(c);
        }
        load(&store, &mut restored);
        let evict = start.elapsed().as_secs_f64() * 1e6;
        for (_, region) in expected.regions() {
            for (&p, &b) in region.iter() {
                assert_eq!(restored.block(p), b);
            }
        }
        println!(
            "{trial},{bucket:.3},{fluid:.3},{save:.3},{reload:.3},{evict:.3},{processed},{pending},{edits},{}",
            restored.dropped_items().len()
        );
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
