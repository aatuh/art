//! Renderer-independent identifiers, metadata, and catalogue operations for the gallery.
//!
//! All metadata uses borrowed static strings so artwork registries can be declared as constants
//! without allocation or runtime initialization.

use core::fmt;

/// Maximum byte length accepted by every public gallery identifier.
pub const MAX_STABLE_ID_BYTES: usize = 64;

/// Returns whether `value` is a canonical, URL-safe gallery identifier.
///
/// IDs contain lowercase ASCII letters and digits separated by single hyphens. They cannot begin
/// or end with a hyphen, and their bounded size makes validation predictable for untrusted route
/// or fragment input.
pub const fn is_valid_stable_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_STABLE_ID_BYTES {
        return false;
    }

    let mut index = 0;
    let mut previous_was_hyphen = false;
    while index < bytes.len() {
        let byte = bytes[index];
        let is_alphanumeric = byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if !is_alphanumeric && byte != b'-' {
            return false;
        }
        if byte == b'-' && (index == 0 || index + 1 == bytes.len() || previous_was_hyphen) {
            return false;
        }
        previous_was_hyphen = byte == b'-';
        index += 1;
    }
    true
}

macro_rules! stable_id_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(&'static str);

        impl $name {
            /// Creates an identifier for static catalogue data.
            ///
            /// # Panics
            ///
            /// Panics when `value` is not a canonical stable identifier. Because this function is
            /// `const`, invalid identifiers in static registries fail during compilation.
            pub const fn new(value: &'static str) -> Self {
                assert!(
                    is_valid_stable_id(value),
                    "invalid stable gallery identifier"
                );
                Self(value)
            }

            /// Attempts to create an identifier without panicking.
            pub const fn try_new(value: &'static str) -> Option<Self> {
                if is_valid_stable_id(value) {
                    Some(Self(value))
                } else {
                    None
                }
            }

            /// Returns the canonical string representation.
            pub const fn as_str(self) -> &'static str {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.0)
            }
        }
    };
}

stable_id_type!(ArtworkId, "Stable public identifier for an artwork.");
stable_id_type!(
    InstallationId,
    "Stable identifier for an installation within a world."
);
stable_id_type!(WorldId, "Stable identifier for a gallery world.");

/// A typed destination resolved by the gallery before mounting an installation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Destination {
    pub world: WorldId,
    pub installation: InstallationId,
}

/// Visitor-facing presentation owned by an artwork rather than by the gallery shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtworkPresentation {
    pub canvas_label: &'static str,
    pub observer_label: &'static str,
    pub enter_label: &'static str,
    pub controls_help: &'static str,
}

/// Renderer-independent metadata for a catalogued artwork.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Artwork {
    pub id: ArtworkId,
    pub title: &'static str,
    pub artist: &'static str,
    pub description: &'static str,
    pub destination: Destination,
    pub presentation: ArtworkPresentation,
}

/// A resolved catalogue entry and its copyable destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedArtwork<'a> {
    pub artwork: &'a Artwork,
    pub destination: Destination,
}

/// Read-only view over any artwork registry.
#[derive(Clone, Copy, Debug)]
pub struct Catalogue<'a> {
    artworks: &'a [Artwork],
}

impl<'a> Catalogue<'a> {
    /// Wraps a static or borrowed artwork registry without allocating.
    pub const fn new(artworks: &'a [Artwork]) -> Self {
        Self { artworks }
    }

    /// Returns the registry in its declared order.
    pub const fn artworks(self) -> &'a [Artwork] {
        self.artworks
    }

    /// Finds an artwork through a validated typed identifier.
    pub fn find(self, id: ArtworkId) -> Option<&'a Artwork> {
        self.find_raw(id.as_str())
    }

    /// Validates an untrusted string before looking it up in the registry.
    pub fn find_raw(self, id: &str) -> Option<&'a Artwork> {
        is_valid_stable_id(id)
            .then(|| {
                self.artworks
                    .iter()
                    .find(|artwork| artwork.id.as_str() == id)
            })
            .flatten()
    }

    /// Resolves a typed artwork identifier to its metadata and destination.
    pub fn resolve(self, id: ArtworkId) -> Option<ResolvedArtwork<'a>> {
        self.resolve_raw(id.as_str())
    }

    /// Validates and resolves an untrusted string without constructing a borrowed ID value.
    pub fn resolve_raw(self, id: &str) -> Option<ResolvedArtwork<'a>> {
        self.find_raw(id).map(|artwork| ResolvedArtwork {
            artwork,
            destination: artwork.destination,
        })
    }
}

/// Finds an artwork in an arbitrary registry through a typed identifier.
pub fn artwork_by_id(artworks: &[Artwork], id: ArtworkId) -> Option<&Artwork> {
    Catalogue::new(artworks).find(id)
}

