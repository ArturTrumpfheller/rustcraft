use super::*;
use bevy::time::TimeUpdateStrategy;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use voxel_core::{
    Block, BlockPos,
    persistence::{decode, encode},
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);
struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustcraft-client-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
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

fn app_at(dir: &TestDir) -> App {
    let (store, world, source) = SaveStore::open(&dir.0).unwrap();
    app_from(store, world, source)
}
fn app_from(store: SaveStore, world: World, source: LoadSource) -> App {
    let session = SaveSession::new(store, &world, source);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .insert_resource(session)
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins(PersistencePlugin);
    app.world_mut().spawn((
        Window {
            focused: true,
            title: "RustCraft — test".into(),
            ..default()
        },
        PrimaryWindow,
    ));
    frame(&mut app, Duration::ZERO);
    app
}
fn frame(app: &mut App, duration: Duration) {
    *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
        TimeUpdateStrategy::ManualDuration(duration);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
}
fn edit(app: &mut App, pos: [i32; 3], block: Block) {
    app.world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(BlockPos(pos), block)
        .unwrap();
}
fn f5(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);
    frame(app, Duration::ZERO);
}
fn drain(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while app.world().resource::<SaveSession>().job.is_some() {
        assert!(Instant::now() < deadline, "save worker timed out");
        std::thread::yield_now();
        frame(app, Duration::ZERO);
    }
}
fn close(app: &mut App) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .write_message(WindowCloseRequested { window });
    frame(app, Duration::ZERO);
}

#[test]
fn platform_paths_obey_override_xdg_and_appdata_without_changing_process_environment() {
    let root = std::env::temp_dir();
    let path = |windows, vars: &[(&str, OsString)]| {
        directory_from(windows, |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.clone())
        })
    };
    let xdg = root.join("xdg");
    let home = root.join("home");
    assert_eq!(
        path(
            false,
            &[
                ("XDG_DATA_HOME", xdg.clone().into()),
                ("HOME", home.clone().into())
            ]
        )
        .unwrap(),
        xdg.join("rustcraft")
    );
    for invalid in ["", "relative"] {
        assert_eq!(
            path(
                false,
                &[
                    ("XDG_DATA_HOME", invalid.into()),
                    ("HOME", home.clone().into())
                ]
            )
            .unwrap(),
            home.join(".local/share/rustcraft")
        );
        assert!(
            path(
                false,
                &[
                    ("RUSTCRAFT_DATA_DIR", invalid.into()),
                    ("HOME", home.clone().into())
                ]
            )
            .is_err()
        );
    }
    for windows in [true, false] {
        assert_eq!(
            path(windows, &[("RUSTCRAFT_DATA_DIR", root.clone().into())]).unwrap(),
            root
        );
        assert!(path(windows, &[]).is_err());
    }
    let local = root.join("local");
    let roaming = root.join("roaming");
    assert_eq!(
        path(
            true,
            &[
                ("LOCALAPPDATA", local.clone().into()),
                ("APPDATA", roaming.clone().into())
            ]
        )
        .unwrap(),
        local.join("RustCraft")
    );
    assert_eq!(
        path(
            true,
            &[
                ("LOCALAPPDATA", "relative".into()),
                ("APPDATA", roaming.clone().into())
            ]
        )
        .unwrap(),
        roaming.join("RustCraft")
    );
}

#[test]
fn unchanged_demo_frames_f5_and_close_do_not_create_a_save() {
    let dir = TestDir::new();
    let mut app = app_at(&dir);
    for _ in 0..3 {
        frame(&mut app, Duration::from_secs(60));
    }
    edit(&mut app, [0; 3], Block::Stone); // The original stone: a no-op.
    f5(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "New demo");
    assert!(!dir.0.join("world.rcw").exists());
    close(&mut app);
    assert!(app.should_exit().is_some());
    assert!(!dir.0.join("world.rcw").exists());
}

