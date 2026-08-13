    float milky_way = exp(-milky_way_distance * milky_way_distance * 30.0);
    vec3 primary_cell = floor(direction * 1300.0);
    vec3 secondary_cell = floor(direction * 2700.0);
    float primary = smoothstep(0.99935, 1.0, hash13(primary_cell));
    float secondary = smoothstep(0.99972, 1.0, hash13(secondary_cell));
    float temperature = hash13(primary_cell + vec3(19.0, 3.0, 7.0));
    vec3 warm = vec3(1.0, 0.78, 0.58);
    vec3 cool = vec3(0.62, 0.76, 1.0);
    vec3 star_color = mix(warm, cool, temperature) * primary * 1.45;
    star_color += vec3(0.82, 0.88, 1.0) * secondary * 0.55;
    star_color += milky_way * vec3(0.010, 0.014, 0.027)
        * (0.30 + 0.70 * fbm(direction * 12.0));
    return star_color;
}

vec3 fresnelSchlick(float cosine_theta, vec3 reflectance_zero) {
    return reflectance_zero + (vec3(1.0) - reflectance_zero) * pow(1.0 - cosine_theta, 5.0);
}

float distributionGgx(float normal_half, float roughness) {
    float alpha = roughness * roughness;
    float alpha_sq = alpha * alpha;
    float denominator = normal_half * normal_half * (alpha_sq - 1.0) + 1.0;
    return alpha_sq / max(PI * denominator * denominator, 1.0e-5);
}

float geometrySchlickGgx(float normal_direction, float roughness) {
    float k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    return normal_direction / max(normal_direction * (1.0 - k) + k, 1.0e-5);
}

