use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlTexture,
    WebGlUniformLocation, WebGlVertexArrayObject,
};
use crate::camera_api::CameraState;
use crate::planet::{
    CelestialFrame, EARTH_ATMOSPHERE_TOP_M, EARTH_EQUATORIAL_RADIUS_M,
    EARTH_POLAR_RADIUS_M, MOON_MEAN_RADIUS_M, SUN_NOMINAL_RADIUS_M, Vec3d,
};
