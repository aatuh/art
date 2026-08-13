use crate::artwork::{Artwork, ArtworkKind, Destination};

pub const BLACK_CUBE_ROOM: Artwork = Artwork {
    id: "black-cube-room",
    title: "Black Cube / White Room",
    artist: "Gallery collection",
    description: "A black cube held in a silent white room.",
    destination: Destination {
        world: super::MVP_GALLERY_WORLD,
        installation: "black-cube-room",
    },
    kind: ArtworkKind::BlackCubeRoom,
};
