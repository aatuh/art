#version 300 es
precision highp float;

in vec2 v_clip;
out vec4 out_color;

uniform vec2 u_resolution;
uniform vec3 u_camera_forward;
uniform vec3 u_camera_right;
uniform vec3 u_camera_up;
uniform vec3 u_earth_center;
uniform vec3 u_moon_center;
uniform vec3 u_sun_center;
uniform float u_earth_polar_radius;
uniform float u_atmosphere_radius;
uniform float u_moon_radius;
uniform float u_sun_radius;
uniform float u_earth_rotation;
uniform float u_time;

const float PI = 3.141592653589793;
const float TAU = 6.283185307179586;
const float INF = 1.0e30;
const vec3 BETA_R = vec3(0.035, 0.082, 0.195);
const vec3 BETA_M = vec3(0.045);
const float RAYLEIGH_HEIGHT = 0.001255;
const float MIE_HEIGHT = 0.000188;

float hash21(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
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
    for (int octave = 0; octave < 5; octave++) {
        result += noise3(p) * amplitude;
        p = p * 2.031 + vec3(7.1, 11.7, 3.9);
        amplitude *= 0.5;
    }
    return result;
}

float ridged(vec3 p) {
    float n = 1.0 - abs(2.0 * fbm(p) - 1.0);
    return n * n;
}

