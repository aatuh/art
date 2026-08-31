//! Browser installation adapters grouped by artwork.

use gallery_core::{Artwork, InstallationId, WorldId, is_valid_stable_id};

use super::runtime::RuntimeFactory;

pub(super) mod black_cube;
pub(super) mod world_in_light;

pub(super) struct ArtworkComponent {
    pub descriptor: &'static Artwork,
    pub create_runtime: RuntimeFactory,
}

static COMPONENTS: [ArtworkComponent; 2] = [
    ArtworkComponent {
        descriptor: &artwork_black_cube::BLACK_CUBE_ROOM,
        create_runtime: black_cube::create,
    },
    ArtworkComponent {
        descriptor: &artwork_world_in_light::WORLD_IN_LIGHT,
        create_runtime: world_in_light::create,
    },
];

/// Typed world composition: catalogue descriptors are paired with their browser factories here.
pub(super) struct GalleryWorld {
    id: WorldId,
    components: &'static [ArtworkComponent],
}

impl GalleryWorld {
    pub(super) fn components(&self) -> impl ExactSizeIterator<Item = &'static ArtworkComponent> {
        self.components.iter()
    }

    /// Resolves untrusted artwork navigation only inside this composed world.
    pub(super) fn resolve_artwork(&self, raw_id: &str) -> Option<&'static ArtworkComponent> {
        let component = is_valid_stable_id(raw_id)
            .then(|| {
                self.components.iter().find(|component| {
                    let artwork = component.descriptor;
                    artwork.id.as_str() == raw_id && artwork.destination.world == self.id
                })
            })
            .flatten()?;
        self.resolve_installation(component.descriptor.destination.installation)
            .filter(|installation| std::ptr::eq(*installation, component))
    }

    pub(super) fn resolve_installation(
        &self,
        installation: InstallationId,
    ) -> Option<&'static ArtworkComponent> {
        self.components.iter().find(|component| {
            let destination = component.descriptor.destination;
            destination.world == self.id && destination.installation == installation
        })
    }
}

static GALLERY_WORLD: GalleryWorld = GalleryWorld {
    id: WorldId::new("mvp-gallery"),
    components: &COMPONENTS,
};

pub(super) const fn gallery_world() -> &'static GalleryWorld {
    &GALLERY_WORLD
}
