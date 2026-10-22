use super::*;
use crate::{
    camera::FirstPersonPlugin,
    player::{PlayerPlugin, SPAWN},
    scene::VoxelScenePlugin,
};
use bevy::{input::mouse::AccumulatedMouseMotion, time::TimeUpdateStrategy};
use std::time::Duration;
use voxel_core::{Block, BlockPos};

fn frame(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app
        .world()
        .resource::<crate::scene::LegacyMeshing>()
        .pending()
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
        app.update();
    }
}
fn click(app: &mut App, button: MouseButton) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(button);
    frame(app);
}
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .add_plugins((
            VoxelScenePlugin,
            PlayerPlugin,
            FirstPersonPlugin,
            BlockInteractionPlugin,
        ));
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        bevy::window::CursorOptions::default(),
        PrimaryWindow,
    ));
    frame(&mut app);
    app
}
fn index_count(app: &App) -> usize {
    app.world()
        .resource::<Assets<Mesh>>()
        .iter()
        .map(|(_, mesh)| mesh.indices().map_or(0, |indices| indices.len()))
        .sum()
}
fn aim_block() -> BlockPos {
    BlockPos([0, 5, 7])
}
fn world(app: &App) -> &voxel_core::World {
    &app.world().resource::<VoxelWorld>().0
}
fn prepare_target(app: &mut App) {
    let change = app
        .world_mut()
        .resource_mut::<VoxelWorld>()
        .0
        .set_block(aim_block(), Block::Stone)
        .unwrap()
        .unwrap();
    app.world_mut().resource_mut::<DirtyChunks>().mark(change);
    frame(app);
}
fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
}

#[test]
fn capture_click_does_not_edit_then_repeated_clicks_break_and_place_selected_blocks() {
    let mut app = app();
    prepare_target(&mut app);
    let initial = world(&app).clone();
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app), &initial);
    assert_eq!(
        app.world().resource::<Editor>().target.unwrap().position,
        aim_block()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit3);
    frame(&mut app);
    let indices_before = index_count(&app);
    click(&mut app, MouseButton::Right);
    assert!(index_count(&app) > indices_before);
    let adjacent = BlockPos([0, 5, 8]);
    assert_eq!(world(&app).block(adjacent), Block::Stone);
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app).block(adjacent), Block::Air);
    assert_eq!(index_count(&app), indices_before);
    assert_eq!(world(&app).block(aim_block()), Block::Stone);
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app).block(aim_block()), Block::Air);
    let count = app.world().resource::<Assets<Mesh>>().len();
    for _ in 0..10 {
        frame(&mut app);
    }
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
}

#[test]
fn pending_respawn_suppresses_targeting_and_edits_until_clearance_is_resolved() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    let before = world(&app).clone();
    app.world_mut().resource_mut::<PlayerState>().awaiting_spawn = true;
    click(&mut app, MouseButton::Left);
    click(&mut app, MouseButton::Right);
    assert_eq!(world(&app), &before);
    assert!(app.world().resource::<Editor>().target.is_none());
    app.world_mut().resource_mut::<PlayerState>().awaiting_spawn = false;
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app).block(aim_block()), Block::Air);
}

#[test]
fn focus_release_missing_camera_and_reset_frames_never_edit() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    let initial = world(&app).clone();
    let window = window(&mut app);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    click(&mut app, MouseButton::Left);
    click(&mut app, MouseButton::Right);
    assert_eq!(world(&app), &initial);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    click(&mut app, MouseButton::Right); // Released cursor does not capture on RMB.
    assert_eq!(world(&app), &initial);
    click(&mut app, MouseButton::Left); // Capture only, also on refocus.
    assert_eq!(world(&app), &initial);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app), &initial);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    click(&mut app, MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app), &initial);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<FirstPersonCamera>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(camera)
        .remove::<FirstPersonCamera>();
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app), &initial);
}