#[test]
fn manual_save_restarts_with_removed_replaced_and_new_boundary_blocks_then_stays_clean() {
    let dir = TestDir::new();
    let expected;
    {
        let mut app = app_at(&dir);
        edit(&mut app, [-1, 0, -1], Block::Air);
        edit(&mut app, [0; 3], Block::Dirt);
        edit(&mut app, [16, 8, -17], Block::Grass);
        frame(&mut app, Duration::ZERO);
        assert_eq!(app.world().resource::<SaveFeedback>().label, "Dirty");
        expected = encode(&app.world().resource::<VoxelWorld>().0).unwrap();
        f5(&mut app);
        drain(&mut app);
        assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
        assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), expected);
    }
    let mut app = app_at(&dir);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Loaded");
    assert_eq!(
        encode(&app.world().resource::<VoxelWorld>().0).unwrap(),
        expected
    );
    frame(&mut app, Duration::from_secs(120));
    f5(&mut app);
    assert!(
        !dir.0.join("world.rcw.bak").exists(),
        "unchanged world was saved again"
    );
}

#[test]
fn autosave_uses_real_time_even_without_player_or_focus_and_skips_idle_intervals() {
    let dir = TestDir::new();
    let mut app = app_at(&dir);
    app.world_mut()
        .query::<&mut Window>()
        .single_mut(app.world_mut())
        .unwrap()
        .focused = false;
    edit(&mut app, [0; 3], Block::Dirt);
    frame(&mut app, Duration::from_secs(29));
    assert!(!dir.0.join("world.rcw").exists());
    frame(&mut app, Duration::from_secs(1));
    drain(&mut app);
    assert!(dir.0.join("world.rcw").exists());
    frame(&mut app, Duration::from_secs(90));
    assert!(!dir.0.join("world.rcw.bak").exists());
}

#[test]
fn edits_during_in_flight_snapshot_remain_dirty_and_close_saves_the_latest_revision() {
    let dir = TestDir::new();
    let mut app = app_at(&dir);
    edit(&mut app, [0; 3], Block::Dirt);
    // Hold an actual worker at a channel barrier to deterministically edit
    // after capture, without timing-dependent assumptions about disk speed.
    let snapshot = app.world().resource::<VoxelWorld>().0.clone();
    let revision = snapshot.revision();
    let store = Arc::clone(&app.world().resource::<SaveSession>().store);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        rx.recv().unwrap();
        store.save(&snapshot)
    });
    app.world_mut().resource_mut::<SaveSession>().job = Some(SaveJob {
        time: Default::default(),
        revision,
        inventory_revision: None,
        handle,
    });
    edit(&mut app, [16, 8, -17], Block::Grass);
    frame(&mut app, Duration::ZERO);
    tx.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Dirty");
    assert_eq!(
        decode(&fs::read(dir.0.join("world.rcw")).unwrap())
            .unwrap()
            .block(BlockPos([16, 8, -17])),
        Block::Air
    );
    close(&mut app);
    assert!(
        app.should_exit().is_none(),
        "exited before the final save finished"
    );
    drain(&mut app);
    assert!(app.should_exit().is_some());
    assert_eq!(
        fs::read(dir.0.join("world.rcw")).unwrap(),
        encode(&app.world().resource::<VoxelWorld>().0).unwrap()
    );
}

#[test]
fn save_failure_preserves_primary_keeps_changes_dirty_cancels_close_and_allows_f5_retry() {
    let dir = TestDir::new();
    let mut app = app_at(&dir);
    edit(&mut app, [0; 3], Block::Dirt);
    f5(&mut app);
    drain(&mut app);
    let primary = fs::read(dir.0.join("world.rcw")).unwrap();
    fs::create_dir(dir.0.join("world.rcw.bak")).unwrap(); // Real cross-platform I/O failure.
    edit(&mut app, [0; 3], Block::Grass);
    close(&mut app);
    drain(&mut app);
    assert_eq!(
        app.world().resource::<SaveFeedback>().label,
        "ERROR / DIRTY"
    );
    assert!(!app.world().resource::<SaveFeedback>().closing);
    assert!(app.should_exit().is_none());
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), primary);
    fs::remove_dir(dir.0.join("world.rcw.bak")).unwrap();
    f5(&mut app);
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
    assert_eq!(fs::read(dir.0.join("world.rcw.bak")).unwrap(), primary);
}

