//! Conservative screen-space bounds for the expensive celestial composite pass.

use crate::{exhibition_camera::CameraBasis, planet::Vec3d};

pub const PLANET_TANGENT_HALF_VERTICAL_FOV: f64 = 0.767_326_987_978_960_4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScissorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ScissorRect {
    fn right(self) -> i32 {
        self.x + self.width
    }

    fn top(self) -> i32 {
        self.y + self.height
    }

    fn overlaps(self, other: Self) -> bool {
        self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.top()
            && other.y <= self.top()
    }

    fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let top = self.top().max(other.top());
        Self {
            x,
            y,
            width: right - x,
            height: top - y,
        }
    }
}

/// Projects a camera-relative sphere to an exact tangent rectangle in each screen axis.
///
/// The result is padded by two framebuffer pixels so the analytic atmosphere and anti-aliased
/// limb cannot be clipped by rounding. A sphere crossing the camera plane conservatively occupies
/// the full viewport.
pub fn sphere_scissor_rect(
    center_m: Vec3d,
    radius_m: f64,
    basis: CameraBasis,
    framebuffer_width: u32,
    framebuffer_height: u32,
) -> Option<ScissorRect> {
    let width = framebuffer_width.min(i32::MAX as u32) as i32;
    let height = framebuffer_height.min(i32::MAX as u32) as i32;
    if width <= 0
        || height <= 0
        || !center_m.is_finite()
        || !radius_m.is_finite()
        || radius_m <= 0.0
    {
        return None;
    }

    let camera_x = center_m.dot(basis.right);
    let camera_y = center_m.dot(basis.up);
    let camera_z = center_m.dot(basis.forward);
    if !camera_x.is_finite() || !camera_y.is_finite() || !camera_z.is_finite() {
        return None;
    }
    if center_m.length_squared() <= radius_m * radius_m || camera_z <= radius_m {
        return (camera_z + radius_m > 0.0).then_some(ScissorRect {
            x: 0,
            y: 0,
            width,
            height,
        });
    }

    let (minimum_x_slope, maximum_x_slope) = tangent_slopes(camera_x, camera_z, radius_m)?;
    let (minimum_y_slope, maximum_y_slope) = tangent_slopes(camera_y, camera_z, radius_m)?;
    let aspect = f64::from(width) / f64::from(height);
    let horizontal_tangent = PLANET_TANGENT_HALF_VERTICAL_FOV * aspect;
    ndc_bounds_to_scissor(
        [
            minimum_x_slope / horizontal_tangent,
            minimum_y_slope / PLANET_TANGENT_HALF_VERTICAL_FOV,
        ],
        [
            maximum_x_slope / horizontal_tangent,
            maximum_y_slope / PLANET_TANGENT_HALF_VERTICAL_FOV,
        ],
        width,
        height,
    )
}

pub fn merge_overlapping_rects(
    rectangles: impl IntoIterator<Item = ScissorRect>,
) -> Vec<ScissorRect> {
    let mut merged: Vec<ScissorRect> = Vec::new();
    for mut candidate in rectangles {
        let mut index = 0;
        while index < merged.len() {
            if candidate.overlaps(merged[index]) {
                candidate = candidate.union(merged.swap_remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        merged.push(candidate);
    }
    merged
}

fn tangent_slopes(axis: f64, forward: f64, radius: f64) -> Option<(f64, f64)> {
    let denominator = forward * forward - radius * radius;
    let tangent_distance_squared = axis * axis + forward * forward - radius * radius;
    if denominator <= 0.0 || tangent_distance_squared < 0.0 {
        return None;
    }
    let offset = radius * tangent_distance_squared.sqrt();
    Some((
        (axis * forward - offset) / denominator,
        (axis * forward + offset) / denominator,
    ))
}

fn ndc_bounds_to_scissor(
    minimum: [f64; 2],
    maximum: [f64; 2],
    width: i32,
    height: i32,
) -> Option<ScissorRect> {
    if maximum[0] < -1.0 || minimum[0] > 1.0 || maximum[1] < -1.0 || minimum[1] > 1.0 {
        return None;
    }
    const PADDING: i32 = 2;
    let x0 = (((minimum[0].clamp(-1.0, 1.0) * 0.5 + 0.5) * f64::from(width)).floor() as i32
        - PADDING)
        .max(0);
    let y0 = (((minimum[1].clamp(-1.0, 1.0) * 0.5 + 0.5) * f64::from(height)).floor() as i32
        - PADDING)
        .max(0);
    let x1 = (((maximum[0].clamp(-1.0, 1.0) * 0.5 + 0.5) * f64::from(width)).ceil() as i32
        + PADDING)
        .min(width);
    let y1 = (((maximum[1].clamp(-1.0, 1.0) * 0.5 + 0.5) * f64::from(height)).ceil() as i32
        + PADDING)
        .min(height);
    (x1 > x0 && y1 > y0).then_some(ScissorRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{exhibition_camera::SpaceflightState, planet::EARTH_EQUATORIAL_RADIUS_M};

    #[test]
    fn default_earth_view_is_tightly_bounded_around_screen_centre() {
        let camera = SpaceflightState::default();
        let rectangle = sphere_scissor_rect(
            -camera.position_m,
            EARTH_EQUATORIAL_RADIUS_M + 100_000.0,
            camera.camera_basis(),
            1440,
            900,
        )
        .expect("Earth is visible");

        assert!(rectangle.width > 250 && rectangle.width < 500);
        assert!(rectangle.height > 250 && rectangle.height < 500);
        assert!((rectangle.x + rectangle.width / 2 - 720).abs() <= 2);
        assert!((rectangle.y + rectangle.height / 2 - 450).abs() <= 2);
    }

    #[test]
    fn hidden_and_invalid_spheres_do_not_schedule_gpu_work() {
        let basis = SpaceflightState::default().camera_basis();
        assert_eq!(
            sphere_scissor_rect(Vec3d::new(0.0, 0.0, 4.0), 1.0, basis, 800, 600),
            None
        );
        assert_eq!(
            sphere_scissor_rect(Vec3d::new(f64::NAN, 0.0, -4.0), 1.0, basis, 800, 600),
            None
        );
        assert_eq!(
            sphere_scissor_rect(Vec3d::new(0.0, 0.0, -4.0), -1.0, basis, 800, 600),
            None
        );
    }

    #[test]
    fn camera_inside_sphere_uses_the_full_framebuffer() {
        let basis = SpaceflightState::default().camera_basis();
        assert_eq!(
            sphere_scissor_rect(Vec3d::ZERO, 10.0, basis, 800, 600),
            Some(ScissorRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            })
        );
    }

    #[test]
    fn overlapping_rectangles_are_merged_transitively() {
        let merged = merge_overlapping_rects([
            ScissorRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            ScissorRect {
                x: 9,
                y: 9,
                width: 10,
                height: 10,
            },
            ScissorRect {
                x: 18,
                y: 18,
                width: 4,
                height: 4,
            },
        ]);
        assert_eq!(
            merged,
            vec![ScissorRect {
                x: 0,
                y: 0,
                width: 22,
                height: 22,
            }]
        );
    }
}
