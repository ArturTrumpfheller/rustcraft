use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustcraft-save-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn edited_world() -> World {
    let mut world = demo_world();
    for (p, block) in [
        ([-1, 0, -1], Block::Air),
        ([0, 0, 0], Block::Dirt),
        ([-17, 10, -16], Block::Grass),
        ([16, 16, 16], Block::Stone),
        ([31, -16, 31], Block::Dirt),
    ] {
        world.set_block(BlockPos(p), block).unwrap();
    }
    world
}
fn repair_checksum(bytes: &mut [u8]) {
    let crc = checksum(bytes);
    bytes[20..24].copy_from_slice(&crc.to_le_bytes());
}
fn one_block() -> Vec<u8> {
    let mut world = World::default();
    world.set_block(BlockPos([0; 3]), Block::Stone).unwrap();
    encode(&world).unwrap()
}

#[test]
fn golden_fixture_fixes_magic_version_coordinates_ids_and_crc() {
    // Independently constructed little-endian fixture with zlib CRC-32.
    let hex = "5255535457524c4401000000010000000e000000b2d255a90000000000000000000000000300";
    let expected: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(&expected[8..12], &1_u32.to_le_bytes());
    let mut upgraded = expected.clone();
    upgraded[8..12].copy_from_slice(&2_u32.to_le_bytes());
    repair_checksum(&mut upgraded);
    assert_eq!(one_block(), upgraded);
    assert_eq!(
        decode(&expected).unwrap().block(BlockPos([0; 3])),
        Block::Stone
    );
}

#[test]
fn full_snapshot_preserves_removals_replacements_and_all_materials_across_seams() {
    let world = edited_world();
    let loaded = decode(&encode(&world).unwrap()).unwrap();
    assert_eq!(loaded.block(BlockPos([-1, 0, -1])), Block::Air);
    assert_eq!(loaded.block(BlockPos([0; 3])), Block::Dirt);
    assert_eq!(loaded.block(BlockPos([-17, 10, -16])), Block::Grass);
    assert_eq!(loaded.block(BlockPos([16; 3])), Block::Stone);
    assert_eq!(loaded.block(BlockPos([31, -16, 31])), Block::Dirt);
    assert_eq!(encode(&loaded).unwrap(), encode(&world).unwrap());
}

#[test]
fn valid_empty_snapshot_is_not_mistaken_for_a_new_demo() {
    let bytes = encode(&World::default()).unwrap();
    assert_eq!(bytes.len(), HEADER_SIZE);
    assert_eq!(decode(&bytes).unwrap().chunks().count(), 0);
    let dir = TestDir::new();
    fs::write(dir.0.join("world.rcw"), bytes).unwrap();
    let (_, world, source) = SaveStore::open(&dir.0).unwrap();
    assert_eq!(source, LoadSource::Saved);
    assert_eq!(world.chunks().count(), 0);
}

#[test]
fn every_truncated_prefix_trailing_bytes_and_payload_damage_are_rejected() {
    let bytes = encode(&edited_world()).unwrap();
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "accepted prefix {end}");
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert_eq!(decode(&extra), Err(FormatError::Length));
    let mut damaged = bytes;
    damaged[HEADER_SIZE + 5] ^= 0x80;
    assert_eq!(decode(&damaged), Err(FormatError::Checksum));
}

#[test]
fn malformed_headers_ids_coordinates_and_record_order_are_rejected() {
    let valid = one_block();
    let mut bytes = valid.clone();
    bytes[0] ^= 1;
    assert_eq!(decode(&bytes), Err(FormatError::Magic));
    let mut bytes = valid.clone();
    bytes[8..12].copy_from_slice(&999_u32.to_le_bytes());
    assert_eq!(decode(&bytes), Err(FormatError::Version(999)));
    let mut bytes = valid.clone();
    bytes[12..16].copy_from_slice(&((MAX_RECORDS + 1) as u32).to_le_bytes());
    assert_eq!(decode(&bytes), Err(FormatError::TooLarge));
    let mut bytes = valid.clone();
    bytes[16..20].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(decode(&bytes), Err(FormatError::Length));
    for id in [0_u16, 31, 65535] {
        let mut bytes = valid.clone();
        bytes[HEADER_SIZE + 12..].copy_from_slice(&id.to_le_bytes());
        repair_checksum(&mut bytes);
        assert_eq!(decode(&bytes), Err(FormatError::BlockId(id)));
    }
    let mut bytes = valid;
    bytes[HEADER_SIZE..HEADER_SIZE + 4].copy_from_slice(&32_i32.to_le_bytes());
    repair_checksum(&mut bytes);
    assert_eq!(decode(&bytes), Err(FormatError::Coordinates));
    let mut world = World::default();
    world.set_block(BlockPos([0; 3]), Block::Dirt).unwrap();
    world.set_block(BlockPos([1, 0, 0]), Block::Stone).unwrap();
    let mut duplicate = encode(&world).unwrap();
    let first = duplicate[HEADER_SIZE..HEADER_SIZE + RECORD_SIZE].to_vec();
    duplicate[HEADER_SIZE + RECORD_SIZE..].copy_from_slice(&first);
    repair_checksum(&mut duplicate);
    assert_eq!(decode(&duplicate), Err(FormatError::UnsortedOrDuplicate));
    let mut reversed = encode(&world).unwrap();
    for i in 0..RECORD_SIZE {
        reversed.swap(HEADER_SIZE + i, HEADER_SIZE + RECORD_SIZE + i);
    }
    repair_checksum(&mut reversed);
    assert_eq!(decode(&reversed), Err(FormatError::UnsortedOrDuplicate));
    assert_eq!(
        decode(&vec![0; MAX_FILE_SIZE + 1]),
        Err(FormatError::TooLarge)
    );
}

