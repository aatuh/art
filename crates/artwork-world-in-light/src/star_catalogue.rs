//! Deterministic, renderer-independent point-star catalogue generation.
//!
//! Each vertex is interleaved as world-space direction `xyz`, linear-light
//! colour `rgb`, and point diameter in CSS pixels. The browser adapter owns the
//! projection and device-pixel scaling; this module has no WebGL dependency.

pub const STAR_COUNT: usize = 1_800;
pub const STAR_VERTEX_STRIDE_FLOATS: usize = 7;
pub const STAR_DIRECTION_OFFSET_FLOATS: usize = 0;
pub const STAR_COLOR_OFFSET_FLOATS: usize = 3;
pub const STAR_SIZE_OFFSET_FLOATS: usize = 6;

pub const MIN_STAR_COLOR: f32 = 0.10;
pub const MAX_STAR_COLOR: f32 = 1.60;
pub const MIN_STAR_SIZE: f32 = 0.72;
pub const MAX_STAR_SIZE: f32 = 3.38;

const CATALOGUE_SEED: u64 = 0x45d9_7c2b_6a13_f081;

/// Builds the gallery's fixed star catalogue.
///
/// The returned allocation contains exactly [`STAR_COUNT`] vertices in the
/// layout described by the offset and stride constants above.
pub fn star_catalogue_vertices() -> Vec<f32> {
    generate_catalogue(CATALOGUE_SEED)
}

fn generate_catalogue(seed: u64) -> Vec<f32> {
    let mut random = SplitMix64::new(seed);
    let mut vertices = Vec::with_capacity(STAR_COUNT * STAR_VERTEX_STRIDE_FLOATS);

    for _ in 0..STAR_COUNT {
        let direction = random.unit_sphere_direction();
        let prominence = random.unit_f32();
        let temperature = random.unit_f32();
        let base_color = stellar_color(temperature);
        let intensity = 0.28 + prominence.powi(7) * 1.32;
        let size = MIN_STAR_SIZE + prominence.powi(9) * 2.54 + random.unit_f32() * 0.12;

        vertices.extend_from_slice(&direction);
        vertices.extend(base_color.map(|channel| channel * intensity));
        vertices.push(size);
    }

    vertices
}

fn stellar_color(temperature: f32) -> [f32; 3] {
    // A compact perceptual approximation: most naked-eye stars are warm or
    // solar-white, with a smaller blue-white population.
    if temperature < 0.68 {
        interpolate([1.00, 0.65, 0.43], [1.00, 0.92, 0.78], temperature / 0.68)
    } else {
        interpolate(
            [1.00, 0.92, 0.78],
            [0.64, 0.79, 1.00],
            (temperature - 0.68) / 0.32,
        )
    }
}

fn interpolate(start: [f32; 3], end: [f32; 3], amount: f32) -> [f32; 3] {
    [
        start[0] + (end[0] - start[0]) * amount,
        start[1] + (end[1] - start[1]) * amount,
        start[2] + (end[2] - start[2]) * amount,
    ]
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn unit_f32(&mut self) -> f32 {
        // Twenty-four significant random bits map exactly into the useful
        // precision of an f32 mantissa and always produce a value below one.
        ((self.next_u64() >> 40) as f32) * (1.0 / 16_777_216.0)
    }

    fn unit_sphere_direction(&mut self) -> [f32; 3] {
        // Marsaglia's rejection method avoids polar clustering and trigonometry.
        for _ in 0..32 {
            let first = self.unit_f32() * 2.0 - 1.0;
            let second = self.unit_f32() * 2.0 - 1.0;
            let radius_squared = first * first + second * second;
            if radius_squared > f32::EPSILON && radius_squared < 1.0 {
                let scale = 2.0 * (1.0 - radius_squared).sqrt();
                return [first * scale, second * scale, 1.0 - 2.0 * radius_squared];
            }
        }

        // SplitMix64 cannot realistically exhaust the attempts, but this keeps
        // catalogue generation strictly bounded for every possible seed.
        [0.0, 1.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_catalogue_is_exactly_reproducible() {
        let first = star_catalogue_vertices();
        let second = star_catalogue_vertices();

        assert_eq!(first, second);
        assert_eq!(first.len(), STAR_COUNT * STAR_VERTEX_STRIDE_FLOATS);
    }

    #[test]
    fn every_direction_is_a_finite_unit_vector() {
        for vertex in star_catalogue_vertices().chunks_exact(STAR_VERTEX_STRIDE_FLOATS) {
            let direction = &vertex[STAR_DIRECTION_OFFSET_FLOATS..STAR_COLOR_OFFSET_FLOATS];
            let length_squared = direction
                .iter()
                .map(|component| component * component)
                .sum::<f32>();

            assert!(direction.iter().all(|component| component.is_finite()));
            assert!((length_squared - 1.0).abs() < 0.000_01, "{direction:?}");
        }
    }

    #[test]
    fn color_and_point_size_stay_within_the_render_contract() {
        for vertex in star_catalogue_vertices().chunks_exact(STAR_VERTEX_STRIDE_FLOATS) {
            let color = &vertex[STAR_COLOR_OFFSET_FLOATS..STAR_SIZE_OFFSET_FLOATS];
            assert!(color.iter().all(|channel| {
                channel.is_finite() && *channel >= MIN_STAR_COLOR && *channel <= MAX_STAR_COLOR
            }));

            let size = vertex[STAR_SIZE_OFFSET_FLOATS];
            assert!(size.is_finite());
            assert!((MIN_STAR_SIZE..=MAX_STAR_SIZE).contains(&size));
        }
    }
}
