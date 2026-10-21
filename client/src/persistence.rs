//! Startup loading is handled before App construction. Per-frame code only
//! clones bounded snapshots and polls completed jobs; all save I/O is off-thread.
use crate::scene::VoxelWorld;
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    window::{PrimaryWindow, WindowCloseRequested},
};
use std::{
    ffi::OsString,
    io,
    path::PathBuf,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};
use voxel_core::{
    World,
    persistence::{LoadSource, SaveError, SaveStore},
};

pub struct PersistencePlugin;
impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveFeedback>()
            .add_message::<WindowCloseRequested>()
            .add_message::<AppExit>()
            .add_systems(Update, update_saving);
    }
}

pub(crate) fn data_directory() -> io::Result<PathBuf> {
    directory_from(cfg!(windows), |name| std::env::var_os(name))
}
fn directory_from(windows: bool, get: impl Fn(&str) -> Option<OsString>) -> io::Result<PathBuf> {
    if let Some(value) = get("RUSTCRAFT_DATA_DIR") {
        let path = PathBuf::from(value);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RUSTCRAFT_DATA_DIR must be an absolute directory",
            ))
        };
    }
    let absolute = |name| get(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if windows {
        absolute("LOCALAPPDATA")
            .or_else(|| absolute("APPDATA"))
            .map(|p| p.join("RustCraft"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "LOCALAPPDATA/APPDATA unavailable; set RUSTCRAFT_DATA_DIR",
                )
            })
    } else {
        absolute("XDG_DATA_HOME")
            .or_else(|| absolute("HOME").map(|p| p.join(".local/share")))
            .map(|p| p.join("rustcraft"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "XDG_DATA_HOME/HOME unavailable; set RUSTCRAFT_DATA_DIR",
                )
            })
    }
}

#[derive(Resource)]
pub(crate) struct SaveFeedback {
    pub label: String,
    pub closing: bool,
}
impl Default for SaveFeedback {
    fn default() -> Self {
        Self {
            label: "Clean".into(),
            closing: false,
        }
    }
}
struct SaveJob {
    revision: u64,
    handle: JoinHandle<Result<(), SaveError>>,
}
#[derive(Resource)]
pub(crate) struct SaveSession {
    store: Arc<SaveStore>,
    saved_revision: u64,
    observed_revision: u64,
    job: Option<SaveJob>,
    manual_pending: bool,
    closing: bool,
    last_attempt: Duration,
    interval: Duration,
    last_error: Option<String>,
    initial_source: LoadSource,
    saved_once: bool,
}
impl SaveSession {
    pub fn new(store: SaveStore, world: &World, source: LoadSource) -> Self {
        Self {
            store: Arc::new(store),
            saved_revision: world.revision(),
            observed_revision: world.revision(),
            job: None,
            manual_pending: false,
            closing: false,
            last_attempt: Duration::ZERO,
            interval: Duration::from_secs(30),
            last_error: None,
            initial_source: source,
            saved_once: false,
        }
    }
    fn start(&mut self, world: &World, now: Duration) {
        let snapshot = world.clone(); // At most 48 small chunk vectors; no I/O.
        let store = Arc::clone(&self.store);
        self.last_attempt = now;
        self.manual_pending = false;
        match thread::Builder::new()
            .name("rustcraft-save".into())
            .spawn(move || store.save(&snapshot))
        {
            Ok(handle) => {
                self.job = Some(SaveJob {
                    revision: world.revision(),
                    handle,
                });
                info!("Saving world to {}", self.store.path().display());
            }
            Err(error) => self.failed(error.to_string()),
        }
    }
    fn failed(&mut self, message: String) {
        error!(
            "World save failed: {message}. Changes remain dirty; F5 retries. Close canceled if unsaved."
        );
        self.last_error = Some(message);
        self.closing = false;
    }
    fn poll(&mut self) {
        if self
            .job
            .as_ref()
            .is_some_and(|job| job.handle.is_finished())
        {
            let job = self.job.take().expect("completed job exists");
            match job.handle.join() {
                Ok(Ok(())) => {
                    self.saved_revision = job.revision;
                    self.saved_once = true;
                    self.last_error = None;
                    info!(
                        "World saved successfully: {} (revision {})",
                        self.store.path().display(),
                        job.revision
                    );
                }
                Ok(Err(error)) => self.failed(error.to_string()),
                Err(_) => self.failed("background save worker stopped unexpectedly".into()),
            }
        }
    }
    fn label(&self, dirty: bool) -> &'static str {
        if self.job.is_some() {
            "Saving"
        } else if self.last_error.is_some() {
            "ERROR / DIRTY"
        } else if dirty {
            "Dirty"
        } else if self.saved_once {
            "Saved"
        } else if self.initial_source == LoadSource::Saved {
            "Loaded"
        } else {
            "New demo"
        }
    }
}
impl Drop for SaveSession {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            // Normal window close already waited without blocking frames. This
            // only drains a pending worker if some other runner drops the App.
            let _ = job.handle.join();
        }
    }
}

#[derive(SystemParam)]
struct SaveUi<'w, 's> {
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    close: MessageReader<'w, 's, WindowCloseRequested>,
    exit: MessageWriter<'w, AppExit>,
}
fn update_saving(
    mut session: ResMut<SaveSession>,
    mut feedback: ResMut<SaveFeedback>,
    world: Res<VoxelWorld>,
    time: Res<Time<Real>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: SaveUi,
) {
    session.poll();
    let current = world.0.revision();
    if current != session.observed_revision {
        if session.observed_revision == session.saved_revision {
            info!("World has unsaved changes (F5 saves; autosave interval 30 seconds)");
        }
        session.observed_revision = current;
    }
    let dirty = current != session.saved_revision;
    if ui.close.read().count() > 0 {
        session.closing = true;
    }
    let manual =
        keyboard.just_pressed(KeyCode::F5) && ui.windows.iter().any(|window| window.focused);
    if manual {
        if dirty {
            session.manual_pending = true;
        } else {
            info!("World unchanged; nothing to save");
        }
    }
    let due = time.elapsed().saturating_sub(session.last_attempt) >= session.interval;
    if session.job.is_none() && dirty && (session.manual_pending || session.closing || due) {
        session.start(&world.0, time.elapsed());
    }
    if !dirty {
        session.manual_pending = false;
    }
    if session.closing && !dirty && session.job.is_none() {
        ui.exit.write(AppExit::Success);
    }
    feedback.closing = session.closing;
    feedback.label = session.label(dirty).to_owned();
    for mut window in &mut ui.windows {
        let suffix = if window.title.starts_with("RustCraft [") {
            window
                .title
                .split_once(']')
                .map_or("", |(_, suffix)| suffix)
        } else {
            window
                .title
                .strip_prefix("RustCraft")
                .unwrap_or(&window.title)
        };
        let title = format!("RustCraft [{}]{suffix}", feedback.label);
        if window.title != title {
            window.title = title;
        }
    }
}

#[cfg(test)]
mod tests;
