//! Deterministic planetary shader selection and detailed-shader composition.

const PLANET_ORBITAL_FRAGMENT: &str = include_str!("browser/shaders/planet_orbital.frag.glsl");
const PLANET_FRAGMENT_BASE: &str = include_str!("browser/shaders/planet_v4.frag.glsl");
const OCEAN_SURFACE_MODULE: &str = include_str!("browser/shaders/ocean_surface.glsl");
const TERRAIN_SHADOW_MODULE: &str = include_str!("browser/shaders/terrain_shadow.glsl");

const OCEAN_NORMAL_BASE: &str = r#"vec3 ocean_normal(vec3 point, vec3 geometric) {
    vec3 radial = normalize(point - u_earth_center_m);
    vec3 local = earth_fixed_direction(point);
    vec3 tangent = cross(abs(radial.y) < 0.95 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0), radial);
    tangent = normalize(tangent);
    vec3 bitangent = normalize(cross(radial, tangent));
    float time = u_time;
    float wave_a = sin(local.x * 1100.0 + local.z * 740.0 + time * 0.85);
    float wave_b = sin(local.z * 1550.0 - local.x * 430.0 + time * 1.17);
    float wave_c = noise3(local * 950.0 + vec3(time * 0.02, 0.0, -time * 0.015)) * 2.0 - 1.0;
    return normalize(geometric + tangent * (wave_a + wave_c) * 0.012 + bitangent * wave_b * 0.010);
}"#;
const ATMOSPHERE_RAYMARCH_BASE: &str = r#"    float step_length = (end_distance - start_distance) / 10.0;
    vec3 view_depth = vec3(0.0);
    vec3 rayleigh_sum = vec3(0.0);
    vec3 mie_sum = vec3(0.0);
    for (int sample_index = 0; sample_index < 10; ++sample_index) {
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;"#;
const ATMOSPHERE_RAYMARCH_REPLACEMENT: &str = r#"    int atmosphere_sample_count = u_camera_altitude_m < 500000.0
        ? 10
        : (u_camera_altitude_m < 5000000.0 ? 8 : 6);
    float step_length = (end_distance - start_distance) / float(atmosphere_sample_count);
    vec3 view_depth = vec3(0.0);
    vec3 rayleigh_sum = vec3(0.0);
    vec3 mie_sum = vec3(0.0);
    for (int sample_index = 0; sample_index < 10; ++sample_index) {
        if (sample_index >= atmosphere_sample_count) {
            break;
        }
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;"#;
const CLOUD_RAYMARCH_BASE: &str = r#"    float step_length = (end_distance - start_distance) / 8.0;
    for (int sample_index = 0; sample_index < 8; ++sample_index) {
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;"#;
const CLOUD_RAYMARCH_REPLACEMENT: &str = r#"    int cloud_sample_count = u_camera_altitude_m < 500000.0
        ? 8
        : (u_camera_altitude_m < 2000000.0 ? 6 : 4);
    float step_length = (end_distance - start_distance) / float(cloud_sample_count);
    float sample_jitter = mix(0.2, 0.8, hash31(vec3(gl_FragCoord.xy, 17.0)));
    for (int sample_index = 0; sample_index < 8; ++sample_index) {
        if (sample_index >= cloud_sample_count) {
            break;
        }
        float distance_along_ray =
            start_distance + (float(sample_index) + sample_jitter) * step_length;"#;
const CLOUD_LIGHT_TRANSMISSION_BASE: &str = r#"float cloud_light_transmission(vec3 point, vec3 light_direction) {
    float optical = 0.0;
    for (int step_index = 0; step_index < 4; ++step_index) {
        float distance_along_light = (float(step_index) + 1.0) * 3600.0;
        optical += cloud_density(point + light_direction * distance_along_light);
    }
    return exp(-optical * 0.72);
}"#;
const CLOUD_LIGHT_TRANSMISSION_REPLACEMENT: &str = r#"float cloud_light_transmission(vec3 point, vec3 light_direction) {
    float optical = 0.0;
    for (int step_index = 0; step_index < 2; ++step_index) {
        float distance_along_light = (float(step_index) + 1.0) * 6000.0;
        optical += cloud_density(point + light_direction * distance_along_light);
    }
    return exp(-optical * 1.05);
}"#;
const SHADOW_INSERTION_MARKER: &str =
    "float cloud_shadow(vec3 surface_point, vec3 light_direction) {";
const DIRECT_LIGHT_MARKER: &str = "    float direct = n_dot_l * visibility * cloud_light;";
const DIRECT_LIGHT_REPLACEMENT: &str = r#"    float terrain_light = n_dot_l > 0.0
        ? terrain_shadow_visibility(point, geometric, light_direction, land)
        : 1.0;
    float direct = n_dot_l * visibility * cloud_light * terrain_light;"#;

/// Shader compiled synchronously when the visitor enters the Earth installation.
///
/// Keep this intentionally small. Browser/driver GLSL compilation occurs on the UI path on
/// some platforms, so compiling the full near-surface raymarcher here can make the tab appear
/// hung or trip a GPU watchdog before the first frame is drawn.
pub fn fragment_source() -> Result<String, &'static str> {
    Ok(PLANET_ORBITAL_FRAGMENT.to_owned())
}

/// Builds the higher-cost near-surface shader without making it part of installation startup.
///
/// The detailed program stays independently validated while it is split into safe LOD programs.
pub fn detailed_fragment_source() -> Result<String, &'static str> {
    let with_ocean = replace_exactly_once(
        PLANET_FRAGMENT_BASE,
        OCEAN_NORMAL_BASE,
        OCEAN_SURFACE_MODULE.trim_end(),
        "planet shader ocean-normal marker is missing or duplicated",
    )?;
    let with_atmosphere_quality = replace_exactly_once(
        &with_ocean,
        ATMOSPHERE_RAYMARCH_BASE,
        ATMOSPHERE_RAYMARCH_REPLACEMENT,
        "planet shader atmosphere-raymarch marker is missing or duplicated",
    )?;
    let with_cloud_quality = replace_exactly_once(
        &with_atmosphere_quality,
        CLOUD_RAYMARCH_BASE,
        CLOUD_RAYMARCH_REPLACEMENT,
        "planet shader cloud-raymarch marker is missing or duplicated",
    )?;
    let with_cloud_lighting = replace_exactly_once(
        &with_cloud_quality,
        CLOUD_LIGHT_TRANSMISSION_BASE,
        CLOUD_LIGHT_TRANSMISSION_REPLACEMENT,
        "planet shader cloud-light marker is missing or duplicated",
    )?;
    let shadow_insertion = format!("{TERRAIN_SHADOW_MODULE}\n\n{SHADOW_INSERTION_MARKER}");
    let with_shadow = replace_exactly_once(
        &with_cloud_lighting,
        SHADOW_INSERTION_MARKER,
        &shadow_insertion,
        "planet shader terrain-shadow insertion marker is missing or duplicated",
    )?;
    replace_exactly_once(
        &with_shadow,
        DIRECT_LIGHT_MARKER,
        DIRECT_LIGHT_REPLACEMENT,
        "planet shader direct-light marker is missing or duplicated",
    )
}

