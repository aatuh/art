//! Camera-relative, physically scaled WebGL 2 renderer for A World in Light.

#[path = "earth_surface.rs"]
mod surface;
#[path = "earth_renderer/texture.rs"]
mod texture;

use std::{cell::Cell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    Element, HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext as Gl, WebGlBuffer,
    WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::{
    camera_api::CameraState,
    navigation_display::format_camera_speed_mps,
    planet::{
        CelestialFrame, EARTH_ATMOSPHERE_TOP_M, EARTH_EQUATORIAL_RADIUS_M, EARTH_POLAR_RADIUS_M,
        MOON_MEAN_RADIUS_M, SUN_NOMINAL_RADIUS_M, Vec3d,
    },
    simulation_clock::SimulationClock,
    surface_lod::{SurfaceTier, select_surface_tier},
};

use super::dom::window;

const UNIX_SECONDS_AT_J2000: f64 = 946_728_000.0;
const MAX_DEVICE_PIXEL_RATIO: f64 = 2.0;
const EARTH_SURFACE_512_WEBP_B64: &str = include_str!("earth_surface_512.webp.b64");
const EARTH_ELEVATION_512_PNG_B64: &str = include_str!("earth_elevation_512.png.b64");
const EARTH_REGIONAL_SURFACE_PATH: &str = "./assets/earth/earth-surface-1024.webp";

pub(super) struct PlanetRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    _buffer: WebGlBuffer,
    _vertex_array: WebGlVertexArrayObject,
    global_surface_texture: WebGlTexture,
    orbital_surface_texture: WebGlTexture,
    regional_surface_texture: WebGlTexture,
    land_mask_texture: WebGlTexture,
    elevation_texture: WebGlTexture,
    surface_image: HtmlImageElement,
    regional_surface_image: HtmlImageElement,
    elevation_image: HtmlImageElement,
    surface_image_upload_attempted: Cell<bool>,
    regional_surface_image_upload_attempted: Cell<bool>,
    elevation_image_upload_attempted: Cell<bool>,
    orbital_surface_ready: Cell<bool>,
    regional_surface_ready: Cell<bool>,
    elevation_ready: Cell<bool>,
    speed_readout: Element,
    uniforms: Uniforms,
    clock: Rc<Cell<SimulationClock>>,
}

struct Uniforms {
    resolution: WebGlUniformLocation,
    camera_forward: WebGlUniformLocation,
    camera_right: WebGlUniformLocation,
    camera_up: WebGlUniformLocation,
    earth_center_m: WebGlUniformLocation,
    moon_center_m: WebGlUniformLocation,
    sun_center_m: WebGlUniformLocation,
    earth_equatorial_radius_m: WebGlUniformLocation,
    earth_polar_radius_m: WebGlUniformLocation,
    atmosphere_top_m: WebGlUniformLocation,
    moon_radius_m: WebGlUniformLocation,
    sun_radius_m: WebGlUniformLocation,
    earth_rotation: WebGlUniformLocation,
    camera_altitude_m: WebGlUniformLocation,
    time: WebGlUniformLocation,
    surface: WebGlUniformLocation,
    land_mask: WebGlUniformLocation,
    elevation: WebGlUniformLocation,
    elevation_ready: WebGlUniformLocation,
}

