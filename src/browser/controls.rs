//! Shared browser input adapter.
//!
//! This component owns pointer lock, configurable bindings, and raw browser events. It delegates
//! simulation and artwork-specific commands to the mounted installation runtime.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use wasm_bindgen::{JsCast, prelude::JsValue};
use web_sys::{Document, Element, HtmlInputElement, KeyboardEvent, MouseEvent};

use crate::input::{InputSettings, is_supported_key_code};

use super::{
    dom::{element, text_element, window},
    lifecycle::{AnimationFrameLoop, EventListeners, ViewLifecycle},
    runtime::{ActionState, InstallationRuntime, VisitorAction},
    storage::save_settings,
};

pub(super) type RuntimeHandle = Rc<RefCell<Box<dyn InstallationRuntime>>>;

pub(super) struct Session {
    settings: InputSettings,
    pressed: BTreeSet<String>,
    listening: Option<VisitorAction>,
    binding_buttons: Vec<(VisitorAction, Element)>,
    active: bool,
    pointer_locked: bool,
    last_frame_ms: Option<f64>,
}

impl Session {
    pub(super) fn new(settings: InputSettings) -> Self {
        Self {
            settings,
            pressed: BTreeSet::new(),
            listening: None,
            binding_buttons: Vec::new(),
            active: true,
            pointer_locked: false,
            last_frame_ms: None,
        }
    }

    pub(super) fn deactivate(&mut self) {
        self.active = false;
        self.pressed.clear();
    }

    fn binding(&self, action: VisitorAction) -> &str {
        match action {
            VisitorAction::Forward => &self.settings.bindings.forward,
            VisitorAction::Backward => &self.settings.bindings.backward,
            VisitorAction::Left => &self.settings.bindings.left,
            VisitorAction::Right => &self.settings.bindings.right,
            VisitorAction::Jump => &self.settings.bindings.jump,
            VisitorAction::Crouch => &self.settings.bindings.crouch,
        }
    }

    fn set_binding(&mut self, action: VisitorAction, value: String) {
        match action {
            VisitorAction::Forward => self.settings.bindings.forward = value,
            VisitorAction::Backward => self.settings.bindings.backward = value,
            VisitorAction::Left => self.settings.bindings.left = value,
            VisitorAction::Right => self.settings.bindings.right = value,
            VisitorAction::Jump => self.settings.bindings.jump = value,
            VisitorAction::Crouch => self.settings.bindings.crouch = value,
        }
    }

    fn action_for_code(&self, code: &str) -> Option<VisitorAction> {
        VisitorAction::ALL.into_iter().find(|action| {
            let binding = self.binding(*action);
            binding == code
                || (*action == VisitorAction::Crouch
                    && matches!(binding, "ControlLeft" | "ControlRight")
                    && matches!(code, "ControlLeft" | "ControlRight"))
        })
    }

    fn action_state(&self) -> ActionState {
        let pressed = |action| {
            let binding = self.binding(action);
            self.pressed.contains(binding)
                || (action == VisitorAction::Crouch
                    && matches!(binding, "ControlLeft" | "ControlRight")
                    && (self.pressed.contains("ControlLeft")
                        || self.pressed.contains("ControlRight")))
        };
        ActionState {
            forward: pressed(VisitorAction::Forward),
            backward: pressed(VisitorAction::Backward),
            left: pressed(VisitorAction::Left),
            right: pressed(VisitorAction::Right),
            jump: pressed(VisitorAction::Jump),
            crouch: pressed(VisitorAction::Crouch),
        }
    }

    fn update_binding_buttons(&self) {
        for (action, button) in &self.binding_buttons {
            button.set_text_content(Some(&format!(
                "{}: {}",
                action.label(),
                self.binding(*action)
            )));
            let _ = button.remove_attribute("data-listening");
        }
    }
}

