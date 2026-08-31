//! Gallery catalogue component. It discovers artworks but knows no renderer details.

use wasm_bindgen::JsValue;
use web_sys::{Document, Element};

use super::{
    artworks,
    dom::{clear_children, element, text_element},
    installation,
    lifecycle::{ViewLifecycle, activate, schedule_transition},
};

pub(super) fn render(document: &Document, root: &Element) -> Result<(), JsValue> {
    clear_children(root)?;
    let mut lifecycle = ViewLifecycle::new();
    let catalogue = element(document, "section", "catalogue")?;
    let content = element(document, "div", "catalogue-content")?;
    text_element(
        document,
        &content,
        "p",
        "A small digital gallery",
        "eyebrow",
    )?;
    text_element(document, &content, "h1", "Choose an installation.", "")?;
    let list = element(document, "div", "artwork-list")?;

    for component in artworks::gallery_world().components() {
        let artwork = component.descriptor;
        let card = element(document, "button", "artwork-card")?;
        card.set_attribute("type", "button")?;
        text_element(document, &card, "p", artwork.artist, "eyebrow")?;
        text_element(document, &card, "h2", artwork.title, "")?;
        text_element(document, &card, "p", artwork.description, "")?;
        text_element(
            document,
            &card,
            "span",
            "Enter installation →",
            "enter-label",
        )?;

        let root_for_click = root.clone();
        let document_for_click = document.clone();
        let artwork_id = artwork.id.as_str();
        lifecycle.listeners.listen(&card, "click", move |_| {
            let document = document_for_click.clone();
            let root = root_for_click.clone();
            schedule_transition(move || installation::render(&document, &root, artwork_id));
        })?;
        list.append_child(&card)?;
    }

    content.append_child(&list)?;
    catalogue.append_child(&content)?;
    root.append_child(&catalogue)?;
    activate(lifecycle)
}
