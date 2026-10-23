//! Explicit client lifecycle. Only Play opens the store; all disk IO runs on workers.
//! A session is retained until the final save and both terrain workers have drained.
use crate::{
    camera, controls, interaction, inventory_ui, persistence, player, scene, streaming, viewmodel,
};
use bevy::{
    ecs::schedule::ScheduleLabel,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowCloseRequested},
};
use std::{
    path::PathBuf,
    thread::{self, JoinHandle},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ClientState {
    #[default]
    Title,
    Loading,
    Playing,
    Paused,
    Saving,
    RecoverableError,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Destination {
    Title,
    Quit,
}
#[derive(Resource, Default)]
pub(crate) struct ClientFlow {
    pub state: ClientState,
    pub destination: Option<Destination>,
    pub error: String,
    pub save_complete: bool,
    pub resume_frame: bool,
}
impl ClientFlow {
    pub fn playing(&self) -> bool {
        self.state == ClientState::Playing
    }
    pub fn leaving(&self) -> bool {
        self.destination.is_some()
    }
    pub fn save(&mut self, destination: Destination) {
        self.destination = Some(destination);
        self.state = ClientState::Saving;
        self.save_complete = false;
        self.error.clear();
    }
}
#[derive(Component)]
pub(crate) struct SessionEntity;
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionSetup;
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FlowInput;
pub(crate) fn has_world(world: &World) -> bool {
    world.contains_resource::<scene::VoxelWorld>()
}
pub(crate) fn playing(flow: Option<Res<ClientFlow>>) -> bool {
    flow.is_none_or(|f| f.playing())
}

struct Loaded {
    world: scene::VoxelWorld,
    inventory: inventory_ui::GameInventory,
    save: persistence::SaveSession,
    stream: streaming::StreamState,
    spawn: player::InitialSpawn,
}
#[derive(Resource)]
struct Loading(JoinHandle<Result<Loaded, String>>);
fn load(directory: PathBuf) -> Result<Loaded, String> {
    let (store, mut world, source) =
        voxel_core::persistence::SaveStore::open_with(&directory, persistence::create_world)
            .map_err(|e| e.to_string())?;
    let inventory = store.load_player(&mut world).map_err(|e| e.to_string())?;
    let start = if world.region_identity().is_some() {
        player::start_column(
            std::env::var_os("RUSTCRAFT_START_CHUNK"),
            world.bounds().1[0] / 16,
        )
        .map_err(|e| e.to_string())?
    } else {
        None
    };
    let column = start.unwrap_or([0, 0]);
    store
        .prepare_regional_column(&mut world, column)
        .map_err(|e| e.to_string())?;
    let stream = if world.identity().is_some() {
        let radius = streaming::radius_from(std::env::var_os("RUSTCRAFT_LOAD_RADIUS"))
            .map_err(|e| e.to_string())?;
        let generator =
            voxel_core::streaming::prepare_column(&mut world, column).map_err(|e| e.to_string())?;
        streaming::StreamState::new(generator, radius)
    } else {
        streaming::StreamState::default()
    };
    let save = persistence::SaveSession::with_player(store, &world, source, &inventory);
    let spawn = start.map_or(player::SPAWN, |c| {
        [
            f64::from(c[0]) * 16. + 8.5,
            32.,
            f64::from(c[1]) * 16. + 8.5,
        ]
    });
    Ok(Loaded {
        world: scene::VoxelWorld(world),
        inventory: inventory_ui::GameInventory::loaded(inventory),
        save,
        stream,
        spawn: player::InitialSpawn(spawn),
    })
}
pub(crate) fn begin_loading(world: &mut World) {
    let result = persistence::data_directory()
        .map_err(|e| e.to_string())
        .and_then(|directory| {
            info!("Opening {}", directory.display());
            thread::Builder::new()
                .name("rustcraft-load".into())
                .spawn(move || load(directory))
                .map_err(|e| e.to_string())
        });
    match result {
        Ok(handle) => {
            world.insert_resource(Loading(handle));
            world.resource_mut::<ClientFlow>().state = ClientState::Loading;
        }
        Err(e) => load_failed(world, e),
    }
}
fn load_failed(world: &mut World, error: String) {
    error!("Loading failed; files preserved: {error}");
    let mut flow = world.resource_mut::<ClientFlow>();
    flow.state = ClientState::RecoverableError;
    flow.error = error;
    flow.destination = None;
}

// IDs created synchronously by SessionSetup belong to that session. Runtime chunk
// meshes are tracked by ChunkRenders. Engine/global/menu assets are never swept.
#[derive(Resource, Default)]
struct SessionAssets {
    meshes: Vec<AssetId<Mesh>>,
    materials: Vec<AssetId<StandardMaterial>>,
    images: Vec<AssetId<Image>>,
}
fn install(world: &mut World, loaded: Loaded) {
    world.insert_resource(loaded.world);
    world.insert_resource(loaded.inventory);
    world.insert_resource(loaded.save);
    world.insert_resource(loaded.stream);
    world.insert_resource(loaded.spawn);
    world.insert_resource(player::PlayerState::default());
    world.insert_resource(player::ControllerInput::default());
    world.insert_resource(controls::MovementGestures::default());
    world.insert_resource(scene::RenderOrigin::default());
    world.insert_resource(scene::DirtyChunks::default());
    world.insert_resource(scene::ChunkRenders::default());
    world.insert_resource(scene::LegacyMeshing::default());
    world.insert_resource(streaming::StreamFailure::default());
    world.insert_resource(persistence::SaveFeedback::default());
    world.insert_resource(viewmodel::HandAction::default());
    camera::reset(world);
    interaction::reset(world);
    let meshes: std::collections::HashSet<_> = world.resource::<Assets<Mesh>>().ids().collect();
    let materials: std::collections::HashSet<_> =
        world.resource::<Assets<StandardMaterial>>().ids().collect();
    let images: std::collections::HashSet<_> = world.resource::<Assets<Image>>().ids().collect();
    world.run_schedule(SessionSetup);
    world.insert_resource(SessionAssets {
        meshes: world
            .resource::<Assets<Mesh>>()
            .ids()
            .filter(|id| !meshes.contains(id))
            .collect(),
        materials: world
            .resource::<Assets<StandardMaterial>>()
            .ids()
            .filter(|id| !materials.contains(id))
            .collect(),
        images: world
            .resource::<Assets<Image>>()
            .ids()
            .filter(|id| !images.contains(id))
            .collect(),
    });
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    let mut flow = world.resource_mut::<ClientFlow>();
    flow.state = if flow.destination.is_some() {
        ClientState::Saving
    } else {
        ClientState::Playing
    };
    flow.resume_frame = flow.playing();
    flow.error.clear();
}
fn cleanup(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<SessionEntity>>()
        .iter(world)
        .collect();
    for entity in entities {
        let _ = world.despawn(entity);
    }
    let chunks: Vec<_> = world
        .resource::<scene::ChunkRenders>()
        .0
        .values()
        .map(|r| r.mesh.id())
        .collect();
    let assets = world.remove_resource::<SessionAssets>().unwrap_or_default();
    inventory_ui::cleanup(world);
    viewmodel::cleanup(world);
    world.remove_resource::<scene::TerrainMaterial>();
    world.remove_resource::<scene::ChunkRenders>();
    world.remove_resource::<scene::VoxelWorld>();
    world.remove_resource::<player::InitialSpawn>();
    world.remove_resource::<streaming::StreamState>();
    world.remove_resource::<scene::LegacyMeshing>();
    world.remove_resource::<persistence::SaveSession>(); // last Arc<SaveStore>, releases OS lock
    for id in assets.meshes.into_iter().chain(chunks) {
        world.resource_mut::<Assets<Mesh>>().remove(id);
    }
    for id in assets.materials {
        world.resource_mut::<Assets<StandardMaterial>>().remove(id);
    }
    for id in assets.images {
        world.resource_mut::<Assets<Image>>().remove(id);
    }
    world.insert_resource(player::ControllerInput::default());
    world.insert_resource(player::PlayerState::default());
    world.insert_resource(inventory_ui::GameInventory::default());
    world.insert_resource(controls::MovementGestures::default());
    world.insert_resource(scene::DirtyChunks::default());
    world.insert_resource(scene::RenderOrigin::default());
    world.insert_resource(streaming::StreamFailure::default());
    world.insert_resource(persistence::SaveFeedback::default());
    world.insert_resource(viewmodel::HandAction::default());
    camera::reset(world);
    interaction::reset(world);
}
fn advance(world: &mut World) {
    if world
        .get_resource::<Loading>()
        .is_some_and(|job| job.0.is_finished())
    {
        match world.remove_resource::<Loading>().unwrap().0.join() {
            Ok(Ok(loaded)) => install(world, loaded),
            Ok(Err(e)) => load_failed(world, e),
            Err(_) => load_failed(world, "Ladeauftrag unerwartet beendet".into()),
        }
    }
    if world.resource::<ClientFlow>().save_complete {
        let stream_done = world.resource_mut::<streaming::StreamState>().drain();
        let legacy_done = world.resource_mut::<scene::LegacyMeshing>().drain();
        if stream_done && legacy_done {
            let destination = world.resource::<ClientFlow>().destination;
            cleanup(world);
            *world.resource_mut::<ClientFlow>() = ClientFlow::default();
            if destination == Some(Destination::Quit) {
                world.write_message(AppExit::Success);
            }
        }
    }
}
fn modal_input(
    mut flow: ResMut<ClientFlow>,
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    inventory: Res<inventory_ui::GameInventory>,
    mut close: MessageReader<WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    if close.read().count() > 0 {
        match flow.state {
            ClientState::Title => {
                exit.write(AppExit::Success);
            }
            ClientState::Loading => {
                flow.destination = Some(Destination::Quit);
            } // wait for lock owner
            ClientState::RecoverableError if flow.destination.is_none() => {
                exit.write(AppExit::Success);
            }
            _ => flow.save(Destination::Quit),
        }
    }
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if flow.playing()
        && (!window.focused && !inventory.open
            || (keys.just_pressed(KeyCode::Escape) && !inventory.open && !inventory.transition))
    {
        flow.state = ClientState::Paused;
    } else if flow.state == ClientState::Paused
        && window.focused
        && keys.just_pressed(KeyCode::Escape)
    {
        flow.state = ClientState::Playing;
        flow.resume_frame = true;
    }
    if !flow.playing() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
pub struct SessionPlugin;
impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientFlow>()
            .init_schedule(SessionSetup)
            .add_systems(
                RunFixedMainLoop,
                modal_input
                    .in_set(FlowInput)
                    .after(inventory_ui::InventoryInput)
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(Last, advance);
    }
}
#[cfg(test)]
mod tests;