#[test]
fn restored_world_initializes_meshes_collision_and_clear_spawn_instead_of_demo() {
    use crate::{
        player::{PlayerPlugin, PlayerSettings, PlayerState, SPAWN},
        scene::VoxelScenePlugin,
    };
    use voxel_core::{
        ChunkPos,
        physics::{MovementInput, PhysicsConfig, Player},
    };
    let mut world = World::default();
    // Saved blocks occupy the usual spawn; initialization must move above them.
    world.set_block(BlockPos([0, 4, 10]), Block::Stone).unwrap();
    world
        .set_block(BlockPos([16, 1, -17]), Block::Dirt)
        .unwrap();
    let world = decode(&encode(&world).unwrap()).unwrap();
    let mut body = Player::new([16.5, 4., -16.5]);
    for _ in 0..120 {
        body.tick(&world, MovementInput::default(), &PhysicsConfig::default());
    }
    assert!(body.grounded);
    assert!((body.position[1] - 2.).abs() < 1e-7);
    let mut app = App::new();
    app.insert_resource(VoxelWorld(world))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_plugins((VoxelScenePlugin, PlayerPlugin));
    app.update();
    let world = app.world_mut();
    let state = world.resource::<PlayerState>();
    assert!(state.body.position[1] > SPAWN[1]);
    assert_eq!(state.body.position, state.previous_position);
    assert!(state.body.fits_in_world(
        &world.resource::<VoxelWorld>().0,
        &world.resource::<PlayerSettings>().0
    ));
    assert_eq!(
        world.resource::<VoxelWorld>().0.block(BlockPos([0; 3])),
        Block::Air
    );
    assert!(
        world
            .resource::<VoxelWorld>()
            .0
            .chunk(ChunkPos([1, 0, -2]))
            .is_some()
    );
    let handles: Vec<_> = world
        .query::<&Mesh3d>()
        .iter(world)
        .map(|h| h.0.clone())
        .collect();
    assert_eq!(handles.len(), 2);
    for handle in handles {
        assert_eq!(
            world
                .resource::<Assets<Mesh>>()
                .get(&handle)
                .unwrap()
                .indices()
                .unwrap()
                .len(),
            36
        );
    }
}

fn generated_app(dir: &TestDir, seed: u64) -> App {
    let (store, world, source) = SaveStore::open_with(&dir.0, || {
        Ok(
            voxel_core::worldgen::Generator::new(voxel_core::worldgen::GenerationConfig {
                seed,
                ..default()
            })
            .unwrap()
            .generate(),
        )
    })
    .unwrap();
    app_from(store, world, source)
}

#[test]
fn new_world_without_edits_is_saved_once_and_changed_seed_never_changes_loaded_blocks() {
    let dir = TestDir::new();
    let original;
    {
        let mut app = generated_app(&dir, 42);
        original = encode(&app.world().resource::<VoxelWorld>().0).unwrap();
        drain(&mut app);
        assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
        assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), original);
        assert!(!dir.0.join("world.rcw.bak").exists());
        frame(&mut app, Duration::from_secs(120));
        f5(&mut app);
        close(&mut app);
        assert!(app.should_exit().is_some());
        assert!(
            !dir.0.join("world.rcw.bak").exists(),
            "clean new world was saved repeatedly"
        );
    }
    let mut app = generated_app(&dir, 999);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Loaded");
    assert_eq!(
        encode(&app.world().resource::<VoxelWorld>().0).unwrap(),
        original
    );
    edit(&mut app, [-17, 30, 16], Block::Wood);
    edit(&mut app, [-16, 30, 16], Block::Leaves);
    edit(&mut app, [0, -16, 0], Block::Air);
    f5(&mut app);
    drain(&mut app);
    let edited = encode(&app.world().resource::<VoxelWorld>().0).unwrap();
    assert_eq!(fs::read(dir.0.join("world.rcw.bak")).unwrap(), original);
    drop(app);
    let app = generated_app(&dir, 0);
    assert_eq!(
        encode(&app.world().resource::<VoxelWorld>().0).unwrap(),
        edited
    );
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([0, -16, 0])),
        Block::Air
    );
}

