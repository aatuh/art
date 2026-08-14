#version 300 es
precision highp float;

in vec2 v_clip;
out vec4 out_color;

uniform vec2 u_resolution;
uniform vec3 u_camera_forward;
uniform vec3 u_camera_right;
uniform vec3 u_camera_up;
uniform vec3 u_earth_center_m;
uniform vec3 u_moon_center_m;
uniform vec3 u_sun_center_m;
uniform float u_earth_equatorial_radius_m;
uniform float u_earth_polar_radius_m;
uniform float u_atmosphere_top_m;
uniform float u_moon_radius_m;
uniform float u_sun_radius_m;
uniform float u_earth_rotation;
uniform float u_camera_altitude_m;
uniform float u_time;
uniform sampler2D u_surface;
uniform sampler2D u_land_mask;
uniform sampler2D u_elevation;
uniform float u_elevation_ready;

const float PI = 3.14159265358979323846;
const float TAU = 6.28318530717958647692;
const float INF = 1.0e30;

float first_positive_root(float near_root, float far_root) {
    if (near_root > 0.0) {
        return near_root;
    }
    if (far_root > 0.0) {
        return far_root;
    }
    return INF;
}

vec2 ray_sphere(vec3 direction, vec3 center, float radius) {
    float b = dot(center, direction);
    float c = dot(center, center) - radius * radius;
    float discriminant = b * b - c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    return vec2(b - root, b + root);
}

float ray_ellipsoid(vec3 direction, vec3 center, vec3 radii) {
    vec3 scaled_origin = -center / radii;
    vec3 scaled_direction = direction / radii;
    float a = dot(scaled_direction, scaled_direction);
    float b = 2.0 * dot(scaled_origin, scaled_direction);
    float c = dot(scaled_origin, scaled_origin) - 1.0;
    float discriminant = b * b - 4.0 * a * c;
    if (discriminant < 0.0) {
        return INF;
    }
    float root = sqrt(discriminant);
    float inverse = 0.5 / a;
    return first_positive_root((-b - root) * inverse, (-b + root) * inverse);
}

vec3 rotate_y(vec3 value, float angle) {
    float sine = sin(angle);
    float cosine = cos(angle);
    return vec3(
        cosine * value.x - sine * value.z,
        value.y,
        sine * value.x + cosine * value.z
    );
}

vec2 earth_uv(vec3 world_direction) {
    vec3 local = normalize(rotate_y(world_direction, -u_earth_rotation));
    float longitude = atan(local.z, local.x);
    float latitude = asin(clamp(local.y, -1.0, 1.0));
    return vec2(fract(longitude / TAU + 0.5), 0.5 - latitude / PI);
}

float cheap_clouds(vec2 uv) {
    float longitude = uv.x * TAU;
    float latitude = (0.5 - uv.y) * PI;
    float drift = u_time * 0.000055;
    float broad = sin(longitude * 5.0 + drift + sin(latitude * 3.0) * 1.7);
    float detail = sin(longitude * 13.0 - drift * 1.6 + latitude * 8.0);
    float belts = cos(latitude * 4.0) * 0.18;
    return smoothstep(0.60, 0.92, broad * 0.28 + detail * 0.15 + belts + 0.56);
}

