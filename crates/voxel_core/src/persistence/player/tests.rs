use super::*;
use crate::inventory::{Click, Item};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("rustcraft-player-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn player_roundtrip_and_corruption_are_strict() {
    let w = crate::streaming::new_large_world(42);
    let id = Binding::new(&w);
    let root = encode(&w).unwrap();
    let mut inv = Inventory::starter();
    inv.select(8);
    inv.click(0, Click::Right);
    let b = player_bytes(id, &root, &inv);
    assert_eq!(b.len(), PLAYER_SIZE);
    let (_, _, decoded) = decode_player(&b).unwrap();
    assert_eq!(decoded.slots(), inv.slots());
    assert_eq!(decoded.cursor(), inv.cursor());
    assert_eq!(decoded.selected(), 8);
    for i in 0..b.len() {
        let mut bad = b.clone();
        bad[i] ^= 1;
        assert!(decode_player(&bad).is_err());
    }
    for size in 0..b.len() {
        assert!(decode_player(&b[..size]).is_err());
    }
}
#[test]
fn saved_player_selection_cursor_and_world_survive_restart() {
    let t = Temp::new();
    let (store, mut w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(42))).unwrap();
    let mut inv = store.load_player(&mut w).unwrap();
    inv.select(6);
    inv.click(3, Click::Right);
    store.save_player_session(&w, &inv, false).unwrap();
    drop(store);
    let (store, mut w, _) = SaveStore::open(&t.0).unwrap();
    let restored = store.load_player(&mut w).unwrap();
    assert_eq!(restored.slots(), inv.slots());
    assert_eq!(restored.selected(), 6);
    assert_eq!(restored.cursor(), inv.cursor());
    assert!(!store.pending_path().exists());
}
#[test]
fn partial_world_player_saves_replay_complete_session_without_item_dupes() {
    for stage in [
        SaveStage::SessionPrepared,
        SaveStage::WorldSaved,
        SaveStage::PlayerSaved,
    ] {
        let t = Temp::new();
        let (store, mut w, _) = SaveStore::open(&t.0).unwrap();
        let mut inv = store.load_player(&mut w).unwrap();
        w.set_block(crate::BlockPos([0, 20, 0]), crate::Block::Wood)
            .unwrap();
        inv.insert(Item::Stone, 3);
        inv.select(7);
        let mut failed = false;
        assert!(
            store
                .save_player_with(&w, &inv, true, |s| {
                    if s == stage && !failed {
                        failed = true;
                        Err(io::Error::other("injected interruption"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        assert!(store.pending_path().exists());
        drop(store);
        let (store, mut restored, _) = SaveStore::open(&t.0).unwrap();
        let got = store.load_player(&mut restored).unwrap();
        assert_eq!(
            restored.block(crate::BlockPos([0, 20, 0])),
            crate::Block::Wood
        );
        assert_eq!(got.slots(), inv.slots());
        assert_eq!(got.selected(), 7);
        assert!(!store.pending_path().exists());
    }
}
#[test]
fn foreign_corrupt_missing_and_unknown_player_data_never_resets_to_starter() {
    let a = Temp::new();
    let b = Temp::new();
    let (sa, mut wa, _) = SaveStore::open(&a.0).unwrap();
    let (sb, mut wb, _) = SaveStore::open(&b.0).unwrap();
    sa.load_player(&mut wa).unwrap();
    sb.load_player(&mut wb).unwrap();
    let original = fs::read(sa.player_path()).unwrap();
    let foreign = fs::read(sb.player_path()).unwrap();
    fs::write(sa.player_path(), foreign).unwrap();
    assert!(sa.load_player(&mut wa).is_err());
    let mut bad = original.clone();
    bad[8] = 9;
    fs::write(sa.player_path(), &bad).unwrap();
    assert!(sa.load_player(&mut wa).is_err());
    assert_eq!(fs::read(sa.player_path()).unwrap(), bad);
    fs::remove_file(sa.player_path()).unwrap();
    assert!(sa.load_player(&mut wa).is_err());
}
#[test]
fn legacy_v1_inventory_only_saves_never_upgrade_or_rewrite_world() {
    let t = Temp::new();
    let hex = "5255535457524c4401000000010000000e000000b2d255a90000000000000000000000000300";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    fs::write(t.0.join("world.rcw"), &bytes).unwrap();
    let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
    let mut inv = s.load_player(&mut w).unwrap();
    inv.select(5);
    s.save_player_session(&w, &inv, false).unwrap();
    assert_eq!(fs::read(s.path()).unwrap(), bytes);
    assert!(!s.backup_path().exists());
}
