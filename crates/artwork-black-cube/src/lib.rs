//! Self-contained Black Cube / White Room artwork.
//!
//! The crate owns the installation's metadata, authored room, geometry, collision
//! rules, and deterministic first-person simulation. Browser event handling,
//! renderer APIs, and persisted key bindings deliberately live elsewhere.

mod descriptor;
mod player;
mod scene;

pub use descriptor::BLACK_CUBE_ROOM;
pub use player::{
    CROUCHING_EYE_HEIGHT, PLAYER_RADIUS, PlayerInput, PlayerState, STANDING_EYE_HEIGHT,
};
pub use scene::{BLACK_CUBE_SCENE, RoomScene, RoomVertex};
