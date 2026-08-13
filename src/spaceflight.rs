//! Deterministic noclip navigation for astronomical-scale installations.

use crate::planet::{ASTRONOMICAL_UNIT_M, EARTH_EQUATORIAL_RADIUS_M, EarthEllipsoid, Vec3d};

pub const MIN_CRUISE_SPEED_MPS: f64 = 0.25;
pub const MAX_CRUISE_SPEED_MPS: f64 = 74_948_114.5;
pub const DEFAULT_CRUISE_SPEED_MPS: f64 = 25_000.0;
const SPEED_NOTCH_RATIO: f64 = 1.778_279_410_038_922_8;
const MAX_FRAME_SECONDS: f64 = 0.1;
const MAX_DISTANCE_FROM_EARTH_M: f64 = ASTRONOMICAL_UNIT_M * 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpaceflightInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub ascend: bool,
    pub descend: bool,
    pub fast: bool,
    pub brake: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpaceflightState {
    pub position_m: Vec3d,
    pub velocity_mps: Vec3d,
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub cruise_speed_mps: f64,
}

impl Default for SpaceflightState {
    fn default() -> Self {
        Self {
            position_m: Vec3d::new(0.0, 0.0, EARTH_EQUATORIAL_RADIUS_M * 4.0),
            velocity_mps: Vec3d::ZERO,
            yaw_radians: 0.0,
            pitch_radians: 0.0,
            cruise_speed_mps: DEFAULT_CRUISE_SPEED_MPS,
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

    pub fn adjust_cruise_speed(&mut self, notches: i32) {
        let multiplier = SPEED_NOTCH_RATIO.powi(notches.clamp(-64, 64));
        self.cruise_speed_mps =
            (self.cruise_speed_mps * multiplier).clamp(MIN_CRUISE_SPEED_MPS, MAX_CRUISE_SPEED_MPS);
    }

    pub fn focus(&mut self, target_m: Vec3d) {
        let direction = (target_m - self.position_m).normalized();
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

    pub fn focus_earth(&mut self) {
        self.focus(Vec3d::ZERO);
    }

    pub fn reset_orbital_view(&mut self) {
        *self = Self::default();
    }

    pub fn radial_altitude_above_earth_m(self) -> f64 {
        EarthEllipsoid::default().radial_altitude_m(self.position_m)
    }

    pub fn tick(&mut self, frame_seconds: f64, input: SpaceflightInput) {
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
        let speed = self.cruise_speed_mps * if input.fast { 24.0 } else { 1.0 };
        let target = if input.brake {
            Vec3d::ZERO
        } else {
            requested * speed
        };
        let response_seconds: f64 = if input.brake { 0.045 } else { 0.14 };
        let blend = 1.0 - (-dt / response_seconds).exp();
        self.velocity_mps += (target - self.velocity_mps) * blend;
        self.position_m += self.velocity_mps * dt;

        if !self.position_m.is_finite()
            || !self.velocity_mps.is_finite()
            || self.position_m.length() > MAX_DISTANCE_FROM_EARTH_M
        {
            self.reset_orbital_view();
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
    fn default_orbital_view_faces_earth() {
        let state = SpaceflightState::default();
        assert!(
            state
                .camera_basis()
                .forward
                .dot((-state.position_m).normalized())
                > 0.999_999
        );
        assert!(state.radial_altitude_above_earth_m() > EARTH_EQUATORIAL_RADIUS_M * 2.9);
    }

    #[test]
    fn diagonal_input_is_normalized_and_unconstrained() {
        let mut state = SpaceflightState::default();
        let initial = state.position_m;
        for _ in 0..30 {
            state.tick(
                1.0 / 60.0,
                SpaceflightInput {
                    forward: true,
                    right: true,
                    ascend: true,
                    ..SpaceflightInput::default()
                },
            );
        }
        let movement = state.position_m - initial;
        assert!(movement.x > 0.0 && movement.y > 0.0 && movement.z < 0.0);
        assert!(state.velocity_mps.length() <= state.cruise_speed_mps * 1.000_001);
    }

    #[test]
    fn speed_notches_cover_precise_and_interplanetary_navigation() {
        let mut state = SpaceflightState::default();
        state.adjust_cruise_speed(-64);
        assert_eq!(state.cruise_speed_mps, MIN_CRUISE_SPEED_MPS);
        state.adjust_cruise_speed(64);
        assert_eq!(state.cruise_speed_mps, MAX_CRUISE_SPEED_MPS);
    }

    #[test]
    fn focus_points_camera_at_target() {
        let mut state = SpaceflightState::default();
        let target = Vec3d::new(10_000_000.0, 7_000_000.0, -3_000_000.0);
        state.focus(target);
        assert!(
            state
                .camera_basis()
                .forward
                .dot((target - state.position_m).normalized())
                > 0.999_999
        );
    }
}
