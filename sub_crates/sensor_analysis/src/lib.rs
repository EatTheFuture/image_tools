//! A crate for computing various things about camera sensors.

mod emor;
mod exposure_mapping;
mod histogram;
mod utils;

pub use exposure_mapping::ExposureMapping;
pub use histogram::Histogram;

/// Uses EMoR curve fitting to estimate a luminance mapping curve that
/// fits the given exposure mappings.
///
/// The returned curve represents a mapping from the source space in
/// [0.0, 1.0] to a linear space in [0.0, 1.0].  For example, if the
/// input exposure mappings are from sRGB images, the returned map would
/// convert sRGB gamma -> linear.
///
/// Also returns the average error of the fit.
pub fn estimate_luma_map_emor(mappings: &[ExposureMapping]) -> (Vec<f32>, f32) {
    let (emor_factors, err) = emor::estimate_emor(mappings);
    (emor::emor_factors_to_curve(&emor_factors), err)
}

/// Calculates the inverse of a luminance map.
///
/// Assumes the slice represents a monotonic function in the range
/// [0.0, 1.0].
pub fn invert_luma_map(slice: &[f32]) -> Vec<f32> {
    let resolution = slice.len();

    let mut curve = Vec::new();
    let mut prev_x = 0.0;
    let mut prev_y = 0.0;
    for i in 0..slice.len() {
        let x = (i as f32 / (slice.len() - 1) as f32).max(prev_x);
        let y = slice[i].max(prev_y);
        curve.push((x, y));
        prev_x = x;
        prev_y = y;
    }

    let mut flipped = Vec::new();
    let mut prev_x = 0.0;
    for i in 0..resolution {
        let y = i as f32 / (resolution - 1) as f32;
        let x = utils::lerp_curve_at_y(&curve, y).max(prev_x);
        flipped.push(x);
        prev_x = x;
    }

    flipped
}

/// Evaluates the given luma map at `t`.
///
/// `t` should be in the range [0.0, 1.0], and (assuming a valid luma
/// map) the output will also be in [0.0, 1.0] and will be monotonic
/// with `t`.
#[inline]
pub fn eval_luma_map(luma_map: &[f32], t: f32) -> f32 {
    debug_assert!(t >= 0.0 && t <= 1.0);
    utils::lerp_slice(luma_map, t)
}
