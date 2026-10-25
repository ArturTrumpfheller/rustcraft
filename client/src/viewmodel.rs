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
    image::{ImageSampler, ImageSamplerDescriptor},
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    render::view::Tonemapping,
};
use voxel_core::{Block, inventory::Item, texture::face_tile};

pub(crate) const OVERLAY_LAYER: usize = 1;
// Uniformly scale camera-space position AND geometry: identical projected
// pixels, but even the nearest swing vertex remains behind the existing UI.
const MODEL_DEPTH_SCALE: f32 = 3.;

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
            .add_systems(
                Startup,
                (setup.after(CameraSetup).after(SceneSetup)).run_if(crate::session::has_world),
            )
            .add_systems(
                crate::session::SessionSetup,
                setup.after(CameraSetup).after(SceneSetup),
            )
            .add_systems(
                RunFixedMainLoop,
                update
                    .after(ViewSync)
                    .run_if(crate::session::has_world)
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
#[derive(Component)]
struct Arm;
#[derive(Resource)]
struct Models {
    blocks: [Handle<Mesh>; 5],
    root: Entity,
    block: Entity,
    arm: Entity,
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
                .map_or(local.map(f32::from), |t| t.uv(local)),
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

// One and only one cuboid. Sleeve/skin are pixels, never extra geometry.
fn arm_mesh() -> Mesh {
    let mut mesh = cube(None);
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for p in positions {
            *p = (Vec3::from_array(*p) * Vec3::new(0.25, 0.75, 0.25)).to_array();
        }
    }
    let VertexAttributeValues::Float32x3(normals) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
    else {
        unreachable!()
    };
    let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
    else {
        unreachable!()
    };
    let uvs: Vec<_> = uvs
        .iter()
        .zip(normals)
        .map(|(uv, n)| {
            // End caps sample only skin/sleeve; never put a sleeve stripe on the fist.
            [
                uv[0],
                if n[1] > 0. {
                    uv[1] * 0.25
                } else if n[1] < 0. {
                    0.75 + uv[1] * 0.25
                } else {
                    uv[1]
                },
            ]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn arm_image() -> Image {
    // Original RustCraft artwork, editable/reproducible here, MIT OR Apache-2.0.
    // Warm clay skin with sparse highlights; a slate sleeve occupies the lower
    // part of the same surface. Not a Minecraft skin layout or imported pixels.
    let palette = [
        [181, 132, 95, 255],
        [192, 144, 105, 255],
        [174, 123, 89, 255],
        [55, 75, 83, 255],
        [62, 84, 92, 255],
    ];
    let mut pixels = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let index = if y >= 6 {
                3 + usize::from((x + y * 3) % 13 == 0)
            } else if (x * 7 + y * 11) % 19 == 0 {
                1
            } else {
                usize::from((x * 3 + y * 5) % 23 == 0) * 2
            };
            pixels.extend_from_slice(&palette[index]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        lod_max_clamp: 0.,
        ..ImageSamplerDescriptor::nearest()
    });
    image
}

fn setup(
    mut commands: Commands,
    camera: Query<Entity, With<FirstPersonCamera>>,
    terrain: Res<TerrainMaterial>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let overlay = commands
        .spawn((
            crate::session::SessionEntity,
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
    let atlas = Some(terrain.1.clone());
    let skin = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(arm_image())),
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
        .spawn((
            crate::session::SessionEntity,
            HandRoot,
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.entity(overlay).add_child(root);
    let arm = commands
        .spawn((
            crate::session::SessionEntity,
            Arm,
            Mesh3d(meshes.add(arm_mesh())),
            MeshMaterial3d(skin),
            Visibility::Inherited,
            RenderLayers::layer(OVERLAY_LAYER),
        ))
        .id();
    let block = commands
        .spawn((
            crate::session::SessionEntity,
            HeldBlock,
            Visibility::Hidden,
            Mesh3d(blocks[0].clone()),
            MeshMaterial3d(textured),
            RenderLayers::layer(OVERLAY_LAYER),
            Transform::default(),
        ))
        .id();
    commands.entity(root).add_children(&[arm, block]);
    commands.insert_resource(Models {
        blocks,
        root,
        block,
        arm,
        selected: None,
        motion: Motion::default(),
    });
}

// Viewmodel-only gait tuning; interaction swing coefficients remain separate.
const GAIT_RADIANS_PER_BLOCK: f32 = 2.0;
const BOB_HORIZONTAL: f32 = 0.020;
const BOB_VERTICAL: f32 = 0.0275;
const BOB_ROLL: f32 = 0.0175;
const BOB_HORIZONTAL_BIAS: f32 = 0.003;
const BOB_VERTICAL_BIAS: f32 = 0.0025;
const LANDING_OFFSET: f32 = 0.030;

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
    swing_time: f32,
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
        let elapsed = dt;
        let dt = dt.min(0.05);
        let blend = 1. - (-12. * dt).exp();
        let amount = if grounded {
            (speed / 5.).min(1.) * if crouched { 0. } else { 1. }
        } else {
            0.
        };
        if grounded && distance <= 4. {
            self.phase = (self.phase + distance * GAIT_RADIANS_PER_BLOCK) % std::f32::consts::TAU;
        }
        self.bob += (amount - self.bob) * blend;
        self.crouch += (f32::from(crouched) - self.crouch) * blend;
        if self.previous_vy < -0.5 && (grounded || vy > 0.) {
            self.landing = LANDING_OFFSET;
        }
        self.previous_vy = vy;
        self.landing *= (-14. * dt).exp();
        if action.serial != self.serial {
            self.serial = action.serial;
            // Consume every action, but avoid repeatedly snapping an early swing
            // back to zero under rapid clicks. No accumulated/queued animation.
            if !self.swing_active || self.swing_time >= SWING_SECONDS * 0.5 {
                self.swing_active = true;
                self.swing_time = 0.;
            }
        }
        if self.swing_active {
            self.swing_time += elapsed;
            self.swing_active = self.swing_time < SWING_SECONDS;
        }
        self.swing = if self.swing_active {
            self.swing_time / SWING_SECONDS
        } else {
            0.
        };
        // Two vertical footfalls per lateral stride; doubling phase here would
        // create four bounces per stride and an unnaturally rapid vertical gait.
        let offset = Vec3::new(
            (self.phase.sin() * BOB_HORIZONTAL - BOB_HORIZONTAL_BIAS) * self.bob,
            -(self.phase.cos().abs() * BOB_VERTICAL + BOB_VERTICAL_BIAS) * self.bob
                - self.crouch * 0.025
                - self.landing
                - (vy as f32 * 0.002).clamp(-0.02, 0.02),
            0.,
        );
        (
            offset,
            Quat::from_rotation_z(self.phase.sin() * self.bob * BOB_ROLL),
        )
    }
}

// Independently authored visual-fit coefficients, not verified Java source
// constants. Reference images/limitations and normalized landmarks: VIEWMODEL.md.
const SWING_SECONDS: f32 = 0.35;
fn model_pose(block: bool, motion: &Motion) -> Transform {
    let elapsed = motion.swing * SWING_SECONDS;
    // The recorded Java held cube first lowers, then swings while returning to
    // its equipped pose. Keep that lead-in separate from the 300-ms arc.
    let t = if block {
        ((elapsed - 0.05) / 0.30).clamp(0., 1.)
    } else {
        motion.swing
    };
    // Fast inward strike followed by a longer outward recovery. A second arc
    // supplies lift early and dip late, rather than retracing a linear jab.
    let strike = (t.sqrt() * std::f32::consts::PI).sin();
    let lift = (t.sqrt() * std::f32::consts::TAU).sin();
    let recovery = (t * std::f32::consts::PI).sin();
    let late = (t * t * std::f32::consts::PI).sin();
    let (position, angles, size, pivot, swing_angles) = if block {
        (
            Vec3::new(0.56, -0.52, -0.72),
            Vec3::new(0., 45., 0.),
            0.40,
            Vec3::new(0.56, -0.52, -0.72),
            Vec3::new(-75. * strike, -20. * late, -20. * strike),
        )
    } else {
        (
            Vec3::new(0.609, -0.641, -0.80),
            Vec3::new(-46.5, 19.4, -23.9),
            1.,
            Vec3::new(0.65, -0.90, -0.45),
            Vec3::new(-7.5 * late, 63.8 * strike, strike),
        )
    };
    let euler = |angles: Vec3| {
        Quat::from_euler(
            EulerRot::XYZ,
            angles.x.to_radians(),
            angles.y.to_radians(),
            angles.z.to_radians(),
        )
    };
    let swing_rotation = euler(swing_angles);
    let rotation = swing_rotation * euler(angles);
    let travel = if block {
        let drop = if elapsed < 0.10 {
            0.32 * elapsed / 0.10
        } else if elapsed < 0.17 {
            0.32
        } else {
            let u = ((elapsed - 0.17) / 0.18).clamp(0., 1.);
            0.32 * (1. - u * u * (3. - 2. * u))
        };
        Vec3::new(-0.35 * strike, 0.20 * lift - drop, -0.18 * recovery)
    } else {
        Vec3::new(
            0.14 * strike - 0.325 * recovery,
            0.26 * lift + 0.03 * recovery,
            0.17 * recovery,
        )
    };
    let position = pivot + swing_rotation * (position - pivot) + travel;
    Transform::from_translation(position * MODEL_DEPTH_SCALE)
        .with_rotation(rotation)
        .with_scale(Vec3::splat(size * MODEL_DEPTH_SCALE))
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
type ArmFilter = (With<Arm>, Without<HandRoot>, Without<HeldBlock>);

fn update(
    frame: Frame,
    models: Option<ResMut<Models>>,
    source: Query<&Projection, With<FirstPersonCamera>>,
    mut overlay: Query<&mut Projection, (With<OverlayCamera>, Without<FirstPersonCamera>)>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<HandRoot>>,
    mut blocks: Query<(&mut Mesh3d, &mut Visibility, &mut Transform), HeldFilter>,
    mut arms: Query<(&mut Visibility, &mut Transform), ArmFilter>,
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
    if let Ok((mut visibility, _)) = arms.get_mut(models.arm) {
        *visibility = if selected.is_none() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok((mut handle, mut visibility, _)) = blocks.get_mut(models.block) {
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
    // Correct the lens in camera X/Y only, BEFORE each child's rotation. Unlike
    // scaling the model uniformly this keeps its perspective and screen bounds.
    let correction = (p.fov * 0.5).tan() / (70_f32.to_radians() * 0.5).tan();
    *transform = Transform::from_scale(Vec3::new(correction, correction, 1.));
    let mut pose = model_pose(selected.is_some(), &models.motion);
    pose.translation = rotation * pose.translation + offset * MODEL_DEPTH_SCALE;
    pose.rotation = rotation * pose.rotation;
    if selected.is_some() {
        if let Ok((_, _, mut block)) = blocks.get_mut(models.block) {
            *block = pose;
        }
    } else if let Ok((_, mut arm)) = arms.get_mut(models.arm) {
        *arm = pose;
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn cleanup(world: &mut World) {
    world.remove_resource::<Models>();
}
