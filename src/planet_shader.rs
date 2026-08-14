//! Deterministic composition of the planetary fragment shader from small effect modules.

const PLANET_FRAGMENT_BASE: &str = include_str!("browser/shaders/planet_v4.frag.glsl");
const TERRAIN_SHADOW_MODULE: &str = include_str!("browser/shaders/terrain_shadow.glsl");

const SHADOW_INSERTION_MARKER: &str =
    "float cloud_shadow(vec3 surface_point, vec3 light_direction) {";
const DIRECT_LIGHT_MARKER: &str = "    float direct = n_dot_l * visibility * cloud_light;";
const DIRECT_LIGHT_REPLACEMENT: &str = r#"    float terrain_light = n_dot_l > 0.0
        ? terrain_shadow_visibility(point, geometric, light_direction, land)
        : 1.0;
    float direct = n_dot_l * visibility * cloud_light * terrain_light;"#;

/// Builds the fragment shader with terrain cast-shadow support injected at stable markers.
///
/// Keeping the effect in a separate GLSL module lets new physical-lighting effects evolve
/// without turning the already-large planet shader into a merge-sensitive monolith.
pub fn fragment_source() -> Result<String, &'static str> {
    let insertion = format!("{TERRAIN_SHADOW_MODULE}\n\n{SHADOW_INSERTION_MARKER}");
    let with_shadow = replace_exactly_once(
        PLANET_FRAGMENT_BASE,
        SHADOW_INSERTION_MARKER,
        &insertion,
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
    use super::{fragment_source, replace_exactly_once};

    #[test]
    fn composed_shader_contains_one_terrain_shadow_function_and_use_site() {
        let shader = fragment_source().expect("stable planet shader markers");
        assert_eq!(
            shader.matches("float terrain_shadow_visibility(").count(),
            1
        );
        assert_eq!(shader.matches("* terrain_light;").count(), 1);
        assert!(shader.contains("u_camera_altitude_m > 750000.0"));
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
