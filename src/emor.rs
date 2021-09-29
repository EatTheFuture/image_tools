use crate::exposure_mapping::ExposureMapping;
use crate::utils::lerp_slice;

// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

const EMOR_FACTOR_COUNT: usize = 4;

#[allow(dead_code)]
pub fn eval_emor(factors: &[f32], x: f32) -> f32 {
    let mut y = x + lerp_slice(&EMOR_TABLE[1], x);
    for i in 0..factors.len() {
        y += lerp_slice(&EMOR_TABLE[i + 2], x) * factors[i];
    }
    y
}

#[allow(dead_code)]
pub fn eval_inv_emor(factors: &[f32], x: f32) -> f32 {
    let mut y = x + lerp_slice(&INV_EMOR_TABLE[1], x);
    for i in 0..factors.len() {
        y += lerp_slice(&INV_EMOR_TABLE[i + 2], x) * factors[i];
    }
    y
}

pub fn estimate_inv_emor(mappings: &[ExposureMapping]) -> [f32; EMOR_FACTOR_COUNT] {
    pub fn calc_error(mappings: &[ExposureMapping], emor_factors: &[f32]) -> f32 {
        let mut err_sum = 0.0f32;
        for mapping in mappings {
            let target_curve = |x: f32| (x * mapping.exposure_ratio).min(1.0);
            for (x, y) in mapping.curve.iter().copied() {
                let x_inv = eval_inv_emor(emor_factors, x);
                let y_inv = eval_inv_emor(emor_factors, y);
                let err = (y_inv - target_curve(x_inv)).abs();
                err_sum += err * err;
            }
        }

        err_sum
    }

    let mut factors = [0.0f32; EMOR_FACTOR_COUNT];
    let mut test_factors = [0.0f32; EMOR_FACTOR_COUNT];
    let mut err = calc_error(mappings, &factors);
    for _ in 0..4 {
        for i in 0..EMOR_FACTOR_COUNT {
            let increment_res = 512usize;
            let increment = 1.0 / (increment_res - 1) as f32;
            for n in 0..increment_res {
                test_factors[i] = ((n as f32 * increment) - 0.5) * 8.0;
                let new_err = calc_error(mappings, &test_factors);
                if new_err < err {
                    factors = test_factors;
                    err = new_err;
                } else {
                    test_factors = factors;
                }
            }
        }
    }

    factors
}

pub fn inv_emor_factors_to_curve(factors: &[f32]) -> Vec<f32> {
    let mut curve: Vec<_> = INV_EMOR_TABLE[0]
        .iter()
        .zip(INV_EMOR_TABLE[1].iter())
        .map(|(a, b)| *a + *b)
        .collect();

    for fac_i in 0..factors.len() {
        let factor = factors[fac_i];
        let table = INV_EMOR_TABLE[fac_i + 2];
        for i in 0..table.len() {
            curve[i] += table[i] * factor;
        }
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
