use super::*;
use crate::{
    ChunkPos,
    regions::{EditSnapshot, MAX_DIRTY_REGIONS},
    streaming,
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rustcraft-regional-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn identity() -> RegionIdentity {
    RegionIdentity {
        seed: 42,
        generator: 3,
        nonce: [7; 16],
    }
}
fn load_column(store: &SaveStore, w: &mut World, c: [i32; 2]) {
    let id = w.region_identity().unwrap();
    let targets = (-1..2).map(|y| ChunkPos([c[0], y, c[1]]));
    let mut edits = w.overlay_snapshot();
    for p in crate::regions::needed_regions(targets) {
        if !edits.contains_region(p) {
            let m = store.read_region(id, p).unwrap();
            w.install_region(p, Arc::clone(&m));
            edits.insert_region(p, m);
        }
    }
    let g = crate::worldgen::Generator::new(crate::worldgen::GenerationConfig {
        seed: id.seed,
        version: 3,
    })
    .unwrap();
    for (p, c, _) in streaming::build_column(&g, edits, c, w.revision()).chunks {
        w.insert_chunk(p, c);
    }
}
#[test]
fn v4_manifest_and_region_golden_bytes_and_strict_validation() {
    let id = identity();
    let b = manifest(id);
    assert_eq!(b.len(), 64);
    assert_eq!(u32_at(&b, 20), 0xccfae85f, "frozen v4 manifest bytes");
    assert_eq!(&b[..12], b"RUSTWRLD\x04\x00\x00\x00");
    assert_eq!(decode(&b).unwrap().region_identity(), Some(id));
    assert_eq!(encode(&decode(&b).unwrap()).unwrap(), b);
    for offset in [12, 16, 32, 36, 40, 60] {
        let mut bad = b.clone();
        bad[offset] ^= 1;
        let bad = finish(bad);
        assert!(decode(&bad).is_err(), "offset {offset}");
    }
    let mut bad = b.clone();
    bad[44..60].fill(0);
    assert!(decode(&finish(bad)).is_err());
    let p = RegionPos([-1, 2]);
    let edits = Arc::new(BTreeMap::from([
        (BlockPos([-64, -15, 128]), Block::Air),
        (BlockPos([-1, 25, 191]), Block::Wood),
    ]));
    let b = encode_region(id, p, &edits).unwrap();
    assert_eq!(b.len(), 92);
    assert_eq!(decode_region(&b, id, p).unwrap(), edits);
    let crc = checksum(&b);
    assert_eq!(crc, 0x6e3278b0);
    for i in 0..b.len() {
        let mut bad = b.clone();
        bad[i] ^= 0x80;
        assert!(decode_region(&bad, id, p).is_err(), "byte {i}");
    }
    for len in 0..b.len() {
        assert!(decode_region(&b[..len], id, p).is_err());
    }
    let mut bad = b.clone();
    bad.extend([0]);
    assert!(decode_region(&bad, id, p).is_err());
    for offset in [8, 12, 16, 24, 40, 48, 52, 56, 60] {
        let mut bad = b.clone();
        bad[offset] ^= 1;
        assert!(
            decode_region(&finish(bad), id, p).is_err(),
            "header/record {offset}"
        );
    }
    let mut duplicate = b.clone();
    duplicate[78..92].copy_from_slice(&b[64..78]);
    assert_eq!(
        decode_region(&finish(duplicate), id, p),
        Err(FormatError::UnsortedOrDuplicate)
    );
}
#[test]
fn far_air_and_wood_survive_save_eviction_clean_overlay_release_and_restart() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    store.prepare_regional_spawn(&mut w).unwrap();
    streaming::prepare_spawn(&mut w).unwrap();
    store.save(&w).unwrap();
    let c = [1000, -1000];
    load_column(&store, &mut w, c);
    let a = BlockPos([16000, -15, -16000]);
    let b = BlockPos([16001, 25, -16000]);
    w.set_block(a, Block::Air).unwrap();
    w.set_block(b, Block::Wood).unwrap();
    let snapshot = w.save_snapshot();
    assert_eq!(snapshot.chunks().count(), 0);
    w.evict_column(c);
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 1, "dirty region is pinned");
    store.save(&snapshot).unwrap();
    w.mark_regions_saved(snapshot.revision());
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 0);
    assert_eq!(fs::metadata(store.path()).unwrap().len(), 64);
    assert_eq!(
        fs::metadata(store.region_path(RegionPos::of(a)))
            .unwrap()
            .len(),
        92
    );
    drop(store);
    let (store, mut w, _) = SaveStore::open_with(&d.0, || panic!("existing identity")).unwrap();
    assert_eq!(w.cached_regions(), 0);
    load_column(&store, &mut w, c);
    assert_eq!(w.block(a), Block::Air);
    assert_eq!(w.block(b), Block::Wood);
}
#[test]
fn failed_region_replacement_keeps_primary_and_dirty_snapshot_retry_is_safe() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    load_column(&store, &mut w, [40, -40]);
    let p = BlockPos([640, 25, -640]);
    w.set_block(p, Block::Wood).unwrap();
    store.save(&w).unwrap();
    w.mark_regions_saved(w.revision());
    let path = store.region_path(RegionPos::of(p));
    let old = fs::read(&path).unwrap();
    w.set_block(p, Block::Air).unwrap();
    let snap = w.save_snapshot();
    for stage in [
        SaveStage::PartialTemp,
        SaveStage::TempSynced,
        SaveStage::BackupCommitted,
        SaveStage::BeforePrimaryRename,
    ] {
        assert!(
            store
                .save_with(&snap, |s| if s == stage {
                    Err(io::Error::other("injected region failure"))
                } else {
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), old);
        assert_eq!(w.dirty_regions(), 1);
    }
    store.save(&snap).unwrap();
    assert_eq!(
        store.read_region(identity(), RegionPos::of(p)).unwrap()[&p],
        Block::Air
    );
    let backup = fs::read(path.with_extension("rcw.bak")).unwrap();
    assert_eq!(backup, old);
    store.save(&snap).unwrap();
    assert_eq!(
        fs::read(path.with_extension("rcw.bak")).unwrap(),
        backup,
        "idempotent retry must not rotate away good backup"
    );
}
#[test]
fn corrupt_missing_unknown_and_foreign_regions_never_silently_regenerate() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    load_column(&store, &mut w, [8, 8]);
    let p = BlockPos([128, 25, 128]);
    w.set_block(p, Block::Wood).unwrap();
    store.save(&w).unwrap();
    let path = store.region_path(RegionPos::of(p));
    let old = fs::read(&path).unwrap();
    fs::write(&path, b"corrupt").unwrap();
    assert!(store.read_region(identity(), RegionPos::of(p)).is_err());
    assert!(store.save(&w).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"corrupt");
    fs::write(path.with_extension("rcw.bak"), &old).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(store.read_region(identity(), RegionPos::of(p)).is_err());
    assert!(store.save(&w).is_err());
    fs::write(&path, &old).unwrap();
    let mut foreign = identity();
    foreign.nonce = [8; 16];
    assert!(store.read_region(foreign, RegionPos::of(p)).is_err());
    let foreign_world = World::regional(foreign);
    let manifest_before = fs::read(store.path()).unwrap();
    assert!(store.save(&foreign_world).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), manifest_before);
}
#[test]
fn dirty_region_limit_pins_changes_and_cow_is_regional_not_global() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    for n in 0..MAX_DIRTY_REGIONS + 1 {
        load_column(&store, &mut w, [n as i32 * 8, 0]);
    }
    for n in 0..MAX_DIRTY_REGIONS {
        w.set_block(BlockPos([n as i32 * 128, 25, 0]), Block::Wood)
            .unwrap();
    }
    assert_eq!(w.dirty_regions(), 8);
    let old = w.overlay_snapshot();
    let snap = w.save_snapshot();
    w.set_block(BlockPos([0, 25, 1]), Block::Leaves).unwrap();
    assert!(old.get(&BlockPos([0, 25, 1])).is_none());
    let newer = w.overlay_snapshot();
    assert!(!Arc::ptr_eq(
        &old.regions[&RegionPos([0, 0])],
        &newer.regions[&RegionPos([0, 0])]
    ));
    assert!(Arc::ptr_eq(
        &old.regions[&RegionPos([2, 0])],
        &newer.regions[&RegionPos([2, 0])]
    ));
    assert_eq!(
        w.set_block(BlockPos([8 * 128, 25, 0]), Block::Wood),
        Err(crate::interaction::EditError::SaveRequired)
    );
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 8);
    store.save(&snap).unwrap();
    w.mark_regions_saved(snap.revision());
    assert_eq!(w.dirty_regions(), 1, "newer edit must stay dirty");
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 1);
    let _: EditSnapshot = old;
}
#[test]
fn multi_region_partial_commit_keeps_failed_and_newer_changes_for_retry() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    for c in [[0, 0], [100, 100]] {
        load_column(&store, &mut w, c);
        w.set_block(BlockPos([c[0] * 16, 25, c[1] * 16]), Block::Wood)
            .unwrap();
    }
    store.save(&World::regional(identity())).unwrap();
    let snap = w.save_snapshot();
    let mut commits = 0;
    assert!(
        store
            .save_with(&snap, |s| {
                if s == SaveStage::PrimaryCommitted {
                    commits += 1;
                }
                if commits == 1 && s == SaveStage::PartialTemp {
                    Err(io::Error::other("second region failed"))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(w.dirty_regions(), 2);
    w.evict_column([0, 0]);
    w.evict_column([100, 100]);
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 2);
    store.save(&snap).unwrap();
    w.mark_regions_saved(snap.revision());
    assert_eq!(w.dirty_regions(), 0);
    for p in [BlockPos([0, 25, 0]), BlockPos([1600, 25, 1600])] {
        assert_eq!(
            store.read_region(identity(), RegionPos::of(p)).unwrap()[&p],
            Block::Wood
        );
    }
}
#[test]
fn orphan_regions_and_missing_manifest_backup_stop_before_creating_an_identity() {
    let d = Dir::new();
    fs::create_dir(d.0.join("regions")).unwrap();
    assert!(SaveStore::open_with(&d.0, || panic!("must refuse orphan files")).is_err());
}

#[test]
fn many_saved_regions_do_not_accumulate_resident_metadata_or_rewrite_manifest() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    store.save(&w).unwrap();
    let manifest_time = fs::metadata(store.path()).unwrap().modified().unwrap();
    for n in 1..=24 {
        let c = [n * 16, -n * 16];
        load_column(&store, &mut w, c);
        let p = BlockPos([c[0] * 16, -15, c[1] * 16]);
        w.set_block(p, Block::Air).unwrap();
        store.save(&w.save_snapshot()).unwrap();
        w.mark_regions_saved(w.revision());
        w.evict_column(c);
        w.retain_regions(&Default::default());
        assert_eq!(w.cached_regions(), 0);
        assert_eq!(w.edit_count(), 0);
        assert_eq!(
            store.read_region(identity(), RegionPos::of(p)).unwrap()[&p],
            Block::Air
        );
    }
    assert_eq!(
        fs::metadata(store.path()).unwrap().modified().unwrap(),
        manifest_time
    );
    assert_eq!(fs::metadata(store.path()).unwrap().len(), 64);
}

#[test]
fn post_rename_failure_retry_preserves_previous_backup_and_reestablishes_sync() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    load_column(&store, &mut w, [100, -100]);
    let p = BlockPos([1600, 25, -1600]);
    w.set_block(p, Block::Wood).unwrap();
    store.save(&w).unwrap();
    w.mark_regions_saved(w.revision());
    let path = store.region_path(RegionPos::of(p));
    let old = fs::read(&path).unwrap();
    w.set_block(p, Block::Air).unwrap();
    assert!(
        store
            .save_with(
                &w.save_snapshot(),
                |s| if s == SaveStage::PrimaryCommitted {
                    Err(io::Error::other("ambiguous post-rename failure"))
                } else {
                    Ok(())
                }
            )
            .is_err()
    );
    assert_eq!(w.dirty_regions(), 1);
    assert_eq!(
        store.read_region(identity(), RegionPos::of(p)).unwrap()[&p],
        Block::Air
    );
    store.save(&w.save_snapshot()).unwrap();
    assert_eq!(fs::read(path.with_extension("rcw.bak")).unwrap(), old);
}

