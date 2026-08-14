//! Renderer-independent gallery domain and first-person movement rules.

pub mod artwork;
pub mod artworks;
pub mod camera_api;
pub mod earth_coordinates;
pub mod exhibition_camera;
pub mod fps;
#[cfg(any(test, target_arch = "wasm32"))]
mod math;
pub mod planet;
pub mod planet_tiles;
pub mod simulation_clock;
pub mod surface_lod;
pub mod terrain_lighting;
pub mod tile_cache;
pub mod tile_streaming;

pub use artwork::{Artwork, ArtworkKind, Destination, is_valid_artwork_id};
pub use artworks::{ARTWORKS, BLACK_CUBE_ROOM, MVP_GALLERY_WORLD, ORBITING_EARTH};

pub fn artwork_by_id(id: &str) -> Option<&'static Artwork> {
    artwork::artwork_by_id(&ARTWORKS, id)
}

pub fn resolve_destination(id: &str) -> Option<(&'static Artwork, Destination)> {
    artwork::resolve_destination(&ARTWORKS, id)
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(test)]
mod tests {
    use super::{ARTWORKS, ArtworkKind, Destination, is_valid_artwork_id, resolve_destination};
    use crate::fps::{FpsInput, FpsSettings, PLAYER_RADIUS, PlayerState, ROOM_HALF_EXTENT};

    #[test]
    fn cube_artwork_resolves_to_its_gallery_destination() {
        let (artwork, destination) =
            resolve_destination("black-cube-room").expect("registered artwork");

        assert_eq!(artwork.title, "Black Cube / White Room");
        assert_eq!(artwork.kind, ArtworkKind::BlackCubeRoom);
        assert_eq!(
            destination,
            Destination {
                world: "mvp-gallery",
                installation: "black-cube-room",
            }
        );
    }

    #[test]
    fn earth_artwork_resolves_to_its_own_stable_destination() {
        let (artwork, destination) =
            resolve_destination("orbiting-earth").expect("registered artwork");

        assert_eq!(artwork.title, "A World in Light");
        assert_eq!(artwork.kind, ArtworkKind::OrbitingEarth);
        assert_eq!(destination.installation, "orbiting-earth");
    }

    #[test]
    fn malformed_and_unknown_ids_are_not_resolved() {
        for id in [
            "",
            "../black-cube-room",
            "black_cube_room",
            "unknown-room",
            "BLACK-CUBE-ROOM",
        ] {
            assert_eq!(resolve_destination(id), None, "{id} must not resolve");
        }
    }

    #[test]
    fn published_artworks_have_unique_ids_and_explicit_render_kinds() {
        for artwork in ARTWORKS {
            assert!(is_valid_artwork_id(artwork.id));
            assert!(matches!(
                artwork.kind,
                ArtworkKind::BlackCubeRoom | ArtworkKind::OrbitingEarth
            ));
        }

        let unique_ids = ARTWORKS
            .iter()
            .map(|artwork| artwork.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique_ids.len(), ARTWORKS.len());
    }

    #[test]
    fn stored_control_settings_are_clamped_and_untrusted_bindings_are_rejected() {
        let settings = FpsSettings::from_storage(Some(
            r#"{"mouse_sensitivity":99,"bindings":{"forward":"<script>","backward":"KeyS","left":"KeyA","right":"KeyD","jump":"Space","crouch":"ControlLeft"}}"#,
        ));

        assert_eq!(settings.mouse_sensitivity, 0.8);
        assert_eq!(settings.bindings.forward, "KeyW");
        assert!(!settings.invert_mouse_y);
    }

    #[test]
    fn stored_control_bindings_are_made_unique() {
        let settings = FpsSettings::from_storage(Some(
            r#"{"bindings":{"forward":"KeyS","backward":"KeyS","left":"KeyS","right":"KeyS","jump":"KeyS","crouch":"KeyS"}}"#,
        ));
        let bindings = [
            settings.bindings.forward,
            settings.bindings.backward,
            settings.bindings.left,
            settings.bindings.right,
            settings.bindings.jump,
            settings.bindings.crouch,
        ];

        assert_eq!(
            bindings
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            bindings.len()
        );
    }

    #[test]
    fn player_jumps_lands_and_cannot_leave_the_room_or_pass_the_cube() {
        let settings = FpsSettings::default();
        let mut player = PlayerState::default();
        player.request_jump();
        player.tick(0.05, FpsInput::default());
        assert!(player.jump_height > 0.0);
        for _ in 0..60 {
            player.tick(0.05, FpsInput::default());
        }
        assert!(player.grounded);
        assert_eq!(player.jump_height, 0.0);

        player.z = 7.5;
        player.tick(
            10.0,
            FpsInput {
                backward: true,
                ..FpsInput::default()
            },
        );
        assert!(player.z <= ROOM_HALF_EXTENT - PLAYER_RADIUS);

        player.z = 2.0;
        player.yaw_degrees = 0.0;
        for _ in 0..20 {
            player.tick(
                0.05,
                FpsInput {
                    forward: true,
                    ..FpsInput::default()
                },
            );
        }
        assert!(player.z >= 1.0 + PLAYER_RADIUS);
        player.look(1.0, 1.0, &settings);
        assert!(player.pitch_degrees > 0.0);
    }

    #[test]
    fn strafe_keys_follow_the_camera_and_crouch_interpolates() {
        let mut left = PlayerState::default();
        left.tick(
            0.05,
            FpsInput {
                left: true,
                ..FpsInput::default()
            },
        );
        assert!(left.x < 0.0, "A must move left while facing the cube");

        let mut right = PlayerState::default();
        right.tick(
            0.05,
            FpsInput {
                right: true,
                ..FpsInput::default()
            },
        );
        assert!(right.x > 0.0, "D must move right while facing the cube");

        let standing_eye_height = right.eye_height();
        right.tick(
            0.05,
            FpsInput {
                crouch: true,
                ..FpsInput::default()
            },
        );
        assert!(right.crouch_amount > 0.0 && right.crouch_amount < 1.0);
        assert!(right.eye_height() < standing_eye_height);
        assert!(right.eye_height() > crate::fps::CROUCHING_EYE_HEIGHT);
        right.tick(0.02, FpsInput::default());
        assert!(right.crouch_amount > 0.0 && right.crouch_amount < 1.0);
    }
}
