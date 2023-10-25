/// Various methods for mapping and clipping out-of-gamut RGB colors.
///
/// All functions in this module take the rgb color to be clipped and an
/// optional `channel_max` as the first two parameters.
///
/// If `channel_max` is `None`, then the gamut is assumed to extend to
/// infinite luminance, and colors will not be clipped on that axis.
/// This is typically useful for processing input colors (e.g. footage,
/// textures).  If `channel_max` is `Some(value)`, then `value` is
/// the maximum value each RGB channel can be in the output, and colors
/// will be clipped to that as well.  This is typically useful for
/// processing output colors (e.g. for display).
use colorbox::{
    matrix::{self, Matrix},
    transforms::rgb_gamut,
};

use crate::config::Transform;

/// A simple but reasonably robust approach that clips in RGB space.
///
/// - `luminance_weights`: the relative amount that each RGB channel
///   contributes to luminance.  The three weights should add up to
///   1.0.
/// - `softness`: if there is a `channel_max`, this is the amount of
///   "cheating" to do to make the transition blowing out to white
///   smooth and pleasing.  0.0 is no cheating, but results in a
///   possibly over-sharp transition.  Larger values cheat more to
///   make the transition smoother.  Good values are generally
///   between 0.05 and 0.2.
pub fn rgb_clip(
    rgb: [f64; 3],
    channel_max: Option<f64>,
    luminance_weights: [f64; 3],
    softness: f64,
) -> [f64; 3] {
    // Early-out for in-gamut colors.
    if channel_max.is_none() && rgb[0] >= 0.0 && rgb[1] >= 0.0 && rgb[2] >= 0.0 {
        return rgb;
    };

    // Compute luminance.
    let l = (rgb[0] * luminance_weights[0])
        + (rgb[1] * luminance_weights[1])
        + (rgb[2] * luminance_weights[2]);

    if let Some(channel_max) = channel_max {
        let rgb1 = [
            rgb[0] / channel_max,
            rgb[1] / channel_max,
            rgb[2] / channel_max,
        ];
        let rgb2 =
            rgb_gamut::closed_domain_clip(rgb_gamut::open_domain_clip(rgb1, l, 0.0), l, softness);
        [
            rgb2[0] * channel_max,
            rgb2[1] * channel_max,
            rgb2[2] * channel_max,
        ]
    } else {
        rgb_gamut::open_domain_clip(rgb, l, 0.0)
    }
}

fn sat_matrix(sat: f64, weights: [f64; 3]) -> Matrix {
    let mat1 = [weights, weights, weights];
    let mat2 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
        [
            (a[0] * (1.0 - t)) + (b[0] * t),
            (a[1] * (1.0 - t)) + (b[1] * t),
            (a[2] * (1.0 - t)) + (b[2] * t),
        ]
    }

    [
        lerp3(mat1[0], mat2[0], sat),
        lerp3(mat1[1], mat2[1], sat),
        lerp3(mat1[2], mat2[2], sat),
    ]
}

pub fn hsv_gamut_clip() -> Vec<Transform> {
    // This whole thing is basically one big hack to work around OCIO's
    // bizarre RGB<->HSV conversion behavior for colors with saturation
    // greater than 1.0.  Instead of just straightforwardly converting to
    // HSV and then clamping S to 1.0, we instead have to pre-desaturate
    // the color by some fixed amount, then convert to HSV, and clamp to
    // that desaturated level, then convert back, and then re-saturate
    // by the same fixed amount we desaturated by.  This keeps the
    // saturation < 1.0 from the point of view of OCIO's RGB<->HSV
    // conversion routines, which in turn makes it behave sanely.
    // Sigh...

    const MAX_V: f64 = (1u64 << 24) as f64;
    const DESAT_FAC: f64 = 4.0;

    let desat_mat = sat_matrix(1.0 / DESAT_FAC, [1.0; 3]);
    let desat_mat_inv = matrix::invert(desat_mat).unwrap();

    vec![
        Transform::MatrixTransform(matrix::to_4x4_f32(desat_mat)),
        Transform::ToHSV,
        Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
            1.0,
            DESAT_FAC,
            1.0 / MAX_V,
        ]))),
        // We have to clamp all channels, because per-channel clamping
        // isn't possible in OCIO.  We pre-scaled V so that clamping it
        // to 1.0 actually clamps to a very large value.  And we simply
        // reverse that scaling afterwards.
        Transform::RangeTransform {
            range_in: (-1.0, 1.0),
            range_out: (-1.0, 1.0),
            clamp: true,
        },
        Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
            1.0,
            1.0 / DESAT_FAC,
            MAX_V,
        ]))),
        Transform::FromHSV,
        Transform::MatrixTransform(matrix::to_4x4_f32(desat_mat_inv)),
    ]
}
