//! Gallery-facing identity and visitor copy for this artwork.

use gallery_core::{Artwork, ArtworkId, ArtworkPresentation, Destination, InstallationId, WorldId};

pub const ARTWORK_ID: ArtworkId = ArtworkId::new("orbiting-earth");
pub const INSTALLATION_ID: InstallationId = InstallationId::new("orbiting-earth");
pub const GALLERY_WORLD_ID: WorldId = WorldId::new("mvp-gallery");

/// Stable catalogue descriptor consumed by the gallery shell.
pub const WORLD_IN_LIGHT: Artwork = Artwork {
    id: ARTWORK_ID,
    title: "A World in Light",
    artist: "Gallery collection",
    description: "Earth, Moon, atmosphere, sunlight, and shadow at astronomical scale.",
    destination: Destination {
        world: GALLERY_WORLD_ID,
        installation: INSTALLATION_ID,
    },
    presentation: ArtworkPresentation {
        canvas_label: "A physically scaled Earth, Moon, and Sun viewed from a free-flight camera",
        observer_label: "Free-flight space view. Activate to use mouse look. Move with WASD, ascend with Space, descend with Control, and hold Shift for fast travel.",
        enter_label: "Explore space",
        controls_help: "Click the view to capture the mouse · WASD moves · Space/Ctrl ascend/descend · Shift boosts · F faces Earth · R resets · Z/C changes speed · X stops · T changes time · Esc releases",
    },
};

/// Compatibility name used by the current root catalogue.
pub const ORBITING_EARTH: Artwork = WORLD_IN_LIGHT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_keeps_its_published_identity_and_destination() {
        assert_eq!(WORLD_IN_LIGHT.id, ARTWORK_ID);
        assert_eq!(WORLD_IN_LIGHT.destination.world, GALLERY_WORLD_ID);
        assert_eq!(WORLD_IN_LIGHT.destination.installation, INSTALLATION_ID);
        assert!(!WORLD_IN_LIGHT.presentation.controls_help.is_empty());
    }
}
