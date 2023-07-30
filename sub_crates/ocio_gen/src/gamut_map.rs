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
use colorbox::transforms::rgb_gamut;

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
