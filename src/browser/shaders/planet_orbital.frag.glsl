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

vec2 spherical_uv(vec3 direction) {
    vec3 normalized_direction = normalize(direction);
    float longitude = atan(normalized_direction.z, normalized_direction.x);
    float latitude = asin(clamp(normalized_direction.y, -1.0, 1.0));
    return vec2(fract(longitude / TAU + 0.5), 0.5 - latitude / PI);
}

vec2 earth_uv(vec3 world_direction) {
    return spherical_uv(rotate_y(world_direction, -u_earth_rotation));
}

float hash21(vec2 value) {
    vec3 p = fract(vec3(value.xyx) * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float hash31(vec3 value) {
    vec3 p = fract(value * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float value_noise(vec3 value) {
    vec3 cell = floor(value);
    vec3 blend = fract(value);
    blend = blend * blend * (3.0 - 2.0 * blend);

    float n000 = hash31(cell + vec3(0.0, 0.0, 0.0));
    float n100 = hash31(cell + vec3(1.0, 0.0, 0.0));
    float n010 = hash31(cell + vec3(0.0, 1.0, 0.0));
    float n110 = hash31(cell + vec3(1.0, 1.0, 0.0));
    float n001 = hash31(cell + vec3(0.0, 0.0, 1.0));
    float n101 = hash31(cell + vec3(1.0, 0.0, 1.0));
    float n011 = hash31(cell + vec3(0.0, 1.0, 1.0));
    float n111 = hash31(cell + vec3(1.0, 1.0, 1.0));

    float lower = mix(
        mix(n000, n100, blend.x),
        mix(n010, n110, blend.x),
        blend.y
    );
    float upper = mix(
        mix(n001, n101, blend.x),
        mix(n011, n111, blend.x),
        blend.y
    );
    return mix(lower, upper, blend.z);
}

float star_layer(vec2 uv, vec2 grid_size, float threshold, float radius, float salt) {
    vec2 grid = uv * grid_size;
    vec2 cell = floor(grid);
    vec2 local = fract(grid) - 0.5;
    float seed = hash21(cell + salt);
    vec2 offset = vec2(
        hash21(cell + vec2(17.1 + salt, 3.7)),
        hash21(cell + vec2(5.9, 29.3 + salt))
    ) - 0.5;
    float distance_to_star = length(local - offset * 0.62);
    float star = (1.0 - smoothstep(0.0, radius, distance_to_star))
        * smoothstep(threshold, 1.0, seed);
    float brightness = mix(0.35, 1.0, pow(hash21(cell + 71.0 + salt), 3.0));
    return star * brightness;
}

vec3 starfield(vec3 direction) {
    vec2 uv = spherical_uv(direction);
    float stars = star_layer(uv, vec2(760.0, 380.0), 0.982, 0.085, 0.0);
    stars += star_layer(uv, vec2(1450.0, 725.0), 0.994, 0.070, 41.0) * 0.72;
    float temperature = hash21(floor(uv * vec2(760.0, 380.0)) + 113.0);
    vec3 tint = mix(vec3(0.72, 0.82, 1.0), vec3(1.0, 0.88, 0.68), temperature);
    return tint * stars;
}

float cloud_density_cheap(vec3 earth_fixed_direction) {
    vec3 moving = rotate_y(normalize(earth_fixed_direction), u_time * 0.000045);
    float broad = value_noise(moving * 3.1 + vec3(4.7, -2.2, 8.1));
    float mesoscale = value_noise(
        moving * 7.3 + vec3(-11.0, 5.0, 3.0) + vec3(u_time * 0.00008, 0.0, 0.0)
    );
    float detail = value_noise(
        moving * 16.0 + vec3(2.0, 9.0, -7.0) + vec3(0.0, 0.0, -u_time * 0.00012)
    );
    float field = broad * 0.62 + mesoscale * 0.28 + detail * 0.10;
    float latitude = abs(earth_fixed_direction.y);
    float polar_breakup = mix(1.0, 0.80, smoothstep(0.70, 0.96, latitude));
    return smoothstep(0.56, 0.70, field) * polar_breakup;
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
    vec3 earth_fixed = normalize(rotate_y(local_point, -u_earth_rotation));
    vec2 uv = spherical_uv(earth_fixed);

    vec3 reference = texture(u_surface, uv).rgb;
    float land = texture(u_land_mask, uv).a;
    float elevation = texture(u_elevation, uv).r;
    reference *= 1.0 + u_elevation_ready * (elevation - 0.5) * 0.025 * land;

    vec3 light_direction = normalize(u_sun_center_m - point);
    float n_dot_l = dot(normal, light_direction);
    float diffuse = max(n_dot_l, 0.0);
    float daylight = pow(diffuse, 0.58);
    vec3 color = reference * mix(0.022, 1.02, daylight);

    float ocean = 1.0 - land;
    vec3 half_vector = normalize(light_direction - direction);
    float ocean_specular = pow(max(dot(normal, half_vector), 0.0), 96.0)
        * ocean
        * smoothstep(0.02, 0.30, diffuse);
    color += vec3(1.0, 0.90, 0.72) * ocean_specular * 0.62;

    float cloud = cloud_density_cheap(earth_fixed);
    float cloud_daylight = mix(0.10, 1.0, pow(diffuse, 0.42));
    vec3 cloud_color = vec3(0.92, 0.95, 0.98) * cloud_daylight;
    float cloud_alpha = cloud * mix(0.18, 0.55, daylight);
    color = mix(color, cloud_color, cloud_alpha);

    float view_facing = max(dot(normal, -direction), 0.0);
    float rim = pow(1.0 - view_facing, 4.0);
    float sunward_rim = mix(0.22, 1.0, smoothstep(-0.15, 0.35, n_dot_l));
    float altitude_factor = mix(1.0, 0.65, clamp(u_camera_altitude_m / 4.0e7, 0.0, 1.0));
    color += vec3(0.08, 0.30, 0.70) * rim * sunward_rim * 0.30 * altitude_factor;
    return color;
}

vec3 shade_moon(vec3 direction, float distance_to_surface) {
    vec3 point = direction * distance_to_surface;
    vec3 normal = normalize(point - u_moon_center_m);
    vec3 light_direction = normalize(u_sun_center_m - point);
    float diffuse = max(dot(normal, light_direction), 0.0);
    float mottling = 0.90 + 0.10 * sin(normal.x * 37.0 + normal.z * 29.0)
        * sin(normal.y * 31.0);
    return vec3(0.47, 0.46, 0.43) * mottling * (0.018 + 0.982 * diffuse);
}

vec3 background(vec3 direction) {
    vec3 color = starfield(direction);
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
            color += vec3(0.035, 0.16, 0.48) * shell * shell * 0.34;
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
        color = vec3(1.0, 0.95, 0.80);
    }

    out_color = vec4(clamp(color, 0.0, 1.0), 1.0);
}
