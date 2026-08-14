float deep_water_angular_frequency(float angular_wavenumber) {
    const float gravity_mps2 = 9.80665;
    float physical_wavenumber = angular_wavenumber / u_earth_equatorial_radius_m;
    return sqrt(gravity_mps2 * physical_wavenumber);
}

vec3 deep_water_wave_slope(
    vec3 local,
    vec3 axis,
    float angular_wavenumber,
    float steepness,
    float phase_offset
) {
    vec3 tangent_axis = axis - local * dot(axis, local);
    float tangent_length = length(tangent_axis);
    if (tangent_length < 0.0001) {
        return vec3(0.0);
    }

    float omega = deep_water_angular_frequency(angular_wavenumber);
    float phase = dot(local, axis) * angular_wavenumber - omega * u_time + phase_offset;
    return tangent_axis / tangent_length * (cos(phase) * steepness);
}

vec3 ocean_normal(vec3 point, vec3 geometric) {
    vec3 local = earth_fixed_direction(point);
    float regional_detail = 1.0 - smoothstep(1500000.0, 12000000.0, u_camera_altitude_m);
    float local_detail = 1.0 - smoothstep(150000.0, 2200000.0, u_camera_altitude_m);

    vec3 slope = vec3(0.0);
    slope += deep_water_wave_slope(
        local,
        normalize(vec3(0.83, 0.08, 0.55)),
        1200.0,
        0.013,
        0.0
    );
    slope += deep_water_wave_slope(
        local,
        normalize(vec3(-0.35, 0.14, 0.93)),
        4200.0,
        0.010,
        1.7
    );
    slope += regional_detail * deep_water_wave_slope(
        local,
        normalize(vec3(0.57, -0.12, -0.81)),
        15000.0,
        0.0065,
        4.1
    );
    slope += regional_detail * deep_water_wave_slope(
        local,
        normalize(vec3(-0.76, 0.05, -0.65)),
        52000.0,
        0.0038,
        2.3
    );
    slope += local_detail * deep_water_wave_slope(
        local,
        normalize(vec3(0.22, 0.03, -0.98)),
        180000.0,
        0.0022,
        5.4
    );

    vec3 world_slope = rotate_y(slope, u_earth_rotation);
    return normalize(geometric - world_slope);
}
