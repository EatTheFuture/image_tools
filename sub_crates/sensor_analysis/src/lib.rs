//! A crate for computing various things about camera sensors.

mod emor;
mod exposure_mapping;
mod histogram;
mod utils;

pub mod known_luma_curves;

pub use histogram::Histogram;

use exposure_mapping::ExposureMapping;

/// Uses EMoR curve fitting to estimate a luminance mapping curve that
/// fits the given histogram-exposure pairs.
///
/// The returned curve represents a mapping from the source space in
/// [0.0, 1.0] to a linear space in [0.0, 1.0].  For example, if the
/// input exposure mappings are from sRGB images, the returned map would
/// convert sRGB gamma -> linear.
///
/// Also returns the average error of the fit.
pub fn estimate_luma_map_emor(histograms: &[&[(Histogram, f32)]]) -> (Vec<f32>, f32) {
    let bucket_count = histograms[0][0].0.buckets.len();

    // Build the exposure mappings and also estimate the sensor floor/ceiling.
    let mut mappings = Vec::new();
    let mut sensor_floor = 0;
    let mut sensor_ceiling = bucket_count - 1;
    for chan in 0..histograms.len() {
        for i in 0..(histograms[chan].len() - 1) {
            mappings.push(ExposureMapping::from_histograms(
                &histograms[chan][i].0,
                &histograms[chan][i + 1].0,
                histograms[chan][i].1,
                histograms[chan][i + 1].1,
            ));

            // Floor.
            let ratio = histograms[chan][i + 1].1 / histograms[chan][0].1;
            let tmp_i = ((ratio - 1.0) * 0.5) as usize;
            if tmp_i > 0 && tmp_i < (bucket_count / 4) {
                let target_sum = histograms[chan][i + 1].0.sum_under(tmp_i);
                sensor_floor = sensor_floor.max(histograms[chan][0].0.find_sum(target_sum));
            }

            // Ceiling.
            let ratio = histograms[chan][i].1 / histograms[chan].last().unwrap().1;
            let tmp_i = (bucket_count as f32 * ((31.0 + ratio) * (1.0 / 32.0))) as usize;
            if tmp_i < (bucket_count - 1) && tmp_i > (bucket_count / 4) {
                let target_sum = histograms[chan][i].0.sum_under(tmp_i);
                sensor_ceiling =
                    sensor_ceiling.min(histograms[chan].last().unwrap().0.find_sum(target_sum));
            }
        }
    }
    dbg!((sensor_floor, sensor_ceiling));

    let sensor_floor = sensor_floor as f32 / (histograms[0][0].0.buckets.len() - 1) as f32;
    let sensor_ceiling = sensor_ceiling as f32 / (histograms[0][0].0.buckets.len() - 1) as f32;

    let (emor_factors, err) = emor::estimate_emor(&mappings, sensor_floor, sensor_ceiling);
    (
        emor::emor_factors_to_curve(&emor_factors, sensor_floor, sensor_ceiling),
        err,
    )
}

/// Calculates the inverse of a luminance map.
///
/// Assumes the slice represents a semi-monotonic function in the range
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
