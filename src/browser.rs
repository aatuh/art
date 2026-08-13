use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    Document, Element, Event, HtmlCanvasElement, WebGlBuffer, WebGlProgram,
    WebGlRenderingContext as Gl, WebGlShader, WebGlUniformLocation,
};

mod controls;
mod dom;
mod earth_renderer;
mod rendering;
mod storage;

use controls::{Session, attach_fps_controls, create_settings_panel};
use dom::{clear_children, document, element, text_element, window};
use storage::read_settings;

use crate::math::{Mat4, normalize};

use crate::{ARTWORKS, MVP_GALLERY_WORLD, fps::PlayerState, resolve_destination};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = document()?;
    let root = document
        .get_element_by_id("gallery")
        .ok_or_else(|| JsValue::from_str("The gallery root is unavailable."))?;
    render_catalogue(&document, &root)
}

fn render_catalogue(document: &Document, root: &Element) -> Result<(), JsValue> {
    clear_children(root)?;
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

    for artwork in ARTWORKS {
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
        let artwork_id = artwork.id;
        let on_click = Closure::<dyn FnMut()>::new(move || {
            let _ = render_installation(&document_for_click, &root_for_click, artwork_id);
        });
        card.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
        on_click.forget();
        list.append_child(&card)?;
    }

    content.append_child(&list)?;
    catalogue.append_child(&content)?;
    root.append_child(&catalogue)?;
    Ok(())
}

