//! Renderer selection boundary between the artwork catalog and WebGL implementations.

use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::{ArtworkKind, fps::PlayerState, spaceflight::SpaceflightState};

use super::{Renderer, earth_renderer::PlanetRenderer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NavigationMode {
    Room,
    Space,
}

pub(super) enum SceneRenderer {
    Room(Renderer),
    Planet(PlanetRenderer),
}

impl SceneRenderer {
    pub(super) fn navigation_mode(&self) -> NavigationMode {
        match self {
            Self::Room(_) => NavigationMode::Room,
            Self::Planet(_) => NavigationMode::Space,
        }
    }

    pub(super) fn render(&self, room_player: PlayerState, spaceflight: SpaceflightState, now: f64) {
        match self {
            Self::Room(renderer) => renderer.render(room_player),
            Self::Planet(renderer) => renderer.render(spaceflight, now),
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
