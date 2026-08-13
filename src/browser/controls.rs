//! Pointer-lock input, rebinding UI, and render-loop state for first-person visiting.

use super::{
    dom::{element, text_element, window},
    rendering::SceneRenderer,
    storage::save_settings,
};
use crate::fps::{FpsInput, FpsSettings, PlayerState, is_supported_key_code};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
use wasm_bindgen::{JsCast, closure::Closure, prelude::JsValue};
use web_sys::{Document, Element, Event, HtmlInputElement, KeyboardEvent, MouseEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Crouch,
}

impl Action {
    const ALL: [Self; 6] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Crouch,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Forward => "Forward",
            Self::Backward => "Backward",
            Self::Left => "Strafe left",
            Self::Right => "Strafe right",
            Self::Jump => "Jump",
            Self::Crouch => "Crouch",
        }
    }

    fn binding<'a>(self, settings: &'a FpsSettings) -> &'a str {
        match self {
            Self::Forward => &settings.bindings.forward,
            Self::Backward => &settings.bindings.backward,
            Self::Left => &settings.bindings.left,
            Self::Right => &settings.bindings.right,
            Self::Jump => &settings.bindings.jump,
            Self::Crouch => &settings.bindings.crouch,
        }
    }

    fn set_binding(self, settings: &mut FpsSettings, value: String) {
        match self {
            Self::Forward => settings.bindings.forward = value,
            Self::Backward => settings.bindings.backward = value,
            Self::Left => settings.bindings.left = value,
            Self::Right => settings.bindings.right = value,
            Self::Jump => settings.bindings.jump = value,
            Self::Crouch => settings.bindings.crouch = value,
        }
    }
}

pub(super) struct Session {
    player: PlayerState,
    settings: FpsSettings,
    pressed: BTreeSet<String>,
    listening: Option<Action>,
    binding_buttons: Vec<(Action, Element)>,
    pub(super) active: bool,
    pointer_locked: bool,
    last_frame_ms: Option<f64>,
}

impl Session {
    pub(super) fn new(settings: FpsSettings) -> Self {
        Self {
            player: PlayerState::default(),
            settings,
            pressed: BTreeSet::new(),
            listening: None,
            binding_buttons: Vec::new(),
            active: true,
            pointer_locked: false,
            last_frame_ms: None,
        }
    }

    fn input(&self) -> FpsInput {
        FpsInput {
            forward: self.action_pressed(Action::Forward),
            backward: self.action_pressed(Action::Backward),
            left: self.action_pressed(Action::Left),
            right: self.action_pressed(Action::Right),
            crouch: self.action_pressed(Action::Crouch),
        }
    }

    fn action_pressed(&self, action: Action) -> bool {
        let binding = action.binding(&self.settings);
        self.pressed.contains(binding)
            || (action == Action::Crouch
                && matches!(binding, "ControlLeft" | "ControlRight")
                && (self.pressed.contains("ControlLeft") || self.pressed.contains("ControlRight")))
    }

    fn update_binding_buttons(&self) {
        for (action, button) in &self.binding_buttons {
            button.set_text_content(Some(&format!(
                "{}: {}",
                action.label(),
                action.binding(&self.settings)
            )));
            let _ = button.remove_attribute("data-listening");
        }
    }
}

pub(super) fn create_settings_panel(
    document: &Document,
    session: &Rc<RefCell<Session>>,
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
    let on_sensitivity = Closure::<dyn FnMut(Event)>::new(move |_| {
        if let Ok(value) = sensitivity_for_change.value().parse::<f32>() {
            let mut session = session_for_sensitivity.borrow_mut();
            session.settings.mouse_sensitivity = value.clamp(0.03, 0.8);
            sensitivity_for_change.set_value(&session.settings.mouse_sensitivity.to_string());
            save_settings(&session.settings);
        }
    });
    sensitivity
        .add_event_listener_with_callback("input", on_sensitivity.as_ref().unchecked_ref())?;
    on_sensitivity.forget();

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
    let on_invert = Closure::<dyn FnMut(Event)>::new(move |_| {
        let mut session = session_for_invert.borrow_mut();
        session.settings.invert_mouse_y = invert_for_change.checked();
        save_settings(&session.settings);
    });
    invert.add_event_listener_with_callback("change", on_invert.as_ref().unchecked_ref())?;
    on_invert.forget();

    let bindings = element(document, "div", "binding-list")?;
    for action in Action::ALL {
        let button = element(document, "button", "binding-button")?;
        button.set_attribute("type", "button")?;
        {
            let mut session = session.borrow_mut();
            button.set_text_content(Some(&format!(
                "{}: {}",
                action.label(),
                action.binding(&session.settings)
            )));
            session.binding_buttons.push((action, button.clone()));
        }
        let session_for_binding = Rc::clone(session);
        let button_for_binding = button.clone();
        let on_binding = Closure::<dyn FnMut()>::new(move || {
            let mut session = session_for_binding.borrow_mut();
            session.listening = Some(action);
            for (_, existing) in &session.binding_buttons {
                let _ = existing.remove_attribute("data-listening");
            }
            let _ = button_for_binding.set_attribute("data-listening", "true");
            button_for_binding.set_text_content(Some("Press a key…"));
        });
        button.add_event_listener_with_callback("click", on_binding.as_ref().unchecked_ref())?;
        on_binding.forget();
        bindings.append_child(&button)?;
    }
    panel.append_child(&bindings)?;
    Ok(panel)
}

