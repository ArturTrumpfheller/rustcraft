//! Catalog UI reuses Slice13's canvas/lifecycle. Disk operations run on one
//! worker; changing page never mutates a save. Confirmation pins a world key.
use super::*;
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};
use std::thread::{self, JoinHandle};
use voxel_core::persistence::catalog::{self, WorldCatalog, WorldEntry, WorldKey};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    List,
    Create,
    Rename,
    Archive,
    Import,
    Details,
}
#[derive(Clone, Debug)]
enum Command {
    Select(usize),
    Previous,
    Next,
    Play,
    Create,
    Rename,
    Archive,
    Import,
    Details,
    Refresh,
    Back,
    Name,
    Seed,
    Confirm,
}
struct Control {
    label: String,
    action: Option<Command>,
    rect: [f32; 4],
}
type Job = JoinHandle<Result<(Vec<WorldEntry>, Option<WorldKey>), String>>;
#[derive(Resource, Default)]
pub(super) struct Browser {
    entries: Vec<WorldEntry>,
    selected: Option<WorldKey>,
    page: Page,
    offset: usize,
    focus: usize,
    pressed: Option<usize>,
    pointer: Option<Vec2>,
    hovered: Option<usize>,
    name: String,
    seed: String,
    error: String,
    job: Option<Job>,
    revision: u64,
}
impl Browser {
    pub fn stamp(&self) -> String {
        format!(
            "{}/{:?}/{}/{:?}",
            self.revision,
            self.page,
            self.focus,
            (self.pressed, self.hovered)
        )
    }
    fn entry(&self) -> Option<&WorldEntry> {
        self.entries
            .iter()
            .find(|e| Some(&e.key) == self.selected.as_ref())
    }
    fn start(
        &mut self,
        f: impl FnOnce(WorldCatalog) -> Result<Option<WorldKey>, String> + Send + 'static,
    ) {
        self.error.clear();
        self.revision += 1;
        self.job = match crate::persistence::data_directory()
            .map_err(|e| e.to_string())
            .and_then(|p| WorldCatalog::new(p).map_err(|e| e.to_string()))
        {
            Ok(c) => match thread::Builder::new()
                .name("rustcraft-catalog".into())
                .spawn(move || {
                    let selected = f(c.clone())?;
                    Ok((c.list().map_err(|e| e.to_string())?, selected))
                }) {
                Ok(j) => Some(j),
                Err(e) => {
                    self.error = e.to_string();
                    None
                }
            },
            Err(e) => {
                self.error = e;
                None
            }
        };
    }
    fn refresh(&mut self) {
        self.start(|_| Ok(None));
    }
    fn controls(&self) -> Vec<Control> {
        if self.job.is_some() {
            return vec![];
        }
        let mut out = vec![];
        let mut add = |label: String, action: Option<Command>, rect| {
            out.push(Control {
                label,
                action,
                rect,
            })
        };
        match self.page {
            Page::List => {
                for (index, e) in self.entries.iter().enumerate().skip(self.offset).take(3) {
                    let selected = Some(&e.key) == self.selected.as_ref();
                    add(
                        format!("{}{}", if selected { "> " } else { "" }, clip(&e.name, 34)),
                        Some(Command::Select(index)),
                        [-208., 47. - (index - self.offset) as f32 * 44., 416., 40.],
                    );
                }
                add(
                    "< Previous".into(),
                    (self.offset > 0).then_some(Command::Previous),
                    [-208., -58., 100., 16.],
                );
                add(
                    "Next >".into(),
                    (self.offset + 3 < self.entries.len()).then_some(Command::Next),
                    [108., -58., 100., 16.],
                );
                let e = self.entry();
                let managed = e.is_some_and(|e| matches!(e.key, WorldKey::Managed(_)));
                let valid = managed && e.is_some_and(|e| e.error.is_none());
                for (i, (label, cmd)) in [
                    ("Play Selected World", valid.then_some(Command::Play)),
                    ("Create New World", Some(Command::Create)),
                    ("Rename", valid.then_some(Command::Rename)),
                    ("Archive", managed.then_some(Command::Archive)),
                    ("Inspect", e.map(|_| Command::Details)),
                    ("Main Menu", Some(Command::Back)),
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        label.into(),
                        cmd,
                        if i < 2 {
                            [-208. + i as f32 * 212., -84., 204., 20.]
                        } else {
                            [-208. + (i - 2) as f32 * 106., -109., 98., 20.]
                        },
                    );
                }
                add(
                    "Refresh".into(),
                    Some(Command::Refresh),
                    [108., 102., 100., 16.],
                );
            }
            Page::Create | Page::Rename => {
                add(
                    format!("Name: {}", tail(&self.name, 39)),
                    Some(Command::Name),
                    [-150., 28., 300., 22.],
                );
                if self.page == Page::Create {
                    add(
                        format!(
                            "Seed: {}",
                            if self.seed.is_empty() {
                                "Random".into()
                            } else {
                                self.seed.clone()
                            }
                        ),
                        Some(Command::Seed),
                        [-150., -20., 300., 22.],
                    );
                }
                add(
                    if self.page == Page::Create {
                        "Create World"
                    } else {
                        "Save Name"
                    }
                    .into(),
                    Some(Command::Confirm),
                    [-204., -101., 198., 20.],
                );
                add("Cancel".into(), Some(Command::Back), [6., -101., 198., 20.]);
            }
            Page::Archive | Page::Import => {
                add(
                    if self.page == Page::Archive {
                        "Yes, Archive"
                    } else {
                        "Yes, Import a Copy"
                    }
                    .into(),
                    Some(Command::Confirm),
                    [-204., -67., 198., 20.],
                );
                add("Cancel".into(), Some(Command::Back), [6., -67., 198., 20.]);
            }
            Page::Details => {
                if self.entry().is_some_and(|e| e.key == WorldKey::Legacy) {
                    add(
                        "Import Legacy World...".into(),
                        Some(Command::Import),
                        [-215., -75., 205., 23.],
                    );
                }
                add(
                    "Back to World List".into(),
                    Some(Command::Back),
                    [10., -75., 205., 23.],
                );
            }
        }
        out
    }
}
pub(super) fn open(world: &mut World) {
    world.resource_mut::<ClientFlow>().state = ClientState::Worlds;
    let mut b = world.resource_mut::<Browser>();
    b.page = Page::List;
    b.focus = 0;
    b.refresh();
}
pub(super) fn poll(world: &mut World) {
    let done = world
        .resource::<Browser>()
        .job
        .as_ref()
        .is_some_and(|j| j.is_finished());
    if done {
        let job = world.resource_mut::<Browser>().job.take().unwrap();
        let result = job.join().unwrap_or_else(|_| {
            Err("World operation stopped unexpectedly; files are preserved".into())
        });
        let mut b = world.resource_mut::<Browser>();
        match result {
            Ok((entries, selected)) => {
                b.entries = entries;
                if selected.is_some() {
                    b.selected = selected;
                }
                if !b
                    .entries
                    .iter()
                    .any(|e| Some(&e.key) == b.selected.as_ref())
                {
                    b.selected = b.entries.first().map(|e| e.key.clone());
                }
                if let Some(index) = b
                    .entries
                    .iter()
                    .position(|e| Some(&e.key) == b.selected.as_ref())
                {
                    b.offset = index / 3 * 3;
                }
                b.page = Page::List;
                b.focus = 0;
            }
            Err(e) => {
                error!("Catalog operation failed; files retained: {e}");
                b.error = e;
            }
        }
        b.revision += 1;
    }
    if world.resource::<ClientFlow>().state == ClientState::Worlds
        && world.resource::<ClientFlow>().destination == Some(Destination::Quit)
        && world.resource::<Browser>().job.is_none()
    {
        world.write_message(AppExit::Success);
    }
}
pub(super) fn text_input(
    mut events: MessageReader<KeyboardInput>,
    flow: Res<ClientFlow>,
    mut b: ResMut<Browser>,
    windows: Query<&Window, With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for event in events.read() {
        if flow.state != ClientState::Worlds
            || b.job.is_some()
            || !windows.iter().any(|w| w.focused)
            || !matches!(b.page, Page::Create | Page::Rename)
            || event.state != ButtonState::Pressed
        {
            continue;
        }
        let field = b.controls().get(b.focus).and_then(|c| c.action.clone());
        let max = if matches!(field, Some(Command::Seed)) {
            20
        } else {
            96
        };
        let text = match field {
            Some(Command::Name) => &mut b.name,
            Some(Command::Seed) => &mut b.seed,
            _ => continue,
        };
        if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
            if event.key_code == KeyCode::KeyA {
                text.clear();
            }
        } else if event.logical_key == Key::Backspace {
            text.pop();
        } else if let Some(input) = &event.text {
            for ch in input.chars().filter(|c| !c.is_control()) {
                if text.chars().count() < max {
                    text.push(ch);
                }
            }
        }
        b.revision += 1;
    }
}
pub(super) fn input(world: &mut World) {
    let controls = world.resource::<Browser>().controls();
    if controls.is_empty() {
        return;
    }
    let (focused, pointer, hovered) = {
        let Ok(w) = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
        else {
            return;
        };
        let point = w
            .cursor_position()
            .map(|p| (p - Vec2::new(w.width(), w.height()) / 2.) / scale(w) * Vec2::new(1., -1.));
        let hit = point.and_then(|p| {
            controls.iter().position(|c| {
                let [x, y, width, h] = c.rect;
                p.x >= x && p.x <= x + width && p.y >= y && p.y <= y + h
            })
        });
        (w.focused, w.cursor_position(), hit)
    };
    if !focused {
        world.resource_mut::<Browser>().pressed = None;
        return;
    }
    let k = world.resource::<ButtonInput<KeyCode>>();
    let up = k.just_pressed(KeyCode::ArrowUp)
        || k.just_pressed(KeyCode::Tab) && k.pressed(KeyCode::ShiftLeft);
    let down = k.just_pressed(KeyCode::ArrowDown)
        || k.just_pressed(KeyCode::Tab) && !k.pressed(KeyCode::ShiftLeft);
    let enter = k.just_pressed(KeyCode::Enter);
    let release = k.just_released(KeyCode::Enter);
    let escape = k.just_pressed(KeyCode::Escape);
    let prev = k.just_pressed(KeyCode::PageUp);
    let next = k.just_pressed(KeyCode::PageDown);
    let mouse = world.resource::<ButtonInput<MouseButton>>();
    let click = mouse.just_pressed(MouseButton::Left);
    let released = mouse.just_released(MouseButton::Left);
    let mut b = world.resource_mut::<Browser>();
    b.focus = b.focus.min(controls.len() - 1);
    // Text focus changes on click/Tab, never from merely moving across fields.
    if b.page == Page::List
        && pointer != b.pointer
        && let Some(i) = hovered
    {
        b.focus = i;
    }
    b.pointer = pointer;
    b.hovered = hovered;
    if up {
        b.focus = (b.focus + controls.len() - 1) % controls.len();
        b.pressed = None;
    }
    if down {
        b.focus = (b.focus + 1) % controls.len();
        b.pressed = None;
    }
    if click && let Some(i) = hovered {
        b.focus = i;
        b.pressed = Some(i);
    }
    if enter {
        b.pressed = Some(b.focus);
    }
    let action = if escape {
        Some(Command::Back)
    } else if b.page == Page::List && prev {
        Some(Command::Previous)
    } else if b.page == Page::List && next {
        Some(Command::Next)
    } else if release || released {
        b.pressed
            .take()
            .filter(|&i| i == b.focus && (release || hovered == Some(i)))
            .and_then(|i| controls[i].action.clone())
    } else {
        None
    };
    if let Some(cmd) = action {
        activate(world, cmd);
    }
}
fn activate(world: &mut World, cmd: Command) {
    if matches!(cmd, Command::Play) {
        if let Some(WorldKey::Managed(id)) = world.resource::<Browser>().selected.clone() {
            world.resource_mut::<ClientFlow>().selected_world = Some(id);
            session::begin_loading(world);
        }
        return;
    }
    let mut b = world.resource_mut::<Browser>();
    b.error.clear();
    b.revision += 1;
    b.pressed = None;
    match cmd {
        Command::Select(i) => b.selected = Some(b.entries[i].key.clone()),
        Command::Previous => {
            b.offset = b.offset.saturating_sub(3);
            b.focus = 0;
        }
        Command::Next => {
            if b.offset + 3 < b.entries.len() {
                b.offset += 3;
            }
            b.focus = 0;
        }
        Command::Refresh => b.refresh(),
        Command::Create => {
            b.page = Page::Create;
            b.name = "New World".into();
            b.seed = std::env::var("RUSTCRAFT_WORLD_SEED").unwrap_or_default();
            b.focus = 0;
        }
        Command::Rename => {
            b.name = b.entry().map(|e| e.name.clone()).unwrap_or_default();
            b.page = Page::Rename;
            b.focus = 0;
        }
        Command::Archive => {
            b.page = Page::Archive;
            b.focus = 1;
        } // Cancel initially selected.
        Command::Import => {
            b.page = Page::Import;
            b.focus = 1;
        }
        Command::Details => {
            b.page = Page::Details;
            b.focus = 0;
            if let Some(e) = b.entry() {
                info!("World inspection: {} / {:?}", e.path.display(), e.error);
            }
        }
        Command::Back => {
            if b.page == Page::List {
                *world.resource_mut::<ClientFlow>() = ClientFlow::default();
            } else {
                b.page = Page::List;
                b.focus = 0;
            }
        }
        Command::Name | Command::Seed => {}
        Command::Confirm => {
            if matches!(b.page, Page::Create | Page::Rename)
                && let Err(e) = catalog::validate_name(&b.name)
            {
                b.error = e.to_string();
                return;
            }
            let name = b.name.clone();
            let key = b.selected.clone();
            match b.page {
                Page::Create => {
                    let seed = match catalog::parse_seed(&b.seed) {
                        Ok(s) => s.unwrap_or_else(catalog::random_seed),
                        Err(e) => {
                            b.error = e.to_string();
                            return;
                        }
                    };
                    b.start(move |c| {
                        c.create(&name, seed)
                            .map(|id| Some(WorldKey::Managed(id)))
                            .map_err(|e| e.to_string())
                    });
                }
                Page::Rename => {
                    if let Some(WorldKey::Managed(id)) = key {
                        b.start(move |c| {
                            c.rename(&id, &name)
                                .map(|_| Some(WorldKey::Managed(id)))
                                .map_err(|e| e.to_string())
                        });
                    }
                }
                Page::Archive => {
                    if let Some(WorldKey::Managed(id)) = key {
                        b.start(move |c| c.archive(&id).map(|_| None).map_err(|e| e.to_string()));
                    }
                }
                Page::Import => {
                    b.start(|c| {
                        c.import_legacy("Imported World")
                            .map(|id| Some(WorldKey::Managed(id)))
                            .map_err(|e| e.to_string())
                    });
                }
                _ => {}
            }
        }
        Command::Play => unreachable!(),
    }
}
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}...", s.chars().take(n - 3).collect::<String>())
    } else {
        s.into()
    }
}
fn tail(s: &str, n: usize) -> String {
    s.chars()
        .skip(s.chars().count().saturating_sub(n))
        .collect()
}
fn date(t: u64) -> String {
    let mut days = t / 86400;
    let mut year = 1970;
    if days > 2932896 {
        return "unknown".into();
    }
    loop {
        let count = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < count {
            break;
        }
        days -= count;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut month = 1;
    for count in [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if days < count {
            break;
        }
        days -= count;
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02} UTC", days + 1)
}
pub(super) fn paint(b: &Browser, c: &mut Canvas) {
    let cream = [0.94, 0.95, 0.96, 1.];
    let copper = [0.69, 0.74, 0.8, 1.];
    let muted = [0.52, 0.55, 0.6, 1.];

    c.centered(
        105.,
        match b.page {
            Page::List => "SELECT WORLD",
            Page::Create => "CREATE NEW WORLD",
            Page::Rename => "RENAME WORLD",
            Page::Archive => "ARCHIVE WORLD?",
            Page::Import => "IMPORT A COPY OF THE LEGACY WORLD?",
            Page::Details => "INSPECT WORLD",
        },
        1.1,
        copper,
    );
    if b.job.is_some() {
        c.centered(10., "CHECKING FILES...", 1.2, cream);
        c.centered(-10., "PLEASE WAIT", 1., muted);
        return;
    }
    if b.page == Page::List {
        c.rect(-211., -41., 422., 132., [0.009, 0.011, 0.015, 1.]);
        c.rect(-211., 90., 422., 1., [0.16, 0.18, 0.21, 1.]);
    }
    if b.page == Page::List {
        if b.entries.is_empty() {
            c.centered(24., "NO WORLDS YET - CREATE A NEW WORLD", 0.9, cream);
        }
        c.centered(
            -54.,
            &format!("WORLDS: {} / PAGE {}", b.entries.len(), b.offset / 3 + 1),
            0.75,
            muted,
        );
    } else if matches!(b.page, Page::Create | Page::Rename) {
        c.text(
            -150.,
            61.,
            "CHOOSE ANY NAME / DUPLICATES ARE ALLOWED",
            0.7,
            muted,
        );
        let identity_note = if b.page == Page::Create {
            format!(
                "EMPTY SEED = RANDOM / GENERATOR V{}",
                voxel_core::worldgen::CURRENT_LOCAL_GENERATOR
            )
        } else {
            "ONLY THE DISPLAY NAME WILL CHANGE".into()
        };
        c.centered(-39., &identity_note, 0.85, muted);
        c.centered(-66., "TAB: NEXT FIELD / CTRL+A: CLEAR", 0.85, muted);
    } else if let Some(e) = b.entry() {
        c.centered(64., &clip(&e.name, 45), 1., cream);
        if b.page == Page::Details {
            let lines = [
                format!(
                    "Type: {} / Seed: {}",
                    e.kind,
                    e.seed.map_or("?".into(), |s| s.to_string())
                ),
                format!("Path: {}", e.path.display()),
                e.error
                    .clone()
                    .unwrap_or_else(|| "Validated. Back up entire directories.".into()),
            ];
            let detail = lines.join("\n");
            let mut y = 46.;
            for line in detail.lines() {
                for chunk in line.chars().collect::<Vec<_>>().chunks(72) {
                    if y < -53. {
                        break;
                    }
                    c.text(-218., y, &chunk.iter().collect::<String>(), 0.85, cream);
                    y -= 10.;
                }
            }
            c.centered(-104., "FULL PATH AND ERROR ARE IN THE LOG", 0.85, muted);
        } else {
            c.centered(
                29.,
                if b.page == Page::Archive {
                    "MOVE TO ARCHIVE / NO PERMANENT DELETION"
                } else {
                    "COPY THE ENTIRE LEGACY SAVE"
                },
                0.9,
                cream,
            );
            c.centered(
                9.,
                if b.page == Page::Archive {
                    "THE WORLD MUST NOT BE OPEN"
                } else {
                    "ORIGINAL IS PRESERVED / NOTHING IS MOVED"
                },
                0.9,
                muted,
            );
            c.centered(-16., "ONLY AFTER YOUR CONFIRMATION", 0.95, copper);
        }
    }
    for (i, control) in b.controls().iter().enumerate() {
        let [x, y, w, h] = control.rect;
        let focused = i == b.focus;
        let enabled = control.action.is_some();
        let selected = matches!(control.action,Some(Command::Select(index)) if Some(&b.entries[index].key)==b.selected.as_ref());
        let row = matches!(control.action, Some(Command::Select(_)));
        let field = matches!(control.action, Some(Command::Name | Command::Seed));
        if row || field {
            c.rect(
                x - 1.,
                y - 1.,
                w + 2.,
                h + 2.,
                if selected || field && focused {
                    cream
                } else if focused {
                    copper
                } else {
                    [0.08, 0.09, 0.11, 1.]
                },
            );
            c.rect(
                x,
                y,
                w,
                h,
                if selected {
                    [0.09, 0.11, 0.14, 1.]
                } else {
                    [0.012, 0.015, 0.02, 1.]
                },
            );
            c.text(x + 7., y + h - 12., &control.label, 1., cream);
        } else {
            c.button(
                control.rect,
                &control.label,
                focused,
                b.hovered == Some(i),
                b.pressed == Some(i),
                enabled,
            );
        }
        if let Some(Command::Select(index)) = control.action {
            let e = &b.entries[index];
            let status = if let Some(error) = &e.error {
                format!("ERROR: {}", clip(error, 65))
            } else if e.key == WorldKey::Legacy {
                "LEGACY / INSPECT AND IMPORT".into()
            } else {
                format!(
                    "{} / Seed {} / Gen {} / {}",
                    e.kind,
                    e.seed.map_or("?".into(), |s| s.to_string()),
                    e.generator.map_or("?".into(), |s| s.to_string()),
                    e.created.map_or("Date ?".into(), date)
                )
            };
            c.text(
                x + 5.,
                y + 4.,
                &status,
                0.75,
                if e.error.is_some() {
                    [1., 0.58, 0.39, 1.]
                } else {
                    muted
                },
            );
        }
    }
    if !b.error.is_empty() {
        for (i, line) in b
            .error
            .chars()
            .take(144)
            .collect::<Vec<_>>()
            .chunks(72)
            .enumerate()
        {
            c.centered(
                -124. + i as f32 * 9.,
                &line.iter().collect::<String>(),
                0.8,
                [1., 0.58, 0.39, 1.],
            );
        }
    }
}

#[cfg(test)]
mod tests;
