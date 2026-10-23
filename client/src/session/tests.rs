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
fn assets(app: &App) -> (usize, usize, usize) {
    (
        app.world().resource::<Assets<Mesh>>().len(),
        app.world().resource::<Assets<StandardMaterial>>().len(),
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
    assert_eq!(camera_count(&mut app), 1);
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
        assert_eq!(camera_count(&mut app), 3); // world, hand/UI overlay, menu
        assert!(SaveStore::open(&dir.0).is_err());
        let pos = BlockPos([1, 31, 1]);
        if index > 0 {
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
        leave(&mut app);
        assert_eq!(camera_count(&mut app), 1);
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
fn inventory_escape_pause_focus_resume_and_click_guard_use_real_schedules() {
    let mut app = app();
    let dir = Directory::new();
    open(&mut app, &dir);
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
    fs::create_dir(dir.0.join("player.rcp.bak")).unwrap();
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
