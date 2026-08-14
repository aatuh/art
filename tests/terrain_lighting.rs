#[path = "../src/terrain_lighting.rs"]
mod terrain_lighting;

use terrain_lighting::{
    TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M, TERRAIN_SHADOW_ORIGIN_BIAS_M,
    TERRAIN_SHADOW_SAMPLE_DISTANCES_M, terrain_clearance_visibility, terrain_shadow_enabled,
    terrain_shadow_visibility,
};

#[test]
fn terrain_shadow_policy_is_ready_for_shader_mirroring() {
    assert_eq!(TERRAIN_SHADOW_SAMPLE_DISTANCES_M.len(), 6);
    assert!(TERRAIN_SHADOW_ORIGIN_BIAS_M > 0.0);
    assert!(terrain_shadow_enabled(
        TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M,
        1.0
    ));
    assert!(!terrain_shadow_enabled(
        TERRAIN_SHADOW_MAX_CAMERA_ALTITUDE_M + 1.0,
        1.0
    ));
    assert_eq!(terrain_clearance_visibility(-1.0), 0.0);
    assert_eq!(terrain_shadow_visibility(&[10_000.0, -1.0]), 0.0);
}
