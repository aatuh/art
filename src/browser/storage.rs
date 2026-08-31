//! Local, validated persistence for non-sensitive visitor control preferences.

use crate::input::InputSettings;
use web_sys::Storage;

use super::dom::window;

const STORAGE_KEY: &str = "black-cube-gallery.fps-controls.v1";

pub(super) fn read_settings() -> InputSettings {
    let value = storage().and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten());
    InputSettings::from_storage(value.as_deref())
}

pub(super) fn save_settings(settings: &InputSettings) {
    let Some(storage) = storage() else {
        return;
    };
    if let Ok(value) = serde_json::to_string(settings) {
        let _ = storage.set_item(STORAGE_KEY, &value);
    }
}

fn storage() -> Option<Storage> {
    window().ok()?.local_storage().ok().flatten()
}
