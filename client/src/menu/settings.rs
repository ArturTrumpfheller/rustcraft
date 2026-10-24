//! Settings pages share the original retained menu surface and release activation.
use super::key_labels::KeyLabels;
use super::*;
use crate::settings::{Config, LABELS, Preferences, SIZES};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Home,
    Graphics,
    Input,
    Keys(usize),
    General,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cmd {
    Page(Page),
    Fov,
    Sensitivity,
    Invert,
    Radius,
    Mode,
    Size,
    Vsync,
    Bind(usize),
    ResetKeys,
    ResetAll,
    Apply,
    Back,
    Confirm,
    Cancel,
}
struct Control {
    label: String,
    cmd: Cmd,
    rect: [f32; 4],
}
struct Preview {
    old: Preferences,
    mode: bevy::window::WindowMode,
    resolution: bevy::window::WindowResolution,
    present_mode: bevy::window::PresentMode,
    deadline: std::time::Duration,
}
#[derive(Resource)]
pub(super) struct Editor {
    draft: Preferences,
    page: Page,
    origin: ClientState,
    focus: usize,
    pressed: Option<usize>,
    pointer: Option<Vec2>,
    capture: Option<usize>,
    drag: Option<Cmd>,
    reset: bool,
    error: String,
    revision: u64,
    preview: Option<Preview>,
    hovered: Option<usize>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            draft: default(),
            page: default(),
            origin: ClientState::Title,
            focus: 0,
            pressed: None,
            pointer: None,
            capture: None,
            drag: None,
            reset: false,
            error: String::new(),
            revision: 0,
            preview: None,
            hovered: None,
        }
    }
}
impl Editor {
    pub fn stamp(&self) -> String {
        format!(
            "{:?}/{}/{:?}/{:?}/{}/{}/{:?}",
            self.page,
            self.focus,
            self.pressed,
            self.capture,
            self.revision,
            self.error,
            (
                self.preview.as_ref().map(|p| p.deadline.as_secs()),
                self.hovered
            )
        )
    }
    fn controls(&self, labels: &KeyLabels) -> Vec<Control> {
        let mut v = vec![];
        let mut add = |label: String, cmd: Cmd, x: f32, y: f32, w: f32| {
            v.push(Control {
                label,
                cmd,
                rect: [x, y, w, 20.],
            })
        };
        if self.preview.is_some() {
            add("Keep Changes".into(), Cmd::Confirm, -202., -60., 198.);
            add("Revert".into(), Cmd::Cancel, 4., -60., 198.);
            return v;
        }
        match self.page {
            Page::Home => {
                for (i, (label, p)) in [
                    ("Video Settings", Page::Graphics),
                    ("Controls", Page::Input),
                    ("General", Page::General),
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        label.into(),
                        Cmd::Page(p),
                        -100.,
                        45. - i as f32 * 27.,
                        200.,
                    );
                }
            }
            Page::Graphics => {
                add(
                    format!("Vertical FOV: {} deg", self.draft.fov),
                    Cmd::Fov,
                    -202.,
                    60.,
                    198.,
                );
                add(
                    format!("Render Distance: {} chunks", self.draft.radius),
                    Cmd::Radius,
                    4.,
                    60.,
                    198.,
                );
                add(
                    format!(
                        "Display Mode: {}",
                        if self.draft.borderless {
                            "Borderless"
                        } else {
                            "Windowed"
                        }
                    ),
                    Cmd::Mode,
                    -202.,
                    25.,
                    198.,
                );
                let (w, h) = SIZES[self.draft.size];
                add(format!("Window Size: {w}x{h}"), Cmd::Size, 4., 25., 198.);
                add(
                    format!("VSync: {}", if self.draft.vsync { "On" } else { "Off" }),
                    Cmd::Vsync,
                    -202.,
                    -10.,
                    198.,
                );
            }
            Page::Input => {
                add(
                    format!("Sensitivity: {}%", self.draft.sensitivity),
                    Cmd::Sensitivity,
                    -202.,
                    60.,
                    198.,
                );
                add(
                    format!(
                        "Invert Y: {}",
                        if self.draft.invert_y { "On" } else { "Off" }
                    ),
                    Cmd::Invert,
                    4.,
                    60.,
                    198.,
                );
                add(
                    "Key Binds...".into(),
                    Cmd::Page(Page::Keys(0)),
                    -100.,
                    20.,
                    200.,
                );
                add("Reset Key Binds".into(), Cmd::ResetKeys, -100., -15., 200.);
            }
            Page::Keys(page) => {
                for (row, i) in (page * 6..((page + 1) * 6).min(19)).enumerate() {
                    add(
                        format!("{}: {}", LABELS[i], labels.name(self.draft.keys[i])),
                        Cmd::Bind(i),
                        -180.,
                        66. - row as f32 * 25.,
                        360.,
                    );
                }
                add(
                    "<".into(),
                    Cmd::Page(Page::Keys((page + 3) % 4)),
                    -202.,
                    -89.,
                    55.,
                );
                add(
                    ">".into(),
                    Cmd::Page(Page::Keys((page + 1) % 4)),
                    147.,
                    -89.,
                    55.,
                );
            }
            Page::General => {
                add(
                    "Restore All Defaults".into(),
                    Cmd::ResetAll,
                    -150.,
                    45.,
                    300.,
                );
            }
        }
        add("Apply".into(), Cmd::Apply, -202., -119., 198.);
        add(
            if self.page == Page::Home {
                "Back (Discard Changes)"
            } else {
                "Back"
            }
            .into(),
            Cmd::Back,
            4.,
            -119.,
            198.,
        );
        v
    }
}
pub(super) fn open(world: &mut World) {
    let origin = world.resource::<ClientFlow>().state;
    let draft = world.resource::<Preferences>().clone();
    *world.resource_mut::<Editor>() = Editor {
        origin,
        draft,
        ..default()
    };
    world.resource_mut::<ClientFlow>().state = ClientState::Settings;
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
}
fn rollback(world: &mut World) {
    let preview = world.resource_mut::<Editor>().preview.take();
    if let Some(preview) = preview {
        let old = preview.old;
        if let Ok(mut w) = world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(world)
        {
            w.mode = preview.mode;
            w.resolution = preview.resolution;
            w.present_mode = preview.present_mode;
        }
        let mut e = world.resource_mut::<Editor>();
        e.draft.borderless = old.borderless;
        e.draft.size = old.size;
        e.revision += 1;
    }
}
fn save(world: &mut World) {
    let e = world.resource::<Editor>();
    let (p, reset) = (e.draft.clone(), e.reset);
    world.resource_mut::<Config>().save(p, reset);
}
fn activate(world: &mut World, cmd: Cmd) {
    world.resource_mut::<Editor>().error.clear();
    world.resource_mut::<Editor>().drag = None;
    match cmd {
        Cmd::Page(page) => {
            let mut e = world.resource_mut::<Editor>();
            e.page = page;
            e.focus = 0;
            e.pressed = None;
        }
        Cmd::Fov | Cmd::Sensitivity | Cmd::Radius => {}
        Cmd::Mode => {
            let mut e = world.resource_mut::<Editor>();
            e.draft.borderless = !e.draft.borderless;
        }
        Cmd::Size => {
            let mut e = world.resource_mut::<Editor>();
            e.draft.size = (e.draft.size + 1) % SIZES.len();
        }
        Cmd::Vsync => {
            let mut e = world.resource_mut::<Editor>();
            e.draft.vsync = !e.draft.vsync;
        }
        Cmd::Invert => {
            let mut e = world.resource_mut::<Editor>();
            e.draft.invert_y = !e.draft.invert_y;
        }
        Cmd::Bind(i) => {
            world.resource_mut::<Editor>().capture = Some(i);
            world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        }
        Cmd::ResetKeys => world.resource_mut::<Editor>().draft.keys = Preferences::default().keys,
        Cmd::ResetAll => {
            let mut e = world.resource_mut::<Editor>();
            e.draft = default();
            e.reset = true;
        }
        Cmd::Apply => {
            if let Err(error) = world.resource::<Editor>().draft.validate() {
                world.resource_mut::<Editor>().error = error;
                return;
            }
            let old = world.resource::<Preferences>().clone();
            let p = world.resource::<Editor>().draft.clone();
            if old.borderless != p.borderless || old.size != p.size {
                let deadline =
                    world.resource::<Time<Real>>().elapsed() + std::time::Duration::from_secs(15);
                let preview = if let Ok(mut w) = world
                    .query_filtered::<&mut Window, With<PrimaryWindow>>()
                    .single_mut(world)
                {
                    let preview = Preview {
                        old,
                        mode: w.mode,
                        resolution: w.resolution.clone(),
                        present_mode: w.present_mode,
                        deadline,
                    };
                    p.window(&mut w);
                    Some(preview)
                } else {
                    None
                };
                world.resource_mut::<Editor>().preview = preview;
            } else {
                save(world);
            }
        }
        Cmd::Confirm => {
            // Restore while IO runs. A failed save cannot leave an unconfirmed mode.
            let p = world.resource::<Editor>().draft.clone();
            rollback(world);
            world.resource_mut::<Editor>().draft = p;
            save(world);
        }
        Cmd::Cancel => rollback(world),
        Cmd::Back => {
            let page = world.resource::<Editor>().page;
            if page == Page::Home {
                world.resource_mut::<ClientFlow>().state = world.resource::<Editor>().origin;
                world.resource_mut::<Navigation>().stamp.clear();
            } else {
                let mut e = world.resource_mut::<Editor>();
                e.page = if matches!(page, Page::Keys(_)) {
                    Page::Input
                } else {
                    Page::Home
                };
                e.focus = 0;
            }
        }
    }
    world.resource_mut::<Editor>().revision += 1;
}
impl Cmd {
    fn range(self) -> Option<(i32, i32)> {
        match self {
            Self::Fov => Some((45, 110)),
            Self::Sensitivity => Some((10, 300)),
            Self::Radius => Some((1, voxel_core::streaming::MAX_RADIUS)),
            _ => None,
        }
    }
    fn value(self, e: &Editor) -> i32 {
        match self {
            Self::Fov => i32::from(e.draft.fov),
            Self::Sensitivity => i32::from(e.draft.sensitivity),
            Self::Radius => e.draft.radius,
            _ => 0,
        }
    }
    fn set(self, e: &mut Editor, value: i32) {
        let Some((min, max)) = self.range() else {
            return;
        };
        let value = value.clamp(min, max);
        match self {
            Self::Fov => e.draft.fov = value as u16,
            Self::Sensitivity => e.draft.sensitivity = value as u16,
            Self::Radius => e.draft.radius = value,
            _ => return,
        }
        e.revision += 1;
    }
    fn fraction(self, e: &Editor) -> Option<f32> {
        self.range()
            .map(|(min, max)| (self.value(e) - min) as f32 / (max - min) as f32)
    }
}
fn slider(e: &mut Editor, cmd: Cmd, amount: f32) {
    if let Some((min, max)) = cmd.range() {
        cmd.set(
            e,
            (min as f32 + amount.clamp(0., 1.) * (max - min) as f32).round() as i32,
        );
    }
}
pub(super) fn input(world: &mut World) {
    if world.resource::<Config>().job.is_some() {
        return;
    }
    if world.resource::<ClientFlow>().destination == Some(Destination::Quit) {
        rollback(world);
        if world.contains_resource::<crate::scene::VoxelWorld>() {
            world.resource_mut::<ClientFlow>().save(Destination::Quit);
        } else {
            world.write_message(AppExit::Success);
        }
        return;
    }
    let (focused, pointer) = {
        let Ok(w) = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
        else {
            return;
        };
        (
            w.focused,
            w.cursor_position()
                .map(|p| Vec2::new(p.x - w.width() / 2., w.height() / 2. - p.y) / scale(w)),
        )
    };
    let now = world.resource::<Time<Real>>().elapsed();
    if world
        .resource::<Editor>()
        .preview
        .as_ref()
        .is_some_and(|preview| now >= preview.deadline || !focused)
    {
        rollback(world);
    }
    if !focused {
        let mut e = world.resource_mut::<Editor>();
        e.pressed = None;
        e.drag = None;
        e.capture = None;
        e.hovered = None;
        return;
    }
    let k = world.resource::<ButtonInput<KeyCode>>();
    let escape = k.just_pressed(KeyCode::Escape);
    if let Some(index) = world.resource::<Editor>().capture {
        let next = k.get_just_pressed().copied().next();
        if let Some(key) = next {
            let mut e = world.resource_mut::<Editor>();
            if escape {
                e.capture = None;
            } else {
                let mut candidate = e.draft.clone();
                candidate.keys[index] = key;
                match candidate.validate() {
                    Ok(()) => {
                        e.draft = candidate;
                        e.capture = None;
                    }
                    Err(error) => e.error = error,
                }
            }
            e.revision += 1;
            world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        }
        return;
    }
    if escape {
        if world.resource::<Editor>().preview.is_some() {
            rollback(world)
        } else {
            activate(world, Cmd::Back)
        }
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        return;
    }
    let up = k.just_pressed(KeyCode::ArrowUp);
    let down = k.just_pressed(KeyCode::ArrowDown) || k.just_pressed(KeyCode::Tab);
    let left = k.just_pressed(KeyCode::ArrowLeft);
    let right = k.just_pressed(KeyCode::ArrowRight);
    let enter = k.just_pressed(KeyCode::Enter);
    let release = k.just_released(KeyCode::Enter);
    let m = world.resource::<ButtonInput<MouseButton>>();
    let (press, mouse_release, held) = (
        m.just_pressed(MouseButton::Left),
        m.just_released(MouseButton::Left),
        m.pressed(MouseButton::Left),
    );
    let controls = world
        .resource::<Editor>()
        .controls(world.resource::<KeyLabels>());
    let hit = pointer.and_then(|p| {
        controls.iter().position(|c| {
            let [x, y, w, h] = c.rect;
            p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
        })
    });
    let mut e = world.resource_mut::<Editor>();
    e.focus = e.focus.min(controls.len() - 1);
    e.hovered = hit;
    if pointer != e.pointer {
        if let Some(i) = hit {
            e.focus = i;
        }
        e.pointer = pointer;
    }
    if up {
        e.focus = (e.focus + controls.len() - 1) % controls.len();
        e.pressed = None;
    }
    if down {
        e.focus = (e.focus + 1) % controls.len();
        e.pressed = None;
    }
    if left || right {
        let cmd = controls[e.focus].cmd;
        let value = cmd.value(&e) + if right { 1 } else { -1 };
        cmd.set(&mut e, value);
        e.revision += 1;
    }
    if enter || press && hit.is_some() {
        if press {
            e.focus = hit.unwrap();
        }
        e.pressed = Some(e.focus);
        let cmd = controls[e.focus].cmd;
        if press && cmd.range().is_some() {
            e.drag = Some(cmd);
        }
    }
    if held && let (Some(cmd), Some(p)) = (e.drag, pointer) {
        let rect = controls.iter().find(|c| c.cmd == cmd).unwrap().rect;
        slider(&mut e, cmd, (p.x - rect[0] - 4.) / (rect[2] - 8.));
    }
    let action = if release || mouse_release {
        e.drag = None;
        e.pressed
            .take()
            .filter(|&i| release || hit == Some(i))
            .map(|i| controls[i].cmd)
    } else {
        None
    };
    if let Some(cmd) = action {
        activate(world, cmd)
    }
}
pub(super) fn paint(
    e: &Editor,
    config: &Config,
    preferences: &Preferences,
    labels: &KeyLabels,
    c: &mut Canvas,
) {
    let cream = [0.94, 0.95, 0.96, 1.];
    c.centered(
        108.,
        match e.page {
            Page::Home => "OPTIONS",
            Page::Graphics => "VIDEO SETTINGS",
            Page::Input => "CONTROLS",
            Page::Keys(_) => "KEY BINDS",
            Page::General => "GENERAL",
        },
        1.5,
        cream,
    );
    if e.preview.is_some() {
        c.centered(
            45.,
            "KEEP THESE SETTINGS? CONFIRM WITHIN 15 SECONDS",
            1.,
            cream,
        );
    }
    for (i, control) in e.controls(labels).iter().enumerate() {
        c.button(
            control.rect,
            &control.label,
            i == e.focus,
            e.hovered == Some(i),
            e.pressed == Some(i),
            config.job.is_none(),
        );
        let fraction = control.cmd.fraction(e);
        if let Some(t) = fraction {
            let [x, y, w, _] = control.rect;
            c.rect(x + 4., y + 2., w - 8., 2., [0.06, 0.07, 0.08, 1.]);
            c.rect(x + 4. + t * (w - 12.), y + 1., 4., 5., cream);
        }
    }
    if e.preview.is_some() {
        return;
    }
    if let Page::Keys(page) = e.page {
        c.centered(
            91.,
            "OS KEY NAMES / SCAN = PHYSICAL POSITION UNTIL PRESSED",
            0.65,
            cream,
        );
        c.centered(-83., &format!("PAGE {} / 4", page + 1), 0.8, cream);
    }
    if e.page == Page::Graphics {
        c.centered(
            -48.,
            "FOV = VERTICAL FIELD OF VIEW (45-110 DEGREES)",
            0.8,
            cream,
        );
        if std::env::var_os("RUSTCRAFT_LOAD_RADIUS").is_some() {
            c.centered(
                -65.,
                "OVERRIDDEN BY RUSTCRAFT_LOAD_RADIUS",
                0.8,
                [1., 0.7, 0.4, 1.],
            );
        }
    }
    if e.page == Page::General {
        c.centered(10., "DEFAULTS TAKE EFFECT AFTER APPLY", 0.8, cream);
        c.centered(-5., "DAMAGED FILE IS PRESERVED AS .REJECTED", 0.8, cream);
    }
    let message = if e.capture.is_some() {
        "PRESS A KEY / ESC TO CANCEL"
    } else if !e.error.is_empty() {
        &e.error
    } else if config.job.is_some() {
        "SAVING OPTIONS..."
    } else if e.draft != *preferences && (config.error.is_empty() || config.error == "Saved") {
        "CHANGES NOT APPLIED"
    } else {
        &config.error
    };
    for (i, line) in message
        .chars()
        .take(72)
        .collect::<Vec<_>>()
        .chunks(72)
        .enumerate()
    {
        c.centered(
            -96. - i as f32 * 8.,
            &line.iter().collect::<String>(),
            0.75,
            [1., 0.75, 0.45, 1.],
        );
    }
}
#[cfg(test)]
mod tests;