#[test]
fn first_creation_failure_remains_unsaved_and_close_cannot_silently_drop_new_world() {
    let dir = TestDir::new();
    fs::create_dir(dir.0.join("world.rcw.bak")).unwrap();
    // Open first, then obstruct the backup path before the first save attempt.
    fs::remove_dir(dir.0.join("world.rcw.bak")).unwrap();
    let (store, world, source) = SaveStore::open_with(&dir.0, || Ok(World::default())).unwrap();
    fs::create_dir(dir.0.join("world.rcw.bak")).unwrap();
    let mut app = app_from(store, world, source);
    drain(&mut app);
    assert_eq!(
        app.world().resource::<SaveFeedback>().label,
        "ERROR / DIRTY"
    );
    assert!(!dir.0.join("world.rcw").exists());
    close(&mut app);
    drain(&mut app);
    assert!(app.should_exit().is_none());
    fs::remove_dir(dir.0.join("world.rcw.bak")).unwrap();
    f5(&mut app);
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
    assert_eq!(
        decode(&fs::read(dir.0.join("world.rcw")).unwrap())
            .unwrap()
            .chunks()
            .count(),
        0
    );
}

#[test]
fn seed_parser_has_defined_default_and_strict_u64_validation() {
    assert_eq!(seed_from(None).unwrap(), 2026);
    for (s, n) in [("0", 0), ("42", 42), ("18446744073709551615", u64::MAX)] {
        assert_eq!(seed_from(Some(s.into())).unwrap(), n);
    }
    for s in [
        "",
        "-1",
        "+1",
        " 42",
        "42 ",
        "0xff",
        "18446744073709551616",
        "seed",
    ] {
        assert!(seed_from(Some(s.into())).is_err(), "{s}");
    }
}

#[test]
fn streamed_dirty_eviction_failed_save_retry_and_restart_retain_complete_overlay() {
    let dir = TestDir::new();
    let (store, mut world, source) =
        SaveStore::open_with(&dir.0, || Ok(voxel_core::streaming::new_world(42))).unwrap();
    let generator = voxel_core::streaming::prepare_spawn(&mut world).unwrap();
    let mut app = app_from(store, world, source);
    drain(&mut app);
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap().len(), 40);
    let removed = BlockPos([0, -15, 0]);
    edit(&mut app, removed.0, Block::Air);
    let data = voxel_core::streaming::build_column(
        &generator,
        app.world().resource::<VoxelWorld>().0.edits(),
        [-3, 2],
        1,
    );
    for (p, c, _) in data.chunks {
        app.world_mut()
            .resource_mut::<VoxelWorld>()
            .0
            .insert_chunk(p, c);
    }
    let added = BlockPos([-40, 25, 40]);
    edit(&mut app, added.0, Block::Leaves);
    {
        let mut w = app.world_mut().resource_mut::<VoxelWorld>();
        w.0.evict_column([0, 0]);
        w.0.evict_column([-3, 2]);
        assert_eq!(w.0.chunks().count(), 0);
    }
    fs::create_dir(dir.0.join("world.rcw.bak")).unwrap();
    f5(&mut app);
    drain(&mut app);
    assert_eq!(
        app.world().resource::<SaveFeedback>().label,
        "ERROR / DIRTY"
    );
    assert_eq!(app.world().resource::<VoxelWorld>().0.edits().len(), 2);
    close(&mut app);
    drain(&mut app);
    assert!(app.should_exit().is_none());
    fs::remove_dir(dir.0.join("world.rcw.bak")).unwrap();
    f5(&mut app);
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap().len(), 68);
    drop(app);
    let (_, mut restored, source) =
        SaveStore::open_with(&dir.0, || panic!("no override seed on load")).unwrap();
    assert_eq!(source, LoadSource::Saved);
    voxel_core::streaming::prepare_spawn(&mut restored).unwrap();
    assert_eq!(restored.block(removed), Block::Air);
    let data = voxel_core::streaming::build_column(&generator, restored.edits(), [-3, 2], 0);
    for (p, c, _) in data.chunks {
        restored.insert_chunk(p, c);
    }
    assert_eq!(restored.block(added), Block::Leaves);
}

