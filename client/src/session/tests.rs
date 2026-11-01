use super::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    time::TimeUpdateStrategy,
};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use voxel_core::{Block, BlockPos, inventory::Click, persistence::SaveStore};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rustcraft-menu-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
        .init_resource::<ClientFlow>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            17,
        )))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .add_plugins((
            scene::VoxelScenePlugin,
            player::PlayerPlugin,
            camera::FirstPersonPlugin,
            inventory_ui::InventoryPlugin,
            interaction::BlockInteractionPlugin,
            persistence::PersistencePlugin,
            crate::save_hud::SaveHudPlugin,
            streaming::StreamingPlugin,
            viewmodel::ViewmodelPlugin,
            SessionPlugin,
            crate::menu::MenuPlugin,
            crate::debug_ui::DebugUiPlugin,
        ));
    app.world_mut().spawn((
        Window {
            focused: true,
            resolution: (1280, 720).into(),
            ..default()
        },
        PrimaryWindow,
        CursorOptions::default(),
    ));
    frame(&mut app);
    app
}
fn frame(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}
fn state(app: &App) -> ClientState {
    app.world().resource::<ClientFlow>().state
}
fn key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    frame(app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
    frame(app);
}
fn until(app: &mut App, predicate: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !predicate(app) {
        assert!(
            Instant::now() < deadline,
            "lifecycle timed out in {:?}",
            state(app)
        );
        frame(app);
        std::thread::yield_now();
    }
}
fn open(app: &mut App, dir: &Directory) {
    let path = dir.0.clone();
    app.world_mut()
        .insert_resource(Loading(thread::spawn(move || load(path))));
    app.world_mut().resource_mut::<ClientFlow>().state = ClientState::Loading;
    until(app, |a| state(a) != ClientState::Loading);
    assert_eq!(state(app), ClientState::Playing);
    frame(app);
}
fn leave(app: &mut App) {
    app.world_mut()
        .resource_mut::<ClientFlow>()
        .save(Destination::Title);
    until(app, |a| state(a) == ClientState::Title);
}
fn assets(app: &App) -> (usize, usize, usize, usize) {
    (
        app.world().resource::<Assets<Mesh>>().len(),
        app.world().resource::<Assets<StandardMaterial>>().len(),
        app.world()
            .resource::<Assets<crate::terrain_material::TerrainSurface>>()
            .len(),
        app.world().resource::<Assets<Image>>().len(),
    )
}
fn camera_count(app: &mut App) -> usize {
    app.world_mut().query::<&Camera>().iter(app.world()).count()
}
fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
}
#[test]
fn title_first_has_no_world_lock_workers_or_gameplay_camera() {
    let mut app = app();
    let dir = Directory::new();
    for _ in 0..5 {
        frame(&mut app);
    }
    assert_eq!(state(&app), ClientState::Title);
    assert!(!has_world(app.world()));
    assert!(!app.world().contains_resource::<persistence::SaveSession>());
    assert!(!app.world().contains_resource::<Loading>());
    assert_eq!(camera_count(&mut app), 2);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    let (store, _, _) = SaveStore::open(&dir.0).unwrap();
    drop(store);
}
#[test]
fn repeated_sessions_save_edits_inventory_release_lock_and_reclaim_assets() {
    let mut app = app();
    let dir = Directory::new();
    let baseline = assets(&app);
    for index in 0..3 {
        open(&mut app, &dir);
        assert_eq!(camera_count(&mut app), 4); // world, hand/UI, menu, debug
        assert!(SaveStore::open(&dir.0).is_err());
        let pos = BlockPos([1, 31, 1]);
        if index > 0 {
            let entity = window(&mut app);
            assert!(app.world().resource::<inventory_ui::GameInventory>().open);
            assert_eq!(
                app.world().get::<CursorOptions>(entity).unwrap().grab_mode,
                CursorGrabMode::None
            );
            assert!(app.world().get::<CursorOptions>(entity).unwrap().visible);

            assert_eq!(
                app.world().resource::<scene::VoxelWorld>().0.block(pos),
                Block::Wood
            );
            assert_eq!(
                app.world()
                    .resource::<inventory_ui::GameInventory>()
                    .contents
                    .selected(),
                index + 3
            );
        }
        app.world_mut()
            .resource_mut::<scene::VoxelWorld>()
            .0
            .set_block(pos, Block::Wood)
            .unwrap();
        let mut inv = app
            .world_mut()
            .resource_mut::<inventory_ui::GameInventory>();
        inv.contents.select(index + 4);
        if index == 0 {
            inv.contents.click(0, Click::Right);
        }
        let slots = inv.contents.slots().to_owned();
        let held = inv.contents.cursor();
        key(&mut app, KeyCode::F3);
        leave(&mut app);
        assert!(
            !app.world()
                .resource::<crate::debug_ui::DebugOverlay>()
                .enabled
        );
        assert_eq!(camera_count(&mut app), 2);
        assert_eq!(assets(&app), baseline);
        assert!(!app.world().contains_resource::<persistence::SaveSession>());
        assert!(!app.world().contains_resource::<streaming::StreamState>());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<SessionEntity>>()
                .iter(app.world())
                .count(),
            0
        );
        let (store, mut world, _) = SaveStore::open(&dir.0).unwrap();
        let inventory = store.load_player(&mut world).unwrap();
        assert_eq!(inventory.slots(), &slots);
        assert_eq!(inventory.cursor(), held);
        drop(store);
    }
}

