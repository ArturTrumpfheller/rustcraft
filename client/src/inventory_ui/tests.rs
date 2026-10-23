use super::*;
use crate::{
    camera::FirstPersonPlugin,
    interaction::BlockInteractionPlugin,
    player::{PlayerPlugin, PlayerState},
    scene::{VoxelScenePlugin, VoxelWorld},
};
use bevy::{input::mouse::AccumulatedMouseMotion, time::TimeUpdateStrategy};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use voxel_core::{Block, BlockPos, World, inventory::Item, persistence::SaveStore};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rustcraft-inventory-ui-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn app() -> (Temp, App) {
    let dir = Temp::new();
    let (store, mut world, source) = SaveStore::open_with(&dir.0, || {
        let mut w = World::default();
        for x in -6..=6 {
            for z in 0..15 {
                w.set_block(BlockPos([x, 3, z]), Block::Grass).unwrap();
            }
        }
        for z in [6, 7] {
            w.set_block(BlockPos([0, 5, z]), Block::Stone).unwrap();
        }
        Ok(w)
    })
    .unwrap();
    let inventory = store.load_player(&mut world).unwrap();
    let session = crate::persistence::SaveSession::with_player(store, &world, source, &inventory);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1. / 60.,
        )))
        .insert_resource(VoxelWorld(world))
        .insert_resource(session)
        .insert_resource(GameInventory::loaded(inventory))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .add_plugins((
            VoxelScenePlugin,
            PlayerPlugin,
            FirstPersonPlugin,
            BlockInteractionPlugin,
            InventoryPlugin,
            crate::persistence::PersistencePlugin,
        ));
    app.world_mut().spawn((
        Window {
            focused: true,
            resolution: (960, 640).into(),
            ..default()
        },
        CursorOptions::default(),
        PrimaryWindow,
    ));
    frame(&mut app);
    (dir, app)
}
fn frame(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta = Vec2::ZERO;
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
fn click(app: &mut App, button: MouseButton) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(button);
    frame(app);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(button);
    frame(app);
}
fn slot(app: &mut App, i: usize, button: MouseButton) {
    {
        let w = app.world_mut();
        let mut q = w.query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut window = q.single_mut(w).unwrap();
        let layout = Layout::new(&window);
        let c = layout.center(i, true);
        window.set_cursor_position(Some(Vec2::new(
            (c.x + layout.half.x) * layout.scale,
            (layout.half.y - c.y) * layout.scale,
        )));
    }
    click(app, button);
}
fn total(app: &App, item: Item) -> u32 {
    let inv = &app.world().resource::<GameInventory>().contents;
    inv.slots()
        .iter()
        .copied()
        .chain([inv.cursor()])
        .flatten()
        .filter(|s| s.item() == item)
        .map(|s| u32::from(s.count()))
        .sum()
}
fn save(app: &mut App) {
    key(app, KeyCode::F5);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        frame(app);
        if app
            .world()
            .resource::<crate::persistence::SaveFeedback>()
            .label
            == "Saved"
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn layout_hit_testing_tracks_every_slot_and_window_scale() {
    for size in [(320, 240), (960, 640), (1920, 1080)] {
        let w = Window {
            resolution: size.into(),
            ..default()
        };
        let l = Layout::new(&w);
        for i in 0..36 {
            let c = l.center(i, true);
            let p = Vec2::new((c.x + l.half.x) * l.scale, (l.half.y - c.y) * l.scale);
            assert_eq!(l.hit(p, true), Some(i));
        }
        assert!(l.hit(Vec2::ZERO, true).is_none());
    }
}
#[test]
fn nine_keys_wheel_wrap_and_empty_selection_do_not_consume_or_build() {
    let (_dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    key(&mut app, KeyCode::Digit9);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        8
    );
    let before = app.world().resource::<VoxelWorld>().0.clone();
    click(&mut app, MouseButton::Right);
    assert_eq!(app.world().resource::<VoxelWorld>().0, before);
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = -1.;
    frame(&mut app);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        0
    );
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = 1.;
    frame(&mut app);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        8
    );
}
#[test]
fn modal_open_pauses_camera_motion_mining_jump_reset_and_escape_closes_first() {
    let (_dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    key(&mut app, KeyCode::KeyE);
    assert!(app.world().resource::<GameInventory>().open);
    let body = app.world().resource::<PlayerState>().body;
    let direction = app.world().resource::<crate::camera::ViewPose>().direction;
    let world = app.world().resource::<VoxelWorld>().0.clone();
    for k in [
        KeyCode::KeyW,
        KeyCode::Space,
        KeyCode::KeyR,
        KeyCode::ControlLeft,
        KeyCode::ShiftLeft,
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(k);
    }
    for _ in 0..120 {
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::splat(80.);
        frame(&mut app);
    }
    click(&mut app, MouseButton::Left);
    assert_eq!(app.world().resource::<PlayerState>().body, body);
    assert_eq!(
        app.world().resource::<crate::camera::ViewPose>().direction,
        direction
    );
    assert_eq!(app.world().resource::<VoxelWorld>().0, world);
    key(&mut app, KeyCode::Escape);
    assert!(!app.world().resource::<GameInventory>().open);
    let w = app.world_mut();
    assert_eq!(
        w.query::<&CursorOptions>().single(w).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    key(&mut app, KeyCode::Escape);
    let w = app.world_mut();
    assert_eq!(
        w.query::<&CursorOptions>().single(w).unwrap().grab_mode,
        CursorGrabMode::None
    );
}
#[test]
fn left_right_quick_transfer_and_outside_clicks_keep_counts_and_cursor() {
    let (_dir, mut app) = app();
    key(&mut app, KeyCode::KeyE);
    slot(&mut app, 0, MouseButton::Right);
    assert_eq!(
        app.world()
            .resource::<GameInventory>()
            .contents
            .cursor()
            .unwrap()
            .count(),
        32
    );
    slot(&mut app, 9, MouseButton::Right);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.slots()[9]
            .unwrap()
            .count(),
        1
    );
    slot(&mut app, 9, MouseButton::Left);
    assert!(
        app.world()
            .resource::<GameInventory>()
            .contents
            .cursor()
            .is_none()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    slot(&mut app, 9, MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftLeft);
    assert!(app.world().resource::<GameInventory>().contents.slots()[9].is_none());
    assert_eq!(total(&app, Item::Grass), 64);
    let inv = app.world().resource::<GameInventory>().contents.clone();
    let w = app.world_mut();
    let mut q = w.query::<&mut Window>();
    q.single_mut(w)
        .unwrap()
        .set_cursor_position(Some(Vec2::ZERO));
    click(&mut app, MouseButton::Right);
    assert_eq!(app.world().resource::<GameInventory>().contents, inv);
}
#[test]
fn actual_mining_and_placement_change_items_once_and_paired_save_restarts() {
    let (dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    key(&mut app, KeyCode::Digit3);
    let start = total(&app, Item::Stone);
    click(&mut app, MouseButton::Left);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([0, 5, 7])),
        Block::Air
    );
    assert_eq!(total(&app, Item::Stone), start + 1);
    click(&mut app, MouseButton::Right);
    assert_eq!(
        app.world()
            .resource::<VoxelWorld>()
            .0
            .block(BlockPos([0, 5, 7])),
        Block::Stone
    );
    assert_eq!(total(&app, Item::Stone), start);
    key(&mut app, KeyCode::Digit8);
    key(&mut app, KeyCode::KeyE);
    slot(&mut app, 0, MouseButton::Right);
    save(&mut app);
    let expected = app.world().resource::<GameInventory>().contents.clone();
    drop(app);
    let (store, mut world, _) = SaveStore::open(&dir.0).unwrap();
    let got = store.load_player(&mut world).unwrap();
    assert_eq!(got.slots(), expected.slots());
    assert_eq!(got.cursor(), expected.cursor());
    assert_eq!(got.selected(), 7);
    assert_eq!(world.block(BlockPos([0, 5, 7])), Block::Stone);
    assert!(GameInventory::loaded(got).open);
}
#[test]
fn retained_ui_reuses_four_meshes_one_atlas_and_two_materials_across_idle_and_modal_cycles() {
    let (_dir, mut app) = app();
    save(&mut app);
    let m = app.world().resource::<Assets<Mesh>>().len();
    let materials = app.world().resource::<Assets<StandardMaterial>>().len();
    let image = app.world().resource::<Assets<Image>>().len();
    let handles = app.world().resource::<UiAssets>().meshes.clone();
    let rebuilds = app.world().resource::<UiAssets>().rebuilds;
    let t = Instant::now();
    for _ in 0..1000 {
        frame(&mut app);
    }
    let idle = t.elapsed();
    assert_eq!(app.world().resource::<UiAssets>().rebuilds, rebuilds);
    let t = Instant::now();
    for _ in 0..30 {
        key(&mut app, KeyCode::KeyE);
        key(&mut app, KeyCode::KeyE);
    }
    let modal = t.elapsed();
    let closed = ui_bytes(&app);
    key(&mut app, KeyCode::KeyE);
    let open = ui_bytes(&app);
    let mut full = Inventory::empty();
    full.insert(Item::Dirt, 36 * 64);
    app.world_mut().resource_mut::<GameInventory>().contents = full;
    frame(&mut app);
    let full_bytes = ui_bytes(&app);
    println!(
        "INVENTORY_UI idle_1000={idle:?} modal_60={modal:?} mesh_assets={m} materials={materials} images={image} closed_bytes={closed} open_bytes={open} full_bytes={full_bytes}"
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), m);
    assert_eq!(
        app.world().resource::<Assets<StandardMaterial>>().len(),
        materials
    );
    assert_eq!(app.world().resource::<Assets<Image>>().len(), image);
    assert_eq!(image, 1);
    assert_eq!(app.world().resource::<UiAssets>().meshes, handles);
}

