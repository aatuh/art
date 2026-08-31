//! Shared installation shell. Artwork-owned metadata and renderer factories plug in here.

use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement};

use super::{
    artworks, catalogue,
    controls::{Session, attach_controls, create_settings_panel},
    dom::{clear_children, element, text_element},
    lifecycle::{ViewLifecycle, activate, schedule_transition},
    runtime::RuntimeContext,
    storage::read_settings,
};

pub(super) fn render(document: &Document, root: &Element, artwork_id: &str) -> Result<(), JsValue> {
    let Some(component) = artworks::gallery_world().resolve_artwork(artwork_id) else {
        return catalogue::render(document, root);
    };
    let artwork = component.descriptor;

    clear_children(root)?;
    let mut lifecycle = ViewLifecycle::new();
    let installation = element(document, "article", "installation fps-installation")?;
    installation.set_attribute("data-artwork", artwork.id.as_str())?;
    let canvas = element(document, "canvas", "fps-canvas")?.dyn_into::<HtmlCanvasElement>()?;
    canvas.set_attribute("aria-label", artwork.presentation.canvas_label)?;
    installation.append_child(&canvas)?;

    let observer = element(document, "section", "room-observer")?;
    observer.set_attribute("tabindex", "0")?;
    observer.set_attribute("aria-label", artwork.presentation.observer_label)?;
    let crosshair = element(document, "span", "crosshair")?;
    crosshair.set_attribute("aria-hidden", "true")?;
    observer.append_child(&crosshair)?;
    installation.append_child(&observer)?;

    let toolbar = element(document, "header", "installation-toolbar")?;
    let back = element(document, "button", "back-button")?;
    back.set_attribute("type", "button")?;
    back.set_text_content(Some("← Gallery"));
    toolbar.append_child(&back)?;
    text_element(document, &toolbar, "span", artwork.title, "")?;
    let toolbar_actions = element(document, "div", "toolbar-actions")?;
    let enter = element(document, "button", "enter-fps-button")?;
    enter.set_attribute("type", "button")?;
    enter.set_text_content(Some(artwork.presentation.enter_label));
    let fullscreen = element(document, "button", "fullscreen-button")?;
    fullscreen.set_attribute("type", "button")?;
    fullscreen.set_attribute("aria-pressed", "false")?;
    fullscreen.set_text_content(Some("Full screen"));
    let settings_toggle = element(document, "button", "settings-button")?;
    settings_toggle.set_attribute("type", "button")?;
    settings_toggle.set_attribute("aria-expanded", "false")?;
    settings_toggle.set_text_content(Some("Controls"));
    toolbar_actions.append_child(&enter)?;
    toolbar_actions.append_child(&fullscreen)?;
    toolbar_actions.append_child(&settings_toggle)?;
    toolbar.append_child(&toolbar_actions)?;
    installation.append_child(&toolbar)?;

    let saved_settings = read_settings();
    let session = Rc::new(RefCell::new(Session::new(saved_settings)));
    let settings_panel = create_settings_panel(document, &session, &mut lifecycle.listeners)?;
    installation.append_child(&settings_panel)?;
    text_element(
        document,
        &installation,
        "p",
        artwork.presentation.controls_help,
        "observer-help",
    )?;

    let shell = ShellElements {
        installation: &installation,
        observer: &observer,
        back: &back,
        enter: &enter,
        fullscreen: &fullscreen,
        settings_toggle: &settings_toggle,
        settings_panel: &settings_panel,
    };
    attach_shell_listeners(document, root, shell, &session, &mut lifecycle)?;

    let runtime_context = RuntimeContext {
        document,
        toolbar_actions: &toolbar_actions,
        listeners: &mut lifecycle.listeners,
    };
    match (component.create_runtime)(&canvas, runtime_context) {
        Ok(runtime) => {
            attach_controls(
                document,
                &installation,
                &observer,
                &session,
                Rc::new(RefCell::new(runtime)),
                &mut lifecycle,
            )?;
        }
        Err(_) => {
            let error = text_element(
                document,
                &installation,
                "p",
                "This browser could not start the gallery’s WebGL installation. Try a current desktop browser with hardware graphics enabled.",
                "graphics-error",
            )?;
            error.set_attribute("role", "alert")?;
        }
    }

    root.append_child(&installation)?;
    activate(lifecycle)
}

struct ShellElements<'a> {
    installation: &'a Element,
    observer: &'a Element,
    back: &'a Element,
    enter: &'a Element,
    fullscreen: &'a Element,
    settings_toggle: &'a Element,
    settings_panel: &'a Element,
}

fn attach_shell_listeners(
    document: &Document,
    root: &Element,
    shell: ShellElements<'_>,
    session: &Rc<RefCell<Session>>,
    lifecycle: &mut ViewLifecycle,
) -> Result<(), JsValue> {
    let root_for_back = root.clone();
    let document_for_back = document.clone();
    let session_for_back = Rc::clone(session);
    lifecycle.listeners.listen(shell.back, "click", move |_| {
        session_for_back.borrow_mut().deactivate();
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            document.exit_pointer_lock();
            if document.fullscreen_element().is_some() {
                document.exit_fullscreen();
            }
        }
        let document = document_for_back.clone();
        let root = root_for_back.clone();
        schedule_transition(move || catalogue::render(&document, &root));
    })?;

    let observer_for_enter = shell.observer.clone();
    lifecycle.listeners.listen(shell.enter, "click", move |_| {
        if let Some(observer) = observer_for_enter.dyn_ref::<HtmlElement>() {
            let _ = observer.focus();
        }
        observer_for_enter.request_pointer_lock();
    })?;

    let document_for_fullscreen = document.clone();
    let installation_for_fullscreen = shell.installation.clone();
    lifecycle
        .listeners
        .listen(shell.fullscreen, "click", move |_| {
            if document_for_fullscreen.fullscreen_element().is_some() {
                document_for_fullscreen.exit_fullscreen();
            } else {
                let _ = installation_for_fullscreen.request_fullscreen();
            }
        })?;

    let document_for_fullscreen_change = document.clone();
    let fullscreen_for_change = shell.fullscreen.clone();
    lifecycle
        .listeners
        .listen(document, "fullscreenchange", move |_| {
            let active = document_for_fullscreen_change
                .fullscreen_element()
                .is_some();
            fullscreen_for_change.set_text_content(Some(if active {
                "Exit full screen"
            } else {
                "Full screen"
            }));
            let _ = fullscreen_for_change
                .set_attribute("aria-pressed", if active { "true" } else { "false" });
        })?;

    let observer_for_click = shell.observer.clone();
    lifecycle
        .listeners
        .listen(shell.observer, "click", move |_| {
            if let Some(observer) = observer_for_click.dyn_ref::<HtmlElement>() {
                let _ = observer.focus();
            }
            observer_for_click.request_pointer_lock();
        })?;

    let panel_for_toggle = shell.settings_panel.clone();
    let toggle_for_toggle = shell.settings_toggle.clone();
    lifecycle
        .listeners
        .listen(shell.settings_toggle, "click", move |_| {
            let open = panel_for_toggle.has_attribute("hidden");
            if open {
                let _ = panel_for_toggle.remove_attribute("hidden");
                let _ = toggle_for_toggle.set_attribute("aria-expanded", "true");
                if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                    document.exit_pointer_lock();
                }
            } else {
                let _ = panel_for_toggle.set_attribute("hidden", "true");
                let _ = toggle_for_toggle.set_attribute("aria-expanded", "false");
            }
        })?;
    Ok(())
}
