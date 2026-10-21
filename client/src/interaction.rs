//! Input adapter and procedural feedback; all authoritative edit rules live in
//! voxel_core. Editing runs once per render frame, after the displayed eye pose.
use crate::{
    camera::{CameraSetup, FirstPersonCamera, ViewPose, ViewSync},
    player::{ControllerInput, PlayerSettings, PlayerState},
    scene::{DirtyChunks, VoxelWorld},
};
use bevy::{
    asset::RenderAssetUsages, ecs::system::SystemParam, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology, window::PrimaryWindow,
};
use voxel_core::{
    interaction::{BuildBlock, EditAction, EditError, edit},
    physics::Player,
    raycast::{DEFAULT_REACH, Ray, RayHit, raycast},
};

pub struct BlockInteractionPlugin;
impl Plugin for BlockInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Editor>()
            .add_systems(Startup, setup_visuals.after(CameraSetup))
            .add_systems(
                RunFixedMainLoop,
                (edit_and_target, update_visuals)
                    .chain()
                    .after(ViewSync)
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

#[derive(Resource)]
struct Editor {
    selected: BuildBlock,
    target: Option<RayHit>,
    reach: f64,
    rejection: Option<EditError>,
    title_initialized: bool,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            selected: BuildBlock::default(),
            target: None,
            reach: DEFAULT_REACH,
            rejection: None,
            title_initialized: false,
        }
    }
}

#[derive(SystemParam)]
struct EditContext<'w, 's> {
    world: ResMut<'w, VoxelWorld>,
    dirty: ResMut<'w, DirtyChunks>,
    player: Res<'w, PlayerState>,
    settings: Res<'w, PlayerSettings>,
    control: Res<'w, ControllerInput>,
    view: Res<'w, ViewPose>,
    editor: ResMut<'w, Editor>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, Entity, With<FirstPersonCamera>>,
}

fn edit_and_target(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ctx: EditContext,
) {
    let old_selection = ctx.editor.selected;
    let active = ctx.control.active && ctx.cameras.single().is_ok();
    if active {
        for (key, slot) in [
            (KeyCode::Digit1, 1),
            (KeyCode::Digit2, 2),
            (KeyCode::Digit3, 3),
        ] {
            if keyboard.just_pressed(key) {
                ctx.editor.selected = BuildBlock::from_slot(slot).expect("valid selection slot");
            }
        }
    }
    if (!ctx.editor.title_initialized || old_selection != ctx.editor.selected)
        && let Ok(mut window) = ctx.windows.single_mut()
    {
        window.title = format!(
            "RustCraft | Block: {} {} | 1/2/3 select | LMB break | RMB place | Esc release | R respawn",
            ctx.editor.selected.slot(),
            ctx.editor.selected.name()
        );
        ctx.editor.title_initialized = true;
    }
    if !active {
        ctx.editor.target = None;
        return;
    }
    let ray = Ray {
        origin: ctx.view.eye,
        direction: ctx.view.direction,
        reach: ctx.editor.reach,
    };
    let action = if mouse.just_pressed(MouseButton::Left) {
        Some(EditAction::Break)
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(EditAction::Place(ctx.editor.selected))
    } else {
        None
    };
    let blocked = ctx.control.captured_this_frame
        || ctx.control.reset_pending
        || keyboard.just_pressed(KeyCode::KeyR);
    if let Some(action) = action.filter(|_| !blocked) {
        let config = ctx.settings.0;
        let body = ctx.player.body;
        // Also protect the displayed/interpolated body during fast falls. The
        // core independently protects the current authoritative physics AABB.
        let displayed_body = Player::new([
            ray.origin[0],
            ray.origin[1] - config.eye_height,
            ray.origin[2],
        ]);
        let overlaps_view = matches!(action, EditAction::Place(_))
            && raycast(&ctx.world.0, ray)
                .and_then(|hit| hit.adjacent)
                .is_some_and(|p| displayed_body.intersects_block(p, &config));
        let result = if overlaps_view {
            Err(EditError::IntersectsPlayer)
        } else {
            edit(&mut ctx.world.0, ray, action, &body, &config)
        };
        ctx.editor.rejection = result.as_ref().err().copied();
        if let Ok(Some(change)) = result {
            ctx.dirty.0.extend(change.affected_chunks());
        }
    }
    ctx.editor.target = raycast(&ctx.world.0, ray);
}

