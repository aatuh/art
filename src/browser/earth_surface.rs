//! Compact global Earth reference derived from NASA Earth Observatory Blue Marble imagery.

pub(super) const WIDTH: i32 = 128;
pub(super) const HEIGHT: i32 = 64;

pub(super) fn pixels_rgba() -> Vec<u8> {
    let mut pixels = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
    for byte in include_bytes!("earth_surface_data.txt") {
        if matches!(byte, b'\n' | b'\r') {
            continue;
        }
        let land = matches!(byte, b'g'..=b'z' | b'0'..=b'9' | b'-' | b'_');
        let color = if !land {
            [4, 25, 66]
        } else if matches!(byte, b'_' | b'-' | b'9' | b'8' | b'7') {
            [220, 225, 224]
        } else if matches!(byte, b'0'..=b'6' | b'x'..=b'z') {
            [171, 144, 101]
        } else if matches!(byte, b'r'..=b'w') {
            [75, 86, 45]
        } else {
            [42, 67, 27]
        };
        pixels.extend_from_slice(&[color[0], color[1], color[2], if land { 255 } else { 0 }]);
    }
    debug_assert_eq!(pixels.len(), WIDTH as usize * HEIGHT as usize * 4);
    pixels
}
