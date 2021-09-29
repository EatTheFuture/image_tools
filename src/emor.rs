use crate::exposure_mapping::ExposureMapping;
use crate::utils::lerp_slice;

// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

const EMOR_FACTOR_COUNT: usize = 4;

pub fn eval_emor(factors: &[f32], x: f32) -> f32 {
    let mut y = x + lerp_slice(&EMOR_TABLE[1], x);
    for i in 0..factors.len() {
        y += lerp_slice(&EMOR_TABLE[i + 2], x) * factors[i];
    }
    y
}

pub fn estimate_emor(mappings: &[ExposureMapping]) -> ([f32; EMOR_FACTOR_COUNT], f32) {
    pub fn calc_error(mappings: &[ExposureMapping], emor_factors: &[f32]) -> f32 {
        const POINTS: usize = 32;
        let mut err_sum = 0.0f32;
        let mut err_count = 0usize;

        // Heavily discourage curves with a range outside of [0.0, 1.0].
        for i in 0..POINTS {
            let x = (i + 1) as f32 / (POINTS + 1) as f32;
            let y = eval_emor(emor_factors, x);
            if y < 0.0 || y > 1.0 {
                err_sum += 1.0 * mappings.len() as f32;
            }
        }

        // Calculate the actual errors.
        for mapping in mappings {
            for i in 0..POINTS {
                let y_linear = (i + 1) as f32 / (POINTS + 1) as f32;
                let x_linear = y_linear / mapping.exposure_ratio;
                let x = eval_emor(emor_factors, x_linear);
                let y = eval_emor(emor_factors, y_linear);

                if let Some(map_y) = mapping.eval_at_x(x) {
                    let err = (y - map_y).abs();
                    err_sum += err * err;
                    err_count += 1;
                }
            }
        }

        err_sum / err_count as f32
    }

    let mut factors = [0.0f32; EMOR_FACTOR_COUNT];
    let mut test_factors = [0.0f32; EMOR_FACTOR_COUNT];
    let mut err = calc_error(mappings, &factors);
    for _ in 0..4 {
        for i in 0..EMOR_FACTOR_COUNT {
            let increment_res = 256usize;
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

    (factors, err)
}

pub fn emor_factors_to_curve(factors: &[f32]) -> Vec<f32> {
    let mut curve: Vec<_> = EMOR_TABLE[0]
        .iter()
        .zip(EMOR_TABLE[1].iter())
        .map(|(a, b)| *a + *b)
        .collect();

    for fac_i in 0..factors.len() {
        let factor = factors[fac_i];
        let table = EMOR_TABLE[fac_i + 2];
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
