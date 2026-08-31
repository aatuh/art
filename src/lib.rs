//! Static WebAssembly composition layer for the Art gallery.
//!
//! Renderer-free contracts and artwork simulation live in independent workspace crates. This
//! package owns only shared visitor preferences and the browser adapter composition root.

pub mod input;

#[cfg(any(test, target_arch = "wasm32"))]
mod math;

pub use artwork_black_cube;
pub use artwork_world_in_light;
pub use gallery_core;

#[cfg(target_arch = "wasm32")]
mod browser;
