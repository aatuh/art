//! Self-contained metadata for gallery artworks.

mod black_cube_room;
mod world_in_light;

use crate::artwork::Artwork;

pub use black_cube_room::BLACK_CUBE_ROOM;
pub use world_in_light::ORBITING_EARTH;

/// Stable world identifier used by every currently published installation.
pub const MVP_GALLERY_WORLD: &str = "mvp-gallery";

pub const ARTWORKS: [Artwork; 2] = [BLACK_CUBE_ROOM, ORBITING_EARTH];
