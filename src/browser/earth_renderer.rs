//! Camera-relative, physically scaled WebGL 2 renderer for A World in Light.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::{
    camera_api::CameraState,
    planet::{
        CelestialFrame, EARTH_ATMOSPHERE_TOP_M, EARTH_EQUATORIAL_RADIUS_M, EARTH_POLAR_RADIUS_M,
        MOON_MEAN_RADIUS_M, SUN_NOMINAL_RADIUS_M,
    },
};

use super::dom::window;

const UNIX_SECONDS_AT_J2000: f64 = 946_728_000.0;
const MAX_DEVICE_PIXEL_RATIO: f64 = 1.5;

pub(super) struct PlanetRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    _buffer: WebGlBuffer,
    _vertex_array: WebGlVertexArrayObject,
    uniforms: Uniforms,
}

struct Uniforms {
    resolution: WebGlUniformLocation,
    camera_forward: WebGlUniformLocation,
    camera_right: WebGlUniformLocation,
    camera_up: WebGlUniformLocation,
    earth_center: WebGlUniformLocation,
    moon_center: WebGlUniformLocation,
    sun_center: WebGlUniformLocation,
    earth_polar_radius: WebGlUniformLocation,
    atmosphere_radius: WebGlUniformLocation,
    moon_radius: WebGlUniformLocation,
    sun_radius: WebGlUniformLocation,
    earth_rotation: WebGlUniformLocation,
    time: WebGlUniformLocation,
}

impl PlanetRenderer {
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("WebGL 2 is unavailable."))?
            .dyn_into::<Gl>()?;
        let vertex = compile_shader(&gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?;
        let fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, FRAGMENT_SHADER)?;
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

        let uniforms = Uniforms {
            resolution: required_uniform(&gl, &program, "u_resolution")?,
            camera_forward: required_uniform(&gl, &program, "u_camera_forward")?,
            camera_right: required_uniform(&gl, &program, "u_camera_right")?,
            camera_up: required_uniform(&gl, &program, "u_camera_up")?,
            earth_center: required_uniform(&gl, &program, "u_earth_center")?,
            moon_center: required_uniform(&gl, &program, "u_moon_center")?,
            sun_center: required_uniform(&gl, &program, "u_sun_center")?,
            earth_polar_radius: required_uniform(&gl, &program, "u_earth_polar_radius")?,
            atmosphere_radius: required_uniform(&gl, &program, "u_atmosphere_radius")?,
            moon_radius: required_uniform(&gl, &program, "u_moon_radius")?,
            sun_radius: required_uniform(&gl, &program, "u_sun_radius")?,
            earth_rotation: required_uniform(&gl, &program, "u_earth_rotation")?,
            time: required_uniform(&gl, &program, "u_time")?,
        };

        gl.disable(Gl::DEPTH_TEST);
        gl.disable(Gl::BLEND);
        gl.disable(Gl::CULL_FACE);

        Ok(Self {
            canvas: canvas.clone(),
            gl,
            program,
            _buffer: buffer,
            _vertex_array: vertex_array,
            uniforms,
        })
    }

    pub(super) fn render(&self, visitor: CameraState, now: f64) {
        let (width, height) = resize_canvas(&self.canvas);
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
        self.gl.use_program(Some(&self.program));

        let seconds_since_j2000 = js_sys::Date::now() / 1_000.0 - UNIX_SECONDS_AT_J2000;
        let celestial = CelestialFrame::at_seconds_since_j2000(seconds_since_j2000);
        let relative = celestial.relative_to(visitor.position_m);
        let basis = visitor.camera_basis();
        let earth = relative.earth_center_m.to_earth_radii_f32();
        let moon = relative.moon_center_m.to_earth_radii_f32();
        let sun = relative.sun_center_m.to_earth_radii_f32();

        self.gl
            .uniform2f(Some(&self.uniforms.resolution), width as f32, height as f32);
        set_vec3(&self.gl, &self.uniforms.camera_forward, basis.forward);
        set_vec3(&self.gl, &self.uniforms.camera_right, basis.right);
        set_vec3(&self.gl, &self.uniforms.camera_up, basis.up);
        self.gl.uniform3f(
            Some(&self.uniforms.earth_center),
            earth[0],
            earth[1],
            earth[2],
        );
        self.gl
            .uniform3f(Some(&self.uniforms.moon_center), moon[0], moon[1], moon[2]);
        self.gl
            .uniform3f(Some(&self.uniforms.sun_center), sun[0], sun[1], sun[2]);
        self.gl.uniform1f(
            Some(&self.uniforms.earth_polar_radius),
            (EARTH_POLAR_RADIUS_M / EARTH_EQUATORIAL_RADIUS_M) as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.atmosphere_radius),
            (1.0 + EARTH_ATMOSPHERE_TOP_M / EARTH_EQUATORIAL_RADIUS_M) as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.moon_radius),
            (MOON_MEAN_RADIUS_M / EARTH_EQUATORIAL_RADIUS_M) as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.sun_radius),
            (SUN_NOMINAL_RADIUS_M / EARTH_EQUATORIAL_RADIUS_M) as f32,
        );
        self.gl.uniform1f(
            Some(&self.uniforms.earth_rotation),
            celestial.earth_rotation_radians as f32,
        );
        self.gl
            .uniform1f(Some(&self.uniforms.time), (now / 1_000.0) as f32);
        self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);
    }
}

fn set_vec3(gl: &Gl, uniform: &WebGlUniformLocation, vector: crate::planet::Vec3d) {
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
        .ok_or_else(|| JsValue::from_str("Could not create the planetary shader program."))?;
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
const FRAGMENT_SHADER: &str = include_str!("shaders/planet.frag.glsl");
