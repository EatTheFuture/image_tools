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

use colorbox::transforms::rgb_gamut_intersect;

/// A simple but reasonably robust approach that clips in RGB space.
///
/// - `luminance_weights`: the relative amount that each RGB channel
///   contributes to luminance.  The three weights should add up to
///   1.0.
/// - `rolloff`: It specifies how much to "cheat" the luminance
///   mapping of out-of-gamut colors so that they stay saturated for
///   longer before blowing out to white.  0.0 is no cheating, and
///   larger values sacrifice luminance more and more.  Good values
///   are generally in the [0.0, 2.0] range.
///   Note: this is only meaningful when `channel_max` is not `None`.
pub fn rgb_clip(
    rgb: [f64; 3],
    channel_max: Option<f64>,
    clip_negative_luminance: bool,
    luminance_weights: [f64; 3],
    rolloff: f64,
) -> [f64; 3] {
    // Early-out for in-gamut colors.
    if let Some(m) = channel_max {
        if rgb[0] >= 0.0
            && rgb[1] >= 0.0
            && rgb[2] >= 0.0
            && rgb[0] <= m
            && rgb[1] <= m
            && rgb[2] <= m
        {
            return rgb;
        }
    } else if rgb[0] >= 0.0 && rgb[1] >= 0.0 && rgb[2] >= 0.0 {
        return rgb;
    };

    // Compute luminance.
    let l = (rgb[0] * luminance_weights[0])
        + (rgb[1] * luminance_weights[1])
        + (rgb[2] * luminance_weights[2]);

    // Early out for zero or clipped negative luminance.
    if l == 0.0 || (l < 0.0 && clip_negative_luminance) {
        return [0.0, 0.0, 0.0];
    }

    // Clip with unbounded channels first.
    let rgb = rgb_gamut_intersect(rgb, [l; 3], false, false);

    // No further processing on negative-luminance colors.
    if l < 0.0 {
        return rgb;
    }

    // Clip with channel maximum if one is specified.
    if let Some(channel_max) = channel_max {
        // Luminance rolloff for still out-of-gamut colors.
        let l = {
            let n = l / channel_max;
            let a = rolloff + 1.0;
            let n2 = 1.0 - (1.0 - (n.min(a) / a)).powf(a);
            n2 * channel_max
        };

        // Early out for over-luminant colors.
        if l > channel_max {
            return [channel_max; 3];
        }

        rgb_gamut_intersect(rgb, [l; 3], true, true)
    } else {
        rgb
    }
}
