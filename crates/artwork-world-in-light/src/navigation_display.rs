//! Human-readable navigation telemetry for the astronomical camera.

/// Formats a virtual-camera speed with a compact SI unit suitable for an on-screen HUD.
pub fn format_camera_speed_mps(speed_mps: f64) -> String {
    let speed = if speed_mps.is_finite() {
        speed_mps.abs()
    } else {
        0.0
    };
    if speed < 10.0 {
        format!("Speed {speed:.2} m/s")
    } else if speed < 1_000.0 {
        format!("Speed {speed:.0} m/s")
    } else if speed < 1_000_000.0 {
        format!("Speed {:.1} km/s", speed / 1_000.0)
    } else if speed < 1_000_000_000.0 {
        format!("Speed {:.2} Mm/s", speed / 1_000_000.0)
    } else {
        format!("Speed {:.2} Gm/s", speed / 1_000_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::format_camera_speed_mps;

    #[test]
    fn speed_display_uses_readable_si_scales() {
        assert_eq!(format_camera_speed_mps(0.25), "Speed 0.25 m/s");
        assert_eq!(format_camera_speed_mps(25.0), "Speed 25 m/s");
        assert_eq!(format_camera_speed_mps(25_000.0), "Speed 25.0 km/s");
        assert_eq!(format_camera_speed_mps(25_000_000.0), "Speed 25.00 Mm/s");
        assert_eq!(format_camera_speed_mps(2_500_000_000.0), "Speed 2.50 Gm/s");
    }

    #[test]
    fn invalid_and_negative_speeds_are_safe_for_display() {
        assert_eq!(format_camera_speed_mps(f64::NAN), "Speed 0.00 m/s");
        assert_eq!(format_camera_speed_mps(-1_500.0), "Speed 1.5 km/s");
    }
}
