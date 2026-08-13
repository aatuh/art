//! Small, deterministic matrix and vector operations used by render adapters.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub(crate) fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    pub(crate) fn translation_scale(position: [f32; 3], scale: f32) -> Self {
        Self([
            scale,
            0.0,
            0.0,
            0.0,
            0.0,
            scale,
            0.0,
            0.0,
            0.0,
            0.0,
            scale,
            0.0,
            position[0],
            position[1],
            position[2],
            1.0,
        ])
    }

    pub(crate) fn perspective(fov_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let focal_length = 1.0 / (fov_radians / 2.0).tan();
        Self([
            focal_length / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            focal_length,
            0.0,
            0.0,
            0.0,
            0.0,
            (far + near) / (near - far),
            -1.0,
            0.0,
            0.0,
            (2.0 * far * near) / (near - far),
            0.0,
        ])
    }

    pub(crate) fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> Self {
        let forward = normalize(subtract(center, eye));
        let side = normalize(cross(forward, up));
        let actual_up = cross(side, forward);
        Self([
            side[0],
            actual_up[0],
            -forward[0],
            0.0,
            side[1],
            actual_up[1],
            -forward[1],
            0.0,
            side[2],
            actual_up[2],
            -forward[2],
            0.0,
            -dot(side, eye),
            -dot(actual_up, eye),
            dot(forward, eye),
            1.0,
        ])
    }

    pub(crate) fn multiply(self, other: Self) -> Self {
        let mut result = [0.0; 16];
        for column in 0..4 {
            for row in 0..4 {
                result[column * 4 + row] = (0..4)
                    .map(|index| self.0[index * 4 + row] * other.0[column * 4 + index])
                    .sum();
            }
        }
        Self(result)
    }
}

pub(crate) fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

pub(crate) fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length = dot(vector, vector).sqrt();
    if length <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [vector[0] / length, vector[1] / length, vector[2] / length]
    }
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::{Mat4, cross, normalize};

    #[test]
    fn vector_operations_preserve_expected_orientation_and_length() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(normalize([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
        assert_eq!(normalize([3.0, 0.0, 4.0]), [0.6, 0.0, 0.8]);
    }

    #[test]
    fn identity_does_not_change_a_matrix() {
        let transform = Mat4::translation_scale([2.0, 3.0, 4.0], 1.5);
        assert_eq!(Mat4::identity().multiply(transform), transform);
    }

    #[test]
    fn camera_matrices_are_finite_for_a_normal_gallery_view() {
        let view = Mat4::look_at([0.0, 1.7, 5.8], [0.0, 1.7, 0.0], [0.0, 1.0, 0.0]);
        let projection = Mat4::perspective(72.0_f32.to_radians(), 16.0 / 9.0, 0.03, 80.0);

        assert!(view.0.into_iter().all(f32::is_finite));
        assert!(projection.0.into_iter().all(f32::is_finite));
    }
}
