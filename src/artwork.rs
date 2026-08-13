//! Renderer-independent artwork discovery and destination contracts.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Destination {
    pub world: &'static str,
    pub installation: &'static str,
}

/// The browser adapter selects a renderer from this stable, domain-owned kind.
/// It is intentionally free of Web APIs and renderer types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkKind {
    BlackCubeRoom,
    OrbitingEarth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Artwork {
    pub id: &'static str,
    pub title: &'static str,
    pub artist: &'static str,
    pub description: &'static str,
    pub destination: Destination,
    pub kind: ArtworkKind,
}

pub fn artwork_by_id(artworks: &'static [Artwork], id: &str) -> Option<&'static Artwork> {
    artworks.iter().find(|artwork| artwork.id == id)
}

/// Resolves only stable public IDs to registered destinations.
pub fn resolve_destination(
    artworks: &'static [Artwork],
    id: &str,
) -> Option<(&'static Artwork, Destination)> {
    is_valid_artwork_id(id)
        .then(|| artwork_by_id(artworks, id))
        .flatten()
        .map(|artwork| (artwork, artwork.destination))
}

pub fn is_valid_artwork_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
