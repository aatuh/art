//! Renderer-independent terrain-lighting policy shared with shader validation.

/// Terrain cast shadows are only worth the extra ray samples near Earth.
pub const TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M: f64 = 750_000.0;
/// Offset the shadow ray above the rendered surface to avoid self-shadow acne.
pub const TERRAIN_SHADOW_ORIGIN_BIAS_M: f64 = 80.0;
/// Softens the binary terrain-clearance test over roughly half a kilometre.
pub const TERRAIN_SHADOW_PENUMBRA_M: f64 = 450.0;
/// Geometric light-ray samples, ordered near-to-far from the shaded point.
pub const TERRAIN_SHADOW_SAMPLE_DISTANCES_M: [f64; 6] =
    [800.0, 2_000.0, 5_000.0, 12_000.0, 28_000.0, 60_000.0];

/// Whether terrain-shadow sampling should run for the current fragment/camera state.
pub fn terrain_shadow_enabled(camera_altitude_m: f64, land_fraction: f64) -> bool {
    camera_altitude_m.is_finite()
        && (0.0..=TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M).contains(&camera_altitude_m)
        && land_fraction.is_finite()
        && land_fraction > 0.02
}

/// Converts terrain clearance above a sampled light ray to a soft visibility value.
///
/// Negative clearance is fully occluded, while clearance beyond the penumbra width is
/// fully visible. The cubic Hermite curve mirrors GLSL `smoothstep(0, width, x)`.
pub fn terrain_clearance_visibility(clearance_m: f64) -> f64 {
    if !clearance_m.is_finite() {
        return 1.0;
    }
    let t = (clearance_m / TERRAIN_SHADOW_PENUMBRA_M).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Combines individual terrain-clearance samples into the conservative direct-light
/// visibility used by the renderer: the darkest obstruction wins.
pub fn terrain_shadow_visibility(clearances_m: &[f64]) -> f64 {
    clearances_m
        .iter()
        .copied()
        .map(terrain_clearance_visibility)
        .fold(1.0, f64::min)
}

#[cfg(test)]
mod tests {
    use super::{
        TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M, TERRAIN_SHADOW_SAMPLE_DISTANCES_M,
        terrain_clearance_visibility, terrain_shadow_enabled, terrain_shadow_visibility,
    };

    #[test]
    fn sample_distances_are_strictly_increasing() {
        assert!(
            TERRAIN_SHADOW_SAMPLE_DISTANCES_M
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
    }

    #[test]
    fn shadow_sampling_is_limited_to_near_land_views() {
        assert!(terrain_shadow_enabled(400_000.0, 1.0));
        assert!(terrain_shadow_enabled(
            TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M,
            0.5
        ));
        assert!(!terrain_shadow_enabled(
            TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M + 1.0,
            1.0
        ));
        assert!(!terrain_shadow_enabled(10_000.0, 0.0));
        assert!(!terrain_shadow_enabled(f64::NAN, 1.0));
    }

    #[test]
    fn clearance_visibility_matches_smoothstep_endpoints() {
        assert_eq!(terrain_clearance_visibility(-100.0), 0.0);
        assert_eq!(terrain_clearance_visibility(0.0), 0.0);
        assert_eq!(terrain_clearance_visibility(450.0), 1.0);
        assert_eq!(terrain_clearance_visibility(10_000.0), 1.0);
        assert_eq!(terrain_clearance_visibility(f64::NAN), 1.0);
    }

    #[test]
    fn midpoint_clearance_is_half_visible() {
        assert!((terrain_clearance_visibility(225.0) - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn darkest_terrain_sample_controls_visibility() {
        let visibility = terrain_shadow_visibility(&[2_000.0, 600.0, 225.0, 1_000.0]);
        assert!((visibility - 0.5).abs() < 1.0e-12);
        assert_eq!(terrain_shadow_visibility(&[]), 1.0);
        assert_eq!(terrain_shadow_visibility(&[-1.0, 5_000.0]), 0.0);
    }
}
