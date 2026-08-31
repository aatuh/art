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
uniform float u_surface_clearance_m;
uniform float u_time;
uniform sampler2D u_surface;
uniform sampler2D u_material;
uniform sampler2D u_weather;
uniform sampler2D u_starfield;
uniform sampler2D u_moon_albedo;
uniform sampler2D u_night;
uniform sampler2D u_terrain_height;
uniform float u_material_ready;
uniform float u_weather_ready;
uniform float u_starfield_ready;
uniform float u_moon_albedo_ready;
uniform float u_night_ready;

const float PI = 3.14159265358979323846;
const float TAU = 6.28318530717958647692;
const float INF = 1.0e30;
const float RAYLEIGH_SCALE_HEIGHT_M = 8000.0;
const float MIE_SCALE_HEIGHT_M = 1200.0;
const vec3 BETA_RAYLEIGH = vec3(5.80e-6, 1.356e-5, 3.31e-5);
const float BETA_MIE_EXTINCTION = 4.44e-6;
const float BETA_MIE_SCATTERING = 3.996e-6;
const vec3 BETA_OZONE = vec3(0.650e-6, 1.881e-6, 0.085e-6);
const float MIE_G = 0.80;
const float SURFACE_SUN_RADIANCE = 5.20;
const float ATMOSPHERE_SUN_RADIANCE = 4.8;
const float HEIGHT_FIELD_MAX_M = 10000.0;
const float LOCAL_TERRAIN_RELIEF_MAX_M = 200.0;
const float MAX_TERRAIN_HEIGHT_M = HEIGHT_FIELD_MAX_M + LOCAL_TERRAIN_RELIEF_MAX_M;
const float TAN_HALF_VERTICAL_FOV = 0.7673269879789604;
const float CLOUD_BASE_M = 1500.0;
const float CLOUD_LOW_TOP_M = 4200.0;
const float CLOUD_DEEP_TOP_M = 12500.0;
const float CIRRUS_BASE_M = 6500.0;
const float CLOUD_TOP_M = 15000.0;
const float CLOUD_SHELL_TOP_M = MAX_TERRAIN_HEIGHT_M + CLOUD_TOP_M;

float saturate(float value) {
    return clamp(value, 0.0, 1.0);
}

float max_component(vec3 value) {
    return max(value.x, max(value.y, value.z));
}

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

float first_positive_root(float near_root, float far_root) {
    if (near_root > 0.0) {
        return near_root;
    }
    if (far_root > 0.0) {
        return far_root;
    }
    return INF;
}

bool is_finite_hit(float distance_to_surface) {
    return distance_to_surface > 0.0 && distance_to_surface < INF * 0.5;
}

vec2 ray_sphere_from(vec3 origin, vec3 direction, vec3 center, float radius) {
    // Solve in sphere-radius units. Camera-relative Sun and Moon coordinates are
    // hundreds of radii away; squaring raw metre values loses enough precision
    // in WebGL's 32-bit floats to invent broad rectangular false intersections.
    float safe_radius = max(radius, 1.0);
    vec3 relative = (origin - center) / safe_radius;
    float b = dot(relative, direction);
    float c = dot(relative, relative) - 1.0;
    float discriminant = b * b - c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    return vec2(-b - root, -b + root) * safe_radius;
}

vec2 ray_sphere(vec3 direction, vec3 center, float radius) {
    return ray_sphere_from(vec3(0.0), direction, center, radius);
}

vec2 ray_ellipsoid_from(vec3 origin, vec3 direction, vec3 center, vec3 radii) {
    vec3 scaled_origin = (origin - center) / radii;
    vec3 scaled_direction = direction / radii;
    float a = dot(scaled_direction, scaled_direction);
    float b = 2.0 * dot(scaled_origin, scaled_direction);
    float c = dot(scaled_origin, scaled_origin) - 1.0;
    float discriminant = b * b - 4.0 * a * c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    float inverse = 0.5 / a;
    return vec2((-b - root) * inverse, (-b + root) * inverse);
}

float ray_ellipsoid(vec3 direction, vec3 center, vec3 radii) {
    vec2 interval = ray_ellipsoid_from(vec3(0.0), direction, center, radii);
    return first_positive_root(interval.x, interval.y);
}

vec3 earth_radii_at_altitude(float altitude_m) {
    return vec3(
        u_earth_equatorial_radius_m + altitude_m,
        u_earth_polar_radius_m + altitude_m,
        u_earth_equatorial_radius_m + altitude_m
    );
}

float ellipsoid_surface_radius(vec3 direction) {
    vec3 normalized_direction = normalize(direction);
    float equatorial_squared = u_earth_equatorial_radius_m
        * u_earth_equatorial_radius_m;
    float polar_squared = u_earth_polar_radius_m * u_earth_polar_radius_m;
    float inverse_radius_squared = (
        normalized_direction.x * normalized_direction.x
            + normalized_direction.z * normalized_direction.z
    ) / equatorial_squared
        + normalized_direction.y * normalized_direction.y / polar_squared;
    return inversesqrt(inverse_radius_squared);
}