#[test]
fn absent_save_and_stale_temporary_files_leave_the_demo_clean_and_do_not_write_world() {
    let dir = TestDir::new();
    fs::write(dir.0.join("world.rcw.tmp.stale"), b"interrupted").unwrap();
    for _ in 0..2 {
        let (store, world, source) = SaveStore::open(&dir.0).unwrap();
        assert_eq!(source, LoadSource::NewDemo);
        assert_eq!(world, demo_world());
        assert!(!store.path().exists());
        assert!(!store.backup_path().exists());
    }
    assert_eq!(
        fs::read(dir.0.join("world.rcw.tmp.stale")).unwrap(),
        b"interrupted"
    );
}

#[test]
fn atomic_replacement_keeps_previous_generation_and_restarts_into_latest_world() {
    let dir = TestDir::new();
    let old = demo_world();
    let new = edited_world();
    {
        let (store, _, _) = SaveStore::open(&dir.0).unwrap();
        store.save(&old).unwrap();
        store.save(&new).unwrap();
        assert_eq!(
            fs::read(store.backup_path()).unwrap(),
            encode(&old).unwrap()
        );
        assert_eq!(fs::read(store.path()).unwrap(), encode(&new).unwrap());
    }
    let (_, loaded, source) = SaveStore::open(&dir.0).unwrap();
    assert_eq!(source, LoadSource::Saved);
    assert_eq!(encode(&loaded).unwrap(), encode(&new).unwrap());
}

