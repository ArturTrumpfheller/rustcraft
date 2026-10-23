//! Original retained first-person meshes and motion; never part of simulation.
use crate::{
    camera::{CameraSetup, FirstPersonCamera, ViewPose, ViewSync},
    controls::MovementBindings,
    inventory_ui::GameInventory,
    player::{ControllerInput, PlayerState},
    scene::{SceneSetup, TerrainMaterial},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{Camera3dDepthLoadOp, visibility::RenderLayers},
    ecs::system::SystemParam,
    mesh::VertexAttributeValues,
    prelude::*,
    render::view::Tonemapping,
};
use voxel_core::{Block, inventory::Item, texture::face_tile};

pub(crate) const OVERLAY_LAYER: usize = 1;

/// Written only by the existing authoritative gameplay-click gate. A serial
/// replaces queued animation events: repeated actions restart one bounded swing.
#[derive(Resource, Default)]
pub(crate) struct HandAction {
    pub serial: u64,
    pub place: bool,
}
impl HandAction {
    pub fn trigger(&mut self, place: bool) {
        self.serial = self.serial.wrapping_add(1);
        self.place = place;
    }
}

pub struct ViewmodelPlugin;
impl Plugin for ViewmodelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HandAction>()
            .add_systems(Startup, setup.after(CameraSetup).after(SceneSetup))
            .add_systems(
                RunFixedMainLoop,
                update
                    .after(ViewSync)
                    .after(crate::interaction::InteractionUpdate)
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

#[derive(Component)]
struct OverlayCamera;
#[derive(Component)]
struct HandRoot;
#[derive(Component)]
struct HeldBlock;
#[derive(Resource)]
struct Models {
    blocks: [Handle<Mesh>; 5],
    root: Entity,
    block: Entity,
    selected: Option<Item>,
    motion: Motion,
}

// Cuboid topology comes from Bevy; art UVs use the existing original atlas.
fn cube(block: Option<Block>) -> Mesh {
    let mut mesh = Mesh::from(Cuboid::new(1., 1., 1.));
    let VertexAttributeValues::Float32x3(positions) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        unreachable!()
    };
    let VertexAttributeValues::Float32x3(normals) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
    else {
        unreachable!()
    };
    let mut uvs = Vec::with_capacity(24);
    let mut colors = Vec::with_capacity(24);
    for (p, n) in positions.iter().zip(normals) {
        let [x, y, z] = p.map(|v| (v + 0.5) as u8);
        let normal = n.map(|v| v as i32);
        let local = match normal {
            [1, 0, 0] => [1 - z, 1 - y],
            [-1, 0, 0] => [z, 1 - y],
            [0, 1, 0] => [x, z],
            [0, -1, 0] => [x, 1 - z],
            [0, 0, 1] => [x, 1 - y],
            _ => [1 - x, 1 - y],
        };
        uvs.push(
            block
                .and_then(|b| face_tile(b, normal))
                .map_or([0.; 2], |t| t.uv(local)),
        );
        let shade = if n[1] > 0. {
            1.
        } else if n[1] < 0. {
            0.55
        } else if n[0] != 0. {
            0.8
        } else {
            0.68
        };
        // Muted constant fill: readable in caves without adding lights/shadows.
        colors.push([shade * 0.55, shade * 0.55, shade * 0.55, 1.]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
    mesh
}

fn arm_mesh() -> Mesh {
    // Four original boxes: forearm, cuff, palm and side thumb, one draw call.
    let parts = [
        (
            Vec3::new(0.28, 0.7, 0.30),
            Vec3::new(0., -0.24, 0.14),
            [0.62, 0.43, 0.30],
        ),
        (
            Vec3::new(0.30, 0.18, 0.32),
            Vec3::new(0., -0.32, 0.14),
            [0.19, 0.28, 0.34],
        ),
        (
            Vec3::new(0.32, 0.29, 0.33),
            Vec3::new(-0.03, 0.16, 0.06),
            [0.69, 0.49, 0.34],
        ),
        (
            Vec3::new(0.11, 0.20, 0.17),
            Vec3::new(-0.20, 0.14, -0.02),
            [0.64, 0.44, 0.30],
        ),
    ];
    let mut combined: Option<Mesh> = None;
    for (size, offset, color) in parts {
        let mut part = cube(None);
        if let Some(VertexAttributeValues::Float32x3(p)) =
            part.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for v in p {
                *v = (Vec3::from_array(*v) * size + offset).to_array();
            }
        }
        if let Some(VertexAttributeValues::Float32x4(c)) = part.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            let linear = Color::srgb(color[0], color[1], color[2])
                .to_linear()
                .to_f32_array();
            for v in c {
                for a in 0..3 {
                    v[a] *= linear[a] / 0.55;
                }
            }
        }
        if let Some(m) = &mut combined {
            m.merge(&part).expect("identical cuboid attributes");
        } else {
            combined = Some(part);
        }
    }
    combined.expect("four authored cuboids")
}

