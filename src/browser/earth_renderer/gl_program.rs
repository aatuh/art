use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as Gl, WebGlProgram, WebGlShader, WebGlUniformLocation};

pub(super) fn required_uniform(
    gl: &Gl,
    program: &WebGlProgram,
    name: &str,
) -> Result<WebGlUniformLocation, JsValue> {
    gl.get_uniform_location(program, name)
        .ok_or_else(|| JsValue::from_str("A required planetary shader uniform is unavailable."))
}

pub(super) fn compile_shader(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
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
        return Ok(shader);
    }
    let message = gl
        .get_shader_info_log(&shader)
        .unwrap_or_else(|| "Unknown shader compiler error.".to_owned());
    web_sys::console::error_1(&JsValue::from_str(&message));
    Err(JsValue::from_str("The planetary shader could not compile."))
}

pub(super) fn link_program(
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
        return Ok(program);
    }
    let message = gl
        .get_program_info_log(&program)
        .unwrap_or_else(|| "Unknown shader linker error.".to_owned());
    web_sys::console::error_1(&JsValue::from_str(&message));
    Err(JsValue::from_str("The planetary shader program could not link."))
}
