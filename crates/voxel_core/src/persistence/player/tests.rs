use super::*;
use crate::inventory::{Click, Item};
use std::{collections::BTreeMap, sync::Arc};
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
    let b = player_bytes(id, &root, &inv, w.time);
    assert_eq!(b.len(), PLAYER_SIZE);
    let (_, _, decoded, _) = decode_player(&b).unwrap();
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

#[test]
fn valid_checksums_do_not_bypass_slot_counts_ids_or_selection_validation() {
    let w = crate::streaming::new_large_world(42);
    let root = encode(&w).unwrap();
    let b = player_bytes(Binding::new(&w), &root, &Inventory::starter(), w.time);
    for (pos, value) in [(64, 9), (65, 8), (67, 0), (67, 65), (65, 0)] {
        let mut bad = b.clone();
        bad[pos] = value;
        let crc = checksum(&bad);
        bad[20..24].copy_from_slice(&crc.to_le_bytes());
        assert!(decode_player(&bad).is_err(), "field {pos}={value}");
    }
}
#[test]
fn partially_written_far_regions_replay_with_the_matching_inventory() {
    for failure in [2, 3] {
        let t = Temp::new();
        let (s, mut w, _) =
            SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(42))).unwrap();
        let mut inv = s.load_player(&mut w).unwrap();
        for c in [[1000, -1000], [-1000, 1000]] {
            // Support-removal validation also needs the four adjacent region
            // overlays when the edited cell lies on a pending region frontier.
            for delta in [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] {
                s.prepare_regional_column(&mut w, [c[0] + delta[0], c[1] + delta[1]])
                    .unwrap();
            }
            crate::streaming::prepare_column(&mut w, c).unwrap();
        }
        let air = crate::BlockPos([16000, -15, -16000]);
        let wood = crate::BlockPos([-16000, 25, 16000]);
        w.set_block(air, crate::Block::Air).unwrap();
        w.set_block(wood, crate::Block::Wood).unwrap();
        inv.insert(Item::Stone, 1);
        inv.select(5);
        let mut commits = 0;
        assert!(
            s.save_player_with(&w, &inv, true, |stage| {
                if stage == SaveStage::PrimaryCommitted {
                    commits += 1;
                    if commits == failure {
                        return Err(io::Error::other("partial region commit"));
                    }
                }
                Ok(())
            })
            .is_err()
        );
        assert_eq!(w.dirty_regions(), 2);
        drop(s);
        let (s, mut got, _) = SaveStore::open(&t.0).unwrap();
        let items = s.load_player(&mut got).unwrap();
        assert_eq!(items.slots(), inv.slots());
        assert_eq!(items.selected(), 5);
        for c in [[1000, -1000], [-1000, 1000]] {
            s.prepare_regional_column(&mut got, c).unwrap();
            crate::streaming::prepare_column(&mut got, c).unwrap();
        }
        assert_eq!(got.block(air), crate::Block::Air);
        assert_eq!(got.block(wood), crate::Block::Wood);
        assert!(!s.pending_path().exists());
    }
}
#[test]
fn interrupted_initial_save_recovers_identity_before_factory_or_starter_can_run() {
    let t = Temp::new();
    let (s, w, _) =
        SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(42))).unwrap();
    let binding = Binding::new(&w);
    atomic_replace(
        &s.binding_path(),
        &pack(ID_MAGIC, 0, &binding.0),
        true,
        &mut |_| Ok(()),
    )
    .unwrap();
    let mut inv = Inventory::starter();
    inv.select(8);
    assert!(
        s.save_player_with(&w, &inv, true, |stage| {
            if stage == SaveStage::SessionPrepared {
                Err(io::Error::other("crash before initial world"))
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert!(!s.path().exists());
    drop(s);
    let (s, mut w, _) =
        SaveStore::open_with(&t.0, || panic!("must not create another world")).unwrap();
    let restored = s.load_player(&mut w).unwrap();
    assert_eq!(restored.selected(), 8);
    assert_eq!(w.identity().unwrap().seed, 42);
    assert!(s.path().exists());
}
#[test]
fn corruption_of_pending_session_blocks_loading_without_touching_files() {
    let t = Temp::new();
    let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
    let inv = s.load_player(&mut w).unwrap();
    assert!(
        s.save_player_with(&w, &inv, false, |stage| {
            if stage == SaveStage::SessionPrepared {
                Err(io::Error::other("stop"))
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    let original_world = fs::read(s.path()).unwrap();
    let original_player = fs::read(s.player_path()).unwrap();
    let good = fs::read(s.pending_path()).unwrap();
    for pos in [0, 8, 16, 24, good.len() - 1] {
        let mut bad = good.clone();
        bad[pos] ^= 1;
        fs::write(s.pending_path(), &bad).unwrap();
        assert!(s.load_player(&mut w).is_err());
        assert_eq!(fs::read(s.pending_path()).unwrap(), bad);
        assert_eq!(fs::read(s.path()).unwrap(), original_world);
        assert_eq!(fs::read(s.player_path()).unwrap(), original_player);
    }
    fs::write(s.pending_path(), &good).unwrap();
    s.load_player(&mut w).unwrap();
}
#[test]
fn inventory_initialization_preserves_world_bytes_in_every_existing_mode() {
    for world in [
        crate::demo_world(),
        crate::streaming::new_world(42),
        crate::streaming::new_large_world(42),
    ] {
        let t = Temp::new();
        let (s, _, _) = SaveStore::open(&t.0).unwrap();
        s.save(&world).unwrap();
        drop(s);
        let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
        let before = fs::read(s.path()).unwrap();
        let inv = s.load_player(&mut w).unwrap();
        s.save_player_session(&w, &inv, false).unwrap();
        assert_eq!(fs::read(s.path()).unwrap(), before);
    }
}

#[test]
fn a_foreign_or_corrupt_player_blocks_redo_before_the_world_advances() {
    let t = Temp::new();
    let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
    let inv = s.load_player(&mut w).unwrap();
    let prior = fs::read(s.path()).unwrap();
    w.set_block(crate::BlockPos([0, 20, 0]), crate::Block::Wood)
        .unwrap();
    assert!(
        s.save_player_with(&w, &inv, true, |stage| {
            if stage == SaveStage::SessionPrepared {
                Err(io::Error::other("stop before world"))
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    let mut corrupt = fs::read(s.player_path()).unwrap();
    corrupt[65] ^= 1;
    fs::write(s.player_path(), &corrupt).unwrap();
    assert!(s.load_player(&mut w).is_err());
    assert_eq!(fs::read(s.path()).unwrap(), prior);
    assert_eq!(fs::read(s.player_path()).unwrap(), corrupt);
    assert!(s.pending_path().exists());
}

#[test]
fn orphan_player_backup_never_invokes_a_new_world_initializer() {
    let t = Temp::new();
    fs::write(t.0.join("player.rcp.bak"), b"preserve").unwrap();
    assert!(
        SaveStore::open_with(&t.0, || panic!(
            "must not initialize over orphan player data"
        ))
        .is_err()
    );
    assert!(!t.0.join("world.rcw").exists());
}

#[test]
fn world_only_save_api_refuses_to_decouple_an_initialized_inventory() {
    let t = Temp::new();
    let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
    s.load_player(&mut w).unwrap();
    let bytes = fs::read(s.path()).unwrap();
    w.set_block(crate::BlockPos([0, 20, 0]), crate::Block::Wood)
        .unwrap();
    assert!(s.save(&w).is_err());
    assert_eq!(fs::read(s.path()).unwrap(), bytes);
}

#[test]
fn historical_player_defaults_without_writing_and_upgrades_only_on_save() {
    let t = Temp::new();
    let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
    let inv = s.load_player(&mut w).unwrap();
    let root = fs::read(s.path()).unwrap();
    let current = fs::read(s.player_path()).unwrap();
    let old = pack(PLAYER_MAGIC, SLOT_COUNT as u32, &current[24..176]);
    fs::write(s.player_path(), &old).unwrap();
    w.time.advance(123);
    let restored = s.load_player(&mut w).unwrap();
    assert_eq!(w.time, crate::time::WorldTime::default());
    assert_eq!(restored.slots(), inv.slots());
    assert_eq!(fs::read(s.player_path()).unwrap(), old);
    assert_eq!(fs::read(s.path()).unwrap(), root);
    w.time.advance(1);
    s.save_player_session(&w, &restored, false).unwrap();
    assert_eq!(fs::read(s.path()).unwrap(), root);
    assert_eq!(u32at(&fs::read(s.player_path()).unwrap(), 8), 2);
}

#[test]
fn time_only_journal_replays_exact_fractional_clock_at_every_commit_boundary() {
    for boundary in [
        SaveStage::SessionPrepared,
        SaveStage::WorldSaved,
        SaveStage::PlayerSaved,
    ] {
        let t = Temp::new();
        let (s, mut w, _) = SaveStore::open(&t.0).unwrap();
        let mut inv = s.load_player(&mut w).unwrap();
        let root = fs::read(s.path()).unwrap();
        w.time = crate::time::WorldTime::from_parts(741, 71999).unwrap();
        inv.select(5);
        assert!(
            s.save_player_with(&w, &inv, false, |stage| {
                if stage == boundary {
                    Err(io::Error::other("injected crash"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        drop(s);
        let (s, mut restored, _) = SaveStore::open(&t.0).unwrap();
        assert_eq!(s.load_player(&mut restored).unwrap().selected(), 5);
        assert_eq!(restored.time, w.time);
        assert_eq!(fs::read(s.path()).unwrap(), root);
        assert!(!s.pending_path().exists());
    }
}

#[test]
fn clock_validation_rejects_repaired_checksum_invalid_ticks_reserved_and_versions() {
    let w = World::default();
    let bytes = player_bytes(
        Binding::new(&w),
        &encode(&w).unwrap(),
        &Inventory::starter(),
        w.time,
    );
    for (pos, data) in [(184, 72000u32), (188, 1), (8, 3)] {
        let mut bad = bytes.clone();
        bad[pos..pos + 4].copy_from_slice(&data.to_le_bytes());
        let crc = checksum(&bad);
        bad[20..24].copy_from_slice(&crc.to_le_bytes());
        assert!(decode_player(&bad).is_err());
    }
    let max = crate::time::WorldTime::from_parts(u64::MAX, 71999).unwrap();
    let b = player_bytes(
        Binding::new(&w),
        &encode(&w).unwrap(),
        &Inventory::starter(),
        max,
    );
    assert_eq!(decode_player(&b).unwrap().3, max);
}

#[test]
fn regional_water_and_inventory_journal_replay_then_persistent_source_removal() {
    use crate::{
        Block, BlockPos, Chunk, ChunkPos, LocalPos, fluid::FluidConfig, regions::RegionPos,
    };
    for stage in [
        SaveStage::SessionPrepared,
        SaveStage::WorldSaved,
        SaveStage::PlayerSaved,
    ] {
        let t = Temp::new();
        let (store, mut w, _) =
            SaveStore::open_with(&t.0, || Ok(crate::streaming::new_large_world(42))).unwrap();
        let inv = store.load_player(&mut w).unwrap();
        for cx in [3, 4] {
            w.install_region(RegionPos([cx / 4, 0]), Arc::new(BTreeMap::new()));
            for cy in -1..2 {
                let mut c = Chunk::default();
                if cy == 0 {
                    for x in 0..16 {
                        for z in 0..16 {
                            c.set(LocalPos::new([x, 0, z]).unwrap(), Block::Stone);
                        }
                    }
                }
                w.insert_chunk(ChunkPos([cx, cy, 0]), c);
            }
        }
        w.set_block(BlockPos([63, 1, 8]), Block::WaterSource)
            .unwrap();
        let mut changes = Vec::new();
        for _ in 0..200 {
            w.tick_fluids(FluidConfig::default(), &mut changes);
            if w.fluid_updates().pending() == 0 {
                break;
            }
        }
        assert_eq!(w.block(BlockPos([64, 1, 8])), Block::Water1);
        let expected = w.overlay_snapshot();
        assert!(
            store
                .save_player_with(&w, &inv, true, |s| if s == stage {
                    Err(io::Error::other("fluid journal interruption"))
                } else {
                    Ok(())
                })
                .is_err()
        );
        drop(store);
        let (store, mut restored, _) = SaveStore::open(&t.0).unwrap();
        let got = store.load_player(&mut restored).unwrap();
        assert_eq!(got.slots(), inv.slots());
        let id = restored.region_identity().unwrap();
        for p in [RegionPos([0, 0]), RegionPos([1, 0])] {
            let edits = store.read_region(id, p).unwrap();
            assert_eq!(
                &edits,
                &expected.regions().find(|(rp, _)| *rp == p).unwrap().1
            );
            restored.install_region(p, edits);
        }
        // Install exactly persisted authority; no generator is invoked for this fixture.
        for cx in [3, 4] {
            for cy in -1..2 {
                let cp = ChunkPos([cx, cy, 0]);
                let mut c = w.chunk(cp).unwrap().clone();
                for (&p, &b) in restored
                    .overlay_snapshot()
                    .regions()
                    .find(|(rp, _)| *rp == RegionPos([cx / 4, 0]))
                    .unwrap()
                    .1
                    .iter()
                    .filter(|(p, _)| p.split().0 == cp)
                {
                    c.set(p.split().1, b);
                }
                restored.insert_chunk(cp, c);
            }
        }
        restored
            .set_block(BlockPos([63, 1, 8]), Block::Air)
            .unwrap();
        for _ in 0..200 {
            restored.tick_fluids(FluidConfig::default(), &mut changes);
            if restored.fluid_updates().pending() == 0 {
                break;
            }
        }
        assert_eq!(restored.water_cells(), 0);
        store.save_player_session(&restored, &got, true).unwrap();
        drop(store);
        let (store, mut empty, _) = SaveStore::open(&t.0).unwrap();
        store.load_player(&mut empty).unwrap();
        assert_eq!(
            store.read_region(id, RegionPos([0, 0])).unwrap()[&BlockPos([63, 1, 8])],
            Block::Air
        );
    }
}