pub(super) fn create_settings_panel(
    document: &Document,
    session: &Rc<RefCell<Session>>,
    listeners: &mut EventListeners,
) -> Result<Element, JsValue> {
    let panel = element(document, "aside", "fps-settings")?;
    panel.set_attribute("hidden", "true")?;
    panel.set_attribute("aria-label", "First-person controls")?;
    text_element(document, &panel, "h2", "Controls", "")?;
    text_element(
        document,
        &panel,
        "p",
        "To reassign a key, click its current key binding, then press the key you want to use.",
        "settings-note",
    )?;

    let sensitivity_label = element(document, "label", "settings-label")?;
    sensitivity_label.set_text_content(Some("Mouse sensitivity"));
    let sensitivity =
        element(document, "input", "sensitivity-input")?.dyn_into::<HtmlInputElement>()?;
    sensitivity.set_attribute("type", "range")?;
    sensitivity.set_attribute("min", "0.03")?;
    sensitivity.set_attribute("max", "0.8")?;
    sensitivity.set_attribute("step", "0.01")?;
    sensitivity.set_value(&session.borrow().settings.mouse_sensitivity.to_string());
    sensitivity_label.append_child(&sensitivity)?;
    panel.append_child(&sensitivity_label)?;

    let session_for_sensitivity = Rc::clone(session);
    let sensitivity_for_change = sensitivity.clone();
    listeners.listen(&sensitivity, "input", move |_| {
        if let Ok(value) = sensitivity_for_change.value().parse::<f32>() {
            let mut session = session_for_sensitivity.borrow_mut();
            session.settings.mouse_sensitivity = value.clamp(0.03, 0.8);
            sensitivity_for_change.set_value(&session.settings.mouse_sensitivity.to_string());
            save_settings(&session.settings);
        }
    })?;

    let invert_label = element(document, "label", "settings-check")?;
    let invert = element(document, "input", "")?.dyn_into::<HtmlInputElement>()?;
    invert.set_attribute("type", "checkbox")?;
    invert.set_checked(session.borrow().settings.invert_mouse_y);
    invert_label.append_child(&invert)?;
    text_element(
        document,
        &invert_label,
        "span",
        "Invert vertical mouse look",
        "",
    )?;
    panel.append_child(&invert_label)?;
    let session_for_invert = Rc::clone(session);
    let invert_for_change = invert.clone();
    listeners.listen(&invert, "change", move |_| {
        let mut session = session_for_invert.borrow_mut();
        session.settings.invert_mouse_y = invert_for_change.checked();
        save_settings(&session.settings);
    })?;

    let bindings = element(document, "div", "binding-list")?;
    for action in VisitorAction::ALL {
        let button = element(document, "button", "binding-button")?;
        button.set_attribute("type", "button")?;
        {
            let mut session = session.borrow_mut();
            button.set_text_content(Some(&format!(
                "{}: {}",
                action.label(),
                session.binding(action)
            )));
            session.binding_buttons.push((action, button.clone()));
        }
        let session_for_binding = Rc::clone(session);
        let button_for_binding = button.clone();
        listeners.listen(&button, "click", move |_| {
            let mut session = session_for_binding.borrow_mut();
            session.listening = Some(action);
            for (_, existing) in &session.binding_buttons {
                let _ = existing.remove_attribute("data-listening");
            }
            let _ = button_for_binding.set_attribute("data-listening", "true");
            button_for_binding.set_text_content(Some("Press a key…"));
        })?;
        bindings.append_child(&button)?;
    }
    panel.append_child(&bindings)?;
    Ok(panel)
}

