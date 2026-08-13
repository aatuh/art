#version 300 es
precision highp float;
precision highp int;

in vec2 v_clip;
out vec4 out_color;

uniform vec2 u_resolution;
uniform float u_time;
uniform float u_fov_y_radians;
uniform float u_camera_altitude_m;
uniform float u_surface_ready;
uniform vec3 u_forward;
uniform vec3 u_right;
uniform vec3 u_up;
uniform vec3 u_earth_center;
uniform vec3 u_moon_center;
uniform vec3 u_sun_center;
uniform vec2 u_earth_radii;
uniform float u_atmosphere_top_m;
uniform float u_moon_radius_m;
uniform float u_sun_radius_m;
uniform float u_earth_rotation_radians;
uniform sampler2D u_surface;

const float PI = 3.14159265358979323846;
const float INF = 1.0e30;
const float RAY_EPSILON_M = 12.0;
const float CLOUD_BASE_M = 1.5e3;
const float CLOUD_TOP_M = 12.0e3;
const float MAX_TERRAIN_M = 8.5e3;
const float RAYLEIGH_SCALE_HEIGHT_M = 8.0e3;
const float MIE_SCALE_HEIGHT_M = 1.2e3;
const vec3 BETA_RAYLEIGH = vec3(5.802e-6, 13.558e-6, 33.100e-6);
const vec3 BETA_MIE_SCATTERING = vec3(3.996e-6);
const vec3 BETA_MIE_EXTINCTION = vec3(4.440e-6);
const vec3 BETA_OZONE_ABSORPTION = vec3(0.650e-6, 1.881e-6, 0.085e-6);
const int ATMOSPHERE_VIEW_STEPS = 12;
const int ATMOSPHERE_LIGHT_STEPS = 4;
const int CLOUD_VIEW_STEPS = 8;
const int CLOUD_LIGHT_STEPS = 3;
const int TERRAIN_STEPS = 10;

float saturate(float value) {
    return clamp(value, 0.0, 1.0);
}

vec3 saturate(vec3 value) {
    return clamp(value, vec3(0.0), vec3(1.0));
}

float hash13(vec3 point) {
    point = fract(point * 0.1031);
    point += dot(point, point.yzx + 33.33);
    return fract((point.x + point.y) * point.z);
}

float valueNoise(vec3 point) {
    vec3 cell = floor(point);
    vec3 blend = fract(point);
    blend = blend * blend * (3.0 - 2.0 * blend);
    float x00 = mix(hash13(cell), hash13(cell + vec3(1.0, 0.0, 0.0)), blend.x);
    float x10 = mix(hash13(cell + vec3(0.0, 1.0, 0.0)), hash13(cell + vec3(1.0, 1.0, 0.0)), blend.x);
    float x01 = mix(hash13(cell + vec3(0.0, 0.0, 1.0)), hash13(cell + vec3(1.0, 0.0, 1.0)), blend.x);
    float x11 = mix(hash13(cell + vec3(0.0, 1.0, 1.0)), hash13(cell + vec3(1.0, 1.0, 1.0)), blend.x);
    return mix(mix(x00, x10, blend.y), mix(x01, x11, blend.y), blend.z);
}

float fbm(vec3 point) {
    float sum = 0.0;
    float amplitude = 0.5;
    for (int octave = 0; octave < 5; ++octave) {
        sum += amplitude * valueNoise(point);
        point = point * 2.031 + vec3(17.1, 9.2, 13.7);
        amplitude *= 0.5;
    }
    return sum;
}

mat2 rotate2d(float angle) {
    float cosine = cos(angle);
    float sine = sin(angle);
    return mat2(cosine, -sine, sine, cosine);
}

vec2 raySphere(vec3 ray_origin, vec3 ray_direction, vec3 center, float radius) {
    vec3 to_center = center - ray_origin;
    float projected = dot(to_center, ray_direction);
    vec3 perpendicular = cross(to_center, ray_direction);
    float perpendicular_sq = dot(perpendicular, perpendicular);
    float half_chord_sq = radius * radius - perpendicular_sq;
    if (half_chord_sq < 0.0) {
        return vec2(INF, -INF);
    }
    float half_chord = sqrt(half_chord_sq);
    return vec2(projected - half_chord, projected + half_chord);
}

vec2 rayEllipsoid(vec3 ray_origin, vec3 ray_direction, vec3 center, vec3 radii) {
    vec3 scaled_origin = (ray_origin - center) / radii;
    vec3 scaled_direction = ray_direction / radii;
    float a = dot(scaled_direction, scaled_direction);
    float b = dot(scaled_origin, scaled_direction);
    float c = dot(scaled_origin, scaled_origin) - 1.0;
    float discriminant = b * b - a * c;
    if (discriminant < 0.0) {
        return vec2(INF, -INF);
    }
    float root = sqrt(discriminant);
    return vec2((-b - root) / a, (-b + root) / a);
}

float nearestPositiveIntersection(vec2 intersection) {
    if (intersection.x > 0.0 && intersection.x < INF * 0.5) {
        return intersection.x;
    }
    if (intersection.y > 0.0 && intersection.y < INF * 0.5) {
        return intersection.y;
    }
    return INF;
}

vec3 earthRadii(float altitude_m) {
    return vec3(
        u_earth_radii.x + altitude_m,
        u_earth_radii.y + altitude_m,
        u_earth_radii.x + altitude_m
    );
}

float surfaceRadius(vec3 direction) {
    vec3 normalized_direction = normalize(direction);
    float equatorial_sq = u_earth_radii.x * u_earth_radii.x;
    float polar_sq = u_earth_radii.y * u_earth_radii.y;
    float inverse_radius_sq =
        (normalized_direction.x * normalized_direction.x + normalized_direction.z * normalized_direction.z) / equatorial_sq
        + normalized_direction.y * normalized_direction.y / polar_sq;
    return inversesqrt(inverse_radius_sq);
}

float altitudeAboveEllipsoid(vec3 point) {
    vec3 local = point - u_earth_center;
    return length(local) - surfaceRadius(local);
}

vec3 ellipsoidNormal(vec3 point, vec3 radii) {
    vec3 local = point - u_earth_center;
    return normalize(local / (radii * radii));
}

vec3 earthFixedDirection(vec3 point) {
    vec3 direction = normalize(point - u_earth_center);
    direction.xz = rotate2d(-u_earth_rotation_radians) * direction.xz;
    return direction;
}

vec2 earthUv(vec3 point) {
    vec3 direction = earthFixedDirection(point);
    float longitude = atan(direction.z, direction.x);
    float latitude = asin(clamp(direction.y, -1.0, 1.0));
    return vec2(fract(longitude / (2.0 * PI) + 0.5), clamp(0.5 - latitude / PI, 0.001, 0.999));
}

float wrappedLongitudeDistance(float longitude, float center) {
    return mod(longitude - center + PI, 2.0 * PI) - PI;
}

float referenceLand(vec2 uv);

