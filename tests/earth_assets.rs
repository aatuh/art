use std::{fs, path::Path};

fn asset(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/earth")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn assert_lossy_webp(name: &str, expected_width: u16, expected_height: u16, minimum_size: usize) {
    let bytes = asset(name);
    assert!(bytes.len() >= minimum_size, "{name} is suspiciously small");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().expect("RIFF size")) as usize + 8,
        bytes.len(),
        "{name} is truncated"
    );
    assert_eq!(
        &bytes[12..16],
        b"VP8 ",
        "{name} must use the checked VP8 layout"
    );
    assert_eq!(&bytes[23..26], [0x9d, 0x01, 0x2a]);
    let width = u16::from_le_bytes(bytes[26..28].try_into().expect("VP8 width")) & 0x3fff;
    let height = u16::from_le_bytes(bytes[28..30].try_into().expect("VP8 height")) & 0x3fff;
    assert_eq!((width, height), (expected_width, expected_height));
}

fn assert_lossless_webp(
    name: &str,
    expected_width: u32,
    expected_height: u32,
    minimum_size: usize,
) {
    let bytes = asset(name);
    assert!(bytes.len() >= minimum_size, "{name} is suspiciously small");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().expect("RIFF size")) as usize + 8,
        bytes.len(),
        "{name} is truncated"
    );
    assert_eq!(&bytes[12..16], b"VP8L", "{name} must be lossless WebP");
    assert_eq!(bytes[20], 0x2f, "{name} has an invalid VP8L signature");
    let dimensions = u32::from_le_bytes(bytes[21..25].try_into().expect("VP8L dimensions"));
    let width = (dimensions & 0x3fff) + 1;
    let height = ((dimensions >> 14) & 0x3fff) + 1;
    assert_eq!((width, height), (expected_width, expected_height));
}

fn assert_png(name: &str, expected_width: u32, expected_height: u32, minimum_size: usize) {
    let bytes = asset(name);
    assert!(bytes.len() >= minimum_size, "{name} is suspiciously small");
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&bytes[12..16], b"IHDR");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height"));
    assert_eq!((width, height), (expected_width, expected_height));
    assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
}

#[test]
fn planet_assets_are_complete_and_correctly_sized() {
    assert_lossy_webp("earth-surface-4096.webp", 4096, 2048, 700_000);
    assert_lossy_webp("earth-night-2048.webp", 2048, 1024, 100_000);
    assert_lossy_webp("moon-albedo-2048.webp", 2048, 1024, 500_000);
    assert_lossless_webp("earth-weather-1024.webp", 1024, 512, 50_000);
    assert_png("earth-material-2048.png", 2048, 1024, 500_000);
    // The star map is intentionally sparse and therefore compresses much more than
    // the material map. Keep a useful floor that still rejects an empty placeholder.
    assert_png("stars-1024.png", 1024, 1024, 16_000);
    let terrain_height = asset("earth-terrain-height-2048x1024-u8.bin");
    assert_eq!(terrain_height.len(), 2048 * 1024);
    assert!(terrain_height.contains(&0));
    assert!(terrain_height.iter().any(|height| *height > 127));
}
