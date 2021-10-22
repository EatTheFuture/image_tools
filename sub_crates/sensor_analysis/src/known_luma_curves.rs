/// Linear -> sRGB
pub fn srgb(n: f32) -> f32 {
    if n < 0.003_130_8 {
        n * 12.92
    } else {
        (1.055 * n.powf(1.0 / 2.4)) - 0.055
    }
}

/// sRGB -> Linear
pub fn srgb_inv(n: f32) -> f32 {
    if n < 0.04045 {
        n / 12.92
    } else {
        ((n + 0.055) / 1.055).powf(2.4)
    }
}

const SLOG2_BLACK: f32 = 64.0 / 1023.0;
const SLOG2_WHITE: f32 = 940.0 / 1023.0;

/// Linear -> S-Log2.
///
/// Note: this should not be used for camera-raw footage.
pub fn sony_slog2(x: f32) -> f32 {
    // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
    // Note: this is not part of the official mapping curve.
    let x = x * 13.758276;

    let y = sony_slog2_raw(x);

    // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
    // Note: this is not part of the official mapping curve.
    (y - 0.088251315) / (1.0 - 0.088251315)
}

/// S-Log2 -> Linear
///
/// Note: this should not be used for camera-raw footage.
pub fn sony_slog2_inv(x: f32) -> f32 {
    // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
    // Note: this is not part of the official mapping curve.
    let x = 0.088251315 + (x * (1.0 - 0.088251315));

    let y = sony_slog2_raw_inv(x);

    // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
    // Note: this is not part of the official mapping curve.
    y / 13.758276
}

/// Linear -> S-Log2 raw CVs.
///
/// Note: this is not a [0.0, 1.0] -> [0.0, 1.0] mapping.  See the
/// documentation of `sony_slog2_cv_inv()` for details.
///
/// For a [0.0, 1.0] mapping, use `sony_slog2()`.
pub fn sony_slog2_raw(x: f32) -> f32 {
    let x = x / 0.9;

    // Mapping curve.
    let y = if x < 0.0 {
        x * 3.538_812_785_388_13 + 0.030_001_222_851_889_303
    } else {
        (0.432699 * (155.0 * x / 219.0 + 0.037584).log10() + 0.616596) + 0.03
    };

    // Map 0.0 and 1.0 to "code value" black and white levels,
    // respectively.
    (y * (SLOG2_WHITE - SLOG2_BLACK)) + SLOG2_BLACK
}

/// S-Log2 raw CVs -> Linear
///
/// This function takes "CVs", which are the values written to raw image
/// files by the camera.  Since the bit-depth of raw files can vary, this
/// function takes those values normalized from [0, max] to [0.0, 1.0].
///
/// It's also important to note that the CV values don't map to linear
/// color space in a straightforward way.  For example, the black value
/// for CV is actually about 0.088, 18% grey is about 0.34, and 90%
/// "white" is about 0.569.  This also means that a CV of 1.0 maps to
/// greater than 1.0 in linear (specifically, about 13.76 linear).
///
/// So this is not a [0.0, 1.0] -> [0.0, 1.0] mapping.
///
/// For a [0.0, 1.0] mapping, use `sony_slog2_inv()`, which is the same
/// as this function except scaled to match a 0.0 value of linear black
/// and a 1.0 value of CV max on both input and output.
pub fn sony_slog2_raw_inv(x: f32) -> f32 {
    // Map "code value" black and white levels to 0.0 and 1.0,
    // respectively.
    let x = (x - SLOG2_BLACK) / (SLOG2_WHITE - SLOG2_BLACK);

    // Mapping curve.
    let y = if x < 0.030_001_222_851_889_303 {
        (x - 0.030_001_222_851_889_303) / 3.538_812_785_388_13
    } else {
        219.0 * (10.0f32.powf((x - 0.03 - 0.616596) / 0.432699) - 0.037584) / 155.0
    };

    y * 0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sony_slog2_test() {
        assert!(sony_slog2(0.0) >= 0.0);
        assert!(sony_slog2(0.0) <= 0.00001);

        assert!((sony_slog2(1.0) - 1.0) <= 0.0);
        assert!((sony_slog2(1.0) - 1.0) >= -0.00001);
    }

    #[test]
    fn sony_slog2_inv_test() {
        assert!(sony_slog2_inv(0.0) >= 0.0);
        assert!(sony_slog2_inv(0.0) <= 0.00001);

        assert!((sony_slog2_inv(1.0) - 1.0) <= 0.0);
        assert!((sony_slog2_inv(1.0) - 1.0) >= -0.00001);
    }

    #[test]
    fn sony_slog2_raw_test() {
        // Invariants from page 6 of "S-Log2 Technical Paper v1.0" from
        // Sony, June 6th 2012.
        assert!((sony_slog2_raw(0.0) - (90.0 / 1023.0)).abs() < 0.001);
        assert!((sony_slog2_raw(0.18) - (347.0 / 1023.0)).abs() < 0.001);
        assert!((sony_slog2_raw(0.9) - (582.0 / 1023.0)).abs() < 0.001);
    }

    #[test]
    fn sony_slog2_raw_inv_test() {
        // Invariants from page 6 of "S-Log2 Technical Paper v1.0" from
        // Sony, June 6th 2012.
        assert!((sony_slog2_raw_inv(90.0 / 1023.0) - 0.0).abs() < 0.001);
        assert!((sony_slog2_raw_inv(347.0 / 1023.0) - 0.18).abs() < 0.001);
        assert!((sony_slog2_raw_inv(582.0 / 1023.0) - 0.9).abs() < 0.001);
    }
}
