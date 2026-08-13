            * (separation - light_radius + blocker_radius)
            * (separation + light_radius + blocker_radius)
    );
    return light_radius * light_radius * light_term
        + blocker_radius * blocker_radius * blocker_term
        - 0.5 * sqrt(radical);
}

float visibleLightDisc(
    vec3 point,
    vec3 light_center,
    float light_radius,
    vec3 blocker_center,
    float blocker_radius
) {
    vec3 to_light = light_center - point;
    vec3 to_blocker = blocker_center - point;
    float light_distance = length(to_light);
    float blocker_distance = length(to_blocker);
    if (blocker_distance >= light_distance || blocker_distance <= blocker_radius) {
        return 1.0;
    }
    float light_angular_radius = asin(clamp(light_radius / light_distance, 0.0, 1.0));
    float blocker_angular_radius = asin(clamp(blocker_radius / blocker_distance, 0.0, 1.0));
    float separation = acos(clamp(dot(to_light / light_distance, to_blocker / blocker_distance), -1.0, 1.0));
    float overlap = circleOverlap(light_angular_radius, blocker_angular_radius, separation);
    return saturate(1.0 - overlap / max(PI * light_angular_radius * light_angular_radius, 1.0e-8));
}

float earthOcclusion(vec3 point, vec3 light_direction) {
    vec2 intersection = rayEllipsoid(
        point + light_direction * RAY_EPSILON_M,
        light_direction,
        u_earth_center,
        earthRadii(0.0)
    );
    return nearestPositiveIntersection(intersection) < INF * 0.5 ? 0.0 : 1.0;
}

float solarVisibilityAtEarth(vec3 point) {
    return visibleLightDisc(
        point,
        u_sun_center,
        u_sun_radius_m,
        u_moon_center,
        u_moon_radius_m
    );
}

float solarVisibilityAtMoon(vec3 point) {
    return visibleLightDisc(
        point,
        u_sun_center,
        u_sun_radius_m,
        u_earth_center,
        u_earth_radii.x
    );
}

float cloudDensity(vec3 point) {
    float altitude_m = altitudeAboveEllipsoid(point);
    float height_fraction = (altitude_m - CLOUD_BASE_M) / (CLOUD_TOP_M - CLOUD_BASE_M);
    if (height_fraction <= 0.0 || height_fraction >= 1.0) {
        return 0.0;
    }

    vec3 direction = earthFixedDirection(point);
    // About 2.9 degrees/hour of zonal advection. The additional evolving offsets make
    // systems grow, erode, merge and dissipate instead of merely sliding over the globe.
    float weather_drift = u_time * 1.4e-5;
    float evolution = u_time * 9.0e-5;
    direction.xz = rotate2d(-weather_drift) * direction.xz;
    vec3 broad_flow = vec3(evolution * 0.31, -evolution * 0.17, evolution * 0.23);
    vec3 detail_flow = vec3(-evolution * 1.15, evolution * 0.73, evolution * 0.51);
    float broad = fbm(direction * 3.2 + vec3(2.1, 7.8, 1.3) + broad_flow);
    float detail = fbm(direction * 17.0 - vec3(4.0, 1.0, 8.0) + detail_flow);
    float longitude = atan(direction.z, direction.x);
    float latitude = asin(clamp(direction.y, -1.0, 1.0));
    float circulation = 0.08 * sin(longitude * 5.0 + latitude * 9.0 + evolution * 0.6)
        + 0.045 * cos(latitude * 19.0 - evolution * 0.35);
    float lifecycle = 0.035 * sin(longitude * 2.0 - latitude * 3.0 + evolution * 0.9);
    float weather = broad + detail * 0.23 + circulation + lifecycle - abs(direction.y) * 0.05;
    float coverage = smoothstep(0.61, 0.76, weather);
    float vertical_profile = smoothstep(0.0, 0.18, height_fraction)
        * (1.0 - smoothstep(0.62, 1.0, height_fraction));
    float erosion = smoothstep(0.34, 0.78, detail + coverage * 0.42);
    return coverage * vertical_profile * mix(0.42, 1.0, erosion);
}

float cloudLightTransmittance(vec3 point, vec3 light_direction) {
    float optical_depth = 0.0;
    float step_length_m = 5.0e3;
    for (int step_index = 0; step_index < CLOUD_LIGHT_STEPS; ++step_index) {
        vec3 sample_point = point + light_direction * (float(step_index) + 0.65) * step_length_m;
        optical_depth += cloudDensity(sample_point) * step_length_m;
    }
    return exp(-optical_depth * 1.35e-4);
}

