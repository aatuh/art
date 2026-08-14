//! Cube-sphere quadtree addressing for future multiscale Earth surface streaming.

use std::collections::BTreeSet;

use crate::planet::{EARTH_MEAN_RADIUS_M, Vec3d};

pub const MAX_STREAMING_LEVEL: u8 = 18;
pub const MAX_WORKING_SET_RADIUS_TILES: u8 = 4;
const MIN_TARGET_TILE_EDGE_M: f64 = 250.0;
const ALTITUDE_TO_TILE_EDGE_RATIO: f64 = 0.5;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CubeFace {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl CubeFace {
    pub const ALL: [Self; 6] = [
        Self::PositiveX,
        Self::NegativeX,
        Self::PositiveY,
        Self::NegativeY,
        Self::PositiveZ,
        Self::NegativeZ,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::PositiveX => "px",
            Self::NegativeX => "nx",
            Self::PositiveY => "py",
            Self::NegativeY => "ny",
            Self::PositiveZ => "pz",
            Self::NegativeZ => "nz",
        }
    }

    /// Maps face-local coordinates in [-1, 1]² onto a normalized cube-sphere direction.
    pub fn direction(self, u: f64, v: f64) -> Vec3d {
        self.direction_unbounded(u.clamp(-1.0, 1.0), v.clamp(-1.0, 1.0))
    }

    fn direction_unbounded(self, u: f64, v: f64) -> Vec3d {
        let cube = match self {
            Self::PositiveX => Vec3d::new(1.0, v, -u),
            Self::NegativeX => Vec3d::new(-1.0, v, u),
            Self::PositiveY => Vec3d::new(u, 1.0, -v),
            Self::NegativeY => Vec3d::new(u, -1.0, v),
            Self::PositiveZ => Vec3d::new(u, v, 1.0),
            Self::NegativeZ => Vec3d::new(-u, v, -1.0),
        };
        cube.normalized()
    }
}

