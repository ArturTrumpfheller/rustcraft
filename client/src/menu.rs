//! Original pixel geometry/glyph menu. No imported artwork, font or UI dependency.
use crate::session::{self, ClientFlow, ClientState, Destination};
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        ScalingMode,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    mesh::Indices,
    prelude::*,
    render::{render_resource::PrimitiveTopology, view::Tonemapping},
    window::PrimaryWindow,
};
mod worlds;
const LAYER: usize = 2;
#[derive(Component)]
struct MenuCamera;
#[derive(Component)]
struct MenuSurface;
#[derive(Resource, Default)]
struct Navigation {
    selected: usize,
    pressed: Option<usize>,
    pointer: Option<Vec2>,
    stamp: String,
}
#[derive(Clone, Copy)]
enum Action {
    Play,
    Resume,
    Title,
    Quit,
    Retry,
}
fn entries(flow: &ClientFlow) -> Vec<(&'static str, Option<Action>)> {
    match flow.state {
        ClientState::Title => vec![
            ("Spielen", Some(Action::Play)),
            ("Beenden", Some(Action::Quit)),
        ],
        ClientState::Paused => vec![
            ("Fortsetzen", Some(Action::Resume)),
            ("Zurück zum Hauptmenü", Some(Action::Title)),
            ("Beenden", Some(Action::Quit)),
        ],
        ClientState::RecoverableError if flow.destination.is_some() => vec![
            ("Erneut speichern", Some(Action::Retry)),
            ("Zurück zum Spiel", Some(Action::Resume)),
        ],
        ClientState::RecoverableError => vec![
            ("Erneut versuchen", Some(Action::Play)),
            ("Hauptmenü", Some(Action::Title)),
            ("Beenden", Some(Action::Quit)),
        ],
        ClientState::Loading => vec![("Welt wird geladen...", None), ("Bitte warten", None)],
        ClientState::Saving => vec![("Wird gespeichert...", None), ("Bitte warten", None)],
        ClientState::Playing | ClientState::Worlds => vec![],
    }
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        MenuCamera,
        Camera3d::default(),
        Camera {
            order: 10,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 270.,
            },
            ..OrthographicProjection::default_3d()
        }),
        RenderLayers::layer(LAYER),
        Tonemapping::None,
        bevy::render::view::DebandDither::Disabled,
    ));
    commands.spawn((
        MenuSurface,
        Mesh3d(meshes.add(Canvas::default().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            ..default()
        })),
        RenderLayers::layer(LAYER),
        NoFrustumCulling,
        Transform::from_xyz(0., 0., -10.),
    ));
}
fn scale(window: &Window) -> f32 {
    (window.width() / 480.)
        .min(window.height() / 270.)
        .max(0.01)
}
fn hit(window: &Window, count: usize) -> Option<usize> {
    let p = (window.cursor_position()? - Vec2::new(window.width(), window.height()) / 2.)
        / scale(window);
    (0..count).find(|&i| p.x.abs() <= 100. && (p.y - (-2. + i as f32 * 26.)).abs() <= 10.)
}
fn input(world: &mut World) {
    if world.resource::<ClientFlow>().state == ClientState::Worlds {
        worlds::input(world);
        return;
    }
    if world.resource::<ClientFlow>().playing() {
        return;
    }
    let options = entries(world.resource::<ClientFlow>());
    let (focused, pointer, hovered) = {
        let Ok(window) = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
        else {
            return;
        };
        (
            window.focused,
            window.cursor_position(),
            hit(window, options.len()),
        )
    };
    if !focused {
        world.resource_mut::<Navigation>().pressed = None;
        return;
    }
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let up = keys.just_pressed(KeyCode::ArrowUp);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    let enter_down = keys.just_pressed(KeyCode::Enter);
    let enter_up = keys.just_released(KeyCode::Enter);
    let mouse = world.resource::<ButtonInput<MouseButton>>();
    let mouse_down = mouse.just_pressed(MouseButton::Left);
    let mouse_up = mouse.just_released(MouseButton::Left);
    let mut nav = world.resource_mut::<Navigation>();
    nav.selected = nav.selected.min(options.len().saturating_sub(1));
    if pointer != nav.pointer {
        if let Some(i) = hovered {
            nav.selected = i;
        }
        nav.pointer = pointer;
    }
    if up {
        nav.selected = (nav.selected + options.len() - 1) % options.len();
        nav.pressed = None;
    }
    if down {
        nav.selected = (nav.selected + 1) % options.len();
        nav.pressed = None;
    }
    if enter_down || mouse_down && hovered.is_some() {
        if let Some(i) = hovered.filter(|_| mouse_down) {
            nav.selected = i;
        }
        nav.pressed = Some(nav.selected);
    }
    let action = if enter_up || mouse_up {
        nav.pressed
            .take()
            .filter(|&i| i == nav.selected && (enter_up || hovered == Some(i)))
            .and_then(|i| options[i].1)
    } else {
        None
    };
    if let Some(action) = action {
        world.resource_mut::<Navigation>().selected = 0;
        match action {
            Action::Play => {
                if world.resource::<ClientFlow>().selected_world.is_some()
                    && world.resource::<ClientFlow>().state == ClientState::RecoverableError
                {
                    session::begin_loading(world);
                } else {
                    worlds::open(world);
                }
            }
            Action::Resume => {
                let mut f = world.resource_mut::<ClientFlow>();
                f.state = ClientState::Playing;
                f.destination = None;
                f.resume_frame = true;
                f.save_complete = false;
            }
            Action::Title => {
                if world.contains_resource::<crate::scene::VoxelWorld>() {
                    world.resource_mut::<ClientFlow>().save(Destination::Title);
                } else {
                    *world.resource_mut::<ClientFlow>() = ClientFlow::default();
                }
            }
            Action::Quit => {
                if world.contains_resource::<crate::scene::VoxelWorld>() {
                    world.resource_mut::<ClientFlow>().save(Destination::Quit);
                } else {
                    world.write_message(AppExit::Success);
                }
            }
            Action::Retry => {
                let target = world.resource::<ClientFlow>().destination.unwrap();
                world.resource_mut::<ClientFlow>().save(target);
            }
        }
    }
}
#[derive(Default)]
struct Canvas {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Canvas {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        let n = self.positions.len() as u32;
        // All quads share a plane, painted in order with tiny depth increments.
        let z = n as f32 * 0.00001;
        self.positions
            .extend([[x, y, z], [x + w, y, z], [x + w, y + h, z], [x, y + h, z]]);
        self.colors.extend([c; 4]);
        self.indices.extend([n, n + 1, n + 2, n, n + 2, n + 3]);
    }
    /// Original bevel geometry: shared by every menu, with a separate flat
    /// treatment for fields and list rows. No texture or external font.
    fn button(&mut self, rect: [f32; 4], label: &str, focused: bool, pressed: bool, enabled: bool) {
        let [x, y, w, h] = rect;
        self.rect(x - 1., y - 1., w + 2., h + 2., [0.012, 0.014, 0.018, 1.]);
        self.rect(
            x,
            y,
            w,
            h,
            if focused && enabled {
                [0.7, 0.73, 0.76, 1.]
            } else {
                [0.25, 0.27, 0.29, 1.]
            },
        );
        self.rect(
            x + 1.,
            y + 1.,
            w - 2.,
            h - 2.,
            if pressed {
                [0.09, 0.10, 0.12, 1.]
            } else if !enabled {
                [0.055, 0.06, 0.07, 1.]
            } else if focused {
                [0.24, 0.27, 0.31, 1.]
            } else {
                [0.15, 0.17, 0.19, 1.]
            },
        );
        self.rect(
            x + 1.,
            y + h - 2.,
            w - 2.,
            1.,
            if pressed {
                [0.07, 0.08, 0.09, 1.]
            } else {
                [0.35, 0.38, 0.41, 1.]
            },
        );
        let size = if h < 19. { 0.75 } else { 0.95 };
        let tx = x + (w - (label.to_uppercase().chars().count() as f32 * 6. - 1.) * size) / 2.;
        let ty = y + (h - 7. * size) / 2. - if pressed { 1. } else { 0. };
        self.text(tx + size, ty - size, label, size, [0.015, 0.018, 0.022, 1.]);
        self.text(
            tx,
            ty,
            label,
            size,
            if enabled {
                [0.94, 0.95, 0.96, 1.]
            } else {
                [0.35, 0.37, 0.4, 1.]
            },
        );
    }
    fn text(&mut self, x: f32, y: f32, text: &str, size: f32, c: [f32; 4]) {
        for (i, ch) in text.to_uppercase().chars().enumerate() {
            for (row, bits) in crate::save_hud::glyph(ch).into_iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.rect(
                            x + (i as f32 * 6. + col as f32) * size,
                            y + (6 - row) as f32 * size,
                            size,
                            size,
                            c,
                        );
                    }
                }
            }
        }
    }
    fn centered(&mut self, y: f32, text: &str, size: f32, c: [f32; 4]) {
        self.text(
            -(text.chars().count() as f32 * 6. - 1.) * size / 2.,
            y,
            text,
            size,
            c,
        );
    }
    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}
