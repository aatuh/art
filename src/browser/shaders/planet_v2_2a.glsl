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

vec3 perturbSurfaceNormal(vec3 point, vec3 geometric_normal, float land_fraction) {
    vec3 fixed_direction = earthFixedDirection(point);
    vec3 tangent = normalize(cross(abs(geometric_normal.y) < 0.95 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0), geometric_normal));
    vec3 bitangent = normalize(cross(geometric_normal, tangent));
    float near_detail = 1.0 - smoothstep(50.0e3, 4.0e6, max(u_camera_altitude_m, 0.0));
    float land_x = fbm(fixed_direction * 360.0 + vec3(0.9, 0.0, 0.0)) - 0.5;
    float land_y = fbm(fixed_direction * 360.0 + vec3(0.0, 0.9, 0.0)) - 0.5;
    float ocean_phase = u_time * 0.75;
    float longitude_wave = sin(fixed_direction.x * 9200.0 + fixed_direction.z * 4700.0 + ocean_phase);
    float cross_wave = sin(fixed_direction.z * 12400.0 - fixed_direction.x * 3500.0 - ocean_phase * 0.73);
    vec2 slope_land = vec2(land_x, land_y) * 0.34;
    vec2 slope_ocean = vec2(longitude_wave, cross_wave) * 0.035;
    vec2 slope = mix(slope_ocean, slope_land, land_fraction) * near_detail;
    return normalize(geometric_normal + tangent * slope.x + bitangent * slope.y);
}

float surfaceCloudShadow(vec3 point, vec3 sun_direction) {
    float optical_depth = 0.0;
    float step_length_m = 4.0e3;
    for (int step_index = 0; step_index < 4; ++step_index) {
        vec3 sample_point = point + sun_direction * (float(step_index) + 0.75) * step_length_m;
        optical_depth += cloudDensity(sample_point) * step_length_m;
    }
    return exp(-optical_depth * 1.1e-4);
}

vec3 renderEarthSurface(vec3 point, vec3 ray_direction, vec3 sun_direction) {
    vec3 radii = earthRadii(0.0);
    vec3 geometric_normal = ellipsoidNormal(point, radii);
    vec4 sampled_surface = texture(u_surface, earthUv(point));
    float land_fraction = mix(0.0, sampled_surface.a, u_surface_ready);
    vec3 fixed_direction = earthFixedDirection(point);
    float latitude = abs(asin(clamp(fixed_direction.y, -1.0, 1.0))) / (0.5 * PI);
    float ocean_broad = fbm(fixed_direction * 11.0 + vec3(3.0, 8.0, -5.0));
    float ocean_depth = fbm(fixed_direction * 37.0 - vec3(9.0, 2.0, 4.0));
    vec3 ocean_albedo = mix(
        vec3(0.0035, 0.020, 0.060),
        vec3(0.012, 0.095, 0.185),
        saturate(ocean_broad * 0.72 + ocean_depth * 0.20 + (1.0 - latitude) * 0.08)
    );
    vec3 encoded_albedo = mix(ocean_albedo, sampled_surface.rgb, land_fraction);
    float near_detail = 1.0 - smoothstep(120.0e3, 5.0e6, max(u_camera_altitude_m, 0.0));
    float fine_albedo = fbm(fixed_direction * 720.0 + vec3(4.0, -9.0, 2.0)) - 0.5;
    encoded_albedo *= 1.0 + fine_albedo * near_detail * mix(0.05, 0.18, land_fraction);
    vec3 albedo = pow(max(encoded_albedo, vec3(0.001)), vec3(2.2));
    vec3 normal = perturbSurfaceNormal(point, geometric_normal, land_fraction);
    vec3 view_direction = -ray_direction;
    vec3 half_direction = normalize(view_direction + sun_direction);
    float normal_light = max(dot(normal, sun_direction), 0.0);
    float normal_view = max(dot(normal, view_direction), 1.0e-4);
    float normal_half = max(dot(normal, half_direction), 0.0);
    float view_half = max(dot(view_direction, half_direction), 0.0);
    float roughness = mix(0.055, mix(0.46, 0.78, fbm(earthFixedDirection(point) * 27.0)), land_fraction);
    vec3 reflectance_zero = mix(vec3(0.0204), vec3(0.035), land_fraction);
    vec3 fresnel = fresnelSchlick(view_half, reflectance_zero);
    float distribution = distributionGgx(normal_half, roughness);
    float geometry = geometrySchlickGgx(normal_view, roughness)
        * geometrySchlickGgx(max(normal_light, 1.0e-4), roughness);
    vec3 specular = distribution * geometry * fresnel
        / max(4.0 * normal_view * max(normal_light, 1.0e-4), 1.0e-4);
    vec3 diffuse = albedo * (vec3(1.0) - fresnel) / PI;
    float eclipse = solarVisibilityAtEarth(point);
    float cloud_shadow = surfaceCloudShadow(point, sun_direction);
    float direct = normal_light * eclipse * cloud_shadow;
    vec3 atmospheric_fill = albedo * vec3(0.015, 0.025, 0.045)
        * (0.10 + 0.90 * saturate(geometric_normal.y * 0.5 + 0.5));
    vec3 ocean_horizon = vec3(0.025, 0.10, 0.23)
        * pow(1.0 - normal_view, 5.0)
        * (1.0 - land_fraction);
    return (diffuse + specular) * direct * vec3(4.3, 4.05, 3.75)
        + atmospheric_fill
        + ocean_horizon;
}