/// Returns the dominant cube face and face-local coordinates for a direction.
pub fn face_uv(direction: Vec3d) -> (CubeFace, f64, f64) {
    let direction = direction.normalized();
    if direction == Vec3d::ZERO {
        return (CubeFace::PositiveZ, 0.0, 0.0);
    }

    let ax = direction.x.abs();
    let ay = direction.y.abs();
    let az = direction.z.abs();
    if ax >= ay && ax >= az {
        if direction.x >= 0.0 {
            (CubeFace::PositiveX, -direction.z / ax, direction.y / ax)
        } else {
            (CubeFace::NegativeX, direction.z / ax, direction.y / ax)
        }
    } else if ay >= az {
        if direction.y >= 0.0 {
            (CubeFace::PositiveY, direction.x / ay, -direction.z / ay)
        } else {
            (CubeFace::NegativeY, direction.x / ay, direction.z / ay)
        }
    } else if direction.z >= 0.0 {
        (CubeFace::PositiveZ, direction.x / az, direction.y / az)
    } else {
        (CubeFace::NegativeZ, -direction.x / az, direction.y / az)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileId {
    pub face: CubeFace,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn new(face: CubeFace, level: u8, x: u32, y: u32) -> Option<Self> {
        if level > MAX_STREAMING_LEVEL {
            return None;
        }
        let side = 1_u32 << level;
        (x < side && y < side).then_some(Self { face, level, x, y })
    }

    pub fn from_direction(direction: Vec3d, level: u8) -> Option<Self> {
        if level > MAX_STREAMING_LEVEL {
            return None;
        }
        let (face, u, v) = face_uv(direction);
        let side = 1_u32 << level;
        let to_index = |coordinate: f64| {
            let normalized =
                ((coordinate.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0 - f64::EPSILON);
            (normalized * f64::from(side)).floor() as u32
        };
        Self::new(face, level, to_index(u), to_index(v))
    }

    pub fn parent(self) -> Option<Self> {
        (self.level > 0).then_some(Self {
            face: self.face,
            level: self.level.saturating_sub(1),
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn children(self) -> Option<[Self; 4]> {
        let level = self.level.checked_add(1)?;
        if level > MAX_STREAMING_LEVEL {
            return None;
        }
        let x = self.x * 2;
        let y = self.y * 2;
        Some([
            Self {
                face: self.face,
                level,
                x,
                y,
            },
            Self {
                face: self.face,
                level,
                x: x + 1,
                y,
            },
            Self {
                face: self.face,
                level,
                x,
                y: y + 1,
            },
            Self {
                face: self.face,
                level,
                x: x + 1,
                y: y + 1,
            },
        ])
    }

    pub fn center_direction(self) -> Vec3d {
        let side = f64::from(1_u32 << self.level);
        let u = ((f64::from(self.x) + 0.5) / side) * 2.0 - 1.0;
        let v = ((f64::from(self.y) + 0.5) / side) * 2.0 - 1.0;
        self.face.direction(u, v)
    }

    /// Moves by tile-sized steps and remaps through neighboring cube faces when needed.
    pub fn offset(self, x_steps: i32, y_steps: i32) -> Option<Self> {
        let side = f64::from(1_u32 << self.level);
        let tile_span = 2.0 / side;
        let u = ((f64::from(self.x) + 0.5) / side) * 2.0 - 1.0
            + f64::from(x_steps) * tile_span;
        let v = ((f64::from(self.y) + 0.5) / side) * 2.0 - 1.0
            + f64::from(y_steps) * tile_span;
        Self::from_direction(self.face.direction_unbounded(u, v), self.level)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TileLayer {
    Surface,
    Elevation,
}

impl TileLayer {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Surface => "surface",
            Self::Elevation => "elevation",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Surface => "webp",
            Self::Elevation => "png",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileAssetRequest {
    pub layer: TileLayer,
    pub tile: TileId,
}

impl TileAssetRequest {
    pub fn path(self) -> String {
        format!(
            "./assets/earth/tiles/{}/{}/{}/{}/{}.{}",
            self.layer.slug(),
            self.tile.face.slug(),
            self.tile.level,
            self.tile.x,
            self.tile.y,
            self.layer.extension()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileStreamingPlan {
    pub level: u8,
    pub primary: Vec<TileId>,
    pub fallback: Vec<TileId>,
}

impl TileStreamingPlan {
    /// Produces coarse-to-fine requests so existing parent coverage can render while
    /// fine tiles are still decoding.
    pub fn requests(&self, layer: TileLayer) -> Vec<TileAssetRequest> {
        self.fallback
            .iter()
            .chain(self.primary.iter())
            .copied()
            .map(|tile| TileAssetRequest { layer, tile })
            .collect()
    }
}

/// Approximate surface edge length represented by one cube-face tile.
pub fn tile_edge_m(level: u8) -> f64 {
    let level = level.min(MAX_STREAMING_LEVEL);
    std::f64::consts::FRAC_PI_2 * EARTH_MEAN_RADIUS_M / 2_f64.powi(i32::from(level))
}

/// Chooses a quadtree level from camera clearance for future regional tile streaming.
pub fn surface_tile_level_for_altitude(altitude_m: f64) -> u8 {
    if !altitude_m.is_finite() {
        return 0;
    }
    let target_edge_m =
        (altitude_m.max(0.0) * ALTITUDE_TO_TILE_EDGE_RATIO).max(MIN_TARGET_TILE_EDGE_M);
    for level in 0..=MAX_STREAMING_LEVEL {
        if tile_edge_m(level) <= target_edge_m {
            return level;
        }
    }
    MAX_STREAMING_LEVEL
}

/// Returns a deterministic square working set around the surface point beneath the camera.
///
/// Offsets crossing a cube-face edge are remapped onto the adjacent face. The radius is
/// bounded so a malformed caller cannot request an unbounded number of tiles per frame.
pub fn surface_tile_working_set(
    surface_direction: Vec3d,
    altitude_m: f64,
    radius_tiles: u8,
) -> Vec<TileId> {
    let level = surface_tile_level_for_altitude(altitude_m);
    let Some(center) = TileId::from_direction(surface_direction, level) else {
        return Vec::new();
    };
    let radius = i32::from(radius_tiles.min(MAX_WORKING_SET_RADIUS_TILES));
    let mut tiles = BTreeSet::new();
    for y_step in -radius..=radius {
        for x_step in -radius..=radius {
            if let Some(tile) = center.offset(x_step, y_step) {
                tiles.insert(tile);
            }
        }
    }
    tiles.into_iter().collect()
}

/// Builds the fine working set plus every unique parent tile needed as coarse fallback.
pub fn surface_tile_streaming_plan(
    surface_direction: Vec3d,
    altitude_m: f64,
    radius_tiles: u8,
) -> TileStreamingPlan {
    let primary = surface_tile_working_set(surface_direction, altitude_m, radius_tiles);
    let level = primary.first().map_or(0, |tile| tile.level);
    let mut fallback_set = BTreeSet::new();
    for tile in &primary {
        let mut ancestor = *tile;
        while let Some(parent) = ancestor.parent() {
            fallback_set.insert(parent);
            ancestor = parent;
        }
    }
    let mut fallback = fallback_set.into_iter().collect::<Vec<_>>();
    fallback.sort_by_key(|tile| (tile.level, tile.face, tile.x, tile.y));
    TileStreamingPlan {
        level,
        primary,
        fallback,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        CubeFace, MAX_STREAMING_LEVEL, TileAssetRequest, TileId, TileLayer, face_uv,
        surface_tile_level_for_altitude, surface_tile_streaming_plan, surface_tile_working_set,
        tile_edge_m,
    };
    use crate::planet::Vec3d;

    #[test]
    fn cube_face_mapping_round_trips_directions() {
        for face in CubeFace::ALL {
            for (u, v) in [(-0.75, -0.4), (0.0, 0.0), (0.61, -0.22), (0.4, 0.8)] {
                let direction = face.direction(u, v);
                let (mapped_face, mapped_u, mapped_v) = face_uv(direction);
                assert_eq!(mapped_face, face);
                assert!((mapped_u - u).abs() < 1.0e-12);
                assert!((mapped_v - v).abs() < 1.0e-12);
            }
        }
    }

    #[test]
    fn cardinal_directions_have_stable_faces() {
        let cases = [
            (Vec3d::new(1.0, 0.0, 0.0), CubeFace::PositiveX),
            (Vec3d::new(-1.0, 0.0, 0.0), CubeFace::NegativeX),
            (Vec3d::new(0.0, 1.0, 0.0), CubeFace::PositiveY),
            (Vec3d::new(0.0, -1.0, 0.0), CubeFace::NegativeY),
            (Vec3d::new(0.0, 0.0, 1.0), CubeFace::PositiveZ),
            (Vec3d::new(0.0, 0.0, -1.0), CubeFace::NegativeZ),
        ];
        for (direction, expected_face) in cases {
            assert_eq!(face_uv(direction).0, expected_face);
        }
    }

    #[test]
    fn tile_hierarchy_round_trips_through_parent() {
        let tile = TileId::new(CubeFace::PositiveZ, 5, 11, 19).expect("valid tile");
        let children = tile.children().expect("children below maximum level");
        for child in children {
            assert_eq!(child.parent(), Some(tile));
        }
    }

    #[test]
    fn direction_lookup_selects_the_tile_containing_its_center() {
        let tile = TileId::new(CubeFace::NegativeX, 8, 44, 191).expect("valid tile");
        assert_eq!(
            TileId::from_direction(tile.center_direction(), 8),
            Some(tile)
        );
    }

    #[test]
    fn tile_offsets_cross_cube_faces_without_changing_level() {
        let edge = TileId::new(CubeFace::PositiveZ, 4, 0, 7).expect("valid edge tile");
        let neighbor = edge.offset(-1, 0).expect("cross-face neighbor");
        assert_ne!(neighbor.face, edge.face);
        assert_eq!(neighbor.level, edge.level);
    }

    #[test]
    fn tile_edges_halve_at_each_level() {
        for level in 0..MAX_STREAMING_LEVEL {
            let ratio = tile_edge_m(level) / tile_edge_m(level + 1);
            assert!((ratio - 2.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn altitude_selects_finer_tiles_toward_the_surface() {
        let distant = surface_tile_level_for_altitude(20_000_000.0);
        let low_orbit = surface_tile_level_for_altitude(400_000.0);
        let aircraft = surface_tile_level_for_altitude(10_000.0);
        let ground = surface_tile_level_for_altitude(0.0);
        assert!(distant < low_orbit);
        assert!(low_orbit < aircraft);
        assert!(aircraft < ground);
        assert_eq!(surface_tile_level_for_altitude(f64::NAN), 0);
    }

    #[test]
    fn working_set_is_deterministic_and_unique_on_one_face() {
        let direction = CubeFace::PositiveZ.direction(0.0, 0.0);
        let first = surface_tile_working_set(direction, 400_000.0, 1);
        let second = surface_tile_working_set(direction, 400_000.0, 1);
        assert_eq!(first, second);
        assert_eq!(first.len(), 9);
        assert_eq!(first.iter().copied().collect::<BTreeSet<_>>().len(), 9);
        assert!(first.iter().all(|tile| tile.face == CubeFace::PositiveZ));
    }

    #[test]
    fn working_set_crosses_faces_near_cube_edges() {
        let direction = CubeFace::PositiveZ.direction(-0.999, 0.0);
        let tiles = surface_tile_working_set(direction, 400_000.0, 1);
        let faces = tiles.iter().map(|tile| tile.face).collect::<BTreeSet<_>>();
        assert!(faces.len() > 1);
        assert!(tiles.iter().all(|tile| tile.level == tiles[0].level));
    }

    #[test]
    fn working_set_radius_is_bounded() {
        let direction = CubeFace::PositiveZ.direction(0.0, 0.0);
        let bounded = surface_tile_working_set(direction, 400_000.0, 255);
        assert!(bounded.len() <= 81);
    }

    #[test]
    fn streaming_plan_adds_unique_coarse_parent_coverage() {
        let direction = CubeFace::PositiveZ.direction(0.2, -0.1);
        let plan = surface_tile_streaming_plan(direction, 400_000.0, 1);
        assert_eq!(plan.primary.len(), 9);
        assert!(plan.level > 0);
        assert!(plan.fallback.iter().all(|tile| tile.level < plan.level));
        assert_eq!(
            plan.fallback.iter().copied().collect::<BTreeSet<_>>().len(),
            plan.fallback.len()
        );
        assert!(plan.fallback.windows(2).all(|pair| pair[0].level <= pair[1].level));
    }

    #[test]
    fn tile_asset_paths_are_stable_and_layer_specific() {
        let tile = TileId::new(CubeFace::PositiveZ, 6, 17, 42).expect("valid tile");
        assert_eq!(
            TileAssetRequest {
                layer: TileLayer::Surface,
                tile,
            }
            .path(),
            "./assets/earth/tiles/surface/pz/6/17/42.webp"
        );
        assert_eq!(
            TileAssetRequest {
                layer: TileLayer::Elevation,
                tile,
            }
            .path(),
            "./assets/earth/tiles/elevation/pz/6/17/42.png"
        );
    }

    #[test]
    fn request_plan_orders_fallback_before_primary_tiles() {
        let direction = CubeFace::PositiveZ.direction(0.0, 0.0);
        let plan = surface_tile_streaming_plan(direction, 400_000.0, 0);
        let requests = plan.requests(TileLayer::Surface);
        let first_primary = requests
            .iter()
            .position(|request| request.tile.level == plan.level)
            .expect("primary request");
        assert!(requests[..first_primary]
            .iter()
            .all(|request| request.tile.level < plan.level));
        assert!(requests[first_primary..]
            .iter()
            .all(|request| request.tile.level == plan.level));
    }
}
