// SPDX-License-Identifier: MIT OR Apache-2.0
//! Real inventory removal, regeneration and paired save/reload of generated plants.
use std::{collections::BTreeSet, time::Instant};
use voxel_core::{
    Block, BlockPos, World,
    inventory::{Inventory, Item, edit_inventory},
    persistence::SaveStore,
    physics::{PhysicsConfig, Player},
    raycast::Ray,
    streaming,
};
fn p(v: &mut [f64], percent: usize) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() - 1) * percent / 100]
}
fn main() {
    let mut breaks = Vec::new();
    let mut saves = Vec::new();
    let mut loads = Vec::new();
    let mut regenerations = Vec::new();
    for trial in 0..24 {
        let dir = std::env::temp_dir().join(format!(
            "rustcraft-vegetation-bench-{}-{trial}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let id = voxel_core::regions::RegionIdentity {
            seed: 0,
            generator: 9,
            nonce: [29; 16],
        };
        let (store, mut world, _) = SaveStore::open_with(&dir, || Ok(World::regional(id))).unwrap();
        let _ = store.load_player(&mut world).unwrap();
        let c = [-152, -256];
        store.prepare_regional_column(&mut world, c).unwrap();
        streaming::prepare_column(&mut world, c).unwrap();
        let mut vegetation = Vec::new();
        for (cp, ch) in world.chunks() {
            for i in 0..4096 {
                let lp = voxel_core::LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                if ch.get(lp).is_plant() {
                    vegetation.push(cp.block_position(lp).unwrap());
                }
            }
        }
        let mut inv = Inventory::empty();
        let player = Player::new([-2400., 40., -4050.]);
        let cfg = PhysicsConfig::default();
        let mut removed = BTreeSet::new();
        let start = Instant::now();
        for &bp in &vegetation {
            let ray = Ray {
                origin: [
                    f64::from(bp.0[0]) + 0.5,
                    f64::from(bp.0[1]) + 1.5,
                    f64::from(bp.0[2]) + 0.5,
                ],
                direction: [0., -1., 0.],
                reach: 2.,
            };
            if edit_inventory(&mut world, &mut inv, ray, false, &player, &cfg)
                .unwrap()
                .is_some()
            {
                removed.insert(bp);
            }
        }
        breaks.push(start.elapsed().as_secs_f64() * 1e6 / removed.len() as f64);
        assert!(!removed.is_empty());
        inv.insert(Item::BirchWood, 3);
        inv.insert(Item::SpruceLeaves, 2);
        let placed = BlockPos([-2431, 28, -4095]);
        world
            .set_block(placed.offset([0, -1, 0]).unwrap(), Block::Dirt)
            .unwrap();
        world.set_block(placed, Block::YellowFlower).unwrap();
        let start = Instant::now();
        store.save_player_session(&world, &inv, true).unwrap();
        saves.push(start.elapsed().as_secs_f64() * 1e6);
        drop(store);
        let start = Instant::now();
        let (store, mut world, _) = SaveStore::open_with(&dir, || panic!("must load")).unwrap();
        let loaded = store.load_player(&mut world).unwrap();
        store.prepare_regional_column(&mut world, c).unwrap();
        streaming::prepare_column(&mut world, c).unwrap();
        loads.push(start.elapsed().as_secs_f64() * 1e6);
        assert_eq!(loaded.slots(), inv.slots());
        for bp in &removed {
            assert_eq!(world.block(*bp), Block::Air);
        }
        assert_eq!(world.block(placed), Block::YellowFlower);
        world.evict_column(c);
        let start = Instant::now();
        streaming::prepare_column(&mut world, c).unwrap();
        regenerations.push(start.elapsed().as_secs_f64() * 1e6);
        for bp in removed {
            assert_eq!(world.block(bp), Block::Air);
        }
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    println!("operation,samples,p50_us,p95_us,p99_us");
    for (name, mut values) in [
        ("plant-inventory-break", breaks),
        ("paired-save", saves),
        ("paired-reload-and-column", loads),
        ("eviction-regeneration", regenerations),
    ] {
        println!(
            "{name},{},{:.2},{:.2},{:.2}",
            values.len(),
            p(&mut values, 50),
            p(&mut values, 95),
            p(&mut values, 99)
        );
    }
}