#[test]
fn dropped_items_survive_session_switches_without_leaking_entities_or_assets() {
    let mut app = app();
    let dir = Directory::new();
    let initial = assets(&app);
    for _ in 0..3 {
        open(&mut app, &dir);
        // Initial terrain may legitimately add meshes during arbitrary frames.
        // Compare item asset reuse only after its independent queue converges.
        until(&mut app, |a| {
            a.world().resource::<streaming::StreamState>().is_idle()
        });
        let p = BlockPos([8, 30, 8]);
        {
            let mut w = app.world_mut().resource_mut::<scene::VoxelWorld>();
            w.0.set_block(BlockPos([8, 29, 8]), Block::Stone).unwrap();
            w.0.set_block(p, Block::Torch).unwrap();
            w.0.set_block(p, Block::WaterSource).unwrap();
        }
        frame(&mut app);
        assert_eq!(
            app.world()
                .resource::<scene::VoxelWorld>()
                .0
                .dropped_items()
                .len(),
            1
        );
        let during = assets(&app);
        for _ in 0..10 {
            frame(&mut app);
        }
        assert_eq!(assets(&app), during);
        leave(&mut app);
        assert_eq!(assets(&app), initial);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<SessionEntity>>()
                .iter(app.world())
                .count(),
            0
        );
    }
    open(&mut app, &dir);
    let drops: Vec<_> = app
        .world()
        .resource::<scene::VoxelWorld>()
        .0
        .dropped_items()
        .iter()
        .collect();
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].1.count(), 3);
    leave(&mut app);
    assert_eq!(assets(&app), initial);
}
#[test]
fn inventory_escape_pause_focus_resume_and_click_guard_use_real_schedules() {
    let mut app = app();
    let dir = Directory::new();
    open(&mut app, &dir);
    key(&mut app, KeyCode::F3);
    assert!(
        app.world()
            .resource::<crate::debug_ui::DebugOverlay>()
            .enabled
    );
    assert!(app.world().resource::<player::ControllerInput>().active);
    assert!(
        app.world()
            .resource::<player::ControllerInput>()
            .simulation_active
    );
    let entity = window(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    key(&mut app, KeyCode::KeyE);
    assert!(app.world().resource::<inventory_ui::GameInventory>().open);
    key(&mut app, KeyCode::Escape);
    assert!(!app.world().resource::<inventory_ui::GameInventory>().open);
    assert_eq!(state(&app), ClientState::Playing);
    key(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), ClientState::Paused);
    let body = app.world().resource::<player::PlayerState>().body;
    let revision = app.world().resource::<scene::VoxelWorld>().0.revision();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    for _ in 0..5 {
        frame(&mut app);
    }
    assert_eq!(app.world().resource::<player::PlayerState>().body, body);
    assert_eq!(
        app.world().resource::<scene::VoxelWorld>().0.revision(),
        revision
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    frame(&mut app);
    // Enter confirms Fortsetzen on release, without replaying the menu click.
    key(&mut app, KeyCode::Enter);
    assert_eq!(state(&app), ClientState::Playing);
    assert!(
        app.world()
            .resource::<player::ControllerInput>()
            .captured_this_frame
    );
    assert_eq!(
        app.world().resource::<scene::VoxelWorld>().0.revision(),
        revision
    );
    let entity = window(&mut app);
    assert_eq!(
        app.world().get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    app.world_mut().get_mut::<Window>(entity).unwrap().focused = false;
    frame(&mut app);
    assert_eq!(state(&app), ClientState::Paused);
    assert!(!app.world().resource::<player::ControllerInput>().active);
    assert_eq!(
        app.world().get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::None
    );
    app.world_mut().get_mut::<Window>(entity).unwrap().focused = true;
    frame(&mut app);
    assert_eq!(state(&app), ClientState::Paused);
    key(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), ClientState::Playing);
    leave(&mut app);
}
#[test]
fn save_failure_retains_session_lock_and_changes_until_explicit_retry() {
    let mut app = app();
    let dir = Directory::new();
    open(&mut app, &dir);
    // Finish initialization before making the next real player replacement fail.
    until(&mut app, |a| {
        a.world().resource::<persistence::SaveFeedback>().label == "Saved"
    });
    app.world_mut()
        .resource_mut::<inventory_ui::GameInventory>()
        .contents
        .select(8);
    // Clock-only autosaves can already have produced a previous generation.
    let backup = dir.0.join("player.rcp.bak");
    if backup.exists() {
        fs::remove_file(&backup).unwrap();
    }
    fs::create_dir(&backup).unwrap();
    app.world_mut()
        .resource_mut::<ClientFlow>()
        .save(Destination::Title);
    until(&mut app, |a| state(a) == ClientState::RecoverableError);
    assert!(has_world(app.world()));
    assert!(SaveStore::open(&dir.0).is_err());
    assert_eq!(
        app.world()
            .resource::<inventory_ui::GameInventory>()
            .contents
            .selected(),
        8
    );
    assert!(!app.world().resource::<ClientFlow>().error.is_empty());
    fs::remove_dir(dir.0.join("player.rcp.bak")).unwrap();
    key(&mut app, KeyCode::Enter); // Erneut speichern
    until(&mut app, |a| state(a) == ClientState::Title);
    open(&mut app, &dir);
    assert_eq!(
        app.world()
            .resource::<inventory_ui::GameInventory>()
            .contents
            .selected(),
        8
    );
    leave(&mut app);
}
#[test]
fn corrupt_load_is_recoverable_and_never_creates_replacement_files() {
    let mut app = app();
    let dir = Directory::new();
    let bytes = b"broken world -- preserve exactly";
    fs::write(dir.0.join("world.rcw"), bytes).unwrap();
    let path = dir.0.clone();
    app.world_mut()
        .insert_resource(Loading(thread::spawn(move || load(path))));
    app.world_mut().resource_mut::<ClientFlow>().state = ClientState::Loading;
    until(&mut app, |a| state(a) == ClientState::RecoverableError);
    assert!(!has_world(app.world()));
    assert_eq!(fs::read(dir.0.join("world.rcw")).unwrap(), bytes);
    assert!(!dir.0.join("player.rcp").exists());
    assert!(!dir.0.join("session.pending").exists());
    // Keyboard navigation returns to title; no restart required.
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::Enter);
    assert_eq!(state(&app), ClientState::Title);
}

