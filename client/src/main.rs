mod camera;
mod interaction;
mod persistence;
mod player;
mod save_hud;
mod scene;
mod streaming;

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
                "RustCraft could not load world: {error}. No world file was overwritten. If available, preserve the damaged primary and recover world.rcw.bak explicitly (see README)."
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
    let stream = if world.identity().is_some() {
        let prepared = std::time::Instant::now();
        let generator = match voxel_core::streaming::prepare_spawn(&mut world) {
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
            "RustCraft mode: streamed 128x128, seed {}, generator v2, radius {radius}",
            world.identity().unwrap().seed
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
    let session = persistence::SaveSession::new(store, &world, source);
    App::new()
        .insert_resource(scene::VoxelWorld(world))
        .insert_resource(session)
        .insert_resource(stream)
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            close_when_requested: false,
            primary_window: Some(Window {
                title: "RustCraft — Click: capture | Esc: release | WASD: move | Space: jump | Shift: sprint | R: respawn".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((scene::VoxelScenePlugin, player::PlayerPlugin, camera::FirstPersonPlugin, interaction::BlockInteractionPlugin, persistence::PersistencePlugin, save_hud::SaveHudPlugin, streaming::StreamingPlugin))
        .run();
    std::process::ExitCode::SUCCESS
}
