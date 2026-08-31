const ROOM_TRIANGLE_VERTICES: usize = 36;
const CUBE_TRIANGLE_VERTICES: usize = 36;

/// Renderer-neutral authored vertex. Browser adapters choose their own GPU packing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

/// Authored scene shared by rendering, spawn placement, and collision rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomScene {
    pub half_extent: f32,
    pub ceiling_height: f32,
    pub cube_half_extent: f32,
    pub cube_height: f32,
    pub spawn_x: f32,
    pub spawn_z: f32,
}

pub const BLACK_CUBE_SCENE: RoomScene = RoomScene {
    half_extent: 8.0,
    ceiling_height: 4.0,
    cube_half_extent: 1.0,
    cube_height: 2.0,
    spawn_x: 0.0,
    spawn_z: 5.8,
};

impl RoomScene {
    /// Returns whether all authored dimensions form a usable enclosed room.
    pub fn is_valid(&self) -> bool {
        [
            self.half_extent,
            self.ceiling_height,
            self.cube_half_extent,
            self.cube_height,
            self.spawn_x,
            self.spawn_z,
        ]
        .into_iter()
        .all(f32::is_finite)
            && self.half_extent > 0.0
            && self.ceiling_height > 0.0
            && self.cube_half_extent > 0.0
            && self.cube_height > 0.0
            && self.cube_half_extent < self.half_extent
            && self.cube_height <= self.ceiling_height
    }

    /// Produces renderer-neutral triangles for the complete installation.
    pub fn vertices(&self) -> Vec<RoomVertex> {
        let mut vertices = Vec::with_capacity(ROOM_TRIANGLE_VERTICES + CUBE_TRIANGLE_VERTICES);
        let extent = self.half_extent;
        let ceiling = self.ceiling_height;
        let wall = [0.94, 0.94, 0.92];
        add_face(
            &mut vertices,
            [
                [-extent, 0.0, -extent],
                [extent, 0.0, -extent],
                [extent, 0.0, extent],
                [-extent, 0.0, extent],
            ],
            [0.82, 0.82, 0.80],
        );
        add_face(
            &mut vertices,
            [
                [-extent, ceiling, -extent],
                [-extent, ceiling, extent],
                [extent, ceiling, extent],
                [extent, ceiling, -extent],
            ],
            [0.99, 0.99, 0.98],
        );
        add_face(
            &mut vertices,
            [
                [-extent, 0.0, -extent],
                [-extent, ceiling, -extent],
                [extent, ceiling, -extent],
                [extent, 0.0, -extent],
            ],
            wall,
        );
        add_face(
            &mut vertices,
            [
                [extent, 0.0, extent],
                [extent, ceiling, extent],
                [-extent, ceiling, extent],
                [-extent, 0.0, extent],
            ],
            [0.90, 0.90, 0.88],
        );
        add_face(
            &mut vertices,
            [
                [-extent, 0.0, extent],
                [-extent, ceiling, extent],
                [-extent, ceiling, -extent],
                [-extent, 0.0, -extent],
            ],
            [0.87, 0.87, 0.85],
        );
        add_face(
            &mut vertices,
            [
                [extent, 0.0, -extent],
                [extent, ceiling, -extent],
                [extent, ceiling, extent],
                [extent, 0.0, extent],
            ],
            [0.91, 0.91, 0.90],
        );
        add_cube(&mut vertices, self.cube_half_extent, self.cube_height);
        vertices
    }

    /// True when a circular player footprint overlaps the cube's floor projection.
    pub fn intersects_cube(&self, x: f32, z: f32, radius: f32) -> bool {
        if !self.is_valid()
            || !x.is_finite()
            || !z.is_finite()
            || !radius.is_finite()
            || radius < 0.0
        {
            return false;
        }

        x > -self.cube_half_extent - radius
            && x < self.cube_half_extent + radius
            && z > -self.cube_half_extent - radius
            && z < self.cube_half_extent + radius
    }

    /// True when the complete player footprint remains inside the room walls.
    pub fn contains_player(&self, x: f32, z: f32, radius: f32) -> bool {
        if !self.is_valid()
            || !x.is_finite()
            || !z.is_finite()
            || !radius.is_finite()
            || !(0.0..self.half_extent).contains(&radius)
        {
            return false;
        }

        let limit = self.half_extent - radius;
        x.abs() <= limit && z.abs() <= limit
    }

    /// Validates that the authored spawn is in bounds and does not overlap the cube.
    pub fn spawn_is_clear(&self, player_radius: f32) -> bool {
        self.contains_player(self.spawn_x, self.spawn_z, player_radius)
            && !self.intersects_cube(self.spawn_x, self.spawn_z, player_radius)
    }
}

fn add_cube(vertices: &mut Vec<RoomVertex>, half_extent: f32, height: f32) {
    let low = -half_extent;
    let high = half_extent;
    add_face(
        vertices,
        [
            [low, 0.0, high],
            [high, 0.0, high],
            [high, height, high],
            [low, height, high],
        ],
        [0.01, 0.01, 0.01],
    );
    add_face(
        vertices,
        [
            [high, 0.0, low],
            [low, 0.0, low],
            [low, height, low],
            [high, height, low],
        ],
        [0.04, 0.04, 0.04],
    );
    add_face(
        vertices,
        [
            [high, 0.0, high],
            [high, 0.0, low],
            [high, height, low],
            [high, height, high],
        ],
        [0.08, 0.08, 0.08],
    );
    add_face(
        vertices,
        [
            [low, 0.0, low],
            [low, 0.0, high],
            [low, height, high],
            [low, height, low],
        ],
        [0.025, 0.025, 0.025],
    );
    add_face(
        vertices,
        [
            [low, height, high],
            [high, height, high],
            [high, height, low],
            [low, height, low],
        ],
        [0.12, 0.12, 0.12],
    );
    add_face(
        vertices,
        [
            [low, 0.0, low],
            [high, 0.0, low],
            [high, 0.0, high],
            [low, 0.0, high],
        ],
        [0.0, 0.0, 0.0],
    );
}

fn add_face(vertices: &mut Vec<RoomVertex>, corners: [[f32; 3]; 4], color: [f32; 3]) {
    for index in [0, 1, 2, 0, 2, 3] {
        vertices.push(RoomVertex {
            position: corners[index],
            color,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::BLACK_CUBE_SCENE;
    use crate::PLAYER_RADIUS;

    #[test]
    fn authored_geometry_matches_room_cube_and_collider_dimensions() {
        let scene = BLACK_CUBE_SCENE;
        let vertices = scene.vertices();

        assert!(scene.is_valid());
        assert_eq!(vertices.len(), 72);
        assert!(vertices.iter().all(|vertex| {
            vertex
                .position
                .iter()
                .chain(&vertex.color)
                .all(|value| value.is_finite())
        }));
        assert!(scene.intersects_cube(0.0, 0.0, PLAYER_RADIUS));
        assert!(scene.intersects_cube(1.2, 0.0, PLAYER_RADIUS));
        assert!(!scene.intersects_cube(1.36, 0.0, PLAYER_RADIUS));
    }

    #[test]
    fn authored_spawn_is_clear_and_inside_room_walls() {
        let scene = BLACK_CUBE_SCENE;

        assert!(scene.spawn_is_clear(PLAYER_RADIUS));
        assert!(scene.contains_player(scene.spawn_x, scene.spawn_z, PLAYER_RADIUS));
        assert!(!scene.intersects_cube(scene.spawn_x, scene.spawn_z, PLAYER_RADIUS));
        assert!(!scene.contains_player(scene.half_extent, 0.0, PLAYER_RADIUS));
    }
}
