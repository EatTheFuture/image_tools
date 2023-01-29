/// A simple filmic tonemapping curve.
///
/// The basic idea behind this is to apply an s-curve on top of a simple
/// Reinhard tone mapping function.  In practice this produces pleasing
/// results, but it has no real basis in the actual physics of film stock.
///
/// - `x`: the input value.
/// - `fixed_point`: the value of `x` that should map to itself.  For
///   example, you might set this to 0.18 (18% gray) so that colors of
///   that brightness remain the same.
/// - `luminance_ceiling`: the luminance level that maps to 1.0 in the
///   output.  Typically you want this to be a large-ish number
///   (e.g. > 30), as it represents the top end of the dynamic range.
///   It can be useful to think in terms of photographic stops: if you
///   want 6 stops of dynamic range above 1.0, then this should be 2^6,
///   or 64.
/// - `toe_sharpness`: how sharp the toe is.  Reasonable values are in
///   [1.0, 2.0]
/// - `shoulder_sharpness`: how sharp the shoulder is.  Reasonable values
///   are in [0.5, 2.0].
///
/// Returns the tonemapped value, always in the range [0.0, 1.0].
pub fn filmic_curve(
    x: f64,
    fixed_point: f64,
    luminance_ceiling: f64,
    toe_sharpness: f64,
    shoulder_sharpness: f64,
) -> f64 {
    let fwd = |n: f64| {
        let n = 1.0 - (1.0 / (1.0 + n));
        s_curve(n, fixed_point, toe_sharpness, shoulder_sharpness)
    };
    let inv = |n: f64| {
        let n = s_curve_inv(n, fixed_point, toe_sharpness, shoulder_sharpness);
        (1.0 / (1.0 - n)) - 1.0
    };

    let scale_x = inv(fixed_point) / fixed_point;
    let scale_y = 1.0 / fwd(luminance_ceiling * scale_x);

    fwd(x * scale_x) * scale_y
}

pub fn filmic_curve_inv(
    x: f64,
    fixed_point: f64,
    luminance_ceiling: f64,
    toe_sharpness: f64,
    shoulder_sharpness: f64,
) -> f64 {
    let fwd = |n: f64| {
        let n = 1.0 - (1.0 / (1.0 + n));
        s_curve(n, fixed_point, toe_sharpness, shoulder_sharpness)
    };
    let inv = |n: f64| {
        let n = s_curve_inv(n, fixed_point, toe_sharpness, shoulder_sharpness);
        (1.0 / (1.0 - n)) - 1.0
    };

    let scale_x = inv(fixed_point) / fixed_point;
    let scale_y = 1.0 / fwd(luminance_ceiling * scale_x);

    inv(x / scale_y) / scale_x
}

/// A tweakable sigmoid function that maps [0.0, 1.0] to [0.0, 1.0].
///
/// - `transition`: the value of `x` where the toe transitions to the shoulder.
/// - `toe_exp`: the exponent used for the toe part of the curve.
///   1.0 = linear, 2.0 = quadratic, etc.
/// - `shoulder_exp`: the exponent used for the shoulder part of the curve.
fn s_curve(x: f64, transition: f64, toe_exp: f64, shoulder_exp: f64) -> f64 {
    // Early-out for off-the-end values.
    if x <= 0.0 {
        return 0.0;
    } else if x >= 1.0 {
        return 1.0;
    }

    // Toe and shoulder curve functions.
    let toe = |x: f64, scale: f64| -> f64 { x.powf(toe_exp) * scale };
    let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

    // Toe and shoulder slopes at the transition.
    let toe_slope = toe_exp * transition.powf(toe_exp - 1.0);
    let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

    // Vertical scale factors needed to make the toe and shoulder meet
    // at the transition with equal slopes.
    let s1 = shoulder_slope / toe_slope;
    let s2 = 1.0 / (1.0 + toe(transition, s1) - shoulder(transition, 1.0));

    // The full curve output.
    if x < transition {
        toe(x, s1 * s2)
    } else {
        shoulder(x, s2)
    }
    .clamp(0.0, 1.0)
}

/// Inverse of `s_curve()`.
fn s_curve_inv(x: f64, transition: f64, toe_exp: f64, shoulder_exp: f64) -> f64 {
    // Early-out for off-the-end values.
    if x <= 0.0 {
        return 0.0;
    } else if x >= 1.0 {
        return 1.0;
    }

    // Toe and shoulder curve functions.
    let toe = |x: f64, scale: f64| -> f64 { x.powf(toe_exp) * scale };
    let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

    // Toe and shoulder slopes at the transition.
    let toe_slope = toe_exp * transition.powf(toe_exp - 1.0);
    let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

    // Vertical scale factors needed to make the toe and shoulder meet
    // at the transition with equal slopes.
    let s1 = shoulder_slope / toe_slope;
    let s2 = 1.0 / (1.0 + toe(transition, s1) - shoulder(transition, 1.0));

    //-------------------------

    let transition_inv = toe(transition, s1 * s2);

    let toe_inv = |x: f64, scale: f64| -> f64 {
        // x.powf(toe_exp) * scale
        (x / scale).powf(1.0 / toe_exp)
    };
    let shoulder_inv = |x: f64, scale: f64| -> f64 {
        // 1.0 - ((1.0 - x).powf(shoulder_exp) * scale)
        1.0 - ((1.0 - x) / scale).powf(1.0 / shoulder_exp)
    };

    // The full curve output.
    if x < transition_inv {
        toe_inv(x, s1 * s2)
    } else {
        shoulder_inv(x, s2)
    }
    .clamp(0.0, 1.0)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn s_curve_round_trip() {
        for i in 0..17 {
            let t = 0.25;
            let p1 = 2.3;
            let p2 = 4.5;

            let x = i as f64 / 16.0;
            let x2 = s_curve_inv(s_curve(x, t, p1, p2), t, p1, p2);
            assert!((x - x2).abs() < 0.000_000_1);
        }
    }
}
