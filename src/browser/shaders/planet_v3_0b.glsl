float terrainHeightM(vec3 point) {
    if (u_surface_ready < 0.5) {
        return 0.0;
    }
    vec3 direction = earthFixedDirection(point);
    float land = referenceLand(earthUv(point)) * u_surface_ready;
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
