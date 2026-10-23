use super::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    time::TimeUpdateStrategy,
};
use std::time::Duration;
use voxel_core::{
    inventory::{Inventory, Stack},
    physics::Player,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .insert_resource(crate::persistence::SaveFeedback {
            label: "Loaded".into(),
            closing: false,
        })
        .add_plugins((
            crate::scene::VoxelScenePlugin,
            crate::player::PlayerPlugin,
            crate::camera::FirstPersonPlugin,
            crate::inventory_ui::InventoryPlugin,
            crate::interaction::BlockInteractionPlugin,
            crate::save_hud::SaveHudPlugin,
            ViewmodelPlugin,
        ));
    app.world_mut().spawn((
        Window {
            focused: true,
            resolution: (960, 640).into(),
            ..default()
        },
        bevy::window::CursorOptions::default(),
        bevy::window::PrimaryWindow,
    ));
    frame(&mut app, Duration::ZERO);
    app
}
fn frame(app: &mut App, dt: Duration) {
    *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(dt);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta = Vec2::ZERO;
}
fn capture(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(app, Duration::ZERO);
    frame(app, Duration::from_millis(17));
}
fn selection(app: &App) -> Option<Item> {
    app.world().resource::<Models>().selected
}
fn body(grounded: bool, crouched: bool, vy: f64) -> Player {
    let mut p = Player::new([0.; 3]);
    p.grounded = grounded;
    p.crouched = crouched;
    p.velocity[1] = vy;
    p
}

#[test]
fn cube_faces_reuse_exact_original_uv_orientation_for_all_five_blocks() {
    for block in [
        Block::Grass,
        Block::Dirt,
        Block::Stone,
        Block::Wood,
        Block::Leaves,
    ] {
        let mesh = cube(Some(block));
        let mut chunk = voxel_core::Chunk::default();
        chunk.set(voxel_core::LocalPos::new([0; 3]).unwrap(), block);
        let mut world = voxel_core::World::default();
        world.insert_chunk(voxel_core::ChunkPos([0; 3]), chunk);
        let original = voxel_core::mesh_chunk(&world, voxel_core::ChunkPos([0; 3]));
        let VertexAttributeValues::Float32x3(pos) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        let VertexAttributeValues::Float32x3(normals) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
        else {
            panic!()
        };
        let VertexAttributeValues::Float32x2(uv) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!()
        };
        for i in 0..24 {
            let index = original
                .positions
                .iter()
                .zip(&original.normals)
                .position(|(p, n)| *p == pos[i].map(|v| v + 0.5) && *n == normals[i])
                .unwrap();
            assert_eq!(uv[i], original.uvs[index]);
        }
        assert_eq!(mesh.indices().unwrap().len(), 36);
    }
    assert_eq!(arm_mesh().count_vertices(), 96);
    assert_eq!(arm_mesh().indices().unwrap().len(), 144);
}

#[test]
fn overlay_clears_world_depth_and_ui_is_on_the_same_layer_in_front_of_hand() {
    let mut app = app();
    let w = app.world_mut();
    let mut q = w.query_filtered::<(&Camera, &Camera3d, &RenderLayers), With<OverlayCamera>>();
    let (camera, c3d, layers) = q.single(w).unwrap();
    assert_eq!(camera.order, 1);
    assert!(matches!(camera.clear_color, ClearColorConfig::None));
    assert!(matches!(c3d.depth_load_op, Camera3dDepthLoadOp::Clear(0.)));
    assert_eq!(*layers, RenderLayers::layer(OVERLAY_LAYER));
    let mut q = w.query::<(&Mesh3d, Option<&RenderLayers>)>();
    let count = q
        .iter(w)
        .filter(|(_, l)| l.is_some_and(|l| *l == RenderLayers::layer(OVERLAY_LAYER)))
        .count();
    assert_eq!(count, 9); // arm + block + 4 inventory + 2 crosshair + save HUD
}

#[test]
fn selection_tracks_numbers_scroll_loaded_inventory_transfers_and_stack_depletion() {
    let mut app = app();
    capture(&mut app);
    for (key, item) in [
        (KeyCode::Digit1, Item::Grass),
        (KeyCode::Digit2, Item::Dirt),
        (KeyCode::Digit3, Item::Stone),
        (KeyCode::Digit4, Item::Wood),
        (KeyCode::Digit5, Item::Leaves),
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        frame(&mut app, Duration::ZERO);
        assert_eq!(
            selection(&app),
            Some(item),
            "slot {:?}, active {}, paused {}",
            app.world()
                .resource::<GameInventory>()
                .contents
                .selected_stack(),
            app.world().resource::<ControllerInput>().active,
            app.world().resource::<GameInventory>().open
        );
        let models = app.world().resource::<Models>();
        assert_eq!(
            app.world().get::<Mesh3d>(models.block).unwrap().0,
            models.blocks[usize::from(item.id() - 1)]
        );
    }
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = -1.;
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), None);
    let mut slots = [None; 36];
    slots[8] = Stack::new(Item::Wood, 64);
    app.insert_resource(GameInventory::loaded(
        Inventory::from_parts(slots, None, 8).unwrap(),
    ));
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), Some(Item::Wood));
    app.world_mut()
        .resource_mut::<GameInventory>()
        .contents
        .click(8, voxel_core::inventory::Click::Quick);
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), None);
    slots = [None; 36];
    slots[8] = Stack::new(Item::Stone, 1);
    let inventory = Inventory::from_parts(slots, None, 8).unwrap();
    app.insert_resource(GameInventory::loaded(inventory));
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), Some(Item::Stone));
    app.world_mut().resource_mut::<GameInventory>().contents = Inventory::empty();
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), None);
    let models = app.world().resource::<Models>();
    assert_eq!(
        *app.world().get::<Visibility>(models.block).unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        *app.world().get::<Visibility>(models.root).unwrap(),
        Visibility::Inherited
    );
}