float altitude_above_ellipsoid(vec3 point) {
    vec3 local = point - u_earth_center_m;
    return length(local) - ellipsoid_surface_radius(local);
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

float vertical_pixel_angle() {
    return 2.0 * TAN_HALF_VERTICAL_FOV / max(u_resolution.y, 1.0);
}

float earth_surface_angular_footprint(float hit_distance_m, float view_incidence) {
    float metres_per_pixel = max(hit_distance_m, 1.0)
        * vertical_pixel_angle()
        / max(view_incidence, 0.08);
    return max(metres_per_pixel / max(u_earth_equatorial_radius_m, 1.0), 1.0e-9);
}

float earth_texture_lod(sampler2D map, float angular_footprint, float bias) {
    float texels_per_radian = float(textureSize(map, 0).x) / TAU;
    float footprint_texels = angular_footprint * texels_per_radian;
    float maximum_lod = log2(max(float(textureSize(map, 0).x), 1.0));
    return clamp(log2(max(footprint_texels, 1.0)) + bias, 0.0, maximum_lod);
}

float moon_texture_lod(sampler2D map) {
    float distance_to_surface = max(length(u_moon_center_m) - u_moon_radius_m, 1.0);
    float angular_footprint = distance_to_surface * vertical_pixel_angle()
        / max(u_moon_radius_m, 1.0);
    float texels_per_radian = float(textureSize(map, 0).x) / TAU;
    float maximum_lod = log2(max(float(textureSize(map, 0).x), 1.0));
    return clamp(log2(max(angular_footprint * texels_per_radian, 1.0)), 0.0, maximum_lod);
}

vec4 sample_equirect_lod(sampler2D map, vec2 uv, float lod) {
    // Every equirectangular texture is uploaded with REPEAT in longitude. Sampling
    // the wrapped coordinate therefore filters across the dateline already. A
    // mirrored "opposite" lookup would fold several texels together and create the
    // very vertical line this helper is intended to avoid.
    uv = vec2(fract(uv.x), clamp(uv.y, 0.0, 1.0));
    return textureLod(map, uv, lod);
}

vec2 octahedral_uv(vec3 direction) {
    vec3 projected = direction
        / max(abs(direction.x) + abs(direction.y) + abs(direction.z), 0.00001);
    vec2 folded = (1.0 - abs(projected.yx)) * sign(projected.xy);
    vec2 encoded = mix(projected.xy, folded, step(projected.z, 0.0));
    return encoded * 0.5 + 0.5;
}

vec3 earth_fixed_direction(vec3 point) {
    return normalize(rotate_y(point - u_earth_center_m, -u_earth_rotation));
}

float hash31(vec3 value) {
    vec3 p = fract(value * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float value_noise3(vec3 value) {
    vec3 cell = floor(value);
    vec3 blend = fract(value);
    blend = blend * blend * blend * (blend * (blend * 6.0 - 15.0) + 10.0);
    float n000 = hash31(cell);
    float n100 = hash31(cell + vec3(1.0, 0.0, 0.0));
    float n010 = hash31(cell + vec3(0.0, 1.0, 0.0));
    float n110 = hash31(cell + vec3(1.0, 1.0, 0.0));
    float n001 = hash31(cell + vec3(0.0, 0.0, 1.0));
    float n101 = hash31(cell + vec3(1.0, 0.0, 1.0));
    float n011 = hash31(cell + vec3(0.0, 1.0, 1.0));
    float n111 = hash31(cell + vec3(1.0, 1.0, 1.0));
    float lower = mix(mix(n000, n100, blend.x), mix(n010, n110, blend.x), blend.y);
    float upper = mix(mix(n001, n101, blend.x), mix(n011, n111, blend.x), blend.y);
    return mix(lower, upper, blend.z);
}

float resolved_cloud_noise3(
    vec3 value,
    float angular_frequency,
    float angular_footprint
) {
    float resolve_weight = 1.0 - smoothstep(
        0.30,
        0.72,
        angular_frequency * angular_footprint
    );
    if (resolve_weight <= 0.001) {
        return 0.5;
    }
    return mix(0.5, value_noise3(value), resolve_weight);
}

float cloud_fbm3(vec3 value, float base_frequency, float angular_footprint) {
    float result = resolved_cloud_noise3(
        value,
        base_frequency,
        angular_footprint
    ) * 0.56;
    result += resolved_cloud_noise3(
        value * 2.03 + vec3(7.1, -3.7, 5.3),
        base_frequency * 2.03,
        angular_footprint
    ) * 0.29;
    result += resolved_cloud_noise3(
        value * 4.11 + vec3(-4.6, 8.2, 1.9),
        base_frequency * 4.11,
        angular_footprint
    ) * 0.15;
    return result;
}

float moon_fbm(vec3 value) {
    float result = value_noise3(value) * 0.55;
    result += value_noise3(value * 2.03 + 7.1) * 0.28;
    result += value_noise3(value * 4.09 - 3.7) * 0.17;
    return result;
}

float solar_visibility(vec3 point, vec3 blocker_center, float blocker_radius) {
    vec3 to_sun = u_sun_center_m - point;
    vec3 to_blocker = blocker_center - point;
    float sun_distance = length(to_sun);
    float blocker_distance = length(to_blocker);
    if (blocker_distance <= blocker_radius || blocker_distance >= sun_distance) {
        return 1.0;
    }
    float sun_angle = clamp(u_sun_radius_m / sun_distance, 0.0, 1.0);
    float blocker_angle = clamp(blocker_radius / blocker_distance, 0.0, 1.0);
    float separation = length(cross(to_sun / sun_distance, to_blocker / blocker_distance));
    float outer = sun_angle + blocker_angle;
    float inner = max(blocker_angle - sun_angle, 0.0);
    return smoothstep(inner, outer, separation);
}

float ozone_density(float altitude_m) {
    return saturate(1.0 - abs(altitude_m - 25000.0) / 15000.0);
}

vec3 sunlight_transmittance(vec3 point, vec3 light_direction) {
    vec3 radial = normalize(point - u_earth_center_m);
    float height = max(altitude_above_ellipsoid(point), 0.0);
    float local_radius = max(ellipsoid_surface_radius(radial), 1.0);
    float horizon_cosine = -sqrt(clamp(
        1.0 - local_radius * local_radius
            / ((local_radius + height) * (local_radius + height)),
        0.0,
        1.0
    ));
    float sun_angular_radius = clamp(
        u_sun_radius_m / max(length(u_sun_center_m - point), 1.0),
        0.00001,
        0.25
    );
    float cosine_zenith = dot(radial, light_direction);
    float solar_limb_visibility = smoothstep(
        horizon_cosine - sun_angular_radius,
        horizon_cosine + sun_angular_radius,
        cosine_zenith
    );
    if (solar_limb_visibility <= 0.0) {
        return vec3(0.0);
    }
    float rayleigh_density = exp(-height / RAYLEIGH_SCALE_HEIGHT_M);
    float mie_density = exp(-height / MIE_SCALE_HEIGHT_M);
    float air_mass = 1.0 / (max(cosine_zenith, 0.0) + 0.055);
    vec3 optical_depth = BETA_RAYLEIGH
        * (rayleigh_density * RAYLEIGH_SCALE_HEIGHT_M * air_mass);
    optical_depth += vec3(
        BETA_MIE_EXTINCTION * mie_density * MIE_SCALE_HEIGHT_M * air_mass
    );
    optical_depth += BETA_OZONE * (ozone_density(height) * 30000.0 * air_mass);
    return exp(-optical_depth) * solar_limb_visibility;
}

float atmosphere_half_column(
    vec3 ray_direction,
    float closest_distance,
    float endpoint_distance,
    float scale_height
) {
    float segment_length = abs(endpoint_distance - closest_distance);
    if (segment_length <= 0.01) {
        return 0.0;
    }
    vec3 closest_point = ray_direction * closest_distance;
    vec3 endpoint = ray_direction * endpoint_distance;
    float closest_radius = max(length(closest_point - u_earth_center_m), 1.0);
    float closest_height = max(closest_radius - u_earth_equatorial_radius_m, 0.0);
    float endpoint_height = max(
        length(endpoint - u_earth_center_m) - u_earth_equatorial_radius_m,
        closest_height
    );
    float density = exp(-closest_height / scale_height);
    vec3 radial = (closest_point - u_earth_center_m) / closest_radius;
    float radial_cosine = abs(dot(radial, ray_direction));
    float curvature = sqrt(
        radial_cosine * radial_cosine + 2.0 * scale_height / closest_radius
    );
    float tangent_weight = exp(-radial_cosine * 28.0);
    float curvature_correction = mix(1.0, sqrt(PI), tangent_weight);
    float finite_extent = 1.0 - exp(
        -max(endpoint_height - closest_height, 0.0) / scale_height
    );
    float column = density * scale_height / max(curvature, 0.0001)
        * curvature_correction * finite_extent;
    return min(column, density * segment_length);
}

float atmosphere_column(
    vec3 ray_direction,
    float start_distance,
    float end_distance,
    float scale_height
) {
    float closest_distance = clamp(
        dot(u_earth_center_m, ray_direction),
        start_distance,
        end_distance
    );
    return atmosphere_half_column(
        ray_direction,
        closest_distance,
        start_distance,
        scale_height
    ) + atmosphere_half_column(
        ray_direction,
        closest_distance,
        end_distance,
        scale_height
    );
}

void integrate_atmosphere(
    vec3 ray_direction,
    float maximum_distance,
    out vec3 scattering,
    out vec3 transmission
) {
    vec2 interval = ray_ellipsoid_from(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        earth_radii_at_altitude(u_atmosphere_top_m)
    );
    float start_distance = max(interval.x, 0.0);
    float end_distance = min(interval.y, maximum_distance);
    scattering = vec3(0.0);
    transmission = vec3(1.0);
    if (interval.y <= 0.0 || end_distance <= start_distance) {
        return;
    }

    vec3 view_optical_depth = vec3(0.0);
    vec3 rayleigh_sum = vec3(0.0);
    vec3 mie_sum = vec3(0.0);
    vec3 midpoint = ray_direction * (0.5 * (start_distance + end_distance));
    vec3 midpoint_light_direction = normalize(u_sun_center_m - midpoint);

    const int sample_count = 6;
    float interval_length = end_distance - start_distance;
    float near_sky_warp = 1.0 - smoothstep(60000.0, 180000.0, u_camera_altitude_m);
    float start_altitude = altitude_above_ellipsoid(ray_direction * start_distance);
    float end_altitude = altitude_above_ellipsoid(ray_direction * end_distance);
    float denser_endpoint_is_start = step(start_altitude, end_altitude);
    float endpoint_density_direction = smoothstep(
        250.0,
        2000.0,
        abs(end_altitude - start_altitude)
    );
    for (int sample_index = 0; sample_index < 6; ++sample_index) {
        float linear_start = float(sample_index) / float(sample_count);
        float linear_end = float(sample_index + 1) / float(sample_count);
        float start_dense_warp = linear_start * linear_start;
        float end_dense_warp = 1.0 - (1.0 - linear_start) * (1.0 - linear_start);
        float warped_start = mix(
            end_dense_warp,
            start_dense_warp,
            denser_endpoint_is_start
        );
        float start_dense_end_warp = linear_end * linear_end;
        float end_dense_end_warp = 1.0 - (1.0 - linear_end) * (1.0 - linear_end);
        float warped_end = mix(
            end_dense_end_warp,
            start_dense_end_warp,
            denser_endpoint_is_start
        );
        float directional_warp = near_sky_warp * endpoint_density_direction;
        warped_start = mix(linear_start, warped_start, directional_warp);
        warped_end = mix(linear_end, warped_end, directional_warp);
        float sample_length = (warped_end - warped_start) * interval_length;
        float distance_along_ray = start_distance
            + 0.5 * (warped_start + warped_end) * interval_length;
        vec3 point = ray_direction * distance_along_ray;
        float altitude = max(altitude_above_ellipsoid(point), 0.0);
        float rayleigh_density = exp(-altitude / RAYLEIGH_SCALE_HEIGHT_M);
        float mie_density = exp(-altitude / MIE_SCALE_HEIGHT_M);
        float local_ozone_density = ozone_density(altitude);
        vec3 sample_extinction = BETA_RAYLEIGH * rayleigh_density
            + vec3(BETA_MIE_EXTINCTION * mie_density)
            + BETA_OZONE * local_ozone_density;
        view_optical_depth += sample_extinction * (0.5 * sample_length);

        vec3 light_direction = normalize(u_sun_center_m - point);
        vec3 sun_transmittance = sunlight_transmittance(point, light_direction);
        float eclipse = solar_visibility(point, u_moon_center_m, u_moon_radius_m);
        vec3 attenuation = exp(-view_optical_depth) * sun_transmittance * eclipse;
        rayleigh_sum += attenuation * (rayleigh_density * sample_length);
        mie_sum += attenuation * (mie_density * sample_length);

        view_optical_depth += sample_extinction * (0.5 * sample_length);
    }
    transmission = exp(-view_optical_depth);
    float view_light_cosine = dot(ray_direction, midpoint_light_direction);
    float rayleigh_phase = 3.0 * (1.0 + view_light_cosine * view_light_cosine) / (16.0 * PI);
    float mie_denominator = max(
        1.0 + MIE_G * MIE_G - 2.0 * MIE_G * view_light_cosine,
        0.001
    );
    float mie_phase = 3.0 / (8.0 * PI)
        * ((1.0 - MIE_G * MIE_G) / (2.0 + MIE_G * MIE_G))
        * (1.0 + view_light_cosine * view_light_cosine)
        / pow(mie_denominator, 1.5);
    scattering = (
        BETA_RAYLEIGH * rayleigh_sum * rayleigh_phase
            + vec3(BETA_MIE_SCATTERING) * mie_sum * mie_phase
    ) * ATMOSPHERE_SUN_RADIANCE;
    // Bounded multiple-scattering fill keeps a daytime tangent path blue-white.
    // A single-scattering-only approximation otherwise removes too much blue
    // sunlight and paints the whole horizon like sunset while the Sun is high.
    vec3 camera_radial = normalize(-u_earth_center_m);
    float camera_sun_height = dot(camera_radial, normalize(u_sun_center_m));
    float midday_fill = smoothstep(0.05, 0.45, camera_sun_height);
    float horizon_view = pow(
        1.0 - abs(dot(camera_radial, ray_direction)),
        2.0
    );
    float atmosphere_column_strength = max_component(vec3(1.0) - transmission);
    scattering += vec3(0.012, 0.030, 0.060)
        * atmosphere_column_strength
        * midday_fill
        * horizon_view;
    float night_side = 1.0 - smoothstep(
        -0.16,
        0.04,
        dot(normalize(midpoint - u_earth_center_m), midpoint_light_direction)
    );
    vec3 airglow_column = vec3(1.0) - exp(-view_optical_depth * 3.0);
    vec3 midpoint_radial = normalize(midpoint - u_earth_center_m);
    float airglow_limb = smoothstep(
        0.72,
        0.98,
        1.0 - abs(dot(midpoint_radial, ray_direction))
    );
    scattering += vec3(0.0010, 0.0062, 0.0020)
        * airglow_column
        * night_side
        * airglow_limb;
}

vec4 material_sample(vec2 uv, float angular_footprint) {
    vec4 sampled = sample_equirect_lod(
        u_material,
        uv,
        earth_texture_lod(u_material, angular_footprint, 0.15)
    );
    vec4 normalized_fallback = vec4(sampled.a, 0.5, 0.78, 0.0);
    return mix(normalized_fallback, sampled, step(0.5, u_material_ready));
}

float decoded_elevation_m(float encoded) {
    return (encoded - 0.5) * 20000.0;
}

vec4 material_sample_lod(vec2 uv, float lod) {
    vec4 sampled = sample_equirect_lod(u_material, uv, lod);
    vec4 normalized_fallback = vec4(sampled.a, 0.5, 0.78, 0.0);
    return mix(normalized_fallback, sampled, step(0.5, u_material_ready));
}

float terrain_base_height_m(vec3 earth_fixed, float lod) {
    vec2 uv = spherical_uv(earth_fixed);
    return textureLod(
        u_terrain_height,
        vec2(fract(uv.x), clamp(uv.y, 0.0, 1.0)),
        lod
    ).r * HEIGHT_FIELD_MAX_M;
}

float local_terrain_relief_capacity_m(float base_height_m) {
    return LOCAL_TERRAIN_RELIEF_MAX_M * smoothstep(0.0, 250.0, base_height_m);
}

float local_terrain_relief_m(vec3 earth_fixed, float base_height_m) {
    float relief_capacity_m = local_terrain_relief_capacity_m(base_height_m);
    if (relief_capacity_m <= 0.01) {
        return 0.0;
    }
    vec3 domain = mat3(
        vec3(0.36, -0.48, 0.80),
        vec3(0.80, 0.60, 0.00),
        vec3(-0.48, 0.64, 0.60)
    ) * normalize(earth_fixed);
    // Kilometre-scale hills remain legible from the inspection preset while the
    // total geometric envelope stays capped at 200 m for stable navigation.
    float rolling = value_noise3(domain * 2800.0 + vec3(4.1, -8.7, 2.3));
    float ridges = 1.0 - abs(
        value_noise3(domain * 9000.0 + vec3(-9.3, 3.4, 12.8)) * 2.0 - 1.0
    );
    ridges *= ridges;
    return relief_capacity_m * (rolling * 0.72 + ridges * 0.28);
}

float terrain_conservative_height_m(vec3 earth_fixed) {
    float base_height_m = terrain_base_height_m(earth_fixed, 0.0);
    return base_height_m + local_terrain_relief_capacity_m(base_height_m);
}

float local_terrain_relief_weight() {
    // Distance changes only the amplitude of the fixed local field. It never
    // changes that field's coordinate frequency or the base height-map mip.
    return 1.0 - smoothstep(100000.0, 300000.0, u_surface_clearance_m);
}

float terrain_surface_lod(float hit_distance_m, float view_incidence) {
    float angular_footprint = hit_distance_m
        * vertical_pixel_angle()
        / max(u_earth_equatorial_radius_m * max(view_incidence, 0.12), 1.0);
    return min(
        earth_texture_lod(u_terrain_height, angular_footprint, 0.0),
        2.0
    );
}

float terrain_geometry_height_m(vec3 earth_fixed, float base_lod) {
    float base_height_m = terrain_base_height_m(earth_fixed, base_lod);
    float relief_weight = local_terrain_relief_weight();
    if (relief_weight <= 0.001) {
        return base_height_m;
    }
    float relief_base_height_m = base_lod <= 0.001
        ? base_height_m
        : terrain_base_height_m(earth_fixed, 0.0);
    return base_height_m
        + local_terrain_relief_m(earth_fixed, relief_base_height_m) * relief_weight;
}

float refine_terrain_distance(vec3 ray_direction, float ellipsoid_distance) {
    float refined_distance = ellipsoid_distance;
    for (int refinement = 0; refinement < 4; ++refinement) {
        vec3 point = ray_direction * refined_distance;
        vec3 radial = normalize(point - u_earth_center_m);
        vec3 earth_fixed = earth_fixed_direction(point);
        float incidence = max(-dot(ray_direction, radial), 0.0);
        float base_lod = terrain_surface_lod(refined_distance, incidence);
        float target_height_m = terrain_geometry_height_m(earth_fixed, base_lod);
        float clearance = altitude_above_ellipsoid(point) - target_height_m;
        float correction = clearance / max(incidence, 0.12);
        refined_distance = max(
            refined_distance + clamp(correction, -20000.0, 20000.0),
            0.0
        );
    }
    return refined_distance;
}

float terrain_silhouette_signed_clearance_m(vec3 point) {
    vec3 earth_fixed = earth_fixed_direction(point);
    // Keep the silhouette on the same source field as close rendered terrain and
    // CPU collision wherever individual source texels are resolvable. Only whole-
    // Earth minification blends into at most mip 2, avoiding distant shimmer
    // without flattening close spatial relief.
    float tangent_angular_footprint = length(point)
        * vertical_pixel_angle()
        / max(u_earth_equatorial_radius_m, 1.0);
    float silhouette_lod = min(
        earth_texture_lod(u_terrain_height, tangent_angular_footprint, 0.0),
        2.0
    );
    return altitude_above_ellipsoid(point)
        - terrain_geometry_height_m(earth_fixed, silhouette_lod);
}

float intersect_terrain(vec3 ray_direction, float ellipsoid_distance) {
    if (is_finite_hit(ellipsoid_distance)) {
        vec3 ellipsoid_point = ray_direction * ellipsoid_distance;
        vec3 ellipsoid_radial = normalize(ellipsoid_point - u_earth_center_m);
        float incidence = max(-dot(ray_direction, ellipsoid_radial), 0.0);
        if (incidence < 0.12) {
            // At the limb, Newton-style distance correction is ill-conditioned.
            // Bracket the actual displaced surface between the conservative outer
            // shell and the reference ellipsoid, then take a fixed stable bisection.
            vec2 outer_interval = ray_ellipsoid_from(
                vec3(0.0),
                ray_direction,
                u_earth_center_m,
                earth_radii_at_altitude(MAX_TERRAIN_HEIGHT_M)
            );
            float low = max(outer_interval.x, 0.0);
            float high = ellipsoid_distance;
            for (int refinement = 0; refinement < 16; ++refinement) {
                float middle = 0.5 * (low + high);
                if (terrain_silhouette_signed_clearance_m(ray_direction * middle) <= 0.0) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return high;
        }
        return refine_terrain_distance(ray_direction, ellipsoid_distance);
    }

    // Rays immediately above the reference horizon can still meet elevated
    // terrain. Test the closest point in the bounded 10.2 km shell, then bisect
    // only that thin silhouette band; ordinary sky rays take one height lookup.
    vec2 outer_interval = ray_ellipsoid_from(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        earth_radii_at_altitude(MAX_TERRAIN_HEIGHT_M)
    );
    if (outer_interval.y <= 0.0) {
        return ellipsoid_distance;
    }
    float low = max(outer_interval.x, 0.0);
    float high = 0.5 * (max(outer_interval.x, 0.0) + outer_interval.y);
    if (high <= low || terrain_silhouette_signed_clearance_m(ray_direction * high) > 0.0) {
        return ellipsoid_distance;
    }
    for (int refinement = 0; refinement < 16; ++refinement) {
        float middle = 0.5 * (low + high);
        if (terrain_silhouette_signed_clearance_m(ray_direction * middle) <= 0.0) {
            high = middle;
        } else {
            low = middle;
        }
    }
    return high;
}

vec3 terrain_normal(
    vec3 geometric,
    vec3 earth_fixed,
    vec2 uv,
    float land,
    float angular_footprint
) {
    if (u_material_ready < 0.5 || land < 0.01) {
        return geometric;
    }
    vec2 texture_dimensions = vec2(textureSize(u_material, 0));
    vec2 texel = 1.0 / texture_dimensions;
    float material_lod = earth_texture_lod(u_material, angular_footprint, 0.0);
    float west = decoded_elevation_m(
        sample_equirect_lod(u_material, uv - vec2(texel.x, 0.0), material_lod).g
    );
    float east_height = decoded_elevation_m(
        sample_equirect_lod(u_material, uv + vec2(texel.x, 0.0), material_lod).g
    );
    float north_height = decoded_elevation_m(
        sample_equirect_lod(u_material, uv - vec2(0.0, texel.y), material_lod).g
    );
    float south = decoded_elevation_m(
        sample_equirect_lod(u_material, uv + vec2(0.0, texel.y), material_lod).g
    );
    vec3 east = normalize(cross(
        abs(geometric.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0),
        geometric
    ));
    vec3 north = normalize(cross(geometric, east));
    float latitude_scale = max(sqrt(max(1.0 - earth_fixed.y * earth_fixed.y, 0.0)), 0.08);
    float metres_east = TAU * u_earth_equatorial_radius_m * latitude_scale * texel.x;
    float metres_north = PI * u_earth_equatorial_radius_m * texel.y;
    float relief_scale = mix(
        1.75,
        1.15,
        smoothstep(100000.0, 3000000.0, u_camera_altitude_m)
    );
    float east_slope = (east_height - west) / max(2.0 * metres_east, 1.0);
    float north_slope = (north_height - south) / max(2.0 * metres_north, 1.0);
    vec3 resolved_normal = normalize(
        geometric - (east * east_slope + north * north_slope) * relief_scale * land
    );
    if (angular_footprint >= 0.00012) {
        return resolved_normal;
    }

    // Match the actual 0–200 m displaced field with finite tangent samples.
    // The physical offset is bounded to remain useful at ground scale without
    // evaluating unresolved procedural slopes from orbit.
    vec3 fixed_east = normalize(cross(
        abs(earth_fixed.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0),
        earth_fixed
    ));
    vec3 fixed_north = normalize(cross(earth_fixed, fixed_east));
    float relief_step = clamp(angular_footprint * 2.0, 0.00004, 0.00020);
    vec3 fixed_west_point = normalize(earth_fixed - fixed_east * relief_step);
    vec3 fixed_east_point = normalize(earth_fixed + fixed_east * relief_step);
    vec3 fixed_north_point = normalize(earth_fixed + fixed_north * relief_step);
    vec3 fixed_south_point = normalize(earth_fixed - fixed_north * relief_step);
    float local_west = local_terrain_relief_m(
        fixed_west_point,
        terrain_base_height_m(fixed_west_point, 0.0)
    );
    float local_east = local_terrain_relief_m(
        fixed_east_point,
        terrain_base_height_m(fixed_east_point, 0.0)
    );
    float local_north = local_terrain_relief_m(
        fixed_north_point,
        terrain_base_height_m(fixed_north_point, 0.0)
    );
    float local_south = local_terrain_relief_m(
        fixed_south_point,
        terrain_base_height_m(fixed_south_point, 0.0)
    );
    float local_east_slope = (local_east - local_west)
        / max(2.0 * relief_step * u_earth_equatorial_radius_m, 1.0);
    float local_north_slope = (local_north - local_south)
        / max(2.0 * relief_step * u_earth_equatorial_radius_m, 1.0);
    vec3 world_fixed_east = rotate_y(fixed_east, u_earth_rotation);
    vec3 world_fixed_north = rotate_y(fixed_north, u_earth_rotation);
    return normalize(
        resolved_normal
            - world_fixed_east * local_east_slope
            - world_fixed_north * local_north_slope
    );
}

float resolved_surface_noise(
    vec3 earth_fixed,
    float frequency,
    vec3 offset,
    float angular_footprint
) {
    float filter_width = angular_footprint * frequency;
    if (filter_width >= 0.55) {
        return 0.5;
    }
    float resolvable = 1.0 - smoothstep(0.12, 0.55, filter_width);
    vec3 domain = mat3(
        vec3(0.36, -0.48, 0.80),
        vec3(0.80, 0.60, 0.00),
        vec3(-0.48, 0.64, 0.60)
    ) * earth_fixed;
    return mix(0.5, value_noise3(domain * frequency + offset), resolvable);
}

float procedural_relief_m(
    vec3 earth_fixed,
    float land,
    float angular_footprint,
    out vec2 relief_detail
) {
    relief_detail = vec2(0.5);
    if (land < 0.01) {
        return 0.0;
    }
    float approach_weight = 1.0 - smoothstep(
        250000.0,
        600000.0,
        u_surface_clearance_m
    );
    float regional_weight = 1.0 - smoothstep(
        70000.0,
        260000.0,
        u_surface_clearance_m
    );
    float local_weight = 1.0 - smoothstep(
        10000.0,
        90000.0,
        u_surface_clearance_m
    );
    float micro_weight = 1.0 - smoothstep(
        1500.0,
        18000.0,
        u_surface_clearance_m
    );
    float approach_detail = resolved_surface_noise(
        earth_fixed,
        900.0,
        vec3(4.1, -8.7, 2.3),
        angular_footprint
    );
    float regional_detail = resolved_surface_noise(
        earth_fixed,
        4200.0,
        vec3(-9.3, 3.4, 12.8),
        angular_footprint
    );
    float local_detail = resolved_surface_noise(
        earth_fixed,
        18000.0,
        vec3(15.7, -4.6, -7.2),
        angular_footprint
    );
    float micro_detail = resolved_surface_noise(
        earth_fixed,
        72000.0,
        vec3(-21.4, 13.8, 5.9),
        angular_footprint
    );
    relief_detail = vec2(local_detail, micro_detail);
    float relief = (approach_detail - 0.5) * 620.0 * approach_weight
        + (regional_detail - 0.5) * 180.0 * regional_weight
        + (local_detail - 0.5) * 38.0 * local_weight
        + (micro_detail - 0.5) * 5.0 * micro_weight;
    return relief * land;
}

float deep_water_angular_frequency(float angular_wavenumber) {
    const float gravity_mps2 = 9.80665;
    float physical_wavenumber = angular_wavenumber / u_earth_equatorial_radius_m;
    return sqrt(gravity_mps2 * physical_wavenumber);
}

float wave_bandlimit(float angular_wavenumber, float angular_footprint) {
    float phase_width = angular_wavenumber * angular_footprint;
    return 1.0 - smoothstep(0.35, 1.10, phase_width);
}

vec3 deep_water_wave_slope(
    vec3 local,
    vec3 axis,
    float angular_wavenumber,
    float steepness,
    float phase_offset,
    float angular_footprint
) {
    vec3 tangent_axis = axis - local * dot(axis, local);
    float tangent_length = length(tangent_axis);
    if (tangent_length < 0.0001) {
        return vec3(0.0);
    }
    float frequency = deep_water_angular_frequency(angular_wavenumber);
    float phase = dot(local, axis) * angular_wavenumber
        - frequency * u_time
        + phase_offset;
    return tangent_axis / tangent_length
        * (cos(phase) * steepness * wave_bandlimit(angular_wavenumber, angular_footprint));
}

float deep_water_wave_signal(
    vec3 local,
    vec3 axis,
    float angular_wavenumber,
    float phase_offset,
    float angular_footprint
) {
    float phase = dot(local, axis) * angular_wavenumber
        - deep_water_angular_frequency(angular_wavenumber) * u_time
        + phase_offset;
    return sin(phase) * wave_bandlimit(angular_wavenumber, angular_footprint);
}

vec3 ocean_normal(vec3 geometric, vec3 earth_fixed, float angular_footprint) {
    float regional_detail = 1.0 - smoothstep(1500000.0, 12000000.0, u_surface_clearance_m);
    float local_detail = 1.0 - smoothstep(150000.0, 2200000.0, u_surface_clearance_m);
    vec3 slope = vec3(0.0);
    slope += deep_water_wave_slope(
        earth_fixed,
        normalize(vec3(0.83, 0.08, 0.55)),
        1200.0,
        0.022,
        0.0,
        angular_footprint
    );
    slope += deep_water_wave_slope(
        earth_fixed,
        normalize(vec3(-0.35, 0.14, 0.93)),
        4200.0,
        0.017,
        1.7,
        angular_footprint
    );
    slope += regional_detail * deep_water_wave_slope(
        earth_fixed,
        normalize(vec3(0.57, -0.12, -0.81)),
        15000.0,
        0.011,
        4.1,
        angular_footprint
    );
    slope += regional_detail * deep_water_wave_slope(
        earth_fixed,
        normalize(vec3(-0.76, 0.05, -0.65)),
        52000.0,
        0.0065,
        2.3,
        angular_footprint
    );
    slope += local_detail * deep_water_wave_slope(
        earth_fixed,
        normalize(vec3(0.22, 0.03, -0.98)),
        180000.0,
        0.0038,
        5.4,
        angular_footprint
    );
    vec3 world_slope = rotate_y(slope, u_earth_rotation);
    return normalize(geometric - world_slope);
}

float cloud_density(
    vec3 point,
    vec3 view_ray_direction,
    out float altitude_agl
) {
    altitude_agl = 0.0;
    if (u_weather_ready < 0.5) {
        return 0.0;
    }
    vec3 local = point - u_earth_center_m;
    float local_length = max(length(local), 1.0);
    vec3 radial = local / local_length;
    float altitude = local_length - ellipsoid_surface_radius(local);
    vec3 terrain_fixed_direction = rotate_y(radial, -u_earth_rotation);
    altitude_agl = altitude - terrain_conservative_height_m(terrain_fixed_direction);
    if (altitude_agl < CLOUD_BASE_M - 500.0 || altitude_agl > CLOUD_TOP_M) {
        return 0.0;
    }

    bool lower_active = altitude_agl < CLOUD_DEEP_TOP_M;
    bool cirrus_active = altitude_agl > CIRRUS_BASE_M;
    vec3 lower_fixed_direction = rotate_y(
        radial,
        -u_earth_rotation + u_time * 0.0000031
    );
    vec3 upper_fixed_direction = rotate_y(
        radial,
        -u_earth_rotation + u_time * 0.0000068 + radial.y * 0.025
    );
    float view_incidence = max(abs(dot(radial, view_ray_direction)), 0.12);
    float cloud_angular_footprint = max(
        length(point) * vertical_pixel_angle()
            / max(u_earth_equatorial_radius_m * view_incidence, 1.0),
        1.0e-9
    );
    float weather_texels_per_pixel = cloud_angular_footprint
        * float(textureSize(u_weather, 0).x)
        / TAU;
    float weather_lod = clamp(log2(max(weather_texels_per_pixel, 1.0)), 0.0, 5.0);
    float orbital_weather_blur = 7.0 * smoothstep(
        300000.0,
        1200000.0,
        u_surface_clearance_m
    );
    weather_lod = min(weather_lod + orbital_weather_blur, 7.0);

    vec3 lower_weather = vec3(0.0);
    float low_profile = 0.0;
    float low_top = CLOUD_LOW_TOP_M;
    float macro_low = 0.0;
    if (lower_active) {
        lower_weather = sample_equirect_lod(
            u_weather,
            spherical_uv(lower_fixed_direction),
            weather_lod
        ).rgb;
        low_top = mix(
            2800.0,
            CLOUD_LOW_TOP_M,
            smoothstep(0.34, 0.72, lower_weather.r)
        );
        low_profile = smoothstep(CLOUD_BASE_M, 2200.0, altitude_agl)
            * (1.0 - smoothstep(max(low_top - 1100.0, 2400.0), low_top, altitude_agl));
        macro_low = pow(saturate(lower_weather.r), 1.35)
            * mix(0.52, 1.0, smoothstep(0.04, 0.62, lower_weather.g))
            * low_profile;
    }

    vec3 upper_weather = vec3(0.0);
    float cirrus_profile = 0.0;
    float cirrus_base = -1.0;
    float macro_cirrus = 0.0;
    if (cirrus_active) {
        upper_weather = sample_equirect_lod(
            u_weather,
            spherical_uv(upper_fixed_direction),
            weather_lod
        ).rgb;
        cirrus_profile = smoothstep(CIRRUS_BASE_M, 8200.0, altitude_agl)
            * (1.0 - smoothstep(13200.0, CLOUD_TOP_M, altitude_agl));
        cirrus_base = upper_weather.b + (upper_weather.g - 0.5) * 0.08;
        macro_cirrus = pow(saturate(upper_weather.b), 1.20)
            * cirrus_profile
            * 0.28;
    }
    if (macro_low <= 0.001 && macro_cirrus <= 0.001) {
        return 0.0;
    }
    // Rotate the volume lattice away from Earth axes so neither whole-globe nor
    // close cloud detail inherits latitude/longitude-aligned cells.
    vec3 cloud_domain = mat3(
        vec3(0.36, -0.48, 0.80),
        vec3(0.80, 0.60, 0.00),
        vec3(-0.48, 0.64, 0.60)
    ) * lower_fixed_direction;

    // Every field has a fixed Earth-space frequency. The blend is uniform for the
    // draw, so endpoint branches are coherent and skip whole field families without
    // changing a formation's coordinates as the visitor moves.
    float globe_detail_blend = 1.0
        - smoothstep(300000.0, 1200000.0, u_surface_clearance_m);
    float shared_shape = cloud_fbm3(
        cloud_domain * 19.0
            + altitude_agl * vec3(0.00005, -0.00007, 0.00004)
            + vec3(5.7, -1.9, -8.3),
        19.0,
        cloud_angular_footprint
    );
    float shared_occupancy = 0.0;
    if (lower_active) {
        float occupancy_threshold = mix(0.70, 0.36, lower_weather.r);
        shared_occupancy = smoothstep(
            occupancy_threshold,
            occupancy_threshold + 0.34,
            shared_shape + (lower_weather.g - 0.5) * 0.10
        );
    }
    float orbital_density = 0.0;
    if (globe_detail_blend < 0.999) {
        // At orbital scale the weather texture already carries natural broad
        // coverage. Thresholding the 3D lattice here exposes its similar-sized
        // interpolation cells as a conspicuous repeated tile field, so reserve
        // that breakup for genuinely resolvable close volume structure.
        orbital_density = saturate(
            macro_low * 0.92
                + macro_cirrus
        );
        if (globe_detail_blend <= 0.001) {
            return orbital_density;
        }
    }

    float coarse_shape = shared_shape;
    float medium_shape = resolved_cloud_noise3(
        cloud_domain * 181.0
            + altitude_agl * vec3(-0.00016, 0.00024, 0.00011)
            + vec3(-2.6, 4.1, 7.4),
        181.0,
        cloud_angular_footprint
    );
    float fine_shape = resolved_cloud_noise3(
        cloud_domain * 733.0
            + altitude_agl * vec3(0.00041, -0.00033, 0.00027)
            + vec3(11.2, -6.4, 2.7),
        733.0,
        cloud_angular_footprint
    );
    float cloud_shape = coarse_shape * 0.50
        + medium_shape * 0.32
        + fine_shape * 0.18;
    float base_variation = (coarse_shape - 0.5) * 700.0;
    float local_cloud_base = CLOUD_BASE_M + base_variation;
    float lower_cloud = 0.0;
    if (lower_weather.r > 0.12) {
        float vertical_driver = smoothstep(
            0.38,
            0.72,
            cloud_shape * 0.72 + lower_weather.r * 0.28
        );
        float resolved_top = mix(
            low_top,
            min(low_top + 6500.0, CLOUD_DEEP_TOP_M),
            vertical_driver
        );
        float shaped_profile = smoothstep(local_cloud_base, local_cloud_base + 700.0, altitude_agl)
            * (1.0 - smoothstep(
                max(resolved_top - 1800.0, local_cloud_base + 900.0),
                resolved_top,
                altitude_agl
            ));
        float coverage_strength = pow(saturate(lower_weather.r), 1.30)
            * mix(0.62, 1.0, lower_weather.g);
        float stratiform = coverage_strength
            * shaped_profile
            * shared_occupancy
            * mix(0.82, 1.08, cloud_shape);

        // Only the strongest systems grow through the middle troposphere.
        float tower_support = smoothstep(
            0.42,
            0.72,
            lower_weather.r * 0.76 + cloud_shape * 0.24
        );
        float deep_top = mix(5200.0, CLOUD_DEEP_TOP_M, tower_support);
        float deep_profile = smoothstep(local_cloud_base + 250.0, 3300.0, altitude_agl)
            * (1.0 - smoothstep(
                max(deep_top - 3200.0, 3900.0),
                deep_top,
                altitude_agl
            ));
        float tower_height = saturate(
            (altitude_agl - 2500.0) / max(deep_top - 2500.0, 1.0)
        );
        float tower_billow = mix(
            coarse_shape * 0.62 + medium_shape * 0.28 + fine_shape * 0.10,
            coarse_shape * 0.44 + medium_shape * 0.39 + fine_shape * 0.17,
            tower_height
        );
        tower_billow += (medium_shape - 0.5) * mix(0.12, 0.22, tower_height);
        float convective_tower = smoothstep(
            mix(0.44, 0.50, tower_height),
            mix(0.67, 0.72, tower_height),
            tower_billow
        ) * deep_profile * tower_support;
        float anvil_profile = smoothstep(0.58, 0.76, tower_height)
            * (1.0 - smoothstep(0.88, 1.0, tower_height));
        float anvil_shape = smoothstep(
            0.43,
            0.66,
            coarse_shape * 0.54 + medium_shape * 0.36 + fine_shape * 0.10
        );
        float convective_anvil = anvil_shape * anvil_profile * tower_support * 0.68;
        lower_cloud = max(
            stratiform * 0.72,
            max(convective_tower * 0.86, convective_anvil * 0.72)
        );
    }

    float cirrus = 0.0;
    if (cirrus_base + 0.03 > 0.60) {
        vec3 cirrus_domain = cloud_domain.yzx;
        float cirrus_coarse = resolved_cloud_noise3(
            cirrus_domain * 137.3
                + altitude_agl * vec3(0.00007, -0.00014, 0.00010)
                + vec3(-9.1, 2.2, 4.6),
            137.3,
            cloud_angular_footprint
        );
        float cirrus_fine = resolved_cloud_noise3(
            cirrus_domain * 911.0
                + altitude_agl * vec3(-0.00021, 0.00018, 0.00016)
                + vec3(3.8, -7.6, 12.1),
            911.0,
            cloud_angular_footprint
        );
        float cirrus_detail = cirrus_coarse * 0.72 + cirrus_fine * 0.28;
        float cirrus_field = cirrus_base + (cirrus_detail - 0.5) * 0.06;
        cirrus = smoothstep(0.60, 0.76, cirrus_field) * cirrus_profile * 0.38;
    }
    float detailed_density = saturate((lower_cloud + cirrus) * 0.82);
    if (globe_detail_blend >= 0.999) {
        return detailed_density;
    }
    return mix(orbital_density, detailed_density, globe_detail_blend);
}

// Sun rays need only broad optical occupancy. Re-evaluating the full view-density
// field for every light sample multiplies its most expensive fBm work without adding
// resolvable shadow detail, so this path keeps one fixed Earth-space noise band and
// the same terrain-relative vertical envelopes.
float cloud_shadow_density(vec3 point) {
    if (u_weather_ready < 0.5) {
        return 0.0;
    }
    vec3 local = point - u_earth_center_m;
    float local_length = max(length(local), 1.0);
    vec3 radial = local / local_length;
    float altitude = local_length - ellipsoid_surface_radius(local);
    vec3 terrain_fixed_direction = rotate_y(radial, -u_earth_rotation);
    float altitude_agl = altitude - terrain_conservative_height_m(terrain_fixed_direction);
    if (altitude_agl < CLOUD_BASE_M - 500.0 || altitude_agl > CLOUD_TOP_M) {
        return 0.0;
    }

    vec3 lower_fixed_direction = rotate_y(
        radial,
        -u_earth_rotation + u_time * 0.0000031
    );
    vec3 shadow_domain = mat3(
        vec3(0.36, -0.48, 0.80),
        vec3(0.80, 0.60, 0.00),
        vec3(-0.48, 0.64, 0.60)
    ) * lower_fixed_direction;
    float shadow_shape = value_noise3(
        shadow_domain * 181.0
            + altitude_agl * vec3(-0.00016, 0.00024, 0.00011)
            + vec3(-2.6, 4.1, 7.4)
    );

    float lower_density = 0.0;
    if (altitude_agl < CLOUD_DEEP_TOP_M) {
        vec3 weather = sample_equirect_lod(
            u_weather,
            spherical_uv(lower_fixed_direction),
            2.0
        ).rgb;
        float low_top = mix(
            2800.0,
            CLOUD_LOW_TOP_M,
            smoothstep(0.34, 0.72, weather.r)
        );
        float local_base = CLOUD_BASE_M + (shadow_shape - 0.5) * 700.0;
        float stratiform_profile = smoothstep(
            local_base,
            local_base + 700.0,
            altitude_agl
        ) * (1.0 - smoothstep(
            max(low_top - 1100.0, local_base + 900.0),
            low_top,
            altitude_agl
        ));
        float coverage_threshold = mix(0.79, 0.37, weather.r);
        float occupied = smoothstep(
            coverage_threshold,
            coverage_threshold + 0.24,
            shadow_shape * 0.87 + weather.g * 0.13
        );
        float tower_support = smoothstep(
            0.42,
            0.72,
            weather.r * 0.76 + shadow_shape * 0.24
        );
        float deep_top = mix(5200.0, CLOUD_DEEP_TOP_M, tower_support);
        float deep_profile = smoothstep(local_base + 250.0, 3300.0, altitude_agl)
            * (1.0 - smoothstep(
                max(deep_top - 3200.0, 3900.0),
                deep_top,
                altitude_agl
            ));
        lower_density = max(
            occupied * stratiform_profile * 0.88,
            occupied * deep_profile * tower_support * 1.18
        );
    }

    float cirrus_density = 0.0;
    if (altitude_agl > CIRRUS_BASE_M) {
        vec3 upper_fixed_direction = rotate_y(
            radial,
            -u_earth_rotation + u_time * 0.0000068 + radial.y * 0.025
        );
        vec3 upper_weather = sample_equirect_lod(
            u_weather,
            spherical_uv(upper_fixed_direction),
            2.0
        ).rgb;
        vec3 cirrus_domain = mat3(
            vec3(0.36, -0.48, 0.80),
            vec3(0.80, 0.60, 0.00),
            vec3(-0.48, 0.64, 0.60)
        ) * upper_fixed_direction;
        float cirrus_shape = value_noise3(
            cirrus_domain.yzx * 137.3
                + altitude_agl * vec3(0.00007, -0.00014, 0.00010)
                + vec3(-9.1, 2.2, 4.6)
        );
        float cirrus_field = upper_weather.b
            + (upper_weather.g - 0.5) * 0.08
            + (cirrus_shape - 0.5) * 0.06;
        float cirrus_profile = smoothstep(CIRRUS_BASE_M, 8200.0, altitude_agl)
            * (1.0 - smoothstep(13200.0, CLOUD_TOP_M, altitude_agl));
        cirrus_density = smoothstep(0.60, 0.76, cirrus_field)
            * cirrus_profile
            * 0.38;
    }
    return saturate(lower_density + cirrus_density);
}

float cloud_phase(float cosine_theta) {
    const float forward_g = 0.76;
    const float backward_g = -0.20;
    float forward = (1.0 - forward_g * forward_g)
        / pow(max(1.0 + forward_g * forward_g - 2.0 * forward_g * cosine_theta, 0.001), 1.5);
    float backward = (1.0 - backward_g * backward_g)
        / pow(max(1.0 + backward_g * backward_g - 2.0 * backward_g * cosine_theta, 0.001), 1.5);
    return 0.78 * forward + 0.22 * backward;
}

float cloud_shadow(vec3 surface_point, vec3 light_direction) {
    // The shadow-volume lattice is only meaningful once its kilometre-scale
    // structure is resolvable. From orbit it otherwise projects a repeating
    // field of dark cells onto an intentionally broad weather layer.
    float resolved_shadow_weight = 1.0 - smoothstep(
        300000.0,
        1200000.0,
        u_surface_clearance_m
    );
    if (resolved_shadow_weight <= 0.001) {
        return 1.0;
    }
    float optical_depth = 0.0;
    for (int sample_index = 0; sample_index < 2; ++sample_index) {
        float distance_to_cloud = sample_index == 0
            ? 2400.0
            : 8200.0;
        float segment_length = sample_index == 0 ? 4800.0 : 6800.0;
        optical_depth += cloud_shadow_density(
            surface_point + light_direction * distance_to_cloud
        ) * segment_length / 8000.0;
    }
    return mix(
        1.0,
        exp(-optical_depth * 1.05),
        resolved_shadow_weight
    );
}

void integrate_clouds(
    vec3 ray_direction,
    float maximum_distance,
    out vec3 scattering,
    out float transmission
) {
    scattering = vec3(0.0);
    transmission = 1.0;
    if (u_weather_ready < 0.5) {
        return;
    }

    vec2 cloud_interval = ray_ellipsoid_from(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        earth_radii_at_altitude(CLOUD_SHELL_TOP_M)
    );
    float start_distance = max(cloud_interval.x, 0.0);
    float end_distance = min(cloud_interval.y, maximum_distance);
    float near_traversal = 1.0 - smoothstep(100000.0, 600000.0, u_surface_clearance_m);
    float traversal_limit_m = mix(300000.0, 60000.0, near_traversal);
    end_distance = min(end_distance, start_distance + traversal_limit_m);
    if (cloud_interval.y <= 0.0 || end_distance <= start_distance) {
        return;
    }

    const int sample_count = 12;
    float interval_length = end_distance - start_distance;
    float near_warp = 1.0 - smoothstep(20000.0, 250000.0, u_surface_clearance_m);
    for (int sample_index = 0; sample_index < 12; ++sample_index) {
        float linear_start = float(sample_index) / float(sample_count);
        float linear_end = float(sample_index + 1) / float(sample_count);
        float warped_start = mix(linear_start, linear_start * linear_start, near_warp);
        float warped_end = mix(linear_end, linear_end * linear_end, near_warp);
        float sample_fraction = 0.5 * (warped_start + warped_end);
        float sample_length = (warped_end - warped_start) * interval_length;
        float distance_along_ray = start_distance + sample_fraction * interval_length;
        vec3 point = ray_direction * distance_along_ray;
        float altitude_agl;
        float density = cloud_density(point, ray_direction, altitude_agl);
        if (density <= 0.001) {
            continue;
        }
        vec3 light_direction = normalize(u_sun_center_m - point);
        vec3 radial = normalize(point - u_earth_center_m);
        float solar_height = dot(radial, light_direction);
        float n_dot_l = max(solar_height, 0.0);
        float eclipse = solar_visibility(point, u_moon_center_m, u_moon_radius_m);
        vec3 atmospheric_sun = sunlight_transmittance(point, light_direction);
        float broad_lighting_weight = smoothstep(
            600000.0,
            1200000.0,
            u_surface_clearance_m
        );
        float sunward_near = density * 0.74;
        if (broad_lighting_weight < 0.999) {
            float detailed_sunward_near = cloud_shadow_density(
                point + light_direction * 4200.0
            );
            sunward_near = mix(
                detailed_sunward_near,
                sunward_near,
                broad_lighting_weight
            );
        }
        float sunward_optical_depth = sunward_near * 4200.0 / 9000.0;
        float self_shadow = exp(-sunward_optical_depth * 1.24);
        float exposed_edge = smoothstep(-0.12, 0.18, density - sunward_near);
        float view_light = dot(ray_direction, light_direction);
        float phase_gain = clamp(cloud_phase(view_light) * 0.28, 0.22, 4.5);
        float horizon_lighting = smoothstep(-0.08, 0.16, solar_height);
        float direct = eclipse
            * self_shadow
            * mix(0.62, 1.28, exposed_edge)
            * horizon_lighting
            * (0.18 + n_dot_l * 0.82);
        float height_fraction = saturate((altitude_agl - CLOUD_BASE_M)
            / (CLOUD_TOP_M - CLOUD_BASE_M));
        float powder = 1.0 - exp(-density * 2.1);
        vec3 cloud_color = vec3(0.0035, 0.0038, 0.0044)
                * mix(0.65, 1.25, height_fraction)
            + atmospheric_sun
                * vec3(1.00, 0.96, 0.89)
                * direct
                * (0.34 + phase_gain * 0.52 + powder * 0.20);
        // Extinction is a material property and therefore does not decrease at night.
        // Unlit clouds stay dark while continuing to occlude cities and the surface.
        float optical_depth = density * sample_length / 10000.0;
        float alpha = 1.0 - exp(-optical_depth);
        scattering += transmission * alpha * cloud_color;
        transmission *= 1.0 - alpha;
        if (transmission < 0.015) {
            break;
        }
    }
}

vec3 shade_earth(vec3 ray_direction, float distance_to_surface) {
    vec3 radii = vec3(
        u_earth_equatorial_radius_m,
        u_earth_polar_radius_m,
        u_earth_equatorial_radius_m
    );
    vec3 point = ray_direction * distance_to_surface;
    vec3 local_point = point - u_earth_center_m;
    vec3 geometric = normalize(local_point / (radii * radii));
    vec3 earth_fixed = earth_fixed_direction(point);
    vec2 uv = spherical_uv(earth_fixed);
    vec3 view_direction = -ray_direction;
    float geometric_view_incidence = max(dot(geometric, view_direction), 0.001);
    float angular_footprint = earth_surface_angular_footprint(
        distance_to_surface,
        geometric_view_incidence
    );
    vec4 material = material_sample(uv, angular_footprint);
    float land = smoothstep(0.42, 0.58, material.r);
    float snow = material.a;
    vec3 reference = srgb_to_linear(sample_equirect_lod(
        u_surface,
        uv,
        earth_texture_lod(u_surface, angular_footprint, 0.10)
    ).rgb);
    vec3 terrain = geometric;
    float fine_relief_m = 0.0;
    vec2 relief_detail = vec2(0.5);
    vec3 normal = geometric;
    if (land > 0.01) {
        terrain = terrain_normal(
            geometric,
            earth_fixed,
            uv,
            land,
            angular_footprint
        );
        fine_relief_m = procedural_relief_m(
            earth_fixed,
            land,
            angular_footprint,
            relief_detail
        );
        normal = terrain;
    }
    if (land < 0.99) {
        vec3 water = ocean_normal(geometric, earth_fixed, angular_footprint);
        normal = normalize(mix(water, terrain, land));
    }

    vec3 light_direction = normalize(u_sun_center_m - point);
    float surface_sun_height = dot(geometric, light_direction);
    float n_dot_l = max(dot(normal, light_direction), 0.0);
    float n_dot_v = max(dot(normal, view_direction), 0.001);
    vec3 atmospheric_sun = sunlight_transmittance(point + geometric * 30.0, light_direction);
    float eclipse = 1.0;
    float cloud_light = 1.0;
    if (n_dot_l > 0.0 && max_component(atmospheric_sun) > 0.0001) {
        eclipse = solar_visibility(point, u_moon_center_m, u_moon_radius_m);
        if (eclipse > 0.0) {
            cloud_light = cloud_shadow(point + geometric * 30.0, light_direction);
        }
    }
    vec3 direct_light = atmospheric_sun
        * (SURFACE_SUN_RADIANCE * n_dot_l * eclipse * cloud_light);

    vec3 land_albedo = reference;
    if (land > 0.01 && snow > 0.001) {
        float glacier_detail = value_noise3(earth_fixed * 185.0 + vec3(3.2, -7.8, 11.1));
        vec3 glacier_albedo = vec3(0.58, 0.68, 0.75)
            * mix(0.90, 1.06, glacier_detail);
        land_albedo = mix(land_albedo, glacier_albedo, snow * 0.42);
    }
    if (land > 0.01) {
        float continental_detail = resolved_surface_noise(
            earth_fixed,
            260.0,
            vec3(4.7, -8.2, 2.1),
            angular_footprint
        );
        float biome_detail = resolved_surface_noise(
            earth_fixed,
            1500.0,
            vec3(-9.4, 3.5, 12.7),
            angular_footprint
        );
        float approach_strength = 1.0 - smoothstep(
            600000.0,
            5000000.0,
            u_surface_clearance_m
        );
        float terrain_pattern = saturate(
            0.5 + (relief_detail.x - 0.5) * 0.85
                + (relief_detail.y - 0.5) * 0.35
        );
        terrain_pattern = smoothstep(0.28, 0.72, terrain_pattern);
        float relief_tone = saturate(0.5 + fine_relief_m / 900.0);
        float near_strength = 1.0 - smoothstep(
            70000.0,
            600000.0,
            u_surface_clearance_m
        );
        land_albedo *= mix(
            1.0,
            mix(0.90, 1.10, continental_detail)
                * mix(0.95, 1.05, biome_detail),
            approach_strength
        );
        vec3 terrain_tint = mix(
            vec3(0.96, 0.96, 0.96),
            vec3(1.06, 1.04, 1.00),
            relief_tone
        );
        land_albedo *= mix(
            vec3(1.0),
            terrain_tint * mix(0.86, 1.14, terrain_pattern),
            near_strength * 0.58
        );
        float ground_patch = smoothstep(
            0.34,
            0.66,
            relief_detail.x * 0.58
                + relief_detail.y * 0.27
                + biome_detail * 0.15
        );
        float ground_grain = resolved_surface_noise(
            earth_fixed,
            120000.0,
            vec3(6.8, -17.3, 22.1),
            angular_footprint
        );
        float ground_fine = resolved_surface_noise(
            earth_fixed,
            360000.0,
            vec3(-14.2, 5.9, 27.6),
            angular_footprint
        );
        float ground_micro = resolved_surface_noise(
            earth_fixed,
            1100000.0,
            vec3(31.7, -12.4, 8.6),
            angular_footprint
        );
        vec3 ground_multiplier = mix(
            vec3(0.56, 0.76, 0.49),
            vec3(1.42, 1.22, 0.86),
            ground_patch
        );
        vec3 ground_palette = mix(
            vec3(0.014, 0.050, 0.009),
            vec3(0.180, 0.120, 0.060),
            ground_patch
        );
        float ground_material_weight = near_strength * (1.0 - snow) * 0.66;
        land_albedo = mix(
            land_albedo,
            mix(land_albedo * ground_multiplier, ground_palette, 0.32),
            ground_material_weight
        );
        float ground_mottle = mix(0.74, 1.26, relief_detail.y);
        land_albedo *= mix(1.0, ground_mottle, near_strength * 0.52);
        land_albedo *= mix(
            1.0,
            mix(
                0.66,
                1.34,
                ground_grain * 0.56 + ground_fine * 0.28 + ground_micro * 0.16
            ),
            near_strength * 0.64
        );
        float resolved_ground_texture = smoothstep(
            0.38,
            0.62,
            ground_grain * 0.56 + ground_fine * 0.28 + ground_micro * 0.16
        );
        vec3 ground_texture_tint = mix(
            vec3(0.62, 0.82, 0.54),
            vec3(1.30, 1.15, 0.82),
            resolved_ground_texture
        );
        land_albedo *= mix(
            vec3(1.0),
            ground_texture_tint,
            near_strength * (1.0 - snow) * 0.38
        );
        float exposed_rock = smoothstep(
            0.68,
            0.88,
            relief_detail.y * 0.62 + terrain_pattern * 0.38
        ) * near_strength * (1.0 - snow);
        land_albedo = mix(land_albedo, vec3(0.125, 0.115, 0.095), exposed_rock * 0.34);
    }
    vec3 albedo = land_albedo;
    float ocean_swell_light = 0.5;
    float resolved_swell_color = 0.0;
    if (land < 0.99) {
        float ocean_depth = saturate(-decoded_elevation_m(material.g) / 7600.0);
        vec3 shallow_water = vec3(0.018, 0.145, 0.205);
        vec3 deep_water = vec3(0.0075, 0.046, 0.088);
        vec3 ocean_albedo = mix(shallow_water, deep_water, ocean_depth);
        ocean_albedo = mix(ocean_albedo, reference * 0.32, 0.16);
        float swell_a = deep_water_wave_signal(
            earth_fixed,
            normalize(vec3(-0.35, 0.14, 0.93)),
            4200.0,
            1.7,
            angular_footprint
        );
        float swell_b = deep_water_wave_signal(
            earth_fixed,
            normalize(vec3(0.57, -0.12, -0.81)),
            15000.0,
            4.1,
            angular_footprint
        );
        ocean_swell_light = saturate(0.5 + swell_a * 0.24 + swell_b * 0.12);
        resolved_swell_color = 1.0 - smoothstep(
            500.0,
            4000.0,
            u_surface_clearance_m
        );
        ocean_albedo *= mix(
            1.0,
            mix(0.82, 1.20, ocean_swell_light),
            resolved_swell_color
        );
        float whitecap = smoothstep(0.72, 0.90, ocean_swell_light)
            * (1.0 - ocean_depth * 0.62)
            * resolved_swell_color;
        ocean_albedo = mix(
            ocean_albedo,
            vec3(0.16, 0.30, 0.34),
            whitecap * 0.18
        );
        float ocean_surface_variation = resolved_surface_noise(
            earth_fixed,
            85000.0,
            vec3(18.4, -7.3, 24.8),
            angular_footprint
        );
        float ocean_fine_variation = resolved_surface_noise(
            earth_fixed,
            270000.0,
            vec3(-11.7, 29.1, 6.2),
            angular_footprint
        );
        float ocean_resolved_texture = ocean_surface_variation * 0.68
            + ocean_fine_variation * 0.32;
        ocean_albedo *= mix(0.94, 1.06, ocean_resolved_texture);
        albedo = mix(ocean_albedo, land_albedo, land);
    }
    vec3 ambient = albedo * mix(
        vec3(0.0018, 0.0019, 0.0021),
        vec3(0.020, 0.030, 0.045),
        smoothstep(-0.12, 0.16, surface_sun_height)
    );
    vec3 color = ambient + albedo * direct_light / PI;

    if (land < 0.99) {
        float environment_fresnel = 0.0211
            + (1.0 - 0.0211) * pow(1.0 - n_dot_v, 5.0);
        float daylight = smoothstep(-0.08, 0.24, surface_sun_height);
        vec3 sky_zenith = vec3(0.075, 0.21, 0.48) * daylight;
        vec3 sky_horizon = vec3(0.23, 0.43, 0.64) * daylight;
        vec3 sky_reflection = mix(
            sky_zenith,
            sky_horizon,
            pow(1.0 - n_dot_v, 0.65)
        );
        color += sky_reflection
            * environment_fresnel
            * (1.0 - land)
            * mix(0.55, 1.0, cloud_light);
        color += vec3(0.003, 0.018, 0.032)
            * daylight
            * (0.25 + mix(0.5, ocean_swell_light, resolved_swell_color) * 1.20)
            * (1.0 - land);
    }

    if (land < 0.99 && n_dot_l > 0.0) {
        vec3 half_vector = normalize(light_direction + view_direction);
        float n_dot_h = max(dot(normal, half_vector), 0.001);
        float v_dot_h = max(dot(view_direction, half_vector), 0.0);
        const float ocean_alpha = 0.197;
        float alpha_squared = ocean_alpha * ocean_alpha;
        float n_dot_h_squared = n_dot_h * n_dot_h;
        float tangent_squared = (1.0 - n_dot_h_squared) / n_dot_h_squared;
        float distribution = exp(-tangent_squared / alpha_squared)
            / (PI * alpha_squared * n_dot_h_squared * n_dot_h_squared);
        float fresnel = 0.0211 + (1.0 - 0.0211) * pow(1.0 - v_dot_h, 5.0);
        float geometry = min(
            1.0,
            min(
                2.0 * n_dot_h * n_dot_v / max(v_dot_h, 0.001),
                2.0 * n_dot_h * n_dot_l / max(v_dot_h, 0.001)
            )
        );
        float ocean_specular = distribution * fresnel * geometry
            / max(4.0 * n_dot_l * n_dot_v, 0.001);
        color += atmospheric_sun
            * (SURFACE_SUN_RADIANCE * n_dot_l * eclipse * cloud_light)
            * ocean_specular
            * (1.0 - land)
            * 0.22;
    }

    if (u_night_ready > 0.5 && land > 0.01 && surface_sun_height < 0.04) {
        vec3 night_reference = srgb_to_linear(sample_equirect_lod(
            u_night,
            uv,
            earth_texture_lod(u_night, angular_footprint, 0.10)
        ).rgb);
        float night_luminance = dot(night_reference, vec3(0.2126, 0.7152, 0.0722));
        float night_chroma = max(
            max(night_reference.r, night_reference.g),
            night_reference.b
        ) - min(min(night_reference.r, night_reference.g), night_reference.b);
        float extracted_lights = saturate(
            (night_luminance - night_chroma * 0.62 - 0.012) * 5.2
        );
        float city_breakup = 0.5;
        float resolved_city_breakup = resolved_surface_noise(
            earth_fixed,
            12500.0,
            vec3(-4.8, 19.2, 7.6),
            angular_footprint
        );
        float city_detail_weight = 1.0 - smoothstep(
            120000.0,
            600000.0,
            u_surface_clearance_m
        );
        city_breakup = mix(city_breakup, resolved_city_breakup, city_detail_weight);
        extracted_lights *= mix(0.42, 1.24, smoothstep(0.30, 0.76, city_breakup));
        float night = 1.0 - smoothstep(-0.12, 0.035, surface_sun_height);
        vec3 city_emission = vec3(1.25, 0.82, 0.42) * pow(extracted_lights, 0.68);
        color += city_emission * night * land * (1.0 - snow * 0.55) * 1.35;
    }
    return color;
}

vec3 shade_moon(vec3 ray_direction, float distance_to_surface) {
    vec3 point = ray_direction * distance_to_surface;
    vec3 geometric = normalize(point - u_moon_center_m);
    vec3 near_side_axis = normalize(u_earth_center_m - u_moon_center_m);
    vec3 reference_up = abs(near_side_axis.y) < 0.96
        ? vec3(0.0, 1.0, 0.0)
        : vec3(0.0, 0.0, 1.0);
    vec3 north_axis = normalize(reference_up - near_side_axis * dot(reference_up, near_side_axis));
    vec3 east_axis = normalize(cross(near_side_axis, north_axis));
    vec3 moon_fixed = normalize(vec3(
        dot(geometric, near_side_axis),
        dot(geometric, north_axis),
        dot(geometric, east_axis)
    ));
    vec2 uv = spherical_uv(moon_fixed);

    vec3 procedural_albedo;
    float broad = moon_fbm(geometric * 4.8 + vec3(1.7, -3.1, 5.2));
    float fine = moon_fbm(geometric * 26.0 + vec3(-8.0, 2.4, 4.1));
    float maria = smoothstep(0.30, 0.68, broad);
    float crater_rims = smoothstep(0.72, 0.79, fine) * (1.0 - smoothstep(0.79, 0.86, fine));
    procedural_albedo = mix(vec3(0.075, 0.071, 0.066), vec3(0.185, 0.176, 0.161), maria);
    procedural_albedo = mix(procedural_albedo, vec3(0.27, 0.25, 0.22), crater_rims * 0.38);

    float moon_lod = moon_texture_lod(u_moon_albedo);
    vec3 measured_albedo = srgb_to_linear(
        sample_equirect_lod(u_moon_albedo, uv, moon_lod).rgb
    ) * 0.42;
    vec3 albedo = mix(procedural_albedo, measured_albedo, step(0.5, u_moon_albedo_ready));

    vec2 texel = 1.0 / vec2(textureSize(u_moon_albedo, 0));
    float west = dot(
        srgb_to_linear(sample_equirect_lod(
            u_moon_albedo,
            uv - vec2(texel.x, 0.0),
            moon_lod
        ).rgb),
        vec3(0.2126, 0.7152, 0.0722)
    );
    float east_height = dot(
        srgb_to_linear(sample_equirect_lod(
            u_moon_albedo,
            uv + vec2(texel.x, 0.0),
            moon_lod
        ).rgb),
        vec3(0.2126, 0.7152, 0.0722)
    );
    float north_height = dot(
        srgb_to_linear(sample_equirect_lod(
            u_moon_albedo,
            uv - vec2(0.0, texel.y),
            moon_lod
        ).rgb),
        vec3(0.2126, 0.7152, 0.0722)
    );
    float south = dot(
        srgb_to_linear(sample_equirect_lod(
            u_moon_albedo,
            uv + vec2(0.0, texel.y),
            moon_lod
        ).rgb),
        vec3(0.2126, 0.7152, 0.0722)
    );
    vec3 surface_east = normalize(cross(
        abs(geometric.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0),
        geometric
    ));
    vec3 surface_north = normalize(cross(geometric, surface_east));
    vec3 detailed_normal = normalize(
        geometric
            + surface_east * (west - east_height) * 0.18
            + surface_north * (south - north_height) * 0.18
    );
    vec3 normal = normalize(mix(geometric, detailed_normal, step(0.5, u_moon_albedo_ready)));
    vec3 light_direction = normalize(u_sun_center_m - point);
    vec3 view_direction = -ray_direction;
    float sunlight = max(dot(normal, light_direction), 0.0);
    float view_cosine = max(dot(normal, view_direction), 0.001);
    float lommel_seeliger = sunlight / max(sunlight + view_cosine, 0.08);
    float diffuse = mix(sunlight, lommel_seeliger, 0.38);
    float visibility = solar_visibility(point, u_earth_center_m, u_earth_equatorial_radius_m);
    vec3 earth_direction = normalize(u_earth_center_m - point);
    vec3 earth_to_moon = normalize(point - u_earth_center_m);
    vec3 earth_to_sun = normalize(u_sun_center_m - u_earth_center_m);
    float illuminated_earth_fraction = 0.5 * (1.0 + dot(earth_to_moon, earth_to_sun));
    float earthshine = max(dot(normal, earth_direction), 0.0)
        * illuminated_earth_fraction
        * 0.045;
    float opposition = pow(max(dot(normal, view_direction), 0.0), 8.0)
        * sunlight
        * 0.035;
    vec3 refracted_umbra = vec3(0.055, 0.0045, 0.0012)
        * sunlight
        * (1.0 - visibility);
    return albedo * (0.0012 + earthshine + visibility * diffuse * 2.15 + opposition)
        + refracted_umbra;
}

vec3 shade_sun(vec3 ray_direction, float distance_to_surface) {
    vec3 point = ray_direction * distance_to_surface;
    vec3 normal = normalize(point - u_sun_center_m);
    float view_cosine = max(dot(normal, -ray_direction), 0.0);
    vec3 rotating_normal = rotate_y(normal, u_time * 0.0000029);
    float granulation = value_noise3(rotating_normal * 115.0 + vec3(7.2, -3.4, 11.8));
    float supergranulation = moon_fbm(rotating_normal * 18.0 + vec3(-2.7, 8.1, 4.3));

    // A few stable active-region groups read as solar structure without turning
    // low-resolution views into a field of random black freckles.
    vec3 active_a = normalize(vec3(0.92, 0.18, -0.35));
    vec3 active_b = normalize(vec3(0.94, 0.15, -0.30));
    vec3 active_c = normalize(vec3(-0.50, -0.20, 0.84));
    vec3 active_d = normalize(vec3(-0.56, -0.17, 0.81));
    float separation_a = 1.0 - dot(rotating_normal, active_a);
    float separation_b = 1.0 - dot(rotating_normal, active_b);
    float separation_c = 1.0 - dot(rotating_normal, active_c);
    float separation_d = 1.0 - dot(rotating_normal, active_d);
    float penumbra = max(
        max(1.0 - smoothstep(0.00012, 0.00072, separation_a), 1.0 - smoothstep(0.00008, 0.00046, separation_b)),
        max(1.0 - smoothstep(0.00010, 0.00062, separation_c), 1.0 - smoothstep(0.00006, 0.00034, separation_d))
    );
    float umbra = max(
        max(1.0 - smoothstep(0.000015, 0.00012, separation_a), 1.0 - smoothstep(0.000012, 0.00008, separation_b)),
        max(1.0 - smoothstep(0.000014, 0.00010, separation_c), 1.0 - smoothstep(0.000010, 0.00006, separation_d))
    );
    float spot_attenuation = 1.0 - penumbra * 0.32 - umbra * 0.38;
    float limb_darkening = 0.20 + 0.80 * pow(view_cosine, 0.65);
    float texture_variation = 0.68 + granulation * 0.42 + supergranulation * 0.16;
    vec3 photosphere = mix(
        vec3(0.92, 0.30, 0.055),
        vec3(1.00, 0.72, 0.38),
        pow(view_cosine, 0.32)
    );
    float faculae = penumbra * (1.0 - view_cosine) * 0.16;
    return photosphere
        * (1.18 * limb_darkening * texture_variation * spot_attenuation + faculae);
}

vec3 space_background(vec3 direction) {
    vec3 background = vec3(0.0);
    if (u_starfield_ready > 0.5) {
        vec3 stars = srgb_to_linear(texture(u_starfield, octahedral_uv(direction)).rgb);
        // Match the full-screen pass so crossing the conservative atmosphere
        // scissor boundary does not change the diffuse sky exposure.
        background += stars * 0.55;
    }
    vec3 sun_direction = normalize(u_sun_center_m);
    float sun_alignment = dot(direction, sun_direction);
    float wide_corona = smoothstep(0.985, 1.0, sun_alignment);
    float inner_corona = smoothstep(0.99965, 1.0, sun_alignment);
    float corona = wide_corona * wide_corona * wide_corona * wide_corona * 0.0015
        + inner_corona * inner_corona * 0.022;
    background += vec3(1.0, 0.55, 0.18) * corona;
    return background;
}

void main() {
    float aspect = max(u_resolution.x / max(u_resolution.y, 1.0), 0.001);
    const float tangent_half_fov = 0.7673269879789604;
    vec3 ray_direction = normalize(
        u_camera_forward
            + u_camera_right * (v_clip.x * aspect * tangent_half_fov)
            + u_camera_up * (v_clip.y * tangent_half_fov)
    );

    vec3 earth_radii = vec3(
        u_earth_equatorial_radius_m,
        u_earth_polar_radius_m,
        u_earth_equatorial_radius_m
    );
    float earth_ellipsoid_distance = ray_ellipsoid(
        ray_direction,
        u_earth_center_m,
        earth_radii
    );
    float earth_distance = intersect_terrain(ray_direction, earth_ellipsoid_distance);
    vec2 moon_interval = ray_sphere(ray_direction, u_moon_center_m, u_moon_radius_m);
    float moon_distance = first_positive_root(moon_interval.x, moon_interval.y);
    vec2 sun_interval = ray_sphere(ray_direction, u_sun_center_m, u_sun_radius_m);
    float sun_distance = first_positive_root(sun_interval.x, sun_interval.y);
    float body_distance = min(earth_distance, min(moon_distance, sun_distance));

    vec2 atmosphere_interval = ray_ellipsoid_from(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        earth_radii_at_altitude(u_atmosphere_top_m)
    );
    bool has_body = is_finite_hit(body_distance);
    bool has_atmosphere = atmosphere_interval.y > 0.0;
    if (!has_body && !has_atmosphere) {
        discard;
    }

    vec3 color;
    bool earth_is_frontmost = false;
    float earth_edge_coverage = 1.0;
    vec3 earth_edge_sky_ray = ray_direction;
    if (
        is_finite_hit(earth_distance)
            && earth_distance <= moon_distance
            && earth_distance <= sun_distance
    ) {
        earth_is_frontmost = true;
        color = shade_earth(ray_direction, earth_distance);
        vec3 edge_point = ray_direction * earth_distance - u_earth_center_m;
        vec3 edge_normal = normalize(edge_point / (earth_radii * earth_radii));
        float edge_incidence = max(dot(edge_normal, -ray_direction), 0.0);
        if (edge_incidence < 0.12) {
            vec3 scaled_earth_origin = -u_earth_center_m / earth_radii;
            vec3 scaled_earth_ray = ray_direction / earth_radii;
            float closest_earth_ray_distance = max(
                -dot(scaled_earth_origin, scaled_earth_ray)
                    / max(dot(scaled_earth_ray, scaled_earth_ray), 1.0e-20),
                0.0
            );
            float closest_terrain_clearance_m = terrain_silhouette_signed_clearance_m(
                ray_direction * closest_earth_ray_distance
            );
            float single_limb_pixel_span_m = max(
                closest_earth_ray_distance * vertical_pixel_angle(),
                1.0
            );
            float limb_pixel_span_m = single_limb_pixel_span_m * 3.0;
            earth_edge_coverage = 1.0 - smoothstep(
                -limb_pixel_span_m,
                0.0,
                closest_terrain_clearance_m
            );
            vec3 earth_center_direction = normalize(u_earth_center_m);
            vec3 center_tangent = earth_center_direction
                - ray_direction * dot(earth_center_direction, ray_direction);
            float center_tangent_length = length(center_tangent);
            if (center_tangent_length > 0.000001) {
                float pixels_inside_limb = max(
                    -closest_terrain_clearance_m / single_limb_pixel_span_m,
                    0.0
                );
                float sky_outward_angle = (pixels_inside_limb + 1.0)
                    * vertical_pixel_angle();
                earth_edge_sky_ray = normalize(
                    ray_direction
                        - center_tangent / center_tangent_length * sky_outward_angle
                );
            }
        }
    } else if (is_finite_hit(moon_distance) && moon_distance <= sun_distance) {
        color = shade_moon(ray_direction, moon_distance);
    } else if (is_finite_hit(sun_distance)) {
        color = shade_sun(ray_direction, sun_distance);
    } else {
        color = space_background(ray_direction);
    }

    vec3 cloud_scattering;
    float cloud_transmission;
    integrate_clouds(ray_direction, body_distance, cloud_scattering, cloud_transmission);
    color = color * cloud_transmission + cloud_scattering;

    vec3 atmosphere_scattering;
    vec3 atmosphere_transmission;
    float atmosphere_maximum_distance = min(
        body_distance,
        earth_ellipsoid_distance
    );
    integrate_atmosphere(
        ray_direction,
        atmosphere_maximum_distance,
        atmosphere_scattering,
        atmosphere_transmission
    );
    color = color * atmosphere_transmission + atmosphere_scattering;

    if (earth_is_frontmost && earth_edge_coverage < 0.999) {
        float sky_ellipsoid_distance = ray_ellipsoid(
            earth_edge_sky_ray,
            u_earth_center_m,
            earth_radii
        );
        float sky_terrain_distance = intersect_terrain(
            earth_edge_sky_ray,
            sky_ellipsoid_distance
        );
        vec2 sky_moon_interval = ray_sphere(
            earth_edge_sky_ray,
            u_moon_center_m,
            u_moon_radius_m
        );
        float sky_moon_distance = first_positive_root(
            sky_moon_interval.x,
            sky_moon_interval.y
        );
        vec2 sky_sun_interval = ray_sphere(
            earth_edge_sky_ray,
            u_sun_center_m,
            u_sun_radius_m
        );
        float sky_sun_distance = first_positive_root(
            sky_sun_interval.x,
            sky_sun_interval.y
        );
        float sky_body_distance = min(
            sky_terrain_distance,
            min(sky_moon_distance, sky_sun_distance)
        );
        vec3 sky_color;
        if (
            is_finite_hit(sky_terrain_distance)
                && sky_terrain_distance <= sky_moon_distance
                && sky_terrain_distance <= sky_sun_distance
        ) {
            sky_color = shade_earth(earth_edge_sky_ray, sky_terrain_distance);
        } else if (
            is_finite_hit(sky_moon_distance)
                && sky_moon_distance <= sky_sun_distance
        ) {
            sky_color = shade_moon(earth_edge_sky_ray, sky_moon_distance);
        } else if (is_finite_hit(sky_sun_distance)) {
            sky_color = shade_sun(earth_edge_sky_ray, sky_sun_distance);
        } else {
            sky_color = space_background(earth_edge_sky_ray);
        }
        vec3 sky_cloud_scattering;
        float sky_cloud_transmission;
        integrate_clouds(
            earth_edge_sky_ray,
            sky_body_distance,
            sky_cloud_scattering,
            sky_cloud_transmission
        );
        sky_color = sky_color * sky_cloud_transmission + sky_cloud_scattering;
        vec3 sky_atmosphere_scattering;
        vec3 sky_atmosphere_transmission;
        integrate_atmosphere(
            earth_edge_sky_ray,
            min(sky_body_distance, sky_ellipsoid_distance),
            sky_atmosphere_scattering,
            sky_atmosphere_transmission
        );
        sky_color = sky_color * sky_atmosphere_transmission
            + sky_atmosphere_scattering;
        color = mix(sky_color, color, earth_edge_coverage);
    }

    color = linear_to_srgb(aces_tone_map(max(color, vec3(0.0))));
    float vignette = 1.0 - 0.045 * dot(v_clip * 0.55, v_clip * 0.55);
    out_color = vec4(clamp(color * vignette, 0.0, 1.0), 1.0);
}
