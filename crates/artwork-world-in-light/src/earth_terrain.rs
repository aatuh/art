//! Deterministic, renderer-independent Earth terrain clearance.
//!
//! The height field is derived from the final quantized material texture and uses the same
//! normalized unsigned-byte representation intended for WebGL. Navigation samples it with
//! WebGL's texel-centre bilinear convention without decoding images or depending on browser APIs.

use crate::planet::Vec3d;

pub const TERRAIN_HEIGHT_WIDTH: usize = 2048;
pub const TERRAIN_HEIGHT_HEIGHT: usize = 1024;
pub const TERRAIN_HEIGHT_BYTE_LEN: usize = TERRAIN_HEIGHT_WIDTH * TERRAIN_HEIGHT_HEIGHT;
pub const TERRAIN_HEIGHT_FIELD_MAX_M: f64 = 10_000.0;
pub const LOCAL_TERRAIN_RELIEF_MAX_M: f64 = 200.0;
pub const MAX_TERRAIN_HEIGHT_M: f64 = TERRAIN_HEIGHT_FIELD_MAX_M + LOCAL_TERRAIN_RELIEF_MAX_M;
pub const TERRAIN_HEIGHT_METRES_PER_LEVEL: f64 = TERRAIN_HEIGHT_FIELD_MAX_M / u8::MAX as f64;

pub static TERRAIN_HEIGHT_BYTES: &[u8; TERRAIN_HEIGHT_BYTE_LEN] =
    include_bytes!("../../../assets/earth/earth-terrain-height-2048x1024-u8.bin");

/// Returns the bilinearly filtered positive land elevation for an Earth-fixed unit direction.
///
/// Ocean texels and malformed directions return sea level (`0 m`). Longitude repeats and latitude
/// clamps at the poles, matching the height texture's WebGL sampler contract.
pub fn terrain_height_m(earth_fixed_direction: Vec3d) -> f64 {
    if !earth_fixed_direction.is_finite() {
        return 0.0;
    }
    let direction = earth_fixed_direction.normalized();
    if direction == Vec3d::ZERO {
        return 0.0;
    }

    let longitude = direction.z.atan2(direction.x);
    let latitude = direction.y.clamp(-1.0, 1.0).asin();
    let u = (longitude / std::f64::consts::TAU + 0.5).rem_euclid(1.0);
    let v = (0.5 - latitude / std::f64::consts::PI).clamp(0.0, 1.0);
    let base_height_m = terrain_height_uv(u, v);
    base_height_m + local_relief_capacity_m(base_height_m)
}

fn local_relief_capacity_m(base_height_m: f64) -> f64 {
    let amount = (base_height_m / 250.0).clamp(0.0, 1.0);
    let smoothed = amount * amount * (3.0 - 2.0 * amount);
    LOCAL_TERRAIN_RELIEF_MAX_M * smoothed
}

fn terrain_height_uv(u: f64, v: f64) -> f64 {
    if !u.is_finite() || !v.is_finite() {
        return 0.0;
    }

    let texel_x = u.rem_euclid(1.0) * TERRAIN_HEIGHT_WIDTH as f64 - 0.5;
    let texel_y = v.clamp(0.0, 1.0) * TERRAIN_HEIGHT_HEIGHT as f64 - 0.5;
    let x0 = texel_x.floor() as isize;
    let y0 = texel_y.floor() as isize;
    let x_fraction = texel_x - x0 as f64;
    let y_fraction = texel_y - y0 as f64;

    let north_west = terrain_height_texel_m(x0, y0);
    let north_east = terrain_height_texel_m(x0 + 1, y0);
    let south_west = terrain_height_texel_m(x0, y0 + 1);
    let south_east = terrain_height_texel_m(x0 + 1, y0 + 1);
    let north = lerp(north_west, north_east, x_fraction);
    let south = lerp(south_west, south_east, x_fraction);
    lerp(north, south, y_fraction).clamp(0.0, TERRAIN_HEIGHT_FIELD_MAX_M)
}

fn terrain_height_texel_m(x: isize, y: isize) -> f64 {
    let x = x.rem_euclid(TERRAIN_HEIGHT_WIDTH as isize) as usize;
    let y = y.clamp(0, TERRAIN_HEIGHT_HEIGHT as isize - 1) as usize;
    f64::from(TERRAIN_HEIGHT_BYTES[y * TERRAIN_HEIGHT_WIDTH + x]) * TERRAIN_HEIGHT_METRES_PER_LEVEL
}

fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_direction(latitude_degrees: f64, longitude_degrees: f64) -> Vec3d {
        let latitude = latitude_degrees.to_radians();
        let longitude = longitude_degrees.to_radians();
        Vec3d::new(
            latitude.cos() * longitude.cos(),
            latitude.sin(),
            latitude.cos() * longitude.sin(),
        )
    }

    #[test]
    fn generated_height_field_is_complete_and_bounded() {
        assert_eq!(TERRAIN_HEIGHT_BYTES.len(), TERRAIN_HEIGHT_BYTE_LEN);
        assert!(TERRAIN_HEIGHT_BYTES.contains(&0));
        assert!(
            TERRAIN_HEIGHT_BYTES
                .iter()
                .any(|height| f64::from(*height) * TERRAIN_HEIGHT_METRES_PER_LEVEL > 5_000.0)
        );
        assert_eq!(terrain_height_texel_m(0, 0), 0.0);
        assert_eq!(
            f64::from(u8::MAX) * TERRAIN_HEIGHT_METRES_PER_LEVEL,
            TERRAIN_HEIGHT_FIELD_MAX_M
        );
    }

    #[test]
    fn texel_centres_and_midpoints_follow_webgl_linear_filtering() {
        let (index, pair) = TERRAIN_HEIGHT_BYTES
            .windows(2)
            .enumerate()
            .find(|(index, pair)| {
                index % TERRAIN_HEIGHT_WIDTH != TERRAIN_HEIGHT_WIDTH - 1 && pair[0] != pair[1]
            })
            .expect("height field must contain a horizontal transition");
        let x = index % TERRAIN_HEIGHT_WIDTH;
        let y = index / TERRAIN_HEIGHT_WIDTH;
        let centre_v = (y as f64 + 0.5) / TERRAIN_HEIGHT_HEIGHT as f64;
        let first_centre_u = (x as f64 + 0.5) / TERRAIN_HEIGHT_WIDTH as f64;
        let midpoint_u = (x as f64 + 1.0) / TERRAIN_HEIGHT_WIDTH as f64;
        let first = f64::from(pair[0]) * TERRAIN_HEIGHT_METRES_PER_LEVEL;
        let second = f64::from(pair[1]) * TERRAIN_HEIGHT_METRES_PER_LEVEL;

        assert!((terrain_height_uv(first_centre_u, centre_v) - first).abs() < 1.0e-9);
        assert!((terrain_height_uv(midpoint_u, centre_v) - (first + second) * 0.5).abs() < 1.0e-9);
    }

    #[test]
    fn longitude_repeats_continuously_and_latitude_clamps() {
        let epsilon = 1.0e-9;
        for v in [0.0, 0.17, 0.5, 0.83, 1.0] {
            assert!((terrain_height_uv(-epsilon, v) - terrain_height_uv(epsilon, v)).abs() < 0.05);
            assert_eq!(terrain_height_uv(0.37, v), terrain_height_uv(1.37, v));
        }
        assert_eq!(terrain_height_uv(0.37, -1.0), terrain_height_uv(0.37, 0.0));
        assert_eq!(terrain_height_uv(0.37, 2.0), terrain_height_uv(0.37, 1.0));
    }

    #[test]
    fn bilinear_samples_remain_finite_and_bounded() {
        for y in 0..=128 {
            for x in 0..=256 {
                let height = terrain_height_uv(x as f64 / 256.0, y as f64 / 128.0);
                assert!(height.is_finite());
                assert!((0.0..=TERRAIN_HEIGHT_FIELD_MAX_M).contains(&height));
            }
        }
    }

    #[test]
    fn navigation_reserves_the_bounded_local_relief_capacity() {
        assert_eq!(local_relief_capacity_m(0.0), 0.0);
        assert_eq!(local_relief_capacity_m(250.0), LOCAL_TERRAIN_RELIEF_MAX_M);
        assert!(local_relief_capacity_m(125.0) > 0.0);
        assert!(local_relief_capacity_m(125.0) < LOCAL_TERRAIN_RELIEF_MAX_M);
        assert!(terrain_height_m(fixed_direction(-6.0, -76.0)) <= MAX_TERRAIN_HEIGHT_M);
    }

    #[test]
    fn authored_land_sites_have_positive_collision_relief() {
        for (latitude, longitude) in [
            (-13.8, -171.8),
            (-6.0, -76.0),
            (-3.0, -60.0),
            (3.0, 36.0),
            (7.0, 81.0),
            (1.0, 114.0),
        ] {
            assert!(terrain_height_m(fixed_direction(latitude, longitude)) > 0.0);
        }
    }

    #[test]
    fn malformed_directions_fall_back_to_sea_level() {
        assert_eq!(terrain_height_m(Vec3d::ZERO), 0.0);
        assert_eq!(terrain_height_m(Vec3d::new(f64::NAN, 0.0, 0.0)), 0.0);
    }
}