fn draw(
    flow: Res<ClientFlow>,
    browser: Res<worlds::Browser>,
    mut nav: ResMut<Navigation>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut camera: Query<(&mut Camera, &mut Projection), With<MenuCamera>>,
    mut surface: Query<(&Mesh3d, &mut Visibility), With<MenuSurface>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(window), Ok((mut camera, mut projection)), Ok((mesh, mut visibility))) =
        (windows.single(), camera.single_mut(), surface.single_mut())
    else {
        return;
    };
    camera.is_active = !flow.playing();
    *visibility = if flow.playing() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    if flow.playing() {
        nav.stamp.clear();
        return;
    }
    let stamp = format!(
        "{:?}/{:?}/{}/{:?}/{}/{}/{}",
        flow.state,
        flow.destination,
        nav.selected,
        nav.pressed,
        window.width(),
        window.height(),
        flow.error
    );
    let stamp = format!("{stamp}/{:?}", browser.stamp());
    if nav.stamp == stamp {
        return;
    }
    nav.stamp = stamp;
    let half = Vec2::new(window.width(), window.height()) / (2. * scale(window));
    if let Projection::Orthographic(p) = &mut *projection {
        p.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: half.y * 2.,
        };
    }
    let mut c = Canvas::default();
    let copper = [0.64, 0.69, 0.75, 1.];
    let cream = [0.94, 0.95, 0.96, 1.];
    c.rect(
        -half.x,
        -half.y,
        half.x * 2.,
        half.y * 2.,
        [0.025, 0.03, 0.04, 1.],
    );
    // Authored slate mosaic, deliberately unrelated to game texture packs.
    for row in 0..(half.y * 2. / 12.).ceil() as i32 {
        for col in 0..(half.x * 2. / 16.).ceil() as i32 {
            let shade = 0.026 + ((col * 13 + row * 7 + col * row) % 5) as f32 * 0.002;
            c.rect(
                -half.x + col as f32 * 16.,
                -half.y + row as f32 * 12.,
                15.,
                11.,
                [shade, shade * 1.12, shade * 1.32, 1.],
            );
        }
    }
    if flow.state == ClientState::Worlds {
        worlds::paint(&browser, &mut c);
        if let Some(mut target) = meshes.get_mut(&mesh.0) {
            *target = c.mesh();
        }
        return;
    }
    c.centered(77., "RUSTCRAFT", 3., [0.12, 0.08, 0.055, 1.]);
    c.centered(80., "RUSTCRAFT", 3., cream);
    c.rect(-45., 68., 90., 1., copper);
    let subtitle = match flow.state {
        ClientState::Title => "DEINE WELT. DEINE IDEEN.",
        ClientState::Paused => "PAUSE",
        ClientState::Loading => "WELT LADEN",
        ClientState::Saving => "WELT SICHERN",
        _ => "VORGANG FEHLGESCHLAGEN",
    };
    c.centered(49., subtitle, 1., copper);
    for (i, (label, action)) in entries(&flow).iter().enumerate() {
        let y = -8. - i as f32 * 26.;
        c.button(
            [-100., y, 200., 20.],
            label,
            i == nav.selected,
            nav.pressed == Some(i),
            action.is_some(),
        );
    }
    if flow.state == ClientState::RecoverableError {
        c.centered(
            -80.,
            if flow.destination.is_some() {
                "ÄNDERUNGEN BLEIBEN ERHALTEN"
            } else {
                "DATEIEN WERDEN NICHT ERSETZT"
            },
            1.,
            [1., 0.67, 0.43, 1.],
        );
        // Detailed OS error also remains in the log, without panics on Unicode paths.
        let detail: String = flow.error.chars().take(72).collect();
        for (i, line) in detail.chars().collect::<Vec<_>>().chunks(36).enumerate() {
            c.centered(
                -93. - i as f32 * 10.,
                &line.iter().collect::<String>(),
                0.9,
                cream,
            );
        }
        c.centered(-115., "HILFE: README / SAVE_FORMAT", 0.8, cream);
    } else {
        c.centered(
            -112.,
            "MAUS ODER PFEILTASTEN / ENTER",
            0.8,
            [0.46, 0.56, 0.57, 1.],
        );
    }
    c.centered(
        -half.y + 7.,
        "RUSTCRAFT  /  VOXEL SANDBOX",
        0.7,
        [0.4, 0.48, 0.47, 1.],
    );
    if let Some(mut target) = meshes.get_mut(&mesh.0) {
        *target = c.mesh();
    }
}
pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Navigation>()
            .init_resource::<worlds::Browser>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .add_systems(Update, worlds::poll)
            .add_systems(
                RunFixedMainLoop,
                worlds::text_input
                    .before(input)
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(Startup, setup)
            .add_systems(
                RunFixedMainLoop,
                input
                    .after(session::FlowInput)
                    .before(crate::camera::CameraInput)
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(PostUpdate, draw);
    }
}