#[test]
fn modal_transition_clicks_never_mine_or_place_behind_the_inventory() {
    let (_dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    let before = app.world().resource::<VoxelWorld>().0.clone();
    let contents = app.world().resource::<GameInventory>().contents.clone();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(&mut app);
    assert!(app.world().resource::<GameInventory>().open);
    assert_eq!(app.world().resource::<VoxelWorld>().0, before);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyE);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    frame(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    frame(&mut app);
    assert!(!app.world().resource::<GameInventory>().open);
    assert_eq!(app.world().resource::<VoxelWorld>().0, before);
    assert_eq!(app.world().resource::<GameInventory>().contents, contents);
}
#[test]
fn full_inventory_declines_mining_and_reports_it_without_losing_the_block() {
    let (_dir, mut app) = app();
    let mut inv = Inventory::empty();
    assert_eq!(inv.insert(Item::Dirt, 36 * 64), 0);
    app.world_mut().resource_mut::<GameInventory>().contents = inv;
    click(&mut app, MouseButton::Left);
    let before = app.world().resource::<VoxelWorld>().0.clone();
    click(&mut app, MouseButton::Left);
    assert_eq!(app.world().resource::<VoxelWorld>().0, before);
    assert_eq!(total(&app, Item::Dirt), 36 * 64);
    assert_eq!(app.world().resource::<GameInventory>().message, "NO ROOM");
    click(&mut app, MouseButton::Right);
    assert_eq!(total(&app, Item::Dirt), 36 * 64 - 1);
    assert_eq!(app.world().resource::<GameInventory>().message, "");
}
#[test]
fn fractional_pixel_wheel_is_bounded_and_focus_loss_discards_its_remainder() {
    let (_dir, mut app) = app();
    {
        let mut s = app.world_mut().resource_mut::<AccumulatedMouseScroll>();
        s.unit = MouseScrollUnit::Pixel;
        s.delta.y = 20.;
    }
    frame(&mut app);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        0
    );
    {
        let w = app.world_mut();
        w.query::<&mut Window>().single_mut(w).unwrap().focused = false;
    }
    frame(&mut app);
    {
        let w = app.world_mut();
        w.query::<&mut Window>().single_mut(w).unwrap().focused = true;
    }
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = 20.;
    frame(&mut app);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        0
    );
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = 20.;
    frame(&mut app);
    assert_eq!(
        app.world().resource::<GameInventory>().contents.selected(),
        8
    );
}

