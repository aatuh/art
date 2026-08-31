//! Browser-side installation runtime port.
//!
//! The gallery shell owns pointer lock and persisted bindings. Each artwork runtime owns
//! exactly one visitor state, its artwork-specific commands, simulation, and renderer.

use wasm_bindgen::JsValue;
use web_sys::{Document, Element, HtmlCanvasElement};

use super::lifecycle::EventListeners;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VisitorAction {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Crouch,
}

impl VisitorAction {
    pub(super) const ALL: [Self; 6] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Crouch,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Forward => "Forward",
            Self::Backward => "Backward",
            Self::Left => "Strafe left",
            Self::Right => "Strafe right",
            Self::Jump => "Jump / ascend",
            Self::Crouch => "Crouch / descend",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ActionState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub crouch: bool,
}

pub(super) struct RuntimeContext<'a> {
    pub document: &'a Document,
    pub toolbar_actions: &'a Element,
    pub listeners: &'a mut EventListeners,
}

pub(super) trait InstallationRuntime {
    /// Stable value exposed only for styling and accessibility hooks.
    fn navigation_label(&self) -> &'static str;

    fn look(&mut self, delta_x: f64, delta_y: f64, sensitivity: f32, invert_y: bool);

    /// Receives configurable gallery actions as edge events.
    fn action_changed(&mut self, action: VisitorAction, pressed: bool, repeat: bool);

    /// Receives artwork-specific commands. Returns whether the key belongs to the artwork.
    fn command_key_changed(&mut self, code: &str, pressed: bool, repeat: bool) -> bool;

    fn frame(&mut self, now_ms: f64, delta_seconds: f64, actions: ActionState);

    fn is_crouching(&self) -> bool {
        false
    }
}

pub(super) type RuntimeFactory = for<'a> fn(
    &HtmlCanvasElement,
    RuntimeContext<'a>,
) -> Result<Box<dyn InstallationRuntime>, JsValue>;
