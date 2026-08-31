//! Pure framebuffer sizing policy shared by browser render adapters.

/// Keeps high-DPI output crisp without allowing a 4K/Retina display to multiply
/// fragment work without bound.
pub const MAX_FRAMEBUFFER_PIXELS: f64 = 2_000_000.0;
pub const MAX_DEVICE_PIXEL_RATIO: f64 = 2.0;
pub const MIN_FRAMEBUFFER_SCALE: f64 = 0.25;

/// The adaptive multiplier never removes more than three quarters of the pixels.
///
/// A multiplier of `0.75` retains over half the native fragments. Going lower makes the global
/// imagery visibly blocky even though explicit LOD selection keeps procedural detail band-limited.
pub const MIN_ADAPTIVE_QUALITY_SCALE: f64 = 0.75;
pub const MAX_ADAPTIVE_QUALITY_SCALE: f64 = 1.0;

const TARGET_FRAME_TIME_MS: f64 = 1_000.0 / 60.0;
// A 60 Hz requestAnimationFrame loop cannot normally report samples below 16.67 ms.
// Treating a stable 60 Hz cadence as spare capacity lets a reduced renderer cautiously
// probe upward again after a temporary slowdown.
const FAST_FRAME_TIME_MS: f64 = 17.25;
const SLOW_FRAME_TIME_MS: f64 = 20.0;
const FRAME_TIME_EMA_ALPHA: f64 = 0.12;
const MAX_EMA_FRAME_TIME_MS: f64 = 100.0;
const MAX_EVIDENCE_PER_SAMPLE_MS: f64 = 1_000.0 / 30.0;
const SLOW_EVIDENCE_REQUIRED_MS: f64 = 400.0;
const FAST_EVIDENCE_REQUIRED_MS: f64 = 900.0;
const DOWNSCALE_COOLDOWN_MS: f64 = 600.0;
const UPSCALE_COOLDOWN_MS: f64 = 600.0;
const DOWNSCALE_FACTOR: f64 = 0.85;
const UPSCALE_STEP: f64 = 0.15;

pub fn framebuffer_scale(width: u32, height: u32, device_pixel_ratio: f64) -> f64 {
    let css_pixels = f64::from(width.max(1)) * f64::from(height.max(1));
    let desired = if device_pixel_ratio.is_finite() {
        device_pixel_ratio.clamp(1.0, MAX_DEVICE_PIXEL_RATIO)
    } else {
        1.0
    };
    desired
        .min((MAX_FRAMEBUFFER_PIXELS / css_pixels).sqrt())
        .max(MIN_FRAMEBUFFER_SCALE)
}

/// Deterministic dynamic-resolution policy driven by observed frame cadence.
///
/// Slow frames reduce quality relatively quickly. Quality only rises after a longer period
/// at a stable 60 Hz cadence, preventing resize churn around the target. The controller is
/// independent of browser APIs so its decisions remain unit-testable and reusable by another
/// rendering adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveFramebufferQuality {
    quality_scale: f64,
    ema_frame_time_ms: Option<f64>,
    last_timestamp_ms: Option<f64>,
    slow_evidence_ms: f64,
    fast_evidence_ms: f64,
    cooldown_remaining_ms: f64,
}

impl Default for AdaptiveFramebufferQuality {
    fn default() -> Self {
        Self {
            quality_scale: MAX_ADAPTIVE_QUALITY_SCALE,
            ema_frame_time_ms: None,
            last_timestamp_ms: None,
            slow_evidence_ms: 0.0,
            fast_evidence_ms: 0.0,
            cooldown_remaining_ms: 0.0,
        }
    }
}

impl AdaptiveFramebufferQuality {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn quality_scale(self) -> f64 {
        self.quality_scale
    }

    pub fn ema_frame_time_ms(self) -> Option<f64> {
        self.ema_frame_time_ms
    }

    /// Returns the static pixel-budget scale adjusted by the current adaptive multiplier.
    pub fn effective_framebuffer_scale(
        self,
        width: u32,
        height: u32,
        device_pixel_ratio: f64,
    ) -> f64 {
        (framebuffer_scale(width, height, device_pixel_ratio) * self.quality_scale)
            .max(MIN_FRAMEBUFFER_SCALE)
    }