#[test]
fn historical_v1_stays_byte_identical_through_loaded_idle_f5_and_close() {
    let dir = TestDir::new();
    let hex = "5255535457524c4401000000010000000e000000b2d255a90000000000000000000000000300";
    let bytes: Vec<_> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    fs::write(dir.0.join("world.rcw"), &bytes).unwrap();
    let mut app = app_at(&dir);
    frame(&mut app, Duration::from_secs(120));
    f5(&mut app);
    close(&mut app);
    assert!(app.should_exit().is_some());
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), bytes);
    assert!(!dir.0.join("world.rcw.bak").exists());
    assert!(app.world().resource::<VoxelWorld>().0.identity().is_none());
}

#[test]
fn inventory_only_autosave_and_final_close_preserve_selection_and_held_stacks() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inv = store.load_player(&mut world).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inv);
    let mut app = app_from_session(world, session, inv);
    let old_world = fs::read(dir.0.join("world.rcw")).unwrap();
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .contents
        .select(8);
    frame(&mut app, Duration::from_secs(31));
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), old_world);
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .contents
        .click(0, voxel_core::inventory::Click::Right);
    close(&mut app);
    drain(&mut app);
    assert!(app.should_exit().is_some());
    let expected = app
        .world()
        .resource::<crate::inventory_ui::GameInventory>()
        .contents
        .clone();
    drop(app);
    let (s, mut w, _) = SaveStore::open(&dir.0).unwrap();
    let got = s.load_player(&mut w).unwrap();
    assert_eq!(got.selected(), 8);
    assert_eq!(got.cursor(), expected.cursor());
    assert_eq!(got.slots(), expected.slots());
}
fn app_from_session(
    world: World,
    session: SaveSession,
    inv: voxel_core::inventory::Inventory,
) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(VoxelWorld(world))
        .insert_resource(session)
        .insert_resource(crate::inventory_ui::GameInventory::loaded(inv))
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins(PersistencePlugin);
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        PrimaryWindow,
    ));
    frame(&mut app, Duration::ZERO);
    app
}
#[test]
fn a_completed_old_player_snapshot_never_clears_newer_inventory_edits() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inv = store.load_player(&mut world).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inv);
    let mut app = app_from_session(world, session, inv);
    let snapshot = app.world().resource::<VoxelWorld>().0.save_snapshot();
    let revision = snapshot.revision();
    let inv = app
        .world()
        .resource::<crate::inventory_ui::GameInventory>()
        .contents
        .clone();
    let inventory_revision = Some(inv.revision());
    let store = Arc::clone(&app.world().resource::<SaveSession>().store);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        rx.recv().unwrap();
        store.save_player_session(&snapshot, &inv, false)
    });
    app.world_mut().resource_mut::<SaveSession>().job = Some(SaveJob {
        time: Default::default(),
        revision,
        inventory_revision,
        handle,
    });
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .contents
        .select(7);
    frame(&mut app, Duration::ZERO);
    tx.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Dirty");
    f5(&mut app);
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
}
#[test]
fn failed_player_save_retains_items_cancels_close_and_retry_saves_the_pair() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inv = store.load_player(&mut world).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inv);
    let mut app = app_from_session(world, session, inv);
    let expected = app
        .world()
        .resource::<crate::inventory_ui::GameInventory>()
        .contents
        .clone();
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .contents
        .select(4);
    edit(&mut app, [0, 20, 0], Block::Wood);
    fs::create_dir(dir.0.join("player.rcp.bak")).unwrap();
    close(&mut app);
    drain(&mut app);
    assert!(app.should_exit().is_none());
    assert_eq!(
        app.world().resource::<SaveFeedback>().label,
        "ERROR / DIRTY"
    );
    assert_eq!(
        app.world()
            .resource::<crate::inventory_ui::GameInventory>()
            .contents
            .slots(),
        expected.slots()
    );
    assert!(dir.0.join("session.pending").exists());
    fs::remove_dir(dir.0.join("player.rcp.bak")).unwrap();
    f5(&mut app);
    drain(&mut app);
    assert_eq!(app.world().resource::<SaveFeedback>().label, "Saved");
    assert!(!dir.0.join("session.pending").exists());
}

