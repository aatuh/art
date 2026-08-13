//! Virtual camera navigation for the astronomical artwork.
//!
//! This controls only a rendered viewpoint. It is not a spacecraft, vehicle controller,
//! guidance system, or orbital-dynamics model.

use crate::planet::{ASTRONOMICAL_UNIT_M, EARTH_EQUATORIAL_RADIUS_M, EarthEllipsoid, Vec3d};

pub const MIN_CAMERA_SPEED_MPS: f64 = 0.25;
pub const MAX_CAMERA_SPEED_MPS: f64 = ASTRONOMICAL_UNIT_M / 10.0;
pub const DEFAULT_CAMERA_SPEED_MPS: f64 = EARTH_EQUATORIAL_RADIUS_M * 3.0 * 0.04;
pub const MIN_SPEED_MULTIPLIER: f64 = 0.01;
pub const MAX_SPEED_MULTIPLIER: f64 = 100.0;
pub const DEFAULT_SPEED_MULTIPLIER: f64 = 1.0;
const CLEARANCE_SPEED_RATIO: f64 = 0.04;
const FAST_TRAVEL_MULTIPLIER: f64 = 12.0;
const SPEED_STEP_RATIO: f64 = 1.778_279_410_038_922_8;
const MAX_FRAME_SECONDS: f64 = 0.1;
const MAX_CAMERA_DISTANCE_M: f64 = ASTRONOMICAL_UNIT_M * 100.0;

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

    pub fn radial_altitude_above_earth_m(self) -> f64 {
        EarthEllipsoid::default().radial_altitude_m(self.position_m)
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
}