#[test]
fn close_during_loading_waits_for_load_and_final_save_before_releasing_lock() {
    let mut app = app();
    let dir = Directory::new();
    let path = dir.0.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.world_mut()
        .insert_resource(Loading(thread::spawn(move || {
            rx.recv().unwrap();
            load(path)
        })));
    app.world_mut().resource_mut::<ClientFlow>().state = ClientState::Loading;
    let window = window(&mut app);
    app.world_mut()
        .write_message(WindowCloseRequested { window });
    frame(&mut app);
    assert!(app.should_exit().is_none());
    assert_eq!(state(&app), ClientState::Loading);
    tx.send(()).unwrap();
    until(&mut app, |a| a.should_exit().is_some());
    assert!(!has_world(app.world()));
    let (store, mut world, _) = SaveStore::open(&dir.0).unwrap();
    assert!(store.load_player(&mut world).is_ok());
}

#[test]
fn managed_world_a_b_a_without_restart_preserves_all_state_and_reclaims_assets() {
    use voxel_core::persistence::catalog::WorldCatalog;
    let dir = Directory::new();
    let catalog = WorldCatalog::new(dir.0.clone()).unwrap();
    let a = catalog.create("A", 42).unwrap();
    let b = catalog.create("B", 43).unwrap();
    let mut app = app();
    let baseline = assets(&app);
    let pos = BlockPos([1, 31, 1]);
    for (round, id) in [&a, &b, &a, &b, &a, &b].into_iter().enumerate() {
        let c = catalog.clone();
        let selected = id.clone();
        app.world_mut()
            .insert_resource(Loading(thread::spawn(move || {
                prepare(c.open(&selected).map_err(|e| e.to_string())?)
            })));
        app.world_mut().resource_mut::<ClientFlow>().state = ClientState::Loading;
        until(&mut app, |a| state(a) != ClientState::Loading);
        assert_eq!(state(&app), ClientState::Playing);
        frame(&mut app);
        let is_a = id == &a;
        let block = if is_a { Block::Wood } else { Block::Leaves };
        let slot = if is_a { 7 } else { 8 };
        assert_eq!(
            app.world()
                .resource::<scene::VoxelWorld>()
                .0
                .identity()
                .unwrap()
                .seed,
            if is_a { 42 } else { 43 }
        );
        if round >= 2 {
            assert_eq!(
                app.world().resource::<scene::VoxelWorld>().0.block(pos),
                block
            );
            let inv = &app
                .world()
                .resource::<inventory_ui::GameInventory>()
                .contents;
            assert_eq!(inv.selected(), slot);
            assert_eq!(inv.cursor().unwrap().count(), if is_a { 32 } else { 64 });
            assert!(app.world().resource::<inventory_ui::GameInventory>().open);
        } else {
            assert_eq!(
                app.world()
                    .resource::<inventory_ui::GameInventory>()
                    .contents
                    .selected(),
                0
            );
            assert!(
                app.world()
                    .resource::<inventory_ui::GameInventory>()
                    .contents
                    .cursor()
                    .is_none()
            );
            app.world_mut()
                .resource_mut::<scene::VoxelWorld>()
                .0
                .set_block(pos, block)
                .unwrap();
            let mut inv = app
                .world_mut()
                .resource_mut::<inventory_ui::GameInventory>();
            inv.contents.select(slot);
            inv.contents
                .click(0, if is_a { Click::Right } else { Click::Left });
        }
        assert!(catalog.open(id).is_err());
        assert!(catalog.archive(id).is_err());
        leave(&mut app);
        assert_eq!(assets(&app), baseline);
        assert_eq!(camera_count(&mut app), 2);
        assert!(!has_world(app.world()));
        assert!(catalog.open(id).is_ok());
    }
}