    /// Records a requestAnimationFrame-style monotonic timestamp.
    ///
    /// The first timestamp establishes a baseline. Non-finite and non-increasing timestamps
    /// are ignored. Every positive finite interval contributes bounded timing evidence, including
    /// very slow foreground frames. Call [`Self::reset_timing`] when animation is suspended or the
    /// document's visibility changes so a background-tab gap is not treated as rendering work.
    /// The return value is `true` only when the quality scale changed.
    pub fn observe_frame_timestamp_ms(&mut self, timestamp_ms: f64) -> bool {
        if !timestamp_ms.is_finite() {
            return false;
        }
        let Some(previous_timestamp_ms) = self.last_timestamp_ms else {
            self.last_timestamp_ms = Some(timestamp_ms);
            return false;
        };
        if timestamp_ms <= previous_timestamp_ms {
            return false;
        }
        self.last_timestamp_ms = Some(timestamp_ms);
        self.observe_frame_time_ms(timestamp_ms - previous_timestamp_ms)
    }

    /// Records a measured CPU, animation-frame, or GPU frame duration.
    ///
    /// Do not mix duration samples with timestamp samples for the same frame. Every positive,
    /// finite duration is accepted; its contribution to the EMA and evidence counters is clamped
    /// so a single long foreground frame cannot dominate the controller. Invalid, zero, and
    /// negative samples leave the controller unchanged.
    pub fn observe_frame_time_ms(&mut self, frame_time_ms: f64) -> bool {
        if !is_valid_frame_time(frame_time_ms) {
            return false;
        }

        let filtered_frame_time_ms = frame_time_ms.min(MAX_EMA_FRAME_TIME_MS);
        let previous_ema = self.ema_frame_time_ms.unwrap_or(TARGET_FRAME_TIME_MS);
        let ema = previous_ema + (filtered_frame_time_ms - previous_ema) * FRAME_TIME_EMA_ALPHA;
        self.ema_frame_time_ms = Some(ema);

        if self.cooldown_remaining_ms > 0.0 {
            self.cooldown_remaining_ms = (self.cooldown_remaining_ms - frame_time_ms).max(0.0);
            self.clear_evidence();
            return false;
        }

        let evidence = frame_time_ms.min(MAX_EVIDENCE_PER_SAMPLE_MS);
        if ema > SLOW_FRAME_TIME_MS {
            self.slow_evidence_ms =
                (self.slow_evidence_ms + evidence).min(SLOW_EVIDENCE_REQUIRED_MS);
            self.fast_evidence_ms = 0.0;
            if self.slow_evidence_ms >= SLOW_EVIDENCE_REQUIRED_MS
                && self.quality_scale > MIN_ADAPTIVE_QUALITY_SCALE
            {
                self.quality_scale =
                    (self.quality_scale * DOWNSCALE_FACTOR).max(MIN_ADAPTIVE_QUALITY_SCALE);
                self.cooldown_remaining_ms = DOWNSCALE_COOLDOWN_MS;
                self.clear_evidence();
                return true;
            }
        } else if ema < FAST_FRAME_TIME_MS {
            self.fast_evidence_ms =
                (self.fast_evidence_ms + evidence).min(FAST_EVIDENCE_REQUIRED_MS);
            self.slow_evidence_ms = 0.0;
            if self.fast_evidence_ms >= FAST_EVIDENCE_REQUIRED_MS
                && self.quality_scale < MAX_ADAPTIVE_QUALITY_SCALE
            {
                self.quality_scale =
                    (self.quality_scale + UPSCALE_STEP).min(MAX_ADAPTIVE_QUALITY_SCALE);
                self.cooldown_remaining_ms = UPSCALE_COOLDOWN_MS;
                self.clear_evidence();
                return true;
            }
        } else {
            self.clear_evidence();
        }

        false
    }

    /// Clears timing history after a pause, resize, or renderer switch while retaining the
    /// quality level already learned for this graphics device.
    pub fn reset_timing(&mut self) {
        self.ema_frame_time_ms = None;
        self.last_timestamp_ms = None;
        self.cooldown_remaining_ms = 0.0;
        self.clear_evidence();
    }

    fn clear_evidence(&mut self) {
        self.slow_evidence_ms = 0.0;
        self.fast_evidence_ms = 0.0;
    }
}

