use super::*;
fn app(origin: ClientState) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ClientFlow {
            state: origin,
            ..default()
        })
        .init_resource::<Editor>()
        .init_resource::<KeyLabels>()
        .init_resource::<Preferences>()
        .init_resource::<Config>()
        .init_resource::<Navigation>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>();
    app.world_mut().spawn((
        Window {
            focused: true,
            resolution: (1280, 720).into(),
            ..default()
        },
        PrimaryWindow,
    ));
    app.update();
    open(app.world_mut());
    app
}
#[test]
fn back_returns_to_origin_and_drafts_are_cancelled() {
    for origin in [ClientState::Title, ClientState::Paused] {
        let mut a = app(origin);
        activate(a.world_mut(), Cmd::Page(Page::Graphics));
        a.world_mut().resource_mut::<Editor>().draft.fov = 78;
        activate(a.world_mut(), Cmd::Back);
        activate(a.world_mut(), Cmd::Back);
        assert_eq!(a.world().resource::<ClientFlow>().state, origin);
        assert_eq!(a.world().resource::<Preferences>().fov, 45);
        open(a.world_mut());
        assert_eq!(a.world().resource::<Editor>().draft.fov, 45);
    }
}
#[test]
fn capture_blocks_conflicts_and_escape_cancels_without_changing_binding() {
    let mut a = app(ClientState::Paused);
    activate(a.world_mut(), Cmd::Bind(0));
    a.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyS);
    input(a.world_mut());
    assert!(a.world().resource::<Editor>().error.contains("Conflict"));
    assert_eq!(a.world().resource::<Editor>().capture, Some(0));
    a.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyZ);
    input(a.world_mut());
    assert_eq!(a.world().resource::<Editor>().draft.keys[0], KeyCode::KeyZ);
    activate(a.world_mut(), Cmd::Bind(1));
    a.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    input(a.world_mut());
    assert_eq!(a.world().resource::<Editor>().capture, None);
    assert_eq!(
        a.world().resource::<ClientFlow>().state,
        ClientState::Settings
    );
    activate(a.world_mut(), Cmd::ResetKeys);
    assert_eq!(
        a.world().resource::<Editor>().draft.keys,
        Preferences::default().keys
    );
}
#[test]
fn sliders_include_exact_78_degrees_and_clamp() {
    let mut e = Editor::default();
    slider(&mut e, Cmd::Fov, 33. / 65.);
    assert_eq!(e.draft.fov, 78);
    slider(&mut e, Cmd::Fov, 2.);
    assert_eq!(e.draft.fov, 110);
    slider(&mut e, Cmd::Sensitivity, -1.);
    assert_eq!(e.draft.sensitivity, 10);
}
#[test]
fn display_preview_restores_all_window_properties_on_cancel_timeout_and_focus_loss() {
    use bevy::window::{PresentMode, WindowMode};
    for cause in 0..3 {
        let mut a = app(ClientState::Title);
        // Capture actual window state, including a manually resized window and
        // a present mode different from the preference adapter's AutoVsync.
        let original_resolution = {
            let mut w = a
                .world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(a.world_mut())
                .unwrap();
            w.resolution.set(1111., 777.);
            w.present_mode = PresentMode::Fifo;
            w.resolution.clone()
        };
        let original_preferences = a.world().resource::<Preferences>().clone();
        {
            let mut e = a.world_mut().resource_mut::<Editor>();
            e.draft.vsync = false;
            if cause == 0 {
                e.draft.size = 0;
            } else {
                e.draft.borderless = true;
            }
        }
        activate(a.world_mut(), Cmd::Apply);
        assert!(a.world().resource::<Editor>().preview.is_some());
        assert!(a.world().resource::<Config>().job.is_none());
        {
            let w = a
                .world_mut()
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .single(a.world())
                .unwrap();
            assert_eq!(w.present_mode, PresentMode::AutoNoVsync);
            if cause == 0 {
                assert_eq!(w.width(), 960.);
            } else {
                assert_ne!(w.mode, WindowMode::Windowed);
            }
        }
        if cause == 0 {
            activate(a.world_mut(), Cmd::Cancel);
        } else if cause == 1 {
            a.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_secs(14));
            input(a.world_mut());
            assert!(a.world().resource::<Editor>().preview.is_some());
            a.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_secs(1));
            input(a.world_mut());
        } else {
            a.world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(a.world_mut())
                .unwrap()
                .focused = false;
            input(a.world_mut());
        }
        assert!(a.world().resource::<Editor>().preview.is_none());
        let w = a
            .world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(a.world())
            .unwrap();
        assert_eq!(w.mode, WindowMode::Windowed);
        assert_eq!(w.resolution, original_resolution);
        assert_eq!(w.present_mode, PresentMode::Fifo);
        assert_eq!(*a.world().resource::<Preferences>(), original_preferences);
        assert!(a.world().resource::<Config>().job.is_none());
    }
}
#[test]
fn confirmed_display_changes_and_vsync_only_persist_before_installing_preferences() {
    use bevy::window::PresentMode;
    for display_change in [true, false] {
        let mut a = app(ClientState::Title);
        let root = std::env::temp_dir().join(format!(
            "rustcraft-vsync-preview-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (store, _) = crate::settings::Store::load(&root);
        a.world_mut().resource_mut::<Config>().store = Some(store);
        let original = a.world().resource::<Preferences>().clone();
        {
            let mut e = a.world_mut().resource_mut::<Editor>();
            e.draft.vsync = false;
            if display_change {
                e.draft.size = 0;
                e.draft.borderless = true;
            }
        }
        let requested = a.world().resource::<Editor>().draft.clone();
        activate(a.world_mut(), Cmd::Apply);
        assert_eq!(*a.world().resource::<Preferences>(), original);
        if display_change {
            assert!(a.world().resource::<Editor>().preview.is_some());
            assert!(!root.join("settings.rcs").exists());
            activate(a.world_mut(), Cmd::Confirm);
        }
        // Both confirmation and VSync-only Apply wait for persistence/polling.
        assert!(a.world().resource::<Editor>().preview.is_none());
        assert!(a.world().resource::<Config>().job.is_some());
        assert_eq!(*a.world().resource::<Preferences>(), original);
        let w = a
            .world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(a.world())
            .unwrap();
        assert_eq!(w.present_mode, PresentMode::Fifo);
        assert_eq!(w.mode, bevy::window::WindowMode::Windowed);
        assert_eq!(w.width(), 1280.);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.world().resource::<Config>().job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            crate::settings::poll(a.world_mut());
            std::thread::yield_now();
        }
        assert_eq!(*a.world().resource::<Preferences>(), requested);
        assert_eq!(crate::settings::Store::load(&root).1.unwrap(), requested);
        let mut expected_window = Window::default();
        requested.window(&mut expected_window);
        let w = a
            .world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(a.world())
            .unwrap();
        assert_eq!(w.present_mode, PresentMode::AutoNoVsync);
        assert_eq!(w.mode, expected_window.mode);
        assert_eq!(w.resolution, expected_window.resolution);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn layouts_remain_inside_shared_logical_viewport() {
    let mut e = Editor::default();
    for page in [
        Page::Home,
        Page::Graphics,
        Page::Input,
        Page::General,
        Page::Keys(0),
        Page::Keys(3),
    ] {
        e.page = page;
        for c in e.controls(&KeyLabels::default()) {
            let [x, y, w, h] = c.rect;
            assert!(x >= -240. && x + w <= 240. && y >= -135. && y + h <= 135.);
        }
    }
    activate(app(ClientState::Title).world_mut(), Cmd::ResetAll);
}

#[test]
fn apply_persists_once_and_close_waits_for_a_running_settings_job() {
    let mut a = app(ClientState::Title);
    a.add_message::<AppExit>();
    let root = std::env::temp_dir().join(format!(
        "rustcraft-settings-menu-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (store, _) = crate::settings::Store::load(&root);
    a.world_mut().resource_mut::<Config>().store = Some(store.clone());
    a.world_mut().resource_mut::<Editor>().draft.fov = 78;
    activate(a.world_mut(), Cmd::Apply);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while a.world().resource::<Config>().job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        crate::settings::poll(a.world_mut());
        std::thread::yield_now();
    }
    assert_eq!(a.world().resource::<Preferences>().fov, 78);
    assert_eq!(crate::settings::Store::load(&root).1.unwrap().fov, 78);
    let (tx, rx) = std::sync::mpsc::channel();
    let p = a.world().resource::<Preferences>().clone();
    a.world_mut().resource_mut::<Config>().job = Some(std::thread::spawn(move || {
        rx.recv().unwrap();
        Ok((store, p))
    }));
    a.world_mut().resource_mut::<ClientFlow>().destination = Some(Destination::Quit);
    input(a.world_mut());
    assert!(a.world().resource::<Messages<AppExit>>().is_empty());
    tx.send(()).unwrap();
    while a.world().resource::<Config>().job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        crate::settings::poll(a.world_mut());
        std::thread::yield_now();
    }
    input(a.world_mut());
    assert!(!a.world().resource::<Messages<AppExit>>().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn discrete_render_slider_snaps_clamps_and_handles_mouse_and_keyboard() {
    let mut a = app(ClientState::Title);
    activate(a.world_mut(), Cmd::Page(Page::Graphics));
    for (fraction, expected) in [
        (-1., 1),
        (0., 1),
        (0.24, 1),
        (0.25, 2),
        (0.5, 2),
        (0.75, 3),
        (1., 3),
        (2., 3),
    ] {
        slider(
            &mut a.world_mut().resource_mut::<Editor>(),
            Cmd::Radius,
            fraction,
        );
        assert_eq!(a.world().resource::<Editor>().draft.radius, expected);
    }
    a.world_mut().resource_mut::<Editor>().focus = 1;
    for (key, expected) in [
        (KeyCode::ArrowLeft, 2),
        (KeyCode::ArrowLeft, 1),
        (KeyCode::ArrowLeft, 1),
        (KeyCode::ArrowRight, 2),
    ] {
        a.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        input(a.world_mut());
        assert_eq!(a.world().resource::<Editor>().draft.radius, expected);
        a.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }
    // Use the same logical rectangle for mouse clicks/dragging and drawing.
    let set_pointer = |world: &mut World, x: f32| {
        let mut w = world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(world)
            .unwrap();
        let p = Vec2::new(
            w.width() / 2. + x * scale(&w),
            w.height() / 2. - 70. * scale(&w),
        );
        w.set_cursor_position(Some(p));
    };
    set_pointer(a.world_mut(), 8.);
    a.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    input(a.world_mut());
    assert_eq!(a.world().resource::<Editor>().draft.radius, 1);
    a.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    set_pointer(a.world_mut(), 103.);
    input(a.world_mut());
    assert_eq!(a.world().resource::<Editor>().draft.radius, 2);
    set_pointer(a.world_mut(), 230.); // Drag beyond the track, then release.
    input(a.world_mut());
    assert_eq!(a.world().resource::<Editor>().draft.radius, 3);
    a.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    input(a.world_mut());
    assert!(a.world().resource::<Editor>().drag.is_none());
    assert_eq!(a.world().resource::<Editor>().draft.radius, 3);
    assert_eq!(a.world().resource::<Preferences>().radius, 2);
}
#[test]
fn render_distance_slider_persists_after_apply() {
    let mut a = app(ClientState::Title);
    let root = std::env::temp_dir().join(format!(
        "rustcraft-radius-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (store, _) = crate::settings::Store::load(&root);
    a.world_mut().resource_mut::<Config>().store = Some(store);
    slider(&mut a.world_mut().resource_mut::<Editor>(), Cmd::Radius, 1.);
    activate(a.world_mut(), Cmd::Apply);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while a.world().resource::<Config>().job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        crate::settings::poll(a.world_mut());
        std::thread::yield_now();
    }
    assert_eq!(a.world().resource::<Preferences>().radius, 3);
    assert_eq!(crate::settings::Store::load(&root).1.unwrap().radius, 3);
    assert!(a.world().resource::<Editor>().preview.is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn english_settings_labels_fit_and_navigation_preserves_confirmation_defaults() {
    let mut e = Editor::default();
    for page in [
        Page::Home,
        Page::Graphics,
        Page::Input,
        Page::General,
        Page::Keys(0),
        Page::Keys(1),
        Page::Keys(2),
        Page::Keys(3),
    ] {
        e.page = page;
        for c in e.controls(&KeyLabels::default()) {
            let size = button_text_size(c.rect, &c.label);
            assert!(size >= 0.75, "label too small: {}", c.label);
            assert!(
                (c.label.to_uppercase().chars().count() as f32 * 6. - 1.) * size
                    <= c.rect[2] - 10.001 + 0.01
            );
        }
    }
}

#[test]
fn os_layout_labels_during_rebinding_keep_saved_physical_keys_functional() {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    for (case, physical, logical) in [
        (0, KeyCode::KeyY, "z"),
        (1, KeyCode::KeyZ, "y"),
        (2, KeyCode::KeyY, "y"),
        (3, KeyCode::KeyZ, "z"),
    ] {
        let mut a = app(ClientState::Title);
        a.add_message::<KeyboardInput>()
            .add_systems(Update, super::super::key_labels::observe);
        let window = a
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(a.world())
            .unwrap();
        activate(a.world_mut(), Cmd::Bind(0));
        a.world_mut().write_message(KeyboardInput {
            key_code: physical,
            logical_key: Key::Character(logical.into()),
            state: ButtonState::Pressed,
            text: Some(logical.into()),
            repeat: false,
            window,
        });
        a.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(physical);
        a.update();
        input(a.world_mut());
        assert_eq!(a.world().resource::<Editor>().capture, None);
        assert_eq!(a.world().resource::<Editor>().draft.keys[0], physical);
        assert_eq!(
            a.world().resource::<KeyLabels>().name(physical),
            logical.to_uppercase()
        );
        let root = std::env::temp_dir().join(format!(
            "rustcraft-layout-{case}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (store, _) = crate::settings::Store::load(&root);
        a.world_mut().resource_mut::<Config>().store = Some(store);
        activate(a.world_mut(), Cmd::Apply);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.world().resource::<Config>().job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            crate::settings::poll(a.world_mut());
            std::thread::yield_now();
        }
        let restored = crate::settings::Store::load(&root).1.unwrap();
        assert_eq!(restored.keys[0], physical);
        assert_eq!(restored.keys[1..], Preferences::default().keys[1..]);
        let bindings = crate::controls::MovementBindings::from_keys(restored.keys);
        let mut keys = ButtonInput::default();
        keys.press(physical);
        assert_ne!(bindings.command(&keys, 0.).direction, [0., 0.]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
