use wasm_bindgen::JsValue;
use web_sys::{
    WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlVertexArrayObject,
};

pub(super) fn create(
    gl: &Gl,
    program: &WebGlProgram,
) -> Result<(WebGlVertexArrayObject, WebGlBuffer), JsValue> {
    let vertex_array = gl
        .create_vertex_array()
        .ok_or_else(|| JsValue::from_str("Could not create the planet vertex array."))?;
    let vertex_buffer = gl
        .create_buffer()
        .ok_or_else(|| JsValue::from_str("Could not create the planet vertex buffer."))?;
    gl.bind_vertex_array(Some(&vertex_array));
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&vertex_buffer));
    let vertices = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0];
    unsafe {
        let array = js_sys::Float32Array::view(&vertices);
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::STATIC_DRAW);
    }
    let position = gl.get_attrib_location(program, "a_position");
    if position < 0 {
        return Err(JsValue::from_str("Planetary position attribute is unavailable."));
    }
    gl.enable_vertex_attrib_array(position as u32);
    gl.vertex_attrib_pointer_with_i32(position as u32, 2, Gl::FLOAT, false, 0, 0);
    Ok((vertex_array, vertex_buffer))
}
