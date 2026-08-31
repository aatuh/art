//! Pure, browser-independent domain model for **A World in Light**.
//!
//! Rendering adapters may consume the deterministic physical model and generated shader text,
//! but this crate does not access Web APIs or own browser lifecycle state.

pub mod celestial_bounds;
pub mod descriptor;
pub mod earth_coordinates;
pub mod earth_terrain;
pub mod exhibition_camera;
pub mod navigation_display;
pub mod planet;
pub mod planet_shader;
pub mod planet_tiles;
pub mod render_quality;
pub mod simulation_clock;
pub mod star_catalogue;
pub mod surface_lod;
pub mod terrain_lighting;
pub mod tile_cache;
pub mod tile_streaming;

pub use descriptor::{
    ARTWORK_ID, GALLERY_WORLD_ID, INSTALLATION_ID, ORBITING_EARTH, WORLD_IN_LIGHT,
};
pub use exhibition_camera::{CameraBasis, CelestialTarget, SpaceflightInput, SpaceflightState};
pub use navigation_display::format_camera_speed_mps;
pub use planet::{CelestialFrame, EarthEllipsoid, Vec3d};
pub use simulation_clock::SimulationClock;
