//! Safe, minimal DOM boundary for the WebAssembly gallery adapter.

use wasm_bindgen::JsValue;
use web_sys::{Document, Element, Window};

pub(super) fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("The gallery needs a browser document."))
}

pub(super) fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("The gallery needs a browser window."))
}

pub(super) fn element(
    document: &Document,
    tag: &str,
    class_name: &str,
) -> Result<Element, JsValue> {
    let element = document.create_element(tag)?;
    if !class_name.is_empty() {
        element.set_attribute("class", class_name)?;
    }
    Ok(element)
}

pub(super) fn text_element(
    document: &Document,
    parent: &Element,
    tag: &str,
    text: &str,
    class_name: &str,
) -> Result<Element, JsValue> {
    let child = element(document, tag, class_name)?;
    child.set_text_content(Some(text));
    parent.append_child(&child)?;
    Ok(child)
}

pub(super) fn clear_children(parent: &Element) -> Result<(), JsValue> {
    while let Some(child) = parent.first_child() {
        parent.remove_child(&child)?;
    }
    Ok(())
}