vec3 renderClouds(
    vec3 ray_origin,
    vec3 ray_direction,
    float maximum_distance,
    vec3 sun_direction,
    out float cloud_transmittance
) {
    vec2 outer_hit = rayEllipsoid(ray_origin, ray_direction, u_earth_center, earthRadii(CLOUD_TOP_M));
    cloud_transmittance = 1.0;
    if (outer_hit.y <= max(0.0, outer_hit.x)) {
        return vec3(0.0);
    }

    float segment_start = max(0.0, outer_hit.x);
    float segment_end = min(outer_hit.y, maximum_distance);
    vec2 inner_hit = rayEllipsoid(ray_origin, ray_direction, u_earth_center, earthRadii(CLOUD_BASE_M));
    if (u_camera_altitude_m < CLOUD_BASE_M && inner_hit.y > 0.0) {
        segment_start = max(segment_start, inner_hit.y);
    } else if (inner_hit.x > segment_start) {
        segment_end = min(segment_end, inner_hit.x);
    }
    if (segment_end <= segment_start) {
        return vec3(0.0);
    }

    float step_length_m = (segment_end - segment_start) / float(CLOUD_VIEW_STEPS);
    float jitter = hash13(vec3(gl_FragCoord.xy, floor(u_time * 2.0))) - 0.5;
    vec3 radiance = vec3(0.0);
    for (int step_index = 0; step_index < CLOUD_VIEW_STEPS; ++step_index) {
        float distance_m = segment_start
            + (float(step_index) + 0.5 + jitter * 0.35) * step_length_m;
        vec3 point = ray_origin + ray_direction * distance_m;
        float density = cloudDensity(point);
        if (density <= 0.001) {
            continue;
        }

        float sunlight = earthOcclusion(point, sun_direction)
            * solarVisibilityAtEarth(point)
            * cloudLightTransmittance(point, sun_direction);
        float forward_phase = henyeyGreenstein(dot(ray_direction, sun_direction), 0.76);
        float backward_phase = henyeyGreenstein(dot(ray_direction, sun_direction), -0.22);
        vec3 direct_light = vec3(1.0, 0.93, 0.84)
            * sunlight
            * (0.40 + 5.5 * forward_phase + 0.5 * backward_phase);
        vec3 ambient_light = vec3(0.19, 0.28, 0.43) * (0.28 + 0.72 * saturate(altitudeAboveEllipsoid(point) / CLOUD_TOP_M));
        float sample_alpha = 1.0 - exp(-density * step_length_m * 7.0e-5);
        radiance += cloud_transmittance * (direct_light + ambient_light) * sample_alpha;
        cloud_transmittance *= 1.0 - sample_alpha;
        if (cloud_transmittance < 0.015) {
            break;
        }
    }
    return radiance;
}

vec3 renderAtmosphere(
    vec3 ray_origin,
    vec3 ray_direction,
    float segment_start,
    float segment_end,
    vec3 sun_direction,
    out vec3 transmittance
) {
    float segment_length_m = (segment_end - segment_start) / float(ATMOSPHERE_VIEW_STEPS);
    vec3 view_optical_depth = vec3(0.0);
    vec3 sum_rayleigh = vec3(0.0);
    vec3 sum_mie = vec3(0.0);
    float cosine_theta = dot(ray_direction, sun_direction);
    float rayleigh_phase = 3.0 / (16.0 * PI) * (1.0 + cosine_theta * cosine_theta);
    float mie_phase = henyeyGreenstein(cosine_theta, 0.78);
    vec3 eclipse_sample = ray_origin + ray_direction * (segment_start + segment_end) * 0.5;
    float eclipse_visibility = solarVisibilityAtEarth(eclipse_sample);

    for (int view_step = 0; view_step < ATMOSPHERE_VIEW_STEPS; ++view_step) {
        float distance_m = segment_start + (float(view_step) + 0.5) * segment_length_m;
        vec3 point = ray_origin + ray_direction * distance_m;
        float altitude_m = max(0.0, altitudeAboveEllipsoid(point));
        float rayleigh_density = exp(-altitude_m / RAYLEIGH_SCALE_HEIGHT_M);
        float mie_density = exp(-altitude_m / MIE_SCALE_HEIGHT_M);
        float ozone = ozoneDensity(altitude_m);
        view_optical_depth += vec3(rayleigh_density, mie_density, ozone) * segment_length_m;

        vec2 top_hit = rayEllipsoid(
            point,
            sun_direction,
            u_earth_center,
            earthRadii(u_atmosphere_top_m)
        );
        float light_length_m = max(0.0, top_hit.y) / float(ATMOSPHERE_LIGHT_STEPS);
        vec3 light_optical_depth = vec3(0.0);
        for (int light_step = 0; light_step < ATMOSPHERE_LIGHT_STEPS; ++light_step) {
            vec3 light_point = point
                + sun_direction * (float(light_step) + 0.5) * light_length_m;
            float light_altitude_m = max(0.0, altitudeAboveEllipsoid(light_point));
            light_optical_depth += vec3(
                exp(-light_altitude_m / RAYLEIGH_SCALE_HEIGHT_M),
                exp(-light_altitude_m / MIE_SCALE_HEIGHT_M),
                ozoneDensity(light_altitude_m)
            ) * light_length_m;
        }

        vec3 combined_depth = view_optical_depth + light_optical_depth;
        vec3 extinction = BETA_RAYLEIGH * combined_depth.x
            + BETA_MIE_EXTINCTION * combined_depth.y
            + BETA_OZONE_ABSORPTION * combined_depth.z;
        vec3 attenuation = exp(-extinction);
        float direct_visibility = earthOcclusion(point, sun_direction) * eclipse_visibility;
        sum_rayleigh += attenuation * rayleigh_density * segment_length_m * direct_visibility;
        sum_mie += attenuation * mie_density * segment_length_m * direct_visibility;
    }

    vec3 view_extinction = BETA_RAYLEIGH * view_optical_depth.x
        + BETA_MIE_EXTINCTION * view_optical_depth.y
        + BETA_OZONE_ABSORPTION * view_optical_depth.z;
    transmittance = exp(-view_extinction);
    vec3 single_scattering = sum_rayleigh * BETA_RAYLEIGH * rayleigh_phase
        + sum_mie * BETA_MIE_SCATTERING * mie_phase;
    vec3 multiple_scattering = (vec3(1.0) - transmittance)
        * vec3(0.018, 0.026, 0.045)
        * (0.25 + 0.75 * eclipse_visibility);
    return single_scattering * 4.5 + multiple_scattering;
}

vec3 proceduralStars(vec3 ray_direction) {
    vec3 direction = normalize(ray_direction);
    float milky_way_distance = abs(direction.y * 0.78 + direction.z * 0.20);
