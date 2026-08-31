//! Browser composition root for the static gallery application.

use wasm_bindgen::prelude::*;

mod artworks;
mod catalogue;
mod controls;
mod dom;
mod installation;
mod lifecycle;
mod runtime;
mod storage;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = dom::document()?;
    let root = document
        .get_element_by_id("gallery")
        .ok_or_else(|| JsValue::from_str("The gallery root is unavailable."))?;
    catalogue::render(&document, &root)
}
