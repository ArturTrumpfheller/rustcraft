//! Startup loading is owned by the client lifecycle worker. Per-frame code only
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
            .add_systems(Update, update_saving.run_if(crate::session::has_world));
    }
}

pub(crate) fn data_directory() -> io::Result<PathBuf> {
    directory_from(cfg!(windows), |name| std::env::var_os(name))
}

#[cfg(test)]
fn seed_from(value: Option<OsString>) -> io::Result<u64> {
    match value {
        None => Ok(voxel_core::worldgen::DEFAULT_SEED),
        Some(value) => value
            .to_str()
            .and_then(|s| {
                (!s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
                    .then(|| s.parse().ok())
                    .flatten()
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "RUSTCRAFT_WORLD_SEED must be a decimal u64",
                )
            }),
    }
}
/// Called lazily by SaveStore only for a genuinely absent world. An invalid
/// override cannot interfere with loading an existing snapshot.
#[cfg(test)]
pub(crate) fn create_world() -> Result<World, SaveError> {
    let seed = seed_from(std::env::var_os("RUSTCRAFT_WORLD_SEED"))?;
    let start = std::time::Instant::now();
    let world = voxel_core::streaming::new_large_world(seed);
    eprintln!(
        "RustCraft new streamed world: seed {seed}, generator v{}, regional 65536x65536 blocks; identity created in {:?}",
        voxel_core::worldgen::CURRENT_LOCAL_GENERATOR,
        start.elapsed()
    );
    Ok(world)
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
    time: voxel_core::time::WorldTime,
    revision: u64,
    inventory_revision: Option<u64>,
    handle: JoinHandle<Result<(), SaveError>>,
}
#[derive(Resource)]
pub(crate) struct SaveSession {
    saved_time: voxel_core::time::WorldTime,
    pub(crate) store: Arc<SaveStore>,
    saved_revision: u64,
    saved_inventory_revision: Option<u64>,
    creation_pending: bool,
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
    // World-only fixture support is unavailable to the production client.
    #[cfg(test)]
    pub fn new(store: SaveStore, world: &World, source: LoadSource) -> Self {
        Self::base(store, world, source)
    }
    fn base(store: SaveStore, world: &World, source: LoadSource) -> Self {
        Self {
            saved_time: world.time,
            store: Arc::new(store),
            saved_revision: world.revision(),
            saved_inventory_revision: None,
            creation_pending: source == LoadSource::NewWorld,
            observed_revision: world.revision(),
            job: None,
            manual_pending: source == LoadSource::NewWorld,
            closing: false,
            last_attempt: Duration::ZERO,
            interval: Duration::from_secs(30),
            last_error: None,
            initial_source: source,
            saved_once: false,
        }
    }
    pub fn with_player(
        store: SaveStore,
        world: &World,
        source: LoadSource,
        inventory: &voxel_core::inventory::Inventory,
    ) -> Self {
        let mut s = Self::base(store, world, source);
        s.saved_inventory_revision = Some(inventory.revision());
        s
    }
    fn start(
        &mut self,
        world: &World,
        now: Duration,
        inventory: Option<&voxel_core::inventory::Inventory>,
    ) {
        let snapshot = world.save_snapshot(); // Stream saves share only identity/edits, no resident chunks.
        let store = Arc::clone(&self.store);
        let inventory = if self.saved_inventory_revision.is_some() {
            let Some(inv) = inventory else {
                self.failed("player inventory resource missing".into());
                return;
            };
            Some(inv.clone())
        } else {
            None
        };
        let inventory_revision = inventory.as_ref().map(|i| i.revision());
        let world_dirty = self.creation_pending || world.revision() != self.saved_revision;
        self.last_attempt = now;
        self.manual_pending = false;
        match thread::Builder::new()
            .name("rustcraft-save".into())
            .spawn(move || {
                if let Some(inv) = inventory {
                    store.save_player_session(&snapshot, &inv, world_dirty)
                } else {
                    store.save(&snapshot)
                }
            }) {
            Ok(handle) => {
                self.job = Some(SaveJob {
                    time: world.time,
                    revision: world.revision(),
                    inventory_revision,
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
    fn poll(&mut self) -> Option<u64> {
        if self
            .job
            .as_ref()
            .is_some_and(|job| job.handle.is_finished())
        {
            let job = self.job.take().expect("completed job exists");
            match job.handle.join() {
                Ok(Ok(())) => {
                    self.saved_revision = job.revision;
                    self.saved_time = job.time;
                    if let Some(r) = job.inventory_revision {
                        self.saved_inventory_revision = Some(r);
                    }
                    self.creation_pending = false;
                    self.saved_once = true;
                    self.last_error = None;
                    info!(
                        "World saved successfully: {} (revision {})",
                        self.store.path().display(),
                        job.revision
                    );
                    return Some(job.revision);
                }
                Ok(Err(error)) => self.failed(error.to_string()),
                Err(_) => self.failed("background save worker stopped unexpectedly".into()),
            }
        }
        None
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
        } else if self.initial_source == LoadSource::NewWorld {
            "New world"
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
    load_failure: Option<Res<'w, crate::streaming::StreamFailure>>,
    flow: Option<ResMut<'w, crate::session::ClientFlow>>,
}
fn update_saving(
    mut session: ResMut<SaveSession>,
    mut feedback: ResMut<SaveFeedback>,
    mut world: ResMut<VoxelWorld>,
    time: Res<Time<Real>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: SaveUi,
    inventory: Option<Res<crate::inventory_ui::GameInventory>>,
) {
    let had_job = session.job.is_some();
    if let Some(revision) = session.poll() {
        world.0.mark_regions_saved(revision);
    }
    if had_job
        && session.job.is_none()
        && let Some(error) = &session.last_error
        && let Some(flow) = &mut ui.flow
        && flow.state == crate::session::ClientState::Saving
    {
        flow.state = crate::session::ClientState::RecoverableError;
        flow.error = error.clone();
    }
    let current = world.0.revision();
    if current != session.observed_revision {
        if session.observed_revision == session.saved_revision {
            info!("World has unsaved changes (F5 saves; autosave interval 30 seconds)");
        }
        session.observed_revision = current;
    }
    let inventory_dirty = session.saved_inventory_revision.is_some_and(|r| {
        inventory
            .as_ref()
            .is_none_or(|i| i.contents.revision() != r)
    });
    let time_dirty =
        session.saved_inventory_revision.is_some() && world.0.time != session.saved_time;
    let dirty = session.creation_pending
        || current != session.saved_revision
        || inventory_dirty
        || time_dirty;
    if ui
        .flow
        .as_ref()
        .is_some_and(|f| f.state == crate::session::ClientState::Saving)
    {
        session.closing = true;
    }
    if ui.close.read().count() > 0 && ui.flow.is_none() {
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
    if session.job.is_none()
        && dirty
        && (session.manual_pending
            || session.closing
            || due
            || (world.0.save_pressure() && session.last_error.is_none()))
    {
        session.start(
            &world.0,
            time.elapsed(),
            inventory.as_ref().map(|i| &i.contents),
        );
    }
    if !dirty {
        session.manual_pending = false;
    }
    if session.closing && !dirty && session.job.is_none() {
        if let Some(flow) = &mut ui.flow {
            flow.save_complete = true;
        } else {
            ui.exit.write(AppExit::Success);
        }
    }
    if let Some(flow) = &mut ui.flow
        && flow.state == crate::session::ClientState::Saving
        && let Some(error) = &session.last_error
        && session.job.is_none()
    {
        flow.state = crate::session::ClientState::RecoverableError;
        flow.error = error.clone();
    }
    feedback.closing = session.closing;
    feedback.label = if session.last_error.is_none()
        && ui.load_failure.as_ref().is_some_and(|f| f.0.is_some())
    {
        "ERROR / LOAD".to_owned()
    } else {
        // A running clock is always newer than the last snapshot. Keep the
        // status useful for edits while time-only dirtiness still drives saves.
        session
            .label(session.creation_pending || current != session.saved_revision || inventory_dirty)
            .to_owned()
    };
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
        let legacy = if world.0.identity().is_none() {
            " / LEGACY 64x64"
        } else {
            ""
        };
        let title = format!("RustCraft [{}{legacy}]{suffix}", feedback.label);
        if window.title != title {
            window.title = title;
        }
    }
}

#[cfg(test)]
mod tests;