pub(super) fn attach_controls(
    document: &Document,
    installation: &Element,
    observer: &Element,
    session: &Rc<RefCell<Session>>,
    runtime: RuntimeHandle,
    lifecycle: &mut ViewLifecycle,
) -> Result<(), JsValue> {
    installation.set_attribute("data-navigation", runtime.borrow().navigation_label())?;

    let session_for_lock = Rc::clone(session);
    let runtime_for_lock = Rc::clone(&runtime);
    let document_for_lock = document.clone();
    let installation_for_lock = installation.clone();
    lifecycle
        .listeners
        .listen(document, "pointerlockchange", move |_| {
            let locked = document_for_lock.pointer_lock_element().is_some();
            let mut session = session_for_lock.borrow_mut();
            session.pointer_locked = locked;
            if locked {
                let _ = installation_for_lock.set_attribute("data-pointer-locked", "true");
            } else {
                let _ = installation_for_lock.remove_attribute("data-pointer-locked");
                release_pressed_keys(&mut session, &runtime_for_lock);
            }
        })?;

    let session_for_mouse = Rc::clone(session);
    let runtime_for_mouse = Rc::clone(&runtime);
    lifecycle
        .listeners
        .listen(document, "mousemove", move |event| {
            let Ok(event) = event.dyn_into::<MouseEvent>() else {
                return;
            };
            let session = session_for_mouse.borrow();
            if !session.active || !session.pointer_locked {
                return;
            }
            runtime_for_mouse.borrow_mut().look(
                event.movement_x() as f64,
                event.movement_y() as f64,
                session.settings.mouse_sensitivity,
                session.settings.invert_mouse_y,
            );
        })?;

    let session_for_down = Rc::clone(session);
    let runtime_for_down = Rc::clone(&runtime);
    lifecycle
        .listeners
        .listen(document, "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            let mut session = session_for_down.borrow_mut();
            if !session.active {
                return;
            }
            if let Some(action) = session.listening {
                if is_supported_key_code(&event.code()) && !event.repeat() {
                    session.set_binding(action, event.code());
                    session.settings = session.settings.clone().sanitized();
                    session.listening = None;
                    session.pressed.clear();
                    session.update_binding_buttons();
                    save_settings(&session.settings);
                    event.prevent_default();
                }
                return;
            }
            if event_target_is_form_control(&event) {
                return;
            }

            let code = event.code();
            let action = session.action_for_code(&code);
            if let Some(action) = action {
                runtime_for_down
                    .borrow_mut()
                    .action_changed(action, true, event.repeat());
            }
            let command =
                runtime_for_down
                    .borrow_mut()
                    .command_key_changed(&code, true, event.repeat());
            if action.is_none() && !command {
                return;
            }
            session.pressed.insert(code);
            event.prevent_default();
        })?;

    let session_for_up = Rc::clone(session);
    let runtime_for_up = Rc::clone(&runtime);
    lifecycle
        .listeners
        .listen(document, "keyup", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            let mut session = session_for_up.borrow_mut();
            if !session.active {
                return;
            }
            let code = event.code();
            let action = session.action_for_code(&code);
            if let Some(action) = action {
                runtime_for_up
                    .borrow_mut()
                    .action_changed(action, false, false);
            }
            let command = runtime_for_up
                .borrow_mut()
                .command_key_changed(&code, false, false);
            session.pressed.remove(&code);
            if action.is_some() || command {
                event.prevent_default();
            }
        })?;

    let session_for_blur = Rc::clone(session);
    let runtime_for_blur = Rc::clone(&runtime);
    lifecycle.listeners.listen(&window()?, "blur", move |_| {
        release_pressed_keys(&mut session_for_blur.borrow_mut(), &runtime_for_blur);
    })?;

    lifecycle.set_animation(start_render_loop(session, runtime, observer)?);
    Ok(())
}

fn release_pressed_keys(session: &mut Session, runtime: &RuntimeHandle) {
    for code in std::mem::take(&mut session.pressed) {
        if let Some(action) = session.action_for_code(&code) {
            runtime.borrow_mut().action_changed(action, false, false);
        }
        runtime
            .borrow_mut()
            .command_key_changed(&code, false, false);
    }
}

fn start_render_loop(
    session: &Rc<RefCell<Session>>,
    runtime: RuntimeHandle,
    observer: &Element,
) -> Result<AnimationFrameLoop, JsValue> {
    let session_for_frame = Rc::clone(session);
    let observer_for_frame = observer.clone();
    AnimationFrameLoop::start(move |now| {
        let (delta, actions) = {
            let mut session = session_for_frame.borrow_mut();
            if !session.active {
                return false;
            }
            let delta = session
                .last_frame_ms
                .replace(now)
                .map(|last_frame| (now - last_frame) / 1_000.0)
                .unwrap_or(0.0);
            (delta, session.action_state())
        };

        let mut runtime = runtime.borrow_mut();
        runtime.frame(now, delta, actions);
        let _ = observer_for_frame.set_attribute(
            "data-crouching",
            if runtime.is_crouching() {
                "true"
            } else {
                "false"
            },
        );
        true
    })
}

fn event_target_is_form_control(event: &KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| matches!(element.tag_name().as_str(), "INPUT" | "BUTTON"))
}
