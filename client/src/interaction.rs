//! Input adapter and procedural feedback; all authoritative edit rules live in
//! voxel_core. Editing runs once per render frame, after the displayed eye pose.
use crate::{
    camera::{CameraSetup, FirstPersonCamera, ViewPose, ViewSync},
    player::{ControllerInput, PlayerSettings, PlayerState},
    scene::{DirtyChunks, RenderOrigin, VoxelWorld},
};
use bevy::{
    asset::RenderAssetUsages, ecs::system::SystemParam, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology, window::PrimaryWindow,
};
use voxel_core::{
    interaction::{BuildBlock, EditAction, EditError},
    raycast::{DEFAULT_REACH, Ray, RayHit, raycast},
};

pub struct BlockInteractionPlugin;
impl Plugin for BlockInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Editor>()
            .init_resource::<crate::inventory_ui::GameInventory>()
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
    last_selection: Option<(usize, Option<voxel_core::inventory::Item>)>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            selected: BuildBlock::default(),
            target: None,
            reach: DEFAULT_REACH,
            rejection: None,
            last_selection: None,
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
    bindings: Res<'w, crate::controls::MovementBindings>,
    view: Res<'w, ViewPose>,
    editor: ResMut<'w, Editor>,
    inventory: ResMut<'w, crate::inventory_ui::GameInventory>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, Entity, With<FirstPersonCamera>>,
}

fn edit_and_target(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ctx: EditContext,
) {
    let active = ctx.control.active
        && !ctx.player.awaiting_spawn
        && ctx.cameras.single().is_ok()
        && !ctx.inventory.open
        && !ctx.inventory.transition;
    if active {
        for (key, slot) in [
            (KeyCode::Digit1, 1),
            (KeyCode::Digit2, 2),
            (KeyCode::Digit3, 3),
            (KeyCode::Digit4, 4),
            (KeyCode::Digit5, 5),
            (KeyCode::Digit6, 6),
            (KeyCode::Digit7, 7),
            (KeyCode::Digit8, 8),
            (KeyCode::Digit9, 9),
        ] {
            if keyboard.just_pressed(key) {
                ctx.inventory.contents.select(slot - 1);
            }
        }
    }
    if let Some(stack) = ctx.inventory.contents.selected_stack() {
        ctx.editor.selected = stack.item().build();
    }
    let selection = (
        ctx.inventory.contents.selected(),
        ctx.inventory.contents.selected_stack().map(|s| s.item()),
    );
    if ctx.editor.last_selection != Some(selection)
        && let Ok(mut window) = ctx.windows.single_mut()
    {
        window.title = format!(
            "RustCraft | Slot: {} {} | 1–9 / wheel | E inventory | LMB break | RMB place | F5 save | Esc release | R respawn",
            ctx.inventory.contents.selected() + 1,
            ctx.inventory
                .contents
                .selected_stack()
                .map_or("Empty", |s| s.item().build().name())
        );
        ctx.editor.last_selection = Some(selection);
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
        || keyboard.just_pressed(ctx.bindings.respawn);
    if let Some(action) = action.filter(|_| !blocked) {
        let config = ctx.settings.0;
        let body = ctx.player.body;
        // Also protect the displayed/interpolated body during fast falls. The
        // core independently protects the current authoritative physics AABB.
        let mut displayed_body = body;
        displayed_body.position = ctx.view.feet;
        displayed_body.crouched = ctx.view.crouched;
        let overlaps_view = matches!(action, EditAction::Place(_))
            && raycast(&ctx.world.0, ray)
                .and_then(|hit| hit.adjacent)
                .is_some_and(|p| displayed_body.intersects_block(p, &config));
        let result = if overlaps_view {
            Err(EditError::IntersectsPlayer)
        } else {
            match voxel_core::inventory::edit_inventory(
                &mut ctx.world.0,
                &mut ctx.inventory.contents,
                ray,
                matches!(action, EditAction::Place(_)),
                &body,
                &config,
            ) {
                Ok(v) => Ok(v),
                Err(voxel_core::inventory::InventoryEditError::World(e)) => Err(e),
                Err(voxel_core::inventory::InventoryEditError::Full) => {
                    ctx.inventory.message = "NO ROOM";
                    Err(EditError::InventoryFull)
                }
                Err(voxel_core::inventory::InventoryEditError::Empty) => Err(EditError::EmptySlot),
            }
        };
        ctx.editor.rejection = result.as_ref().err().copied();
        if let Ok(Some(change)) = result {
            ctx.inventory.message = "";
            ctx.dirty.mark(change);
        }
    }
    ctx.editor.target = raycast(&ctx.world.0, ray).filter(|hit| ctx.world.0.ready(hit.position));
}

#[derive(Component)]
struct TargetHighlight;
#[derive(Component)]
struct OverlayRoot;

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
    });
}

type OverlayFilter = (With<OverlayRoot>, Without<TargetHighlight>);
#[derive(SystemParam)]
struct VisualState<'w> {
    editor: Res<'w, Editor>,
    control: Res<'w, ControllerInput>,
    origin: Res<'w, RenderOrigin>,
}
fn update_visuals(
    state: VisualState,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Projection, With<FirstPersonCamera>>,
    mut highlights: Query<(&mut Transform, &mut Visibility), With<TargetHighlight>>,
    mut overlays: Query<(&mut Transform, &mut Visibility), OverlayFilter>,
) {
    for (mut transform, mut visibility) in &mut highlights {
        if let Some(hit) = state.editor.target {
            *transform = Transform::from_translation(Vec3::from_array(std::array::from_fn(|a| {
                (f64::from(hit.position.0[a]) - state.origin.0[a]) as f32 - 0.002
            })))
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
            *visibility = if state.control.active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

#[cfg(test)]
mod tests;
