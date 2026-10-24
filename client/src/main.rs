mod camera;
mod controls;
mod interaction;
mod inventory_ui;
mod menu;
mod persistence;
mod player;
mod save_hud;
mod scene;
mod session;
mod settings;
mod streaming;
mod terrain_texture;
mod viewmodel;

use bevy::prelude::*;

fn main() {
    let (config, preferences) = settings::Config::load();
    if !config.error.is_empty() {
        eprintln!("RustCraft settings: {}", config.error);
    }
    let mut window = Window {
        title: "RustCraft".into(),
        ..default()
    };
    preferences.window(&mut window);
    App::new()
        .insert_resource(controls::MovementBindings::from_keys(preferences.keys))
        .insert_resource(preferences)
        .insert_resource(config)
        .init_resource::<session::ClientFlow>()
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            close_when_requested: false,
            primary_window: Some(Window {
                resize_constraints: WindowResizeConstraints {
                    min_width: 320.,
                    min_height: 240.,
                    ..default()
                },
                ..window
            }),
            ..default()
        }))
        .add_plugins((
            scene::VoxelScenePlugin,
            inventory_ui::InventoryPlugin,
            player::PlayerPlugin,
            camera::FirstPersonPlugin,
            interaction::BlockInteractionPlugin,
            persistence::PersistencePlugin,
            save_hud::SaveHudPlugin,
            streaming::StreamingPlugin,
            viewmodel::ViewmodelPlugin,
            session::SessionPlugin,
            menu::MenuPlugin,
        ))
        .run();
}