fn setup(
    mut commands: Commands,
    camera: Query<Entity, With<FirstPersonCamera>>,
    terrain: Res<TerrainMaterial>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let overlay = commands
        .spawn((
            OverlayCamera,
            Camera3d {
                depth_load_op: Camera3dDepthLoadOp::Clear(0.),
                ..default()
            },
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            RenderLayers::layer(OVERLAY_LAYER),
            Tonemapping::Reinhard,
            Transform::default(),
        ))
        .id();
    commands.entity(camera).add_child(overlay);
    let atlas = materials
        .get(&terrain.0)
        .unwrap()
        .base_color_texture
        .clone();
    let skin = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    let textured = materials.add(StandardMaterial {
        unlit: true,
        base_color_texture: atlas,
        ..default()
    });
    let blocks = [
        Block::Grass,
        Block::Dirt,
        Block::Stone,
        Block::Wood,
        Block::Leaves,
    ]
    .map(|b| meshes.add(cube(Some(b))));
    let root = commands
        .spawn((HandRoot, Transform::default(), Visibility::Hidden))
        .id();
    commands.entity(overlay).add_child(root);
    let arm = commands
        .spawn((
            Mesh3d(meshes.add(arm_mesh())),
            MeshMaterial3d(skin),
            Visibility::Inherited,
            RenderLayers::layer(OVERLAY_LAYER),
        ))
        .id();
    let block = commands
        .spawn((
            HeldBlock,
            Visibility::Hidden,
            Mesh3d(blocks[0].clone()),
            MeshMaterial3d(textured),
            RenderLayers::layer(OVERLAY_LAYER),
            Transform::from_xyz(-0.19, 0.41, -0.17)
                .with_scale(Vec3::splat(0.67))
                .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.15, -0.36, 0.)),
        ))
        .id();
    commands.entity(root).add_children(&[arm, block]);
    commands.insert_resource(Models {
        blocks,
        root,
        block,
        selected: None,
        motion: Motion::default(),
    });
}

#[derive(Default)]
struct Motion {
    previous: Option<[f64; 3]>,
    previous_vy: f64,
    phase: f32,
    bob: f32,
    crouch: f32,
    landing: f32,
    swing: f32,
    swing_active: bool,
    swing_start: f32,
    swing_time: f32,
    swing_strength: f32,
    strength_target: f32,
    serial: u64,
}
impl Motion {
    fn reset(&mut self, feet: [f64; 3], serial: u64) {
        *self = Self {
            previous: Some(feet),
            serial,
            ..Default::default()
        };
    }
    fn step(
        &mut self,
        dt: f32,
        feet: [f64; 3],
        body: &voxel_core::physics::Player,
        action: &HandAction,
    ) -> (Vec3, Quat) {
        let vy = body.velocity[1];
        let grounded = body.grounded;
        let crouched = body.crouched;
        let previous = self.previous.replace(feet).unwrap_or(feet);
        let distance = (feet[0] - previous[0]).hypot(feet[2] - previous[2]) as f32;
        if distance > 4. || (feet[1] - previous[1]).abs() > 4. {
            self.reset(feet, action.serial);
        }
        let speed = if dt > 0. { distance / dt } else { 0. };
        let dt = dt.min(0.05);
        let blend = 1. - (-12. * dt).exp();
        let amount = if grounded {
            (speed / 5.).min(1.8) * if crouched { 0.35 } else { 1. }
        } else {
            0.
        };
        if grounded && distance <= 4. {
            self.phase = (self.phase + distance * 2.) % std::f32::consts::TAU;
        }
        self.bob += (amount - self.bob) * blend;
        self.crouch += (f32::from(crouched) - self.crouch) * blend;
        if self.previous_vy < -0.5 && (grounded || vy > 0.) {
            self.landing = 0.045;
        }
        self.previous_vy = vy;
        self.landing *= (-14. * dt).exp();
        if action.serial != self.serial {
            self.serial = action.serial;
            self.swing_active = true;
            self.swing_start = self.swing;
            self.swing_time = 0.;
            self.strength_target = if action.place { 0.55 } else { 1. };
        }
        self.swing_time += dt;
        let smooth = |t: f32| {
            let t = t.clamp(0., 1.);
            t * t * (3. - 2. * t)
        };
        self.swing = if self.swing_time < 0.06 {
            self.swing_start + (1. - self.swing_start) * smooth(self.swing_time / 0.06)
        } else {
            1. - smooth((self.swing_time - 0.06) / 0.18)
        };
        // No initial/no-action swing, including fresh capture.
        if !self.swing_active || self.swing_time >= 0.24 {
            self.swing = 0.;
            self.swing_active = false;
        }
        self.swing_strength += (self.strength_target - self.swing_strength) * blend;
        let swing = self.swing * self.swing_strength;
        let offset = Vec3::new(
            self.phase.sin() * self.bob * 0.018 - swing * 0.10,
            (self.phase * 2.).cos() * self.bob * 0.010
                - self.crouch * 0.07
                - self.landing
                - (vy as f32 * 0.003).clamp(-0.035, 0.035)
                + swing * 0.10,
            -swing * 0.18,
        );
        (
            offset,
            Quat::from_euler(
                EulerRot::XYZ,
                -0.12 - swing * 0.32,
                -0.22 - swing * 0.28,
                -0.12 + self.phase.sin() * self.bob * 0.025 + swing * 0.18,
            ),
        )
    }
}

