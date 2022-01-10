use randomize::{formulas::f32_half_open_right, PCG32};
use rayon::prelude::*;

use crate::exposure_mapping::ExposureMapping;
use crate::utils::lerp_slice;

// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

const EMOR_FACTOR_COUNT: usize = 6;

pub struct EmorEstimator<'a> {
    mappings: &'a [ExposureMapping],
    factors: [f32; EMOR_FACTOR_COUNT],
    err: f32,
    best_factors: [f32; EMOR_FACTOR_COUNT],
    best_err: f32,
    current_round: usize,
    rounds_without_change: usize,
    step_size: f32,
    rand: PCG32,
}

impl<'a> EmorEstimator<'a> {
    pub fn new(mappings: &'a [ExposureMapping]) -> EmorEstimator<'a> {
        let initial_factors = [0.5f32; EMOR_FACTOR_COUNT];
        let initial_err = calc_emor_error(mappings, &initial_factors);
        EmorEstimator {
            mappings: mappings,
            factors: initial_factors,
            err: initial_err,
            best_factors: initial_factors,
            best_err: initial_err,
            current_round: 0,
            rounds_without_change: 0,
            step_size: 1.0,
            rand: PCG32::seed(0xdd60c3b293895214, 0xc16fa8cdc70cc1c3),
        }
    }

    fn rand_0_1(&mut self) -> f32 {
        f32_half_open_right(self.rand.next_u32())
    }

    fn rand_bool(&mut self) -> bool {
        self.rand.next_u32() & 1 == 0
    }

    pub fn do_rounds(&mut self, rounds: usize) {
        // Use gradient descent to find the lowest error.
        for _ in self.current_round..(self.current_round + rounds) {
            self.current_round += 1;
            let delta = 0.01 * self.step_size;
            let delta_inv = 1.0 / delta;
            let mut error_diffs = [0.0f32; EMOR_FACTOR_COUNT];
            for i in 0..EMOR_FACTOR_COUNT {
                let neg = self.rand_bool();
                let delta = if neg { -delta } else { delta };
                let delta_inv = if neg { -delta_inv } else { delta_inv };
                let mut test_factors = self.factors;
                test_factors[i] += delta;
                error_diffs[i] = (calc_emor_error(self.mappings, &test_factors) - self.err)
                    * delta_inv
                    * self.rand_0_1();
            }

            let mut diff_length = error_diffs.iter().fold(0.0f32, |a, b| a + (b * b)).sqrt();

            // Jostle it a bit if seems to be stuck.
            if !(diff_length > 0.0) {
                for i in 0..EMOR_FACTOR_COUNT {
                    error_diffs[i] = self.rand_0_1() * delta;
                }
                diff_length = error_diffs.iter().fold(0.0f32, |a, b| a + (b * b)).sqrt();
            }

            if diff_length > 0.0 {
                let diff_norm = 1.0 / diff_length;
                for i in 0..EMOR_FACTOR_COUNT {
                    self.factors[i] -= error_diffs[i] * diff_norm * self.step_size;
                }
                self.err = calc_emor_error(self.mappings, &self.factors);

                if self.err < self.best_err {
                    self.best_err = self.err;
                    self.best_factors = self.factors;
                    self.rounds_without_change = 0;
                } else {
                    self.rounds_without_change += 1;
                }
            } else {
                break;
            }

            if self.rounds_without_change >= 64 {
                self.step_size *= 0.9090909;
                self.rounds_without_change = 0;
                self.factors = self.best_factors;
                self.err = self.best_err;
            }
        }
    }

    pub fn current_estimate(&self) -> ([f32; EMOR_FACTOR_COUNT], f32) {
        (self.best_factors, self.best_err)
    }
}

// pub fn emor_at_index(factors: &[f32], i: usize) -> f32 {
//     let mut y = EMOR_TABLE[0][i] + EMOR_TABLE[1][i];
//     for f in 0..factors.len() {
//         y += EMOR_TABLE[f + 2][i] * factors[f];
//     }
//     y
// }

// pub fn eval_emor(factors: &[f32], x: f32) -> f32 {
//     let mut y = x + lerp_slice(&EMOR_TABLE[1], x);
//     for f in 0..factors.len() {
//         y += lerp_slice(&EMOR_TABLE[f + 2], x) * factors[f];
//     }
//     y
// }

pub fn inv_emor_at_index(factors: &[f32], i: usize) -> f32 {
    let mut y = INV_EMOR_TABLE[0][i] + INV_EMOR_TABLE[1][i];
    for f in 0..factors.len() {
        y += INV_EMOR_TABLE[f + 2][i] * factors[f];
    }
    y
}

pub fn eval_inv_emor(factors: &[f32], x: f32) -> f32 {
    let mut y = x + lerp_slice(&INV_EMOR_TABLE[1], x);
    for f in 0..factors.len() {
        y += lerp_slice(&INV_EMOR_TABLE[f + 2], x) * factors[f];
    }
    y
}

/// Estimates inverse EMoR factors to fit the passed mappings.
///
/// Returns the inverse EMoR factors and the average error of the fit.
pub fn estimate_inv_emor(mappings: &[ExposureMapping]) -> ([f32; EMOR_FACTOR_COUNT], f32) {
    let mut estimator = EmorEstimator::new(mappings);
    estimator.do_rounds(4000);
    estimator.current_estimate()
}

pub fn inv_emor_factors_to_curve(
    factors: &[f32],
    sensor_floor: f32,
    sensor_ceiling: f32,
) -> Vec<f32> {
    let resolution = INV_EMOR_TABLE[0].len();
    let step = 1.0 / (resolution - 1) as f32;

    // Compute floor/ceiling factors.
    let inv_floor = eval_inv_emor(factors, sensor_floor);
    let inv_ceil = eval_inv_emor(factors, sensor_ceiling);
    let norm = 1.0 / (inv_ceil - inv_floor);

    // Build the curve.
    let mut curve = vec![0.0f32; resolution];
    for i in 0..resolution {
        curve[i] = (eval_inv_emor(factors, i as f32 * step) - inv_floor) * norm;
    }

    // Ensure monotonicity.
    let min_diff = 0.005 / curve.len() as f32;
    for i in 1..curve.len() {
        if (curve[i] - curve[i - 1]) < min_diff {
            curve[i] = curve[i - 1] + min_diff;
        }
    }

    curve
}

fn calc_emor_error(mappings: &[ExposureMapping], emor_factors: &[f32]) -> f32 {
    let mut err_sum = 0.0f32;
    let mut err_weight_sum = 0.0f32;

    let point_count: usize = mappings.iter().map(|m| m.curve.len()).sum();

    // Compute the curve.
    let transfer_curve: Vec<f32> = (0..INV_EMOR_TABLE[0].len())
        .map(|i| inv_emor_at_index(emor_factors, i))
        .collect();

    // Discourage non-monotonic curves by strongly encouraging a minimum slope.
    const MIN_SLOPE: f32 = 1.0 / (1 << 16) as f32;
    const MIN_DELTA: f32 = MIN_SLOPE / INV_EMOR_TABLE[0].len() as f32;
    let total_error_measurements = mappings.len() * point_count;
    let non_mono_weight =
        (4096.0 / INV_EMOR_TABLE[0].len() as f32) * total_error_measurements as f32;
    let mut last_y = -MIN_DELTA;
    for y in transfer_curve.iter().copied() {
        let non_mono = (last_y - y + MIN_DELTA).max(0.0);
        last_y = y;
        err_sum += non_mono * non_mono_weight;
    }

    // Calculate the actual errors.
    let (err, err_weight) = mappings
        .par_iter()
        .map(|mapping| {
            let mut mapping_err = 0.0;
            let mut mapping_err_weight = 0.0;

            // Compute floor/ceiling adjustments.
            let inv_floor = lerp_slice(&transfer_curve, mapping.floor);
            let inv_ceil = lerp_slice(&transfer_curve, mapping.ceiling);
            let inv_floor_ceil_norm = 1.0 / (inv_ceil - inv_floor);
            let floor_ceil_norm = 1.0 / (mapping.ceiling - mapping.floor);

            // Compute the overall mapping weight.  We favor mappings with
            // and exposure ratio close to 2.0.
            let mapping_weight = if mapping.exposure_ratio < 2.0 {
                let x = mapping.exposure_ratio - 1.0;
                x * x * (3.0 - 2.0 * x)
            } else {
                let x = mapping.exposure_ratio - 2.0;
                1.0 / (0.5 * x * x + 1.0)
            };

            // Compute error for each point in the mapping.
            if mapping_weight > 0.0 {
                // Helper function.
                let eval = |n: f32| -> f32 {
                    (lerp_slice(&transfer_curve, n) - inv_floor) * inv_floor_ceil_norm
                };

                for i in 0..mapping.curve.len() {
                    let (x, y) = mapping.curve[i];
                    if x.min(y) <= mapping.floor || y.max(x) >= mapping.ceiling {
                        continue;
                    }

                    let point_weight = {
                        let n = (y - mapping.floor) * floor_ceil_norm;
                        let mut tmp = (2.0 * n) - 1.0;
                        for _ in 0..2 {
                            tmp = tmp * tmp;
                        }
                        1.0 - tmp
                    };

                    // Compute "linear" x and y with our current estimated inverse EMoR curves.
                    let x_linear = eval(x);
                    let y_linear = eval(y);

                    // Compute error.
                    let err = (((x_linear / y_linear) * mapping.exposure_ratio) - 1.0).abs();
                    mapping_err += err * point_weight * mapping_weight;
                    mapping_err_weight += mapping_weight;
                }
            }

            (mapping_err, mapping_err_weight)
        })
        .reduce(|| (0.0f32, 0.0f32), |a, b| (a.0 + b.0, a.1 + b.1));

    err_sum += err;
    err_weight_sum += err_weight;

    err_sum / err_weight_sum as f32
}