#[test]
fn return_to_title_waits_for_inflight_save_and_newer_inventory_revision() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inventory = store.load_player(&mut world).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inventory);
    let mut app = app_from_session(world, session, inventory);
    drain(&mut app);
    let snapshot = app.world().resource::<VoxelWorld>().0.save_snapshot();
    let revision = snapshot.revision();
    let inv = app
        .world()
        .resource::<crate::inventory_ui::GameInventory>()
        .contents
        .clone();
    let inventory_revision = Some(inv.revision());
    let store = Arc::clone(&app.world().resource::<SaveSession>().store);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        rx.recv().unwrap();
        store.save_player_session(&snapshot, &inv, false)
    });
    app.world_mut().resource_mut::<SaveSession>().job = Some(SaveJob {
        time: Default::default(),
        revision,
        inventory_revision,
        handle,
    });
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .contents
        .select(8);
    let mut flow = crate::session::ClientFlow::default();
    flow.save(crate::session::Destination::Title);
    app.insert_resource(flow);
    frame(&mut app, Duration::ZERO);
    assert!(
        !app.world()
            .resource::<crate::session::ClientFlow>()
            .save_complete
    );
    assert!(app.should_exit().is_none());
    tx.send(()).unwrap();
    drain(&mut app);
    assert!(
        app.world()
            .resource::<crate::session::ClientFlow>()
            .save_complete
    );
    let session = app.world().resource::<SaveSession>();
    let mut saved = decode(&fs::read(dir.0.join("world.rcw")).unwrap()).unwrap();
    assert_eq!(session.store.load_player(&mut saved).unwrap().selected(), 8);
}

#[test]
fn time_only_changes_use_autosave_f5_and_final_save_without_rotating_world_files() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inv = store.load_player(&mut world).unwrap();
    let root = fs::read(store.path()).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inv);
    let mut app = app_from_session(world, session, inv);
    for (steps, mode) in [(19, 0), (37, 1), (71, 2)] {
        app.world_mut()
            .resource_mut::<VoxelWorld>()
            .0
            .time
            .advance(steps);
        let expected = app.world().resource::<VoxelWorld>().0.time;
        match mode {
            0 => frame(&mut app, Duration::from_secs(31)),
            1 => f5(&mut app),
            _ => close(&mut app),
        }
        drain(&mut app);
        assert_eq!(app.world().resource::<SaveSession>().saved_time, expected);
        assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), root);
        assert!(!dir.0.join("world.rcw.bak").exists());
    }
    let expected = app.world().resource::<VoxelWorld>().0.time;
    drop(app);
    let (store, mut world, _) = SaveStore::open(&dir.0).unwrap();
    store.load_player(&mut world).unwrap();
    assert_eq!(world.time, expected);
}

#[test]
fn final_save_includes_clock_ticks_after_an_inflight_snapshot_without_any_item_or_block_edit() {
    let dir = TestDir::new();
    let (store, mut world, source) = SaveStore::open(&dir.0).unwrap();
    let inventory = store.load_player(&mut world).unwrap();
    let session = SaveSession::with_player(store, &world, source, &inventory);
    let mut app = app_from_session(world, session, inventory);
    drain(&mut app);
    let snapshot = app.world().resource::<VoxelWorld>().0.save_snapshot();
    let time = snapshot.time;
    let revision = snapshot.revision();
    let inv = app
        .world()
        .resource::<crate::inventory_ui::GameInventory>()
        .contents
        .clone();
    let inventory_revision = Some(inv.revision());
    let store = Arc::clone(&app.world().resource::<SaveSession>().store);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        rx.recv().unwrap();
        store.save_player_session(&snapshot, &inv, false)
    });
    app.world_mut().resource_mut::<SaveSession>().job = Some(SaveJob {
        time,
        revision,
        inventory_revision,
        handle,
    });
    app.world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .time
        .advance(199);
    let expected = app.world().resource::<VoxelWorld>().0.time;
    close(&mut app);
    assert!(app.should_exit().is_none());
    tx.send(()).unwrap();
    drain(&mut app);
    assert!(app.should_exit().is_some());
    assert_eq!(app.world().resource::<SaveSession>().saved_time, expected);
    drop(app);
    let (store, mut restored, _) = SaveStore::open(&dir.0).unwrap();
    store.load_player(&mut restored).unwrap();
    assert_eq!(restored.time, expected);
}
