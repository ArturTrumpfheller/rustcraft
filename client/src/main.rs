mod camera;
mod scene;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustCraft — Click: capture | Esc: release | WASD: move | Space/Ctrl: fly | R: reset".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((scene::VoxelScenePlugin, camera::FirstPersonPlugin))
        .run();
}