vec3 rotate_y(vec3 p, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

vec2 ray_sphere(vec3 origin, vec3 direction, vec3 center, float radius) {
    vec3 offset = origin - center;
    float b = dot(offset, direction);
    float c = dot(offset, offset) - radius * radius;
    float discriminant = b * b - c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    return vec2(-b - root, -b + root);
}

vec2 ray_ellipsoid(vec3 origin, vec3 direction) {
    vec3 radii = vec3(1.0, u_earth_polar_radius, 1.0);
    vec3 offset = (origin - u_earth_center) / radii;
    vec3 scaled_direction = direction / radii;
    float a = dot(scaled_direction, scaled_direction);
    float b = 2.0 * dot(offset, scaled_direction);
    float c = dot(offset, offset) - 1.0;
    float discriminant = b * b - 4.0 * a * c;
    if (discriminant < 0.0) {
        return vec2(-1.0);
    }
    float root = sqrt(discriminant);
    return vec2((-b - root) / (2.0 * a), (-b + root) / (2.0 * a));
}

float hit_distance(vec2 interval) {
    if (interval.x > 0.000001) {
        return interval.x;
    }
    if (interval.y > 0.000001) {
        return interval.y;
    }
    return INF;
}

float wrap_pi(float value) {
    return mod(value + PI, TAU) - PI;
}

float ellipse_field(vec2 longitude_latitude, vec2 center, vec2 radii, float angle) {
    vec2 point = vec2(
        wrap_pi(longitude_latitude.x - center.x) * cos(center.y),
        longitude_latitude.y - center.y
    );
    float c = cos(angle);
    float s = sin(angle);
    point = mat2(c, -s, s, c) * point;
    return 1.0 - length(point / radii);
}

float continent_field(vec2 ll, vec3 direction) {
    float field = -2.0;
    field = max(field, ellipse_field(ll, radians(vec2(-112.0, 49.0)), radians(vec2(36.0, 25.0)), -0.20));
    field = max(field, ellipse_field(ll, radians(vec2(-82.0, 37.0)), radians(vec2(24.0, 19.0)), 0.18));
    field = max(field, ellipse_field(ll, radians(vec2(-61.0, -16.0)), radians(vec2(17.0, 32.0)), -0.18));
    field = max(field, ellipse_field(ll, radians(vec2(21.0, 6.0)), radians(vec2(21.0, 31.0)), 0.04));
    field = max(field, ellipse_field(ll, radians(vec2(54.0, 51.0)), radians(vec2(70.0, 23.0)), -0.03));
    field = max(field, ellipse_field(ll, radians(vec2(105.0, 30.0)), radians(vec2(45.0, 22.0)), 0.05));
    field = max(field, ellipse_field(ll, radians(vec2(137.0, -25.0)), radians(vec2(20.0, 14.0)), -0.08));
    field = max(field, ellipse_field(ll, radians(vec2(-42.0, 72.0)), radians(vec2(10.0, 14.0)), 0.12));
    field = max(field, (-ll.y - radians(67.0)) / radians(9.0));

    float hudson = smoothstep(0.0, 0.55, ellipse_field(ll, radians(vec2(-84.0, 58.0)), radians(vec2(9.0, 7.0)), 0.0));
    float gulf = smoothstep(0.0, 0.65, ellipse_field(ll, radians(vec2(-90.0, 24.0)), radians(vec2(10.0, 7.0)), 0.0));
    float mediterranean = smoothstep(0.0, 0.75, ellipse_field(ll, radians(vec2(18.0, 36.0)), radians(vec2(17.0, 4.0)), 0.0));
    field -= hudson * 0.75 + gulf * 0.82 + mediterranean * 0.72;

    field += (fbm(direction * 3.4 + vec3(4.0, -2.0, 8.0)) - 0.5) * 0.44;
    field += (fbm(direction * 17.0 - vec3(9.0, 3.0, 1.0)) - 0.5) * 0.11;
    return field;
}

float cloud_density(vec3 local_direction) {
    vec3 moving = rotate_y(local_direction, u_time * 0.000018);
    float broad = fbm(moving * 4.2 + vec3(2.0, 11.0, -5.0));
    float detail = fbm(moving * 17.0 - vec3(7.0, 4.0, 13.0));
    float bands = 0.06 * sin(local_direction.y * 28.0 + fbm(moving * 2.0) * 8.0);
    return smoothstep(0.57, 0.72, broad + detail * 0.22 + bands);
}

vec2 atmosphere_density(vec3 point) {
    float height = max(length(point - u_earth_center) - 1.0, 0.0);
    return vec2(exp(-height / RAYLEIGH_HEIGHT), exp(-height / MIE_HEIGHT));
}

vec2 light_optical_depth(vec3 point, vec3 light_direction) {
    vec2 atmosphere_hit = ray_sphere(point, light_direction, u_earth_center, u_atmosphere_radius);
    float distance_to_exit = atmosphere_hit.y;
    if (distance_to_exit <= 0.0) {
        return vec2(0.0);
    }
    vec2 ground_hit = ray_ellipsoid(point + light_direction * 0.00001, light_direction);
    if (hit_distance(ground_hit) < distance_to_exit) {
        return vec2(10000.0);
    }

    vec2 depth = vec2(0.0);
    float step_length = distance_to_exit / 6.0;
    for (int sample_index = 0; sample_index < 6; sample_index++) {
        float distance_along_ray = (float(sample_index) + 0.5) * step_length;
        depth += atmosphere_density(point + light_direction * distance_along_ray) * step_length;
    }
    return depth;
}

float rayleigh_phase(float cosine) {
    return 0.0596831 * (1.0 + cosine * cosine);
}

float mie_phase(float cosine) {
    const float anisotropy = 0.76;
    float g2 = anisotropy * anisotropy;
    return 0.1193662 * (1.0 - g2) * (1.0 + cosine * cosine)
        / ((2.0 + g2) * pow(max(0.001, 1.0 + g2 - 2.0 * anisotropy * cosine), 1.5));
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

    float step_length = (end_distance - start_distance) / 12.0;
    vec2 view_depth = vec2(0.0);
    vec3 rayleigh_sum = vec3(0.0);
    vec3 mie_sum = vec3(0.0);
    for (int sample_index = 0; sample_index < 12; sample_index++) {
        float distance_along_ray = start_distance + (float(sample_index) + 0.5) * step_length;
        vec3 point = ray_direction * distance_along_ray;
        vec2 density = atmosphere_density(point);
        view_depth += density * step_length;
        vec3 light_direction = normalize(u_sun_center - point);
        vec2 sun_depth = light_optical_depth(point, light_direction);
        vec3 attenuation = exp(
            -(BETA_R * (view_depth.x + sun_depth.x) + BETA_M * (view_depth.y + sun_depth.y))
        );
        rayleigh_sum += attenuation * density.x * step_length;
        mie_sum += attenuation * density.y * step_length;
    }

    vec3 center_direction = normalize(u_sun_center - ray_direction * ((start_distance + end_distance) * 0.5));
    float phase_cosine = dot(ray_direction, center_direction);
    scattering = 18.0
        * (rayleigh_sum * BETA_R * rayleigh_phase(phase_cosine)
            + mie_sum * BETA_M * mie_phase(phase_cosine));
    transmission = exp(-(BETA_R * view_depth.x + BETA_M * view_depth.y));
}

float circle_overlap_fraction(float source_radius, float occulter_radius, float separation) {
    if (separation >= source_radius + occulter_radius) {
        return 0.0;
    }
    if (separation <= abs(source_radius - occulter_radius)) {
        if (occulter_radius >= source_radius) {
            return 1.0;
        }
        return occulter_radius * occulter_radius / (source_radius * source_radius);
    }

    float distance = max(separation, 0.0000001);
    float source_angle = acos(clamp(
        (distance * distance + source_radius * source_radius - occulter_radius * occulter_radius)
            / (2.0 * distance * source_radius),
        -1.0,
        1.0
    ));
    float occulter_angle = acos(clamp(
        (distance * distance + occulter_radius * occulter_radius - source_radius * source_radius)
            / (2.0 * distance * occulter_radius),
        -1.0,
        1.0
    ));
    float radical = max(
        0.0,
        (-distance + source_radius + occulter_radius)
            * (distance + source_radius - occulter_radius)
            * (distance - source_radius + occulter_radius)
            * (distance + source_radius + occulter_radius)
    );
    float area = source_radius * source_radius * source_angle
        + occulter_radius * occulter_radius * occulter_angle
        - 0.5 * sqrt(radical);
    return clamp(area / (PI * source_radius * source_radius), 0.0, 1.0);
}

float solar_visibility(vec3 point, vec3 occulter_center, float occulter_radius) {
    vec3 to_sun = u_sun_center - point;
    vec3 to_occulter = occulter_center - point;
    float sun_distance = length(to_sun);
    float occulter_distance = length(to_occulter);
    if (occulter_distance >= sun_distance || occulter_distance <= occulter_radius) {
        return 1.0;
    }
    float source_radius = asin(clamp(u_sun_radius / sun_distance, 0.0, 1.0));
    float apparent_occulter = asin(clamp(occulter_radius / occulter_distance, 0.0, 1.0));
    float separation = acos(clamp(dot(to_sun, to_occulter) / (sun_distance * occulter_distance), -1.0, 1.0));
    return 1.0 - circle_overlap_fraction(source_radius, apparent_occulter, separation);
}

vec3 earth_surface(vec3 point, vec3 ray_direction) {
    vec3 relative = point - u_earth_center;
    vec3 normal = normalize(vec3(
        relative.x,
        relative.y / (u_earth_polar_radius * u_earth_polar_radius),
        relative.z
    ));
    vec3 radial = normalize(relative);
    vec3 local = rotate_y(radial, -u_earth_rotation);
    vec2 longitude_latitude = vec2(atan(local.z, local.x), asin(clamp(local.y, -1.0, 1.0)));
    float field = continent_field(longitude_latitude, local);
    float land = smoothstep(-0.045, 0.045, field);
    float latitude = abs(longitude_latitude.y) / (0.5 * PI);
    float moisture = fbm(local * 7.0 + vec3(-3.0, 8.0, 5.0));
    float relief = ridged(local * 8.0 + vec3(5.0, -7.0, 2.0));

    float andes = exp(-pow(wrap_pi(longitude_latitude.x - radians(-71.0)) / radians(4.0), 2.0))
        * smoothstep(radians(-50.0), radians(-10.0), longitude_latitude.y)
        * (1.0 - smoothstep(radians(12.0), radians(24.0), longitude_latitude.y));
    float himalaya = exp(-pow(wrap_pi(longitude_latitude.x - radians(84.0)) / radians(15.0), 2.0))
        * exp(-pow((longitude_latitude.y - radians(30.0)) / radians(5.0), 2.0));
    float rockies = exp(-pow(wrap_pi(longitude_latitude.x - radians(-113.0)) / radians(7.0), 2.0))
        * exp(-pow((longitude_latitude.y - radians(43.0)) / radians(17.0), 2.0));
    float elevation = land * clamp(relief * 0.35 + andes * 0.65 + himalaya * 0.85 + rockies * 0.5, 0.0, 1.0);

    float sahara = smoothstep(0.0, 0.45, ellipse_field(
        longitude_latitude,
        radians(vec2(16.0, 23.0)),
        radians(vec2(28.0, 12.0)),
        0.0
    ));
    float australia_desert = smoothstep(0.0, 0.5, ellipse_field(
        longitude_latitude,
        radians(vec2(134.0, -25.0)),
        radians(vec2(13.0, 8.0)),
        0.0
    ));
    float aridity = clamp((0.58 - moisture) * 2.2 + sahara * 0.85 + australia_desert * 0.65, 0.0, 1.0);
    vec3 vegetation = mix(vec3(0.035, 0.12, 0.045), vec3(0.18, 0.27, 0.075), moisture);
    vec3 desert = mix(vec3(0.42, 0.28, 0.12), vec3(0.72, 0.52, 0.27), moisture);
    vec3 rock = mix(vec3(0.24, 0.22, 0.19), vec3(0.48, 0.43, 0.35), relief);
    vec3 land_color = mix(vegetation, desert, aridity);
    land_color = mix(land_color, rock, smoothstep(0.42, 0.86, elevation));
    float snow = smoothstep(0.68, 0.94, latitude + elevation * 0.36);
    land_color = mix(land_color, vec3(0.88, 0.93, 0.97), snow);

    float ocean_detail = fbm(local * 35.0 + vec3(1.0, 9.0, -4.0));
    vec3 ocean_color = mix(vec3(0.004, 0.018, 0.055), vec3(0.015, 0.15, 0.25), ocean_detail * 0.42);
    vec3 base_color = mix(ocean_color, land_color, land);

    vec3 light_direction = normalize(u_sun_center - point);
    vec3 view_direction = -ray_direction;
    float sunlight = max(dot(normal, light_direction), 0.0);
    float visibility = solar_visibility(point, u_moon_center, u_moon_radius);
    vec2 sun_depth = light_optical_depth(point + normal * 0.000015, light_direction);
    vec3 sun_transmission = exp(-(BETA_R * sun_depth.x + BETA_M * sun_depth.y));
    float cloud = cloud_density(local);
    float cloud_shadow = cloud_density(normalize(local + rotate_y(light_direction, -u_earth_rotation) * 0.018));
    float direct = sunlight * visibility * (1.0 - cloud_shadow * 0.48);
    vec3 color = base_color * (0.006 + direct * sun_transmission * 1.35);

    vec3 half_direction = normalize(light_direction + view_direction);
    float fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
    float ocean_specular = pow(max(dot(normal, half_direction), 0.0), 420.0)
        * (0.35 + ocean_detail * 0.95)
        * (1.0 - land)
        * visibility
        * sunlight;
    color += sun_transmission * (ocean_specular * 4.2 + fresnel * (1.0 - land) * sunlight * 0.055);

    float night = 1.0 - smoothstep(-0.08, 0.12, dot(normal, light_direction));
    float settlement = pow(noise3(local * 210.0 + vec3(7.0, 19.0, 3.0)), 15.0)
        * smoothstep(0.24, 0.70, moisture)
        * (1.0 - snow)
        * land;
    color += vec3(1.0, 0.48, 0.12) * settlement * night * 2.0;

    float cloud_light = 0.07 + pow(sunlight, 0.65) * visibility * 1.15;
    vec3 cloud_color = mix(vec3(0.08, 0.10, 0.14), vec3(0.92, 0.95, 1.0), cloud_light);
    color = mix(color, cloud_color, cloud * (0.45 + 0.45 * sunlight));
    return color;
}

vec3 moon_surface(vec3 point, vec3 ray_direction) {
    vec3 normal = normalize(point - u_moon_center);
    vec3 light_direction = normalize(u_sun_center - point);
    vec3 earth_direction = normalize(u_earth_center - point);
    float crater_noise = fbm(normal * 13.0 + vec3(2.0, -5.0, 9.0));
    float small_craters = pow(noise3(normal * 61.0 - vec3(8.0, 3.0, 1.0)), 7.0);
    vec3 albedo = mix(vec3(0.085, 0.082, 0.078), vec3(0.27, 0.255, 0.235), crater_noise);
    albedo *= 1.0 - small_craters * 0.38;
    float sunlight = max(dot(normal, light_direction), 0.0);
    float visibility = solar_visibility(point, u_earth_center, 1.0);
    float earthshine = max(dot(normal, earth_direction), 0.0) * 0.025;
    float opposition = pow(max(dot(normal, -ray_direction), 0.0), 12.0) * sunlight * 0.08;
    return albedo * (0.003 + sunlight * visibility * 1.15 + earthshine + opposition);
}

vec3 sun_surface(vec3 point, vec3 ray_direction) {
    vec3 normal = normalize(point - u_sun_center);
    float limb = max(dot(normal, -ray_direction), 0.0);
    float granulation = fbm(normal * 180.0 + vec3(u_time * 0.008, 0.0, 0.0));
    vec3 color = mix(vec3(1.0, 0.45, 0.08), vec3(1.0, 0.96, 0.78), 0.42 + 0.58 * limb);
    return color * (28.0 + granulation * 14.0) * (0.35 + 0.65 * limb);
}

vec3 space_background(vec3 ray_direction) {
    vec2 star_uv = vec2(
        atan(ray_direction.z, ray_direction.x) / TAU + 0.5,
        asin(clamp(ray_direction.y, -1.0, 1.0)) / PI + 0.5
    );
    vec2 star_cell = floor(star_uv * vec2(1900.0, 950.0));
    float seed = hash21(star_cell);
    float star = smoothstep(0.9974, 0.9999, seed)
        * (0.45 + 2.5 * pow(hash21(star_cell + 17.0), 5.0));
    vec3 star_temperature = mix(
        vec3(0.48, 0.68, 1.0),
        vec3(1.0, 0.72, 0.45),
        hash21(star_cell + 43.0)
    );

    vec3 galactic_axis = normalize(vec3(0.24, 0.91, -0.34));
    float galactic_plane = exp(-pow(dot(ray_direction, galactic_axis) * 9.0, 2.0));
    float dust = fbm(ray_direction * 5.0 + vec3(11.0, -4.0, 2.0));
    vec3 milky_way = vec3(0.10, 0.13, 0.20)
        * galactic_plane
        * smoothstep(0.31, 0.82, dust)
        * 0.28;

    vec3 sun_direction = normalize(u_sun_center);
    float sun_angle = acos(clamp(dot(ray_direction, sun_direction), -1.0, 1.0));
    float solar_halo = exp(-sun_angle * 420.0) * 0.18 + exp(-sun_angle * 48.0) * 0.008;
    return vec3(0.00008, 0.00012, 0.00024) + star_temperature * star + milky_way
        + vec3(1.0, 0.66, 0.32) * solar_halo;
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

    float earth_distance = hit_distance(ray_ellipsoid(vec3(0.0), ray_direction));
    float moon_distance = hit_distance(ray_sphere(vec3(0.0), ray_direction, u_moon_center, u_moon_radius));
    float sun_distance = hit_distance(ray_sphere(vec3(0.0), ray_direction, u_sun_center, u_sun_radius));
    float body_distance = min(earth_distance, min(moon_distance, sun_distance));

    vec3 color = space_background(ray_direction);
    if (body_distance < INF) {
        vec3 point = ray_direction * body_distance;
        if (earth_distance <= moon_distance && earth_distance <= sun_distance) {
            color = earth_surface(point, ray_direction);
        } else if (moon_distance <= sun_distance) {
            color = moon_surface(point, ray_direction);
        } else {
            color = sun_surface(point, ray_direction);
        }
    }

    vec2 atmosphere_interval = ray_sphere(
        vec3(0.0),
        ray_direction,
        u_earth_center,
        u_atmosphere_radius
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

    color = aces_tone_map(color);
    color = pow(color, vec3(1.0 / 2.2));
    float vignette = 1.0 - 0.11 * dot(v_clip * 0.55, v_clip * 0.55);
    out_color = vec4(color * vignette, 1.0);
}
