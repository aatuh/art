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

float terrainHeightM(vec3 point) {
    if (u_surface_ready < 0.5) {
        return 0.0;
    }
    vec3 direction = earthFixedDirection(point);
    vec4 surface_sample = texture(u_surface, earthUv(point));
    float land = smoothstep(0.08, 0.72, surface_sample.a);
    if (land <= 0.001) {
        return 0.0;
    }
    float longitude = atan(direction.z, direction.x);
    float latitude = asin(clamp(direction.y, -1.0, 1.0));
    float rough = valueNoise(direction * 92.0 + vec3(7.0, -3.0, 11.0));
    float andes = exp(-pow(wrappedLongitudeDistance(longitude, radians(-71.0)) / radians(4.5), 2.0))
        * smoothstep(radians(-55.0), radians(-8.0), latitude)
        * (1.0 - smoothstep(radians(10.0), radians(20.0), latitude));
    float himalaya = exp(-pow(wrappedLongitudeDistance(longitude, radians(84.0)) / radians(16.0), 2.0))
        * exp(-pow((latitude - radians(30.0)) / radians(5.5), 2.0));
    float rockies = exp(-pow(wrappedLongitudeDistance(longitude, radians(-113.0)) / radians(8.0), 2.0))
        * exp(-pow((latitude - radians(43.0)) / radians(18.0), 2.0));
    float belt = max(andes * 0.82, max(himalaya, rockies * 0.62));
    float base_relief = pow(saturate((rough - 0.42) / 0.58), 2.2) * 2.1e3;
    return land * min(MAX_TERRAIN_M, base_relief + belt * 6.5e3);
}

float intersectTerrain(vec3 ray_origin, vec3 ray_direction) {
    float base_distance = nearestPositiveIntersection(
        rayEllipsoid(ray_origin, ray_direction, u_earth_center, earthRadii(0.0))
    );
    if (base_distance >= INF * 0.5 || u_surface_ready < 0.5 || u_camera_altitude_m > 2.0e6) {
        return base_distance;
    }
    vec2 outer = rayEllipsoid(
        ray_origin,
        ray_direction,
        u_earth_center,
        earthRadii(MAX_TERRAIN_M)
    );
    float segment_start = max(0.0, outer.x);
    float segment_end = min(base_distance, outer.y);
    if (segment_end <= segment_start) {
        return base_distance;
    }
    float previous_distance = segment_start;
    for (int step_index = 0; step_index < TERRAIN_STEPS; ++step_index) {
        float fraction = float(step_index + 1) / float(TERRAIN_STEPS);
        float distance_along_ray = mix(segment_start, segment_end, fraction);
        vec3 point = ray_origin + ray_direction * distance_along_ray;
        float signed_height = altitudeAboveEllipsoid(point) - terrainHeightM(point);
        if (signed_height <= 0.0) {
            float low = previous_distance;
            float high = distance_along_ray;
            for (int refinement = 0; refinement < 3; ++refinement) {
                float middle = 0.5 * (low + high);
                vec3 middle_point = ray_origin + ray_direction * middle;
                if (altitudeAboveEllipsoid(middle_point) - terrainHeightM(middle_point) <= 0.0) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return high;
        }
        previous_distance = distance_along_ray;
    }
    return base_distance;
}

float ozoneDensity(float altitude_m) {
    return saturate(1.0 - abs(altitude_m - 25.0e3) / 15.0e3);
}

float henyeyGreenstein(float cosine_theta, float anisotropy) {
    float anisotropy_sq = anisotropy * anisotropy;
    float denominator = max(1.0e-3, 1.0 + anisotropy_sq - 2.0 * anisotropy * cosine_theta);
    return (1.0 - anisotropy_sq) / (4.0 * PI * pow(denominator, 1.5));
}

float circleOverlap(float light_radius, float blocker_radius, float separation) {
    if (separation >= light_radius + blocker_radius) {
        return 0.0;
    }
    if (separation <= abs(blocker_radius - light_radius)) {
        float covered_radius = min(light_radius, blocker_radius);
        return PI * covered_radius * covered_radius;
    }
    float light_term = acos(clamp(
        (separation * separation + light_radius * light_radius - blocker_radius * blocker_radius)
            / (2.0 * separation * light_radius),
        -1.0,
        1.0
    ));
    float blocker_term = acos(clamp(
        (separation * separation + blocker_radius * blocker_radius - light_radius * light_radius)
            / (2.0 * separation * blocker_radius),
        -1.0,
        1.0
    ));
    float radical = max(
        0.0,
        (-separation + light_radius + blocker_radius)
            * (separation + light_radius - blocker_radius)