fn render_installation(
    document: &Document,
    root: &Element,
    artwork_id: &str,
) -> Result<(), JsValue> {
    let Some((artwork, destination)) = resolve_destination(artwork_id) else {
        return render_catalogue(document, root);
    };
    if destination.world != MVP_GALLERY_WORLD {
        return render_catalogue(document, root);
    }

    clear_children(root)?;
    let installation = element(document, "article", "installation fps-installation")?;
    installation.set_attribute("data-artwork", artwork.id)?;
    let canvas = element(document, "canvas", "fps-canvas")?.dyn_into::<HtmlCanvasElement>()?;
    canvas.set_attribute(
        "aria-label",
        if artwork.id == "orbiting-earth" {
            "A room-scale Earth-like planet with an orbiting moon"
        } else {
            "A white room containing a black cube"
        },
    )?;
    installation.append_child(&canvas)?;

    let observer = element(document, "section", "room-observer")?;
    observer.set_attribute("tabindex", "0")?;
    observer.set_attribute(
        "aria-label",
        "First-person room. Activate to use mouse look. Move with WASD, jump with Space, and crouch with Control.",
    )?;
    let crosshair = element(document, "span", "crosshair")?;
    crosshair.set_attribute("aria-hidden", "true")?;
    observer.append_child(&crosshair)?;
    installation.append_child(&observer)?;

    let toolbar = element(document, "header", "installation-toolbar")?;
    let back = element(document, "button", "back-button")?;
    back.set_attribute("type", "button")?;
    back.set_text_content(Some("← Gallery"));
    toolbar.append_child(&back)?;
    text_element(document, &toolbar, "span", artwork.title, "")?;
    let toolbar_actions = element(document, "div", "toolbar-actions")?;
    let enter = element(document, "button", "enter-fps-button")?;
    enter.set_attribute("type", "button")?;
    enter.set_text_content(Some("Explore room"));
    let fullscreen = element(document, "button", "fullscreen-button")?;
    fullscreen.set_attribute("type", "button")?;
    fullscreen.set_attribute("aria-pressed", "false")?;
    fullscreen.set_text_content(Some("Full screen"));
    let settings_toggle = element(document, "button", "settings-button")?;
    settings_toggle.set_attribute("type", "button")?;
    settings_toggle.set_attribute("aria-expanded", "false")?;
    settings_toggle.set_text_content(Some("Controls"));
    toolbar_actions.append_child(&enter)?;
    toolbar_actions.append_child(&fullscreen)?;
    toolbar_actions.append_child(&settings_toggle)?;
    toolbar.append_child(&toolbar_actions)?;
    installation.append_child(&toolbar)?;

    let saved_settings = read_settings();
    let session = Rc::new(RefCell::new(Session::new(saved_settings)));
    let settings_panel = create_settings_panel(document, &session)?;
    installation.append_child(&settings_panel)?;
    text_element(
        document,
        &installation,
        "p",
        "Click the room to capture the mouse · WASD moves · Space jumps · Ctrl crouches · Esc releases the mouse",
        "observer-help",
    )?;

    let root_for_back = root.clone();
    let document_for_back = document.clone();
    let session_for_back = Rc::clone(&session);
    let on_back = Closure::<dyn FnMut()>::new(move || {
        session_for_back.borrow_mut().active = false;
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            document.exit_pointer_lock();
        }
        let _ = render_catalogue(&document_for_back, &root_for_back);
    });
    back.add_event_listener_with_callback("click", on_back.as_ref().unchecked_ref())?;
    on_back.forget();

    let observer_for_enter = observer.clone();
    let on_enter = Closure::<dyn FnMut()>::new(move || {
        observer_for_enter.request_pointer_lock();
    });
    enter.add_event_listener_with_callback("click", on_enter.as_ref().unchecked_ref())?;
    on_enter.forget();

    let document_for_fullscreen = document.clone();
    let installation_for_fullscreen = installation.clone();
    let on_fullscreen = Closure::<dyn FnMut()>::new(move || {
        if document_for_fullscreen.fullscreen_element().is_some() {
            document_for_fullscreen.exit_fullscreen();
        } else {
            let _ = installation_for_fullscreen.request_fullscreen();
        }
    });
    fullscreen.add_event_listener_with_callback("click", on_fullscreen.as_ref().unchecked_ref())?;
    on_fullscreen.forget();

    let document_for_fullscreen_change = document.clone();
    let fullscreen_for_change = fullscreen.clone();
    let on_fullscreen_change = Closure::<dyn FnMut(Event)>::new(move |_| {
        let active = document_for_fullscreen_change
            .fullscreen_element()
            .is_some();
        fullscreen_for_change.set_text_content(Some(if active {
            "Exit full screen"
        } else {
            "Full screen"
        }));
        let _ = fullscreen_for_change
            .set_attribute("aria-pressed", if active { "true" } else { "false" });
    });
    document.add_event_listener_with_callback(
        "fullscreenchange",
        on_fullscreen_change.as_ref().unchecked_ref(),
    )?;
    on_fullscreen_change.forget();

    let observer_for_click = observer.clone();
    let on_observer_click = Closure::<dyn FnMut()>::new(move || {
        observer_for_click.request_pointer_lock();
    });
    observer
        .add_event_listener_with_callback("click", on_observer_click.as_ref().unchecked_ref())?;
    on_observer_click.forget();

    let panel_for_toggle = settings_panel.clone();
    let toggle_for_toggle = settings_toggle.clone();
    let on_toggle = Closure::<dyn FnMut()>::new(move || {
        let open = panel_for_toggle.has_attribute("hidden");
        if open {
            let _ = panel_for_toggle.remove_attribute("hidden");
            let _ = toggle_for_toggle.set_attribute("aria-expanded", "true");
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                document.exit_pointer_lock();
            }
        } else {
            let _ = panel_for_toggle.set_attribute("hidden", "true");
            let _ = toggle_for_toggle.set_attribute("aria-expanded", "false");
        }
    });
    settings_toggle
        .add_event_listener_with_callback("click", on_toggle.as_ref().unchecked_ref())?;
    on_toggle.forget();

    let renderer = rendering::create(artwork.kind, &canvas);
    match renderer {
        Ok(renderer) => {
            let renderer = Rc::new(renderer);
            attach_fps_controls(document, &installation, &observer, &session, renderer)?;
        }
        Err(_) => {
            let error = text_element(
                document,
                &installation,
                "p",
                "This browser could not start the gallery’s WebGL room. Try a current desktop browser with hardware graphics enabled.",
                "graphics-error",
            )?;
            error.set_attribute("role", "alert")?;
        }
    }

    root.append_child(&installation)?;
    Ok(())
}

struct Renderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    _buffer: WebGlBuffer,
    matrix_uniform: WebGlUniformLocation,
    vertex_count: i32,
}

