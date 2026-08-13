use web_sys::HtmlCanvasElement;

use super::super::dom::window;

pub(super) fn resize(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let css_width = canvas.client_width().max(1) as u32;
    let css_height = canvas.client_height().max(1) as u32;
    let device_scale = window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0)
        .clamp(1.0, 2.0);
    let area = f64::from(css_width) * f64::from(css_height);
    let scale = device_scale.min((4_200_000.0 / area).sqrt().clamp(1.0, 2.0));
    let width = (f64::from(css_width) * scale).round() as u32;
    let height = (f64::from(css_height) * scale).round() as u32;
    if canvas.width() != width || canvas.height() != height {
        canvas.set_width(width);
        canvas.set_height(height);
    }
    (width, height)
}
