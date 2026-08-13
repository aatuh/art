//! Renderer selection boundary between the artwork catalog and WebGL implementations.

use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::{ArtworkKind, fps::PlayerState};

use super::{Renderer, earth_renderer::PlanetRenderer};

pub(super) enum SceneRenderer {
    Room(Renderer),
    Planet(PlanetRenderer),
}

impl SceneRenderer {
    pub(super) fn render(&self, player: PlayerState, now: f64) {
        match self {
            Self::Room(renderer) => renderer.render(player),
            Self::Planet(renderer) => renderer.render(player, now),
        }
    }
}

/// The exhaustive domain enum forces a renderer decision for every published artwork.
pub(super) fn create(
    kind: ArtworkKind,
    canvas: &HtmlCanvasElement,
) -> Result<SceneRenderer, JsValue> {
    match kind {
        ArtworkKind::BlackCubeRoom => Renderer::new(canvas).map(SceneRenderer::Room),
        ArtworkKind::OrbitingEarth => PlanetRenderer::new(canvas).map(SceneRenderer::Planet),
    }
}