#[test]
fn far_start_preparation_reads_saved_region_before_any_physics_or_mesh() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    load_column(&store, &mut w, [1000, -1000]);
    let p = BlockPos([16008, 25, -15992]);
    w.set_block(p, Block::Wood).unwrap();
    store.save(&w).unwrap();
    drop(store);
    let (store, mut w, _) = SaveStore::open_with(&d.0, || panic!("must load")).unwrap();
    store
        .prepare_regional_column(&mut w, [1000, -1000])
        .unwrap();
    streaming::prepare_column(&mut w, [1000, -1000]).unwrap();
    assert_eq!(w.block(p), Block::Wood);
    assert_eq!(w.cached_regions(), 1);
    assert!(w.chunk(ChunkPos([0, 0, 0])).is_none());
}

#[test]
fn torches_survive_region_seams_eviction_reload_and_removal() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    let torches = [BlockPos([-65, 25, -1]), BlockPos([-64, 25, -1])];
    for p in torches {
        load_column(&store, &mut w, [p.0[0].div_euclid(16), -1]);
        w.set_block(p.offset([0, -1, 0]).unwrap(), Block::Stone)
            .unwrap();
        w.set_block(p, Block::Torch).unwrap();
    }
    let snap = w.save_snapshot();
    store.save(&snap).unwrap();
    w.mark_regions_saved(snap.revision());
    for c in [[-5, -1], [-4, -1]] {
        w.evict_column(c);
    }
    w.retain_regions(&Default::default());
    assert_eq!(w.cached_regions(), 0);
    for p in torches {
        load_column(&store, &mut w, [p.0[0].div_euclid(16), -1]);
        assert_eq!(w.block(p), Block::Torch);
    }
    drop(store);
    let (store, mut w, _) = SaveStore::open_with(&d.0, || panic!("must reopen")).unwrap();
    for p in torches {
        load_column(&store, &mut w, [p.0[0].div_euclid(16), -1]);
        assert_eq!(w.block(p), Block::Torch);
        w.set_block(p, Block::Air).unwrap();
    }
    store.save(&w).unwrap();
    drop(store);
    let (store, mut w, _) = SaveStore::open_with(&d.0, || panic!("must reopen")).unwrap();
    for p in torches {
        load_column(&store, &mut w, [p.0[0].div_euclid(16), -1]);
        assert_eq!(w.block(p), Block::Air);
    }
}