fn is_valid_frame_time(frame_time_ms: f64) -> bool {
    frame_time_ms.is_finite() && frame_time_ms > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIXTY_FPS_FRAME_MS: f64 = 1_000.0 / 60.0;

    fn observe_repeatedly(
        quality: &mut AdaptiveFramebufferQuality,
        frame_time_ms: f64,
        sample_count: usize,
    ) -> usize {
        (0..sample_count)
            .filter(|_| quality.observe_frame_time_ms(frame_time_ms))
            .count()
    }

    #[test]
    fn ordinary_desktop_view_remains_native_resolution() {
        assert_eq!(framebuffer_scale(1440, 900, 1.0), 1.0);
    }

    #[test]
    fn high_dpi_and_4k_views_respect_the_pixel_budget() {
        for (width, height, ratio) in [(1440, 900, 2.0), (3840, 2160, 2.0)] {
            let scale = framebuffer_scale(width, height, ratio);
            let pixels = f64::from(width) * f64::from(height) * scale * scale;
            assert!(pixels <= MAX_FRAMEBUFFER_PIXELS * 1.000_001);
            assert!(scale <= MAX_DEVICE_PIXEL_RATIO);
        }
    }

    #[test]
    fn malformed_ratios_and_dimensions_are_bounded() {
        assert_eq!(framebuffer_scale(0, 0, f64::NAN), 1.0);
        assert_eq!(framebuffer_scale(u32::MAX, u32::MAX, f64::INFINITY), 0.25);
    }

    #[test]
    fn adaptive_quality_starts_at_the_static_pixel_budget() {
        let quality = AdaptiveFramebufferQuality::new();
        assert_eq!(quality.quality_scale(), 1.0);
        assert_eq!(quality.ema_frame_time_ms(), None);
        assert_eq!(
            quality.effective_framebuffer_scale(1440, 900, 1.0),
            framebuffer_scale(1440, 900, 1.0)
        );
    }

    #[test]
    fn sustained_slow_frames_reduce_quality_to_a_safe_floor() {
        let mut quality = AdaptiveFramebufferQuality::new();
        let changes = observe_repeatedly(&mut quality, 40.0, 600);

        assert!(changes >= 2);
        assert_eq!(quality.quality_scale(), MIN_ADAPTIVE_QUALITY_SCALE);
        assert_eq!(
            quality.effective_framebuffer_scale(1440, 900, 1.0),
            MIN_ADAPTIVE_QUALITY_SCALE
        );
        assert_eq!(
            quality.effective_framebuffer_scale(u32::MAX, u32::MAX, 2.0),
            MIN_FRAMEBUFFER_SCALE
        );
    }

    #[test]
    fn stable_sixty_fps_cautiously_restores_full_quality() {
        let mut quality = AdaptiveFramebufferQuality::new();
        observe_repeatedly(&mut quality, 40.0, 600);
        assert_eq!(quality.quality_scale(), MIN_ADAPTIVE_QUALITY_SCALE);

        quality.reset_timing();
        let immediate_change = observe_repeatedly(&mut quality, SIXTY_FPS_FRAME_MS, 30);
        assert_eq!(immediate_change, 0);
        assert_eq!(quality.quality_scale(), MIN_ADAPTIVE_QUALITY_SCALE);

        let changes = observe_repeatedly(&mut quality, SIXTY_FPS_FRAME_MS, 2_000);
        assert!(changes >= 2);
        assert_eq!(quality.quality_scale(), MAX_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn neutral_frame_times_do_not_chatter_between_scales() {
        let mut quality = AdaptiveFramebufferQuality::new();
        while !quality.observe_frame_time_ms(40.0) {}
        let reduced_scale = quality.quality_scale();
        quality.reset_timing();

        assert_eq!(observe_repeatedly(&mut quality, 18.5, 2_000), 0);
        assert_eq!(quality.quality_scale(), reduced_scale);
    }

    #[test]
    fn one_long_frame_does_not_reduce_quality() {
        let mut quality = AdaptiveFramebufferQuality::new();
        observe_repeatedly(&mut quality, SIXTY_FPS_FRAME_MS, 60);

        assert!(!quality.observe_frame_time_ms(200.0));
        assert_eq!(observe_repeatedly(&mut quality, SIXTY_FPS_FRAME_MS, 120), 0);
        assert_eq!(quality.quality_scale(), MAX_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn downscale_cooldown_prevents_back_to_back_resizes() {
        let mut quality = AdaptiveFramebufferQuality::new();
        while !quality.observe_frame_time_ms(40.0) {}
        let first_reduction = quality.quality_scale();

        assert_eq!(observe_repeatedly(&mut quality, 40.0, 6), 0);
        assert_eq!(quality.quality_scale(), first_reduction);
        assert!(observe_repeatedly(&mut quality, 40.0, 24) >= 1);
        assert!(quality.quality_scale() < first_reduction);
    }

    #[test]
    fn invalid_duration_samples_are_ignored() {
        let mut quality = AdaptiveFramebufferQuality::new();
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0] {
            assert!(!quality.observe_frame_time_ms(invalid));
        }
        assert_eq!(quality, AdaptiveFramebufferQuality::new());
    }

    #[test]
    fn long_finite_duration_is_accepted_with_bounded_influence() {
        let mut quality = AdaptiveFramebufferQuality::new();

        assert!(!quality.observe_frame_time_ms(1_500.0));

        let ema = quality.ema_frame_time_ms().expect("long frame updates EMA");
        let expected_ema = TARGET_FRAME_TIME_MS
            + (MAX_EMA_FRAME_TIME_MS - TARGET_FRAME_TIME_MS) * FRAME_TIME_EMA_ALPHA;
        assert!((ema - expected_ema).abs() < f64::EPSILON);
        assert_eq!(quality.slow_evidence_ms, MAX_EVIDENCE_PER_SAMPLE_MS);
        assert_eq!(quality.quality_scale(), MAX_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn increasing_long_foreground_timestamps_reduce_quality() {
        let mut quality = AdaptiveFramebufferQuality::new();
        assert!(!quality.observe_frame_timestamp_ms(1_000.0));

        let changes = (1..=40)
            .filter(|frame_index| {
                quality.observe_frame_timestamp_ms(1_000.0 + 1_500.0 * f64::from(*frame_index))
            })
            .count();

        assert!(changes >= 2);
        assert_eq!(quality.quality_scale(), MIN_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn timestamps_ignore_invalid_order() {
        let mut quality = AdaptiveFramebufferQuality::new();
        assert!(!quality.observe_frame_timestamp_ms(1_000.0));
        for invalid in [f64::NAN, f64::INFINITY, 999.0, 1_000.0] {
            assert!(!quality.observe_frame_timestamp_ms(invalid));
        }
        assert_eq!(quality.ema_frame_time_ms(), None);

        assert!(!quality.observe_frame_timestamp_ms(1_000.0 + SIXTY_FPS_FRAME_MS));
        assert!(quality.ema_frame_time_ms().is_some());
        assert_eq!(quality.quality_scale(), MAX_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn reset_timing_excludes_a_visibility_gap() {
        let mut quality = AdaptiveFramebufferQuality::new();
        assert!(!quality.observe_frame_timestamp_ms(1_000.0));

        quality.reset_timing();

        assert!(!quality.observe_frame_timestamp_ms(500_000.0));
        assert_eq!(quality.ema_frame_time_ms(), None);
        assert_eq!(quality.slow_evidence_ms, 0.0);
        assert_eq!(quality.quality_scale(), MAX_ADAPTIVE_QUALITY_SCALE);
    }

    #[test]
    fn resetting_timing_preserves_learned_quality_only() {
        let mut quality = AdaptiveFramebufferQuality::new();
        while !quality.observe_frame_time_ms(40.0) {}
        let reduced_scale = quality.quality_scale();
        assert!(quality.ema_frame_time_ms().is_some());

        quality.reset_timing();

        assert_eq!(quality.quality_scale(), reduced_scale);
        assert_eq!(quality.ema_frame_time_ms(), None);
        assert_eq!(quality.last_timestamp_ms, None);
        assert_eq!(quality.cooldown_remaining_ms, 0.0);
        assert_eq!(quality.slow_evidence_ms, 0.0);
        assert_eq!(quality.fast_evidence_ms, 0.0);
    }

    #[test]
    fn identical_sample_streams_produce_identical_decisions() {
        let mut first = AdaptiveFramebufferQuality::new();
        let mut second = AdaptiveFramebufferQuality::new();
        let samples = [16.7, 18.5, 22.0, 40.0, 16.6, 200.0, 14.0, 33.0];

        for frame_index in 0..2_000 {
            let frame_time_ms = samples[frame_index % samples.len()];
            assert_eq!(
                first.observe_frame_time_ms(frame_time_ms),
                second.observe_frame_time_ms(frame_time_ms)
            );
        }

        assert_eq!(first, second);
    }
}
