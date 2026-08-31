#version 300 es
precision highp float;

in vec2 v_clip;
layout(location = 0) out vec4 out_color;

uniform vec2 u_resolution;
uniform vec3 u_camera_forward;
uniform vec3 u_camera_right;
uniform vec3 u_camera_up;
uniform vec3 u_sun_center_m;
uniform float u_sun_radius_m;
uniform sampler2D u_starfield;
uniform float u_starfield_ready;

const float TAN_HALF_VERTICAL_FOV = 0.7673269879789604;

vec3 srgb_to_linear(vec3 color) {
    vec3 low = color / 12.92;
    vec3 high = pow((color + 0.055) / 1.055, vec3(2.4));
    return mix(low, high, step(vec3(0.04045), color));
}

vec3 linear_to_srgb(vec3 color) {
    vec3 low = color * 12.92;
    vec3 high = 1.055 * pow(max(color, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
    return mix(low, high, step(vec3(0.0031308), color));
}

vec3 aces_tone_map(vec3 color) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), 0.0, 1.0);
}

vec2 octahedral_uv(vec3 direction) {
    vec3 projected = direction
        / max(abs(direction.x) + abs(direction.y) + abs(direction.z), 0.00001);
    vec2 folded = (1.0 - abs(projected.yx)) * sign(projected.xy);
    vec2 encoded = mix(projected.xy, folded, step(projected.z, 0.0));
    return encoded * 0.5 + 0.5;
}

vec3 view_ray() {
    float aspect = max(u_resolution.x / max(u_resolution.y, 1.0), 0.001);
    return normalize(
        u_camera_forward
            + u_camera_right * (v_clip.x * aspect * TAN_HALF_VERTICAL_FOV)
            + u_camera_up * (v_clip.y * TAN_HALF_VERTICAL_FOV)
    );
}

vec3 solar_disc_and_corona(vec3 ray_direction) {
    float distance_to_sun = length(u_sun_center_m);
    if (distance_to_sun <= 0.0 || u_sun_radius_m <= 0.0) {
        return vec3(0.0);
    }

    vec3 sun_direction = u_sun_center_m / distance_to_sun;
    float forward_facing = step(0.0, dot(ray_direction, sun_direction));
    float sine_separation = length(cross(ray_direction, sun_direction));
    float sine_angular_radius = clamp(u_sun_radius_m / distance_to_sun, 0.0000001, 0.9999);
    float radius_units = sine_separation / sine_angular_radius;
    float edge_width = max(fwidth(radius_units), 0.002);
    float disc = forward_facing
        * (1.0 - smoothstep(1.0 - edge_width, 1.0 + edge_width, radius_units));

    float outside_disc = smoothstep(0.92, 1.08, radius_units);
    // In a normal photographic exposure the K-corona falls away quickly. Keeping
    // this compact prevents a close solar inspection from turning all of space brown.
    float corona_extent = 1.0 - smoothstep(1.0, 4.2, radius_units);
    float corona = forward_facing * outside_disc * corona_extent
        * pow(max(radius_units, 1.0), -1.72);
    float near_bloom = forward_facing
        * (1.0 - smoothstep(1.0, 2.4, radius_units))
        * pow(max(radius_units, 1.0), -0.72);

    return disc * vec3(8.0, 5.2, 2.7)
        + corona * vec3(0.013, 0.006, 0.002)
        + near_bloom * vec3(0.004, 0.0018, 0.0005);
}

void main() {
    vec3 ray_direction = view_ray();
    vec3 color = vec3(0.0);

    if (u_starfield_ready > 0.5) {
        // The map supplies only a dim, large-scale galactic haze. Individual
        // stars are emitted by the point pass, so empty space stays truly black.
        color += srgb_to_linear(texture(u_starfield, octahedral_uv(ray_direction)).rgb) * 0.55;
    }

    color += solar_disc_and_corona(ray_direction);
    out_color = vec4(linear_to_srgb(aces_tone_map(max(color, vec3(0.0)))), 1.0);
}
