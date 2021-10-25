use crate::exposure_mapping::ExposureMapping;
use crate::utils::lerp_slice;

// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

const EMOR_FACTOR_COUNT: usize = 5;

pub fn emor_at_index(factors: &[f32], i: usize) -> f32 {
    eval_emor(factors, (i as f32 * (1.0 / 1023.0)).min(1.0))
}

pub fn eval_emor(factors: &[f32], x: f32) -> f32 {
    // let mut yar = [0.0f32; EMOR_FACTOR_COUNT + 2];
    // yar[0] = 0.0;
    // yar[EMOR_FACTOR_COUNT + 1] = 1.0;
    // for i in 0..factors.len() {
    //     yar[i + 1] = factors[i];
    // }
    // return lerp_slice(&yar, x);

    let mut y = x + lerp_slice(&EMOR_TABLE[1], x);
    for i in 0..factors.len() {
        y += lerp_slice(&EMOR_TABLE[i + 2], x) * factors[i];
    }
    y
}

/// Estimates EMoR factors to fit the passed mappings.
///
/// Returns the EMoR factors and the average error of the fit.
pub fn estimate_emor(mappings: &[ExposureMapping]) -> ([f32; EMOR_FACTOR_COUNT], f32) {
    let calc_error = |mappings: &[ExposureMapping], emor_factors: &[f32]| -> f32 {
        const POINTS: usize = 200;
        let mut err_sum = 0.0f32;
        let mut err_weight_sum = 0.0f32;

        // Discourage non-monotonic curves by strongly encouraging a minimum slope.
        const MIN_SLOPE: f32 = 1.0 / 1024.0;
        const MIN_DELTA: f32 = MIN_SLOPE / EMOR_TABLE[0].len() as f32;
        let non_mono_weight =
            4.0 * mappings.len() as f32 * POINTS as f32 * (1.0 / EMOR_TABLE[0].len() as f32);
        let mut last_y = -MIN_DELTA;
        for i in 0..EMOR_TABLE[0].len() {
            let y = emor_at_index(emor_factors, i);
            let non_mono = (last_y - y + MIN_DELTA).max(0.0);
            last_y = y;
            err_sum += non_mono * non_mono_weight;
            err_weight_sum += non_mono_weight;
        }

        // Calculate the actual errors.
        for mapping in mappings {
            let sensor_range = mapping.ceiling - mapping.floor;
            let map_floor_ceil = |n: f32| -> f32 { n * sensor_range + mapping.floor };
            let relative_err = |a: f32, b: f32| -> f32 {
                let x = a.min(b);
                let y = a.max(b);
                let err = if y > 0.0 { (y - x) / y } else { 0.0 };
                err * err
            };

            let weight = {
                const MIN_EXTENT: f32 = 0.5;
                let y_extent = (mapping.curve[0].1 - mapping.curve.last().unwrap().1).abs();
                let extent_weight = {
                    let adjusted_extent = (y_extent - MIN_EXTENT).max(0.0) / (1.0 - MIN_EXTENT);
                    adjusted_extent.abs() // * adjusted_extent
                };
                let sample_count_weight = mapping.curve.len() as f32 / 256.0;
                sample_count_weight * extent_weight
            };
            if weight > 0.0 {
                for i in 0..POINTS {
                    let y_linear = (i + 1) as f32 / (POINTS + 1) as f32;
                    let x_linear = y_linear / mapping.exposure_ratio;
                    let x = map_floor_ceil(eval_emor(emor_factors, x_linear));
                    let y = map_floor_ceil(eval_emor(emor_factors, y_linear));

                    if let Some(x_err) = mapping.eval_at_y(y).map(|x_map| relative_err(x, x_map)) {
                        err_sum += x_err * weight;
                        err_weight_sum += weight;
                    }
                    if let Some(y_err) = mapping.eval_at_x(x).map(|y_map| relative_err(y, y_map)) {
                        err_sum += y_err * weight;
                        err_weight_sum += weight;
                    }
                }
            }
        }

        err_sum / err_weight_sum as f32
    };

    // Use gradient descent to find the lowest error.
    let mut factors = [0.5f32; EMOR_FACTOR_COUNT];
    let mut err = calc_error(mappings, &factors);
    let mut best_factors = factors;
    let mut best_err = err;
    const ROUNDS: usize = 2000;
    const WARMUP: usize = 100;
    for round in 0..ROUNDS {
        let delta = 0.0001;
        let delta_inv = 1.0 / delta;
        let mut error_diffs = [0.0f32; EMOR_FACTOR_COUNT];
        for i in 0..EMOR_FACTOR_COUNT {
            let mut test_factors = factors;
            test_factors[i] += delta;
            error_diffs[i] = (calc_error(mappings, &test_factors) - err) * delta_inv;
        }

        let diff_length = error_diffs.iter().fold(0.0f32, |a, b| a + (b * b)).sqrt();

        if diff_length > 0.0 {
            let diff_norm = 1.0 / diff_length;
            let step = if round < WARMUP {
                0.1
            } else {
                0.1 / (0.1 * (round - WARMUP) as f32 + 1.0)
            };
            for i in 0..EMOR_FACTOR_COUNT {
                factors[i] -= error_diffs[i] * diff_norm * step;
            }
            err = calc_error(mappings, &factors);

            if err < best_err {
                best_err = err;
                best_factors = factors;
            }
        } else {
            break;
        }
    }

    (best_factors, best_err)
}

pub fn emor_factors_to_curve(factors: &[f32], sensor_floor: f32, sensor_ceiling: f32) -> Vec<f32> {
    let sensor_range = sensor_ceiling - sensor_floor;
    let map_floor_ceil = |n: f32| -> f32 { n * sensor_range + sensor_floor };
    let resolution = EMOR_TABLE[0].len();
    let step = 1.0 / (resolution - 1) as f32;

    let mut curve = vec![0.0f32; resolution];
    for i in 0..resolution {
        curve[i] = map_floor_ceil(eval_emor(factors, i as f32 * step));
    }

    // Ensure monotonicity.
    let min_diff = 0.005 / curve.len() as f32;
    for i in 1..curve.len() {
        if (curve[i] - curve[i - 1]) < min_diff {
            curve[i] = curve[i - 1] + min_diff;
        }
    }

    // Ensure they're in the range [0, 1].
    for n in curve.iter_mut() {
        *n = n.max(0.0).min(1.0);
    }

    curve
}
