//! Virtual camera navigation for the astronomical artwork.
//!
//! This controls only a rendered viewpoint. It is not a spacecraft, vehicle controller,
//! guidance system, or orbital-dynamics model.

use crate::{
    earth_coordinates::{earth_fixed_to_inertial, inertial_to_earth_fixed},
    earth_terrain::{MAX_TERRAIN_HEIGHT_M, terrain_height_m},
    planet::{
        ASTRONOMICAL_UNIT_M, CelestialFrame, EARTH_EQUATORIAL_RADIUS_M, EarthEllipsoid, Vec3d,
    },
};

pub const MIN_CAMERA_SPEED_MPS: f64 = 0.25;
pub const MAX_CAMERA_SPEED_MPS: f64 = ASTRONOMICAL_UNIT_M / 10.0;
pub const DEFAULT_CAMERA_SPEED_MPS: f64 = EARTH_EQUATORIAL_RADIUS_M * 3.0 * 0.04;
pub const MIN_SPEED_MULTIPLIER: f64 = 0.01;
pub const MAX_SPEED_MULTIPLIER: f64 = 100.0;
pub const DEFAULT_SPEED_MULTIPLIER: f64 = 1.0;
pub const CELESTIAL_SURFACE_CLEARANCE_M: f64 = 2.0;
const CLEARANCE_SPEED_RATIO: f64 = 0.04;
const FAST_TRAVEL_MULTIPLIER: f64 = 12.0;
const SPEED_STEP_RATIO: f64 = 1.778_279_410_038_922_8;
const MAX_FRAME_SECONDS: f64 = 0.1;
const MAX_CAMERA_DISTANCE_M: f64 = ASTRONOMICAL_UNIT_M * 100.0;
const INSPECTION_DISTANCE_RADII: f64 = 4.0;
const SURFACE_INSPECTION_CLEARANCE_M: f64 = 300.0;
const SURFACE_LOOK_DISTANCE_M: f64 = 20_000.0;
const SURFACE_LOOK_DOWN_RADIANS: f64 = 18.0_f64.to_radians();
const SURFACE_INSPECTION_SITES_DEGREES: [[f64; 2]; 6] = [
    [-13.8, -171.8], // Samoa
    [-6.0, -76.0],   // Peruvian Andes
    [-3.0, -60.0],   // western Amazon
    [3.0, 36.0],     // Kenyan highlands
    [7.0, 81.0],     // Sri Lanka
    [1.0, 114.0],    // Borneo
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CelestialTarget {
    Earth,
    EarthSurface,
    Moon,
    Sun,
}

impl CelestialTarget {
    pub const fn next(self) -> Self {
        match self {
            Self::Earth => Self::EarthSurface,
            Self::EarthSurface => Self::Moon,
            Self::Moon => Self::Sun,
            Self::Sun => Self::Earth,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Earth => "Earth",
            Self::EarthSurface => "Earth surface",
            Self::Moon => "Moon",
            Self::Sun => "Sun",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpaceflightInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub ascend: bool,
    pub descend: bool,
    pub accelerated: bool,
    pub stop: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpaceflightState {
    pub position_m: Vec3d,
    pub velocity_mps: Vec3d,
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub camera_speed_mps: f64,
    pub speed_multiplier: f64,
}

impl Default for SpaceflightState {
    fn default() -> Self {
        Self {
            position_m: Vec3d::new(0.0, 0.0, EARTH_EQUATORIAL_RADIUS_M * 4.0),
            velocity_mps: Vec3d::ZERO,
            yaw_radians: 0.0,
            pitch_radians: 0.0,
            camera_speed_mps: DEFAULT_CAMERA_SPEED_MPS,
            speed_multiplier: DEFAULT_SPEED_MULTIPLIER,
        }
    }
}

impl SpaceflightState {
    pub fn look(&mut self, delta_x: f64, delta_y: f64, radians_per_unit: f64) {
        if !(delta_x.is_finite() && delta_y.is_finite() && radians_per_unit.is_finite()) {
            return;
        }
        self.yaw_radians =
            (self.yaw_radians + delta_x * radians_per_unit).rem_euclid(std::f64::consts::TAU);
        self.pitch_radians = (self.pitch_radians + delta_y * radians_per_unit).clamp(
            -std::f64::consts::FRAC_PI_2 + 0.001,
            std::f64::consts::FRAC_PI_2 - 0.001,
        );
    }

    pub fn camera_basis(self) -> CameraBasis {
        let cos_pitch = self.pitch_radians.cos();
        let forward = Vec3d::new(
            self.yaw_radians.sin() * cos_pitch,
            -self.pitch_radians.sin(),
            -self.yaw_radians.cos() * cos_pitch,
        )
        .normalized();
        let right = Vec3d::new(self.yaw_radians.cos(), 0.0, self.yaw_radians.sin()).normalized();
        let up = right.cross(forward).normalized();
        CameraBasis { forward, right, up }
    }

    pub fn adjust_camera_speed(&mut self, steps: i32) {
        let multiplier = SPEED_STEP_RATIO.powi(steps.clamp(-64, 64));
        self.speed_multiplier =
            (self.speed_multiplier * multiplier).clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
    }

    pub fn effective_camera_speed_mps(
        self,
        nearest_surface_distance_m: f64,
        accelerated: bool,
    ) -> f64 {
        let clearance = if nearest_surface_distance_m.is_finite() {
            nearest_surface_distance_m.max(0.0)
        } else {
            0.0
        };
        let base_speed =
            (clearance * CLEARANCE_SPEED_RATIO).clamp(MIN_CAMERA_SPEED_MPS, MAX_CAMERA_SPEED_MPS);
        let acceleration = if accelerated {
            FAST_TRAVEL_MULTIPLIER
        } else {
            1.0
        };
        (base_speed * self.speed_multiplier * acceleration)
            .clamp(MIN_CAMERA_SPEED_MPS, MAX_CAMERA_SPEED_MPS)
    }

    pub fn point_at(&mut self, point_m: Vec3d) {
        let direction = (point_m - self.position_m).normalized();
        if direction == Vec3d::ZERO {
            return;
        }
        self.yaw_radians = direction
            .x
            .atan2(-direction.z)
            .rem_euclid(std::f64::consts::TAU);
        self.pitch_radians = (-direction.y.asin()).clamp(
            -std::f64::consts::FRAC_PI_2 + 0.001,
            std::f64::consts::FRAC_PI_2 - 0.001,
        );
    }

    pub fn point_at_earth(&mut self) {
        self.point_at(Vec3d::ZERO);
    }

    pub fn reset_view(&mut self) {
        *self = Self::default();
    }

    /// Moves the virtual viewpoint to a useful, physically scaled inspection pose.
    ///
    /// Earth retains the installation's authored opening direction. The Moon is viewed
    /// from its Earth-facing side so its real phase remains meaningful, and the Sun is
    /// viewed from the Earth-facing side. Distances are expressed in each body's true
    /// radius; no celestial size or separation is changed.
    pub fn inspect_celestial_target(
        &mut self,
        target: CelestialTarget,
        frame: crate::planet::CelestialFrame,
    ) {
        if target == CelestialTarget::EarthSurface {
            self.inspect_earth_surface(frame);
            return;
        }
        let (center_m, radius_m, outward_direction) = match target {
            CelestialTarget::Earth => (frame.earth.center_m, frame.earth.radius_m, Vec3d::Z),
            CelestialTarget::EarthSurface => unreachable!("surface preset handled above"),
            CelestialTarget::Moon => (
                frame.moon.center_m,
                frame.moon.radius_m,
                (frame.earth.center_m - frame.moon.center_m).normalized(),
            ),
            CelestialTarget::Sun => (
                frame.sun.center_m,
                frame.sun.radius_m,
                (frame.earth.center_m - frame.sun.center_m).normalized(),
            ),
        };
        if !center_m.is_finite()
            || !radius_m.is_finite()
            || radius_m <= 0.0
            || outward_direction == Vec3d::ZERO
        {
            self.reset_view();
            return;
        }
        self.position_m = center_m + outward_direction * (radius_m * INSPECTION_DISTANCE_RADII);
        self.velocity_mps = Vec3d::ZERO;
        self.point_at(center_m);
    }

    fn inspect_earth_surface(&mut self, frame: CelestialFrame) {
        let sun_direction = (frame.sun.center_m - frame.earth.center_m).normalized();
        let mut selected_outward = Vec3d::ZERO;
        let mut selected_north = Vec3d::ZERO;
        let mut selected_terrain_height_m = 0.0;
        let mut selected_sunlight = f64::NEG_INFINITY;
        for [latitude_degrees, longitude_degrees] in SURFACE_INSPECTION_SITES_DEGREES {
            let latitude = latitude_degrees.to_radians();
            let longitude = longitude_degrees.to_radians();
            let fixed_outward = Vec3d::new(
                latitude.cos() * longitude.cos(),
                latitude.sin(),
                latitude.cos() * longitude.sin(),
            );
            let fixed_north = Vec3d::new(
                -latitude.sin() * longitude.cos(),
                latitude.cos(),
                -latitude.sin() * longitude.sin(),
            );
            let outward = earth_fixed_to_inertial(fixed_outward, frame.earth_rotation_radians);
            let sunlight = outward.dot(sun_direction);
            if sunlight > selected_sunlight {
                selected_outward = outward;
                selected_north = earth_fixed_to_inertial(fixed_north, frame.earth_rotation_radians);
                selected_terrain_height_m = terrain_height_m(fixed_outward);
                selected_sunlight = sunlight;
            }
        }

        if selected_outward == Vec3d::ZERO || selected_north == Vec3d::ZERO {
            self.reset_view();
            return;
        }
        let ellipsoid = EarthEllipsoid::default();
        let surface_radius_m = ellipsoid.surface_radius_m(selected_outward);
        self.position_m = frame.earth.center_m
            + selected_outward
                * (surface_radius_m + selected_terrain_height_m + SURFACE_INSPECTION_CLEARANCE_M);
        self.velocity_mps = Vec3d::ZERO;
        let forward = (selected_north * SURFACE_LOOK_DOWN_RADIANS.cos()
            - selected_outward * SURFACE_LOOK_DOWN_RADIANS.sin())
        .normalized();
        self.point_at(self.position_m + forward * SURFACE_LOOK_DISTANCE_M);
    }

    pub fn radial_altitude_above_earth_m(self) -> f64 {
        EarthEllipsoid::default().radial_altitude_m(self.position_m)
    }

    /// Keeps the virtual camera outside the solid Earth, Moon, and Sun.
    ///
    /// `previous_position_m` is the position immediately before this frame's movement. It lets
    /// the constraint stop fast movement at the first surface instead of allowing a complete
    /// body crossing whose endpoint happens to be outside again. The returned value reports
    /// whether position or velocity recovery was required.
    pub fn constrain_to_celestial_frame(
        &mut self,
        previous_position_m: Vec3d,
        frame: CelestialFrame,
    ) -> bool {
        let mut constrained = false;
        let mut previous_position_m = if previous_position_m.is_finite() {
            previous_position_m
        } else {
            self.position_m
        };

        // Reset is an explicit teleport, not a movement segment through
        // intervening bodies. Its collision sweep starts at the destination
        // instead of the old surface view.
        if *self == Self::default() {
            previous_position_m = self.position_m;
        }

        if !self.position_m.is_finite() || !self.velocity_mps.is_finite() {
            self.reset_view();
            constrained = true;
        }

        constrained |= constrain_to_earth(
            &mut self.position_m,
            &mut self.velocity_mps,
            previous_position_m,
            frame.earth.center_m,
            frame.earth_rotation_radians,
        );
        constrained |= constrain_to_sphere(
            &mut self.position_m,
            &mut self.velocity_mps,
            previous_position_m,
            frame.moon.center_m,
            frame.moon.radius_m,
        );
        constrained |= constrain_to_sphere(
            &mut self.position_m,
            &mut self.velocity_mps,
            previous_position_m,
            frame.sun.center_m,
            frame.sun.radius_m,
        );

        if !self.position_m.is_finite() || !self.velocity_mps.is_finite() {
            self.reset_view();
            return true;
        }
        constrained
    }

    /// Advances the virtual camera using Earth clearance as a compatibility fallback.
    pub fn tick(&mut self, frame_seconds: f64, input: SpaceflightInput) {
        let earth_clearance = self.radial_altitude_above_earth_m().max(0.0);
        self.tick_with_clearance(frame_seconds, input, earth_clearance);
    }

    /// Advances the virtual camera at a speed proportional to the nearest celestial surface.
    pub fn tick_with_clearance(
        &mut self,
        frame_seconds: f64,
        input: SpaceflightInput,
        nearest_surface_distance_m: f64,
    ) {
        let dt = frame_seconds.clamp(0.0, MAX_FRAME_SECONDS);
        if dt <= 0.0 {
            return;
        }
        let basis = self.camera_basis();
        let requested = basis.forward * axis(input.forward, input.backward)
            + basis.right * axis(input.right, input.left)
            + basis.up * axis(input.ascend, input.descend);
        let requested = if requested.length_squared() > 1.0 {
            requested.normalized()
        } else {
            requested
        };
        self.camera_speed_mps =
            self.effective_camera_speed_mps(nearest_surface_distance_m, input.accelerated);
        let desired_velocity = if input.stop {
            Vec3d::ZERO
        } else {
            requested * self.camera_speed_mps
        };
        let response_seconds: f64 = if input.stop { 0.045 } else { 0.14 };
        let blend = 1.0 - (-dt / response_seconds).exp();
        self.velocity_mps += (desired_velocity - self.velocity_mps) * blend;
        self.position_m += self.velocity_mps * dt;

        if !self.position_m.is_finite()
            || !self.velocity_mps.is_finite()
            || self.position_m.length() > MAX_CAMERA_DISTANCE_M
        {
            self.reset_view();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraBasis {
    pub forward: Vec3d,
    pub right: Vec3d,
    pub up: Vec3d,
}

fn axis(positive: bool, negative: bool) -> f64 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

fn constrain_to_earth(
    position_m: &mut Vec3d,
    velocity_mps: &mut Vec3d,
    previous_position_m: Vec3d,
    center_m: Vec3d,
    earth_rotation_radians: f64,
) -> bool {
    if !center_m.is_finite() || !earth_rotation_radians.is_finite() {
        return false;
    }
    let ellipsoid = EarthEllipsoid::default();
    let previous_relative = previous_position_m - center_m;
    let current_relative = *position_m - center_m;

    let contact_relative = if terrain_clearance_m(previous_relative, earth_rotation_radians)
        >= CELESTIAL_SURFACE_CLEARANCE_M
    {
        segment_terrain_entry(previous_relative, current_relative, earth_rotation_radians)
    } else {
        None
    };
    let contact_relative = contact_relative.or_else(|| {
        (terrain_clearance_m(current_relative, earth_rotation_radians)
            < CELESTIAL_SURFACE_CLEARANCE_M)
            .then(|| project_to_terrain(current_relative, *velocity_mps, earth_rotation_radians))
    });
    let Some(contact_relative) = contact_relative else {
        return false;
    };

    *position_m = center_m + contact_relative;
    remove_inward_velocity(velocity_mps, ellipsoid.surface_normal(contact_relative));
    true
}

fn terrain_clearance_m(relative_position_m: Vec3d, earth_rotation_radians: f64) -> f64 {
    if !relative_position_m.is_finite() {
        return f64::NEG_INFINITY;
    }
    let ellipsoid = EarthEllipsoid::default();
    let earth_fixed = inertial_to_earth_fixed(relative_position_m, earth_rotation_radians);
    ellipsoid.radial_altitude_m(relative_position_m) - terrain_height_m(earth_fixed)
}

fn terrain_surface_radius_m(direction: Vec3d, earth_rotation_radians: f64) -> f64 {
    let ellipsoid = EarthEllipsoid::default();
    let outward = direction.normalized();
    ellipsoid.surface_radius_m(outward)
        + terrain_height_m(inertial_to_earth_fixed(outward, earth_rotation_radians))
        + CELESTIAL_SURFACE_CLEARANCE_M
}

fn segment_terrain_entry(
    start_m: Vec3d,
    end_m: Vec3d,
    earth_rotation_radians: f64,
) -> Option<Vec3d> {
    let movement_m = end_m - start_m;
    let movement_length_m = movement_m.length();
    if movement_length_m <= f64::EPSILON || !movement_length_m.is_finite() {
        return None;
    }
    let direction = movement_m / movement_length_m;
    let outer = EarthEllipsoid {
        equatorial_radius_m: EarthEllipsoid::default().equatorial_radius_m
            + MAX_TERRAIN_HEIGHT_M
            + CELESTIAL_SURFACE_CLEARANCE_M,
        polar_radius_m: EarthEllipsoid::default().polar_radius_m
            + MAX_TERRAIN_HEIGHT_M
            + CELESTIAL_SURFACE_CLEARANCE_M,
    }
    .ray_intersection(start_m, direction)?;
    let start_distance = outer.near_m.max(0.0);
    let end_distance = outer.far_m.min(movement_length_m);
    if end_distance < start_distance || start_distance > movement_length_m {
        return None;
    }

    let signed_clearance = |distance_m: f64| {
        terrain_clearance_m(start_m + direction * distance_m, earth_rotation_radians)
            - CELESTIAL_SURFACE_CLEARANCE_M
    };
    let mut previous_distance = start_distance;
    if signed_clearance(previous_distance) <= 0.0 {
        return Some(project_to_terrain(
            start_m + direction * previous_distance,
            movement_m,
            earth_rotation_radians,
        ));
    }
    for sample_index in 1..=16 {
        let distance =
            start_distance + (end_distance - start_distance) * f64::from(sample_index) / 16.0;
        if signed_clearance(distance) <= 0.0 {
            let mut low = previous_distance;
            let mut high = distance;
            for _ in 0..12 {
                let middle = 0.5 * (low + high);
                if signed_clearance(middle) <= 0.0 {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return Some(project_to_terrain(
                start_m + direction * high,
                movement_m,
                earth_rotation_radians,
            ));
        }
        previous_distance = distance;
    }
    None
}

fn project_to_terrain(
    point_m: Vec3d,
    fallback_velocity_mps: Vec3d,
    earth_rotation_radians: f64,
) -> Vec3d {
    let outward = fallback_outward(point_m, fallback_velocity_mps);
    outward * terrain_surface_radius_m(outward, earth_rotation_radians)
}

fn constrain_to_sphere(
    position_m: &mut Vec3d,
    velocity_mps: &mut Vec3d,
    previous_position_m: Vec3d,
    center_m: Vec3d,
    radius_m: f64,
) -> bool {
    if !center_m.is_finite() || !radius_m.is_finite() || radius_m <= 0.0 {
        return false;
    }
    let minimum_radius_m = radius_m + CELESTIAL_SURFACE_CLEARANCE_M;
    let previous_relative = previous_position_m - center_m;
    let current_relative = *position_m - center_m;
    let contact_relative =
        if previous_relative.length_squared() >= minimum_radius_m * minimum_radius_m {
            segment_sphere_entry(previous_relative, current_relative, minimum_radius_m)
        } else {
            None
        };
    let contact_relative = contact_relative.or_else(|| {
        (current_relative.length_squared() < minimum_radius_m * minimum_radius_m)
            .then(|| fallback_outward(current_relative, *velocity_mps) * minimum_radius_m)
    });
    let Some(contact_relative) = contact_relative else {
        return false;
    };

    let outward = contact_relative.normalized();
    *position_m = center_m + outward * minimum_radius_m;
    remove_inward_velocity(velocity_mps, outward);
    true
}

fn segment_sphere_entry(start_m: Vec3d, end_m: Vec3d, radius_m: f64) -> Option<Vec3d> {
    let movement = end_m - start_m;
    let a = movement.length_squared();
    if a <= f64::EPSILON || !a.is_finite() {
        return None;
    }
    let half_b = start_m.dot(movement);
    let c = start_m.length_squared() - radius_m * radius_m;
    let discriminant = half_b * half_b - a * c;
    if discriminant < 0.0 || !discriminant.is_finite() {
        return None;
    }
    let entry = (-half_b - discriminant.sqrt()) / a;
    if !(-1.0e-12..=1.0).contains(&entry) {
        return None;
    }
    let contact = start_m + movement * entry.max(0.0);
    Some(fallback_outward(contact, movement) * radius_m)
}

fn fallback_outward(relative_position_m: Vec3d, velocity_mps: Vec3d) -> Vec3d {
    let radial = relative_position_m.normalized();
    if radial != Vec3d::ZERO {
        radial
    } else {
        let against_motion = (-velocity_mps).normalized();
        if against_motion == Vec3d::ZERO {
            Vec3d::Z
        } else {
            against_motion
        }
    }
}

fn remove_inward_velocity(velocity_mps: &mut Vec3d, outward_normal: Vec3d) {
    let inward_speed_mps = velocity_mps.dot(outward_normal);
    if inward_speed_mps < 0.0 {
        *velocity_mps -= outward_normal * inward_speed_mps;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_view_faces_earth_and_has_visible_orbital_speed() {
        let camera = SpaceflightState::default();
        assert!(
            camera
                .camera_basis()
                .forward
                .dot((-camera.position_m).normalized())
                > 0.999_999
        );
        assert!(camera.camera_speed_mps > 700_000.0);
    }

    #[test]
    fn diagonal_camera_input_is_normalized() {
        let mut camera = SpaceflightState::default();
        let initial = camera.position_m;
        for _ in 0..30 {
            camera.tick(
                1.0 / 60.0,
                SpaceflightInput {
                    forward: true,
                    right: true,
                    ascend: true,
                    ..SpaceflightInput::default()
                },
            );
        }
        let movement = camera.position_m - initial;
        assert!(movement.x > 0.0 && movement.y > 0.0 && movement.z < 0.0);
        assert!(camera.velocity_mps.length() <= camera.camera_speed_mps * 1.000_001);
    }

    #[test]
    fn adaptive_speed_remains_precise_near_a_surface_and_visible_in_orbit() {
        let camera = SpaceflightState::default();
        let walking_scale = camera.effective_camera_speed_mps(10.0, false);
        let low_orbit = camera.effective_camera_speed_mps(400_000.0, false);
        let initial_view =
            camera.effective_camera_speed_mps(EARTH_EQUATORIAL_RADIUS_M * 3.0, false);
        let fast_travel = camera.effective_camera_speed_mps(EARTH_EQUATORIAL_RADIUS_M * 3.0, true);

        assert!((walking_scale - 0.4).abs() < 1.0e-12);
        assert!((low_orbit - 16_000.0).abs() < 1.0e-9);
        assert!(initial_view > 700_000.0);
        assert!(fast_travel > initial_view * 11.9);
    }

    #[test]
    fn speed_steps_cover_slow_inspection_and_fast_travel() {
        let mut camera = SpaceflightState::default();
        camera.adjust_camera_speed(-64);
        assert_eq!(camera.speed_multiplier, MIN_SPEED_MULTIPLIER);
        camera.adjust_camera_speed(64);
        assert_eq!(camera.speed_multiplier, MAX_SPEED_MULTIPLIER);
    }

    #[test]
    fn point_at_orients_the_camera_exactly() {
        let mut camera = SpaceflightState::default();
        let point = Vec3d::new(10_000_000.0, 7_000_000.0, -3_000_000.0);
        camera.point_at(point);
        assert!(
            camera
                .camera_basis()
                .forward
                .dot((point - camera.position_m).normalized())
                > 0.999_999
        );
    }

    #[test]
    fn inspection_presets_keep_true_scale_and_face_each_target() {
        let frame = CelestialFrame::at_seconds_since_j2000(12_345.0);
        for (target, center, radius) in [
            (
                CelestialTarget::Earth,
                frame.earth.center_m,
                frame.earth.radius_m,
            ),
            (
                CelestialTarget::Moon,
                frame.moon.center_m,
                frame.moon.radius_m,
            ),
            (CelestialTarget::Sun, frame.sun.center_m, frame.sun.radius_m),
        ] {
            let mut camera = SpaceflightState {
                velocity_mps: Vec3d::new(1.0, 2.0, 3.0),
                ..SpaceflightState::default()
            };
            camera.inspect_celestial_target(target, frame);

            assert!(
                (camera.position_m.distance(center) / radius - INSPECTION_DISTANCE_RADII).abs()
                    < 1.0e-9
            );
            assert!(
                camera
                    .camera_basis()
                    .forward
                    .dot((center - camera.position_m).normalized())
                    > 0.999_999
            );
            assert_eq!(camera.velocity_mps, Vec3d::ZERO);
        }
    }

    #[test]
    fn earth_surface_preset_selects_daylight_land_at_terrain_clearance() {
        let ellipsoid = EarthEllipsoid::default();
        for seconds in [0.0, 7_654_321.0, 31_556_925.0] {
            let frame = CelestialFrame::at_seconds_since_j2000(seconds);
            let mut camera = SpaceflightState::default();
            camera.inspect_celestial_target(CelestialTarget::EarthSurface, frame);

            let relative = camera.position_m - frame.earth.center_m;
            let outward = relative.normalized();
            let fixed = inertial_to_earth_fixed(outward, frame.earth_rotation_radians);
            let terrain_height = terrain_height_m(fixed);
            assert!(
                (ellipsoid.radial_altitude_m(relative)
                    - terrain_height
                    - SURFACE_INSPECTION_CLEARANCE_M)
                    .abs()
                    < 1.0e-6
            );
            let sunlight = (frame.sun.center_m - frame.earth.center_m).normalized();
            assert!(outward.dot(sunlight) > 0.45);
            let basis = camera.camera_basis();
            let forward = basis.forward;
            assert!((forward.dot(outward) + 18.0_f64.to_radians().sin()).abs() < 1.0e-9);
            let surface_normal = ellipsoid.surface_normal(relative);
            assert!(basis.right.dot(surface_normal).abs() < 1.0e-9);
            assert!(basis.up.dot(surface_normal) > 0.90);
            let local_terrain = EarthEllipsoid {
                equatorial_radius_m: ellipsoid.equatorial_radius_m + terrain_height,
                polar_radius_m: ellipsoid.polar_radius_m + terrain_height,
            };
            let ground_hit = local_terrain
                .ray_intersection(relative, forward)
                .expect("surface preset center ray must meet the ground");
            assert!((800.0..1_200.0).contains(&ground_hit.near_m));
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let point = relative + forward * (ground_hit.near_m * fraction);
                assert!(local_terrain.radial_altitude_m(point) < 1_500.0);
            }
            assert_eq!(camera.velocity_mps, Vec3d::ZERO);
        }
    }

    #[test]
    fn earth_surface_catalog_stays_in_daylight_across_day_and_season() {
        for day in [0.0, 91.0, 182.0, 273.0] {
            for hour in 0..24 {
                let frame = CelestialFrame::at_seconds_since_j2000(
                    (day + f64::from(hour) / 24.0) * 86_400.0,
                );
                let mut camera = SpaceflightState::default();
                camera.inspect_celestial_target(CelestialTarget::EarthSurface, frame);
                let outward = (camera.position_m - frame.earth.center_m).normalized();
                let sunlight = (frame.sun.center_m - frame.earth.center_m).normalized();

                assert!(outward.dot(sunlight) > 0.45);
            }
        }
    }

    #[test]
    fn reset_from_earth_surface_is_not_swept_back_into_the_planet() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let mut camera = SpaceflightState::default();
        camera.inspect_celestial_target(CelestialTarget::EarthSurface, frame);
        let surface_position = camera.position_m;

        camera.reset_view();
        let reset = camera;

        assert!(!camera.constrain_to_celestial_frame(surface_position, frame));
        assert_eq!(camera, reset);
        assert!(camera.radial_altitude_above_earth_m() > EARTH_EQUATORIAL_RADIUS_M * 2.0);
    }

    #[test]
    fn inspection_target_order_is_stable_for_the_browser_control() {
        assert_eq!(CelestialTarget::Earth.next(), CelestialTarget::EarthSurface);
        assert_eq!(CelestialTarget::EarthSurface.next(), CelestialTarget::Moon);
        assert_eq!(CelestialTarget::Moon.next(), CelestialTarget::Sun);
        assert_eq!(CelestialTarget::Sun.next(), CelestialTarget::Earth);
    }

    #[test]
    fn ordinary_open_space_motion_is_not_constrained() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let mut camera = SpaceflightState::default();
        let previous_position = camera.position_m;
        camera.tick(
            1.0 / 60.0,
            SpaceflightInput {
                right: true,
                ..SpaceflightInput::default()
            },
        );
        let moved = camera;

        assert!(!camera.constrain_to_celestial_frame(previous_position, frame));
        assert_eq!(camera, moved);
    }

    #[test]
    fn earth_penetration_recovers_to_terrain_clearance() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let earth = EarthEllipsoid::default();
        let terrain_height = terrain_height_m(inertial_to_earth_fixed(
            Vec3d::Y,
            frame.earth_rotation_radians,
        ));
        let previous_position =
            frame.earth.center_m + Vec3d::Y * (earth.polar_radius_m + terrain_height + 100.0);
        let mut camera = SpaceflightState {
            position_m: frame.earth.center_m + Vec3d::Y * (earth.polar_radius_m - 100.0),
            velocity_mps: Vec3d::new(13.0, -500.0, 7.0),
            ..SpaceflightState::default()
        };

        assert!(camera.constrain_to_celestial_frame(previous_position, frame));
        let relative = camera.position_m - frame.earth.center_m;
        let clearance = earth.radial_altitude_m(relative) - terrain_height;
        let normal = earth.surface_normal(relative);
        assert!((clearance - CELESTIAL_SURFACE_CLEARANCE_M).abs() < 1.0e-6);
        assert!(camera.velocity_mps.dot(normal) >= -1.0e-9);
        assert!((camera.velocity_mps.x - 13.0).abs() < 1.0e-9);
        assert!((camera.velocity_mps.z - 7.0).abs() < 1.0e-9);
    }

    #[test]
    fn moon_and_sun_penetration_recover_to_actual_radii() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        for body in [frame.moon, frame.sun] {
            let previous_position =
                body.center_m + Vec3d::X * (body.radius_m + CELESTIAL_SURFACE_CLEARANCE_M + 50.0);
            let mut camera = SpaceflightState {
                position_m: body.center_m + Vec3d::X * (body.radius_m - 50.0),
                velocity_mps: Vec3d::new(-800.0, 19.0, 0.0),
                ..SpaceflightState::default()
            };

            assert!(camera.constrain_to_celestial_frame(previous_position, frame));
            let relative = camera.position_m - body.center_m;
            assert!(
                (relative.length() - body.radius_m - CELESTIAL_SURFACE_CLEARANCE_M).abs() < 1.0e-5
            );
            let normal = relative.normalized();
            assert!(camera.velocity_mps.dot(normal) >= -1.0e-8);
            assert!((camera.velocity_mps.y - 19.0).abs() < 1.0e-8);
        }
    }

    #[test]
    fn swept_constraint_stops_a_complete_earth_crossing_at_entry() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let contact_radius = EARTH_EQUATORIAL_RADIUS_M
            + terrain_height_m(inertial_to_earth_fixed(
                Vec3d::Z,
                frame.earth_rotation_radians,
            ))
            + CELESTIAL_SURFACE_CLEARANCE_M;
        let previous_position = frame.earth.center_m + Vec3d::Z * (contact_radius + 10_000.0);
        let mut camera = SpaceflightState {
            position_m: frame.earth.center_m - Vec3d::Z * (contact_radius + 10_000.0),
            velocity_mps: Vec3d::Z * -1_000_000_000.0,
            ..SpaceflightState::default()
        };

        assert!(camera.constrain_to_celestial_frame(previous_position, frame));
        let relative = camera.position_m - frame.earth.center_m;
        assert!((relative.z - contact_radius).abs() < 1.0e-6);
        assert!(camera.velocity_mps.dot(Vec3d::Z) >= -1.0e-9);
    }

    #[test]
    fn swept_constraint_stops_complete_moon_and_sun_crossings_at_entry() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        for body in [frame.moon, frame.sun] {
            let contact_radius = body.radius_m + CELESTIAL_SURFACE_CLEARANCE_M;
            let previous_position = body.center_m + Vec3d::Z * (contact_radius + 10_000.0);
            let mut camera = SpaceflightState {
                position_m: body.center_m - Vec3d::Z * (contact_radius + 10_000.0),
                velocity_mps: Vec3d::Z * -1_000_000_000.0,
                ..SpaceflightState::default()
            };

            assert!(camera.constrain_to_celestial_frame(previous_position, frame));
            let relative = camera.position_m - body.center_m;
            assert!((relative.z - contact_radius).abs() < 1.0e-4);
            assert!(camera.velocity_mps.dot(Vec3d::Z) >= -1.0e-8);
        }
    }

    #[test]
    fn constraint_recovers_nonfinite_camera_state_deterministically() {
        let frame = CelestialFrame::at_seconds_since_j2000(0.0);
        let mut camera = SpaceflightState {
            position_m: Vec3d::new(f64::NAN, 0.0, 0.0),
            velocity_mps: Vec3d::new(0.0, f64::INFINITY, 0.0),
            ..SpaceflightState::default()
        };

        assert!(
            camera.constrain_to_celestial_frame(Vec3d::new(f64::NAN, f64::NAN, f64::NAN), frame,)
        );
        assert!(camera.position_m.is_finite());
        assert!(camera.velocity_mps.is_finite());
        assert_eq!(camera, SpaceflightState::default());
    }
}
