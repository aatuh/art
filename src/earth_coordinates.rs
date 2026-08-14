//! Coordinate transforms between the inertial scene and Earth-fixed surface data.

use crate::planet::Vec3d;

/// Converts an inertial direction into the rotating Earth-fixed frame used by
/// surface imagery, elevation and cube-sphere tiles.
pub fn inertial_to_earth_fixed(direction: Vec3d, earth_rotation_radians: f64) -> Vec3d {
    rotate_y(
        direction.normalized(),
        -finite_angle(earth_rotation_radians),
    )
}

/// Converts an Earth-fixed direction back into the inertial scene frame.
pub fn earth_fixed_to_inertial(direction: Vec3d, earth_rotation_radians: f64) -> Vec3d {
    rotate_y(
        direction.normalized(),
        finite_angle(earth_rotation_radians),
    )
}

fn finite_angle(angle: f64) -> f64 {
    if angle.is_finite() { angle } else { 0.0 }
}

fn rotate_y(direction: Vec3d, angle: f64) -> Vec3d {
    let cosine = angle.cos();
    let sine = angle.sin();
    Vec3d::new(
        cosine * direction.x + sine * direction.z,
        direction.y,
        -sine * direction.x + cosine * direction.z,
    )
}

#[cfg(test)]
mod tests {
    use super::{earth_fixed_to_inertial, inertial_to_earth_fixed};
    use crate::planet::Vec3d;

    #[test]
    fn quarter_turn_matches_shader_rotation_convention() {
        let fixed = inertial_to_earth_fixed(Vec3d::X, std::f64::consts::FRAC_PI_2);
        assert!(fixed.x.abs() < 1.0e-12);
        assert!((fixed.z - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn conversion_round_trips_arbitrary_direction() {
        let inertial = Vec3d::new(0.31, -0.42, 0.85).normalized();
        let rotation = 4.91;
        let fixed = inertial_to_earth_fixed(inertial, rotation);
        let restored = earth_fixed_to_inertial(fixed, rotation);
        assert!((restored - inertial).length() < 1.0e-12);
    }

    #[test]
    fn nonfinite_rotation_falls_back_to_identity() {
        let direction = Vec3d::new(0.2, 0.3, 0.9).normalized();
        assert_eq!(inertial_to_earth_fixed(direction, f64::NAN), direction);
        assert_eq!(earth_fixed_to_inertial(direction, f64::INFINITY), direction);
    }

    #[test]
    fn zero_direction_remains_zero() {
        assert_eq!(
            inertial_to_earth_fixed(Vec3d::ZERO, 1.5),
            Vec3d::ZERO
        );
    }
}
