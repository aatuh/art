//! Visitor controller and installation UI for A World in Light.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use wasm_bindgen::JsValue;
use web_sys::{Element, HtmlCanvasElement};

use artwork_world_in_light::{
    SpaceflightState as CameraState,
    exhibition_camera::{CelestialTarget, SpaceflightInput},
    format_camera_speed_mps,
    planet::CelestialFrame,
    simulation_clock::SimulationClock,
};

use crate::browser::{
    dom::element,
    runtime::{ActionState, InstallationRuntime, RuntimeContext, VisitorAction},
};

use super::renderer::WorldInLightRenderer;

const UNIX_SECONDS_AT_J2000: f64 = 946_728_000.0;

pub(crate) fn create(
    canvas: &HtmlCanvasElement,
    context: RuntimeContext<'_>,
) -> Result<Box<dyn InstallationRuntime>, JsValue> {
    let renderer = WorldInLightRenderer::new(canvas)?;
    let camera = Rc::new(RefCell::new(CameraState::default()));
    let seconds_since_j2000 = js_sys::Date::now() / 1_000.0 - UNIX_SECONDS_AT_J2000;
    let clock = Rc::new(Cell::new(SimulationClock::new(seconds_since_j2000)));

    let speed_readout = element(context.document, "span", "camera-speed-readout")?;
    speed_readout.set_attribute("aria-label", "Current virtual camera speed")?;
    speed_readout.set_text_content(Some(&format_camera_speed_mps(
        camera.borrow().camera_speed_mps,
    )));
    context.toolbar_actions.append_child(&speed_readout)?;

    let time_button = element(context.document, "button", "time-scale-button")?;
    time_button.set_attribute("type", "button")?;
    update_time_button(&time_button, clock.get().scale())?;
    context.toolbar_actions.append_child(&time_button)?;
    let clock_for_time = Rc::clone(&clock);
    let time_button_for_click = time_button.clone();
    context.listeners.listen(&time_button, "click", move |_| {
        let mut state = clock_for_time.get();
        let scale = state.cycle_scale();
        clock_for_time.set(state);
        let _ = update_time_button(&time_button_for_click, scale);
    })?;

    let target_button = element(context.document, "button", "celestial-target-button")?;
    target_button.set_attribute("type", "button")?;
    let next_target = Rc::new(Cell::new(CelestialTarget::EarthSurface));
    update_target_button(&target_button, next_target.get())?;
    context.toolbar_actions.append_child(&target_button)?;
    let camera_for_target = Rc::clone(&camera);
    let clock_for_target = Rc::clone(&clock);
    let next_target_for_click = Rc::clone(&next_target);
    let target_button_for_click = target_button.clone();
    context
        .listeners
        .listen(&target_button, "click", move |_| {
            let target = next_target_for_click.get();
            let celestial = CelestialFrame::at_seconds_since_j2000(
                clock_for_target.get().seconds_since_epoch(),
            );
            camera_for_target
                .borrow_mut()
                .inspect_celestial_target(target, celestial);
            let following = target.next();
            next_target_for_click.set(following);
            let _ = update_target_button(&target_button_for_click, following);
        })?;

    Ok(Box::new(WorldInLightRuntime {
        renderer,
        camera,
        clock,
        speed_readout,
        time_button,
        shift_left: false,
        shift_right: false,
        braking: false,
    }))
}

struct WorldInLightRuntime {
    renderer: WorldInLightRenderer,
    camera: Rc<RefCell<CameraState>>,
    clock: Rc<Cell<SimulationClock>>,
    speed_readout: Element,
    time_button: Element,
    shift_left: bool,
    shift_right: bool,
    braking: bool,
}

impl InstallationRuntime for WorldInLightRuntime {
    fn navigation_label(&self) -> &'static str {
        "space"
    }

    fn look(&mut self, delta_x: f64, delta_y: f64, sensitivity: f32, invert_y: bool) {
        let vertical_direction = if invert_y { -1.0 } else { 1.0 };
        self.camera.borrow_mut().look(
            delta_x,
            delta_y * vertical_direction,
            sensitivity.to_radians() as f64,
        );
    }

    fn action_changed(&mut self, _action: VisitorAction, _pressed: bool, _repeat: bool) {}

    fn command_key_changed(&mut self, code: &str, pressed: bool, repeat: bool) -> bool {
        match code {
            "ShiftLeft" => self.shift_left = pressed,
            "ShiftRight" => self.shift_right = pressed,
            "KeyX" => self.braking = pressed,
            "KeyF" if pressed && !repeat => self.camera.borrow_mut().point_at_earth(),
            "KeyR" if pressed && !repeat => self.camera.borrow_mut().reset_view(),
            "KeyZ" if pressed && !repeat => self.camera.borrow_mut().adjust_camera_speed(-1),
            "KeyC" if pressed && !repeat => self.camera.borrow_mut().adjust_camera_speed(1),
            "KeyT" if pressed && !repeat => self.cycle_time_scale(),
            "KeyF" | "KeyR" | "KeyZ" | "KeyC" | "KeyT" => {}
            _ => return false,
        }
        true
    }

    fn frame(&mut self, now_ms: f64, delta_seconds: f64, actions: ActionState) {
        let previous_position_m;
        {
            let mut camera = self.camera.borrow_mut();
            previous_position_m = camera.position_m;
            let current_frame =
                CelestialFrame::at_seconds_since_j2000(self.clock.get().seconds_since_epoch());
            let clearance_m = current_frame.nearest_surface_distance_m(camera.position_m);
            camera.tick_with_clearance(
                delta_seconds,
                SpaceflightInput {
                    forward: actions.forward,
                    backward: actions.backward,
                    left: actions.left,
                    right: actions.right,
                    ascend: actions.jump,
                    descend: actions.crouch,
                    accelerated: self.shift_left || self.shift_right,
                    stop: self.braking,
                },
                clearance_m,
            );
        }

        let mut clock = self.clock.get();
        let seconds_since_j2000 = clock.advance(now_ms);
        let animation_seconds = clock.animation_seconds();
        self.clock.set(clock);
        let celestial = CelestialFrame::at_seconds_since_j2000(seconds_since_j2000);

        let mut camera = self.camera.borrow_mut();
        camera.constrain_to_celestial_frame(previous_position_m, celestial);
        let speed_label = format_camera_speed_mps(camera.camera_speed_mps);
        if self.speed_readout.text_content().as_deref() != Some(speed_label.as_str()) {
            self.speed_readout.set_text_content(Some(&speed_label));
        }
        self.renderer
            .render(&camera, celestial, animation_seconds, now_ms);
    }
}

impl WorldInLightRuntime {
    fn cycle_time_scale(&self) {
        let mut state = self.clock.get();
        let scale = state.cycle_scale();
        self.clock.set(state);
        let _ = update_time_button(&self.time_button, scale);
    }
}

fn update_target_button(button: &Element, target: CelestialTarget) -> Result<(), JsValue> {
    let label = format!("View {}", target.label());
    button.set_text_content(Some(&label));
    button.set_attribute(
        "aria-label",
        &format!("Move the camera to view {}", target.label()),
    )
}

fn update_time_button(button: &Element, scale: f64) -> Result<(), JsValue> {
    let value = if scale == 0.0 {
        "paused".to_owned()
    } else {
        format!("{scale:.0}×")
    };
    button.set_text_content(Some(&format!("Time {value}")));
    button.set_attribute(
        "aria-label",
        &format!("Simulation time {value}. Activate to change simulation speed."),
    )
}
