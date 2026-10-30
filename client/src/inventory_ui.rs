//! Original retained pixel UI + modal input, no external UI/font assets.
use crate::{
    camera::{CameraSetup, FirstPersonCamera},
    scene::{SceneSetup, TerrainMaterial},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    transform::TransformSystems,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use voxel_core::{
    inventory::{Click, HOTBAR_SLOTS, Inventory, Item, SLOT_COUNT},
    texture::{Tile, face_tile},
};
#[derive(Resource)]
pub(crate) struct GameInventory {
    pub contents: Inventory,
    pub open: bool,
    pub transition: bool,
    pub resume_capture: bool,
    pub message: &'static str,
    wheel: f32,
}
impl Default for GameInventory {
    fn default() -> Self {
        Self::loaded(Inventory::starter())
    }
}
impl GameInventory {
    pub fn loaded(contents: Inventory) -> Self {
        Self {
            open: contents.cursor().is_some(),
            contents,
            transition: false,
            resume_capture: false,
            message: "",
            wheel: 0.,
        }
    }
}
#[derive(SystemSet, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct InventoryInput;
pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameInventory>()
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
                input
                    .in_set(InventoryInput)
                    .run_if(crate::session::playing)
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                PostUpdate,
                draw.before(TransformSystems::Propagate)
                    .run_if(crate::session::has_world),
            );
    }
}
#[derive(Clone, Copy)]
struct Layout {
    scale: f32,
    half: Vec2,
}
impl Layout {
    fn new(w: &Window) -> Self {
        let scale = (w.width() / 300.)
            .min(w.height() / 260.)
            .floor()
            .clamp(1., 4.);
        Self {
            scale,
            half: Vec2::new(w.width(), w.height()) / (2. * scale),
        }
    }
    fn center(self, i: usize, open: bool) -> Vec2 {
        let col = i % HOTBAR_SLOTS;
        Vec2::new(
            (col as f32 - 4.) * 28.,
            if i < HOTBAR_SLOTS {
                if open { -54. } else { -self.half.y + 20. }
            } else {
                42. - ((i - HOTBAR_SLOTS) / HOTBAR_SLOTS) as f32 * 28.
            },
        )
    }
    fn pointer(self, p: Vec2) -> Vec2 {
        Vec2::new(p.x, -p.y) / self.scale + Vec2::new(-self.half.x, self.half.y)
    }
    fn hit(self, p: Vec2, open: bool) -> Option<usize> {
        let p = self.pointer(p);
        (0..if open { SLOT_COUNT } else { HOTBAR_SLOTS }).find(|&i| {
            let d = (p - self.center(i, open)).abs();
            d.x < 12. && d.y < 12.
        })
    }
}
fn input(
    mut state: ResMut<GameInventory>,
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Option<Res<crate::controls::MovementBindings>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Option<Res<AccumulatedMouseScroll>>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    save: Option<Res<crate::persistence::SaveFeedback>>,
) {
    state.transition = false;
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    if !window.focused || save.is_some_and(|s| s.closing) {
        state.wheel = 0.;
        return;
    }
    let bindings = bindings.as_deref().copied().unwrap_or_default();
    if keyboard.just_pressed(bindings.inventory)
        || (state.open
            && (keyboard.just_pressed(bindings.menu) || keyboard.just_pressed(KeyCode::Escape)))
    {
        if state.open {
            if state.contents.return_cursor() {
                state.open = false;
                state.transition = true;
                state.message = "";
            } else {
                state.message = "NO ROOM";
            }
        } else {
            state.resume_capture = cursor.grab_mode != CursorGrabMode::None;
            state.open = true;
            state.transition = true;
            state.message = "";
        }
        return;
    }
    for (i, key) in bindings.hotbar.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            state.contents.select(i);
        }
    }
    if !state.open {
        if let Some(scroll) = scroll {
            let delta = match scroll.unit {
                MouseScrollUnit::Line => scroll.delta.y,
                MouseScrollUnit::Pixel => scroll.delta.y / 40.,
            };
            if delta.is_finite() {
                state.wheel = (state.wheel - delta).clamp(-64., 64.);
                let steps = state.wheel.trunc() as i32;
                if steps != 0 {
                    state.contents.scroll(steps);
                    state.wheel -= steps as f32;
                }
            }
        }
        return;
    }
    state.wheel = 0.;
    let Some(p) = window.cursor_position() else {
        return;
    };
    let layout = Layout::new(window);
    let point = layout.pointer(p);
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(item) = Item::CREATIVE
            .into_iter()
            .enumerate()
            .find_map(|(i, item)| {
                let center = Vec2::new(((i % 10) as f32 - 4.5) * 28., 94. + (i / 10) as f32 * 28.);
                ((point - center).abs().max_element() < 12.).then_some(item)
            })
        {
            let count = if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
                item.max_stack()
            } else {
                1
            };
            state.message = if state.contents.grant(item, u16::from(count)) {
                ""
            } else {
                "NO ROOM"
            };
            return;
        }
        if (point - Vec2::new(91., 70.))
            .abs()
            .cmplt(Vec2::new(30., 7.))
            .all()
            && state
                .contents
                .selected_stack()
                .is_some_and(|s| s.item() == Item::WaterSource)
        {
            state.message = if state.contents.convert_legacy_water() {
                ""
            } else {
                "NO ROOM"
            };
            return;
        }
    }
    let Some(i) = Layout::new(window).hit(p, true) else {
        return;
    };
    let click = if mouse.just_pressed(MouseButton::Left) {
        Some(
            if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
                Click::Quick
            } else {
                Click::Left
            },
        )
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(Click::Right)
    } else {
        None
    };
    if let Some(c) = click {
        state.contents.click(i, c);
        state.message = "";
    }
}

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
impl Batch {
    fn rect(&mut self, p: Vec2, size: Vec2, z: f32, color: [f32; 4]) {
        let n = self.positions.len() as u32;
        self.positions.extend([
            [p.x, p.y, z],
            [p.x + size.x, p.y, z],
            [p.x + size.x, p.y + size.y, z],
            [p.x, p.y + size.y, z],
        ]);
        self.normals.extend([[0., 0., 1.]; 4]);
        self.colors.extend([color; 4]);
        self.indices.extend([n, n + 1, n + 2, n, n + 2, n + 3]);
    }
    fn icon(&mut self, p: Vec2, item: Item) {
        if item == Item::Torch {
            let tile = Tile::Torch.uv([0, 0]);
            for (offset, size, v0, v1) in [
                (Vec2::new(6.5, 0.), Vec2::new(3., 11.), 0.25, 1.),
                (Vec2::new(5.5, 11.), Vec2::new(5., 5.), 0., 0.25),
            ] {
                self.rect(p + offset, size, 1., [1.; 4]);
                self.uvs.extend(
                    [[0., v1], [1., v1], [1., v0], [0., v0]]
                        .map(|uv| [tile[0] + uv[0] * 16. / 128., tile[1] + uv[1] * 16. / 192.]),
                );
            }
            return;
        }
        self.rect(p, Vec2::splat(16.), 1., [1.; 4]);
        let tile = item
            .build()
            .and_then(|b| face_tile(b.block(), [0, 0, 1]))
            .unwrap_or(Tile::Stone);
        self.uvs.extend([
            tile.uv([0, 1]),
            tile.uv([1, 1]),
            tile.uv([1, 0]),
            tile.uv([0, 0]),
        ]);
    }
    fn bucket(&mut self, p: Vec2, item: Item) {
        for y in 0..16 {
            for x in 0..16 {
                if let Some([r, g, b, _]) = voxel_core::item_art::bucket_pixel(item, x, y) {
                    self.rect(
                        p + Vec2::new(x as f32, 15. - y as f32),
                        Vec2::ONE,
                        1.,
                        Color::srgb_u8(r, g, b).to_linear().to_f32_array(),
                    );
                }
            }
        }
    }
    fn text(&mut self, p: Vec2, s: &str, color: [f32; 4]) {
        for (i, ch) in s.chars().enumerate() {
            let rows = glyph(ch);
            for (y, bits) in rows.into_iter().enumerate() {
                for x in 0..3 {
                    if bits & (1 << (2 - x)) != 0 {
                        self.rect(
                            p + Vec2::new(i as f32 * 4. + x as f32, 4. - y as f32),
                            Vec2::ONE,
                            2.,
                            color,
                        );
                    }
                }
            }
        }
    }
    fn count(&mut self, c: Vec2, n: u8) {
        let text = n.to_string();
        let p = c + Vec2::new(10. - text.len() as f32 * 4., -10.);
        self.rect(
            p - Vec2::ONE,
            Vec2::new(text.len() as f32 * 4. + 1., 7.),
            1.5,
            [0.025, 0.03, 0.04, 1.],
        );
        self.text(p, &text, [1., 1., 0.85, 1.]);
    }
    fn mesh(self, textured: bool) -> Mesh {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_indices(Indices::U32(self.indices));
        if textured {
            m.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        } else {
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }
        m
    }
}
// Original editable three-by-five pixel numerals/letters, no font dependency.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '-' => [0, 0, 7, 0, 0],
        'A' => [2, 5, 7, 5, 5],
        'D' => [6, 5, 5, 5, 6],
        'K' => [5, 5, 6, 5, 5],
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [6, 1, 7, 4, 7],
        '3' => [6, 1, 3, 1, 6],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 6, 1, 6],
        '6' => [3, 4, 7, 5, 7],
        '7' => [7, 1, 2, 2, 2],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 6],
        'I' => [7, 2, 2, 2, 7],
        'N' => [5, 7, 7, 7, 5],
        'V' => [5, 5, 5, 5, 2],
        'E' => [7, 4, 6, 4, 7],
        'T' => [7, 2, 2, 2, 2],
        'O' => [2, 5, 5, 5, 2],
        'R' => [6, 5, 6, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'M' => [5, 7, 7, 5, 5],
        'C' => [3, 4, 4, 4, 3],
        'L' => [4, 4, 4, 4, 7],
        'S' => [3, 4, 2, 1, 6],
        _ => [0; 5],
    }
}
#[derive(Component)]
struct UiRoot;
#[derive(Component)]
struct CursorRoot;
#[derive(Resource)]
struct UiAssets {
    meshes: [Handle<Mesh>; 4],
    entities: [Entity; 4],
    root: Entity,
    cursor: Entity,
    last: Option<(u64, bool, [u32; 2], &'static str)>,
    pub rebuilds: u64,
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain: Res<TerrainMaterial>,
    cameras: Query<Entity, With<FirstPersonCamera>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let atlas = Some(terrain.1.clone());
    let flat = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    let icon = materials.add(StandardMaterial {
        alpha_mode: AlphaMode::Mask(0.5),
        base_color_texture: atlas,
        unlit: true,
        ..default()
    });
    let handles: [Handle<Mesh>; 4] = std::array::from_fn(|i| {
        let mut b = Batch::default();
        if i % 2 == 1 {
            b.icon(Vec2::ZERO, Item::Stone);
        } else {
            b.rect(Vec2::ZERO, Vec2::ONE, 0., [1.; 4]);
        }
        meshes.add(b.mesh(i % 2 == 1))
    });
    let root = commands
        .spawn((
            crate::session::SessionEntity,
            UiRoot,
            Transform::from_xyz(0., 0., -0.2),
            Visibility::Inherited,
        ))
        .id();
    let cursor = commands
        .spawn((
            crate::session::SessionEntity,
            CursorRoot,
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.entity(camera).add_child(root);
    commands.entity(root).add_child(cursor);
    let mut entities = Vec::new();
    for (i, h) in handles.iter().enumerate() {
        let e = commands
            .spawn((
                crate::session::SessionEntity,
                bevy::camera::visibility::RenderLayers::layer(crate::viewmodel::OVERLAY_LAYER),
                Mesh3d(h.clone()),
                MeshMaterial3d(if i % 2 == 0 {
                    flat.clone()
                } else {
                    icon.clone()
                }),
                NoFrustumCulling,
                Visibility::Hidden,
            ))
            .id();
        entities.push(e);
        commands
            .entity(if i < 2 { root } else { cursor })
            .add_child(e);
    }
    commands.insert_resource(UiAssets {
        meshes: handles,
        entities: entities.try_into().unwrap(),
        root,
        cursor,
        last: None,
        rebuilds: 0,
    });
}
fn draw(
    state: Res<GameInventory>,
    assets: Option<ResMut<UiAssets>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Projection, With<FirstPersonCamera>>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(mut assets), Ok(window), Ok(Projection::Perspective(projection))) =
        (assets, windows.single(), cameras.single())
    else {
        return;
    };
    let layout = Layout::new(window);
    if let Ok(mut t) = transforms.get_mut(assets.root) {
        t.scale = Vec3::splat(
            2. * 0.2 * (projection.fov * 0.5).tan() / window.height().max(1.) * layout.scale,
        );
    }
    if let Ok(mut t) = transforms.get_mut(assets.cursor) {
        let p = window
            .cursor_position()
            .map(|p| layout.pointer(p))
            .unwrap_or(Vec2::ZERO);
        t.translation = Vec3::new(p.x + 10., p.y - 10., 4.);
    }
    if let Ok(mut v) = visibility.get_mut(assets.cursor) {
        *v = if state.open
            && state.contents.cursor().is_some()
            && window.focused
            && window.cursor_position().is_some()
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let stamp = (
        state.contents.revision(),
        state.open,
        [window.width().to_bits(), window.height().to_bits()],
        state.message,
    );
    if assets.last == Some(stamp) {
        return;
    }
    assets.last = Some(stamp);
    assets.rebuilds += 1;
    let mut flat = Batch::default();
    let mut icons = Batch::default();
    let mut held_text = Batch::default();
    let mut held_icon = Batch::default();
    let y = layout.center(0, state.open).y;
    let height = if state.open { 210. } else { 34. };
    let bottom = if state.open { y - 26. } else { y - 17. };
    flat.rect(
        Vec2::new(-136., bottom),
        Vec2::new(272., height),
        0.,
        [0.17, 0.11, 0.075, 1.],
    );
    flat.rect(
        Vec2::new(-134., bottom + 2.),
        Vec2::new(268., height - 4.),
        0.1,
        [0.035, 0.055, 0.07, 1.],
    );
    if !state.open && !state.message.is_empty() {
        let width = state.message.len() as f32 * 4.;
        flat.rect(
            Vec2::new(-width / 2. - 3., y + 19.),
            Vec2::new(width + 6., 10.),
            0.,
            [0.035, 0.055, 0.07, 1.],
        );
        flat.text(
            Vec2::new(-width / 2., y + 22.),
            state.message,
            [1., 0.7, 0.3, 1.],
        );
    }
    if state.open {
        flat.rect(
            Vec2::new(-128., -34.),
            Vec2::new(256., 1.),
            0.2,
            [0.3, 0.22, 0.14, 1.],
        );
        flat.text(Vec2::new(-18., 72.), "INVENTORY", [0.9, 0.68, 0.35, 1.]);
        flat.text(
            Vec2::new(-18., -72.),
            if state.message.is_empty() {
                "E CLOSE"
            } else {
                state.message
            },
            [1., 0.85, 0.55, 1.],
        );
    }
    if state.open {
        flat.text(
            Vec2::new(-44., 144.),
            "ITEMS - CLICK TO ADD",
            [0.9, 0.75, 0.5, 1.],
        );
        for (i, item) in Item::CREATIVE.into_iter().enumerate() {
            let c = Vec2::new(((i % 10) as f32 - 4.5) * 28., 94. + (i / 10) as f32 * 28.);
            flat.rect(
                c - Vec2::splat(12.),
                Vec2::splat(24.),
                0.3,
                [0.29, 0.32, 0.32, 1.],
            );
            flat.rect(
                c - Vec2::splat(10.),
                Vec2::splat(20.),
                0.5,
                [0.055, 0.075, 0.09, 1.],
            );
            if item.build().is_none() {
                flat.bucket(c - Vec2::splat(8.), item);
            } else {
                icons.icon(c - Vec2::splat(8.), item);
            }
        }
        if state
            .contents
            .selected_stack()
            .is_some_and(|s| s.item() == Item::WaterSource)
        {
            flat.rect(
                Vec2::new(61., 63.),
                Vec2::new(60., 14.),
                0.5,
                [0.3, 0.22, 0.14, 1.],
            );
            flat.text(Vec2::new(73., 68.), "CONVERT 7", [1., 0.85, 0.55, 1.]);
        }
    }
    for i in 0..if state.open { SLOT_COUNT } else { HOTBAR_SLOTS } {
        let c = layout.center(i, state.open);
        let selected = i == state.contents.selected();
        flat.rect(
            c - Vec2::splat(12.),
            Vec2::splat(24.),
            0.3,
            if selected {
                [0.95, 0.62, 0.23, 1.]
            } else {
                [0.29, 0.32, 0.32, 1.]
            },
        );
        flat.rect(
            c - Vec2::splat(10.),
            Vec2::splat(20.),
            0.5,
            [0.055, 0.075, 0.09, 1.],
        );
        if let Some(s) = state.contents.slots()[i] {
            if s.item().build().is_none() {
                flat.bucket(c - Vec2::splat(8.), s.item());
            } else {
                icons.icon(c - Vec2::splat(8.), s.item());
            }
            flat.count(c, s.count());
        }
        if i < HOTBAR_SLOTS {
            flat.text(
                c + Vec2::new(-10., 6.),
                &(i + 1).to_string(),
                [0.8, 0.85, 0.9, 1.],
            );
        }
    }
    if let Some(s) = state.contents.cursor() {
        if s.item().build().is_none() {
            held_text.bucket(-Vec2::splat(8.), s.item());
        } else {
            held_icon.icon(-Vec2::splat(8.), s.item());
        }
        held_text.count(Vec2::ZERO, s.count());
    }
    for (i, b) in [flat, icons, held_text, held_icon].into_iter().enumerate() {
        let empty = b.positions.is_empty();
        if let Ok(mut v) = visibility.get_mut(assets.entities[i]) {
            *v = if empty {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
        // Keep a bounded valid cached buffer while hidden; never upload empty
        // meshes (the renderer otherwise repeatedly warns during UI transitions).
        if !empty {
            *meshes.get_mut(&assets.meshes[i]).unwrap() = b.mesh(i % 2 == 1);
        }
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn cleanup(world: &mut World) {
    world.remove_resource::<UiAssets>();
}