impl Renderer {
    fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl")?
            .ok_or_else(|| JsValue::from_str("WebGL is unavailable."))?
            .dyn_into::<Gl>()?;
        let vertex = compile_shader(
            &gl,
            Gl::VERTEX_SHADER,
            r#"attribute vec3 a_position;
               attribute vec3 a_color;
               uniform mat4 u_matrix;
               varying vec3 v_color;
               void main() { gl_Position = u_matrix * vec4(a_position, 1.0); v_color = a_color; }"#,
        )?;
        let fragment = compile_shader(
            &gl,
            Gl::FRAGMENT_SHADER,
            r#"precision mediump float;
               varying vec3 v_color;
               void main() { gl_FragColor = vec4(v_color, 1.0); }"#,
        )?;
        let program = link_program(&gl, &vertex, &fragment)?;
        gl.use_program(Some(&program));
        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("Could not create the room buffer."))?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        let vertices = room_vertices();
        unsafe {
            let bytes = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &bytes, Gl::STATIC_DRAW);
        }
        let position = gl.get_attrib_location(&program, "a_position");
        let color = gl.get_attrib_location(&program, "a_color");
        if position < 0 || color < 0 {
            return Err(JsValue::from_str(
                "The room shader attributes are unavailable.",
            ));
        }
        gl.enable_vertex_attrib_array(position as u32);
        gl.vertex_attrib_pointer_with_i32(position as u32, 3, Gl::FLOAT, false, 24, 0);
        gl.enable_vertex_attrib_array(color as u32);
        gl.vertex_attrib_pointer_with_i32(color as u32, 3, Gl::FLOAT, false, 24, 12);
        let matrix_uniform = gl
            .get_uniform_location(&program, "u_matrix")
            .ok_or_else(|| JsValue::from_str("The room shader matrix is unavailable."))?;
        gl.enable(Gl::DEPTH_TEST);
        gl.depth_func(Gl::LEQUAL);
        Ok(Self {
            canvas: canvas.clone(),
            gl,
            program,
            _buffer: buffer,
            matrix_uniform,
            vertex_count: (vertices.len() / 6) as i32,
        })
    }

    fn render(&self, player: PlayerState) {
        let width = self.canvas.client_width().max(1) as u32;
        let height = self.canvas.client_height().max(1) as u32;
        let scale = window()
            .map(|window| window.device_pixel_ratio())
            .unwrap_or(1.0);
        let pixel_width = (width as f64 * scale).round() as u32;
        let pixel_height = (height as f64 * scale).round() as u32;
        if self.canvas.width() != pixel_width || self.canvas.height() != pixel_height {
            self.canvas.set_width(pixel_width);
            self.canvas.set_height(pixel_height);
        }
        self.gl
            .viewport(0, 0, pixel_width as i32, pixel_height as i32);
        self.gl.clear_color(0.93, 0.93, 0.92, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        self.gl.use_program(Some(&self.program));
        let (forward_x, forward_z) = player.forward_vector();
        let pitch = player.pitch_degrees.to_radians();
        let forward = [
            forward_x * pitch.cos(),
            -pitch.sin(),
            forward_z * pitch.cos(),
        ];
        let eye = [player.x, player.camera_y(), player.z];
        let center = [
            eye[0] + forward[0],
            eye[1] + forward[1],
            eye[2] + forward[2],
        ];
        let projection = Mat4::perspective(
            75.0_f32.to_radians(),
            pixel_width as f32 / pixel_height as f32,
            0.05,
            80.0,
        );
        let matrix = projection.multiply(Mat4::look_at(eye, center, [0.0, 1.0, 0.0]));
        self.gl
            .uniform_matrix4fv_with_f32_array(Some(&self.matrix_uniform), false, &matrix.0);
        self.gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }
}

fn required_uniform(
    gl: &Gl,
    program: &WebGlProgram,
    name: &str,
) -> Result<WebGlUniformLocation, JsValue> {
    gl.get_uniform_location(program, name)
        .ok_or_else(|| JsValue::from_str("The planetary shader is unavailable."))
}

fn upload_vertices(gl: &Gl, vertices: &[f32]) -> Result<WebGlBuffer, JsValue> {
    let buffer = gl
        .create_buffer()
        .ok_or_else(|| JsValue::from_str("Could not create a planetary mesh buffer."))?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    unsafe {
        let bytes = js_sys::Float32Array::view(vertices);
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &bytes, Gl::STATIC_DRAW);
    }
    Ok(buffer)
}

