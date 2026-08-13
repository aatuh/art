//! Deterministic simulation-time control for time-scaled artworks.

pub const REAL_TIME_SCALE: f64 = 1.0;
pub const TIME_SCALES: [f64; 4] = [REAL_TIME_SCALE, 60.0, 3600.0, 0.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimulationClock {
    seconds_since_epoch: f64,
    last_frame_ms: Option<f64>,
    scale_index: usize,
}

impl SimulationClock {
    pub fn new(seconds_since_epoch: f64) -> Self {
        Self {
            seconds_since_epoch: finite_or_zero(seconds_since_epoch),
            last_frame_ms: None,
            scale_index: 0,
        }
    }

    pub fn seconds_since_epoch(self) -> f64 {
        self.seconds_since_epoch
    }

    pub fn scale(self) -> f64 {
        TIME_SCALES[self.scale_index]
    }

    pub fn cycle_scale(&mut self) -> f64 {
        self.scale_index = (self.scale_index + 1) % TIME_SCALES.len();
        self.scale()
    }

    pub fn advance(&mut self, frame_ms: f64) -> f64 {
        if !frame_ms.is_finite() {
            return self.seconds_since_epoch;
        }
        let Some(previous_ms) = self.last_frame_ms.replace(frame_ms) else {
            return self.seconds_since_epoch;
        };
        let elapsed_seconds = ((frame_ms - previous_ms) / 1000.0).max(0.0);
        if elapsed_seconds.is_finite() {
            self.seconds_since_epoch += elapsed_seconds * self.scale();
        }
        self.seconds_since_epoch
    }

    pub fn animation_seconds(self) -> f64 {
        self.seconds_since_epoch.rem_euclid(86_400.0)
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_defaults_to_real_time_and_advances_from_second_frame() {
        let mut clock = SimulationClock::new(1000.0);
        assert_eq!(clock.scale(), 1.0);
        assert_eq!(clock.advance(500.0), 1000.0);
        assert_eq!(clock.advance(1500.0), 1001.0);
    }

    #[test]
    fn time_scale_cycles_through_fast_motion_pause_and_real_time() {
        let mut clock = SimulationClock::new(0.0);
        assert_eq!(clock.cycle_scale(), 60.0);
        assert_eq!(clock.cycle_scale(), 3600.0);
        assert_eq!(clock.cycle_scale(), 0.0);
        assert_eq!(clock.cycle_scale(), 1.0);
    }

    #[test]
    fn paused_clock_does_not_advance() {
        let mut clock = SimulationClock::new(42.0);
        clock.advance(0.0);
        clock.cycle_scale();
        clock.cycle_scale();
        clock.cycle_scale();
        assert_eq!(clock.scale(), 0.0);
        assert_eq!(clock.advance(10_000.0), 42.0);
    }

    #[test]
    fn accelerated_clock_advances_by_selected_multiplier() {
        let mut clock = SimulationClock::new(5.0);
        clock.advance(1000.0);
        clock.cycle_scale();
        assert_eq!(clock.scale(), 60.0);
        assert_eq!(clock.advance(2000.0), 65.0);
        clock.cycle_scale();
        assert_eq!(clock.scale(), 3600.0);
        assert_eq!(clock.advance(3000.0), 3665.0);
    }

    #[test]
    fn invalid_or_backward_frame_values_do_not_rewind_time() {
        let mut clock = SimulationClock::new(f64::NAN);
        assert_eq!(clock.seconds_since_epoch(), 0.0);
        clock.advance(1000.0);
        assert_eq!(clock.advance(500.0), 0.0);
        assert_eq!(clock.advance(f64::NAN), 0.0);
    }
}