#[test]
fn user_settings_survive_world_switches_pause_focus_and_new_session_cameras() {
    let mut app = app();
    let a = Directory::new();
    let b = Directory::new();
    let baseline = assets(&app);
    let mut preferences = crate::settings::Preferences {
        fov: 78,
        sensitivity: 123,
        radius: 1,
        ..default()
    };
    preferences.keys[0] = KeyCode::KeyZ;
    preferences.keys[8] = KeyCode::KeyI;
    preferences.keys[9] = KeyCode::KeyP;
    preferences.keys[10] = KeyCode::KeyX;
    crate::settings::apply(app.world_mut(), preferences.clone());
    for dir in [&a, &b, &a] {
        open(&mut app, dir);
        let fov = app
            .world_mut()
            .query_filtered::<&Projection, With<camera::FirstPersonCamera>>()
            .single(app.world())
            .unwrap();
        assert!(matches!(fov,Projection::Perspective(p) if p.fov==78_f32.to_radians()));
        assert_eq!(
            app.world().resource::<controls::MovementBindings>().forward,
            KeyCode::KeyZ
        );
        key(&mut app, KeyCode::KeyX);
        assert_eq!(
            app.world()
                .resource::<inventory_ui::GameInventory>()
                .contents
                .selected(),
            0
        );
        key(&mut app, KeyCode::KeyI);
        assert!(app.world().resource::<inventory_ui::GameInventory>().open);
        key(&mut app, KeyCode::KeyI);
        assert!(!app.world().resource::<inventory_ui::GameInventory>().open);
        key(&mut app, KeyCode::KeyP);
        assert_eq!(state(&app), ClientState::Paused);
        key(&mut app, KeyCode::KeyP);
        assert_eq!(state(&app), ClientState::Playing);
        let id = window(&mut app);
        app.world_mut().get_mut::<Window>(id).unwrap().focused = false;
        frame(&mut app);
        assert_eq!(state(&app), ClientState::Paused);
        app.world_mut().get_mut::<Window>(id).unwrap().focused = true;
        frame(&mut app);
        assert_eq!(state(&app), ClientState::Paused);
        assert_eq!(
            *app.world().resource::<crate::settings::Preferences>(),
            preferences
        );
        leave(&mut app);
        assert_eq!(assets(&app), baseline);
    }
}

