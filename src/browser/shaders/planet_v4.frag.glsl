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

const float PI = 3.141592653589793;
const float TAU = 6.283185307179586;
const float INF = 1.0e30;
const float MAX_TERRAIN_M = 8500.0;
const float CLOUD_BASE_M = 1500.0;
const float CLOUD_TOP_M = 12000.0;
const float RAYLEIGH_SCALE_HEIGHT_M = 8000.0;
const float MIE_SCALE_HEIGHT_M = 1200.0;
const vec3 BETA_R = vec3(5.802e-6, 13.558e-6, 33.100e-6);
const vec3 BETA_M_SCATTER = vec3(3.996e-6);
const vec3 BETA_M_EXTINCT = vec3(4.440e-6);
const vec3 BETA_OZONE = vec3(0.650e-6, 1.881e-6, 0.085e-6);

float saturate(float value) {
    return clamp(value, 0.0, 1.0);
}

float hash31(vec3 p) {
    p = fract(p * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

float noise3(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(hash31(i), hash31(i + vec3(1.0, 0.0, 0.0)), f.x),
            mix(hash31(i + vec3(0.0, 1.0, 0.0)), hash31(i + vec3(1.0, 1.0, 0.0)), f.x),
            f.y
        ),
        mix(
            mix(hash31(i + vec3(0.0, 0.0, 1.0)), hash31(i + vec3(1.0, 0.0, 1.0)), f.x),
            mix(hash31(i + vec3(0.0, 1.0, 1.0)), hash31(i + vec3(1.0, 1.0, 1.0)), f.x),
            f.y
        ),
        f.z
    );
}

float fbm(vec3 p) {
    float result = 0.0;
    float amplitude = 0.5;
    for (int octave = 0; octave < 4; ++octave) {
        result += noise3(p) * amplitude;
        p = p * 2.031 + vec3(7.1, 11.7, 3.9);
        amplitude *= 0.5;
    }
    return result;
}

