/// A simple filmic tonemapping curve.
///
/// The basic idea behind this is to apply an s-curve in log2 space.  In
/// practice this produces pleasing results, but it has no real basis in
/// e.g. the actual physics of film stock.
///
/// - `x`: the input value.
/// - `fixed_point`: the value of `x` that should map to itself.  For
///   example, you might set this to 0.18 (18% gray) so that colors of
///   that brightness remain the same.
/// - `stops_range`: the stops to map to [0.0, 1.0], specified relative
///   to `fixed_point`. `[-16.0, 8.0]` is a reasonable setting.  The
///   upper range value tends to determine how constrasty the filmic look
///   with smaller values producing more constrast.  The lower range also
///   has some impact, but not as much, and can typically be left around -16.
/// - `foot_sharpness`: how sharp the foot is.  Reasonable values are in [0.0, 1.0]
/// - `shoulder_sharpness`: how sharp the shoulder is.  Reasonable values are in [0.0, 1.0].
///
/// Returns the tonemapped value, always in the range [0.0, 1.0].
pub fn filmic_curve(
    x: f64,
    fixed_point: f64,
    stops_range: [f64; 2],
    foot_sharpness: f64,
    shoulder_sharpness: f64,
) -> f64 {
    // Map inputs in an user-friendly way, so that [0.0, 1.0] are reasonable.
    let foot_start = (0.6 - (0.6 * foot_sharpness)).sqrt(); // [0.0, 1.0] -> [~0.77, 0.0]
    let shoulder_sharpness = 3.0 + (5.0 * shoulder_sharpness * 4.0); // [0.0, 1.0] -> [3.0, 8.0]

    let mapper = |n: f64| {
        // Map to [0.0, 1.0] in log2 space, spanning `[stops_below, stops_above]` from the fixed_point.
        let a = fixed_point.log2() + stops_range[0];
        let b = fixed_point.log2() + stops_range[1];
        let lg2 = (n.log2() - a) / (b - a);
        s_curve(lg2, foot_start, 1.0, shoulder_sharpness)
    };

    // Exponent needed to map `fixed_point` back to itself.
    let exp = fixed_point.log2() / mapper(fixed_point).log2();

    mapper(x).powf(exp)
}

pub fn filmic_curve_inv(
    x: f64,
    fixed_point: f64,
    stops_range: [f64; 2],
    foot_sharpness: f64,
    shoulder_sharpness: f64,
) -> f64 {
    // Map inputs in an user-friendly way, so that [0.0, 1.0] are reasonable.
    let foot_start = (0.6 - (0.6 * foot_sharpness)).sqrt(); // [0.0, 1.0] -> [~0.77, 0.0]
    let shoulder_sharpness = 3.0 + (5.0 * shoulder_sharpness * 4.0); // [0.0, 1.0] -> [3.0, 8.0]

    let mapper = |n: f64| {
        // Map to [0.0, 1.0] in log2 space, spanning `[stops_below, stops_above]` from the fixed_point.
        let a = fixed_point.log2() + stops_range[0];
        let b = fixed_point.log2() + stops_range[1];
        let lg2 = (n.log2() - a) / (b - a);
        s_curve(lg2, foot_start, 1.0, shoulder_sharpness)
    };

    // Exponent needed to map `fixed_point` back to itself.
    let exp = fixed_point.log2() / mapper(fixed_point).log2();

    //-------------------------

    let mapper_inv = |n: f64| {
        let a = fixed_point.log2() + stops_range[0];
        let b = fixed_point.log2() + stops_range[1];

        let lg2 = s_curve_inv(n, foot_start, 1.0, shoulder_sharpness);
        ((lg2 * (b - a)) + a).exp2()
    };

    mapper_inv(x.powf(1.0 / exp))
}

/// Returns the absolute channel range that the filmic curve will map to [0.0, 1.0].
pub fn filmic_curve_absolute_range(fixed_point: f64, stops_range: [f64; 2]) -> [f64; 2] {
    [
        (fixed_point.log2() + stops_range[0]).exp2(),
        (fixed_point.log2() + stops_range[1]).exp2(),
    ]
}

/// A tweakable sigmoid function that maps [0.0, 1.0] to [0.0, 1.0].
///
/// - `transition`: the value of `x` where the foot transitions to the shoulder.
/// - `foot_exp`: the exponent used for the foot part of the curve.
///   1.0 = linear, 2.0 = quadratic, etc.
/// - `shoulder_exp`: the exponent used for the shoulder part of the curve.
fn s_curve(x: f64, transition: f64, foot_exp: f64, shoulder_exp: f64) -> f64 {
    // Early-out for off-the-end values.
    if x <= 0.0 {
        return 0.0;
    } else if x >= 1.0 {
        return 1.0;
    }

    // Foot and shoulder curve functions.
    let foot = |x: f64, scale: f64| -> f64 { x.powf(foot_exp) * scale };
    let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

    // Foot and shoulder slopes at the transition.
    let foot_slope = foot_exp * transition.powf(foot_exp - 1.0);
    let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

    // Vertical scale factors needed to make the foot and shoulder meet
    // at the transition with equal slopes.
    let s1 = shoulder_slope / foot_slope;
    let s2 = 1.0 / (1.0 + foot(transition, s1) - shoulder(transition, 1.0));

    // The full curve output.
    if x < transition {
        foot(x, s1 * s2)
    } else {
        shoulder(x, s2)
    }
    .clamp(0.0, 1.0)
}

/// Inverse of `s_curve()`.
fn s_curve_inv(x: f64, transition: f64, foot_exp: f64, shoulder_exp: f64) -> f64 {
    // Early-out for off-the-end values.
    if x <= 0.0 {
        return 0.0;
    } else if x >= 1.0 {
        return 1.0;
    }

    // Foot and shoulder curve functions.
    let foot = |x: f64, scale: f64| -> f64 { x.powf(foot_exp) * scale };
    let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

    // Foot and shoulder slopes at the transition.
    let foot_slope = foot_exp * transition.powf(foot_exp - 1.0);
    let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

    // Vertical scale factors needed to make the foot and shoulder meet
    // at the transition with equal slopes.
    let s1 = shoulder_slope / foot_slope;
    let s2 = 1.0 / (1.0 + foot(transition, s1) - shoulder(transition, 1.0));

    //-------------------------

    let transition_inv = foot(transition, s1 * s2);

    let foot_inv = |x: f64, scale: f64| -> f64 {
        // x.powf(foot_exp) * scale
        (x / scale).powf(1.0 / foot_exp)
    };
    let shoulder_inv = |x: f64, scale: f64| -> f64 {
        // 1.0 - ((1.0 - x).powf(shoulder_exp) * scale)
        1.0 - ((1.0 - x) / scale).powf(1.0 / shoulder_exp)
    };

    // The full curve output.
    if x < transition_inv {
        foot_inv(x, s1 * s2)
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
