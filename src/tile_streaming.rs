//! Pure tile residency and request scheduling for multiscale Earth streaming.

use std::collections::BTreeSet;

use crate::planet_tiles::{TileAssetRequest, TileId, TileLayer, TileStreamingPlan};

pub const DEFAULT_MAX_NEW_REQUESTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileLoadDelta {
    /// Missing assets to start loading this frame, ordered coarse-to-fine.
    pub requested: Vec<TileAssetRequest>,
    /// Assets that are still relevant to the current camera working set.
    pub retained: BTreeSet<TileAssetRequest>,
}

/// Computes a bounded set of new requests from the current streaming plan.
///
/// `resident` contains already decoded GPU-ready assets and `in_flight` contains
/// requests that have been started but have not completed. Neither set is
/// re-requested. Parent fallback assets remain relevant while fine tiles load.
pub fn plan_tile_loads(
    plan: &TileStreamingPlan,
    layer: TileLayer,
    resident: &BTreeSet<TileAssetRequest>,
    in_flight: &BTreeSet<TileAssetRequest>,
    max_new_requests: usize,
) -> TileLoadDelta {
    let ordered = plan.requests(layer);
    let retained = ordered.iter().copied().collect::<BTreeSet<_>>();
    let requested = ordered
        .into_iter()
        .filter(|request| !resident.contains(request) && !in_flight.contains(request))
        .take(max_new_requests)
        .collect();
    TileLoadDelta {
        requested,
        retained,
    }
}

/// Returns the finest resident tile that can cover `tile`, including the tile itself.
pub fn finest_resident_coverage(
    tile: TileId,
    layer: TileLayer,
    resident: &BTreeSet<TileAssetRequest>,
) -> Option<TileAssetRequest> {
    let mut candidate = Some(tile);
    while let Some(tile) = candidate {
        let request = TileAssetRequest { layer, tile };
        if resident.contains(&request) {
            return Some(request);
        }
        candidate = tile.parent();
    }
    None
}

/// Selects stale resident assets that may be evicted without removing any
/// currently requested fine tile or its fallback ancestry.
pub fn stale_resident_assets(
    retained: &BTreeSet<TileAssetRequest>,
    resident: &BTreeSet<TileAssetRequest>,
) -> Vec<TileAssetRequest> {
    resident.difference(retained).copied().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{finest_resident_coverage, plan_tile_loads, stale_resident_assets};
    use crate::{
        planet::Vec3d,
        planet_tiles::{
            CubeFace, TileAssetRequest, TileId, TileLayer, surface_tile_streaming_plan,
        },
    };

    fn request(layer: TileLayer, tile: TileId) -> TileAssetRequest {
        TileAssetRequest { layer, tile }
    }

    #[test]
    fn planner_requests_missing_fallbacks_before_fine_tiles() {
        let plan = surface_tile_streaming_plan(Vec3d::new(0.0, 0.0, 1.0), 400_000.0, 0);
        let delta = plan_tile_loads(
            &plan,
            TileLayer::Surface,
            &BTreeSet::new(),
            &BTreeSet::new(),
            4,
        );
        assert_eq!(delta.requested.len(), 4);
        assert!(delta
            .requested
            .windows(2)
            .all(|pair| pair[0].tile.level <= pair[1].tile.level));
        assert!(delta
            .requested
            .iter()
            .all(|request| request.tile.level < plan.level));
    }

    #[test]
    fn planner_skips_resident_and_in_flight_assets() {
        let plan = surface_tile_streaming_plan(Vec3d::new(0.0, 0.0, 1.0), 400_000.0, 0);
        let ordered = plan.requests(TileLayer::Surface);
        let resident = BTreeSet::from([ordered[0]]);
        let in_flight = BTreeSet::from([ordered[1]]);
        let delta = plan_tile_loads(&plan, TileLayer::Surface, &resident, &in_flight, 3);
        assert_eq!(delta.requested, ordered[2..5]);
        assert!(delta.retained.contains(&ordered[0]));
        assert!(delta.retained.contains(&ordered[1]));
    }

    #[test]
    fn zero_budget_starts_no_new_requests_but_retains_working_set() {
        let plan = surface_tile_streaming_plan(Vec3d::new(0.0, 0.0, 1.0), 400_000.0, 1);
        let delta = plan_tile_loads(
            &plan,
            TileLayer::Elevation,
            &BTreeSet::new(),
            &BTreeSet::new(),
            0,
        );
        assert!(delta.requested.is_empty());
        assert_eq!(delta.retained.len(), plan.requests(TileLayer::Elevation).len());
    }

    #[test]
    fn finest_coverage_prefers_child_over_parent() {
        let child = TileId::new(CubeFace::PositiveZ, 4, 7, 9).expect("valid child");
        let parent = child.parent().expect("parent");
        let grandparent = parent.parent().expect("grandparent");
        let layer = TileLayer::Surface;
        let resident = BTreeSet::from([
            request(layer, grandparent),
            request(layer, parent),
            request(layer, child),
        ]);
        assert_eq!(
            finest_resident_coverage(child, layer, &resident),
            Some(request(layer, child))
        );
    }

    #[test]
    fn finest_coverage_falls_back_through_ancestors() {
        let child = TileId::new(CubeFace::NegativeX, 5, 3, 17).expect("valid child");
        let parent = child.parent().expect("parent");
        let resident = BTreeSet::from([request(TileLayer::Elevation, parent)]);
        assert_eq!(
            finest_resident_coverage(child, TileLayer::Elevation, &resident),
            Some(request(TileLayer::Elevation, parent))
        );
        assert_eq!(
            finest_resident_coverage(child, TileLayer::Surface, &resident),
            None
        );
    }

    #[test]
    fn stale_selection_preserves_only_current_working_set_assets() {
        let tile_a = TileId::new(CubeFace::PositiveZ, 2, 1, 1).expect("tile a");
        let tile_b = TileId::new(CubeFace::PositiveZ, 2, 2, 1).expect("tile b");
        let keep = request(TileLayer::Surface, tile_a);
        let stale = request(TileLayer::Surface, tile_b);
        let resident = BTreeSet::from([keep, stale]);
        let retained = BTreeSet::from([keep]);
        assert_eq!(stale_resident_assets(&retained, &resident), vec![stale]);
    }
}