vec3 rotate_y(vec3 p, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

vec2 ray_sphere(vec3 origin, vec3 direction, vec3 center, float radius) {
    vec3 to_center = center - origin;
    float projected = dot(to_center, direction);
    vec3 perpendicular = cross(to_center, direction);
    float half_chord_sq = radius * radius - dot(perpendicular, perpendicular);
    if (half_chord_sq < 0.0) {
        return vec2(-1.0);
    }
    float half_chord = sqrt(half_chord_sq);
    return vec2(projected - half_chord, projected + half_chord);
}

vec2 ray_ellipsoid(vec3 origin, vec3 direction, vec3 center, vec3 radii) {
    vec3 scaled_origin = (origin - center) / radii;
    vec3 scaled_direction = direction / radii;
    float a = dot(scaled_direction, scaled_direction);
    float b = dot(scaled_origin, scaled_direction);
    float c = dot(scaled_origin, scaled_origin) - 1.0;
    float discriminant = b * b - a * c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    return vec2((-b - root) / a, (-b + root) / a);
}

float hit_distance(vec2 interval) {
    if (interval.x > 0.0) {
        return interval.x;
    }
    if (interval.y > 0.0) {
        return interval.y;
    }
    return INF;
}

vec3 earth_radii(float altitude_m) {
    return vec3(
        u_earth_equatorial_radius_m + altitude_m,
        u_earth_polar_radius_m + altitude_m,
        u_earth_equatorial_radius_m + altitude_m
    );
}

float ellipsoid_surface_radius(vec3 direction) {
    vec3 d = normalize(direction);
    float a2 = u_earth_equatorial_radius_m * u_earth_equatorial_radius_m;
    float b2 = u_earth_polar_radius_m * u_earth_polar_radius_m;
    float inverse_r2 = (d.x * d.x + d.z * d.z) / a2 + d.y * d.y / b2;
    return inversesqrt(inverse_r2);
}

float altitude_above_ellipsoid(vec3 point) {
    vec3 local = point - u_earth_center_m;
    return length(local) - ellipsoid_surface_radius(local);
}

vec3 geometric_earth_normal(vec3 point) {
    vec3 local = point - u_earth_center_m;
    vec3 radii = earth_radii(0.0);
    return normalize(local / (radii * radii));
}

vec3 earth_fixed_direction_from_world(vec3 world_direction) {
    return rotate_y(normalize(world_direction), -u_earth_rotation);
}

vec3 earth_fixed_direction(vec3 point) {
    return earth_fixed_direction_from_world(point - u_earth_center_m);
}

vec2 earth_uv_from_local(vec3 local_direction) {
    float longitude = atan(local_direction.z, local_direction.x);
    float latitude = asin(clamp(local_direction.y, -1.0, 1.0));
    return vec2(
        fract(longitude / TAU + 0.5),
        clamp(0.5 - latitude / PI, 0.001, 0.999)
    );
}

vec4 earth_reference(vec3 local_direction) {
    return texture(u_surface, earth_uv_from_local(local_direction));
}

float land_reference(vec3 local_direction) {
    return texture(u_land_mask, earth_uv_from_local(local_direction)).a;
}

float land_mask(vec3 local_direction) {
    float reference = land_reference(local_direction);
    float edge = 1.0 - abs(reference * 2.0 - 1.0);
    float breakup = (fbm(local_direction * 90.0 + vec3(3.0, 7.0, -4.0)) - 0.5) * 0.16 * edge;
    return smoothstep(0.32, 0.68, reference + breakup);
}

float wrapped_longitude_distance(float longitude, float center) {
    return mod(longitude - center + PI, TAU) - PI;
}

float terrain_height_local(vec3 local_direction) {
    float land = land_mask(local_direction);
    if (land < 0.01) {
        return 0.0;
    }
    float longitude = atan(local_direction.z, local_direction.x);
    float latitude = asin(clamp(local_direction.y, -1.0, 1.0));
    float rough = fbm(local_direction * 52.0 + vec3(4.0, -9.0, 2.0));
    float andes = exp(-pow(wrapped_longitude_distance(longitude, radians(-71.0)) / radians(4.5), 2.0))
        * smoothstep(radians(-55.0), radians(-8.0), latitude)
        * (1.0 - smoothstep(radians(10.0), radians(20.0), latitude));
    float himalaya = exp(-pow(wrapped_longitude_distance(longitude, radians(84.0)) / radians(16.0), 2.0))
        * exp(-pow((latitude - radians(30.0)) / radians(5.5), 2.0));
    float rockies = exp(-pow(wrapped_longitude_distance(longitude, radians(-113.0)) / radians(8.0), 2.0))
        * exp(-pow((latitude - radians(43.0)) / radians(18.0), 2.0));
    float mountain_belt = max(andes * 0.82, max(himalaya, rockies * 0.62));
    float base_relief = pow(saturate((rough - 0.42) / 0.58), 2.2) * 2100.0;
    return land * min(MAX_TERRAIN_M, base_relief + mountain_belt * 6500.0);
}

float terrain_height_world_direction(vec3 world_direction) {
    return terrain_height_local(earth_fixed_direction_from_world(world_direction));
}

float intersect_terrain(vec3 origin, vec3 direction) {
    float base_distance = hit_distance(
        ray_ellipsoid(origin, direction, u_earth_center_m, earth_radii(0.0))
    );
    if (base_distance >= INF * 0.5 || u_camera_altitude_m > 2000000.0) {
        return base_distance;
    }

    vec2 outer = ray_ellipsoid(origin, direction, u_earth_center_m, earth_radii(MAX_TERRAIN_M));
    if (outer.x < 0.0 && outer.y < 0.0) {
        return base_distance;
    }
    float start_distance = max(0.0, outer.x);
    float end_distance = min(base_distance, outer.y);
    if (end_distance <= start_distance) {
        return base_distance;
    }

    float previous_distance = start_distance;
    for (int step_index = 0; step_index < 12; ++step_index) {
        float fraction = float(step_index + 1) / 12.0;
        float distance_along_ray = mix(start_distance, end_distance, fraction);
        vec3 point = origin + direction * distance_along_ray;
        float signed_height = altitude_above_ellipsoid(point)
            - terrain_height_world_direction(point - u_earth_center_m);
        if (signed_height <= 0.0) {
            float low = previous_distance;
            float high = distance_along_ray;
            for (int refinement = 0; refinement < 4; ++refinement) {
                float middle = 0.5 * (low + high);
                vec3 middle_point = origin + direction * middle;
                float middle_height = altitude_above_ellipsoid(middle_point)
                    - terrain_height_world_direction(middle_point - u_earth_center_m);
                if (middle_height <= 0.0) {
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

vec3 terrain_normal(vec3 point, float land) {
    vec3 geometric = geometric_earth_normal(point);
    if (land < 0.02) {
        return geometric;
    }
    vec3 radial = normalize(point - u_earth_center_m);
    vec3 tangent = cross(abs(radial.y) < 0.95 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0), radial);
    tangent = normalize(tangent);
    vec3 bitangent = normalize(cross(radial, tangent));
    const float epsilon = 0.00035;
    float h_t0 = terrain_height_world_direction(normalize(radial - tangent * epsilon));
    float h_t1 = terrain_height_world_direction(normalize(radial + tangent * epsilon));
    float h_b0 = terrain_height_world_direction(normalize(radial - bitangent * epsilon));
    float h_b1 = terrain_height_world_direction(normalize(radial + bitangent * epsilon));
    float denominator = 2.0 * u_earth_equatorial_radius_m * epsilon;
    float slope_t = (h_t1 - h_t0) / denominator;
    float slope_b = (h_b1 - h_b0) / denominator;
    return normalize(geometric - tangent * slope_t - bitangent * slope_b);
}

vec3 ocean_normal(vec3 point, vec3 geometric) {
    vec3 radial = normalize(point - u_earth_center_m);
    vec3 local = earth_fixed_direction(point);
    vec3 tangent = cross(abs(radial.y) < 0.95 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0), radial);
    tangent = normalize(tangent);
    vec3 bitangent = normalize(cross(radial, tangent));
    float time = u_time;
    float wave_a = sin(local.x * 1100.0 + local.z * 740.0 + time * 0.85);
    float wave_b = sin(local.z * 1550.0 - local.x * 430.0 + time * 1.17);
    float wave_c = noise3(local * 950.0 + vec3(time * 0.02, 0.0, -time * 0.015)) * 2.0 - 1.0;
    return normalize(geometric + tangent * (wave_a + wave_c) * 0.012 + bitangent * wave_b * 0.010);
}

float ozone_density(float altitude_m) {
    return saturate(1.0 - abs(altitude_m - 25000.0) / 15000.0);
}

vec3 atmosphere_density(vec3 point) {
    float altitude = max(altitude_above_ellipsoid(point), 0.0);
    return vec3(
        exp(-altitude / RAYLEIGH_SCALE_HEIGHT_M),
        exp(-altitude / MIE_SCALE_HEIGHT_M),
        ozone_density(altitude)
    );
}

vec3 atmosphere_extinction(vec3 density) {
    return BETA_R * density.x + BETA_M_EXTINCT * density.y + BETA_OZONE * density.z;
}

float rayleigh_phase(float cosine_theta) {
    return 3.0 / (16.0 * PI) * (1.0 + cosine_theta * cosine_theta);
}

float mie_phase(float cosine_theta) {
    const float g = 0.76;
    float g2 = g * g;
    return 3.0 / (8.0 * PI)
        * ((1.0 - g2) * (1.0 + cosine_theta * cosine_theta))
        / ((2.0 + g2) * pow(max(0.001, 1.0 + g2 - 2.0 * g * cosine_theta), 1.5));
}

vec3 light_optical_depth(vec3 point, vec3 light_direction) {
    vec2 atmosphere_hit = ray_sphere(
        point,
        light_direction,
        u_earth_center_m,
        u_earth_equatorial_radius_m + u_atmosphere_top_m
    );
    float exit_distance = atmosphere_hit.y;
    if (exit_distance <= 0.0) {
        return vec3(0.0);
    }
    float ground_distance = hit_distance(
        ray_ellipsoid(point + light_direction * 10.0, light_direction, u_earth_center_m, earth_radii(0.0))
    );
    if (ground_distance < exit_distance) {
        return vec3(1000000.0);
    }
    vec3 depth = vec3(0.0);
    float step_length = exit_distance / 4.0;
    for (int sample_index = 0; sample_index < 4; ++sample_index) {
        float distance_along_ray = (float(sample_index) + 0.5) * step_length;
        depth += atmosphere_density(point + light_direction * distance_along_ray) * step_length;
    }
    return depth;
}

void integrate_atmosphere(
    vec3 ray_direction,
    float start_distance,
    float end_distance,
    out vec3 scattering,
    out vec3 transmission
) {
    scattering = vec3(0.0);
    transmission = vec3(1.0);
    if (end_distance <= start_distance) {
        return;
    }

    float step_length = (end_distance - start_distance) / 10.0;
    vec3 view_depth = vec3(0.0);
    vec3 rayleigh_sum = vec3(0.0);
    vec3 mie_sum = vec3(0.0);
    for (int sample_index = 0; sample_index < 10; ++sample_index) {
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;
        vec3 point = ray_direction * distance_along_ray;
        vec3 density = atmosphere_density(point);
        view_depth += density * step_length;
        vec3 light_direction = normalize(u_sun_center_m - point);
        vec3 sun_depth = light_optical_depth(point, light_direction);
        vec3 attenuation = exp(
            -(BETA_R * (view_depth.x + sun_depth.x)
                + BETA_M_EXTINCT * (view_depth.y + sun_depth.y)
                + BETA_OZONE * (view_depth.z + sun_depth.z))
        );
        rayleigh_sum += attenuation * density.x * step_length;
        mie_sum += attenuation * density.y * step_length;
    }

    vec3 midpoint = ray_direction * ((start_distance + end_distance) * 0.5);
    vec3 light_direction = normalize(u_sun_center_m - midpoint);
    float cosine_theta = dot(ray_direction, light_direction);
    scattering = 18.0
        * (rayleigh_sum * BETA_R * rayleigh_phase(cosine_theta)
            + mie_sum * BETA_M_SCATTER * mie_phase(cosine_theta));
    transmission = exp(-atmosphere_extinction(view_depth));
}

float cloud_vertical_profile(float altitude_m) {
    float base = smoothstep(CLOUD_BASE_M, CLOUD_BASE_M + 1800.0, altitude_m);
    float top = 1.0 - smoothstep(CLOUD_TOP_M - 3500.0, CLOUD_TOP_M, altitude_m);
    return base * top;
}

float cloud_density(vec3 point) {
    float altitude = altitude_above_ellipsoid(point);
    float vertical = cloud_vertical_profile(altitude);
    if (vertical <= 0.0) {
        return 0.0;
    }

    vec3 local = earth_fixed_direction(point);
    float weather_time = u_time * 0.018;
    vec3 wind_direction = rotate_y(local, u_time * 0.00010);
    float macro = fbm(wind_direction * 4.2 + vec3(weather_time * 0.07, -weather_time * 0.03, weather_time * 0.05));
    float weather = fbm(wind_direction * 1.55 + vec3(-weather_time * 0.018, 7.0, weather_time * 0.012));
    float detail = noise3(wind_direction * 34.0 + vec3(weather_time * 0.31, altitude * 0.00055, -weather_time * 0.24));
    float threshold = mix(0.66, 0.53, weather);
    float body = smoothstep(threshold, threshold + 0.17, macro + detail * 0.16);
    return body * vertical;
}

float cloud_light_transmission(vec3 point, vec3 light_direction) {
    float optical = 0.0;
    for (int step_index = 0; step_index < 3; ++step_index) {
        float distance_along_light = (float(step_index) + 1.0) * 3500.0;
        optical += cloud_density(point + light_direction * distance_along_light);
    }
    return exp(-optical * 0.85);
}

void integrate_clouds(
    vec3 ray_direction,
    float body_distance,
    out vec3 cloud_scattering,
    out float cloud_transmission
) {
    cloud_scattering = vec3(0.0);
    cloud_transmission = 1.0;

    vec2 cloud_hit = ray_sphere(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        u_earth_equatorial_radius_m + CLOUD_TOP_M
    );
    if (cloud_hit.y <= 0.0) {
        return;
    }

    float start_distance = max(cloud_hit.x, 0.0);
    float end_distance = min(cloud_hit.y, body_distance);
    if (end_distance <= start_distance) {
        return;
    }

    float step_length = (end_distance - start_distance) / 8.0;
    for (int sample_index = 0; sample_index < 8; ++sample_index) {
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;
        vec3 point = ray_direction * distance_along_ray;
        float density = cloud_density(point);
        if (density <= 0.001) {
            continue;
        }
        vec3 light_direction = normalize(u_sun_center_m - point);
        float light_visibility = cloud_light_transmission(point, light_direction);
        float forward_scatter = 0.35 + 0.65 * pow(max(dot(ray_direction, light_direction), 0.0), 8.0);
        vec3 ambient = vec3(0.18, 0.22, 0.28);
        vec3 sunlit = vec3(1.0, 0.97, 0.91) * (0.55 + forward_scatter * 0.85);
        vec3 sample_color = mix(ambient, sunlit, light_visibility);
        float optical = density * step_length / 6500.0;
        float alpha = 1.0 - exp(-optical);
        cloud_scattering += cloud_transmission * alpha * sample_color;
        cloud_transmission *= 1.0 - alpha;
        if (cloud_transmission < 0.02) {
            break;
        }
    }
}

float cloud_shadow(vec3 surface_point, vec3 light_direction) {
    float optical = 0.0;
    for (int sample_index = 0; sample_index < 4; ++sample_index) {
        float distance_along_light = 2500.0 + float(sample_index) * 3000.0;
        optical += cloud_density(surface_point + light_direction * distance_along_light);
    }
    return exp(-optical * 0.52);
}

float solar_visibility(vec3 point, vec3 blocker_center, float blocker_radius) {
    vec3 to_sun = u_sun_center_m - point;
    vec3 to_blocker = blocker_center - point;
    float sun_distance = length(to_sun);
    float blocker_distance = length(to_blocker);
    if (blocker_distance <= blocker_radius || blocker_distance >= sun_distance) {
        return 1.0;
    }
    float sun_angle = asin(clamp(u_sun_radius_m / sun_distance, 0.0, 1.0));
    float blocker_angle = asin(clamp(blocker_radius / blocker_distance, 0.0, 1.0));
    float separation = acos(clamp(dot(to_sun, to_blocker) / (sun_distance * blocker_distance), -1.0, 1.0));
    float outer = sun_angle + blocker_angle;
    float inner = max(blocker_angle - sun_angle, 0.0);
    return smoothstep(inner, outer, separation);
}

vec3 shade_earth(vec3 point, vec3 ray_direction) {
    vec3 local = earth_fixed_direction(point);
    vec4 reference = earth_reference(local);
    float land_reference_value = land_reference(local);
    float land = land_mask(local);
    vec3 geometric = geometric_earth_normal(point);
    vec3 normal = mix(ocean_normal(point, geometric), terrain_normal(point, land), land);

    vec3 base_reference = pow(max(reference.rgb, vec3(0.001)), vec3(2.2));
    float macro_detail = fbm(local * 65.0 + vec3(5.0, -8.0, 2.0));
    float micro_detail = noise3(local * 420.0 + vec3(-7.0, 3.0, 11.0));

    vec3 ocean_color = mix(
        vec3(0.0025, 0.012, 0.035),
        vec3(0.012, 0.10, 0.18),
        saturate(reference.b * 1.8 + macro_detail * 0.25)
    );
    vec3 land_color = base_reference * mix(0.72, 1.28, macro_detail);
    land_color *= mix(0.88, 1.12, micro_detail);
    vec3 base_color = mix(ocean_color, land_color, land);

    float latitude = abs(asin(clamp(local.y, -1.0, 1.0))) / (0.5 * PI);
    float elevation = terrain_height_local(local) / MAX_TERRAIN_M;
    float snow = smoothstep(0.68, 0.96, latitude + elevation * 0.38);
    base_color = mix(base_color, vec3(0.82, 0.88, 0.92), snow * land);

    vec3 light_direction = normalize(u_sun_center_m - point);
    vec3 view_direction = -ray_direction;
    float n_dot_l = max(dot(normal, light_direction), 0.0);
    float visibility = solar_visibility(point, u_moon_center_m, u_moon_radius_m);
    float cloud_light = cloud_shadow(point + geometric * 20.0, light_direction);
    float direct = n_dot_l * visibility * cloud_light;

    vec3 sunlight_color = mix(
        vec3(1.0, 0.46, 0.18),
        vec3(1.0, 0.98, 0.90),
        smoothstep(0.0, 0.18, n_dot_l)
    );
    vec3 color = base_color * (0.008 + direct * sunlight_color * 1.22);

    vec3 half_direction = normalize(light_direction + view_direction);
    float fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
    float ocean_specular = pow(max(dot(normal, half_direction), 0.0), 520.0)
        * (1.0 - land) * direct;
    color += sunlight_color * (ocean_specular * 5.4 + fresnel * (1.0 - land) * n_dot_l * 0.08);

    float night = 1.0 - smoothstep(-0.06, 0.10, dot(geometric, light_direction));
    float settlement = pow(noise3(local * 230.0 + vec3(7.0, 19.0, 3.0)), 18.0)
        * land * (1.0 - snow);
    color += vec3(1.0, 0.42, 0.10) * settlement * night * 1.5;

    float coast_edge = 1.0 - abs(land_reference_value * 2.0 - 1.0);
    float foam = coast_edge * (1.0 - land) * saturate(0.4 + 0.6 * sin(u_time * 1.3 + local.x * 900.0));
    color += vec3(0.55, 0.70, 0.78) * foam * n_dot_l * 0.18;

    return color;
}

vec3 shade_moon(vec3 point, vec3 ray_direction) {
    vec3 normal = normalize(point - u_moon_center_m);
    vec3 light_direction = normalize(u_sun_center_m - point);
    vec3 earth_direction = normalize(u_earth_center_m - point);
    float maria = fbm(normal * 5.5 + vec3(1.0, -4.0, 7.0));
    float crater = pow(noise3(normal * 72.0 + vec3(8.0, -3.0, 5.0)), 6.0);
    vec3 albedo = mix(vec3(0.055, 0.052, 0.049), vec3(0.24, 0.225, 0.205), maria);
    albedo *= 1.0 - crater * 0.42;
    float sunlight = max(dot(normal, light_direction), 0.0);
    float visibility = solar_visibility(point, u_earth_center_m, u_earth_equatorial_radius_m);
    float earthshine = max(dot(normal, earth_direction), 0.0) * 0.028;
    float opposition = pow(max(dot(normal, -ray_direction), 0.0), 10.0) * sunlight * 0.08;
    return albedo * (0.003 + sunlight * visibility * 1.2 + earthshine + opposition);
}

vec3 shade_sun(vec3 point, vec3 ray_direction) {
    vec3 normal = normalize(point - u_sun_center_m);
    float limb = max(dot(normal, -ray_direction), 0.0);
    float granulation = fbm(normal * 190.0 + vec3(u_time * 0.006, 0.0, 0.0));
    vec3 color = mix(vec3(1.0, 0.35, 0.045), vec3(1.0, 0.96, 0.74), 0.30 + 0.70 * limb);
    return color * (24.0 + granulation * 12.0) * (0.30 + 0.70 * limb);
}

vec3 space_background(vec3 direction) {
    vec3 primary_cell = floor(direction * 1400.0);
    vec3 secondary_cell = floor(direction * 2900.0);
    float primary = smoothstep(0.99945, 1.0, hash31(primary_cell));
    float secondary = smoothstep(0.99976, 1.0, hash31(secondary_cell));
    float temperature = hash31(primary_cell + vec3(19.0, 3.0, 7.0));
    vec3 star_color = mix(vec3(1.0, 0.76, 0.54), vec3(0.58, 0.74, 1.0), temperature)
        * primary * 1.4;
    star_color += vec3(0.80, 0.87, 1.0) * secondary * 0.52;

    vec3 galactic_axis = normalize(vec3(0.24, 0.91, -0.34));
    float galactic_plane = exp(-pow(dot(direction, galactic_axis) * 9.0, 2.0));
    float dust = fbm(direction * 5.0 + vec3(11.0, -4.0, 2.0));
    vec3 milky_way = vec3(0.08, 0.11, 0.18)
        * galactic_plane * smoothstep(0.32, 0.82, dust) * 0.25;

    vec3 sun_direction = normalize(u_sun_center_m);
    float sun_angle = acos(clamp(dot(direction, sun_direction), -1.0, 1.0));
    float solar_halo = exp(-sun_angle * 420.0) * 0.16 + exp(-sun_angle * 48.0) * 0.007;
    return vec3(0.00006, 0.00009, 0.00018) + star_color + milky_way
        + vec3(1.0, 0.58, 0.24) * solar_halo;
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
    vec2 screen = v_clip;
    screen.x *= u_resolution.x / u_resolution.y;
    float focal_length = 1.0 / tan(radians(60.0) * 0.5);
    vec3 ray_direction = normalize(
        u_camera_forward * focal_length + u_camera_right * screen.x + u_camera_up * screen.y
    );

    float earth_distance = intersect_terrain(vec3(0.0), ray_direction);
    float moon_distance = hit_distance(
        ray_sphere(vec3(0.0), ray_direction, u_moon_center_m, u_moon_radius_m)
    );
    float sun_distance = hit_distance(
        ray_sphere(vec3(0.0), ray_direction, u_sun_center_m, u_sun_radius_m)
    );
    float body_distance = min(earth_distance, min(moon_distance, sun_distance));

    vec3 color = space_background(ray_direction);
    if (body_distance < INF * 0.5) {
        vec3 point = ray_direction * body_distance;
        if (earth_distance <= moon_distance && earth_distance <= sun_distance) {
            color = shade_earth(point, ray_direction);
        } else if (moon_distance <= sun_distance) {
            color = shade_moon(point, ray_direction);
        } else {
            color = shade_sun(point, ray_direction);
        }
    }

    vec2 atmosphere_interval = ray_sphere(
        vec3(0.0),
        ray_direction,
        u_earth_center_m,
        u_earth_equatorial_radius_m + u_atmosphere_top_m
    );
    float atmosphere_start = max(atmosphere_interval.x, 0.0);
    float atmosphere_end = min(atmosphere_interval.y, body_distance);
    if (atmosphere_interval.y > 0.0 && atmosphere_end > atmosphere_start) {
        vec3 scattering;
        vec3 transmission;
        integrate_atmosphere(
            ray_direction,
            atmosphere_start,
            atmosphere_end,
            scattering,
            transmission
        );
        color = color * transmission + scattering;
    }

    vec3 cloud_scattering;
    float cloud_transmission;
    integrate_clouds(ray_direction, body_distance, cloud_scattering, cloud_transmission);
    color = color * cloud_transmission + cloud_scattering;

    color = aces_tone_map(color);
    color = pow(color, vec3(1.0 / 2.2));
    float vignette = 1.0 - 0.08 * dot(v_clip * 0.55, v_clip * 0.55);
    out_color = vec4(color * vignette, 1.0);
}
