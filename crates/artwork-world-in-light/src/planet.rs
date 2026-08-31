//! Renderer-independent physical dimensions and low-cost celestial ephemerides.
//!
//! All positions and distances use SI metres and `f64`. Browser renderers should subtract
//! the camera origin before converting values to `f32`; casting absolute astronomical
//! coordinates directly to the GPU loses the precision needed near a surface.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use crate::{earth_coordinates::inertial_to_earth_fixed, earth_terrain::terrain_height_m};

pub const SECONDS_PER_DAY: f64 = 86_400.0;
pub const ASTRONOMICAL_UNIT_M: f64 = 149_597_870_700.0;
pub const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

/// WGS 84 semi-major axis.
pub const EARTH_EQUATORIAL_RADIUS_M: f64 = 6_378_137.0;
/// WGS 84 semi-minor axis.
pub const EARTH_POLAR_RADIUS_M: f64 = 6_356_752.314_245;
pub const EARTH_MEAN_RADIUS_M: f64 = 6_371_008.8;
pub const EARTH_ATMOSPHERE_TOP_M: f64 = 100_000.0;
pub const EARTH_SIDEREAL_ROTATION_S: f64 = 86_164.090_5;

pub const MOON_MEAN_RADIUS_M: f64 = 1_737_400.0;
pub const MEAN_EARTH_MOON_DISTANCE_M: f64 = 384_400_000.0;
pub const SUN_NOMINAL_RADIUS_M: f64 = 695_700_000.0;

const J2000_OBLIQUITY_RADIANS: f64 = 0.409_092_804_222_328_97;
const DEGREES_TO_RADIANS: f64 = std::f64::consts::PI / 180.0;
/// IERS Conventions (2010) defining Earth Rotation Angle at JD 2451545.0 UT1.
const J2000_EARTH_ROTATION_ANGLE_TURNS: f64 = 0.779_057_273_264_0;
/// IERS defining rate of Earth Rotation Angle in revolutions per UT1 day.
const EARTH_ROTATION_RATE_TURNS_PER_UT1_DAY: f64 = 1.002_737_811_911_354_6;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length <= f64::EPSILON || !length.is_finite() {
            Self::ZERO
        } else {
            self / length
        }
    }

    pub fn distance(self, other: Self) -> f64 {
        (self - other).length()
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Converts an already camera-relative vector to GPU-friendly Earth-radius units.
    pub fn to_earth_radii_f32(self) -> [f32; 3] {
        [
            (self.x / EARTH_EQUATORIAL_RADIUS_M) as f32,
            (self.y / EARTH_EQUATORIAL_RADIUS_M) as f32,
            (self.z / EARTH_EQUATORIAL_RADIUS_M) as f32,
        ]
    }
}

impl Add for Vec3d {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl AddAssign for Vec3d {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for Vec3d {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl SubAssign for Vec3d {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl Mul<f64> for Vec3d {
    type Output = Self;

