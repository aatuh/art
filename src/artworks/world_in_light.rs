use crate::artwork::{Artwork, ArtworkKind, Destination};

pub const ORBITING_EARTH: Artwork = Artwork {
    id: "orbiting-earth",
    title: "A World in Light",
    artist: "Gallery collection",
    description: "A living blue world, held in a room beneath an unseen sun.",
    destination: Destination {
        world: super::MVP_GALLERY_WORLD,
        installation: "orbiting-earth",
    },
    kind: ArtworkKind::OrbitingEarth,
};