/// Validates and finds an artwork in an arbitrary registry.
pub fn artwork_by_raw_id<'a>(artworks: &'a [Artwork], id: &str) -> Option<&'a Artwork> {
    Catalogue::new(artworks).find_raw(id)
}

/// Resolves an artwork in an arbitrary registry through a typed identifier.
pub fn resolve_destination(artworks: &[Artwork], id: ArtworkId) -> Option<ResolvedArtwork<'_>> {
    Catalogue::new(artworks).resolve(id)
}

/// Validates and resolves an artwork in an arbitrary registry.
pub fn resolve_raw_destination<'a>(
    artworks: &'a [Artwork],
    id: &str,
) -> Option<ResolvedArtwork<'a>> {
    Catalogue::new(artworks).resolve_raw(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: WorldId = WorldId::new("mvp-gallery");
    const CUBE_ID: ArtworkId = ArtworkId::new("black-cube-room");
    const CUBE_INSTALLATION: InstallationId = InstallationId::new("black-cube-room");
    const EARTH_ID: ArtworkId = ArtworkId::new("orbiting-earth");
    const EARTH_INSTALLATION: InstallationId = InstallationId::new("orbiting-earth");

    const PRESENTATION: ArtworkPresentation = ArtworkPresentation {
        canvas_label: "Artwork canvas",
        observer_label: "Artwork observer",
        enter_label: "Explore",
        controls_help: "Use the configured controls",
    };

    const ARTWORKS: [Artwork; 2] = [
        Artwork {
            id: CUBE_ID,
            title: "Black Cube / White Room",
            artist: "Gallery collection",
            description: "A black cube held in a silent white room.",
            destination: Destination {
                world: WORLD,
                installation: CUBE_INSTALLATION,
            },
            presentation: PRESENTATION,
        },
        Artwork {
            id: EARTH_ID,
            title: "A World in Light",
            artist: "Gallery collection",
            description: "A physically scaled planetary installation.",
            destination: Destination {
                world: WORLD,
                installation: EARTH_INSTALLATION,
            },
            presentation: PRESENTATION,
        },
    ];

    #[test]
    fn typed_ids_are_const_friendly_and_preserve_their_canonical_value() {
        assert_eq!(CUBE_ID.as_str(), "black-cube-room");
        assert_eq!(CUBE_INSTALLATION.as_ref(), "black-cube-room");
        assert_eq!(WORLD.to_string(), "mvp-gallery");
    }

    #[test]
    fn stable_id_validation_rejects_noncanonical_or_oversized_input() {
        for valid in ["a", "art-2", "black-cube-room", "mvp-gallery"] {
            assert!(is_valid_stable_id(valid), "{valid}");
        }
        for invalid in [
            "",
            "-art",
            "art-",
            "art--room",
            "Black-cube",
            "black_cube",
            "../black-cube",
            "art room",
        ] {
            assert!(!is_valid_stable_id(invalid), "{invalid}");
        }
        let oversized = "a".repeat(MAX_STABLE_ID_BYTES + 1);
        assert!(!is_valid_stable_id(&oversized));
        assert_eq!(ArtworkId::try_new("not_canonical"), None);
    }

    #[test]
    fn catalogue_looks_up_typed_and_validated_raw_ids() {
        let catalogue = Catalogue::new(&ARTWORKS);

        assert_eq!(catalogue.artworks(), &ARTWORKS);
        assert_eq!(
            catalogue.find(CUBE_ID).map(|artwork| artwork.title),
            Some("Black Cube / White Room")
        );
        assert_eq!(
            catalogue
                .find_raw("orbiting-earth")
                .map(|artwork| artwork.id),
            Some(EARTH_ID)
        );
        assert_eq!(catalogue.find_raw("../orbiting-earth"), None);
        assert_eq!(catalogue.find_raw("unknown-artwork"), None);
    }

    #[test]
    fn catalogue_resolves_the_registered_destination_without_renderer_data() {
        let resolved = Catalogue::new(&ARTWORKS)
            .resolve_raw("orbiting-earth")
            .expect("registered artwork");

        assert_eq!(resolved.artwork.id, EARTH_ID);
        assert_eq!(resolved.destination.world, WORLD);
        assert_eq!(resolved.destination.installation, EARTH_INSTALLATION);
    }

    #[test]
    fn free_functions_work_with_any_borrowed_registry() {
        let subset = &ARTWORKS[..1];

        assert_eq!(artwork_by_id(subset, CUBE_ID), Some(&ARTWORKS[0]));
        assert_eq!(artwork_by_raw_id(subset, "orbiting-earth"), None);
        assert_eq!(
            resolve_destination(subset, CUBE_ID).map(|resolved| resolved.destination),
            Some(ARTWORKS[0].destination)
        );
        assert_eq!(resolve_raw_destination(subset, "invalid_id"), None);
    }
}
