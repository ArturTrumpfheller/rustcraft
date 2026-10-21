mod camera;
mod interaction;
mod player;
mod scene;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.92)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustCraft — Click: capture | Esc: release | WASD: move | Space: jump | Shift: sprint | R: respawn".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((scene::VoxelScenePlugin, player::PlayerPlugin, camera::FirstPersonPlugin, interaction::BlockInteractionPlugin))
        .run();
}