#[test]
fn wall_mounts_survive_signed_region_seams_eviction_reload_and_protect_pending_support() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    let cases = [
        (BlockPos([64, 25, 2]), Block::TorchEast),
        (BlockPos([-65, 25, 2]), Block::TorchWest),
        (BlockPos([2, 25, 64]), Block::TorchSouth),
        (BlockPos([2, 25, -65]), Block::TorchNorth),
    ];
    for (p, b) in cases {
        let s = p.offset(b.torch_support().unwrap()).unwrap();
        for pos in [p, s] {
            load_column(
                &store,
                &mut w,
                [pos.0[0].div_euclid(16), pos.0[2].div_euclid(16)],
            );
        }
        w.set_block(s, Block::Stone).unwrap();
        w.set_block(p, b).unwrap();
        w.evict_column([p.0[0].div_euclid(16), p.0[2].div_euclid(16)]);
        assert_eq!(
            w.set_block(s, Block::Air),
            Err(crate::interaction::EditError::SupportsTorch)
        );
    }
    store.save(&w).unwrap();
    drop(store);
    let (store, mut w, _) =
        SaveStore::open_with(&d.0, || panic!("must load existing world")).unwrap();
    for (p, b) in cases {
        let c = [p.0[0].div_euclid(16), p.0[2].div_euclid(16)];
        load_column(&store, &mut w, c);
        assert_eq!(w.block(p), b);
        w.evict_column(c);
        w.retain_regions(&Default::default());
        load_column(&store, &mut w, c);
        assert_eq!(w.block(p), b);
    }
}