#[derive(Component)]
struct TargetHighlight;
#[derive(Component)]
struct OverlayRoot;
#[derive(Component)]
struct SelectionMarker;

fn outline_mesh() -> Mesh {
    let corners: Vec<[f32; 3]> = (0..8)
        .map(|i| [(i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32])
        .collect();
    let indices: Vec<u32> = (0..8)
        .flat_map(|i| {
            [1, 2, 4].into_iter().filter_map(move |bit| {
                if i & bit == 0 {
                    Some([i, i | bit])
                } else {
                    None
                }
            })
        })
        .flatten()
        .collect();
    Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 8])
    .with_inserted_indices(Indices::U32(indices))
}

fn setup_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<Entity, With<FirstPersonCamera>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        ..default()
    });
    commands.spawn((
        TargetHighlight,
        Mesh3d(meshes.add(outline_mesh())),
        MeshMaterial3d(black),
        Visibility::Hidden,
    ));
    let crosshair = meshes.add(Rectangle::new(12.0, 2.0));
    let square = meshes.add(Rectangle::new(12.0, 12.0));
    let marker = meshes.add(Rectangle::new(16.0, 16.0));
    let overlay = commands
        .spawn((
            OverlayRoot,
            Transform::from_xyz(0.0, 0.0, -0.2),
            Visibility::Hidden,
        ))
        .id();
    commands.entity(camera).add_child(overlay);
    commands.entity(overlay).with_children(|parent| {
        parent.spawn((Mesh3d(crosshair.clone()), MeshMaterial3d(white.clone())));
        parent.spawn((
            Mesh3d(crosshair),
            MeshMaterial3d(white.clone()),
            Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
        parent.spawn((
            SelectionMarker,
            Mesh3d(marker),
            MeshMaterial3d(white),
            Transform::from_xyz(-20.0, -24.0, -0.1),
        ));
        for slot in 1..=3 {
            let color = BuildBlock::from_slot(slot).unwrap().block().color();
            let material = materials.add(StandardMaterial {
                base_color: Color::linear_rgba(color[0], color[1], color[2], color[3]),
                unlit: true,
                ..default()
            });
            parent.spawn((
                Mesh3d(square.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz((f32::from(slot) - 2.0) * 20.0, -24.0, 0.0),
            ));
        }
    });
}

type OverlayFilter = (With<OverlayRoot>, Without<TargetHighlight>);
type MarkerFilter = (
    With<SelectionMarker>,
    Without<TargetHighlight>,
    Without<OverlayRoot>,
);

fn update_visuals(
    editor: Res<Editor>,
    control: Res<ControllerInput>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Projection, With<FirstPersonCamera>>,
    mut highlights: Query<(&mut Transform, &mut Visibility), With<TargetHighlight>>,
    mut overlays: Query<(&mut Transform, &mut Visibility), OverlayFilter>,
    mut markers: Query<&mut Transform, MarkerFilter>,
) {
    for (mut transform, mut visibility) in &mut highlights {
        if let Some(hit) = editor.target {
            *transform = Transform::from_translation(Vec3::from_array(
                hit.position.0.map(|v| v as f32 - 0.002),
            ))
            .with_scale(Vec3::splat(1.004));
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    if let (Ok(window), Ok(Projection::Perspective(projection))) =
        (windows.single(), cameras.single())
    {
        let pixel_scale = 2.0 * 0.2 * (projection.fov * 0.5).tan() / window.height().max(1.0);
        for (mut transform, mut visibility) in &mut overlays {
            transform.scale = Vec3::splat(pixel_scale);
            *visibility = if control.active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    for mut transform in &mut markers {
        transform.translation.x = (f32::from(editor.selected.slot()) - 2.0) * 20.0;
    }
}

#[cfg(test)]
mod tests;