#[test]
fn injected_partial_write_sync_and_rename_failures_preserve_previous_primary() {
    for stage in [
        SaveStage::PartialTemp,
        SaveStage::TempSynced,
        SaveStage::BackupCommitted,
        SaveStage::BeforePrimaryRename,
    ] {
        let dir = TestDir::new();
        let (store, _, _) = SaveStore::open(&dir.0).unwrap();
        store.save(&demo_world()).unwrap();
        let before = fs::read(store.path()).unwrap();
        let result = store.save_with(&edited_world(), |current| {
            if current == stage {
                Err(io::Error::other("injected disk failure"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(fs::read(store.path()).unwrap(), before);
        assert!(decode(&fs::read(store.path()).unwrap()).is_ok());
        let leftovers: Vec<_> = fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}

#[test]
fn first_failed_save_never_leaves_a_partial_primary_and_postcommit_error_is_not_success() {
    let dir = TestDir::new();
    let (store, _, _) = SaveStore::open(&dir.0).unwrap();
    assert!(
        store
            .save_with(&demo_world(), |stage| if stage == SaveStage::PartialTemp {
                Err(io::Error::other("disk full"))
            } else {
                Ok(())
            })
            .is_err()
    );
    assert!(!store.path().exists());
    store.save(&demo_world()).unwrap();
    assert!(
        store
            .save_with(&edited_world(), |stage| {
                if stage == SaveStage::PrimaryCommitted {
                    Err(io::Error::other("injected completion failure"))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(
        fs::read(store.path()).unwrap(),
        encode(&edited_world()).unwrap()
    );
    assert_eq!(
        fs::read(store.backup_path()).unwrap(),
        encode(&demo_world()).unwrap()
    );
}

#[test]
fn corrupt_primary_is_never_overwritten_and_backup_can_be_recovered_explicitly() {
    let dir = TestDir::new();
    {
        let (store, _, _) = SaveStore::open(&dir.0).unwrap();
        store.save(&demo_world()).unwrap();
        store.save(&edited_world()).unwrap();
        fs::write(store.path(), b"damaged user data").unwrap();
        assert!(store.save(&demo_world()).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), b"damaged user data");
        assert!(decode(&fs::read(store.backup_path()).unwrap()).is_ok());
    }
    assert!(SaveStore::open(&dir.0).is_err());
    assert_eq!(
        fs::read(dir.0.join("world.rcw")).unwrap(),
        b"damaged user data"
    );
    fs::rename(dir.0.join("world.rcw"), dir.0.join("world.rcw.damaged")).unwrap();
    assert!(SaveStore::open(&dir.0).is_err()); // Backup presence is not a new world.
    fs::copy(dir.0.join("world.rcw.bak"), dir.0.join("world.rcw")).unwrap();
    let (_, recovered, source) = SaveStore::open(&dir.0).unwrap();
    assert_eq!(source, LoadSource::Saved);
    assert_eq!(encode(&recovered).unwrap(), encode(&demo_world()).unwrap());
}

#[test]
fn invalid_paths_backup_failure_and_oversized_files_do_not_damage_existing_save() {
    let dir = TestDir::new();
    fs::write(dir.0.join("not-a-directory"), b"keep").unwrap();
    assert!(SaveStore::open(dir.0.join("not-a-directory/child")).is_err());
    let (store, _, _) = SaveStore::open(dir.0.join("valid")).unwrap();
    store.save(&demo_world()).unwrap();
    fs::create_dir(store.backup_path()).unwrap();
    assert!(store.save(&edited_world()).is_err());
    assert_eq!(
        fs::read(store.path()).unwrap(),
        encode(&demo_world()).unwrap()
    );
    let other = dir.0.join("oversized");
    fs::create_dir(&other).unwrap();
    let file = File::create(other.join("world.rcw")).unwrap();
    file.set_len(MAX_FILE_SIZE as u64 + 1).unwrap();
    drop(file);
    assert!(matches!(
        SaveStore::open(&other),
        Err(SaveError::Invalid(FormatError::TooLarge))
    ));
}

#[test]
fn second_instance_cannot_open_the_same_world_until_the_lock_is_released() {
    let dir = TestDir::new();
    let (store, _, _) = SaveStore::open(&dir.0).unwrap();
    assert!(SaveStore::open(&dir.0).is_err());
    drop(store);
    assert!(SaveStore::open(&dir.0).is_ok());
}

#[test]
fn encoder_refuses_unsupported_imported_coordinates_and_noops_do_not_change_revision() {
    let mut world = demo_world();
    let revision = world.revision();
    assert_eq!(
        world.set_block(BlockPos([0; 3]), Block::Stone).unwrap(),
        None
    );
    assert_eq!(world.revision(), revision);
    world.set_block(BlockPos([0; 3]), Block::Grass).unwrap();
    assert_ne!(world.revision(), revision);
    world.insert_chunk(crate::ChunkPos([2, 0, 0]), crate::Chunk::default());
    assert_eq!(encode(&world), Err(FormatError::Coordinates));
}

fn historical_v1() -> Vec<u8> {
    // Independently authored with Python struct + zlib, not the current encoder.
    let hex = "5255535457524c4401000000030000002a000000972f333aefffffffffffffff100000000100ffffffff02000000ffffffff02000000000000000000000000000300";
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn historical_v1_load_is_read_only_factory_is_not_called_and_changed_save_upgrades_with_backup() {
    let dir = TestDir::new();
    let fixture = historical_v1();
    fs::write(dir.0.join("world.rcw"), &fixture).unwrap();
    let (store, mut world, source) =
        SaveStore::open_with(&dir.0, || panic!("must not regenerate existing worlds")).unwrap();
    assert_eq!(source, LoadSource::Saved);
    assert_eq!(world.block(BlockPos([-17, -1, 16])), Block::Grass);
    assert_eq!(world.block(BlockPos([-1, 2, -1])), Block::Dirt);
    assert_eq!(world.block(BlockPos([0; 3])), Block::Stone);
    assert_eq!(world.block(BlockPos([1, 0, 0])), Block::Air); // No demo overlay.
    assert_eq!(fs::read(store.path()).unwrap(), fixture);
    assert!(!store.backup_path().exists());
    world
        .set_block(BlockPos([16, 16, -17]), Block::Wood)
        .unwrap();
    world
        .set_block(BlockPos([-16, 17, 16]), Block::Leaves)
        .unwrap();
    store.save(&world).unwrap();
    assert_eq!(fs::read(store.backup_path()).unwrap(), fixture);
    assert_eq!(
        &fs::read(store.path()).unwrap()[8..12],
        &2_u32.to_le_bytes()
    );
    drop(store);
    let (_, loaded, source) =
        SaveStore::open_with(&dir.0, || panic!("override seed must be ignored")).unwrap();
    assert_eq!(source, LoadSource::Saved);
    assert_eq!(encode(&loaded).unwrap(), encode(&world).unwrap());
    assert_eq!(loaded.block(BlockPos([1, 0, 0])), Block::Air);
}

#[test]
fn v1_never_accepts_new_ids_and_unknown_formats_never_call_new_world_factory() {
    let mut world = World::default();
    world.set_block(BlockPos([0; 3]), Block::Wood).unwrap();
    let wood = encode(&world).unwrap();
    assert_eq!(decode(&wood).unwrap().block(BlockPos([0; 3])), Block::Wood);
    for id in [4_u16, 5] {
        let mut bytes = wood.clone();
        bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
        bytes[36..38].copy_from_slice(&id.to_le_bytes());
        repair_checksum(&mut bytes);
        assert_eq!(decode(&bytes), Err(FormatError::BlockId(id)));
    }
    let dir = TestDir::new();
    let mut bytes = wood;
    bytes[8..12].copy_from_slice(&5_u32.to_le_bytes());
    repair_checksum(&mut bytes);
    fs::write(dir.0.join("world.rcw"), &bytes).unwrap();
    assert!(SaveStore::open_with(&dir.0, || panic!("not a missing save")).is_err());
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), bytes);
}

#[test]
fn v3_identity_empty_overlay_golden_fixture_and_strict_metadata_validation() {
    let mut world = crate::streaming::new_world(42);
    let bytes = encode(&world).unwrap();
    let hex = "5255535457524c44030000000000000000000000d0795dd72a000000000000000200000000000000";
    let fixture: Vec<_> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(bytes, fixture);
    assert_eq!(decode(&bytes).unwrap().identity(), world.identity());
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err());
    }
    for (offset, value) in [(32, 99_u32), (36, 1)] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        repair_checksum(&mut bad);
        assert_eq!(decode(&bad), Err(FormatError::Identity));
    }
    crate::streaming::prepare_spawn(&mut world).unwrap();
    world.set_block(BlockPos([0, -15, 0]), Block::Air).unwrap();
    let air = encode(&world).unwrap();
    assert_eq!(
        decode(&air).unwrap().edits()[&BlockPos([0, -15, 0])],
        Block::Air
    );
    for id in [31_u16, 65535] {
        let mut bad = air.clone();
        bad[52..54].copy_from_slice(&id.to_le_bytes());
        repair_checksum(&mut bad);
        assert_eq!(decode(&bad), Err(FormatError::BlockId(id)));
    }
    let mut bad = air.clone();
    bad[40..44].copy_from_slice(&64_i32.to_le_bytes());
    repair_checksum(&mut bad);
    assert_eq!(decode(&bad), Err(FormatError::Coordinates));
    let mut bad = air;
    bad[24] ^= 1;
    assert_eq!(decode(&bad), Err(FormatError::Checksum));
}

#[test]
fn failed_stream_saves_preserve_dirty_evicted_overlay_and_prior_durable_generation() {
    let dir = TestDir::new();
    let (store, mut world, _) =
        SaveStore::open_with(&dir.0, || Ok(crate::streaming::new_world(7))).unwrap();
    crate::streaming::prepare_spawn(&mut world).unwrap();
    store.save(&world).unwrap();
    let prior = fs::read(store.path()).unwrap();
    world.set_block(BlockPos([0, -15, 0]), Block::Air).unwrap();
    world.set_block(BlockPos([0, 25, 0]), Block::Wood).unwrap();
    let revision = world.revision();
    world.evict_column([0, 0]);
    for stage in [
        SaveStage::PartialTemp,
        SaveStage::TempSynced,
        SaveStage::BackupCommitted,
        SaveStage::BeforePrimaryRename,
    ] {
        assert!(
            store
                .save_with(&world, |current| if current == stage {
                    Err(io::Error::other("injected stream disk failure"))
                } else {
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(store.path()).unwrap(), prior);
        assert_eq!(world.edits().len(), 2);
        assert_eq!(world.revision(), revision);
    }
    store.save(&world).unwrap();
    assert_eq!(fs::read(store.backup_path()).unwrap(), prior);
    drop(store);
    let (_, mut loaded, _) =
        SaveStore::open_with(&dir.0, || panic!("never recreate on restart")).unwrap();
    assert_eq!(loaded.chunks().count(), 0);
    crate::streaming::prepare_spawn(&mut loaded).unwrap();
    assert_eq!(loaded.block(BlockPos([0, -15, 0])), Block::Air);
    assert_eq!(loaded.block(BlockPos([0, 25, 0])), Block::Wood);
}

#[test]
fn save_store_refuses_implicit_identity_mode_or_seed_migration() {
    for legacy in [false, true] {
        let dir = TestDir::new();
        let original = if legacy {
            demo_world()
        } else {
            crate::streaming::new_world(1)
        };
        let (store, _, _) = SaveStore::open(&dir.0).unwrap();
        store.save(&original).unwrap();
        let bytes = fs::read(store.path()).unwrap();
        for other in [
            crate::streaming::new_world(2),
            if legacy {
                crate::streaming::new_world(1)
            } else {
                demo_world()
            },
        ] {
            assert!(store.save(&other).is_err());
            assert_eq!(fs::read(store.path()).unwrap(), bytes);
            assert!(!store.backup_path().exists());
        }
    }
}

#[test]
fn torch_snapshot_and_finite_overlay_roundtrip_without_reinterpreting_old_ids() {
    for mut w in [World::default(), crate::streaming::new_world(42)] {
        if w.identity().is_some() {
            crate::streaming::prepare_spawn(&mut w).unwrap();
        }
        let support = BlockPos([1, 24, 0]);
        let torch = BlockPos([1, 25, 0]);
        w.set_block(support, Block::Stone).unwrap();
        w.set_block(torch, Block::Torch).unwrap();
        let bytes = encode(&w).unwrap();
        let mut loaded = decode(&bytes).unwrap();
        assert_eq!(encode(&loaded).unwrap(), bytes);
        if loaded.identity().is_some() {
            crate::streaming::prepare_spawn(&mut loaded).unwrap();
        }
        assert_eq!(loaded.block(torch), Block::Torch);
        let targets = std::collections::BTreeSet::from([torch.split().0]);
        assert_eq!(
            crate::lighting::Skylight::for_chunks(&loaded, &targets).block_level(torch),
            14
        );
    }
    assert_eq!(stored_block(6, 1), Err(FormatError::BlockId(6)));
    assert_eq!(stored_block(31, 2), Err(FormatError::BlockId(31)));
    for (id, block) in [
        (0, Block::Air),
        (1, Block::Grass),
        (2, Block::Dirt),
        (3, Block::Stone),
        (4, Block::Wood),
        (5, Block::Leaves),
        (6, Block::Torch),
    ] {
        assert_eq!(block_id(block), id);
        assert_eq!(stored_block(id, STREAM_FORMAT_VERSION), Ok(block));
    }
}

#[test]
fn wall_ids_are_append_only_and_orientations_round_trip_all_world_modes() {
    let variants = [
        Block::TorchEast,
        Block::TorchWest,
        Block::TorchSouth,
        Block::TorchNorth,
    ];
    for (n, b) in variants.into_iter().enumerate() {
        assert_eq!(block_id(b), 7 + n as u16);
        assert!(stored_block(7 + n as u16, 1).is_err());
        for mut w in [World::default(), crate::streaming::new_world(42)] {
            if w.identity().is_some() {
                crate::streaming::prepare_spawn(&mut w).unwrap();
            }
            let p = BlockPos([8, 25, 8]);
            w.set_block(p.offset(b.torch_support().unwrap()).unwrap(), Block::Stone)
                .unwrap();
            w.set_block(p, b).unwrap();
            let bytes = encode(&w).unwrap();
            let mut got = decode(&bytes).unwrap();
            assert_eq!(encode(&got).unwrap(), bytes);
            if got.identity().is_some() {
                crate::streaming::prepare_spawn(&mut got).unwrap();
            }
            assert_eq!(got.block(p), b);
            assert_eq!(got.block(p).emission(), 14);
        }
    }
    assert_eq!(stored_block(6, 2), Ok(Block::Torch));
    assert_eq!(block_id(Block::Torch), 6);
    assert_eq!(stored_block(31, 3), Err(FormatError::BlockId(31)));
}

#[test]
fn water_registry_is_explicit_append_only_for_every_state() {
    let old = [
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
    ];
    for (id, b) in old.into_iter().enumerate() {
        assert_eq!(block_id(b), id as u16);
        assert_eq!(stored_block(id as u16, 3).unwrap(), b);
    }
    for state in 0..=8 {
        let b = crate::fluid::Fluid::from_state_id(state).unwrap().block();
        assert_eq!(block_id(b), 11 + u16::from(state));
        assert_eq!(stored_block(11 + u16::from(state), 2).unwrap(), b);
        assert!(stored_block(11 + u16::from(state), 1).is_err());
    }
}
