//! Visitor controller for Black Cube / White Room.

use artwork_black_cube::{BLACK_CUBE_SCENE, PlayerInput, PlayerState};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::browser::runtime::{ActionState, InstallationRuntime, RuntimeContext, VisitorAction};

use super::renderer::BlackCubeRenderer;

pub(crate) fn create(
    canvas: &HtmlCanvasElement,
    _context: RuntimeContext<'_>,
) -> Result<Box<dyn InstallationRuntime>, JsValue> {
    Ok(Box::new(BlackCubeRuntime {
        renderer: BlackCubeRenderer::new(canvas)?,
        player: PlayerState::at_spawn(&BLACK_CUBE_SCENE),
        jump_requested: false,
    }))
}

struct BlackCubeRuntime {
    renderer: BlackCubeRenderer,
    player: PlayerState,
    jump_requested: bool,
}

impl InstallationRuntime for BlackCubeRuntime {
    fn navigation_label(&self) -> &'static str {
        "room"
    }

    fn look(&mut self, delta_x: f64, delta_y: f64, sensitivity: f32, invert_y: bool) {
        self.player
            .look(delta_x as f32, delta_y as f32, sensitivity, invert_y);
    }

    fn action_changed(&mut self, action: VisitorAction, pressed: bool, repeat: bool) {
        if action == VisitorAction::Jump && pressed && !repeat {
            self.jump_requested = true;
        }
    }

    fn command_key_changed(&mut self, _code: &str, _pressed: bool, _repeat: bool) -> bool {
        false
    }

    fn frame(&mut self, _now_ms: f64, delta_seconds: f64, actions: ActionState) {
        self.player.tick(
            &BLACK_CUBE_SCENE,
            delta_seconds as f32,
            PlayerInput {
                forward: actions.forward,
                backward: actions.backward,
                left: actions.left,
                right: actions.right,
                jump_pressed: std::mem::take(&mut self.jump_requested),
                crouch: actions.crouch,
            },
        );
        self.renderer.render(&self.player);
    }

    fn is_crouching(&self) -> bool {
        self.player.is_crouching()
    }
}
