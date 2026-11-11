use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<PlayerState>()
        .init_resource::<crate::camera::ViewPose>()
        .init_resource::<crate::inventory_ui::GameInventory>()
        .init_resource::<crate::player::ControllerInput>()
        .insert_resource(ClientFlow {
            state: crate::session::ClientState::Playing,
            ..default()
        })
        .insert_resource(VoxelWorld(voxel_core::World::default()))
        .add_plugins(DebugUiPlugin);
    app.world_mut().spawn((
        Window {
            focused: true,
            resolution: (960, 540).into(),
            ..default()
        },
        PrimaryWindow,
    ));
    app.update();
    app
}
fn visible(app: &mut App) -> bool {
    let w = app.world_mut();
    w.query_filtered::<&Camera, With<DebugCamera>>()
        .single(w)
        .unwrap()
        .is_active
}
fn frame(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
}
#[test]
fn f3_edges_toggle_once_without_holding_flicker_or_modal_capture_conflicts() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F3);
    frame(&mut app);
    assert!(visible(&mut app));
    for _ in 0..30 {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        frame(&mut app);
        assert!(visible(&mut app));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::F3);
    frame(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F3);
    frame(&mut app);
    assert!(!visible(&mut app));
    for state in [
        crate::session::ClientState::Title,
        crate::session::ClientState::Worlds,
        crate::session::ClientState::Paused,
        crate::session::ClientState::Settings,
        crate::session::ClientState::Loading,
        crate::session::ClientState::Saving,
        crate::session::ClientState::RecoverableError,
    ] {
        app.world_mut().resource_mut::<ClientFlow>().state = state;
        app.world_mut().resource_mut::<DebugOverlay>().enabled = true;
        frame(&mut app);
        assert!(!visible(&mut app));
    }
    app.world_mut().resource_mut::<ClientFlow>().state = crate::session::ClientState::Playing;
    app.world_mut()
        .resource_mut::<crate::inventory_ui::GameInventory>()
        .open = true;
    frame(&mut app);
    assert!(!visible(&mut app));
    app.world_mut().remove_resource::<VoxelWorld>();
    frame(&mut app);
    assert!(!app.world().resource::<DebugOverlay>().enabled);
    assert!(
        crate::settings::Preferences {
            keys: {
                let mut k = crate::settings::Preferences::default().keys;
                k[0] = KeyCode::F3;
                k
            },
            ..default()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn signed_block_chunk_local_coordinates_and_cardinals_use_world_conventions() {
    for (position, block, chunk, local) in [
        (
            [-0.001, -0.001, -16.001],
            [-1, -1, -17],
            [-1, -2],
            [15, 15, 15],
        ),
        ([-16., 16., 15.999], [-16, 16, 15], [-1, 0], [0, 0, 15]),
        ([16., -16., -32.], [16, -16, -32], [1, -2], [0, 0, 0]),
    ] {
        assert_eq!(coordinates(position), (block, chunk, local));
    }
    for (dir, name, yaw) in [
        ([0., 0., -1.], "NORTH -Z", 0.),
        ([1., 0., 0.], "EAST +X", -90.),
        ([0., 0., 1.], "SOUTH +Z", -180.),
        ([-1., 0., 0.], "WEST -X", 90.),
    ] {
        let (actual, y, pitch) = facing(dir);
        assert_eq!(actual, name);
        assert!((y - yaw).abs() < 1e-8);
        assert_eq!(pitch, 0.);
    }
}
#[test]
fn smoothed_frame_measurements_are_finite_for_startup_stalls_and_bad_samples() {
    let mut d = DebugOverlay::default();
    for dt in [0., -1., f64::NAN, f64::INFINITY] {
        d.sample_frame(dt);
        assert!(d.frame_seconds.is_none());
    }
    for dt in [1. / 144., 1. / 30., 10., f64::MIN_POSITIVE, 0., f64::NAN] {
        d.sample_frame(dt);
        let seconds = d.frame_seconds.unwrap();
        assert!(seconds.is_finite() && seconds > 0.);
        assert!((1. / seconds).is_finite());
    }
}
#[test]
fn retained_assets_bounded_refresh_and_authoritative_far_coordinates() {
    let mut app = app();
    app.world_mut().resource_mut::<PlayerState>().body.position = [16008.5, 12.25, -16000.001];
    app.insert_resource(crate::scene::RenderOrigin([16000., 0., -16016.]));
    app.world_mut().resource_mut::<VoxelWorld>().0.time =
        voxel_core::time::WorldTime::from_parts(57, 71999).unwrap();
    app.world_mut().resource_mut::<DebugOverlay>().enabled = true;
    frame(&mut app);
    assert!(
        app.world()
            .resource::<DebugOverlay>()
            .text
            .contains("DAY: 57 / TIME: 23999 / DAYLIGHT: 0.38")
    );
    assert!(
        app.world()
            .resource::<DebugOverlay>()
            .text
            .contains("XYZ: 16008.500 / 12.250 / -16000.001")
    );
    assert!(
        app.world()
            .resource::<DebugOverlay>()
            .text
            .contains("CHUNK XZ: 1000 / -1001")
    );
    let before = app.world().resource::<DebugOverlay>().text.clone();
    {
        let w = app.world_mut();
        w.query::<&mut Window>()
            .single_mut(w)
            .unwrap()
            .resolution
            .set(1280., 720.);
    }
    app.world_mut().resource_mut::<PlayerState>().body.position[0] += 1.;
    frame(&mut app);
    assert_eq!(before, app.world().resource::<DebugOverlay>().text);
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    let materials = app.world().resource::<Assets<StandardMaterial>>().len();
    for i in 0..1000 {
        app.world_mut().resource_mut::<DebugOverlay>().enabled = i % 3 != 0;
        frame(&mut app);
    }
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
    assert_eq!(
        app.world().resource::<Assets<StandardMaterial>>().len(),
        materials
    );
    for size in [(960, 540), (1280, 720), (1920, 1080)] {
        let w = app.world_mut();
        w.query::<&mut Window>()
            .single_mut(w)
            .unwrap()
            .resolution
            .set(size.0 as f32, size.1 as f32);
        w.resource_mut::<DebugOverlay>().enabled = true;
        frame(&mut app);
        let w = app.world_mut();
        let handle = w
            .query_filtered::<&Mesh3d, With<DebugSurface>>()
            .single(w)
            .unwrap();
        let mesh = w.resource::<Assets<Mesh>>().get(&handle.0).unwrap();
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        assert!(
            positions
                .iter()
                .all(|p| p[0] >= -240. && p[0] <= 240. && p[1] >= -135. && p[1] <= 135.)
        );
    }
}

#[test]
fn session_reset_hides_the_retained_surface_immediately() {
    let mut app = app();
    app.world_mut().resource_mut::<DebugOverlay>().enabled = true;
    frame(&mut app);
    assert!(visible(&mut app));
    reset(app.world_mut());
    assert!(!visible(&mut app));
    assert!(!app.world().resource::<DebugOverlay>().enabled);
    assert!(app.world().resource::<DebugOverlay>().text.is_empty());
}

#[test]
#[ignore = "Explicit CPU-only cost measurement; run with --ignored --nocapture"]
fn overlay_cpu_cost_and_retained_buffer_size() {
    let mut app = app();
    for enabled in [false, true] {
        app.world_mut().resource_mut::<DebugOverlay>().enabled = enabled;
        for _ in 0..10 {
            frame(&mut app);
        }
        let mut batches = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            for _ in 0..500 {
                frame(&mut app);
            }
            batches.push(start.elapsed().as_nanos() / 500);
        }
        batches.sort_unstable();
        println!(
            "DEBUG_CPU enabled={enabled} median_ns_per_app_update={} meshes={} materials={}",
            batches[2],
            app.world().resource::<Assets<Mesh>>().len(),
            app.world().resource::<Assets<StandardMaterial>>().len()
        );
    }
    let w = app.world_mut();
    let handle = w
        .query_filtered::<&Mesh3d, With<DebugSurface>>()
        .single(w)
        .unwrap();
    let mesh = w.resource::<Assets<Mesh>>().get(&handle.0).unwrap();
    println!(
        "DEBUG_GEOMETRY vertices={} indices={} logical_bytes={}",
        mesh.count_vertices(),
        mesh.indices().unwrap().len(),
        mesh.count_vertices() * 40 + mesh.indices().unwrap().len() * 4
    );
}

#[test]
fn mesh_diagnostics_use_only_world_view_and_label_resident_triangles() {
    use crate::scene::{ChunkRender, ChunkRenders};
    use bevy::camera::visibility::VisibleEntities;
    use voxel_core::ChunkPos;
    let mut app = app();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1., 1., 1.));
    let opaque = app.world_mut().spawn_empty().id();
    let water = app.world_mut().spawn_empty().id();
    let plant = app.world_mut().spawn_empty().id();
    let mut renders = ChunkRenders::default();
    for (map, entity) in [
        (&mut renders.0, opaque),
        (&mut renders.1, water),
        (&mut renders.2, plant),
    ] {
        map.insert(
            ChunkPos([0; 3]),
            ChunkRender {
                entity,
                mesh: mesh.clone(),
            },
        );
    }
    app.insert_resource(renders);
    let mut world_view = VisibleEntities::default();
    world_view.push(opaque, std::any::TypeId::of::<Mesh3d>());
    app.world_mut()
        .spawn((crate::camera::FirstPersonCamera, world_view));
    let mut overlay = VisibleEntities::default();
    overlay.push(water, std::any::TypeId::of::<Mesh3d>());
    overlay.push(plant, std::any::TypeId::of::<Mesh3d>());
    app.world_mut().spawn(overlay);
    app.world_mut().resource_mut::<DebugOverlay>().enabled = true;
    frame(&mut app);
    let text = &app.world().resource::<DebugOverlay>().text;
    assert!(text.contains("VIEW MESH O/W/P: 1 / 0 / 0 (PREVIOUS)"));
    assert!(text.contains("RESIDENT TRIS O/W/P: 12 / 12 / 12"));
}