#[test]
fn selection_feedback_and_crosshair_target_track_the_current_frame() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    let window = window(&mut app);
    for (key, selected) in [
        (KeyCode::Digit1, BuildBlock::Grass),
        (KeyCode::Digit2, BuildBlock::Dirt),
        (KeyCode::Digit3, BuildBlock::Stone),
        (KeyCode::Digit4, BuildBlock::Wood),
        (KeyCode::Digit5, BuildBlock::Leaves),
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        frame(&mut app);
        assert_eq!(app.world().resource::<Editor>().selected, selected);
        assert!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .title
                .contains(selected.name())
        );
    }
    let highlights: Vec<_> = app
        .world_mut()
        .query_filtered::<&Visibility, With<TargetHighlight>>()
        .iter(app.world())
        .copied()
        .collect();
    assert_eq!(highlights, [Visibility::Inherited]);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    frame(&mut app);
    assert_eq!(app.world().resource::<Editor>().target, None);
}

#[test]
fn editing_from_the_displayed_eye_still_rejects_the_authoritative_player_body() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    // The displayed pose remains at the previous fixed position while the
    // authoritative body has advanced into the prospective placement cell.
    app.world_mut().resource_mut::<PlayerState>().body.position = [0.5, 4.0, 8.5];
    let before = world(&app).clone();
    click(&mut app, MouseButton::Right);
    assert_eq!(
        app.world().resource::<Editor>().rejection,
        Some(EditError::IntersectsPlayer)
    );
    assert_eq!(world(&app), &before);
    assert_ne!(app.world().resource::<PlayerState>().body.position, SPAWN);
}

#[test]
fn empty_clicks_and_simultaneous_buttons_do_not_create_blocks_or_double_edit() {
    let mut app = app();
    click(&mut app, MouseButton::Left);
    // Initial overview ray is above the terraces; no target in five blocks.
    assert_eq!(app.world().resource::<Editor>().target, None);
    let before = world(&app).clone();
    click(&mut app, MouseButton::Right);
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app), &before);
    prepare_target(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    frame(&mut app);
    assert_eq!(world(&app).block(aim_block()), Block::Air);
    assert_eq!(world(&app).block(BlockPos([0, 5, 8])), Block::Air);
}

#[test]
fn breaking_the_floor_in_the_real_input_schedule_changes_the_next_physics_tick() {
    let mut app = app();
    click(&mut app, MouseButton::Left);
    let step = app.world().resource::<Time<Fixed>>().timestep();
    *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
        TimeUpdateStrategy::ManualDuration(step);
    for _ in 0..90 {
        frame(&mut app);
    }
    assert!(app.world().resource::<PlayerState>().body.grounded);
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta
        .y = 1000.0;
    frame(&mut app);
    assert_eq!(
        app.world().resource::<Editor>().target.unwrap().position,
        BlockPos([0, 2, 10])
    );
    click(&mut app, MouseButton::Left);
    assert_eq!(world(&app).block(BlockPos([0, 2, 10])), Block::Air);
    frame(&mut app);
    assert!(!app.world().resource::<PlayerState>().body.grounded);
    assert!(app.world().resource::<PlayerState>().body.velocity[1] < 0.0);
    // Filling the freshly mined cell would now intersect the falling feet.
    click(&mut app, MouseButton::Right);
    assert_eq!(world(&app).block(BlockPos([0, 2, 10])), Block::Air);
    assert_eq!(
        app.world().resource::<Editor>().rejection,
        Some(EditError::IntersectsPlayer)
    );
}

#[test]
fn configured_reach_applies_to_highlight_and_clicks() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    app.world_mut().resource_mut::<Editor>().reach = 1.0;
    let before = world(&app).clone();
    click(&mut app, MouseButton::Left);
    assert_eq!(app.world().resource::<Editor>().target, None);
    assert_eq!(world(&app), &before);
    click(&mut app, MouseButton::Right);
    assert_eq!(world(&app), &before);
}

#[test]
fn wood_and_leaves_select_place_break_and_use_opaque_chunk_faces() {
    let mut app = app();
    prepare_target(&mut app);
    click(&mut app, MouseButton::Left);
    for (key, block) in [
        (KeyCode::Digit4, Block::Wood),
        (KeyCode::Digit5, Block::Leaves),
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        frame(&mut app);
        let before = index_count(&app);
        click(&mut app, MouseButton::Right);
        assert_eq!(world(&app).block(BlockPos([0, 5, 8])), block);
        assert!(index_count(&app) > before);
        click(&mut app, MouseButton::Left);
        assert_eq!(world(&app).block(BlockPos([0, 5, 8])), Block::Air);
        assert_eq!(index_count(&app), before);
    }
}