fn sphere_vertices(longitude_segments: u32, latitude_segments: u32) -> Vec<f32> {
    let mut vertices =
        Vec::with_capacity((longitude_segments * latitude_segments * 6 * 6) as usize);
    for latitude in 0..latitude_segments {
        for longitude in 0..longitude_segments {
            let corner = |longitude: u32, latitude: u32| {
                let u = longitude as f32 / longitude_segments as f32;
                let v = latitude as f32 / latitude_segments as f32;
                let theta = u * std::f32::consts::TAU;
                let phi = v * std::f32::consts::PI;
                [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()]
            };
            let corners = [
                corner(longitude, latitude),
                corner(longitude + 1, latitude),
                corner(longitude + 1, latitude + 1),
                corner(longitude, latitude + 1),
            ];
            for index in [0, 1, 2, 0, 2, 3] {
                vertices.extend_from_slice(&corners[index]);
                vertices.extend_from_slice(&corners[index]);
            }
        }
    }
    vertices
}

fn planetarium_room_vertices() -> Vec<f32> {
    let mut vertices = Vec::with_capacity(36 * 6);
    add_normal_face(
        &mut vertices,
        [
            [-8.0, 0.0, -8.0],
            [8.0, 0.0, -8.0],
            [8.0, 0.0, 8.0],
            [-8.0, 0.0, 8.0],
        ],
        [0.0, 1.0, 0.0],
    );
    add_normal_face(
        &mut vertices,
        [
            [-8.0, 4.0, 8.0],
            [8.0, 4.0, 8.0],
            [8.0, 4.0, -8.0],
            [-8.0, 4.0, -8.0],
        ],
        [0.0, -1.0, 0.0],
    );
    add_normal_face(
        &mut vertices,
        [
            [-8.0, 0.0, -8.0],
            [-8.0, 4.0, -8.0],
            [8.0, 4.0, -8.0],
            [8.0, 0.0, -8.0],
        ],
        [0.0, 0.0, 1.0],
    );
    add_normal_face(
        &mut vertices,
        [
            [8.0, 0.0, 8.0],
            [8.0, 4.0, 8.0],
            [-8.0, 4.0, 8.0],
            [-8.0, 0.0, 8.0],
        ],
        [0.0, 0.0, -1.0],
    );
    add_normal_face(
        &mut vertices,
        [
            [-8.0, 0.0, 8.0],
            [-8.0, 4.0, 8.0],
            [-8.0, 4.0, -8.0],
            [-8.0, 0.0, -8.0],
        ],
        [1.0, 0.0, 0.0],
    );
    add_normal_face(
        &mut vertices,
        [
            [8.0, 0.0, -8.0],
            [8.0, 4.0, -8.0],
            [8.0, 4.0, 8.0],
            [8.0, 0.0, 8.0],
        ],
        [-1.0, 0.0, 0.0],
    );
    vertices
}

fn add_normal_face(vertices: &mut Vec<f32>, corners: [[f32; 3]; 4], normal: [f32; 3]) {
    for index in [0, 1, 2, 0, 2, 3] {
        vertices.extend_from_slice(&corners[index]);
        vertices.extend_from_slice(&normal);
    }
}

fn star_vertices() -> Vec<f32> {
    let mut seed = 0x8a5c_1f27_u32;
    let mut stars = Vec::with_capacity(300 * 6);
    for _ in 0..300 {
        let next = |seed: &mut u32| {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (*seed >> 8) as f32 / ((u32::MAX >> 8) as f32)
        };
        let direction = normalize([
            next(&mut seed) * 2.0 - 1.0,
            next(&mut seed) * 2.0 - 1.0,
            next(&mut seed) * 2.0 - 1.0,
        ]);
        let radius = 28.0 + next(&mut seed) * 20.0;
        stars.extend_from_slice(&[
            direction[0] * radius,
            direction[1] * radius,
            direction[2] * radius,
            0.0,
            0.0,
            0.0,
        ]);
    }
    stars
}

fn resize_canvas(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let width = canvas.client_width().max(1) as u32;
    let height = canvas.client_height().max(1) as u32;
    let scale = window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0);
    let pixel_width = (width as f64 * scale).round() as u32;
    let pixel_height = (height as f64 * scale).round() as u32;
    if canvas.width() != pixel_width || canvas.height() != pixel_height {
        canvas.set_width(pixel_width);
        canvas.set_height(pixel_height);
    }
    (pixel_width, pixel_height)
}

