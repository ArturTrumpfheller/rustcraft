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
            self.revision, self.page, self.focus, self.pressed
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
                        [-220., 48. - (index - self.offset) as f32 * 32., 440., 29.],
                    );
                }
                add(
                    "< Vorherige".into(),
                    (self.offset > 0).then_some(Command::Previous),
                    [-220., -34., 110., 16.],
                );
                add(
                    "Naechste >".into(),
                    (self.offset + 3 < self.entries.len()).then_some(Command::Next),
                    [110., -34., 110., 16.],
                );
                let e = self.entry();
                let managed = e.is_some_and(|e| matches!(e.key, WorldKey::Managed(_)));
                let valid = managed && e.is_some_and(|e| e.error.is_none());
                for (i, (label, cmd)) in [
                    ("Welt spielen", valid.then_some(Command::Play)),
                    ("Neue Welt", Some(Command::Create)),
                    ("Umbenennen", valid.then_some(Command::Rename)),
                    ("Archivieren", managed.then_some(Command::Archive)),
                    ("Untersuchen", e.map(|_| Command::Details)),
                    ("Hauptmenü", Some(Command::Back)),
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        label.into(),
                        cmd,
                        [
                            -220. + (i % 3) as f32 * 150.,
                            -63. - (i / 3) as f32 * 24.,
                            140.,
                            20.,
                        ],
                    );
                }
                add(
                    "Aktualisieren".into(),
                    Some(Command::Refresh),
                    [-70., -111., 140., 16.],
                );
            }
            Page::Create | Page::Rename => {
                add(
                    format!("Name: {}", tail(&self.name, 39)),
                    Some(Command::Name),
                    [-220., 26., 440., 25.],
                );
                if self.page == Page::Create {
                    add(
                        format!(
                            "Seed: {}",
                            if self.seed.is_empty() {
                                "Zufaellig".into()
                            } else {
                                self.seed.clone()
                            }
                        ),
                        Some(Command::Seed),
                        [-220., -14., 440., 25.],
                    );
                }
                add(
                    if self.page == Page::Create {
                        "Welt erstellen"
                    } else {
                        "Namen speichern"
                    }
                    .into(),
                    Some(Command::Confirm),
                    [-205., -65., 195., 23.],
                );
                add(
                    "Abbrechen".into(),
                    Some(Command::Back),
                    [10., -65., 195., 23.],
                );
            }
            Page::Archive | Page::Import => {
                add(
                    if self.page == Page::Archive {
                        "Ja, archivieren"
                    } else {
                        "Ja, Kopie importieren"
                    }
                    .into(),
                    Some(Command::Confirm),
                    [-215., -65., 205., 23.],
                );
                add(
                    "Abbrechen".into(),
                    Some(Command::Back),
                    [10., -65., 205., 23.],
                );
            }
            Page::Details => {
                if self.entry().is_some_and(|e| e.key == WorldKey::Legacy) {
                    add(
                        "Legacy importieren...".into(),
                        Some(Command::Import),
                        [-215., -75., 205., 23.],
                    );
                }
                add(
                    "Zurück zur Liste".into(),
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
            Err("Weltenauftrag unerwartet beendet; Dateien bleiben erhalten".into())
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
            b.name = "Neue Welt".into();
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
                        c.import_legacy("Importierte Welt")
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
        return "unbekannt".into();
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
    let cream = [0.94, 0.88, 0.73, 1.];
    let copper = [0.76, 0.43, 0.21, 1.];
    let muted = [0.53, 0.65, 0.65, 1.];
    c.centered(112., "RUSTCRAFT", 1.4, cream);
    c.centered(
        91.,
        match b.page {
            Page::List => "WELTENAUSWAHL",
            Page::Create => "NEUE WELT ERSTELLEN",
            Page::Rename => "WELT UMBENENNEN",
            Page::Archive => "WELT ARCHIVIEREN?",
            Page::Import => "LEGACY ALS KOPIE IMPORTIEREN?",
            Page::Details => "WELT UNTERSUCHEN",
        },
        1.1,
        copper,
    );
    if b.job.is_some() {
        c.centered(10., "DATEIEN WERDEN GEPRÜFT...", 1.2, cream);
        c.centered(-10., "BITTE WARTEN", 1., muted);
        return;
    }
    if b.page == Page::List {
        if b.entries.is_empty() {
            c.centered(24., "NOCH KEINE WELTEN - NEUE WELT ERSTELLEN", 0.9, cream);
        }
        c.centered(
            -29.,
            &format!("{} WELTEN / SEITE {}", b.entries.len(), b.offset / 3 + 1),
            0.75,
            muted,
        );
    } else if matches!(b.page, Page::Create | Page::Rename) {
        c.text(
            -218.,
            61.,
            "NAME FREI WAEHLBAR / DOPPELTE NAMEN ERLAUBT",
            0.85,
            muted,
        );
        c.centered(
            -34.,
            if b.page == Page::Create {
                "LEERER SEED = ZUFALL / GENERATOR V3"
            } else {
                "NUR DER ANZEIGENAME WIRD GEAENDERT"
            },
            0.85,
            muted,
        );
        c.centered(-91., "TAB: FELD WECHSELN / CTRL+A: LEEREN", 0.85, muted);
    } else if let Some(e) = b.entry() {
        c.centered(64., &clip(&e.name, 45), 1., cream);
        if b.page == Page::Details {
            let lines = [
                format!(
                    "Typ: {} / Seed: {}",
                    e.kind,
                    e.seed.map_or("?".into(), |s| s.to_string())
                ),
                format!("Pfad: {}", e.path.display()),
                e.error
                    .clone()
                    .unwrap_or_else(|| "Validiert. Ganze Verzeichnisse sichern.".into()),
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
            c.centered(-104., "VOLLSTAENDIGER PFAD UND FEHLER IM LOG", 0.85, muted);
        } else {
            c.centered(
                29.,
                if b.page == Page::Archive {
                    "VERSCHIEBEN NACH ARCHIVE / KEIN LOESCHEN"
                } else {
                    "VOLLSTAENDIGE KOPIE DES ALTEN SPIELSTANDS"
                },
                0.9,
                cream,
            );
            c.centered(
                9.,
                if b.page == Page::Archive {
                    "WELT DARF NICHT GEOEFFNET SEIN"
                } else {
                    "ORIGINAL BLEIBT ERHALTEN / KEIN VERSCHIEBEN"
                },
                0.9,
                muted,
            );
            c.centered(-16., "NUR NACH DEINER BESTAETIGUNG", 0.95, copper);
        }
    }
    for (i, control) in b.controls().iter().enumerate() {
        let [x, y, w, h] = control.rect;
        let focused = i == b.focus;
        let enabled = control.action.is_some();
        let selected = matches!(control.action,Some(Command::Select(index)) if Some(&b.entries[index].key)==b.selected.as_ref());
        c.rect(
            x - 1.,
            y - 1.,
            w + 2.,
            h + 2.,
            if (focused || selected) && enabled {
                copper
            } else {
                [0.2, 0.27, 0.29, 1.]
            },
        );
        c.rect(
            x,
            y,
            w,
            h,
            if b.pressed == Some(i) {
                [0.18, 0.28, 0.29, 1.]
            } else {
                [0.07, 0.12, 0.15, 1.]
            },
        );
        c.text(
            x + 5.,
            y + h - 10.,
            &control.label,
            if h < 24. { 0.85 } else { 1. },
            if enabled { cream } else { muted },
        );
        if let Some(Command::Select(index)) = control.action {
            let e = &b.entries[index];
            let status = if let Some(error) = &e.error {
                format!("FEHLER: {}", clip(error, 65))
            } else if e.key == WorldKey::Legacy {
                "LEGACY / UNTERSUCHEN UND IMPORTIEREN".into()
            } else {
                format!(
                    "{} / Seed {} / Gen {} / {}",
                    e.kind,
                    e.seed.map_or("?".into(), |s| s.to_string()),
                    e.generator.map_or("?".into(), |s| s.to_string()),
                    e.created.map_or("Datum ?".into(), date)
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