#[test]
fn unknown_pending_support_dependencies_defer_edits_until_overlay_is_known() {
    let d = Dir::new();
    let (store, mut w, _) = SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
    store.prepare_regional_column(&mut w, [0, 0]).unwrap();
    streaming::prepare_column(&mut w, [0, 0]).unwrap();
    let p = BlockPos([0, -15, 0]);
    let revision = w.revision();
    assert_eq!(
        w.set_block(p, Block::Air),
        Err(crate::interaction::EditError::NotLoaded)
    );
    assert_eq!(w.revision(), revision);
    for c in [[-1, 0], [0, -1]] {
        store.prepare_regional_column(&mut w, c).unwrap();
    }
    assert!(w.set_block(p, Block::Air).unwrap().is_some());
}

#[test]
fn support_batch_and_inventory_survive_signed_region_eviction_and_interrupted_session_saves() {
    use crate::{
        inventory::{Inventory, Item},
        support_tests::{count, mine},
    };
    let directions = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
    for p in [BlockPos([63, 25, 63]), BlockPos([-64, 25, -64])] {
        for fault in [
            SaveStage::SessionPrepared,
            SaveStage::WorldSaved,
            SaveStage::PlayerSaved,
        ] {
            let d = Dir::new();
            let (store, mut w, _) =
                SaveStore::open_with(&d.0, || Ok(World::regional(identity()))).unwrap();
            store.load_player(&mut w).unwrap();
            let mut inv = Inventory::empty();
            let center = [p.0[0].div_euclid(16), p.0[2].div_euclid(16)];
            for x in center[0] - 1..=center[0] + 1 {
                for z in center[1] - 1..=center[1] + 1 {
                    load_column(&store, &mut w, [x, z]);
                }
            }
            w.set_block(p, Block::Stone).unwrap();
            for delta in directions {
                w.set_block(
                    p.offset(delta).unwrap(),
                    Block::torch_on_face(delta).unwrap(),
                )
                .unwrap();
            }
            store.save_player_session(&w, &inv, true).unwrap();
            w.mark_regions_saved(w.revision());
            let batch = mine(&mut w, &mut inv, p).unwrap().unwrap();
            assert_eq!(batch.changes().len(), 6);
            for x in center[0] - 1..=center[0] + 1 {
                for z in center[1] - 1..=center[1] + 1 {
                    w.evict_column([x, z]);
                }
            }
            w.retain_regions(&Default::default());
            assert_eq!(w.dirty_regions(), 3);
            assert!(
                store
                    .save_player_with(&w, &inv, true, |stage| if stage == fault {
                        Err(io::Error::other("injected support-batch save interruption"))
                    } else {
                        Ok(())
                    })
                    .is_err()
            );
            drop(store);
            let (store, mut got, _) =
                SaveStore::open_with(&d.0, || panic!("must replay existing session")).unwrap();
            let items = store.load_player(&mut got).unwrap();
            assert_eq!(count(&items, Item::Stone), 1);
            assert_eq!(count(&items, Item::Torch), 5);
            for x in center[0] - 1..=center[0] + 1 {
                for z in center[1] - 1..=center[1] + 1 {
                    load_column(&store, &mut got, [x, z]);
                }
            }
            for change in batch.changes() {
                assert_eq!(got.block(change.position), Block::Air);
            }
            assert!(!d.0.join("session.pending").exists());
        }
    }
}

