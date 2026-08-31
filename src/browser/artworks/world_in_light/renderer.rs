//! Camera-relative, physically scaled WebGL 2 renderer for A World in Light.

#[path = "texture.rs"]
mod texture;

use std::{cell::Cell, mem::size_of};

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram,
    WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

use artwork_world_in_light::{
    celestial_bounds::{merge_overlapping_rects, sphere_scissor_rect},
    earth_terrain::{TERRAIN_HEIGHT_BYTES, TERRAIN_HEIGHT_HEIGHT, TERRAIN_HEIGHT_WIDTH},
    exhibition_camera::SpaceflightState as CameraState,
    planet::{
        CelestialFrame, EARTH_ATMOSPHERE_TOP_M, EARTH_EQUATORIAL_RADIUS_M, EARTH_POLAR_RADIUS_M,
        MOON_MEAN_RADIUS_M, SUN_NOMINAL_RADIUS_M, Vec3d,
    },
    render_quality::AdaptiveFramebufferQuality,
    star_catalogue::{
        STAR_COLOR_OFFSET_FLOATS, STAR_COUNT, STAR_DIRECTION_OFFSET_FLOATS,
        STAR_SIZE_OFFSET_FLOATS, STAR_VERTEX_STRIDE_FLOATS, star_catalogue_vertices,
    },
};

use crate::browser::dom::window;

const EARTH_SURFACE_PATH: &str = "./assets/earth/earth-surface-4096.webp";
const EARTH_MATERIAL_PATH: &str = "./assets/earth/earth-material-2048.png";
const EARTH_WEATHER_PATH: &str = "./assets/earth/earth-weather-1024.webp";
const STARFIELD_PATH: &str = "./assets/earth/stars-1024.png";
const MOON_ALBEDO_PATH: &str = "./assets/earth/moon-albedo-2048.webp";
const EARTH_NIGHT_PATH: &str = "./assets/earth/earth-night-2048.webp";
const EARTH_SURFACE_WIDTH: u32 = 4096;
const EARTH_SURFACE_HEIGHT: u32 = 2048;
const EARTH_MATERIAL_WIDTH: u32 = 2048;
const EARTH_MATERIAL_HEIGHT: u32 = 1024;
const EARTH_WEATHER_WIDTH: u32 = 1024;
const EARTH_WEATHER_HEIGHT: u32 = 512;
const MOON_ALBEDO_WIDTH: u32 = 2048;
const MOON_ALBEDO_HEIGHT: u32 = 1024;
const EARTH_NIGHT_WIDTH: u32 = 2048;
const EARTH_NIGHT_HEIGHT: u32 = 1024;

pub(super) struct WorldInLightRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    background_program: WebGlProgram,
    star_program: WebGlProgram,
    _screen_buffer: WebGlBuffer,
    screen_vertex_array: WebGlVertexArrayObject,
    _star_buffer: WebGlBuffer,
    star_vertex_array: WebGlVertexArrayObject,
    fallback_texture: WebGlTexture,
    surface_texture: WebGlTexture,
    material_texture: WebGlTexture,
    weather_texture: WebGlTexture,
    starfield_texture: WebGlTexture,
    moon_albedo_texture: WebGlTexture,
    night_texture: WebGlTexture,
    terrain_height_texture: WebGlTexture,
    surface_image: HtmlImageElement,
    material_image: HtmlImageElement,
    weather_image: HtmlImageElement,
    starfield_image: HtmlImageElement,
    moon_albedo_image: HtmlImageElement,
    night_image: HtmlImageElement,
    surface_image_upload_attempted: Cell<bool>,
    material_image_upload_attempted: Cell<bool>,
    weather_image_upload_attempted: Cell<bool>,
    starfield_image_upload_attempted: Cell<bool>,
    moon_albedo_image_upload_attempted: Cell<bool>,
    night_image_upload_attempted: Cell<bool>,
    surface_ready: Cell<bool>,
    material_ready: Cell<bool>,
    weather_ready: Cell<bool>,
    starfield_ready: Cell<bool>,
    moon_albedo_ready: Cell<bool>,
    night_ready: Cell<bool>,
    uniforms: Uniforms,
    background_uniforms: BackgroundUniforms,
    star_uniforms: StarUniforms,
    framebuffer_quality: Cell<AdaptiveFramebufferQuality>,
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
    surface_clearance_m: WebGlUniformLocation,
    time: WebGlUniformLocation,
    surface: WebGlUniformLocation,
    material: WebGlUniformLocation,
    weather: WebGlUniformLocation,
    starfield: WebGlUniformLocation,
    moon_albedo: WebGlUniformLocation,
    night: WebGlUniformLocation,
    terrain_height: WebGlUniformLocation,
    material_ready: WebGlUniformLocation,
    weather_ready: WebGlUniformLocation,
    starfield_ready: WebGlUniformLocation,
    moon_albedo_ready: WebGlUniformLocation,
    night_ready: WebGlUniformLocation,
}

