//! Renderer-independent visitor input preferences shared by all installations.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub forward: String,
    pub backward: String,
    pub left: String,
    pub right: String,
    pub jump: String,
    pub crouch: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            forward: "KeyW".into(),
            backward: "KeyS".into(),
            left: "KeyA".into(),
            right: "KeyD".into(),
            jump: "Space".into(),
            crouch: "ControlLeft".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputSettings {
    pub mouse_sensitivity: f32,
    pub invert_mouse_y: bool,
    pub bindings: KeyBindings,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.14,
            invert_mouse_y: false,
            bindings: KeyBindings::default(),
        }
    }
}

impl InputSettings {
    pub fn from_storage(value: Option<&str>) -> Self {
        value
            .and_then(|value| serde_json::from_str::<Self>(value).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.mouse_sensitivity = self.mouse_sensitivity.clamp(0.03, 0.8);
        let defaults = KeyBindings::default();
        let mut used = Vec::new();
        for (binding, default) in [
            (&mut self.bindings.forward, defaults.forward),
            (&mut self.bindings.backward, defaults.backward),
            (&mut self.bindings.left, defaults.left),
            (&mut self.bindings.right, defaults.right),
            (&mut self.bindings.jump, defaults.jump),
            (&mut self.bindings.crouch, defaults.crouch),
        ] {
            if !is_supported_key_code(binding) || used.iter().any(|used| used == binding) {
                *binding = first_available_binding(&default, &used).to_owned();
            }
            used.push(binding.clone());
        }
        self
    }
}

fn first_available_binding<'a>(preferred: &'a str, used: &[String]) -> &'a str {
    [
        preferred,
        "KeyW",
        "KeyS",
        "KeyA",
        "KeyD",
        "Space",
        "ControlLeft",
        "KeyQ",
        "KeyE",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
    ]
    .into_iter()
    .find(|candidate| !used.iter().any(|used| used == candidate))
    .unwrap_or("KeyZ")
}

pub fn is_supported_key_code(code: &str) -> bool {
    matches!(
        code,
        "Space" | "ControlLeft" | "ControlRight" | "ShiftLeft" | "ShiftRight"
    ) || (code.len() == 4 && code.starts_with("Key") && code.as_bytes()[3].is_ascii_uppercase())
        || matches!(code, "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight")
}

#[cfg(test)]
mod tests {
    use super::InputSettings;

    #[test]
    fn persisted_input_is_bounded_and_uses_unique_safe_codes() {
        let settings = InputSettings::from_storage(Some(
            r#"{"mouse_sensitivity":99,"bindings":{"forward":"<script>","backward":"KeyS","left":"KeyS","right":"KeyS","jump":"KeyS","crouch":"KeyS"}}"#,
        ));
        let bindings = [
            settings.bindings.forward,
            settings.bindings.backward,
            settings.bindings.left,
            settings.bindings.right,
            settings.bindings.jump,
            settings.bindings.crouch,
        ];

        assert_eq!(settings.mouse_sensitivity, 0.8);
        assert_eq!(bindings[0], "KeyW");
        assert_eq!(
            bindings
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            bindings.len()
        );
    }
}