#[test]
fn moving_a_held_stack_changes_only_its_transform_not_mesh_or_image_assets() {
    let (_dir, mut app) = app();
    key(&mut app, KeyCode::KeyE);
    slot(&mut app, 0, MouseButton::Right);
    let before = app.world().resource::<UiAssets>().rebuilds;
    let count = app.world().resource::<Assets<Mesh>>().len();
    for n in 0..100 {
        let w = app.world_mut();
        w.query::<&mut Window>()
            .single_mut(w)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(200. + n as f32, 200.)));
        frame(&mut app);
    }
    assert_eq!(app.world().resource::<UiAssets>().rebuilds, before);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    let cursor = app.world().resource::<UiAssets>().cursor;
    assert_eq!(
        *app.world().get::<Visibility>(cursor).unwrap(),
        Visibility::Inherited
    );
    {
        let w = app.world_mut();
        w.query::<&mut Window>().single_mut(w).unwrap().focused = false;
    }
    frame(&mut app);
    assert_eq!(
        *app.world().get::<Visibility>(cursor).unwrap(),
        Visibility::Hidden
    );
}

fn ui_bytes(app: &App) -> usize {
    let w = app.world();
    w.resource::<UiAssets>()
        .meshes
        .iter()
        .map(|h| {
            let m = w.resource::<Assets<Mesh>>().get(h).unwrap();
            m.count_vertices()
                * if m.attribute(Mesh::ATTRIBUTE_COLOR).is_some() {
                    40
                } else {
                    32
                }
                + m.indices().map_or(0, |i| i.len() * 4)
        })
        .sum()
}

