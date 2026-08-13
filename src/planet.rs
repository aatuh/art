//! Renderer-independent physical dimensions and low-cost celestial ephemerides.
//!
//! All positions and distances use SI metres and `f64`. Browser renderers should subtract
//! the camera origin before converting values to `f32`; casting absolute astronomical
//! coordinates directly to the GPU loses the precision needed near a surface.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

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
        let earth_rotation_radians = (seconds / EARTH_SIDEREAL_ROTATION_S * std::f64::consts::TAU)
            .rem_euclid(std::f64::consts::TAU);

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
        ecliptic.y * J2000_OBLIQUITY_RADIANS.cos() - ecliptic.z * J2000_OBLIQUITY_RADIANS.sin(),
        ecliptic.y * J2000_OBLIQUITY_RADIANS.sin() + ecliptic.z * J2000_OBLIQUITY_RADIANS.cos(),
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