#[derive(SystemParam)]
struct Frame<'w> {
    time: Res<'w, Time<Real>>,
    player: Res<'w, PlayerState>,
    view: Res<'w, ViewPose>,
    input: Res<'w, ControllerInput>,
    inventory: Res<'w, GameInventory>,
    action: Res<'w, HandAction>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    bindings: Res<'w, MovementBindings>,
}
type HeldFilter = (With<HeldBlock>, Without<HandRoot>);

fn update(
    frame: Frame,
    models: Option<ResMut<Models>>,
    source: Query<&Projection, With<FirstPersonCamera>>,
    mut overlay: Query<&mut Projection, (With<OverlayCamera>, Without<FirstPersonCamera>)>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<HandRoot>>,
    mut blocks: Query<(&mut Mesh3d, &mut Visibility), HeldFilter>,
) {
    let (Some(mut models), Ok(Projection::Perspective(p))) = (models, source.single()) else {
        return;
    };
    for mut projection in &mut overlay {
        let mut next = p.clone();
        next.near = 0.01;
        next.far = 10.;
        if !matches!(&*projection, Projection::Perspective(old) if old.fov == next.fov && old.aspect_ratio == next.aspect_ratio && old.near == next.near && old.far == next.far)
        {
            *projection = Projection::Perspective(next);
        }
    }
    let selected = frame.inventory.contents.selected_stack().map(|s| s.item());
    if let Ok((mut handle, mut visibility)) = blocks.get_mut(models.block) {
        *visibility = if selected.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if selected != models.selected {
            if let Some(item) = selected {
                handle.0 = models.blocks[usize::from(item.id() - 1)].clone();
            }
            models.selected = selected;
        }
    }
    let active = frame.input.active
        && !frame.input.captured_this_frame
        && !frame.inventory.open
        && !frame.inventory.transition
        && !frame.player.awaiting_spawn
        && !frame.keys.just_pressed(frame.bindings.respawn);
    let Ok((mut transform, mut visibility)) = roots.get_mut(models.root) else {
        return;
    };
    *visibility = if active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !active {
        models.motion.reset(frame.view.feet, frame.action.serial);
        return;
    }
    let (offset, rotation) = models.motion.step(
        frame.time.delta_secs(),
        frame.view.feet,
        &frame.player.body,
        &frame.action,
    );
    let half = (p.fov * 0.5).tan();
    let scale = half * 0.68 * (p.aspect_ratio / 1.4).min(1.);
    *transform = Transform::from_xyz(half * p.aspect_ratio * 0.64, -half * 0.60, -1.)
        .with_scale(Vec3::splat(scale))
        .with_rotation(rotation);
    transform.translation += offset * scale;
}

#[cfg(test)]
mod tests;