#[test]
fn repeated_real_input_break_place_cycles_conserve_items_and_reuse_render_assets() {
    let (_dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    key(&mut app, KeyCode::Digit3);
    let items = total(&app, Item::Stone);
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    for _ in 0..64 {
        click(&mut app, MouseButton::Left);
        assert_eq!(total(&app, Item::Stone), items + 1);
        click(&mut app, MouseButton::Right);
        assert_eq!(total(&app, Item::Stone), items);
        assert_eq!(
            app.world()
                .resource::<VoxelWorld>()
                .0
                .block(BlockPos([0, 5, 7])),
            Block::Stone
        );
    }
    save(&mut app);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
}

#[test]
fn sprint_fov_keeps_inventory_meshes_and_assets_retained_and_modal_blocks_held_jump() {
    let (_dir, mut app) = app();
    click(&mut app, MouseButton::Left);
    let rebuilds = app.world().resource::<UiAssets>().rebuilds;
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    let materials = app.world().resource::<Assets<StandardMaterial>>().len();
    let images = app.world().resource::<Assets<Image>>().len();
    let root = app.world().resource::<UiAssets>().root;
    let scale = app.world().get::<Transform>(root).unwrap().scale;
    for k in [KeyCode::KeyD, KeyCode::ControlLeft] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(k);
    }
    for _ in 0..30 {
        frame(&mut app);
    }
    assert_ne!(app.world().get::<Transform>(root).unwrap().scale, scale);
    assert_eq!(app.world().resource::<UiAssets>().rebuilds, rebuilds);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
    assert_eq!(
        app.world().resource::<Assets<StandardMaterial>>().len(),
        materials
    );
    assert_eq!(app.world().resource::<Assets<Image>>().len(), images);
    key(&mut app, KeyCode::KeyE);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    for _ in 0..3 {
        frame(&mut app);
    }
    let before = app.world().resource::<PlayerState>().body;
    key(&mut app, KeyCode::KeyE);
    assert!(
        !app.world()
            .resource::<crate::player::ControllerInput>()
            .command
            .jump
    );
    assert!(
        !app.world()
            .resource::<crate::player::ControllerInput>()
            .jump_pending
    );
    assert_eq!(
        app.world().resource::<PlayerState>().body.position[1],
        before.position[1]
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Space);
    frame(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    frame(&mut app);
    assert!(app.world().resource::<PlayerState>().body.velocity[1] > 0.);
}
