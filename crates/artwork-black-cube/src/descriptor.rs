use gallery_core::{Artwork, ArtworkId, ArtworkPresentation, Destination, InstallationId, WorldId};

/// Stable catalogue entry and visitor-facing presentation for this installation.
pub const BLACK_CUBE_ROOM: Artwork = Artwork {
    id: ArtworkId::new("black-cube-room"),
    title: "Black Cube / White Room",
    artist: "Gallery collection",
    description: "A black cube held in a silent white room.",
    destination: Destination {
        world: WorldId::new("mvp-gallery"),
        installation: InstallationId::new("black-cube-room"),
    },
    presentation: ArtworkPresentation {
        canvas_label: "A white room containing a black cube",
        observer_label: "First-person room. Activate to use mouse look. Move with WASD, jump with Space, and crouch with Control.",
        enter_label: "Explore room",
        controls_help: "Click the room to capture the mouse · WASD moves · Space jumps · Ctrl crouches · Esc releases the mouse",
    },
};

#[cfg(test)]
mod tests {
    use super::BLACK_CUBE_ROOM;

    #[test]
    fn descriptor_owns_stable_catalogue_and_presentation_data() {
        assert_eq!(BLACK_CUBE_ROOM.id.as_str(), "black-cube-room");
        assert_eq!(BLACK_CUBE_ROOM.destination.world.as_str(), "mvp-gallery");
        assert_eq!(
            BLACK_CUBE_ROOM.destination.installation.as_str(),
            BLACK_CUBE_ROOM.id.as_str()
        );
        assert_eq!(BLACK_CUBE_ROOM.presentation.enter_label, "Explore room");
        assert!(BLACK_CUBE_ROOM.presentation.controls_help.contains("WASD"));
    }
}