struct BackgroundUniforms {
    resolution: WebGlUniformLocation,
    camera_forward: WebGlUniformLocation,
    camera_right: WebGlUniformLocation,
    camera_up: WebGlUniformLocation,
    sun_center_m: WebGlUniformLocation,
    sun_radius_m: WebGlUniformLocation,
    starfield: WebGlUniformLocation,
    starfield_ready: WebGlUniformLocation,
}

struct StarUniforms {
    resolution: WebGlUniformLocation,
    camera_forward: WebGlUniformLocation,
    camera_right: WebGlUniformLocation,
    camera_up: WebGlUniformLocation,
    pixel_scale: WebGlUniformLocation,
}

impl WorldInLightRenderer {
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("WebGL 2 is unavailable."))?
            .dyn_into::<Gl>()?;
        let vertex = compile_shader(&gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?;
        let fragment_source =
            artwork_world_in_light::planet_shader::fragment_source().map_err(JsValue::from_str)?;
        let fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, &fragment_source)?;
        let program = link_program(&gl, &vertex, &fragment)?;
        let background_fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, BACKGROUND_SHADER)?;
        let background_program = link_program(&gl, &vertex, &background_fragment)?;
        let star_vertex = compile_shader(&gl, Gl::VERTEX_SHADER, STAR_VERTEX_SHADER)?;
        let star_fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, STAR_FRAGMENT_SHADER)?;
        let star_program = link_program(&gl, &star_vertex, &star_fragment)?;

        let screen_vertex_array = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("Could not create the planet vertex array."))?;
        gl.bind_vertex_array(Some(&screen_vertex_array));
        let screen_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("Could not create the planet screen buffer."))?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&screen_buffer));
        let vertices = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0];
        unsafe {
            let view = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &view, Gl::STATIC_DRAW);
        }
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 2, Gl::FLOAT, false, 0, 0);

        let star_vertex_array = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("Could not create the star vertex array."))?;
        gl.bind_vertex_array(Some(&star_vertex_array));
        let star_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("Could not create the star catalogue buffer."))?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&star_buffer));
        let star_vertices = star_catalogue_vertices();
        unsafe {
            let view = js_sys::Float32Array::view(&star_vertices);
            gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &view, Gl::STATIC_DRAW);
        }
        let stride = (STAR_VERTEX_STRIDE_FLOATS * size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(
            0,
            3,
            Gl::FLOAT,
            false,
            stride,
            (STAR_DIRECTION_OFFSET_FLOATS * size_of::<f32>()) as i32,
        );
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(
            1,
            3,
            Gl::FLOAT,
            false,
            stride,
            (STAR_COLOR_OFFSET_FLOATS * size_of::<f32>()) as i32,
        );
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_with_i32(
            2,
            1,
            Gl::FLOAT,
            false,
            stride,
            (STAR_SIZE_OFFSET_FLOATS * size_of::<f32>()) as i32,
        );

        let fallback_texture = texture::create(&gl)?;
        let surface_texture = texture::create(&gl)?;
        let material_texture = texture::create(&gl)?;
        let weather_texture = texture::create(&gl)?;
        let starfield_texture = texture::create(&gl)?;
        let moon_albedo_texture = texture::create(&gl)?;
        let night_texture = texture::create(&gl)?;
        let terrain_height_texture = texture::create_height_field(
            &gl,
            TERRAIN_HEIGHT_WIDTH as i32,
            TERRAIN_HEIGHT_HEIGHT as i32,
            TERRAIN_HEIGHT_BYTES,
        )?;
        let surface_image = create_asset_image(EARTH_SURFACE_PATH)?;
        let material_image = create_asset_image(EARTH_MATERIAL_PATH)?;
        let weather_image = create_asset_image(EARTH_WEATHER_PATH)?;
        let starfield_image = create_asset_image(STARFIELD_PATH)?;
        let moon_albedo_image = create_asset_image(MOON_ALBEDO_PATH)?;
        let night_image = create_asset_image(EARTH_NIGHT_PATH)?;
        gl.use_program(Some(&program));
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
            surface_clearance_m: required_uniform(&gl, &program, "u_surface_clearance_m")?,
            time: required_uniform(&gl, &program, "u_time")?,
            surface: required_uniform(&gl, &program, "u_surface")?,
            material: required_uniform(&gl, &program, "u_material")?,
            weather: required_uniform(&gl, &program, "u_weather")?,
            starfield: required_uniform(&gl, &program, "u_starfield")?,
            moon_albedo: required_uniform(&gl, &program, "u_moon_albedo")?,
            night: required_uniform(&gl, &program, "u_night")?,
            terrain_height: required_uniform(&gl, &program, "u_terrain_height")?,
            material_ready: required_uniform(&gl, &program, "u_material_ready")?,
            weather_ready: required_uniform(&gl, &program, "u_weather_ready")?,
            starfield_ready: required_uniform(&gl, &program, "u_starfield_ready")?,
            moon_albedo_ready: required_uniform(&gl, &program, "u_moon_albedo_ready")?,
            night_ready: required_uniform(&gl, &program, "u_night_ready")?,
        };
        gl.uniform1i(Some(&uniforms.surface), 0);
        gl.uniform1i(Some(&uniforms.material), 1);
        gl.uniform1i(Some(&uniforms.weather), 2);
        gl.uniform1i(Some(&uniforms.starfield), 3);
        gl.uniform1i(Some(&uniforms.moon_albedo), 4);
        gl.uniform1i(Some(&uniforms.night), 5);
        gl.uniform1i(Some(&uniforms.terrain_height), 6);
        let background_uniforms = BackgroundUniforms {
            resolution: required_uniform(&gl, &background_program, "u_resolution")?,
            camera_forward: required_uniform(&gl, &background_program, "u_camera_forward")?,
            camera_right: required_uniform(&gl, &background_program, "u_camera_right")?,
            camera_up: required_uniform(&gl, &background_program, "u_camera_up")?,
            sun_center_m: required_uniform(&gl, &background_program, "u_sun_center_m")?,
            sun_radius_m: required_uniform(&gl, &background_program, "u_sun_radius_m")?,
            starfield: required_uniform(&gl, &background_program, "u_starfield")?,
            starfield_ready: required_uniform(&gl, &background_program, "u_starfield_ready")?,
        };
        gl.use_program(Some(&background_program));
        gl.uniform1i(Some(&background_uniforms.starfield), 3);
        let star_uniforms = StarUniforms {
            resolution: required_uniform(&gl, &star_program, "u_resolution")?,
            camera_forward: required_uniform(&gl, &star_program, "u_camera_forward")?,
            camera_right: required_uniform(&gl, &star_program, "u_camera_right")?,
            camera_up: required_uniform(&gl, &star_program, "u_camera_up")?,
            pixel_scale: required_uniform(&gl, &star_program, "u_pixel_scale")?,
        };

        gl.disable(Gl::DEPTH_TEST);
        gl.disable(Gl::BLEND);
        gl.disable(Gl::CULL_FACE);

        Ok(Self {
            canvas: canvas.clone(),
            gl,
            program,
            background_program,
            star_program,
            _screen_buffer: screen_buffer,
            screen_vertex_array,
            _star_buffer: star_buffer,
            star_vertex_array,
            fallback_texture,
            surface_texture,
            material_texture,
            weather_texture,
            starfield_texture,
            moon_albedo_texture,
            night_texture,
            terrain_height_texture,
            surface_image,
            material_image,
            weather_image,
            starfield_image,
            moon_albedo_image,
            night_image,
            surface_image_upload_attempted: Cell::new(false),
            material_image_upload_attempted: Cell::new(false),
            weather_image_upload_attempted: Cell::new(false),
            starfield_image_upload_attempted: Cell::new(false),
            moon_albedo_image_upload_attempted: Cell::new(false),
            night_image_upload_attempted: Cell::new(false),
            surface_ready: Cell::new(false),
            material_ready: Cell::new(false),
            weather_ready: Cell::new(false),
            starfield_ready: Cell::new(false),
            moon_albedo_ready: Cell::new(false),
            night_ready: Cell::new(false),
            uniforms,
            background_uniforms,
            star_uniforms,
            framebuffer_quality: Cell::new(AdaptiveFramebufferQuality::new()),
        })
    }

    pub(super) fn render(
        &self,
        visitor: &CameraState,
        celestial: CelestialFrame,
        animation_seconds: f64,
        now: f64,
    ) {
        self.try_upload_assets();
        let mut framebuffer_quality = self.framebuffer_quality.get();
        framebuffer_quality.observe_frame_timestamp_ms(now);
        let (width, height) = resize_canvas(&self.canvas, framebuffer_quality);
        self.framebuffer_quality.set(framebuffer_quality);
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.disable(Gl::SCISSOR_TEST);
        self.gl.disable(Gl::BLEND);
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);

        let surface_texture = if self.surface_ready.get() {
            &self.surface_texture
        } else {
            &self.fallback_texture
        };
        let material_texture = if self.material_ready.get() {
            &self.material_texture
        } else {
            &self.fallback_texture
        };
        let weather_texture = if self.weather_ready.get() {
            &self.weather_texture
        } else {
            &self.fallback_texture
        };
        let starfield_texture = if self.starfield_ready.get() {
            &self.starfield_texture
        } else {
            &self.fallback_texture
        };
        let moon_albedo_texture = if self.moon_albedo_ready.get() {
            &self.moon_albedo_texture
        } else {
            &self.fallback_texture
        };
        let night_texture = if self.night_ready.get() {
            &self.night_texture
        } else {
            &self.fallback_texture
        };
        self.gl.active_texture(Gl::TEXTURE0);
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(surface_texture));
        self.gl.active_texture(Gl::TEXTURE1);
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(material_texture));
        self.gl.active_texture(Gl::TEXTURE2);
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(weather_texture));
        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(starfield_texture));
        self.gl.active_texture(Gl::TEXTURE4);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(moon_albedo_texture));
        self.gl.active_texture(Gl::TEXTURE5);
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(night_texture));
        self.gl.active_texture(Gl::TEXTURE6);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.terrain_height_texture));

        let camera_altitude_m = visitor.radial_altitude_above_earth_m().max(0.0);
        let surface_clearance_m = celestial.earth_surface_clearance_m(visitor.position_m);
        let relative = celestial.relative_to(visitor.position_m);
        let basis = visitor.camera_basis();

        // Empty space is deliberately cheap: one full-screen haze/corona pass and a
        // fixed point catalogue. The expensive atmospheric integration is only run
        // inside conservative screen-space bounds around visible celestial bodies.
        self.gl.use_program(Some(&self.background_program));
        self.gl.bind_vertex_array(Some(&self.screen_vertex_array));
        self.gl.uniform2f(
            Some(&self.background_uniforms.resolution),
            width as f32,
            height as f32,
        );
        set_vec3(
            &self.gl,
            &self.background_uniforms.camera_forward,
            basis.forward,
        );
        set_vec3(
            &self.gl,
            &self.background_uniforms.camera_right,
            basis.right,
        );
        set_vec3(&self.gl, &self.background_uniforms.camera_up, basis.up);
        set_vec3(
            &self.gl,
            &self.background_uniforms.sun_center_m,
            relative.sun_center_m,
        );
        self.gl.uniform1f(
            Some(&self.background_uniforms.sun_radius_m),
            SUN_NOMINAL_RADIUS_M as f32,
        );
        self.gl.uniform1f(
            Some(&self.background_uniforms.starfield_ready),
            if self.starfield_ready.get() { 1.0 } else { 0.0 },
        );
        self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);

        self.gl.use_program(Some(&self.star_program));
        self.gl.bind_vertex_array(Some(&self.star_vertex_array));
        self.gl.uniform2f(
            Some(&self.star_uniforms.resolution),
            width as f32,
            height as f32,
        );
        set_vec3(&self.gl, &self.star_uniforms.camera_forward, basis.forward);
        set_vec3(&self.gl, &self.star_uniforms.camera_right, basis.right);
        set_vec3(&self.gl, &self.star_uniforms.camera_up, basis.up);
        let css_width = self.canvas.client_width().max(1) as f32;
        self.gl.uniform1f(
            Some(&self.star_uniforms.pixel_scale),
            width as f32 / css_width,
        );
        self.gl.enable(Gl::BLEND);
        self.gl.blend_func(Gl::ONE, Gl::ONE_MINUS_SRC_ALPHA);
        self.gl.draw_arrays(Gl::POINTS, 0, STAR_COUNT as i32);
        self.gl.disable(Gl::BLEND);

        self.gl.use_program(Some(&self.program));
        self.gl.bind_vertex_array(Some(&self.screen_vertex_array));
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
        self.gl.uniform1f(
            Some(&self.uniforms.surface_clearance_m),
            surface_clearance_m as f32,
        );
        self.gl
            .uniform1f(Some(&self.uniforms.time), animation_seconds as f32);
        self.gl.uniform1f(
            Some(&self.uniforms.material_ready),
            if self.material_ready.get() { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.uniforms.weather_ready),
            if self.weather_ready.get() { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.uniforms.starfield_ready),
            if self.starfield_ready.get() { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.uniforms.moon_albedo_ready),
            if self.moon_albedo_ready.get() {
                1.0
            } else {
                0.0
            },
        );
        self.gl.uniform1f(
            Some(&self.uniforms.night_ready),
            if self.night_ready.get() { 1.0 } else { 0.0 },
        );

        let rectangles = merge_overlapping_rects(
            [
                sphere_scissor_rect(
                    relative.earth_center_m,
                    EARTH_EQUATORIAL_RADIUS_M + EARTH_ATMOSPHERE_TOP_M,
                    basis,
                    width,
                    height,
                ),
                sphere_scissor_rect(
                    relative.moon_center_m,
                    MOON_MEAN_RADIUS_M * 1.02,
                    basis,
                    width,
                    height,
                ),
                sphere_scissor_rect(
                    relative.sun_center_m,
                    SUN_NOMINAL_RADIUS_M * 1.02,
                    basis,
                    width,
                    height,
                ),
            ]
            .into_iter()
            .flatten(),
        );
        self.gl.enable(Gl::SCISSOR_TEST);
        for rectangle in rectangles {
            self.gl
                .scissor(rectangle.x, rectangle.y, rectangle.width, rectangle.height);
            self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        }
        self.gl.disable(Gl::SCISSOR_TEST);
    }

    fn try_upload_assets(&self) {
        try_upload_asset(
            &self.gl,
            &self.surface_image,
            &self.surface_texture,
            &self.surface_image_upload_attempted,
            &self.surface_ready,
            [EARTH_SURFACE_WIDTH, EARTH_SURFACE_HEIGHT],
            "Earth surface",
        );
        try_upload_asset(
            &self.gl,
            &self.material_image,
            &self.material_texture,
            &self.material_image_upload_attempted,
            &self.material_ready,
            [EARTH_MATERIAL_WIDTH, EARTH_MATERIAL_HEIGHT],
            "Earth material",
        );
        try_upload_asset(
            &self.gl,
            &self.weather_image,
            &self.weather_texture,
            &self.weather_image_upload_attempted,
            &self.weather_ready,
            [EARTH_WEATHER_WIDTH, EARTH_WEATHER_HEIGHT],
            "Earth weather",
        );
        try_upload_asset(
            &self.gl,
            &self.starfield_image,
            &self.starfield_texture,
            &self.starfield_image_upload_attempted,
            &self.starfield_ready,
            [EARTH_WEATHER_WIDTH, EARTH_WEATHER_WIDTH],
            "Starfield",
        );
        try_upload_asset(
            &self.gl,
            &self.moon_albedo_image,
            &self.moon_albedo_texture,
            &self.moon_albedo_image_upload_attempted,
            &self.moon_albedo_ready,
            [MOON_ALBEDO_WIDTH, MOON_ALBEDO_HEIGHT],
            "Moon albedo",
        );
        try_upload_asset(
            &self.gl,
            &self.night_image,
            &self.night_texture,
            &self.night_image_upload_attempted,
            &self.night_ready,
            [EARTH_NIGHT_WIDTH, EARTH_NIGHT_HEIGHT],
            "Earth night lights",
        );
    }
}

