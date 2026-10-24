use super::*;
fn app(page: Page) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ClientFlow {
            state: ClientState::Worlds,
            ..default()
        })
        .insert_resource(Browser {
            page,
            name: "Neue Welt".into(),
            ..default()
        })
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<KeyboardInput>()
        .add_systems(Update, text_input);
    app.world_mut().spawn((
        Window {
            focused: true,
            ..default()
        },
        PrimaryWindow,
    ));
    app
}
fn type_text(app: &mut App, text: &str) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: Some(text.into()),
        repeat: false,
        window,
    });
    app.update();
}
#[test]
fn unicode_input_seed_validation_and_cancel_have_no_filesystem_side_effects() {
    let mut a = app(Page::Create);
    a.world_mut().resource_mut::<Browser>().name.clear();
    type_text(&mut a, "ÄÖü ß & / Café ♥");
    assert_eq!(a.world().resource::<Browser>().name, "ÄÖü ß & / Café ♥");
    a.world_mut().resource_mut::<Browser>().focus = 1;
    type_text(&mut a, "-1");
    activate(a.world_mut(), Command::Confirm);
    assert!(a.world().resource::<Browser>().error.contains("Seed"));
    assert!(a.world().resource::<Browser>().job.is_none());
    assert_eq!(a.world().resource::<Browser>().page, Page::Create);
    activate(a.world_mut(), Command::Back);
    assert_eq!(a.world().resource::<Browser>().page, Page::List);
    activate(a.world_mut(), Command::Back);
    assert_eq!(a.world().resource::<ClientFlow>().state, ClientState::Title);
}
#[test]
fn paging_selection_and_confirmations_pin_ids_and_start_on_cancel() {
    let mut a = app(Page::List);
    a.world_mut().resource_mut::<Browser>().entries = (0..8)
        .map(|i| WorldEntry {
            key: WorldKey::Managed(format!("{i:032x}")),
            name: "Same Name".into(),
            path: std::path::PathBuf::new(),
            seed: Some(i),
            generator: Some(3),
            kind: "Regional".into(),
            created: None,
            error: None,
        })
        .collect();
    activate(a.world_mut(), Command::Next);
    assert_eq!(a.world().resource::<Browser>().offset, 3);
    activate(a.world_mut(), Command::Select(4));
    let selected = a.world().resource::<Browser>().selected.clone();
    activate(a.world_mut(), Command::Archive);
    let b = a.world().resource::<Browser>();
    assert_eq!(b.focus, 1);
    assert!(matches!(b.controls()[b.focus].action, Some(Command::Back)));
    assert_eq!(b.selected, selected);
    assert!(b.job.is_none());
    activate(a.world_mut(), Command::Back);
    activate(a.world_mut(), Command::Previous);
    assert_eq!(a.world().resource::<Browser>().offset, 0);
    assert_eq!(a.world().resource::<Browser>().selected, selected);
    activate(a.world_mut(), Command::Import);
    assert_eq!(a.world().resource::<Browser>().focus, 1);
    assert!(a.world().resource::<Browser>().job.is_none());
}
#[test]
fn corrupt_and_legacy_entries_remain_inspectable_without_play_or_rename() {
    let mut a = app(Page::List);
    let key = WorldKey::Legacy;
    a.world_mut()
        .resource_mut::<Browser>()
        .entries
        .push(WorldEntry {
            key: key.clone(),
            name: "Legacy".into(),
            path: "/safe/test".into(),
            seed: None,
            generator: None,
            kind: "?".into(),
            created: None,
            error: Some("broken player".into()),
        });
    a.world_mut().resource_mut::<Browser>().selected = Some(key);
    let b = a.world().resource::<Browser>();
    let controls = b.controls();
    assert!(
        !controls
            .iter()
            .any(|c| matches!(c.action, Some(Command::Play | Command::Rename)))
    );
    assert!(
        controls
            .iter()
            .any(|c| matches!(c.action, Some(Command::Details)))
    );
    activate(a.world_mut(), Command::Details);
    assert!(
        a.world()
            .resource::<Browser>()
            .controls()
            .iter()
            .any(|c| matches!(c.action, Some(Command::Import)))
    );
}
#[test]
fn date_and_layout_fit_720p_1080p_and_four_three() {
    assert_eq!(date(0), "1970-01-01 UTC");
    assert_eq!(date(1709164800), "2024-02-29 UTC");
    for size in [(1280, 720), (1920, 1080), (1024, 768)] {
        let w = Window {
            resolution: size.into(),
            ..default()
        };
        for page in [
            Page::List,
            Page::Create,
            Page::Rename,
            Page::Archive,
            Page::Import,
            Page::Details,
        ] {
            let b = Browser { page, ..default() };
            for c in b.controls() {
                let [x, y, width, h] = c.rect;
                assert!(x >= -240. && x + width <= 240. && y >= -135. && y + h <= 135.);
            }
            assert!(scale(&w) * 480. <= w.width() + 0.01);
            assert!(scale(&w) * 270. <= w.height() + 0.01);
        }
    }
}
#[test]
fn native_quit_waits_for_catalog_worker_completion() {
    let mut a = app(Page::List);
    a.add_message::<AppExit>();
    a.world_mut().resource_mut::<ClientFlow>().destination = Some(Destination::Quit);
    let (tx, rx) = std::sync::mpsc::channel();
    a.world_mut().resource_mut::<Browser>().job = Some(thread::spawn(move || {
        rx.recv().unwrap();
        Ok((vec![], None))
    }));
    poll(a.world_mut());
    assert!(a.world().resource::<Messages<AppExit>>().is_empty());
    tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while a.world().resource::<Browser>().job.is_some() {
        assert!(std::time::Instant::now() < deadline);
        poll(a.world_mut());
        std::thread::yield_now();
    }
    assert!(!a.world().resource::<Messages<AppExit>>().is_empty());
}
