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
    app_with_inventory(Inventory::starter())
}
fn app_with_inventory(inventory: Inventory) -> App {
    let mut app = App::new();
    app.insert_resource(GameInventory::loaded(inventory));
    app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
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
    assert_eq!(arm_mesh().count_vertices(), 24);
    assert_eq!(arm_mesh().indices().unwrap().len(), 36);
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
    assert!((fast.bob - 1.).abs() < 0.001 && sneak.bob == 0. && sneak.crouch > 0.99);
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
    bytes += 24 * 48 + 36 * 4;
    assert_eq!(bytes, 7776);
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
    {
        let w = app.world_mut();
        let mut source =
            w.query_filtered::<(&GlobalTransform, &Projection), With<FirstPersonCamera>>();
        let (camera, projection) = source.single(w).unwrap();
        let camera = *camera;
        let Projection::Perspective(p) = projection else {
            panic!()
        };
        let (fov, aspect) = (p.fov, p.aspect_ratio);
        let mut overlay =
            w.query_filtered::<(&GlobalTransform, &Projection), With<OverlayCamera>>();
        let (global, projection) = overlay.single(w).unwrap();
        assert_eq!(*global, camera);
        assert!(
            matches!(projection, Projection::Perspective(p) if p.fov == fov && p.aspect_ratio == aspect && p.near == 0.01 && p.far == 10.)
        );
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

#[test]
fn actual_place_consumption_and_mining_change_the_visible_model_in_the_same_frame() {
    let mut slots = [None; 36];
    slots[0] = Stack::new(Item::Stone, 1);
    let mut app = app_with_inventory(Inventory::from_parts(slots, None, 0).unwrap());
    capture(&mut app);
    app.world_mut()
        .resource_mut::<crate::scene::VoxelWorld>()
        .0
        .set_block(voxel_core::BlockPos([0, 5, 7]), Block::Stone)
        .unwrap();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    frame(&mut app, Duration::ZERO);
    assert_eq!(
        app.world()
            .resource::<crate::scene::VoxelWorld>()
            .0
            .block(voxel_core::BlockPos([0, 5, 8])),
        Block::Stone
    );
    assert_eq!(selection(&app), None);
    assert!(app.world().resource::<HandAction>().place);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), Some(Item::Stone));
    assert!(!app.world().resource::<HandAction>().place);
}

#[test]
fn saved_inventory_reopens_with_the_correct_selected_model_and_empty_hand() {
    let path = std::env::temp_dir().join(format!(
        "rustcraft-viewmodel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (store, mut world, _) =
        voxel_core::persistence::SaveStore::open_with(&path, || Ok(voxel_core::demo_world()))
            .unwrap();
    store.load_player(&mut world).unwrap();
    let mut slots = [None; 36];
    slots[7] = Stack::new(Item::Leaves, 64);
    let inventory = Inventory::from_parts(slots, None, 7).unwrap();
    store
        .save_player_session(&world, &inventory, false)
        .unwrap();
    drop(store);
    let (store, mut world, _) =
        voxel_core::persistence::SaveStore::open_with(&path, || panic!("existing world must load"))
            .unwrap();
    let restored = store.load_player(&mut world).unwrap();
    assert_eq!(restored.selected(), 7);
    let mut app = app_with_inventory(restored);
    capture(&mut app);
    assert_eq!(selection(&app), Some(Item::Leaves));
    app.world_mut()
        .resource_mut::<GameInventory>()
        .contents
        .select(6);
    frame(&mut app, Duration::ZERO);
    assert_eq!(selection(&app), None);
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn arm_is_exactly_one_textured_wide_cuboid() {
    let mesh = arm_mesh();
    let VertexAttributeValues::Float32x3(p) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        panic!()
    };
    assert_eq!(p.len(), 24);
    assert_eq!(mesh.indices().unwrap().len(), 36);
    for axis in 0..3 {
        let extent = if axis == 1 { 0.375 } else { 0.125 };
        assert!(p.iter().all(|v| v[axis] == -extent || v[axis] == extent));
    }
    let mut corners: Vec<_> = p.iter().map(|v| v.map(f32::to_bits)).collect();
    corners.sort();
    corners.dedup();
    assert_eq!(corners.len(), 8);
    let VertexAttributeValues::Float32x2(uv) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() else {
        panic!()
    };
    assert!(uv.iter().all(|v| v.iter().all(|x| (0. ..=1.).contains(x))));
    let image = arm_image();
    assert_eq!(image.texture_descriptor.size.width, 16);
    assert_eq!(image.texture_descriptor.size.height, 16);
    assert_eq!(image.texture_descriptor.mip_level_count, 1);
    assert_eq!(image.data.as_ref().unwrap().len(), 1024);
    assert!(
        image
            .data
            .as_ref()
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 255)
    );
    let ImageSampler::Descriptor(sampler) = image.sampler else {
        panic!()
    };
    assert_eq!(sampler.mag_filter, bevy::image::ImageFilterMode::Nearest);
    assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Nearest);
    assert_eq!(sampler.lod_max_clamp, 0.);
}

