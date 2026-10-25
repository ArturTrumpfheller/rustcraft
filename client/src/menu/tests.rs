use super::*;
fn luminance(c: [f32; 4]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}
#[test]
fn button_states_are_distinct_and_text_has_readable_contrast() {
    let styles = [
        ButtonStyle::new(false, false, false, true),
        ButtonStyle::new(false, true, false, true),
        ButtonStyle::new(true, false, false, true),
        ButtonStyle::new(true, true, true, true),
        ButtonStyle::new(false, false, false, false),
    ];
    for (i, s) in styles.iter().enumerate() {
        assert!((luminance(s.text) + 0.05) / (luminance(s.fill) + 0.05) >= 4.5);
        for old in &styles[..i] {
            assert_ne!(s.fill, old.fill);
            assert_ne!(s.border, old.border);
        }
        assert!(luminance(s.fill) > 0.02); // Stand out from the dark slate background.
    }
}
#[test]
fn english_title_and_pause_navigation_fit_supported_viewports() {
    for state in [
        ClientState::Title,
        ClientState::Paused,
        ClientState::RecoverableError,
    ] {
        let flow = ClientFlow { state, ..default() };
        for (i, (label, _)) in entries(&flow).iter().enumerate() {
            let rect = [-100., -8. - i as f32 * 26., 200., 20.];
            let size = button_text_size(rect, label);
            assert!(size >= 0.75);
            assert!(
                (label.to_uppercase().chars().count() as f32 * 6. - 1.) * size <= rect[2] - 10.
            );
            for (w, h) in [(1280, 720), (1920, 1080), (1024, 768)] {
                let window = Window {
                    resolution: (w, h).into(),
                    ..default()
                };
                assert!((rect[0] + rect[2]).abs() * scale(&window) <= window.width() / 2.);
                assert!(rect[1].abs() * scale(&window) <= window.height() / 2.);
            }
        }
    }
    assert_eq!(entries(&ClientFlow::default())[0].0, "Singleplayer");
}

#[test]
fn backdrop_context_alpha_and_camera_preserve_world_and_opaque_buttons() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Startup, setup);
    app.update();
    let w = app.world_mut();
    let camera = w
        .query_filtered::<&Camera, With<MenuCamera>>()
        .single(w)
        .unwrap();
    assert_eq!(camera.order, 10);
    assert!(matches!(camera.clear_color, ClearColorConfig::None));
    let handle = w
        .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<MenuSurface>>()
        .single(w)
        .unwrap();
    assert_eq!(
        w.resource::<Assets<StandardMaterial>>()
            .get(&handle.0)
            .unwrap()
            .alpha_mode,
        AlphaMode::Blend
    );
    let mut editor = settings::Editor::default();
    for (state, expected) in [
        (ClientState::Title, false),
        (ClientState::Worlds, false),
        (ClientState::Paused, true),
        (ClientState::Settings, false),
        (ClientState::Loading, false),
        (ClientState::Saving, false),
    ] {
        assert_eq!(
            world_backdrop(&ClientFlow { state, ..default() }, &editor),
            expected
        );
    }
    // Use the real Options entry to establish the editor's paused origin.
    w.init_resource::<ButtonInput<KeyCode>>();
    w.init_resource::<ButtonInput<MouseButton>>();
    w.init_resource::<ClientFlow>();
    w.init_resource::<crate::settings::Preferences>();
    w.init_resource::<settings::Editor>();
    w.init_resource::<Navigation>();
    w.resource_mut::<ClientFlow>().state = ClientState::Paused;
    settings::open(w);
    editor = w.remove_resource::<settings::Editor>().unwrap();
    assert!(world_backdrop(w.resource::<ClientFlow>(), &editor));
    for focused in [false, true] {
        for hovered in [false, true] {
            let mut canvas = Canvas::default();
            canvas.button(
                [-100., 0., 200., 20.],
                "Back to Game",
                focused,
                hovered,
                false,
                true,
            );
            assert!(canvas.colors.iter().all(|c| c[3] == 1.));
        }
    }
}