#[test]
fn v5_manifest_generator_identity_and_old_v4_bytes_are_strictly_separate() {
    let old = identity();
    let new = RegionIdentity {
        generator: 4,
        ..old
    };
    let b4 = manifest(old);
    let b5 = manifest(new);
    assert_eq!(
        u32_at(&b5, 20),
        0x1da306c7,
        "frozen Slice24 v5/gen4 manifest"
    );
    assert_eq!(u32_at(&b5, 8), 5);
    assert_eq!(u32_at(&b5, 32), 4);
    assert_eq!(decode(&b4).unwrap().identity().unwrap().generator, 3);
    assert_eq!(decode(&b5).unwrap().region_identity(), Some(new));
    assert_eq!(encode(&decode(&b5).unwrap()).unwrap(), b5);
    // Repaired checksums must never let v4 mean gen4 or v5 mean gen3.
    for (base, generator) in [(&b4, 4u32), (&b5, 3)] {
        let mut bad = base.clone();
        bad[32..36].copy_from_slice(&generator.to_le_bytes());
        assert_eq!(decode(&finish(bad)), Err(FormatError::Identity));
    }
    let mut future = b5.clone();
    future[32..36].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        decode(&finish(future.clone())),
        Err(FormatError::Generator(99))
    );
    let dir = Dir::new();
    fs::write(dir.0.join("world.rcw"), finish(future)).unwrap();
    let before = fs::read(dir.0.join("world.rcw")).unwrap();
    assert!(
        SaveStore::open_with(&dir.0, || panic!(
            "unsupported generator must not invoke creation"
        ))
        .is_err()
    );
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), before);
    for offset in [12, 16, 36, 40, 44, 60] {
        let mut bad = b5.clone();
        bad[offset] ^= 1;
        if offset == 44 {
            bad[44..60].fill(0);
        }
        assert!(decode(&finish(bad)).is_err());
    }
    let p = RegionPos([-1, 0]);
    let edits = Arc::new(BTreeMap::from([(BlockPos([-1, -15, 0]), Block::Air)]));
    let region = encode_region(new, p, &edits).unwrap();
    assert_eq!(u32_at(&region, 48), 4);
    assert_eq!(decode_region(&region, new, p).unwrap(), edits);
    assert_eq!(decode_region(&region, old, p), Err(FormatError::Identity));
    assert!(decode_region(&encode_region(old, p, &edits).unwrap(), new, p).is_err());
}

