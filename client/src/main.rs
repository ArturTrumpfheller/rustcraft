mod camera;
mod controls;
mod interaction;
mod inventory_ui;
mod persistence;
mod player;
mod save_hud;
mod scene;
mod streaming;
mod terrain_texture;
mod viewmodel;

use bevy::prelude::*;

fn main() -> std::process::ExitCode {
    let directory = match persistence::data_directory() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("RustCraft data path error: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    eprintln!(
        "RustCraft world save: {}",
        directory.join("world.rcw").display()
    );
    let (store, mut world, source) = match voxel_core::persistence::SaveStore::open_with(
        &directory,
        persistence::create_world,
    ) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!(
                "RustCraft could not load world: {error}. Preserve the whole data directory and recover a complete consistent backup (see README); do not restore an individual world/player file."
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    let inventory = match store.load_player(&mut world) {
        Ok(i) => i,
        Err(e) => {
            eprintln!(
                "RustCraft player/session recovery error: {e}; preserve the whole data directory"
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    eprintln!("RustCraft world source: {source:?}");
    if source == voxel_core::persistence::LoadSource::Saved {
        eprintln!(
            "RustCraft loaded existing snapshot unchanged; world seed overrides do not apply."
        );
    }
    let start = if world.region_identity().is_some() {
        match player::start_column(
            std::env::var_os("RUSTCRAFT_START_CHUNK"),
            world.bounds().1[0] / 16,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("RustCraft start position error: {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let column = start.unwrap_or([0, 0]);
    if let Err(e) = store.prepare_regional_column(&mut world, column) {
        eprintln!("RustCraft spawn region load failed: {e}; files preserved");
        return std::process::ExitCode::FAILURE;
    }
    let stream = if world.identity().is_some() {
        let prepared = std::time::Instant::now();
        let generator = match voxel_core::streaming::prepare_column(&mut world, column) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("RustCraft generation error: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        let radius = match streaming::radius_from(std::env::var_os("RUSTCRAFT_LOAD_RADIUS")) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("RustCraft radius error: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        eprintln!(
            "RustCraft mode: streamed {}x{}, seed {}, generator v{}, radius {radius}",
            world.bounds().1[0] * 2,
            world.bounds().1[2] * 2,
            world.identity().unwrap().seed,
            world.identity().unwrap().generator
        );
        eprintln!(
            "RustCraft spawn prepared: {} chunks in {:?}",
            world.chunks().count(),
            prepared.elapsed()
        );
        streaming::StreamState::new(generator, radius)
    } else {
        eprintln!("RustCraft mode: legacy finite snapshot (64x64); no generation or eviction");
        streaming::StreamState::default()
    };
    let session = persistence::SaveSession::with_player(store, &world, source, &inventory);
    let preferred = start.map_or(player::SPAWN, |c| {
        [
            f64::from(c[0]) * 16. + 8.5,
            32.,
            f64::from(c[1]) * 16. + 8.5,
        ]
    });
    App::new().insert_resource(inventory_ui::GameInventory::loaded(inventory)).insert_resource(player::InitialSpawn(preferred))
        .insert_resource(scene::VoxelWorld(world))
        .insert_resource(session)
        .insert_resource(stream)
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            close_when_requested: false,
            primary_window: Some(Window {
                title: "RustCraft — Click: capture | Esc: release | WASD: move | Space: jump | Ctrl: sprint | Shift: sneak | R: respawn".into(),
                resolution: (1280, 720).into(),
                resize_constraints: WindowResizeConstraints {min_width:320.,min_height:240.,..default()},
                ..default()
            }),
            ..default()
        }))
        .add_plugins((scene::VoxelScenePlugin, inventory_ui::InventoryPlugin, player::PlayerPlugin, camera::FirstPersonPlugin, interaction::BlockInteractionPlugin, persistence::PersistencePlugin, save_hud::SaveHudPlugin, streaming::StreamingPlugin, viewmodel::ViewmodelPlugin))
        .run();
    std::process::ExitCode::SUCCESS
}