    fn mul(self, scalar: f64) -> Self {
        Self::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl Div<f64> for Vec3d {
    type Output = Self;

    fn div(self, scalar: f64) -> Self {
        Self::new(self.x / scalar, self.y / scalar, self.z / scalar)
    }
}

impl Neg for Vec3d {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPose {
    pub center_m: Vec3d,
    pub radius_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CelestialFrame {
    pub seconds_since_j2000: f64,
    pub earth: BodyPose,
    pub moon: BodyPose,
    pub sun: BodyPose,
    pub earth_rotation_radians: f64,
}

impl CelestialFrame {
    /// Low-cost, deterministic apparent geocentric positions suitable for an artwork.
    ///
    /// The solar and lunar equations retain the dominant eccentricity, inclination and
    /// anomaly terms. They are intentionally not advertised as navigation ephemerides.
    pub fn at_seconds_since_j2000(seconds_since_j2000: f64) -> Self {
        let seconds = if seconds_since_j2000.is_finite() {
            seconds_since_j2000
        } else {
            0.0
        };
        let days = seconds / SECONDS_PER_DAY;
        let sun = solar_position_m(days);
        let moon = lunar_position_m(days);
        let earth_rotation_radians = earth_rotation_angle_radians(seconds);

        Self {
            seconds_since_j2000: seconds,
            earth: BodyPose {
                center_m: Vec3d::ZERO,
                radius_m: EARTH_EQUATORIAL_RADIUS_M,
            },
            moon: BodyPose {
                center_m: moon,
                radius_m: MOON_MEAN_RADIUS_M,
            },
            sun: BodyPose {
                center_m: sun,
                radius_m: SUN_NOMINAL_RADIUS_M,
            },
            earth_rotation_radians,
        }
    }

    pub fn relative_to(self, camera_position_m: Vec3d) -> CameraRelativeFrame {
        CameraRelativeFrame {
            earth_center_m: self.earth.center_m - camera_position_m,
            moon_center_m: self.moon.center_m - camera_position_m,
            sun_center_m: self.sun.center_m - camera_position_m,
            earth_rotation_radians: self.earth_rotation_radians,
        }
    }

    /// Returns the distance to the nearest solid celestial surface.
    ///
    /// Earth uses its WGS-84 ellipsoid; Moon and Sun use their published spherical
    /// radii. A viewpoint inside a body reports zero so navigation remains bounded.
    pub fn nearest_surface_distance_m(self, position_m: Vec3d) -> f64 {
        if !position_m.is_finite() {
            return 0.0;
        }
        let earth_distance = self.earth_surface_clearance_m(position_m);
        let moon_distance = spherical_surface_distance(position_m, self.moon);
        let sun_distance = spherical_surface_distance(position_m, self.sun);
        earth_distance.min(moon_distance).min(sun_distance)
    }

    /// Clearance above the same conservative artistic terrain used by Earth collision.
    pub fn earth_surface_clearance_m(self, position_m: Vec3d) -> f64 {
        if !position_m.is_finite() || !self.earth.center_m.is_finite() {
            return 0.0;
        }
        let relative = position_m - self.earth.center_m;
        let radial_altitude = EarthEllipsoid::default().radial_altitude_m(relative);
        let earth_fixed = inertial_to_earth_fixed(relative, self.earth_rotation_radians);
        (radial_altitude - terrain_height_m(earth_fixed)).max(0.0)
    }
}

fn spherical_surface_distance(position_m: Vec3d, body: BodyPose) -> f64 {
    if !body.center_m.is_finite() || !body.radius_m.is_finite() || body.radius_m <= 0.0 {
        return f64::INFINITY;
    }
    (position_m.distance(body.center_m) - body.radius_m).max(0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRelativeFrame {
    pub earth_center_m: Vec3d,
    pub moon_center_m: Vec3d,
    pub sun_center_m: Vec3d,
    pub earth_rotation_radians: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayInterval {
    pub near_m: f64,
    pub far_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthEllipsoid {
    pub equatorial_radius_m: f64,
    pub polar_radius_m: f64,
}

impl Default for EarthEllipsoid {
    fn default() -> Self {
        Self {
            equatorial_radius_m: EARTH_EQUATORIAL_RADIUS_M,
            polar_radius_m: EARTH_POLAR_RADIUS_M,
        }
    }
}

impl EarthEllipsoid {
    pub fn flattening(self) -> f64 {
        (self.equatorial_radius_m - self.polar_radius_m) / self.equatorial_radius_m
    }

    pub fn surface_radius_m(self, direction: Vec3d) -> f64 {
        let direction = direction.normalized();
        if direction == Vec3d::ZERO {
            return self.equatorial_radius_m;
        }
        let equatorial_component = direction.x * direction.x + direction.z * direction.z;
        let polar_component = direction.y * direction.y;
        1.0 / (equatorial_component / self.equatorial_radius_m.powi(2)
            + polar_component / self.polar_radius_m.powi(2))
        .sqrt()
    }

    pub fn radial_altitude_m(self, position_m: Vec3d) -> f64 {
        position_m.length() - self.surface_radius_m(position_m)
    }

    pub fn surface_normal(self, point_m: Vec3d) -> Vec3d {
        Vec3d::new(
            point_m.x / self.equatorial_radius_m.powi(2),
            point_m.y / self.polar_radius_m.powi(2),
            point_m.z / self.equatorial_radius_m.powi(2),
        )
        .normalized()
    }

    pub fn ray_intersection(self, origin_m: Vec3d, direction: Vec3d) -> Option<RayInterval> {
        let direction = direction.normalized();
        if direction == Vec3d::ZERO {
            return None;
        }
        let a2 = self.equatorial_radius_m.powi(2);
        let b2 = self.polar_radius_m.powi(2);
        let quadratic_a = (direction.x * direction.x + direction.z * direction.z) / a2
            + direction.y * direction.y / b2;
        let quadratic_b = 2.0
            * ((origin_m.x * direction.x + origin_m.z * direction.z) / a2
                + origin_m.y * direction.y / b2);
        let quadratic_c = (origin_m.x * origin_m.x + origin_m.z * origin_m.z) / a2
            + origin_m.y * origin_m.y / b2
            - 1.0;
        let discriminant = quadratic_b * quadratic_b - 4.0 * quadratic_a * quadratic_c;
        if discriminant < 0.0 || !discriminant.is_finite() {
            return None;
        }
        let root = discriminant.sqrt();
        let near_m = (-quadratic_b - root) / (2.0 * quadratic_a);
        let far_m = (-quadratic_b + root) / (2.0 * quadratic_a);
        Some(RayInterval { near_m, far_m })
    }
}

pub fn apparent_angular_radius_radians(radius_m: f64, distance_m: f64) -> f64 {
    if radius_m <= 0.0 || distance_m <= 0.0 {
        return 0.0;
    }
    if distance_m <= radius_m {
        return std::f64::consts::FRAC_PI_2;
    }
    (radius_m / distance_m).clamp(0.0, 1.0).asin()
}

/// IERS Earth Rotation Angle using `seconds_since_j2000` as an approximate UT1 interval.
///
/// Browser wall time is UTC, not UT1, so this intentionally omits DUT1 and polar motion.
/// It nevertheless preserves the defining J2000 phase and rotation rate instead of
/// arbitrarily setting Greenwich to zero at the epoch.
pub fn earth_rotation_angle_radians(seconds_since_j2000: f64) -> f64 {
    let seconds = if seconds_since_j2000.is_finite() {
        seconds_since_j2000
    } else {
        0.0
    };
    let ut1_days = seconds / SECONDS_PER_DAY;
    (std::f64::consts::TAU
        * (J2000_EARTH_ROTATION_ANGLE_TURNS + EARTH_ROTATION_RATE_TURNS_PER_UT1_DAY * ut1_days))
        .rem_euclid(std::f64::consts::TAU)
}

fn solar_position_m(days_since_j2000: f64) -> Vec3d {
    let mean_anomaly = radians(357.529 + 0.985_600_28 * days_since_j2000);
    let mean_longitude = radians(280.459 + 0.985_647_36 * days_since_j2000);
    let ecliptic_longitude = mean_longitude
        + radians(1.915) * mean_anomaly.sin()
        + radians(0.020) * (2.0 * mean_anomaly).sin();
    let distance_au =
        1.000_14 - 0.016_71 * mean_anomaly.cos() - 0.000_14 * (2.0 * mean_anomaly).cos();
    ecliptic_to_equatorial(ecliptic_longitude, 0.0).normalized()
        * (distance_au * ASTRONOMICAL_UNIT_M)
}

fn lunar_position_m(days_since_j2000: f64) -> Vec3d {
    let mean_longitude = radians(218.316 + 13.176_396 * days_since_j2000);
    let mean_anomaly = radians(134.963 + 13.064_993 * days_since_j2000);
    let argument_of_latitude = radians(93.272 + 13.229_350 * days_since_j2000);
    let ecliptic_longitude = mean_longitude + radians(6.289) * mean_anomaly.sin();
    let ecliptic_latitude = radians(5.128) * argument_of_latitude.sin();
    let distance_m = (385_001.0 - 20_905.0 * mean_anomaly.cos()) * 1_000.0;
    ecliptic_to_equatorial(ecliptic_longitude, ecliptic_latitude).normalized() * distance_m
}

fn ecliptic_to_equatorial(longitude: f64, latitude: f64) -> Vec3d {
    let cos_latitude = latitude.cos();
    let ecliptic = Vec3d::new(
        cos_latitude * longitude.cos(),
        latitude.sin(),
        cos_latitude * longitude.sin(),
    );
    Vec3d::new(
        ecliptic.x,
        ecliptic.y * J2000_OBLIQUITY_RADIANS.cos() + ecliptic.z * J2000_OBLIQUITY_RADIANS.sin(),
        -ecliptic.y * J2000_OBLIQUITY_RADIANS.sin() + ecliptic.z * J2000_OBLIQUITY_RADIANS.cos(),
    )
}

fn radians(degrees: f64) -> f64 {
    (degrees * DEGREES_TO_RADIANS).rem_euclid(std::f64::consts::TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_dimensions_preserve_real_scale_ratios() {
        let moon_ratio = MOON_MEAN_RADIUS_M / EARTH_EQUATORIAL_RADIUS_M;
        let sun_ratio = SUN_NOMINAL_RADIUS_M / EARTH_EQUATORIAL_RADIUS_M;
        let lunar_distance_ratio = MEAN_EARTH_MOON_DISTANCE_M / EARTH_EQUATORIAL_RADIUS_M;

        assert!((moon_ratio - 0.2725).abs() < 0.001);
        assert!((sun_ratio - 109.08).abs() < 0.1);
        assert!((lunar_distance_ratio - 60.27).abs() < 0.1);
    }

    #[test]
    fn nearest_surface_distance_tracks_each_body_at_its_own_scale() {
        let frame = CelestialFrame::at_seconds_since_j2000(12_345.0);
        for body in [frame.moon, frame.sun] {
            let position = body.center_m + Vec3d::Z * (body.radius_m * 4.0);
            let clearance = frame.nearest_surface_distance_m(position);
            assert!((clearance / body.radius_m - 3.0).abs() < 1.0e-9);
        }

        let earth_position = Vec3d::Z * (EARTH_EQUATORIAL_RADIUS_M * 4.0);
        let earth_clearance = frame.nearest_surface_distance_m(earth_position);
        assert!(earth_clearance <= EARTH_EQUATORIAL_RADIUS_M * 3.0);
        assert!(earth_clearance >= EARTH_EQUATORIAL_RADIUS_M * 3.0 - 10_000.0);
        assert_eq!(frame.nearest_surface_distance_m(Vec3d::ZERO), 0.0);
        assert_eq!(
            frame.nearest_surface_distance_m(Vec3d::new(f64::NAN, 0.0, 0.0)),
            0.0
        );
    }

    #[test]
    fn iers_earth_rotation_angle_has_the_correct_j2000_phase() {
        let angle = earth_rotation_angle_radians(0.0);
        let expected = std::f64::consts::TAU * J2000_EARTH_ROTATION_ANGLE_TURNS;
        assert!((angle - expected).abs() < 1.0e-14);
        assert!((angle.to_degrees() - 280.460_618_375_04).abs() < 1.0e-10);
    }

    #[test]
    fn iers_earth_rotation_rate_matches_the_sidereal_day() {
        let derived_sidereal_day = SECONDS_PER_DAY / EARTH_ROTATION_RATE_TURNS_PER_UT1_DAY;
        assert!((derived_sidereal_day - EARTH_SIDEREAL_ROTATION_S).abs() < 0.02);

        let initial = earth_rotation_angle_radians(0.0);
        let after_sidereal_day = earth_rotation_angle_radians(derived_sidereal_day);
        let wrapped_difference = (after_sidereal_day - initial)
            .rem_euclid(std::f64::consts::TAU)
            .min((initial - after_sidereal_day).rem_euclid(std::f64::consts::TAU));
        assert!(wrapped_difference < 1.0e-12);
    }

    #[test]
    fn solar_declination_has_the_correct_northern_seasons() {
        let june_solstice = CelestialFrame::at_seconds_since_j2000(171.0 * SECONDS_PER_DAY)
            .sun
            .center_m;
        let december_solstice = CelestialFrame::at_seconds_since_j2000(355.0 * SECONDS_PER_DAY)
            .sun
            .center_m;
        let june_declination = (june_solstice.y / june_solstice.length())
            .asin()
            .to_degrees();
        let december_declination = (december_solstice.y / december_solstice.length())
            .asin()
            .to_degrees();

        assert!((22.0..24.0).contains(&june_declination));
        assert!((-24.0..-22.0).contains(&december_declination));
    }

    #[test]
    fn nonfinite_rotation_time_falls_back_to_j2000() {
        let j2000 = earth_rotation_angle_radians(0.0);
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(earth_rotation_angle_radians(seconds), j2000);
        }
    }

    #[test]
    fn apparent_solar_and_lunar_diameters_are_close_to_half_a_degree() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let solar_diameter = 2.0
            * apparent_angular_radius_radians(SUN_NOMINAL_RADIUS_M, frame.sun.center_m.length())
                .to_degrees();
        let lunar_diameter = 2.0
            * apparent_angular_radius_radians(MOON_MEAN_RADIUS_M, frame.moon.center_m.length())
                .to_degrees();

        assert!((0.50..0.55).contains(&solar_diameter));
        assert!((0.48..0.56).contains(&lunar_diameter));
    }

    #[test]
    fn lunar_model_retains_plausible_distance_and_inclination_changes() {
        let mut minimum_distance = f64::MAX;
        let mut maximum_distance: f64 = 0.0;
        let mut maximum_latitude: f64 = 0.0;
        for day in 0..28 {
            let moon = CelestialFrame::at_seconds_since_j2000(day as f64 * SECONDS_PER_DAY).moon;
            minimum_distance = minimum_distance.min(moon.center_m.length());
            maximum_distance = maximum_distance.max(moon.center_m.length());
            maximum_latitude = maximum_latitude.max(
                (moon.center_m.y / moon.center_m.length())
                    .clamp(-1.0, 1.0)
                    .asin()
                    .abs(),
            );
        }

        assert!(minimum_distance < MEAN_EARTH_MOON_DISTANCE_M);
        assert!(maximum_distance > MEAN_EARTH_MOON_DISTANCE_M);
        assert!(maximum_latitude.to_degrees() > 10.0);
    }

    #[test]
    fn camera_relative_conversion_keeps_metre_scale_detail_near_the_moon() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let camera = frame.moon.center_m + Vec3d::new(0.0, 0.0, MOON_MEAN_RADIUS_M + 10.0);
        let relative = frame.relative_to(camera);
        let moon_gpu = relative.moon_center_m.to_earth_radii_f32();
        let expected = -((MOON_MEAN_RADIUS_M + 10.0) / EARTH_EQUATORIAL_RADIUS_M) as f32;

        assert!((moon_gpu[2] - expected).abs() < 1.0e-7);
    }

    #[test]
    fn ellipsoid_intersection_and_normal_respect_polar_flattening() {
        let earth = EarthEllipsoid::default();
        assert!((earth.flattening() - 1.0 / 298.257_223_563).abs() < 1.0e-12);
        assert!(earth.surface_radius_m(Vec3d::Y) < earth.surface_radius_m(Vec3d::X));

        let origin = Vec3d::new(0.0, 0.0, EARTH_EQUATORIAL_RADIUS_M * 2.0);
        let hit = earth
            .ray_intersection(origin, Vec3d::new(0.0, 0.0, -1.0))
            .expect("ray must hit Earth");
        assert!((hit.near_m - EARTH_EQUATORIAL_RADIUS_M).abs() < 1.0e-6);
        let point = origin + Vec3d::new(0.0, 0.0, -1.0) * hit.near_m;
        assert_eq!(earth.surface_normal(point), Vec3d::Z);
    }
}
