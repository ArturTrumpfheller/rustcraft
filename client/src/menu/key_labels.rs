//! Layout-aware display names from the OS-translated Bevy input stream.
//! Bindings remain physical KeyCodes; this cache is never serialized.
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};
use std::collections::HashMap;
#[derive(Resource, Default)]
pub(super) struct KeyLabels {
    observed: HashMap<KeyCode, String>,
    pub revision: u64,
}
impl KeyLabels {
    pub fn name(&self, key: KeyCode) -> String {
        self.observed.get(&key).cloned().unwrap_or_else(|| {
            let name = crate::settings::key_name(key);
            if format!("{key:?}").starts_with("Key") || format!("{key:?}").starts_with("Digit") {
                format!("Scan {name}")
            } else {
                name
            }
        })
    }
    fn observe(&mut self, key: KeyCode, logical: &Key) {
        let position = format!("{key:?}");
        if !position.starts_with("Key") && !position.starts_with("Digit") {
            return;
        }
        let Key::Character(text) = logical else {
            return;
        };
        let mut chars = text.chars();
        let Some(ch) = chars.next() else { return };
        // Ignore composed text and modifier-generated punctuation; letters are
        // case-normalized so Shift does not change labels or imply a layout switch.
        if chars.next().is_some() || !ch.is_alphanumeric() {
            return;
        }
        let name = ch.to_uppercase().to_string();
        if self.observed.get(&key) == Some(&name) {
            return;
        }
        // An OS layout change invalidates other learned character labels too.
        if self.observed.contains_key(&key)
            || self.observed.iter().any(|(k, n)| *k != key && *n == name)
        {
            self.observed.clear();
        }
        self.observed.insert(key, name);
        self.revision += 1;
    }
}
pub(super) fn observe(
    mut events: MessageReader<KeyboardInput>,
    mut labels: ResMut<KeyLabels>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
) {
    for event in events.read() {
        if event.state == ButtonState::Pressed
            && !event.repeat
            && windows
                .iter()
                .any(|(id, w)| id == event.window && w.focused)
        {
            labels.observe(event.key_code, &event.logical_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qwertz_and_qwerty_labels_follow_logical_events_without_changing_physical_bindings() {
        for (physical_z, physical_y) in [
            (KeyCode::KeyY, KeyCode::KeyZ),
            (KeyCode::KeyZ, KeyCode::KeyY),
        ] {
            let mut labels = KeyLabels::default();
            labels.observe(physical_z, &Key::Character("z".into()));
            labels.observe(physical_y, &Key::Character("y".into()));
            assert_eq!(labels.name(physical_z), "Z");
            assert_eq!(labels.name(physical_y), "Y");
            assert_eq!(
                KeyLabels::default().name(physical_z),
                format!("Scan {}", crate::settings::key_name(physical_z))
            );
        }
    }
    #[test]
    fn layout_changes_invalidate_stale_labels_and_shift_case_does_not() {
        let mut labels = KeyLabels::default();
        labels.observe(KeyCode::KeyY, &Key::Character("y".into()));
        labels.observe(KeyCode::KeyZ, &Key::Character("z".into()));
        let revision = labels.revision;
        labels.observe(KeyCode::KeyZ, &Key::Character("Z".into()));
        assert_eq!(labels.revision, revision);
        labels.observe(KeyCode::KeyY, &Key::Character("z".into()));
        assert_eq!(labels.name(KeyCode::KeyY), "Z");
        assert_eq!(labels.name(KeyCode::KeyZ), "Scan Z");
        labels.observe(KeyCode::KeyZ, &Key::Character("y".into()));
        assert_eq!(labels.name(KeyCode::KeyZ), "Y");
    }
}
