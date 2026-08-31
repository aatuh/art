//! Deterministic planetary shader selection and detailed-shader composition.

const PLANET_ORBITAL_FRAGMENT: &str = include_str!("../shaders/planet_orbital.frag.glsl");
const PLANET_FRAGMENT_BASE: &str = include_str!("../shaders/planet_v4.frag.glsl");
const OCEAN_SURFACE_MODULE: &str = include_str!("../shaders/ocean_surface.glsl");
const TERRAIN_SHADOW_MODULE: &str = include_str!("../shaders/terrain_shadow.glsl");

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
        assert!(shader.contains("float cloud_density("));
        assert!(shader.contains("void integrate_atmosphere("));
        assert!(shader.contains("float atmosphere_column("));
        assert!(shader.contains("for (int sample_index = 0; sample_index < 12; ++sample_index)"));
        assert!(shader.contains("for (int sample_index = 0; sample_index < 6; ++sample_index)"));
        assert!(!shader.contains("sample_index >= sample_count"));
        assert!(shader.contains("vec3 aces_tone_map("));
        assert!(shader.contains("float solar_visibility("));
        assert!(shader.contains("bool is_finite_hit("));
        assert!(shader.contains("textureSize(u_material, 0)"));
        assert!(shader.contains("is_finite_hit(earth_distance)"));
        assert!(shader.contains("is_finite_hit(moon_distance)"));
        assert!(shader.contains("is_finite_hit(sun_distance)"));
        assert!(shader.contains("vec3 scaled_earth_origin = -u_earth_center_m / earth_radii"));
        assert!(shader.contains("vec3 scaled_earth_ray = ray_direction / earth_radii"));
        assert!(shader.contains("float closest_earth_ray_distance = max("));
        assert!(shader.contains("float closest_terrain_clearance_m ="));
        assert!(shader.contains("float single_limb_pixel_span_m = max("));
        assert!(shader.contains("float limb_pixel_span_m = single_limb_pixel_span_m * 3.0"));
        assert!(shader.contains("float earth_edge_coverage = 1.0"));
        assert!(shader.contains("earth_edge_coverage = 1.0 - smoothstep("));
        assert!(shader.contains("if (edge_incidence < 0.12)"));
        assert!(shader.contains("if (earth_is_frontmost && earth_edge_coverage < 0.999)"));
        assert!(shader.contains("vec3 earth_edge_sky_ray = ray_direction"));
        assert!(shader.contains("float pixels_inside_limb = max("));
        assert!(shader.contains("float sky_terrain_distance = intersect_terrain("));
        assert!(shader.contains("float sky_body_distance = min("));
        assert!(shader.contains("sky_color = shade_earth(earth_edge_sky_ray"));
        assert!(shader.contains("sky_color = shade_moon(earth_edge_sky_ray"));
        assert!(shader.contains("sky_color = shade_sun(earth_edge_sky_ray"));
        assert!(shader.contains("space_background(earth_edge_sky_ray)"));
        assert!(
            !shader.contains("integrate_atmosphere(\n            ray_direction,\n            INF,")
        );
        assert!(shader.contains("vec3 sky_atmosphere_scattering"));
        assert!(shader.contains("if (!has_body && !has_atmosphere)"));
        assert!(shader.contains("float intersect_terrain("));
        assert!(shader.contains("uniform sampler2D u_terrain_height"));
        assert!(
            shader.contains("return refine_terrain_distance(ray_direction, ellipsoid_distance)")
        );
        assert!(shader.contains("for (int refinement = 0; refinement < 4; ++refinement)"));
        assert!(shader.contains("for (int refinement = 0; refinement < 16; ++refinement)"));
        assert!(!shader.contains("for (int refinement = 0; refinement < 8; ++refinement)"));
        assert!(shader.contains("terrain_base_height_m(earth_fixed, 0.0)"));
        assert!(!shader.contains("for (int terrain_step"));
        assert!(!shader.contains("float geometry_weight"));
        assert!(!shader.contains("terrain_shadow_visibility("));
        assert!(!shader.contains("for (int sample_index = 0; sample_index < 16"));
    }

    #[test]
    fn startup_shader_wraps_equirectangular_seams_with_explicit_stable_lod() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("cosine * value.x - sine * value.z"));
        assert!(shader.contains("sine * value.x + cosine * value.z"));
        assert!(shader.contains("vec4 sample_equirect_lod("));
        assert!(!shader.contains("float equirect_seam_weight("));
        assert!(!shader.contains("fract(1.0 - uv.x)"));
        assert!(shader.contains("return textureLod(map, uv, lod)"));
        assert!(shader.contains(
            "float earth_texture_lod(sampler2D map, float angular_footprint, float bias)"
        ));
        assert!(shader.contains("float moon_texture_lod(sampler2D map)"));
        for derivative in ["dFdx(", "dFdy(", "fwidth(", "textureGrad("] {
            assert!(
                !shader.contains(derivative),
                "{derivative} is undefined inside divergent body/material branches"
            );
        }
        for sampler in [
            "u_surface",
            "u_material",
            "u_weather",
            "u_moon_albedo",
            "u_night",
        ] {
            assert!(
                !shader.contains(&format!("texture({sampler},")),
                "{sampler} bypasses seam-safe sampling"
            );
        }
    }

    #[test]
    fn startup_shader_has_bounded_resolvable_surface_detail() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("float resolved_surface_noise("));
        assert!(shader.contains("float procedural_relief_m("));
        assert!(shader.contains("relief_detail = vec2(local_detail, micro_detail)"));
        assert!(shader.contains("float terrain_pattern = saturate("));
        assert!(!shader.contains("80000.0,\n            vec3(17.2, -6.1, 3.8)"));
        assert!(shader.contains("float filter_width = angular_footprint * frequency"));
        assert!(shader.contains("if (filter_width >= 0.55)"));
        assert!(shader.contains("uniform float u_surface_clearance_m"));
        assert!(shader.contains("float terrain_geometry_height_m("));
        assert!(shader.contains("float terrain_surface_lod("));
        assert!(shader.contains("float local_terrain_relief_weight()"));
        assert!(!shader.contains("float terrain_geometry_lod()"));
        assert!(shader.contains("float intersect_terrain("));
        assert!(shader.contains("float approach_weight = 1.0 - smoothstep("));
        assert!(shader.contains("float ground_grain = resolved_surface_noise("));
        assert!(shader.contains("float ground_micro = resolved_surface_noise("));
        assert!(shader.contains("float resolved_ground_texture = smoothstep("));
        assert!(!shader.contains("u_camera_altitude_m < 350000.0"));
        assert!(!shader.contains("u_camera_altitude_m >= 350000.0"));
        assert!(shader.contains("float deep_water_angular_frequency("));
        assert!(shader.contains("float wave_bandlimit("));
        assert!(shader.contains("terrain = terrain_normal("));
        assert!(shader.contains("vec3 water = ocean_normal("));
        assert!(shader.contains("if (n_dot_l > 0.0 && max_component(atmospheric_sun) > 0.0001)"));
        assert!(shader.contains("if (land < 0.99 && n_dot_l > 0.0)"));
        assert!(!shader.contains("fract(uv * vec2(43.0, 21.5)"));
    }

    #[test]
    fn startup_atmosphere_resolves_near_surface_paths_without_a_single_midpoint_tint() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("const int sample_count = 6"));
        assert!(shader.contains(
            "float near_sky_warp = 1.0 - smoothstep(60000.0, 180000.0, u_camera_altitude_m)"
        ));
        assert!(shader.contains("float denser_endpoint_is_start = step("));
        assert!(shader.contains("float start_dense_warp = linear_start * linear_start"));
        assert!(
            shader.contains(
                "float end_dense_warp = 1.0 - (1.0 - linear_start) * (1.0 - linear_start)"
            )
        );
        assert!(shader.contains("float endpoint_density_direction = smoothstep("));
        assert!(
            shader.contains("float directional_warp = near_sky_warp * endpoint_density_direction")
        );
        assert!(shader.contains("float atmosphere_maximum_distance = min("));
        assert!(
            shader.contains("float sample_length = (warped_end - warped_start) * interval_length")
        );
        assert!(shader.contains("float midday_fill = smoothstep(0.05, 0.45, camera_sun_height)"));
        assert!(!shader.contains("sqrt(transmission) * sun_transmittance * eclipse"));
    }

    #[test]
    fn startup_clouds_use_stable_volumes_and_bounded_density_work() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("const float CLOUD_LOW_TOP_M"));
        assert!(shader.contains("const float CIRRUS_BASE_M"));
        assert!(shader.contains("float cloud_phase("));
        assert!(shader.contains("float cloud_fbm3("));
        assert!(shader.contains("float resolved_cloud_noise3("));
        assert!(shader.contains("float cloud_angular_footprint = max("));
        assert!(shader.contains("float orbital_weather_blur = 7.0 * smoothstep("));
        assert!(shader.contains("max(abs(dot(radial, view_ray_direction)), 0.12)"));
        assert!(!shader.contains("float sample_jitter"));
        assert!(!shader.contains("float jitter_strength"));
        assert!(shader.contains("const int sample_count = 12"));
        assert!(!shader.contains("float orbital_structure"));
        assert!(shader.contains("macro_low * 0.92"));
        assert!(!shader.contains("macro_low * shared_occupancy * mix(0.78, 1.10, shared_shape)"));
        assert!(shader.contains("float globe_detail_blend = 1.0"));
        assert!(shader.contains("if (globe_detail_blend <= 0.001)"));
        assert!(shader.contains("if (globe_detail_blend >= 0.999)"));
        assert!(shader.contains("float shared_shape = cloud_fbm3("));
        assert!(shader.contains("float coarse_shape = shared_shape"));
        assert!(shader.contains("float shared_occupancy = 0.0"));
        assert!(shader.contains("float fine_shape = resolved_cloud_noise3("));
        assert!(!shader.contains("cloud_domain * mix(37.0, 1350.0, close_detail)"));
        assert!(!shader.contains("cirrus_domain * mix(137.3, 5500.0, close_detail)"));
        assert!(shader.contains("altitude_agl = altitude - terrain_conservative_height_m("));
        assert!(shader.contains("float resolved_top = mix("));
        assert!(shader.contains("linear_start * linear_start"));
        assert!(shader.contains("bool lower_active = altitude_agl < CLOUD_DEEP_TOP_M"));
        assert!(shader.contains("if (lower_weather.r > 0.12)"));
        assert!(shader.contains("pow(saturate(lower_weather.r), 1.35)"));
        assert!(shader.contains("float tower_height = saturate("));
        assert!(shader.contains("float tower_billow = mix("));
        assert!(shader.contains("float anvil_profile = smoothstep("));
        assert!(shader.contains("float exposed_edge = smoothstep("));
        assert!(shader.contains("bool cirrus_active = altitude_agl > CIRRUS_BASE_M"));
        assert!(shader.contains("if (cirrus_base + 0.03 > 0.60)"));
        assert!(
            shader.contains("float sample_length = (warped_end - warped_start) * interval_length")
        );
        assert!(shader.contains("density * sample_length / 10000.0"));
        assert!(!shader.contains("pow(density, 1.35)"));
        assert!(shader.contains("earth_radii_at_altitude(CLOUD_SHELL_TOP_M)"));
        assert!(shader.contains("float traversal_limit_m = mix(300000.0, 60000.0"));
        assert!(!shader.contains("cloud_base_interval"));
        assert!(shader.contains("float cloud_shadow_density(vec3 point)"));
        assert!(shader.contains("float resolved_shadow_weight = 1.0 - smoothstep("));
        assert!(shader.contains("if (resolved_shadow_weight <= 0.001)"));
        assert!(shader.contains("float broad_lighting_weight = smoothstep("));
        assert!(shader.contains("float detailed_sunward_near = cloud_shadow_density("));
        assert_eq!(
            shader.matches("cloud_density(").count(),
            2,
            "the fixed view march should be the only full cloud-density call site"
        );
        assert!(shader.contains("for (int sample_index = 0; sample_index < 2; ++sample_index)"));
        assert!(!shader.contains("float night_visibility"));
    }

    #[test]
    fn startup_ocean_reflects_the_daylight_sky_without_unfiltered_wave_aliasing() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("float wave_bandlimit("));
        assert!(shader.contains("float deep_water_wave_signal("));
        assert!(shader.contains("ocean_swell_light = saturate("));
        assert!(shader.contains("float whitecap = smoothstep("));
        assert!(shader.contains("float ocean_surface_variation = resolved_surface_noise("));
        assert!(shader.contains("float ocean_fine_variation = resolved_surface_noise("));
        assert!(shader.contains("float resolved_swell_color = 0.0"));
        assert!(shader.contains("resolved_swell_color = 1.0 - smoothstep("));
        assert!(shader.contains("mix(0.5, ocean_swell_light, resolved_swell_color)"));
        assert!(shader.contains("500.0,\n            4000.0,"));
        assert!(shader.contains("vec3 sky_reflection"));
        assert!(shader.contains("float environment_fresnel"));
        assert!(!shader.contains("float ocean_swell = resolved_surface_noise("));
        assert!(shader.contains("* 0.22;"));
    }

    #[test]
    fn startup_night_rendering_rejects_coloured_map_background_and_limits_airglow_to_limb() {
        let shader = fragment_source().expect("static orbital shader");
        assert!(shader.contains("float night_luminance"));
        assert!(shader.contains("float night_chroma"));
        assert!(shader.contains("surface_sun_height < 0.04"));
        assert!(shader.contains("float airglow_limb"));
        assert!(shader.contains("vec3(0.0010, 0.0062, 0.0020)"));
        assert!(!shader.contains("vec3(0.0018, 0.0048, 0.0096)"));
    }

    #[test]
    fn detailed_shader_contains_one_terrain_shadow_function_and_use_site() {
        let shader = detailed_fragment_source().expect("stable planet shader markers");
        assert!(shader.contains("c * p.x - s * p.z"));
        assert!(shader.contains("s * p.x + c * p.z"));
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
        assert!(
            !shader.contains("step_index < 4; ++step_index) {\n        float distance_along_light")
        );
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