#[test]
fn capture_pauses_focus_and_respawn_do_not_queue_click_swings() {
    let mut app = app();
    capture(&mut app);
    assert_eq!(app.world().resource::<HandAction>().serial, 0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(&mut app, Duration::from_millis(17));
    assert_eq!(app.world().resource::<HandAction>().serial, 1);
    assert!(app.world().resource::<Models>().motion.swing_active);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    frame(&mut app, Duration::from_millis(17));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    frame(&mut app, Duration::from_millis(17));
    assert_eq!(app.world().resource::<HandAction>().serial, 1);
    assert!(!app.world().resource::<Models>().motion.swing_active);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyE);
    frame(&mut app, Duration::ZERO);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    frame(&mut app, Duration::ZERO);
    frame(&mut app, Duration::from_millis(17));
    assert_eq!(app.world().resource::<Models>().motion.swing, 0.);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(&mut app, Duration::from_millis(17));
    assert_eq!(app.world().resource::<HandAction>().serial, 1);
    let w = app.world_mut();
    let mut q = w.query::<&mut Window>();
    q.single_mut(w).unwrap().focused = false;
    frame(&mut app, Duration::from_millis(17));
    assert!(!app.world().resource::<Models>().motion.swing_active);
}

#[test]
fn animation_follows_actual_distance_sprint_sneak_air_and_bounded_rapid_actions() {
    let action = HandAction::default();
    let mut values = Vec::new();
    for fps in [30, 60, 144] {
        let mut m = Motion::default();
        m.reset([0.; 3], 0);
        let mut pose = (Vec3::ZERO, Quat::IDENTITY);
        for n in 1..=fps {
            pose = m.step(
                1. / fps as f32,
                [5. * f64::from(n) / f64::from(fps), 0., 0.],
                &body(true, false, 0.),
                &action,
            );
        }
        assert!((m.bob - 1.).abs() < 0.001);
        values.push(pose.0);
    }
    assert!(values[0].distance(values[1]) < 1e-5 && values[1].distance(values[2]) < 1e-5);
    let mut idle = Motion::default();
    let mut fast = Motion::default();
    let mut sneak = Motion::default();
    for n in 1..=60 {
        idle.step(1. / 60., [0.; 3], &body(true, false, 0.), &action);
        fast.step(
            1. / 60.,
            [8. * f64::from(n) / 60., 0., 0.],
            &body(true, false, 0.),
            &action,
        );
        sneak.step(
            1. / 60.,
            [1.8 * f64::from(n) / 60., 0., 0.],
            &body(true, true, 0.),
            &action,
        );
    }
    assert_eq!(idle.bob, 0.);
    assert!(fast.bob > 1.5 && sneak.bob < 0.15 && sneak.crouch > 0.99);
    let mut m = Motion::default();
    let mut action = HandAction::default();
    for n in 0..1000 {
        action.trigger(n % 2 == 0);
        let (offset, rotation) = m.step(1. / 144., [0.; 3], &body(false, false, -6.), &action);
        assert!((0. ..=1.).contains(&m.swing) && offset.length() < 0.4 && rotation.is_finite());
    }
    m.reset([0.; 3], action.serial);
    m.step(1. / 60., [0.; 3], &body(true, false, 0.), &action);
    assert_eq!(m.swing, 0.);
    m.step(1. / 60., [0.; 3], &body(false, false, -6.), &action);
    m.step(1. / 60., [0.; 3], &body(true, false, 0.), &action);
    assert!(m.landing > 0.);
}

#[test]
fn repeated_selection_animation_fov_resize_and_rebase_keep_assets_bounded() {
    let mut app = app();
    capture(&mut app);
    let counts = (
        app.world().resource::<Assets<Mesh>>().len(),
        app.world().resource::<Assets<StandardMaterial>>().len(),
        app.world().resource::<Assets<Image>>().len(),
    );
    let mut bytes = 0;
    let models = app.world().resource::<Models>();
    for h in &models.blocks {
        let m = app.world().resource::<Assets<Mesh>>().get(h).unwrap();
        bytes += m.count_vertices() * 48 + m.indices().unwrap().len() * 4;
    }
    bytes += 96 * 48 + 144 * 4;
    assert_eq!(bytes, 11664);
    for n in 0..600 {
        app.world_mut()
            .resource_mut::<GameInventory>()
            .contents
            .select(n % 9);
        app.world_mut()
            .resource_mut::<HandAction>()
            .trigger(n % 2 == 0);
        if n % 100 == 0 {
            let w = app.world_mut();
            let mut q = w.query_filtered::<&mut Projection, With<FirstPersonCamera>>();
            if let Projection::Perspective(p) = &mut *q.single_mut(w).unwrap() {
                p.fov = (35. + (n % 120) as f32).min(110.).to_radians();
                p.aspect_ratio = if n % 200 == 0 { 0.5 } else { 2.5 };
            }
            w.resource_mut::<PlayerState>().body.position = [16000.5, 32., -16000.5];
            w.resource_mut::<PlayerState>().previous_position = [16000.5, 32., -16000.5];
        }
        frame(&mut app, Duration::ZERO);
        let models = app.world().resource::<Models>();
        let t = app.world().get::<Transform>(models.root).unwrap();
        assert!(t.translation.is_finite() && t.translation.length() < 10.);
    }
    assert_eq!(
        counts,
        (
            app.world().resource::<Assets<Mesh>>().len(),
            app.world().resource::<Assets<StandardMaterial>>().len(),
            app.world().resource::<Assets<Image>>().len()
        )
    );
}