impl Drop for WorldInLightRenderer {
    fn drop(&mut self) {
        self.gl.use_program(None);
        self.gl.bind_vertex_array(None);
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, None);
        self.gl.bind_texture(Gl::TEXTURE_2D, None);
        self.gl.delete_buffer(Some(&self._screen_buffer));
        self.gl.delete_buffer(Some(&self._star_buffer));
        self.gl.delete_vertex_array(Some(&self.screen_vertex_array));
        self.gl.delete_vertex_array(Some(&self.star_vertex_array));
        for texture in [
            &self.fallback_texture,
            &self.surface_texture,
            &self.material_texture,
            &self.weather_texture,
            &self.starfield_texture,
            &self.moon_albedo_texture,
            &self.night_texture,
            &self.terrain_height_texture,
        ] {
            self.gl.delete_texture(Some(texture));
        }
        self.gl.delete_program(Some(&self.program));
        self.gl.delete_program(Some(&self.background_program));
        self.gl.delete_program(Some(&self.star_program));
    }
}

fn try_upload_asset(
    gl: &Gl,
    image: &HtmlImageElement,
    texture: &WebGlTexture,
    attempted: &Cell<bool>,
    ready: &Cell<bool>,
    expected_size: [u32; 2],
    label: &str,
) {
    if attempted.get() || !image.complete() {
        return;
    }
    attempted.set(true);
    if image.natural_width() != expected_size[0] || image.natural_height() != expected_size[1] {
        web_sys::console::warn_1(
            &format!("{label} texture was unavailable or had unexpected dimensions.").into(),
        );
        return;
    }
    gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
    match upload_image_to_bound_texture(gl, image) {
        Ok(()) => ready.set(true),
        Err(error) => web_sys::console::warn_1(&error),
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

fn create_asset_image(source: &str) -> Result<HtmlImageElement, JsValue> {
    let image = HtmlImageElement::new()?;
    image.set_src(source);
    Ok(image)
}

fn set_vec3(gl: &Gl, uniform: &WebGlUniformLocation, vector: Vec3d) {
    gl.uniform3f(
        Some(uniform),
        vector.x as f32,
        vector.y as f32,
        vector.z as f32,
    );
}

fn resize_canvas(
    canvas: &HtmlCanvasElement,
    framebuffer_quality: AdaptiveFramebufferQuality,
) -> (u32, u32) {
    let width = canvas.client_width().max(1) as u32;
    let height = canvas.client_height().max(1) as u32;
    let device_pixel_ratio = window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0);
    let scale = framebuffer_quality.effective_framebuffer_scale(width, height, device_pixel_ratio);
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

const VERTEX_SHADER: &str =
    include_str!("../../../../crates/artwork-world-in-light/shaders/planet.vert.glsl");
const BACKGROUND_SHADER: &str =
    include_str!("../../../../crates/artwork-world-in-light/shaders/space_background.frag.glsl");
const STAR_VERTEX_SHADER: &str =
    include_str!("../../../../crates/artwork-world-in-light/shaders/star_points.vert.glsl");
const STAR_FRAGMENT_SHADER: &str =
    include_str!("../../../../crates/artwork-world-in-light/shaders/star_points.frag.glsl");
