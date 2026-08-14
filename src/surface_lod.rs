//! Renderer-independent Earth surface level-of-detail selection.

/// Above this altitude the global reference texture is sufficient.
pub const ORBITAL_DETAIL_ALTITUDE_M: f64 = 30_000_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceTier {
    Global,
    Orbital,
}

/// Chooses the cheapest available surface tier that still provides useful detail.
///
/// Non-finite altitude values deliberately fall back to the global tier so that a
/// malformed camera state cannot force the renderer onto a more expensive path.
pub fn select_surface_tier(altitude_m: f64, orbital_available: bool) -> SurfaceTier {
    if orbital_available
        && altitude_m.is_finite()
        && altitude_m.max(0.0) <= ORBITAL_DETAIL_ALTITUDE_M
    {
        SurfaceTier::Orbital
    } else {
        SurfaceTier::Global
    }
}

#[cfg(test)]
mod tests {
    use super::{ORBITAL_DETAIL_ALTITUDE_M, SurfaceTier, select_surface_tier};

    #[test]
    fn orbital_texture_is_used_only_when_available_and_close_enough() {
        assert_eq!(
            select_surface_tier(ORBITAL_DETAIL_ALTITUDE_M, true),
            SurfaceTier::Orbital
        );
        assert_eq!(
            select_surface_tier(ORBITAL_DETAIL_ALTITUDE_M + 1.0, true),
            SurfaceTier::Global
        );
        assert_eq!(select_surface_tier(1_000.0, false), SurfaceTier::Global);
    }

    #[test]
    fn invalid_altitudes_fall_back_to_the_global_tier() {
        for altitude in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(select_surface_tier(altitude, true), SurfaceTier::Global);
        }
    }

    #[test]
    fn negative_altitudes_are_clamped_to_surface_distance() {
        assert_eq!(select_surface_tier(-100.0, true), SurfaceTier::Orbital);
    }
}
