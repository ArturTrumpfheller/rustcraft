// SPDX-License-Identifier: MIT OR Apache-2.0
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use voxel_core::{
    Block, BlockPos, LocalPos, World,
    inventory::{Inventory, edit_inventory},
    persistence::SaveStore,
    physics::{PhysicsConfig, Player},
    raycast::Ray,
    regions::RegionIdentity,
    streaming,
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustcraft-slice30-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn load(store: &SaveStore, world: &mut World, c: [i32; 2]) {
    store.prepare_regional_column(world, c).unwrap();
    streaming::prepare_column(world, c).unwrap();
}
#[test]
fn generated_removals_and_cross_region_builds_survive_save_evict_regenerate_restart() {
    for version in [3, 6, 8, 9] {
        let dir = Directory::new();
        let id = RegionIdentity {
            seed: 0,
            generator: version,
            nonce: [30; 16],
        };
        let (store, mut world, _) =
            SaveStore::open_with(&dir.0, || Ok(World::regional(id))).unwrap();
        let _ = store.load_player(&mut world).unwrap();
        let root = std::fs::read(store.path()).unwrap();
        let left = [-137, -256];
        let right = [-136, -256];
        load(&store, &mut world, left);
        // Unknown support dependencies fail closed. Read the neighboring overlay
        // without materializing its chunks, then edit while that side is pending.
        let stone = BlockPos([-2177, world.world_height().min + 1, -4095]);
        assert_eq!(world.block(stone), Block::Stone);
        assert_eq!(
            world.set_block(stone, Block::Air),
            Err(voxel_core::interaction::EditError::NotLoaded)
        );
        store.prepare_regional_column(&mut world, right).unwrap();
        assert!(!world.ready(BlockPos([-2176, stone.0[1], -4095])));
        world.set_block(stone, Block::Air).unwrap();
        let mut inv = Inventory::empty();
        store.save_player_session(&world, &inv, true).unwrap();
        load(&store, &mut world, right);
        let mut removals = vec![stone];
        if version == 9 {
            let mut tree = None;
            let mut plant = None;
            for (cp, ch) in world.chunks() {
                for i in 0..4096 {
                    let lp = LocalPos::new([i % 16, i / 256, i / 16 % 16]).unwrap();
                    let p = cp.block_position(lp).unwrap();
                    if ch.get(lp).is_plant() {
                        plant.get_or_insert(p);
                    }
                    if matches!(
                        ch.get(lp),
                        Block::Wood | Block::BirchWood | Block::SpruceWood
                    ) {
                        tree.get_or_insert(p);
                    }
                }
            }
            let tree = tree.expect("fixture contains a generated tree");
            world.set_block(tree, Block::Air).unwrap();
            removals.push(tree);
            let p = plant.expect("fixture contains a generated plant");
            let ray = Ray {
                origin: [
                    f64::from(p.0[0]) + 0.5,
                    f64::from(p.0[1]) + 1.5,
                    f64::from(p.0[2]) + 0.5,
                ],
                direction: [0., -1., 0.],
                reach: 2.,
            };
            let batch = edit_inventory(
                &mut world,
                &mut inv,
                ray,
                false,
                &Player::new([0., 40., 0.]),
                &PhysicsConfig::default(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(batch.changes()[0].position, p);
            removals.push(p);
        }
        let build = BlockPos([-2176, world.world_height().max - 2, -4095]);
        world.set_block(build, Block::BirchWood).unwrap();
        let snapshot = world.save_snapshot();
        assert_eq!(snapshot.chunk_count(), 0);
        store.save_player_session(&snapshot, &inv, true).unwrap();
        for c in [left, right] {
            world.evict_column(c);
        }
        for c in [right, left] {
            load(&store, &mut world, c);
        }
        for p in &removals {
            assert_eq!(world.block(*p), Block::Air);
        }
        assert_eq!(world.block(build), Block::BirchWood);
        drop(store);
        let (store, mut restored, _) =
            SaveStore::open_with(&dir.0, || panic!("must not generate a replacement world"))
                .unwrap();
        let restored_inv = store.load_player(&mut restored).unwrap();
        assert_eq!(restored_inv.slots(), inv.slots());
        assert_eq!(restored_inv.cursor(), inv.cursor());
        assert_eq!(restored.region_identity(), Some(id));
        assert_eq!(std::fs::read(store.path()).unwrap(), root);
        for c in [right, left] {
            load(&store, &mut restored, c);
        }
        for p in removals {
            assert_eq!(restored.block(p), Block::Air);
            assert_eq!(restored.overlay_snapshot().get(&p), Some(&Block::Air));
        }
        assert_eq!(restored.block(build), Block::BirchWood);
    }
}