fn compile_shader(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("Could not create a room shader."))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    gl.get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .filter(|compiled| *compiled)
        .map(|_| shader)
        .ok_or_else(|| JsValue::from_str("The room shader could not compile."))
}

fn link_program(
    gl: &Gl,
    vertex: &WebGlShader,
    fragment: &WebGlShader,
) -> Result<WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("Could not create a room shader program."))?;
    gl.attach_shader(&program, vertex);
    gl.attach_shader(&program, fragment);
    gl.link_program(&program);
    gl.get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .filter(|linked| *linked)
        .map(|_| program)
        .ok_or_else(|| JsValue::from_str("The room shader could not link."))
}

fn room_vertices() -> Vec<f32> {
    let mut vertices = Vec::with_capacity(36 * 6 * 6);
    let wall = [0.94, 0.94, 0.92];
    add_face(
        &mut vertices,
        [
            [-8.0, 0.0, -8.0],
            [8.0, 0.0, -8.0],
            [8.0, 0.0, 8.0],
            [-8.0, 0.0, 8.0],
        ],
        [0.82, 0.82, 0.80],
    );
    add_face(
        &mut vertices,
        [
            [-8.0, 4.0, -8.0],
            [-8.0, 4.0, 8.0],
            [8.0, 4.0, 8.0],
            [8.0, 4.0, -8.0],
        ],
        [0.99, 0.99, 0.98],
    );
    add_face(
        &mut vertices,
        [
            [-8.0, 0.0, -8.0],
            [-8.0, 4.0, -8.0],
            [8.0, 4.0, -8.0],
            [8.0, 0.0, -8.0],
        ],
        wall,
    );
    add_face(
        &mut vertices,
        [
            [8.0, 0.0, 8.0],
            [8.0, 4.0, 8.0],
            [-8.0, 4.0, 8.0],
            [-8.0, 0.0, 8.0],
        ],
        [0.90, 0.90, 0.88],
    );
    add_face(
        &mut vertices,
        [
            [-8.0, 0.0, 8.0],
            [-8.0, 4.0, 8.0],
            [-8.0, 4.0, -8.0],
            [-8.0, 0.0, -8.0],
        ],
        [0.87, 0.87, 0.85],
    );
    add_face(
        &mut vertices,
        [
            [8.0, 0.0, -8.0],
            [8.0, 4.0, -8.0],
            [8.0, 4.0, 8.0],
            [8.0, 0.0, 8.0],
        ],
        [0.91, 0.91, 0.90],
    );
    add_cube(&mut vertices);
    vertices
}

fn add_cube(vertices: &mut Vec<f32>) {
    add_face(
        vertices,
        [
            [-1.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 2.0, 1.0],
            [-1.0, 2.0, 1.0],
        ],
        [0.01, 0.01, 0.01],
    );
    add_face(
        vertices,
        [
            [1.0, 0.0, -1.0],
            [-1.0, 0.0, -1.0],
            [-1.0, 2.0, -1.0],
            [1.0, 2.0, -1.0],
        ],
        [0.04, 0.04, 0.04],
    );
    add_face(
        vertices,
        [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, -1.0],
            [1.0, 2.0, -1.0],
            [1.0, 2.0, 1.0],
        ],
        [0.08, 0.08, 0.08],
    );
    add_face(
        vertices,
        [
            [-1.0, 0.0, -1.0],
            [-1.0, 0.0, 1.0],
            [-1.0, 2.0, 1.0],
            [-1.0, 2.0, -1.0],
        ],
        [0.025, 0.025, 0.025],
    );
    add_face(
        vertices,
        [
            [-1.0, 2.0, 1.0],
            [1.0, 2.0, 1.0],
            [1.0, 2.0, -1.0],
            [-1.0, 2.0, -1.0],
        ],
        [0.12, 0.12, 0.12],
    );
    add_face(
        vertices,
        [
            [-1.0, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
        ],
        [0.0, 0.0, 0.0],
    );
}

fn add_face(vertices: &mut Vec<f32>, corners: [[f32; 3]; 4], color: [f32; 3]) {
    for index in [0, 1, 2, 0, 2, 3] {
        vertices.extend_from_slice(&corners[index]);
        vertices.extend_from_slice(&color);
    }
}
