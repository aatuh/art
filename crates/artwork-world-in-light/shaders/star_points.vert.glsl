#version 300 es
precision highp float;

layout(location = 0) in vec3 a_direction;
layout(location = 1) in vec3 a_color;
layout(location = 2) in float a_size;

uniform vec2 u_resolution;
uniform vec3 u_camera_forward;
uniform vec3 u_camera_right;
uniform vec3 u_camera_up;
uniform float u_pixel_scale;

out vec3 v_color;

const float TAN_HALF_VERTICAL_FOV = 0.7673269879789604;
const float MIN_PIXEL_SCALE = 0.5;
const float MAX_PIXEL_SCALE = 2.0;
const float MIN_POINT_SIZE = 0.72;
const float MAX_POINT_SIZE = 3.38;

void main() {
    vec3 direction = normalize(a_direction);
    vec3 view_direction = vec3(
        dot(direction, u_camera_right),
        dot(direction, u_camera_up),
        dot(direction, u_camera_forward)
    );
    v_color = max(a_color, vec3(0.0));

    if (view_direction.z <= 0.0001) {
        gl_Position = vec4(2.0, 2.0, 1.0, 1.0);
        gl_PointSize = 1.0;
        v_color = vec3(0.0);
        return;
    }

    float aspect = max(u_resolution.x / max(u_resolution.y, 1.0), 0.001);
    vec2 projected = vec2(
        view_direction.x / (view_direction.z * TAN_HALF_VERTICAL_FOV * aspect),
        view_direction.y / (view_direction.z * TAN_HALF_VERTICAL_FOV)
    );
    gl_Position = vec4(projected, 0.0, 1.0);
    gl_PointSize = clamp(a_size, MIN_POINT_SIZE, MAX_POINT_SIZE)
        * clamp(u_pixel_scale, MIN_PIXEL_SCALE, MAX_PIXEL_SCALE);
}