impl PlanetRenderer {
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("WebGL 2 is unavailable."))?
            .dyn_into::<Gl>()?;
        let vertex = compile_shader(&gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?;
        let fragment_source = crate::planet_shader::fragment_source().map_err(JsValue::from_str)?;
        let fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, &fragment_source)?;
        let program = link_program(&gl, &vertex, &fragment)?;
        gl.use_program(Some(&program));

        let vertex_array = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("Could not create the planet vertex array."))?;
        gl.bind_vertex_array(Some(&vertex_array));
        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("Could not create the planet screen buffer."))?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        let vertices = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0];
        unsafe {
            let view = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &view, Gl::STATIC_DRAW);
        }
        let position = gl.get_attrib_location(&program, "a_position");
        if position < 0 {
            return Err(JsValue::from_str(
                "The planetary screen shader attribute is unavailable.",
            ));
        }
        gl.enable_vertex_attrib_array(position as u32);
        gl.vertex_attrib_pointer_with_i32(position as u32, 2, Gl::FLOAT, false, 0, 0);

        let global_surface_texture = texture::create(&gl)?;
        let orbital_surface_texture = texture::create(&gl)?;
        let regional_surface_texture = texture::create(&gl)?;
        let land_mask_texture = texture::create(&gl)?;
        let elevation_texture = texture::create(&gl)?;
        let surface_image = create_data_image("image/webp", EARTH_SURFACE_512_WEBP_B64)?;
        let regional_surface_image = create_asset_image(EARTH_REGIONAL_SURFACE_PATH)?;
        let elevation_image = create_data_image("image/png", EARTH_ELEVATION_512_PNG_B64)?;
        let uniforms = Uniforms {
            resolution: required_uniform(&gl, &program, "u_resolution")?,
            camera_forward: required_uniform(&gl, &program, "u_camera_forward")?,
            camera_right: required_uniform(&gl, &program, "u_camera_right")?,
            camera_up: required_uniform(&gl, &program, "u_camera_up")?,
            earth_center_m: required_uniform(&gl, &program, "u_earth_center_m")?,
            moon_center_m: required_uniform(&gl, &program, "u_moon_center_m")?,
            sun_center_m: required_uniform(&gl, &program, "u_sun_center_m")?,
            earth_equatorial_radius_m: required_uniform(
                &gl,
                &program,
                "u_earth_equatorial_radius_m",
            )?,
            earth_polar_radius_m: required_uniform(&gl, &program, "u_earth_polar_radius_m")?,
            atmosphere_top_m: required_uniform(&gl, &program, "u_atmosphere_top_m")?,
            moon_radius_m: required_uniform(&gl, &program, "u_moon_radius_m")?,
            sun_radius_m: required_uniform(&gl, &program, "u_sun_radius_m")?,
            earth_rotation: required_uniform(&gl, &program, "u_earth_rotation")?,
            camera_altitude_m: required_uniform(&gl, &program, "u_camera_altitude_m")?,
            time: required_uniform(&gl, &program, "u_time")?,
            surface: required_uniform(&gl, &program, "u_surface")?,
            land_mask: required_uniform(&gl, &program, "u_land_mask")?,
            elevation: required_uniform(&gl, &program, "u_elevation")?,
            elevation_ready: required_uniform(&gl, &program, "u_elevation_ready")?,
        };
        gl.uniform1i(Some(&uniforms.surface), 0);
        gl.uniform1i(Some(&uniforms.land_mask), 1);
        gl.uniform1i(Some(&uniforms.elevation), 2);

        gl.disable(Gl::DEPTH_TEST);
        gl.disable(Gl::BLEND);
        gl.disable(Gl::CULL_FACE);

        let seconds_since_j2000 = js_sys::Date::now() / 1_000.0 - UNIX_SECONDS_AT_J2000;
        let clock = Rc::new(Cell::new(SimulationClock::new(seconds_since_j2000)));
        let speed_readout = attach_speed_readout(canvas)?;
        attach_time_control(canvas, Rc::clone(&clock))?;

        Ok(Self {
            canvas: canvas.clone(),
            gl,
            program,
            _buffer: buffer,
            _vertex_array: vertex_array,
            global_surface_texture,
            orbital_surface_texture,
            regional_surface_texture,
            land_mask_texture,
            elevation_texture,
            surface_image,
            regional_surface_image,
            elevation_image,
            surface_image_upload_attempted: Cell::new(false),
            regional_surface_image_upload_attempted: Cell::new(false),
            elevation_image_upload_attempted: Cell::new(false),
            orbital_surface_ready: Cell::new(false),
            regional_surface_ready: Cell::new(false),
            elevation_ready: Cell::new(false),
            speed_readout,
            uniforms,
            clock,
        })
    }

    pub(super) fn cycle_time_scale(&self) {
        let mut state = self.clock.get();
        let scale = state.cycle_scale();
        self.clock.set(state);
        if let Some(installation) = self.canvas.parent_element() {
            if let Ok(Some(button)) = installation.query_selector(".time-scale-button") {
                let _ = update_time_button(&button, scale);
            }
        }
    }

    pub(super) fn time_scale(&self) -> f64 {
        self.clock.get().scale()
    }

    pub(super) fn render(&self, visitor: CameraState, now: f64) {
        self.try_upload_orbital_surface();
        self.try_upload_regional_surface();
        self.try_upload_elevation();
        let (width, height) = resize_canvas(&self.canvas);
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
        self.gl.use_program(Some(&self.program));

        let speed_label = format_camera_speed_mps(visitor.camera_speed_mps);
        if self.speed_readout.text_content().as_deref() != Some(speed_label.as_str()) {
            self.speed_readout.set_text_content(Some(&speed_label));
        }

        let camera_altitude_m = visitor.radial_altitude_above_earth_m().max(0.0);
        let surface_tier = select_surface_tier(
            camera_altitude_m,
            self.orbital_surface_ready.get(),
            self.regional_surface_ready.get(),
        );
        let surface_texture = match surface_tier {
            SurfaceTier::Global => &self.global_surface_texture,
            SurfaceTier::Orbital => &self.orbital_surface_texture,
            SurfaceTier::Regional => &self.regional_surface_texture,
        };
        self.gl.active_texture(Gl::TEXTURE0);
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(surface_texture));
        self.gl.active_texture(Gl::TEXTURE1);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.land_mask_texture));
        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.elevation_texture));

        let mut clock = self.clock.get();
        let seconds_since_j2000 = clock.advance(now);
        let animation_seconds = clock.animation_seconds();
        self.clock.set(clock);

        let celestial = CelestialFrame::at_seconds_since_j2000(seconds_since_j2000);
        let relative = celestial.relative_to(visitor.position_m);
        let basis = visitor.camera_basis();

        self.gl
            .uniform2f(Some(&self.uniforms.resolution), width as f32, height as f32);
        set_vec3(&self.gl, &self.uniforms.camera_forward, basis.forward);
        set_vec3(&self.gl, &self.uniforms.camera_right, basis.right);
        set_vec3(&self.gl, &self.uniforms.camera_up, basis.up);
        set_vec3(
            &self.gl,
            &self.uniforms.earth_center_m,
            relative.earth_center_m,
        );
        set_vec3(
            &self.gl,
            &self.uniforms.moon_center_m,
            relative.moon_center_m,
        );
        set_vec3(&self.gl, &self.uniforms.sun_center_m, relative.sun_center_m);
        self.gl.uniform1f(
            Some(&self.uniforms.earth_equatorial_radius_m),
            EARTH_EQUATORIAL_RADIUS_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.earth_polar_radius_m),
            EARTH_POLAR_RADIUS_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.atmosphere_top_m),
            EARTH_ATMOSPHERE_TOP_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.moon_radius_m),
            MOON_MEAN_RADIUS_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.sun_radius_m),
            SUN_NOMINAL_RADIUS_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.earth_rotation),
            celestial.earth_rotation_radians as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.camera_altitude_m),
            camera_altitude_m as f32,
        );
        self.gl
            .uniform1f(Some(&self.uniforms.time), animation_seconds as f32);
        self.gl.uniform1f(
            Some(&self.uniforms.elevation_ready),
            if self.elevation_ready.get() { 1.0 } else { 0.0 },
        );
        self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);
    }

    fn try_upload_orbital_surface(&self) {
        if self.surface_image_upload_attempted.get()
            || !self.surface_image.complete()
            || self.surface_image.natural_width() == 0
        {
            return;
        }
        self.surface_image_upload_attempted.set(true);
        self.gl.active_texture(Gl::TEXTURE0);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.orbital_surface_texture));
        match upload_image_to_bound_texture(&self.gl, &self.surface_image) {
            Ok(()) => self.orbital_surface_ready.set(true),
            Err(error) => web_sys::console::warn_1(&error),
        }
    }

    fn try_upload_regional_surface(&self) {
        if self.regional_surface_image_upload_attempted.get()
            || !self.regional_surface_image.complete()
            || self.regional_surface_image.natural_width() == 0
        {
            return;
        }
        self.regional_surface_image_upload_attempted.set(true);
        self.gl.active_texture(Gl::TEXTURE0);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.regional_surface_texture));
        match upload_image_to_bound_texture(&self.gl, &self.regional_surface_image) {
            Ok(()) => self.regional_surface_ready.set(true),
            Err(error) => web_sys::console::warn_1(&error),
        }
    }

    fn try_upload_elevation(&self) {
        if self.elevation_image_upload_attempted.get()
            || !self.elevation_image.complete()
            || self.elevation_image.natural_width() == 0
        {
            return;
        }
        self.elevation_image_upload_attempted.set(true);
        if self.elevation_image.natural_width() <= 1 || self.elevation_image.natural_height() <= 1 {
            return;
        }
        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.elevation_texture));
        match upload_image_to_bound_texture(&self.gl, &self.elevation_image) {
            Ok(()) => self.elevation_ready.set(true),
            Err(error) => web_sys::console::warn_1(&error),
        }
    }
}

