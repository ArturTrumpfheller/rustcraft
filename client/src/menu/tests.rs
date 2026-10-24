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
