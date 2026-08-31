//! WebGL adapter for the Black Cube / White Room artwork.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext as Gl, WebGlShader,
    WebGlUniformLocation,
};

use artwork_black_cube::{BLACK_CUBE_SCENE, PlayerState};

use crate::math::Mat4;

use crate::browser::dom::window;

pub(super) struct BlackCubeRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    buffer: WebGlBuffer,
    matrix_uniform: WebGlUniformLocation,
    vertex_count: i32,
}

impl Drop for BlackCubeRenderer {
    fn drop(&mut self) {
        self.gl.use_program(None);
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, None);
        self.gl.delete_buffer(Some(&self.buffer));
        self.gl.delete_program(Some(&self.program));
    }
}

impl BlackCubeRenderer {
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
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
        let authored_vertices = BLACK_CUBE_SCENE.vertices();
        let vertices = authored_vertices
            .iter()
            .flat_map(|vertex| vertex.position.into_iter().chain(vertex.color))
            .collect::<Vec<_>>();
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
            buffer,
            matrix_uniform,
            vertex_count: authored_vertices.len() as i32,
        })
    }

    pub(super) fn render(&self, player: &PlayerState) {
        let width = self.canvas.client_width().max(1) as u32;
        let height = self.canvas.client_height().max(1) as u32;
        let scale = window()
            .map(|window| window.device_pixel_ratio())
            .unwrap_or(1.0);
        let pixel_width = (f64::from(width) * scale).round() as u32;
        let pixel_height = (f64::from(height) * scale).round() as u32;
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
        let pitch = player.pitch_degrees().to_radians();
        let forward = [
            forward_x * pitch.cos(),
            -pitch.sin(),
            forward_z * pitch.cos(),
        ];
        let (player_x, player_z) = player.position();
        let eye = [player_x, player.camera_y(), player_z];
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