#[test]
fn only_one_model_is_visible_and_its_perspective_survives_custom_fov() {
    let mut app = app();
    capture(&mut app);
    for selected in [true, false] {
        app.world_mut()
            .resource_mut::<GameInventory>()
            .contents
            .select(if selected { 0 } else { 8 });
        let mut reference: Option<Vec<Vec2>> = None;
        for degrees in [35_f32, 45., 70., 110.] {
            let w = app.world_mut();
            let mut q = w.query_filtered::<&mut Projection, With<FirstPersonCamera>>();
            if let Projection::Perspective(p) = &mut *q.single_mut(w).unwrap() {
                p.fov = degrees.to_radians();
                p.aspect_ratio = 16. / 9.;
            }
            frame(&mut app, Duration::ZERO);
            let w = app.world();
            let models = w.resource::<Models>();
            assert_eq!(
                *w.get::<Visibility>(models.root).unwrap(),
                Visibility::Inherited
            );
            assert_eq!(
                *w.get::<Visibility>(models.arm).unwrap(),
                if selected {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                }
            );
            assert_eq!(
                *w.get::<Visibility>(models.block).unwrap(),
                if selected {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            );
            let entity = if selected { models.block } else { models.arm };
            let root = w.get::<Transform>(models.root).unwrap().to_matrix();
            let model = root * w.get::<Transform>(entity).unwrap().to_matrix();
            let mesh = w
                .resource::<Assets<Mesh>>()
                .get(&w.get::<Mesh3d>(entity).unwrap().0)
                .unwrap();
            let VertexAttributeValues::Float32x3(vertices) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!()
            };
            let points: Vec<_> = vertices
                .iter()
                .map(|v| {
                    let p = model.transform_point3(Vec3::from_array(*v));
                    assert!(p.z < -0.1, "geometry must stay behind UI/near plane");
                    Vec2::new(p.x / (16. / 9.), -p.y)
                        / (-2. * p.z * (degrees.to_radians() * 0.5).tan())
                        + Vec2::splat(0.5)
                })
                .collect();
            if let Some(reference) = &reference {
                assert!(
                    points
                        .iter()
                        .zip(reference)
                        .all(|(a, b)| a.distance(*b) < 1e-5)
                );
            } else {
                reference = Some(points);
            }
        }
    }
}

#[test]
fn swing_is_a_timed_pivoted_arc_and_never_replays_hidden_actions() {
    let mut action = HandAction::default();
    action.trigger(false);
    for fps in [10, 30, 60, 144] {
        let mut motion = Motion::default();
        for _ in 0..fps {
            motion.step(1. / fps as f32, [0.; 3], &body(true, false, 0.), &action);
        }
        assert!(!motion.swing_active);
        assert_eq!(motion.swing, 0.);
        motion.reset([0.; 3], action.serial);
        motion.step(1. / fps as f32, [0.; 3], &body(true, false, 0.), &action);
        assert_eq!(motion.swing, 0.);
    }
    for block in [true, false] {
        let mut motion = Motion::default();
        let idle = model_pose(block, &motion);
        motion.swing = 0.25;
        let strike = model_pose(block, &motion);
        motion.swing = 0.75;
        let recovery = model_pose(block, &motion);
        motion.swing = 1.;
        let end = model_pose(block, &motion);
        assert!(strike.rotation.angle_between(idle.rotation) > 0.5);
        assert!(strike.translation.x < idle.translation.x - 0.1);
        assert!(strike.translation.distance(recovery.translation) > 0.1);
        assert!(end.translation.distance(idle.translation) < 1e-5);
        assert!(end.rotation.angle_between(idle.rotation) < 0.001);
        for n in 0..=100 {
            motion.swing = n as f32 / 100.;
            let pose = model_pose(block, &motion);
            assert!(pose.translation.is_finite() && pose.rotation.is_finite());
            assert!(pose.translation.z < -0.3 && pose.translation.length() < 6.);
        }
    }
}

