#version 300 es

in vec2 a_position;
out vec2 v_clip;

void main() {
    v_clip = a_position;
    gl_Position = vec4(a_position, 0.0, 1.0);
}
