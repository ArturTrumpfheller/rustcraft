use super::*;
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rustcraft-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn first_start_does_not_create_files_and_restart_restores_all_values() {
    let dir = Dir::new();
    let (mut store, p) = Store::load(&dir.0);
    assert_eq!(p.unwrap(), Preferences::default());
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    let mut p = Preferences {
        fov: 78,
        sensitivity: 137,
        invert_y: true,
        radius: 3,
        borderless: true,
        size: 2,
        vsync: false,
        ..default()
    };
    p.keys[0] = KeyCode::KeyZ;
    store.save(&p, false).unwrap();
    assert_eq!(Store::load(&dir.0).1.unwrap(), p);
    p = Preferences::default();
    store.save(&p, false).unwrap();
    assert_eq!(Store::load(&dir.0).1.unwrap(), p);
}
#[test]
fn corrupt_incompatible_missing_fields_and_ranges_never_become_defaults() {
    let dir = Dir::new();
    let p = Preferences::default();
    for bad in [
        vec![255],
        b"RUSTCRAFT_SETTINGS 2\n".to_vec(),
        p.encode()[..30].to_vec(),
        String::from_utf8(p.encode())
            .unwrap()
            .replace("fov=45", "fov=111")
            .into_bytes(),
        String::from_utf8(p.encode())
            .unwrap()
            .replace("radius=2", "radius=0")
            .into_bytes(),
        vec![0; 4097],
    ] {
        fs::write(dir.0.join("settings.rcs"), &bad).unwrap();
        assert!(Store::load(&dir.0).1.is_err());
        assert_eq!(fs::read(dir.0.join("settings.rcs")).unwrap(), bad);
    }
}
#[test]
fn bounds_conflicts_and_reserved_keys_are_rejected() {
    let p = Preferences::default();
    assert!(p.validate().is_ok());
    for i in 0..19 {
        let mut p = p.clone();
        p.keys[i] = KeyCode::F5;
        assert!(p.validate().is_err());
    }
    let mut p = p.clone();
    p.keys[0] = p.keys[1];
    assert!(p.validate().unwrap_err().contains("Conflict"));
    p = Preferences::default();
    p.keys[9] = KeyCode::KeyP;
    assert!(p.validate().is_ok());
    p.keys[0] = KeyCode::Escape;
    assert!(p.validate().is_err());
    for (fov, sensitivity) in [(44, 100), (111, 100), (78, 9), (78, 301)] {
        let p = Preferences {
            fov,
            sensitivity,
            ..default()
        };
        assert!(p.validate().is_err());
    }
}
#[test]
fn explicit_corrupt_reset_preserves_bytes_and_external_edits_are_not_overwritten() {
    let dir = Dir::new();
    let path = dir.0.join("settings.rcs");
    fs::write(&path, b"damaged").unwrap();
    let (mut store, result) = Store::load(&dir.0);
    assert!(result.is_err());
    store.save(&Preferences::default(), true).unwrap();
    assert_eq!(
        fs::read(dir.0.join("settings.rcs.rejected")).unwrap(),
        b"damaged"
    );
    fs::write(&path, b"external edit").unwrap();
    assert!(
        store
            .save(&Preferences::default(), false)
            .unwrap_err()
            .contains("externally")
    );
    assert_eq!(fs::read(path).unwrap(), b"external edit");
}
#[test]
fn interrupted_temp_and_lock_contention_preserve_primary() {
    let dir = Dir::new();
    let (mut store, _) = Store::load(&dir.0);
    let p = Preferences::default();
    store.save(&p, false).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.0.join(".settings.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(store.save(&p, false).is_err());
    drop(lock);
    fs::write(dir.0.join("settings.rcs.tmp"), b"interrupted").unwrap();
    assert!(
        store
            .save(
                &Preferences {
                    fov: 78,
                    ..default()
                },
                false
            )
            .is_err()
    );
    assert_eq!(Store::load(&dir.0).1.unwrap(), p);
    assert_eq!(
        fs::read(dir.0.join("settings.rcs.tmp")).unwrap(),
        b"interrupted"
    );
}
#[test]
fn non_regular_config_is_an_error() {
    let dir = Dir::new();
    fs::create_dir(dir.0.join("settings.rcs")).unwrap();
    assert!(Store::load(&dir.0).1.is_err());
}
#[cfg(unix)]
#[test]
fn config_symlink_is_not_followed() {
    let dir = Dir::new();
    fs::write(dir.0.join("other"), Preferences::default().encode()).unwrap();
    std::os::unix::fs::symlink(dir.0.join("other"), dir.0.join("settings.rcs")).unwrap();
    assert!(Store::load(&dir.0).1.is_err());
}
#[test]
fn display_options_use_supported_fallback_presentation_modes() {
    let mut w = Window::default();
    let mut p = Preferences::default();
    p.window(&mut w);
    assert_eq!(w.mode, WindowMode::Windowed);
    assert_eq!(w.present_mode, PresentMode::AutoVsync);
    assert_eq!(w.width(), 1280.);
    p.borderless = true;
    p.vsync = false;
    p.size = 0;
    p.window(&mut w);
    assert_eq!(
        w.mode,
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    );
    assert_eq!(w.present_mode, PresentMode::AutoNoVsync);
    assert_eq!(w.width(), 960.);
}
#[test]
fn saving_preferences_does_not_touch_world_payloads() {
    let dir = Dir::new();
    fs::create_dir(dir.0.join("worlds")).unwrap();
    fs::write(dir.0.join("worlds/sentinel"), b"world inventory journal").unwrap();
    let (mut store, _) = Store::load(&dir.0);
    store
        .save(
            &Preferences {
                fov: 78,
                ..default()
            },
            false,
        )
        .unwrap();
    assert_eq!(
        fs::read(dir.0.join("worlds/sentinel")).unwrap(),
        b"world inventory journal"
    );
}
#[test]
fn missing_primary_with_recovery_evidence_requires_explicit_recovery() {
    let dir = Dir::new();
    fs::write(dir.0.join("settings.rcs.tmp"), b"interrupted").unwrap();
    assert!(Store::load(&dir.0).1.is_err());
    assert_eq!(
        fs::read(dir.0.join("settings.rcs.tmp")).unwrap(),
        b"interrupted"
    );
}