pub(super) fn attach_fps_controls(
    document: &Document,
    installation: &Element,
    observer: &Element,
    session: &Rc<RefCell<Session>>,
    renderer: Rc<SceneRenderer>,
) -> Result<(), JsValue> {
    let session_for_lock = Rc::clone(session);
    let document_for_lock = document.clone();
    let installation_for_lock = installation.clone();
    let on_pointer_lock = Closure::<dyn FnMut(Event)>::new(move |_| {
        let locked = document_for_lock.pointer_lock_element().is_some();
        session_for_lock.borrow_mut().pointer_locked = locked;
        if locked {
            let _ = installation_for_lock.set_attribute("data-pointer-locked", "true");
        } else {
            let _ = installation_for_lock.remove_attribute("data-pointer-locked");
            session_for_lock.borrow_mut().pressed.clear();
        }
    });
    document.add_event_listener_with_callback(
        "pointerlockchange",
        on_pointer_lock.as_ref().unchecked_ref(),
    )?;
    on_pointer_lock.forget();

    let session_for_mouse = Rc::clone(session);
    let on_mouse_move = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
        let mut session = session_for_mouse.borrow_mut();
        if !session.active || !session.pointer_locked {
            return;
        }
        let settings = session.settings.clone();
        session.player.look(
            event.movement_x() as f32,
            event.movement_y() as f32,
            &settings,
        );
    });
    document
        .add_event_listener_with_callback("mousemove", on_mouse_move.as_ref().unchecked_ref())?;
    on_mouse_move.forget();

    let session_for_down = Rc::clone(session);
    let on_key_down = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        let mut session = session_for_down.borrow_mut();
        if !session.active {
            return;
        }
        if let Some(action) = session.listening {
            if is_supported_key_code(&event.code()) && !event.repeat() {
                action.set_binding(&mut session.settings, event.code());
                session.settings = session.settings.clone().sanitized();
                session.listening = None;
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
        let is_action = Action::ALL
            .iter()
            .any(|action| action.binding(&session.settings) == code)
            || (matches!(
                session.settings.bindings.crouch.as_str(),
                "ControlLeft" | "ControlRight"
            ) && matches!(code.as_str(), "ControlLeft" | "ControlRight"));
        if !is_action {
            return;
        }
        if code == session.settings.bindings.jump && !event.repeat() {
            session.player.request_jump();
        }
        session.pressed.insert(code);
        event.prevent_default();
    });
    document.add_event_listener_with_callback("keydown", on_key_down.as_ref().unchecked_ref())?;
    on_key_down.forget();

    let session_for_up = Rc::clone(session);
    let on_key_up = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        let mut session = session_for_up.borrow_mut();
        if !session.active || event_target_is_form_control(&event) {
            return;
        }
        session.pressed.remove(&event.code());
    });
    document.add_event_listener_with_callback("keyup", on_key_up.as_ref().unchecked_ref())?;
    on_key_up.forget();

    let session_for_blur = Rc::clone(session);
    let on_blur = Closure::<dyn FnMut(Event)>::new(move |_| {
        session_for_blur.borrow_mut().pressed.clear();
    });
    window()?.add_event_listener_with_callback("blur", on_blur.as_ref().unchecked_ref())?;
    on_blur.forget();

    start_render_loop(session, renderer, observer)
}

fn start_render_loop(
    session: &Rc<RefCell<Session>>,
    renderer: Rc<SceneRenderer>,
    observer: &Element,
) -> Result<(), JsValue> {
    let animation: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let animation_for_frame = Rc::clone(&animation);
    let session_for_frame = Rc::clone(session);
    let renderer_for_frame = Rc::clone(&renderer);
    let observer_for_frame = observer.clone();
    let window_for_frame = window()?;
    *animation.borrow_mut() = Some(Closure::new(move |now: f64| {
        let mut session = session_for_frame.borrow_mut();
        if !session.active {
            return;
        }
        let delta = session
            .last_frame_ms
            .replace(now)
            .map(|last_frame| ((now - last_frame) / 1000.0) as f32)
            .unwrap_or(0.0);
        let input = session.input();
        session.player.tick(delta, input);
        renderer_for_frame.render(session.player, now);
        let _ = observer_for_frame.set_attribute(
            "data-crouching",
            if session.player.crouching {
                "true"
            } else {
                "false"
            },
        );
        drop(session);
        if let Some(callback) = animation_for_frame.borrow().as_ref() {
            let _ = window_for_frame.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    let animation_borrow = animation.borrow();
    let callback = animation_borrow
        .as_ref()
        .ok_or_else(|| JsValue::from_str("Animation callback is unavailable."))?;
    window()?.request_animation_frame(callback.as_ref().unchecked_ref())?;
    Ok(())
}

fn event_target_is_form_control(event: &KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| matches!(element.tag_name().as_str(), "INPUT" | "BUTTON"))
}