fn replace_exactly_once(
    source: &str,
    marker: &str,
    replacement: &str,
    error: &'static str,
) -> Result<String, &'static str> {
    let mut matches = source.match_indices(marker);
    if matches.next().is_none() || matches.next().is_some() {
        return Err(error);
    }
    Ok(source.replacen(marker, replacement, 1))
}

#[cfg(test)]
mod tests {
    use super::{detailed_fragment_source, fragment_source, replace_exactly_once};

    #[test]
    fn startup_shader_stays_performance_safe() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("vec3 shade_earth("));
        assert!(shader.contains("float cloud_density_cheap("));
        assert!(shader.contains("vec3 sharpen_regional_surface("));
        assert!(shader.contains("earth_distance <= moon_distance"));
        assert!(!shader.contains("integrate_clouds("));
        assert!(!shader.contains("integrate_atmosphere("));
        assert!(!shader.contains("intersect_terrain("));
        assert!(!shader.contains("terrain_shadow_visibility("));
    }

    #[test]
    fn detailed_shader_contains_one_terrain_shadow_function_and_use_site() {
        let shader = detailed_fragment_source().expect("stable planet shader markers");
        assert_eq!(
            shader.matches("float terrain_shadow_visibility(").count(),
            1
        );
        assert_eq!(shader.matches("* terrain_light;").count(), 1);
        assert!(shader.contains("u_camera_altitude_m > 750000.0"));
    }

    #[test]
    fn detailed_shader_replaces_the_legacy_ocean_normal() {
        let shader = detailed_fragment_source().expect("stable planet shader markers");
        assert_eq!(
            shader
                .matches("float deep_water_angular_frequency(")
                .count(),
            1
        );
        assert_eq!(shader.matches("vec3 ocean_normal(").count(), 1);
        assert!(!shader.contains("float wave_a = sin("));
    }

    #[test]
    fn detailed_shader_limits_atmosphere_work_per_fragment() {
        let shader = detailed_fragment_source().expect("stable planet shader markers");
        assert!(shader.contains("int atmosphere_sample_count"));
        assert!(shader.contains("? 10"));
        assert!(shader.contains("? 8 : 6"));
        assert!(shader.contains("sample_index >= atmosphere_sample_count"));
    }

    #[test]
    fn detailed_shader_limits_cloud_work_per_fragment() {
        let shader = detailed_fragment_source().expect("stable planet shader markers");
        assert!(shader.contains("int cloud_sample_count"));
        assert!(shader.contains("? 8"));
        assert!(shader.contains("? 6 : 4"));
        assert!(shader.contains("sample_index >= cloud_sample_count"));
        assert!(shader.contains("step_index < 2"));
        assert!(shader.contains("sample_jitter"));
        assert!(!shader.contains("step_index < 4; ++step_index) {\n        float distance_along_light"));
    }

    #[test]
    fn exact_replacement_rejects_missing_and_duplicate_markers() {
        assert!(replace_exactly_once("abc", "x", "y", "missing").is_err());
        assert!(replace_exactly_once("x-x", "x", "y", "duplicate").is_err());
        assert_eq!(
            replace_exactly_once("a-x-b", "x", "y", "error"),
            Ok("a-y-b".to_owned())
        );
    }
}