fn upload_image_to_bound_texture(gl: &Gl, image: &HtmlImageElement) -> Result<(), JsValue> {
    gl.tex_image_2d_with_u32_and_u32_and_html_image_element(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA as i32,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        image,
    )?;
    gl.generate_mipmap(Gl::TEXTURE_2D);
    Ok(())
}

fn create_data_image(media_type: &str, encoded: &str) -> Result<HtmlImageElement, JsValue> {
    let image = HtmlImageElement::new()?;
    image.set_src(&format!("data:{media_type};base64,{}", encoded.trim()));
    Ok(image)
}

fn create_asset_image(source: &str) -> Result<HtmlImageElement, JsValue> {
    let image = HtmlImageElement::new()?;
    image.set_src(source);
    Ok(image)
}

fn attach_speed_readout(canvas: &HtmlCanvasElement) -> Result<Element, JsValue> {
    let document = canvas
        .owner_document()
        .ok_or_else(|| JsValue::from_str("The gallery document is unavailable."))?;
    let toolbar_actions = installation_toolbar_actions(canvas)?;
    let readout = document.create_element("span")?;
    readout.set_attribute("class", "camera-speed-readout")?;
    readout.set_attribute("aria-label", "Current virtual camera speed")?;
    readout.set_text_content(Some(&format_camera_speed_mps(
        CameraState::default().camera_speed_mps,
    )));
    toolbar_actions.append_child(&readout)?;
    Ok(readout)
}

