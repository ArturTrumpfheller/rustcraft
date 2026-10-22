//! Original 5x7 bitmap lettering rendered as a tiny camera-relative mesh. No
//! font files, UI subsystem, texture assets or additional dependencies.
use crate::{
    camera::{CameraSetup, FirstPersonCamera},
    persistence::SaveFeedback,
};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology, window::PrimaryWindow,
};

pub struct SaveHudPlugin;
impl Plugin for SaveHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup.after(CameraSetup))
            .add_systems(PostUpdate, update);
    }
}
#[derive(Component)]
struct SaveHud {
    text: String,
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        '5' => [31, 16, 16, 30, 1, 17, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        _ => [0; 7],
    }
}
fn message(label: &str) -> (&'static str, [f32; 4]) {
    match label {
        "Saving" => ("SAVING...", [1.0, 0.8, 0.1, 1.0]),
        "Saved" => ("SAVED", [0.3, 1.0, 0.35, 1.0]),
        "ERROR / DIRTY" => ("SAVE ERROR - F5 RETRY", [1.0, 0.2, 0.15, 1.0]),
        "Dirty" => ("UNSAVED - F5 SAVE", [1.0, 0.8, 0.1, 1.0]),
        "Loaded" => ("LOADED", [0.8, 0.95, 1.0, 1.0]),
        _ => ("NEW WORLD", [0.8, 0.95, 1.0, 1.0]),
    }
}
fn text_mesh(text: &str, color: [f32; 4]) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let mut quad = |x: f32, y: f32, w: f32, h: f32, z: f32, c: [f32; 4]| {
        let base = positions.len() as u32;
        positions.extend([[x, y, z], [x + w, y, z], [x + w, y + h, z], [x, y + h, z]]);
        colors.extend([c; 4]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    quad(
        -8.,
        -8.,
        text.len() as f32 * 18. + 16.,
        37.,
        -0.5,
        [0.012, 0.018, 0.025, 1.],
    );
    for (i, c) in text.chars().enumerate() {
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    quad(
                        i as f32 * 18. + col as f32 * 3.,
                        (6 - row) as f32 * 3.,
                        3.,
                        3.,
                        0.,
                        color,
                    );
                }
            }
        }
    }
    let normals = vec![[0., 0., 1.]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<Entity, With<FirstPersonCamera>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let child = commands
        .spawn((
            SaveHud {
                text: String::new(),
            },
            Mesh3d(meshes.add(text_mesh("NEW WORLD", [1.; 4]))),
            MeshMaterial3d(materials.add(StandardMaterial {
                unlit: true,
                ..default()
            })),
            Transform::default(),
            Visibility::Inherited,
        ))
        .id();
    commands.entity(camera).add_child(child);
}
fn update(
    feedback: Res<SaveFeedback>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Projection, With<FirstPersonCamera>>,
    mut hud: Query<(&mut SaveHud, &Mesh3d, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (text, color) = message(&feedback.label);
    for (mut state, handle, mut transform) in &mut hud {
        if state.text != text {
            if let Some(mut mesh) = meshes.get_mut(&handle.0) {
                *mesh = text_mesh(text, color);
            }
            state.text = text.into();
        }
        if let (Ok(window), Ok(Projection::Perspective(projection))) =
            (windows.single(), cameras.single())
        {
            let scale = 2. * 0.15 * (projection.fov * 0.5).tan() / window.height().max(1.);
            // Top left in logical pixels; independent of pointer capture/pause.
            *transform = Transform::from_xyz(
                (-window.width() * 0.5 + 24.) * scale,
                (window.height() * 0.5 - 50.) * scale,
                -0.15,
            )
            .with_scale(Vec3::splat(scale));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ecs_status_is_visible_without_capture_and_reuses_assets_through_errors_and_resize() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<SaveFeedback>()
            .add_plugins(SaveHudPlugin);
        app.world_mut().spawn((
            FirstPersonCamera,
            Projection::Perspective(PerspectiveProjection::default()),
            Transform::default(),
        ));
        app.world_mut().spawn((
            PrimaryWindow,
            Window {
                resolution: (1280, 720).into(),
                ..default()
            },
        ));
        app.update();
        for _ in 0..10 {
            for label in ["Saving", "Saved", "ERROR / DIRTY", "Dirty", "Loaded"] {
                app.world_mut().resource_mut::<SaveFeedback>().label = label.into();
                app.update();
                let world = app.world_mut();
                let (state, handle, visibility, transform) = world
                    .query::<(&SaveHud, &Mesh3d, &Visibility, &Transform)>()
                    .single(world)
                    .unwrap();
                assert_eq!(state.text, message(label).0);
                assert_eq!(*visibility, Visibility::Inherited);
                assert!(transform.translation.x < 0. && transform.translation.y > 0.);
                assert!(
                    world
                        .resource::<Assets<Mesh>>()
                        .get(&handle.0)
                        .unwrap()
                        .count_vertices()
                        > 4
                );
            }
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
        app.world_mut()
            .query::<&mut Window>()
            .single_mut(app.world_mut())
            .unwrap()
            .resolution
            .set(800., 600.);
        app.update();
        let world = app.world_mut();
        assert!(world.query::<&SaveHud>().single(world).is_ok());
    }
    #[test]
    fn all_status_messages_have_drawn_glyphs_and_distinct_success_error_colors() {
        for state in [
            "Saving",
            "Saved",
            "ERROR / DIRTY",
            "Dirty",
            "Loaded",
            "New world",
        ] {
            let (text, color) = message(state);
            for c in text.chars().filter(|c| *c != ' ') {
                assert_ne!(glyph(c), [0; 7], "missing {c}");
            }
            let mesh = text_mesh(text, color);
            assert!(mesh.count_vertices() > 4);
        }
        assert_ne!(message("Saved").1, message("ERROR / DIRTY").1);
    }
}
