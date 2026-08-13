vec3 shadeSatellite(vec3 point, vec3 ray_direction, vec3 light_direction) {
    vec3 normal = normalize(point - u_moon_center);
    float broad = fbm(normal * 13.0);
    float fine = fbm(normal * 54.0 + vec3(8.0, 3.0, 1.0));
    vec3 albedo = mix(vec3(0.065, 0.063, 0.059), vec3(0.19, 0.18, 0.165), broad);
    albedo += pow(abs(fine * 2.0 - 1.0), 5.0) * 0.025;
    float direct = max(dot(normal, light_direction), 0.0) * solarVisibilityAtMoon(point);
    vec3 earth_direction = normalize(u_earth_center - point);
    float reflected = max(dot(normal, earth_direction), 0.0) * 0.025;
    return albedo * (0.003 + direct * 1.1 + reflected);
}

vec3 shadePrimary(vec3 point, vec3 ray_direction) {
    vec3 normal = normalize(point - u_sun_center);
    float mu = saturate(dot(normal, -ray_direction));
    float granulation = 0.96 + 0.04 * fbm(normal * 180.0 + u_time * 0.012);
    return vec3(8.6, 7.45, 5.85) * (0.40 + 0.60 * mu) * granulation;
}

vec3 toneMap(vec3 color) {
    color = max(color, vec3(0.0));
    return saturate((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14));
}

