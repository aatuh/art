void main() {
    vec2 screen = v_clip;
    screen.x *= u_resolution.x / max(u_resolution.y, 1.0);
    float tangent_half_fov = tan(u_fov_y_radians * 0.5);
    vec3 ray_origin = vec3(0.0);
    vec3 ray_direction = normalize(
        u_forward + screen.x * tangent_half_fov * u_right + screen.y * tangent_half_fov * u_up
    );
    vec3 sun_direction = normalize(u_sun_center - u_earth_center);

    vec3 color = proceduralStars(ray_direction);
    float nearest_opaque = INF;

    float sun_distance = nearestPositiveIntersection(
        raySphere(ray_origin, ray_direction, u_sun_center, u_sun_radius_m)
    );
    if (sun_distance < nearest_opaque) {
        nearest_opaque = sun_distance;
        color = shadePrimary(ray_origin + ray_direction * sun_distance, ray_direction);
    }

    float moon_distance = nearestPositiveIntersection(
        raySphere(ray_origin, ray_direction, u_moon_center, u_moon_radius_m)
    );
    if (moon_distance < nearest_opaque) {
        nearest_opaque = moon_distance;
        color = shadeSatellite(
            ray_origin + ray_direction * moon_distance,
            ray_direction,
            normalize(u_sun_center - u_moon_center)
        );
    }

    float earth_distance = intersectTerrain(ray_origin, ray_direction);
    if (earth_distance < nearest_opaque) {
        nearest_opaque = earth_distance;
        color = renderEarthSurface(
            ray_origin + ray_direction * earth_distance,
            ray_direction,
            sun_direction
        );
    }

    float cloud_transmittance;
    vec3 cloud_radiance = renderClouds(
        ray_origin,
        ray_direction,
        nearest_opaque,
        sun_direction,
        cloud_transmittance
    );
    color = cloud_radiance + color * cloud_transmittance;

    vec2 atmosphere_hit = rayEllipsoid(
        ray_origin,
        ray_direction,
        u_earth_center,
        earthRadii(u_atmosphere_top_m)
    );
    if (atmosphere_hit.y > max(0.0, atmosphere_hit.x)) {
        float atmosphere_start = max(0.0, atmosphere_hit.x);
        float atmosphere_end = min(atmosphere_hit.y, nearest_opaque);
        if (atmosphere_end > atmosphere_start) {
            vec3 atmosphere_transmittance;
            vec3 atmosphere_radiance = renderAtmosphere(
                ray_origin,
                ray_direction,
                atmosphere_start,
                atmosphere_end,
                sun_direction,
                atmosphere_transmittance
            );
            color = color * atmosphere_transmittance + atmosphere_radiance;
        }
    }

    float dither = (hash13(vec3(gl_FragCoord.xy, fract(u_time))) - 0.5) / 255.0;
    vec3 mapped = toneMap(color + dither);
    out_color = vec4(pow(mapped, vec3(1.0 / 2.2)), 1.0);
}