#[test]
fn v5_edits_water_player_journal_backup_and_eviction_survive_all_commit_interruptions() {
    for stage in [
        SaveStage::SessionPrepared,
        SaveStage::WorldSaved,
        SaveStage::PlayerSaved,
    ] {
        let dir = Dir::new();
        let (store, mut world, _) =
            SaveStore::open_with(&dir.0, || Ok(streaming::new_large_world(42))).unwrap();
        let mut inventory = store.load_player(&mut world).unwrap();
        let id = world.region_identity().unwrap();
        assert_eq!(id.generator, 5);
        let root = fs::read(store.path()).unwrap();
        assert_eq!(u32_at(&root, 8), 5);
        // Signed region/chunk boundaries, explicit Air and water retain their IDs.
        for c in [[-5, -1], [-4, -1], [-5, 0], [-4, 0]] {
            store.prepare_regional_column(&mut world, c).unwrap();
            streaming::prepare_column(&mut world, c).unwrap();
        }
        let air = BlockPos([-65, -15, -1]);
        let water = BlockPos([-64, 25, -1]);
        let torch = BlockPos([-63, 25, -1]);
        let untouched = BlockPos([-62, -15, -1]);
        world.set_block(air, Block::Air).unwrap();
        world.set_block(water, Block::WaterSource).unwrap();
        world
            .set_block(torch.offset([0, -1, 0]).unwrap(), Block::Stone)
            .unwrap();
        world.set_block(torch, Block::Torch).unwrap();
        store.save_player_session(&world, &inventory, true).unwrap();
        world.mark_regions_saved(world.revision());
        world.set_block(water, Block::Air).unwrap();
        inventory.select(8);
        world.time.advance(12345);
        let time = world.time;
        assert!(
            store
                .save_player_with(&world, &inventory, true, |at| if at == stage {
                    Err(io::Error::other("v5 interrupted paired save"))
                } else {
                    Ok(())
                })
                .is_err()
        );
        for c in [[-5, -1], [-4, -1], [-5, 0], [-4, 0]] {
            world.evict_column(c);
        }
        assert_eq!(world.dirty_regions(), 1);
        drop(store);
        let (store, mut got, _) =
            SaveStore::open_with(&dir.0, || panic!("must retain v5 identity")).unwrap();
        let inv = store.load_player(&mut got).unwrap();
        assert_eq!(got.region_identity(), Some(id));
        assert_eq!(got.time, time);
        assert_eq!(inv.selected(), 8);
        for c in [[-5, -1], [-4, -1], [-5, 0], [-4, 0]] {
            store.prepare_regional_column(&mut got, c).unwrap();
            streaming::prepare_column(&mut got, c).unwrap();
        }
        assert_eq!(got.block(air), Block::Air);
        assert_eq!(got.block(water), Block::Air);
        assert_eq!(got.block(torch), Block::Torch);
        assert_eq!(got.block(untouched), Block::Stone);
        assert_eq!(
            fs::read(store.path()).unwrap(),
            root,
            "manifest must remain immutable"
        );
        assert!(
            store
                .region_path(RegionPos::of(water))
                .with_extension("rcw.bak")
                .exists()
        );
        assert!(!dir.0.join("session.pending").exists());
        // A gen3 world with the same seed/nonce still cannot overwrite gen4.
        let foreign = World::regional(RegionIdentity { generator: 3, ..id });
        assert!(store.save(&foreign).is_err());
    }
}

