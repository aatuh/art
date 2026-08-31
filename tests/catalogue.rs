use artwork_black_cube::BLACK_CUBE_ROOM;
use artwork_world_in_light::WORLD_IN_LIGHT;
use gallery_core::{Artwork, Catalogue, WorldId};

const ARTWORKS: [Artwork; 2] = [BLACK_CUBE_ROOM, WORLD_IN_LIGHT];

#[test]
fn independent_artwork_crates_compose_into_one_typed_catalogue() {
    let catalogue = Catalogue::new(&ARTWORKS);
    let cube = catalogue
        .resolve_raw("black-cube-room")
        .expect("Black Cube descriptor");
    let earth = catalogue
        .resolve_raw("orbiting-earth")
        .expect("World in Light descriptor");

    assert_eq!(catalogue.artworks().len(), 2);
    assert_eq!(cube.destination.world, WorldId::new("mvp-gallery"));
    assert_eq!(earth.destination.world, cube.destination.world);
    assert_ne!(cube.artwork.id, earth.artwork.id);
    assert_ne!(
        cube.destination.installation,
        earth.destination.installation
    );
    assert!(!cube.artwork.presentation.enter_label.is_empty());
    assert!(!earth.artwork.presentation.enter_label.is_empty());
}

#[test]
fn malformed_routes_never_reach_an_artwork_component() {
    let catalogue = Catalogue::new(&ARTWORKS);
    for raw_id in ["", "../orbiting-earth", "Orbiting-Earth", "orbiting_earth"] {
        assert!(catalogue.resolve_raw(raw_id).is_none(), "{raw_id}");
    }
}
