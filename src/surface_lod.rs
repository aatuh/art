//! Renderer-independent Earth surface level-of-detail selection.

/// Above this altitude the global reference texture is sufficient.
pub const ORBITAL_DETAIL_ALTITUDE_M: f64 = 30_000_000.0;
/// Below this altitude the denser regional texture becomes worthwhile.
pub const REGIONAL_DETAIL_ALTITUDE_M: f64 = 5_000_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceTier {
    Global,
    Orbital,
    Regional,
}

/// Chooses the cheapest available surface tier that still provides useful detail.
///
/// Non-finite altitude values deliberately fall back to the global tier so that a
/// malformed camera state cannot force the renderer onto a more expensive path.
pub fn select_surface_tier(
    altitude_m: f64,
    orbital_available: bool,
    regional_available: bool,
) -> SurfaceTier {
    if !altitude_m.is_finite() {
        return SurfaceTier::Global;
    }

    let altitude_m = altitude_m.max(0.0);
    if regional_available && altitude_m <= REGIONAL_DETAIL_ALTITUDE_M {
        SurfaceTier::Regional
    } else if orbital_available && altitude_m <= ORBITAL_DETAIL_ALTITUDE_M {
        SurfaceTier::Orbital
    } else {
        SurfaceTier::Global
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ORBITAL_DETAIL_ALTITUDE_M, REGIONAL_DETAIL_ALTITUDE_M, SurfaceTier, select_surface_tier,
    };

    #[test]
    fn texture_tiers_follow_altitude_when_all_are_available() {
        assert_eq!(
            select_surface_tier(ORBITAL_DETAIL_ALTITUDE_M + 1.0, true, true),
            SurfaceTier::Global
        );
        assert_eq!(
            select_surface_tier(ORBITAL_DETAIL_ALTITUDE_M, true, true),
            SurfaceTier::Orbital
        );
        assert_eq!(
            select_surface_tier(REGIONAL_DETAIL_ALTITUDE_M + 1.0, true, true),
            SurfaceTier::Orbital
        );
        assert_eq!(
            select_surface_tier(REGIONAL_DETAIL_ALTITUDE_M, true, true),
            SurfaceTier::Regional
        );
    }

    #[test]
    fn missing_detail_tiers_fall_back_gracefully() {
        assert_eq!(
            select_surface_tier(1_000.0, true, false),
            SurfaceTier::Orbital
        );
        assert_eq!(
            select_surface_tier(1_000.0, false, true),
            SurfaceTier::Regional
        );
        assert_eq!(
            select_surface_tier(1_000.0, false, false),
            SurfaceTier::Global
        );
    }

    #[test]
    fn invalid_altitudes_fall_back_to_the_global_tier() {
        for altitude in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                select_surface_tier(altitude, true, true),
                SurfaceTier::Global
            );
        }
    }

    #[test]
    fn negative_altitudes_are_clamped_to_surface_distance() {
        assert_eq!(
            select_surface_tier(-100.0, true, true),
            SurfaceTier::Regional
        );
    }
}
