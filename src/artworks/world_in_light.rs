use crate::artwork::{Artwork, ArtworkKind, Destination};

pub const ORBITING_EARTH: Artwork = Artwork {
    id: "orbiting-earth",
    title: "A World in Light",
    artist: "Gallery collection",
    description: "Earth, Moon, atmosphere, sunlight, and shadow at astronomical scale.",
    destination: Destination {
        world: super::MVP_GALLERY_WORLD,
        installation: "orbiting-earth",
    },
    kind: ArtworkKind::OrbitingEarth,
};
