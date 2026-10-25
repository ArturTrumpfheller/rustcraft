//! Versioned, bounded user preferences. Independent of every world/save identity.
use crate::controls::MovementBindings;
use bevy::{
    prelude::*,
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread::JoinHandle,
};

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub fov: u16,
    pub sensitivity: u16,
    pub invert_y: bool,
    pub radius: i32,
    pub borderless: bool,
    pub size: usize,
    pub vsync: bool,
    pub keys: [KeyCode; 19],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            fov: 45,
            sensitivity: 100,
            invert_y: false,
            radius: 2,
            borderless: false,
            size: 1,
            vsync: true,
            keys: MovementBindings::default().keys(),
        }
    }
}
pub(crate) const SIZES: [(u32, u32); 4] = [(960, 540), (1280, 720), (1600, 900), (1920, 1080)];
pub(crate) const LABELS: [&str; 19] = [
    "Forward",
    "Back",
    "Left",
    "Right",
    "Jump",
    "Sprint",
    "Sneak",
    "Respawn",
    "Inventory",
    "Pause Menu",
    "Hotbar 1",
    "Hotbar 2",
    "Hotbar 3",
    "Hotbar 4",
    "Hotbar 5",
    "Hotbar 6",
    "Hotbar 7",
    "Hotbar 8",
    "Hotbar 9",
];
// Deliberately bounded portable physical-key vocabulary. Navigation, F3 debug and F5 save stay reserved.
pub(crate) fn key_options() -> Vec<KeyCode> {
    use KeyCode::*;
    vec![
        KeyA,
        KeyB,
        KeyC,
        KeyD,
        KeyE,
        KeyF,
        KeyG,
        KeyH,
        KeyI,
        KeyJ,
        KeyK,
        KeyL,
        KeyM,
        KeyN,
        KeyO,
        KeyP,
        KeyQ,
        KeyR,
        KeyS,
        KeyT,
        KeyU,
        KeyV,
        KeyW,
        KeyX,
        KeyY,
        KeyZ,
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        Space,
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
        Escape,
    ]
}
pub(crate) fn key_name(k: KeyCode) -> String {
    match k {
        KeyCode::ControlLeft => "L-CTRL".into(),
        KeyCode::ControlRight => "R-CTRL".into(),
        KeyCode::ShiftLeft => "L-SHIFT".into(),
        KeyCode::ShiftRight => "R-SHIFT".into(),
        KeyCode::AltLeft => "L-ALT".into(),
        KeyCode::AltRight => "R-ALT".into(),
        KeyCode::Space => "SPACE".into(),
        KeyCode::Escape => "ESC".into(),
        _ => format!("{k:?}").replace("Key", "").replace("Digit", ""),
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if !(45..=110).contains(&self.fov)
            || !(10..=300).contains(&self.sensitivity)
            || !(1..=3).contains(&self.radius)
            || self.size >= SIZES.len()
        {
            return Err("Setting value is out of range".into());
        }
        let allowed = key_options();
        for (i, k) in self.keys.iter().enumerate() {
            if !allowed.contains(k) || (*k == KeyCode::Escape && i != 9) {
                return Err(format!("Key for {} is reserved", LABELS[i]));
            }
            if let Some(j) = self.keys[..i].iter().position(|old| old == k) {
                return Err(format!("Conflict: {} / {}", LABELS[j], LABELS[i]));
            }
        }
        Ok(())
    }
    fn encode(&self) -> Vec<u8> {
        format!("RUSTCRAFT_SETTINGS 1\nfov={}\nsensitivity={}\ninvert_y={}\nradius={}\nborderless={}\nsize={}\nvsync={}\nkeys={}\n",self.fov,self.sensitivity,self.invert_y,self.radius,self.borderless,self.size,self.vsync,
            self.keys.iter().map(|k|format!("{k:?}")).collect::<Vec<_>>().join(",")).into_bytes()
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "Configuration is not valid UTF-8")?;
        let lines: Vec<_> = text.lines().collect();
        if lines.len() != 9 || lines[0] != "RUSTCRAFT_SETTINGS 1" {
            return Err("Unknown or damaged settings format".into());
        }
        let value = |i: usize, name: &str| {
            lines[i]
                .strip_prefix(name)
                .ok_or_else(|| format!("Invalid field: {name}"))
        };
        let number = |i, name| {
            value(i, name)?
                .parse::<u16>()
                .map_err(|_| format!("Invalid number: {name}"))
        };
        let boolean = |i, name| {
            value(i, name)?
                .parse::<bool>()
                .map_err(|_| format!("Invalid boolean: {name}"))
        };
        let allowed = key_options();
        let keys = value(8, "keys=")?
            .split(',')
            .map(|name| {
                allowed
                    .iter()
                    .copied()
                    .find(|k| format!("{k:?}") == name)
                    .ok_or("Unknown key".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = Self {
            fov: number(1, "fov=")?,
            sensitivity: number(2, "sensitivity=")?,
            invert_y: boolean(3, "invert_y=")?,
            radius: i32::from(number(4, "radius=")?),
            borderless: boolean(5, "borderless=")?,
            size: usize::from(number(6, "size=")?),
            vsync: boolean(7, "vsync=")?,
            keys: keys.try_into().map_err(|_| "Incorrect key count")?,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn window(&self, w: &mut Window) {
        w.mode = if self.borderless {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        let (width, height) = SIZES[self.size];
        w.resolution.set(width as f32, height as f32);
        w.present_mode = if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
    }
}
fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err(io::Error::other("Configuration must be a regular file"));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(io::Error::other("Configuration is too large"));
    }
    Ok(Some(bytes))
}
#[derive(Clone)]
pub(crate) struct Store {
    path: PathBuf,
    expected: Option<Vec<u8>>,
}
impl Store {
    pub fn load(root: &Path) -> (Self, Result<Preferences, String>) {
        let path = root.join("settings.rcs");
        match read(&path) {
            Ok(bytes) => {
                let result = if bytes.is_none()
                    && ["settings.rcs.tmp", "settings.rcs.rejected"]
                        .iter()
                        .any(|name| root.join(name).symlink_metadata().is_ok())
                {
                    Err(
                        "Configuration is missing but recovery files exist; inspect the files"
                            .into(),
                    )
                } else {
                    bytes
                        .as_ref()
                        .map_or_else(|| Ok(Preferences::default()), |b| Preferences::decode(b))
                };
                (
                    Self {
                        path,
                        expected: bytes,
                    },
                    result,
                )
            }
            Err(e) => (
                Self {
                    path,
                    expected: None,
                },
                Err(format!("Cannot read configuration: {e}")),
            ),
        }
    }
    fn save(&mut self, p: &Preferences, preserve: bool) -> Result<(), String> {
        p.validate()?;
        let run = || -> io::Result<()> {
            let root = self.path.parent().unwrap();
            fs::create_dir_all(root)?;
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(root.join(".settings.lock"))?;
            lock.try_lock().map_err(io::Error::other)?;
            if read(&self.path)? != self.expected {
                return Err(io::Error::other(
                    "Configuration changed externally; please restart",
                ));
            }
            if preserve && let Some(old) = &self.expected {
                let mut backup = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join("settings.rcs.rejected"))?;
                backup.write_all(old)?;
                backup.sync_all()?;
            }
            let temp = root.join("settings.rcs.tmp");
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let result = (|| {
                f.write_all(&p.encode())?;
                f.sync_all()?;
                drop(f);
                fs::rename(&temp, &self.path)?;
                #[cfg(unix)]
                File::open(root)?.sync_all()?;
                Ok(())
            })();
            if result.is_err() {
                let _ = fs::remove_file(&temp);
            }
            result
        };
        run().map_err(|e| format!("Could not save options: {e}"))?;
        self.expected = Some(p.encode());
        Ok(())
    }
}
#[derive(Resource, Default)]
pub(crate) struct Config {
    pub store: Option<Store>,
    pub error: String,
    pub invalid: bool,
    pub job: Option<JoinHandle<Result<(Store, Preferences), String>>>,
}
impl Config {
    pub fn load() -> (Self, Preferences) {
        match crate::persistence::data_directory() {
            Ok(root) => {
                let (store, result) = Store::load(&root);
                match result {
                    Ok(p) => (
                        Self {
                            store: Some(store),
                            ..default()
                        },
                        p,
                    ),
                    Err(e) => (
                        Self {
                            store: Some(store),
                            error: e,
                            invalid: true,
                            ..default()
                        },
                        Preferences::default(),
                    ),
                }
            }
            Err(e) => (
                Self {
                    error: e.to_string(),
                    invalid: true,
                    ..default()
                },
                Preferences::default(),
            ),
        }
    }
    pub fn save(&mut self, p: Preferences, reset: bool) {
        if self.job.is_some() {
            return;
        }
        if self.invalid && !reset {
            self.error =
                "File preserved. Explicitly restore defaults or repair the file externally.".into();
            return;
        }
        let Some(mut store) = self.store.clone() else {
            return;
        };
        let preserve = self.invalid;
        self.error.clear();
        match std::thread::Builder::new()
            .name("rustcraft-settings".into())
            .spawn(move || {
                store.save(&p, preserve)?;
                Ok((store, p))
            }) {
            Ok(job) => self.job = Some(job),
            Err(e) => self.error = e.to_string(),
        }
    }
}
pub(crate) fn apply(world: &mut World, p: Preferences) {
    world.insert_resource(MovementBindings::from_keys(p.keys));
    if let Some(mut s) = world.get_resource_mut::<crate::streaming::StreamState>()
        && std::env::var_os("RUSTCRAFT_LOAD_RADIUS").is_none()
    {
        s.set_radius(p.radius);
    }
    // The camera adapter owns the sprint envelope; replacing its base resets it.
    crate::camera::set_fov(world, p.fov);
    if let Ok(mut w) = world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(world)
    {
        p.window(&mut w);
    }
    world.insert_resource(p);
    world.insert_resource(crate::controls::MovementGestures::default());
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
}
pub(crate) fn poll(world: &mut World) {
    if !world
        .resource::<Config>()
        .job
        .as_ref()
        .is_some_and(|j| j.is_finished())
    {
        return;
    }
    let result = world.resource_mut::<Config>().job.take().unwrap().join();
    match result {
        Ok(Ok((store, p))) => {
            let mut c = world.resource_mut::<Config>();
            c.store = Some(store);
            c.error = "Saved".into();
            c.invalid = false;
            apply(world, p);
        }
        Ok(Err(e)) => {
            error!("{e}");
            world.resource_mut::<Config>().error = e;
        }
        Err(_) => world.resource_mut::<Config>().error = "Save operation failed".into(),
    }
}
#[cfg(test)]
mod tests;