vec3 shade_earth(vec3 direction, float distance_to_surface) {
    vec3 radii = vec3(
        u_earth_equatorial_radius_m,
        u_earth_polar_radius_m,
        u_earth_equatorial_radius_m
    );
    vec3 point = direction * distance_to_surface;
    vec3 local_point = point - u_earth_center_m;
    vec3 normal = normalize(local_point / (radii * radii));
    vec2 uv = earth_uv(local_point);

    vec3 albedo = pow(max(texture(u_surface, uv).rgb, vec3(0.002)), vec3(2.2));
    float land = texture(u_land_mask, uv).a;
    float elevation = texture(u_elevation, uv).r;
    albedo *= 1.0 + u_elevation_ready * (elevation - 0.5) * 0.035 * land;

    vec3 light_direction = normalize(u_sun_center_m - point);
    float diffuse = max(dot(normal, light_direction), 0.0);
    float night = smoothstep(0.04, -0.14, dot(normal, light_direction));
    vec3 color = albedo * (0.055 + 0.945 * diffuse);
    color += albedo * vec3(0.035, 0.055, 0.09) * night;

    float cloud = cheap_clouds(uv);
    vec3 cloud_color = mix(vec3(0.30, 0.34, 0.39), vec3(1.0, 0.98, 0.94), 0.35 + 0.65 * diffuse);
    color = mix(color, cloud_color, cloud * (0.18 + 0.36 * diffuse));

    float rim = pow(1.0 - max(dot(normal, -direction), 0.0), 3.2);
    float altitude_factor = mix(1.0, 0.72, clamp(u_camera_altitude_m / 4.0e7, 0.0, 1.0));
    color += vec3(0.08, 0.32, 0.75) * rim * 0.48 * altitude_factor;
    return color;
}

vec3 shade_moon(vec3 direction, float distance_to_surface) {
    vec3 point = direction * distance_to_surface;
    vec3 normal = normalize(point - u_moon_center_m);
    vec3 light_direction = normalize(u_sun_center_m - point);
    float diffuse = max(dot(normal, light_direction), 0.0);
    float mottling = 0.90 + 0.10 * sin(normal.x * 37.0 + normal.z * 29.0) * sin(normal.y * 31.0);
    return vec3(0.42, 0.41, 0.39) * mottling * (0.025 + 0.975 * diffuse);
}

vec3 background(vec3 direction) {
    vec3 color = vec3(0.0008, 0.0012, 0.0025);
    vec2 atmosphere_hit = ray_sphere(
        direction,
        u_earth_center_m,
        u_earth_equatorial_radius_m + u_atmosphere_top_m
    );
    if (atmosphere_hit.y > 0.0) {
        vec2 earth_hit = ray_sphere(direction, u_earth_center_m, u_earth_equatorial_radius_m);
        if (earth_hit.y <= 0.0) {
            vec3 closest = direction * max(dot(u_earth_center_m, direction), 0.0);
            float radial_distance = length(closest - u_earth_center_m);
            float shell = 1.0 - smoothstep(
                u_earth_equatorial_radius_m,
                u_earth_equatorial_radius_m + u_atmosphere_top_m,
                radial_distance
            );
            color += vec3(0.05, 0.22, 0.62) * shell * shell * 0.44;
        }
    }
    return color;
}

void main() {
    float aspect = max(u_resolution.x / max(u_resolution.y, 1.0), 0.001);
    const float tangent_half_fov = 0.7673269879789604; // tan(37.5 degrees)
    vec3 direction = normalize(
        u_camera_forward
            + u_camera_right * (v_clip.x * aspect * tangent_half_fov)
            + u_camera_up * (v_clip.y * tangent_half_fov)
    );

    vec3 color = background(direction);
    float nearest = INF;

    vec3 earth_radii = vec3(
        u_earth_equatorial_radius_m,
        u_earth_polar_radius_m,
        u_earth_equatorial_radius_m
    );
    float earth_distance = ray_ellipsoid(direction, u_earth_center_m, earth_radii);
    if (earth_distance < nearest) {
        nearest = earth_distance;
        color = shade_earth(direction, earth_distance);
    }

    vec2 moon_roots = ray_sphere(direction, u_moon_center_m, u_moon_radius_m);
    float moon_distance = first_positive_root(moon_roots.x, moon_roots.y);
    if (moon_distance < nearest) {
        nearest = moon_distance;
        color = shade_moon(direction, moon_distance);
    }

    vec2 sun_roots = ray_sphere(direction, u_sun_center_m, u_sun_radius_m);
    float sun_distance = first_positive_root(sun_roots.x, sun_roots.y);
    if (sun_distance < nearest) {
        color = vec3(7.5, 6.6, 5.2);
    }

    color = color / (vec3(1.0) + color);
    color = pow(max(color, vec3(0.0)), vec3(1.0 / 2.2));
    out_color = vec4(color, 1.0);
}