#[test]
fn biome_blocks_and_items_survive_journal_replay_in_all_regional_generators() {
    use crate::inventory::Item;
    for generator in [3, 4, 5] {
        for stage in [
            SaveStage::SessionPrepared,
            SaveStage::WorldSaved,
            SaveStage::PlayerSaved,
        ] {
            let dir = Dir::new();
            let id = RegionIdentity {
                generator,
                ..identity()
            };
            let (store, mut world, _) =
                SaveStore::open_with(&dir.0, || Ok(World::regional(id))).unwrap();
            let mut inv = store.load_player(&mut world).unwrap();
            store.prepare_regional_column(&mut world, [-1, -1]).unwrap();
            streaming::prepare_column(&mut world, [-1, -1]).unwrap();
            let root = fs::read(store.path()).unwrap();
            for (p, b) in [
                (BlockPos([-2, 25, -2]), Block::Sand),
                (BlockPos([-1, 25, -2]), Block::SnowyGround),
                (BlockPos([-2, -15, -2]), Block::Air),
            ] {
                world.set_block(p, b).unwrap();
            }
            inv.insert(Item::Sand, 17);
            inv.insert(Item::SnowyGround, 23);
            assert!(
                store
                    .save_player_with(&world, &inv, true, |at| if at == stage {
                        Err(io::Error::other("biome interrupted save"))
                    } else {
                        Ok(())
                    })
                    .is_err()
            );
            drop(store);
            let (store, mut restored, _) =
                SaveStore::open_with(&dir.0, || panic!("existing identity")).unwrap();
            let got = store.load_player(&mut restored).unwrap();
            assert_eq!(got.slots(), inv.slots());
            assert_eq!(restored.region_identity(), Some(id));
            store
                .prepare_regional_column(&mut restored, [-1, -1])
                .unwrap();
            streaming::prepare_column(&mut restored, [-1, -1]).unwrap();
            assert_eq!(restored.block(BlockPos([-2, 25, -2])), Block::Sand);
            assert_eq!(restored.block(BlockPos([-1, 25, -2])), Block::SnowyGround);
            assert_eq!(restored.block(BlockPos([-2, -15, -2])), Block::Air);
            assert_eq!(fs::read(store.path()).unwrap(), root);
            assert!(!dir.0.join("session.pending").exists());
        }
    }
}
#[test]
fn additive_biome_block_ids_do_not_reinterpret_historical_materials() {
    assert_eq!(block_id(Block::Sand), 20);
    assert_eq!(block_id(Block::SnowyGround), 21);
    for b in [
        Block::Air,
        Block::Grass,
        Block::Dirt,
        Block::Stone,
        Block::Wood,
        Block::Leaves,
        Block::Torch,
        Block::TorchEast,
        Block::TorchWest,
        Block::TorchSouth,
        Block::TorchNorth,
        Block::WaterSource,
        Block::Water1,
        Block::Water2,
        Block::Water3,
        Block::Water4,
        Block::Water5,
        Block::Water6,
        Block::Water7,
        Block::WaterFalling,
        Block::Sand,
        Block::SnowyGround,
    ] {
        assert_eq!(stored_block(block_id(b), 3).unwrap(), b);
    }
    for version in [3, 4, 5] {
        let id = RegionIdentity {
            generator: version,
            ..identity()
        };
        let b = manifest(id);
        assert_eq!(decode(&b).unwrap().region_identity(), Some(id));
        assert_eq!(u32_at(&b, 8), if version == 3 { 4 } else { 5 });
    }
}
