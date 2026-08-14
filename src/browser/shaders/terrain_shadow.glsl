float terrain_shadow_visibility(
    vec3 surface_point,
    vec3 geometric,
    vec3 light_direction,
    float land
) {
    if (land < 0.02 || u_camera_altitude_m > 750000.0) {
        return 1.0;
    }

    vec3 origin = surface_point + geometric * 80.0;
    float visibility = 1.0;
    for (int sample_index = 0; sample_index < 6; ++sample_index) {
        float distance_along_light = 800.0;
        if (sample_index == 1) {
            distance_along_light = 2000.0;
        } else if (sample_index == 2) {
            distance_along_light = 5000.0;
        } else if (sample_index == 3) {
            distance_along_light = 12000.0;
        } else if (sample_index == 4) {
            distance_along_light = 28000.0;
        } else if (sample_index == 5) {
            distance_along_light = 60000.0;
        }

        vec3 sample_point = origin + light_direction * distance_along_light;
        float clearance = altitude_above_ellipsoid(sample_point)
            - terrain_height_world_direction(sample_point - u_earth_center_m);
        visibility = min(visibility, smoothstep(0.0, 450.0, clearance));
        if (visibility <= 0.01) {
            break;
        }
    }
    return visibility;
}