// Hand-measured silhouettes in the unmodified Java1.21.1 wide-Steve reference,
// 854x480/FOV45. These are observations, not Mojang transform/source constants.
fn projected(mesh: &Mesh, pose: Transform, size: Vec2) -> Vec<Vec2> {
    let VertexAttributeValues::Float32x3(vertices) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        panic!()
    };
    vertices
        .iter()
        .map(|p| {
            let v = pose.transform_point(Vec3::from_array(*p));
            assert!(
                v.z < -0.16,
                "UI must stay in front throughout the swing: {v:?}"
            );
            let half = (70_f32.to_radians() * 0.5).tan();
            Vec2::new(v.x / (size.x / size.y), -v.y) / (-2. * v.z * half) * size + size * 0.5
        })
        .collect()
}
#[test]
fn idle_silhouettes_match_measured_wide_arm_and_grass_reference_landmarks() {
    for (block, landmarks) in [
        (false, [[602., 374.], [658., 338.], [737., 383.]]),
        (true, [[559., 392.], [618., 349.], [828., 392.]]),
    ] {
        let mesh = if block {
            cube(Some(Block::Grass))
        } else {
            arm_mesh()
        };
        let pose = model_pose(block, &Motion::default());
        let points = projected(&mesh, pose, Vec2::new(854., 480.));
        for point in landmarks {
            assert!(
                points
                    .iter()
                    .any(|p| p.distance(Vec2::from_array(point)) < 3.),
                "{block}: {point:?} in {points:?}"
            );
        }
        for n in 0..=100 {
            let motion = Motion {
                swing: n as f32 / 100.,
                ..default()
            };
            projected(&mesh, model_pose(block, &motion), Vec2::new(854., 480.));
        }
    }
    let points = projected(
        &arm_mesh(),
        model_pose(false, &Motion::default()),
        Vec2::new(854., 640.),
    );
    assert!(
        points
            .iter()
            .any(|p| p.distance(Vec2::new(736., 450.)) < 4.)
    );
}

#[test]
fn walk_sway_matches_reference_range_and_sprint_increases_rhythm_not_size() {
    let mut walk = Motion::default();
    let mut sprint = Motion::default();
    walk.reset([0.; 3], 0);
    sprint.reset([0.; 3], 0);
    walk.step(
        0.02,
        [0.1, 0., 0.],
        &body(true, false, 0.),
        &HandAction::default(),
    );
    sprint.step(
        0.02,
        [0.16, 0., 0.],
        &body(true, false, 0.),
        &HandAction::default(),
    );
    assert!(sprint.phase > walk.phase);
    assert_eq!(sprint.bob, walk.bob);
    let mut tips = Vec::new();
    for n in 0..128 {
        let mut motion = Motion {
            bob: 1.,
            phase: n as f32 / 128. * std::f32::consts::TAU,
            ..default()
        };
        let (offset, rotation) =
            motion.step(0., [0.; 3], &body(true, false, 0.), &HandAction::default());
        let mut pose = model_pose(false, &motion);
        pose.translation = rotation * pose.translation + offset * MODEL_DEPTH_SCALE;
        pose.rotation = rotation * pose.rotation;
        let p = projected(&arm_mesh(), pose, Vec2::new(854., 480.));
        tips.push(*p.iter().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap());
    }
    let min = tips.iter().copied().reduce(Vec2::min).unwrap();
    let max = tips.iter().copied().reduce(Vec2::max).unwrap();
    assert!(min.distance(Vec2::new(639.6, 336.)) < 4., "{min:?}");
    assert!(max.distance(Vec2::new(674.5, 368.)) < 4., "{max:?}");
}