fn attach_time_control(
    canvas: &HtmlCanvasElement,
    clock: Rc<Cell<SimulationClock>>,
) -> Result<(), JsValue> {
    let document = canvas
        .owner_document()
        .ok_or_else(|| JsValue::from_str("The gallery document is unavailable."))?;
    let toolbar_actions = installation_toolbar_actions(canvas)?;
    let button = document.create_element("button")?;
    button.set_attribute("type", "button")?;
    button.set_attribute("class", "time-scale-button")?;
    update_time_button(&button, clock.get().scale())?;
    toolbar_actions.append_child(&button)?;

    let button_for_click = button.clone();
    let on_click = Closure::<dyn FnMut()>::new(move || {
        let mut state = clock.get();
        let scale = state.cycle_scale();
        clock.set(state);
        let _ = update_time_button(&button_for_click, scale);
    });
    button.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
    on_click.forget();
    Ok(())
}

fn installation_toolbar_actions(canvas: &HtmlCanvasElement) -> Result<Element, JsValue> {
    canvas
        .parent_element()
        .ok_or_else(|| JsValue::from_str("The Earth installation is unavailable."))?
        .query_selector(".toolbar-actions")?
        .ok_or_else(|| JsValue::from_str("The installation toolbar is unavailable."))
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

fn set_vec3(gl: &Gl, uniform: &WebGlUniformLocation, vector: Vec3d) {
    gl.uniform3f(
        Some(uniform),
        vector.x as f32,
        vector.y as f32,
        vector.z as f32,
    );
}

fn resize_canvas(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let width = canvas.client_width().max(1) as u32;
    let height = canvas.client_height().max(1) as u32;
    let scale = window()
        .map(|window| {
            window
                .device_pixel_ratio()
                .clamp(1.0, MAX_DEVICE_PIXEL_RATIO)
        })
        .unwrap_or(1.0);
    let pixel_width = (width as f64 * scale).round() as u32;
    let pixel_height = (height as f64 * scale).round() as u32;
    if canvas.width() != pixel_width || canvas.height() != pixel_height {
        canvas.set_width(pixel_width);
        canvas.set_height(pixel_height);
    }
    (pixel_width, pixel_height)
}

fn required_uniform(
    gl: &Gl,
    program: &WebGlProgram,
    name: &str,
) -> Result<WebGlUniformLocation, JsValue> {
    gl.get_uniform_location(program, name)
        .ok_or_else(|| JsValue::from_str("A required planetary shader uniform is unavailable."))
}

fn compile_shader(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("Could not create a planetary shader."))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        web_sys::console::error_1(
            &gl.get_shader_info_log(&shader)
                .unwrap_or_else(|| "Planetary shader compilation failed.".to_owned())
                .into(),
        );
        Err(JsValue::from_str(
            "The planetary renderer could not compile on this graphics device.",
        ))
    }
}

fn link_program(
    gl: &Gl,
    vertex: &WebGlShader,
    fragment: &WebGlShader,
) -> Result<WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("Could not create a planetary shader program."))?;
    gl.attach_shader(&program, vertex);
    gl.attach_shader(&program, fragment);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        web_sys::console::error_1(
            &gl.get_program_info_log(&program)
                .unwrap_or_else(|| "Planetary shader linking failed.".to_owned())
                .into(),
        );
        Err(JsValue::from_str(
            "The planetary renderer could not link on this graphics device.",
        ))
    }
}

const VERTEX_SHADER: &str = include_str!("shaders/planet.vert.glsl");
