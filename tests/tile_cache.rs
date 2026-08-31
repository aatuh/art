use artwork_world_in_light::{
    planet_tiles::{CubeFace, TileAssetRequest, TileId, TileLayer},
    tile_cache::{ResidentTileCache, SlotReservation},
};

fn request(x: u32) -> TileAssetRequest {
    TileAssetRequest {
        layer: TileLayer::Surface,
        tile: TileId::new(CubeFace::PositiveZ, 3, x, 0).expect("valid tile"),
    }
}

#[test]
fn streamed_tile_slots_preserve_recent_coverage() {
    let mut cache = ResidentTileCache::new(2);
    assert_eq!(
        cache.reserve(request(0)),
        Some(SlotReservation::Vacant { slot: 0 })
    );
    assert_eq!(
        cache.reserve(request(1)),
        Some(SlotReservation::Vacant { slot: 1 })
    );
    assert!(cache.touch(request(0)));
    assert_eq!(
        cache.reserve(request(2)),
        Some(SlotReservation::Evicted {
            slot: 1,
            previous: request(1),
        })
    );
    assert_eq!(cache.slot_for(request(0)), Some(0));
    assert_eq!(cache.slot_for(request(2)), Some(1));
}