#[test]
fn atmosphere_is_world_only_and_reclaimed_across_session_and_settings_transitions() {
    use crate::atmosphere::{AtmosphereUniform, Sky, SkyAssets, SkyMaterial};
    use bevy::camera::visibility::RenderLayers;
    let mut app = app();
    let dir = Directory::new();
    let baseline = assets(&app);
    assert_eq!(app.world().resource::<Assets<SkyMaterial>>().len(), 0);
    for radius in [1, 3, 2] {
        open(&mut app, &dir);
        assert_eq!(app.world().resource::<Assets<SkyMaterial>>().len(), 1);
        let sky = app.world().resource::<SkyAssets>();
        assert!(app.world().get::<Sky>(sky.entity).is_some());
        let layers = app
            .world()
            .get::<RenderLayers>(sky.entity)
            .cloned()
            .unwrap_or_default();
        assert!(!layers.intersects(&RenderLayers::layer(crate::viewmodel::OVERLAY_LAYER)));
        assert!(!layers.intersects(&RenderLayers::layer(2)));
        let p = crate::settings::Preferences {
            radius,
            ..default()
        };
        crate::settings::apply(app.world_mut(), p);
        frame(&mut app);
        let handle = &app.world().resource::<scene::TerrainMaterial>().0;
        assert_eq!(
            app.world()
                .resource::<Assets<crate::terrain_material::TerrainSurface>>()
                .get(handle)
                .unwrap()
                .extension
                .atmosphere,
            AtmosphereUniform::at_time(radius, app.world().resource::<scene::VoxelWorld>().0.time)
        );
        key(&mut app, KeyCode::Escape);
        assert_eq!(state(&app), ClientState::Paused);
        assert_eq!(app.world().resource::<Assets<SkyMaterial>>().len(), 1);
        key(&mut app, KeyCode::Escape);
        leave(&mut app);
        assert!(!app.world().contains_resource::<SkyAssets>());
        assert_eq!(app.world().resource::<Assets<SkyMaterial>>().len(), 0);
        assert_eq!(assets(&app), baseline);
    }
}

#[test]
fn airborne_pause_settings_and_inventory_focus_loss_freeze_until_explicit_resume() {
    let mut app = app();
    let dir = Directory::new();
    open(&mut app, &dir);
    app.world_mut()
        .resource_mut::<player::PlayerState>()
        .body
        .position[1] = 24.;
    frame(&mut app);
    key(&mut app, KeyCode::Escape);
    let paused = app.world().resource::<player::PlayerState>().body;
    assert_eq!(state(&app), ClientState::Paused);
    assert!(
        !app.world()
            .resource::<player::ControllerInput>()
            .simulation_active
    );
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::Enter);
    assert_eq!(state(&app), ClientState::Settings);
    for _ in 0..60 {
        frame(&mut app);
    }
    assert_eq!(app.world().resource::<player::PlayerState>().body, paused);
    key(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), ClientState::Paused);
    key(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), ClientState::Playing);
    key(&mut app, KeyCode::KeyE);
    let entity = window(&mut app);
    app.world_mut().get_mut::<Window>(entity).unwrap().focused = false;
    frame(&mut app);
    assert_eq!(state(&app), ClientState::Paused);
    let frozen = app.world().resource::<player::PlayerState>().body;
    for focused in [false, true] {
        app.world_mut().get_mut::<Window>(entity).unwrap().focused = focused;
        for _ in 0..20 {
            frame(&mut app);
        }
        assert_eq!(app.world().resource::<player::PlayerState>().body, frozen);
    }
    key(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), ClientState::Playing);
    assert!(app.world().resource::<inventory_ui::GameInventory>().open);
    frame(&mut app);
    assert!(app.world().resource::<player::PlayerState>().body.position[1] < frozen.position[1]);
    leave(&mut app);
}

// Slice30 integration and repeatable CPU measurement.
#[test]
#[ignore = "CPU-only session measurement; run alone with --ignored --nocapture"]
fn stabilization_world_switch_measurements() {
    let mut app = app();
    let dirs = [Directory::new(), Directory::new()];
    let baseline = assets(&app);
    let mut switches = Vec::new();
    for i in 0..12 {
        let start = std::time::Instant::now();
        open(&mut app, &dirs[i % 2]);
        assert_eq!(
            app.world()
                .resource::<scene::VoxelWorld>()
                .0
                .identity()
                .unwrap()
                .generator,
            9
        );
        leave(&mut app);
        switches.push(start.elapsed().as_secs_f64() * 1000.);
        assert_eq!(assets(&app), baseline);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<SessionEntity>>()
                .iter(app.world())
                .count(),
            0
        );
    }
    switches.sort_by(f64::total_cmp);
    println!(
        "STABILIZATION_SWITCH samples={} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} retained_assets={baseline:?}",
        switches.len(),
        switches[5],
        switches[10],
        switches[10]
    );
}
