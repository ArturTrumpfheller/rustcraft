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
