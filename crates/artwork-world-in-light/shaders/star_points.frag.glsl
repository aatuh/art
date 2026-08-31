#version 300 es
precision highp float;

in vec3 v_color;
layout(location = 0) out vec4 out_color;

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

void main() {
    vec2 point = gl_PointCoord * 2.0 - 1.0;
    float radius_squared = dot(point, point);
    if (radius_squared > 1.0) {
        discard;
    }

    float edge_width = max(fwidth(radius_squared), 0.08);
    float coverage = 1.0 - smoothstep(1.0 - edge_width, 1.0, radius_squared);
    float core = exp2(-7.0 * radius_squared);
    vec3 hdr_color = v_color * (0.42 + core * 2.6);
    vec3 display_color = linear_to_srgb(aces_tone_map(hdr_color));

    // Premultiplied output: use ONE, ONE_MINUS_SRC_ALPHA blending over the
    // opaque background pass to preserve antialiased star edges.
    out_color = vec4(display_color * coverage, coverage);
}
