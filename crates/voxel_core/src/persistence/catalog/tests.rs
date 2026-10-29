use super::*;
use crate::{Block, BlockPos, inventory::Click};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("rustcraft-catalog-{}", fresh_id()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn catalog(&self) -> WorldCatalog {
        WorldCatalog::new(self.0.clone()).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bytes(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut b: Vec<_> = tree(path, true)
        .unwrap()
        .into_iter()
        .filter(|p| p.is_file())
        .map(|p| {
            (
                p.strip_prefix(path).unwrap().to_owned(),
                fs::read(&p).unwrap(),
            )
        })
        .collect();
    b.sort();
    b
}
#[test]
fn worlds_have_unique_identity_reproducible_terrain_and_isolated_player_regions() {
    let t = Temp::new();
    let c = t.catalog();
    let a = c.create("Welt / Ä & ß", 42).unwrap();
    let b = c.create("Welt / Ä & ß", 42).unwrap();
    let d = c.create("Andere", 43).unwrap();
    assert_ne!(a, b);
    let listing = c.list().unwrap();
    let entry = listing
        .iter()
        .find(|entry| entry.key == WorldKey::Managed(a.clone()))
        .unwrap();
    assert_eq!(entry.generator, Some(5));
    assert_eq!(entry.kind, "Regional v5");
    let (sa, mut wa, _) = c.open(&a).unwrap();
    let (sb, mut wb, _) = c.open(&b).unwrap();
    let (sd, wd, _) = c.open(&d).unwrap();
    assert_ne!(wa.region_identity(), wb.region_identity());
    assert_ne!(wa.identity(), wd.identity());
    assert_eq!(wa.identity(), wb.identity());
    assert_eq!(wa.identity().unwrap().generator, 5);
    assert_eq!(wa.region_identity().unwrap().format_version(), 5);
    crate::streaming::prepare_column(&mut wa, [0, 0]).unwrap();
    crate::streaming::prepare_column(&mut wb, [0, 0]).unwrap();
    for x in -4..4 {
        for z in -4..4 {
            for y in -16..32 {
                assert_eq!(wa.block(BlockPos([x, y, z])), wb.block(BlockPos([x, y, z])));
            }
        }
    }
    let mut ia = sa.load_player(&mut wa).unwrap();
    let ib = sb.load_player(&mut wb).unwrap();
    sa.prepare_regional_spawn(&mut wa).unwrap();
    wa.set_block(BlockPos([1, 31, 1]), Block::Wood).unwrap();
    ia.select(8);
    ia.click(0, Click::Right);
    sa.save_player_session(&wa, &ia, true).unwrap();
    assert_ne!(ia.selected(), ib.selected());
    assert_ne!(ia.cursor(), ib.cursor());
    assert!(c.open(&a).is_err());
    assert!(c.archive(&a).is_err());
    assert!(c.rename(&a, "locked").is_err());
    drop((sa, sb, sd));
    let (sa, mut wa, _) = c.open(&a).unwrap();
    sa.prepare_regional_spawn(&mut wa).unwrap();
    crate::streaming::prepare_column(&mut wa, [0, 0]).unwrap();
    assert_eq!(wa.block(BlockPos([1, 31, 1])), Block::Wood);
    assert_eq!(sa.load_player(&mut wa).unwrap().selected(), 8);
    let (sb, mut wb, _) = c.open(&b).unwrap();
    sb.prepare_regional_spawn(&mut wb).unwrap();
    crate::streaming::prepare_column(&mut wb, [0, 0]).unwrap();
    assert_ne!(wb.block(BlockPos([1, 31, 1])), Block::Wood);
}
#[test]
fn rename_archive_and_collision_preserve_bundle() {
    let t = Temp::new();
    let c = t.catalog();
    let id = c.create("Name", 0).unwrap();
    let p = c.managed(&id).unwrap();
    let before = fs::read(p.join("world.rcw")).unwrap();
    c.rename(&id, ".. / CON: ♥").unwrap();
    assert_eq!(c.list().unwrap()[0].name, ".. / CON: ♥");
    assert_eq!(fs::read(p.join("world.rcw")).unwrap(), before);
    // The catalog gate must still exclude a new session if Windows has to
    // close the checked bundle handle immediately before moving the directory.
    let gate = c.transaction().unwrap();
    let checked = SaveStore::lock_directory(&p).unwrap();
    drop(checked);
    assert!(c.open(&id).is_err());
    drop(gate);
    fs::create_dir_all(t.0.join("archive").join(&id)).unwrap();
    assert!(c.archive(&id).is_err());
    assert!(p.exists());
    fs::remove_dir(t.0.join("archive").join(&id)).unwrap();
    let archive = c.archive(&id).unwrap();
    assert!(archive.join("player-world.rcid").exists());
    assert!(c.list().unwrap().is_empty());
    assert!(c.open(&id).is_err());
}
#[test]
fn legacy_import_copies_complete_bundle_without_modifying_source_or_overwriting() {
    let t = Temp::new();
    let c = t.catalog();
    fs::write(t.0.join("world.lock"), b"retained lock bookkeeping").unwrap();
    let (store, mut w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(88))).unwrap();
    let mut inv = store.load_player(&mut w).unwrap();
    inv.select(7);
    store.save_player_session(&w, &inv, false).unwrap();
    fs::write(t.0.join("custom.recovery"), b"opaque sidecar").unwrap();
    fs::write(t.0.join("settings.rcs"), b"root user settings").unwrap();
    assert!(c.import_legacy("Import").is_err());
    drop(store);
    let before = bytes(&t.0);
    let id = c.import_legacy("Import").unwrap();
    assert_eq!(bytes(&t.0), before);
    let p = c.managed(&id).unwrap();
    assert!(!p.join("settings.rcs").exists());
    assert_eq!(
        fs::read(p.join("custom.recovery")).unwrap(),
        b"opaque sidecar"
    );
    assert_eq!(
        fs::read(p.join("player.rcp.bak")).unwrap(),
        fs::read(t.0.join("player.rcp.bak")).unwrap()
    );
    assert_eq!(
        fs::read(p.join("world.lock")).unwrap(),
        b"retained lock bookkeeping"
    );
    let (store, mut w, _) = c.open(&id).unwrap();
    assert_eq!(store.load_player(&mut w).unwrap().selected(), 7);
    drop(store);
    let second = c.import_legacy("Import").unwrap();
    assert_ne!(id, second);
    assert!(c.list().unwrap().iter().any(|e| e.key == WorldKey::Legacy));
}
#[test]
fn invalid_ids_names_seeds_corrupt_metadata_and_incomplete_staging_fail_closed() {
    let t = Temp::new();
    let c = t.catalog();
    for id in ["../outside", "", "/tmp", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"] {
        assert!(c.open(id).is_err());
        assert!(c.archive(id).is_err());
        assert!(c.rename(id, "x").is_err());
    }
    for name in ["", "  ", "line\nname", "\0"] {
        assert!(c.create(name, 1).is_err());
    }
    assert_eq!(parse_seed("").unwrap(), None);
    assert_eq!(parse_seed("18446744073709551615").unwrap(), Some(u64::MAX));
    for s in ["-1", "1.0", " 42", "18446744073709551616"] {
        assert!(parse_seed(s).is_err());
    }
    let id = c.create("fine", 1).unwrap();
    fs::write(c.managed(&id).unwrap().join(META), b"broken").unwrap();
    assert!(c.open(&id).is_err());
    assert!(c.list().unwrap()[0].error.is_some());
    let _guard = c.transaction().unwrap();
    let (_, p) = c.stage().unwrap();
    fs::write(p.join("world.rcw"), b"interrupted copy").unwrap();
    drop(_guard);
    assert!(
        c.list()
            .unwrap()
            .iter()
            .any(|e| matches!(e.key, WorldKey::Incomplete(_)) && e.error.is_some())
    );
    assert_eq!(fs::read(p.join("world.rcw")).unwrap(), b"interrupted copy");
}
#[test]
fn corrupt_legacy_and_missing_initialized_files_are_never_replaced() {
    let t = Temp::new();
    let c = t.catalog();
    fs::write(t.0.join("world.rcw"), b"damaged").unwrap();
    let before = bytes(&t.0);
    assert!(c.import_legacy("x").is_err());
    // Lock bookkeeping is allowed; payload bytes never change.
    let after = bytes(&t.0);
    for (p, b) in before {
        assert!(after.contains(&(p, b)));
    }
    assert!(c.list().unwrap().iter().all(|e| e.error.is_some()));
    let u = Temp::new();
    let c = u.catalog();
    let id = c.create("test", 1).unwrap();
    let p = c.managed(&id).unwrap();
    fs::remove_file(p.join("player.rcp")).unwrap();
    assert!(c.open(&id).is_err());
    assert!(!p.join("player.rcp").exists());
}
#[test]
fn obstructed_creation_import_and_catalog_lock_do_not_publish_or_damage_source() {
    let t = Temp::new();
    let c = t.catalog();
    let lock = c.transaction().unwrap();
    assert!(c.create("locked", 1).is_err());
    drop(lock);
    fs::write(t.0.join(".staging"), b"obstructed").unwrap();
    assert!(c.create("x", 1).is_err());
    assert!(!t.0.join("worlds").exists());
    let (store, mut w, _) = SaveStore::open(&t.0).unwrap();
    store.load_player(&mut w).unwrap();
    drop(store);
    let before = bytes(&t.0);
    assert!(c.import_legacy("x").is_err());
    assert_eq!(bytes(&t.0), before);
}
#[cfg(unix)]
#[test]
fn symlink_escape_in_roots_entries_regions_and_sidecars_is_rejected() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let outside = Temp::new();
    let c = t.catalog();
    symlink(&outside.0, t.0.join("worlds")).unwrap();
    assert!(c.create("x", 1).is_err());
    fs::remove_file(t.0.join("worlds")).unwrap();
    let id = c.create("x", 1).unwrap();
    let p = c.managed(&id).unwrap();
    symlink(&outside.0, p.join("regions")).unwrap();
    assert!(c.open(&id).is_err());
    assert!(c.archive(&id).is_err());
    fs::remove_file(p.join("regions")).unwrap();
    symlink(outside.0.join("missing"), p.join("custom.sidecar")).unwrap();
    assert!(c.open(&id).is_err());
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
}
#[test]
fn interrupted_creation_and_partial_copy_stay_visible_but_unpublished() {
    let t = Temp::new();
    let c = t.catalog();
    assert!(
        c.create_with("interrupted", 99, || Err(io::Error::other(
            "power cut before publish"
        )))
        .is_err()
    );
    assert_eq!(fs::read_dir(t.0.join("worlds")).unwrap().count(), 0);
    let entries = c.list().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(matches!(entries[0].key, WorldKey::Incomplete(_)));
    assert!(entries[0].path.join("player.rcp").exists());
    let (store, mut w, _) = SaveStore::open(&t.0).unwrap();
    store.load_player(&mut w).unwrap();
    drop(store);
    let before = bytes(&t.0);
    assert!(
        c.import_with("partial", || Err(io::Error::other("copy interrupted")))
            .is_err()
    );
    assert_eq!(bytes(&t.0), before);
    assert_eq!(
        c.list()
            .unwrap()
            .iter()
            .filter(|e| matches!(e.key, WorldKey::Incomplete(_)))
            .count(),
        2
    );
    assert_eq!(fs::read_dir(t.0.join("worlds")).unwrap().count(), 0);
}
#[test]
fn pending_journal_is_copied_intact_and_replayed_only_in_its_world() {
    let t = Temp::new();
    let c = t.catalog();
    let (store, mut w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(42))).unwrap();
    let mut inv = store.load_player(&mut w).unwrap();
    inv.select(8);
    store.prepare_regional_spawn(&mut w).unwrap();
    crate::streaming::prepare_column(&mut w, [0, 0]).unwrap();
    w.set_block(BlockPos([2, 31, 2]), Block::Leaves).unwrap();
    assert!(
        store
            .save_player_with(&w, &inv, true, |stage| {
                if stage == SaveStage::SessionPrepared {
                    Err(io::Error::other("interrupted save"))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    drop(store);
    let before = bytes(&t.0);
    let pending = fs::read(t.0.join("session.pending")).unwrap();
    let a = c.import_legacy("journal copy").unwrap();
    let b = c.create("other", 43).unwrap();
    assert_eq!(bytes(&t.0), before);
    assert_eq!(
        fs::read(c.managed(&a).unwrap().join("session.pending")).unwrap(),
        pending
    );
    let (sa, mut wa, _) = c.open(&a).unwrap();
    assert_eq!(sa.load_player(&mut wa).unwrap().selected(), 8);
    sa.prepare_regional_spawn(&mut wa).unwrap();
    crate::streaming::prepare_column(&mut wa, [0, 0]).unwrap();
    assert_eq!(wa.block(BlockPos([2, 31, 2])), Block::Leaves);
    let (sb, mut wb, _) = c.open(&b).unwrap();
    assert_eq!(sb.load_player(&mut wb).unwrap().selected(), 0);
    assert_eq!(fs::read(t.0.join("session.pending")).unwrap(), pending);
    assert!(!c.managed(&a).unwrap().join("session.pending").exists());
    // A foreign but otherwise valid journal must never advance the other world.
    fs::write(c.managed(&b).unwrap().join("session.pending"), pending).unwrap();
    drop((sa, sb));
    assert!(c.open(&b).is_err());
}
#[cfg(unix)]
#[test]
fn permission_denial_preserves_metadata_and_prevents_publication() {
    use std::os::unix::fs::PermissionsExt;
    let t = Temp::new();
    let c = t.catalog();
    let id = c.create("original", 1).unwrap();
    let p = c.managed(&id).unwrap();
    let before = fs::read(p.join(META)).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o500)).unwrap();
    let result = c.rename(&id, "changed");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err(), "run permission tests as a non-root user");
    assert_eq!(fs::read(p.join(META)).unwrap(), before);
    let staging = t.0.join(".staging");
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o500)).unwrap();
    let result = c.create("denied", 2);
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_dir(t.0.join("worlds")).unwrap().count(), 1);
}
#[test]
fn corrupt_distant_region_and_missing_primary_with_backup_prevent_import() {
    let t = Temp::new();
    let c = t.catalog();
    let (s, mut w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(99))).unwrap();
    let inv = s.load_player(&mut w).unwrap();
    let region = crate::regions::RegionPos([10, -10]);
    let p = s.region_path(region);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, b"broken distant region").unwrap();
    drop(s);
    assert!(c.import_legacy("bad").is_err());
    assert_eq!(fs::read(&p).unwrap(), b"broken distant region");
    fs::rename(&p, p.with_extension("rcw.bak")).unwrap();
    assert!(c.import_legacy("missing").is_err());
    assert_eq!(inv.selected(), 0);
}
#[cfg(unix)]
#[test]
fn dangling_catalog_lock_never_creates_a_file_outside_root() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let outside = Temp::new();
    let target = outside.0.join("must-not-exist");
    symlink(&target, t.0.join(".catalog.lock")).unwrap();
    assert!(t.catalog().create("x", 1).is_err());
    assert!(!target.exists());
}
#[test]
fn missing_root_with_valid_pending_intent_imports_without_reinitializing_source() {
    let t = Temp::new();
    let c = t.catalog();
    let (s, mut w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(123))).unwrap();
    let mut inv = s.load_player(&mut w).unwrap();
    inv.select(6);
    assert!(
        s.save_player_with(&w, &inv, true, |stage| {
            if stage == SaveStage::SessionPrepared {
                Err(io::Error::other("interrupted initial root commit"))
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    fs::remove_file(s.path()).unwrap();
    drop(s);
    let before = bytes(&t.0);
    let id = c.import_legacy("recovered intent").unwrap();
    assert_eq!(bytes(&t.0), before);
    assert!(!t.0.join("world.rcw").exists());
    let (s, mut w, _) = c.open(&id).unwrap();
    assert_eq!(s.load_player(&mut w).unwrap().selected(), 6);
    assert_eq!(w.identity().unwrap().seed, 123);
    assert!(s.path().exists());
}
#[cfg(windows)]
#[test]
fn directory_junction_escape_is_refused() {
    use std::process::Command;
    let t = Temp::new();
    let outside = Temp::new();
    let c = t.catalog();
    let id = c.create("junction", 1).unwrap();
    let link = c.managed(&id).unwrap().join("regions");
    let result = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "junction creation: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(c.open(&id).is_err());
    assert!(c.archive(&id).is_err());
    fs::remove_dir(&link).unwrap();
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
}
