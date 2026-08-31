use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as Gl, WebGlTexture};

pub(super) fn create(gl: &Gl) -> Result<WebGlTexture, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("Could not create the Earth surface texture."))?;
    gl.active_texture(Gl::TEXTURE0);
    gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
    gl.pixel_storei(Gl::UNPACK_ALIGNMENT, 1);
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::REPEAT as i32);
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(
        Gl::TEXTURE_2D,
        Gl::TEXTURE_MIN_FILTER,
        Gl::LINEAR_MIPMAP_LINEAR as i32,
    );
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);
    // Every image-backed sampler starts from a complete neutral texture. Readiness
    // uniforms select the procedural fallbacks until the validated image arrives.
    let pixels = [4_u8, 25, 66, 0];
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA as i32,
        1,
        1,
        0,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(&pixels),
    )?;
    gl.generate_mipmap(Gl::TEXTURE_2D);
    Ok(texture)
}

pub(super) fn create_height_field(
    gl: &Gl,
    width: i32,
    height: i32,
    pixels: &[u8],
) -> Result<WebGlTexture, JsValue> {
    if width <= 0 || height <= 0 || pixels.len() != width as usize * height as usize {
        return Err(JsValue::from_str(
            "Earth terrain height data has invalid dimensions.",
        ));
    }
    let texture = create(gl)?;
    gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        Gl::TEXTURE_2D,
        0,
        Gl::R8 as i32,
        width,
        height,
        0,
        Gl::RED,
        Gl::UNSIGNED_BYTE,
        Some(pixels),
    )?;
    gl.generate_mipmap(Gl::TEXTURE_2D);
    Ok(texture)
}
