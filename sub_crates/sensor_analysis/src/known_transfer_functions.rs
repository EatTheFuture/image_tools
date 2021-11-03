/// The sRGB gamma curve.
pub mod srgb {
    /// Linear -> sRGB
    #[inline]
    pub fn from_linear(n: f32) -> f32 {
        if n < 0.003_130_8 {
            n * 12.92
        } else {
            (1.055 * n.powf(1.0 / 2.4)) - 0.055
        }
    }

    /// sRGB -> Linear
    #[inline]
    pub fn to_linear(n: f32) -> f32 {
        if n < 0.04045 {
            n / 12.92
        } else {
            ((n + 0.055) / 1.055).powf(2.4)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn from_linear_test() {
            assert_eq!(from_linear(0.0), 0.0);
            assert!((from_linear(1.0) - 1.0).abs() < 0.0000001);
        }

        #[test]
        fn to_linear_test() {
            assert_eq!(to_linear(0.0), 0.0);
            assert!((to_linear(1.0) - 1.0).abs() < 0.0000001);
        }
    }
}

/// Perceptual Quantizer from Rec.2100.
///
/// Note: the main functions in this module are not
/// [0.0, 1.0] -> [0.0, 1.0] mappings.  They are mappings between linear
/// [0.0, `LUMINANCE_MAX`] and non-linear [0.0, 1.0].
///
/// If you want a [0.0, 1.0] -> [0.0, 1.0] mapping, use the `*_norm()`
/// functions instead, which simply scale `LUMINANCE_MAX` to 1.0.
pub mod pq {
    /// The maximum allowed luminance of linear values, in cd/m^2.
    pub const LUMINANCE_MAX: f32 = 10000.0;

    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;

    /// Linear -> PQ (OETF function).
    ///
    /// Input is in the range [0, `LUMINANCE_MAX`], representing display
    /// luminance in cd/m^2.
    /// Output is in the range [0.0, 1.0].
    #[inline(always)]
    pub fn from_linear(n: f32) -> f32 {
        assert!(n >= 0.0 && n <= LUMINANCE_MAX);
        from_linear_norm(n / LUMINANCE_MAX)
    }

    /// PQ -> Linear (EOTF function).
    ///
    /// Input is in the range [0.0, 1.0].
    /// Output is in the range [0, `LUMINANCE_MAX`], representing display
    /// luminance in cd/m^2.
    #[inline(always)]
    pub fn to_linear(n: f32) -> f32 {
        assert!(n >= 0.0 && n <= 1.0);
        to_linear_norm(n) * LUMINANCE_MAX
    }

    /// Linear -> PQ, except both input and output are [0.0, 1.0].
    #[inline]
    pub fn from_linear_norm(n: f32) -> f32 {
        let n_m1 = n.powf(M1);
        ((C1 + (C2 * n_m1)) / (1.0 + (C3 * n_m1))).powf(M2)
    }

    /// PQ -> Linear, except both input and output are [0.0, 1.0].
    #[inline]
    pub fn to_linear_norm(n: f32) -> f32 {
        let n_1_m2 = n.powf(1.0 / M2);

        ((n_1_m2 - C1).max(0.0) / (C2 - (C3 * n_1_m2))).powf(1.0 / M1)
    }
}

/// Hybrid Log-Gamma from Rec.2100.
pub mod hlg {
    const A: f32 = 0.17883277;
    const B: f32 = 1.0 - (4.0 * A);

    /// Linear -> HLG (OETF function).
    ///
    /// Input and output are both [0.0, 1.0].
    #[inline]
    pub fn from_linear(n: f32) -> f32 {
        let c = 0.5 - (A * (4.0 * A).ln()); // Should be a `const`, but can't because of `ln()`.

        assert!(n >= 0.0 && n <= 1.0);
        if n <= (1.0 / 12.0) {
            (3.0 * n).sqrt()
        } else {
            A * (12.0 * n - B).ln() + c
        }
    }

    /// HLG -> Linear (EOTF function).
    ///
    /// Input and output are both [0.0, 1.0].
    #[inline]
    pub fn to_linear(n: f32) -> f32 {
        let c = 0.5 - (A * (4.0 * A).ln()); // Should be a `const`, but can't because of `ln()`.

        assert!(n >= 0.0 && n <= 1.0);
        if n <= 0.5 {
            (n * n) / 3.0
        } else {
            (((n - c) / A).exp() + B) / 12.0
        }
    }
}

/// Sony's S-Log2 curve.
///
/// Note: the main functions in this module are not
/// [0.0, 1.0] -> [0.0, 1.0] mappings.  They are mappings between "scene
/// linear" and "code values".  For example, scene-linear 0.0 maps to
/// `CV_BLACK` (which is > 0.0) and scene-linear 1.0 maps to `CV_WHITE`
/// (which is < 1.0).  (And just to spell it out: this means that
/// scene-linear values can be both less than 0.0 and greater than 1.0).
///
/// If you want a [0.0, 1.0] -> [0.0, 1.0] mapping with the same
/// non-linearity as S-Log2, use the `*_norm()` functions instead.
pub mod sony_slog2 {
    /// The normalized code value of scene-linear 0.0.
    pub const CV_BLACK: f32 = 0.088251315;

    /// The normalized code value of scene-linear 1.0.
    pub const CV_WHITE: f32 = 0.58509105;

    /// The scene-linear value of normalized code value 1.0.
    pub const LINEAR_MAX: f32 = 13.758276;

    /// Misc internal constants used on the S-Log2 formulas.
    const SLOG2_BLACK: f32 = 64.0 / 1023.0;
    const SLOG2_WHITE: f32 = 940.0 / 1023.0;

    /// From scene linear to (normalized) code values.
    ///
    /// For example, to get 10-bit code values do
    /// `from_linear(scene_linear_in) * 1023.0`
    #[inline]
    pub fn from_linear(x: f32) -> f32 {
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

    /// From (normalized) code values to scene linear.
    ///
    /// For example, if using 10-bit code values do
    /// `to_linear(10_bit_cv_in / 1023.0)`
    #[inline]
    pub fn to_linear(x: f32) -> f32 {
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

    /// Same as `from_linear()` except remapped so input [0.0, 1.0] ->
    /// output [0.0, 1.0].
    ///
    /// Essentially, this is the non-linearity curve of S-Log2.
    #[inline(always)]
    pub fn from_linear_norm(x: f32) -> f32 {
        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        let x = x * LINEAR_MAX;

        let y = from_linear(x);

        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        (y - CV_BLACK) / (1.0 - CV_BLACK)
    }

    /// Inverse of `from_linear_norm()`.
    #[inline(always)]
    pub fn to_linear_norm(x: f32) -> f32 {
        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        let x = CV_BLACK + (x * (1.0 - CV_BLACK));

        let y = to_linear(x);

        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        y / LINEAR_MAX
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn from_linear_test() {
            // Invariants from page 6 of "S-Log2 Technical Paper v1.0" from
            // Sony, June 6th 2012.
            assert!((from_linear(0.0) - (90.0 / 1023.0)).abs() < 0.001);
            assert!((from_linear(0.18) - (347.0 / 1023.0)).abs() < 0.001);
            assert!((from_linear(0.9) - (582.0 / 1023.0)).abs() < 0.001);
        }

        #[test]
        fn to_linear_test() {
            // Invariants from page 6 of "S-Log2 Technical Paper v1.0" from
            // Sony, June 6th 2012.
            assert!((to_linear(90.0 / 1023.0) - 0.0).abs() < 0.001);
            assert!((to_linear(347.0 / 1023.0) - 0.18).abs() < 0.001);
            assert!((to_linear(582.0 / 1023.0) - 0.9).abs() < 0.001);
        }

        #[test]
        fn from_linear_norm_test() {
            assert!(from_linear_norm(0.0) >= 0.0);
            assert!(from_linear_norm(0.0) <= 0.00001);

            assert!((from_linear_norm(1.0) - 1.0) <= 0.0);
            assert!((from_linear_norm(1.0) - 1.0) >= -0.00001);
        }

        #[test]
        fn to_linear_norm_test() {
            assert!(to_linear_norm(0.0) >= 0.0);
            assert!(to_linear_norm(0.0) <= 0.00001);

            assert!((to_linear_norm(1.0) - 1.0) <= 0.0);
            assert!((to_linear_norm(1.0) - 1.0) >= -0.00001);
        }
    }
}

/// Sony's S-Log3 curve.
///
/// Note: the main functions in this module are not
/// [0.0, 1.0] -> [0.0, 1.0] mappings.  They are mappings between "scene
/// linear" and "code values".  For example, scene-linear 0.0 maps to
/// `CV_BLACK` (which is > 0.0) and scene-linear 1.0 maps to `CV_WHITE`
/// (which is < 1.0).  (And just to spell it out: this means that
/// scene-linear values can be both less than 0.0 and greater than 1.0).
///
/// If you want a [0.0, 1.0] -> [0.0, 1.0] mapping with the same
/// non-linearity as S-Log3, use the `*_norm()` functions instead.
pub mod sony_slog3 {
    /// The normalized code value of scene-linear 0.0.
    pub const CV_BLACK: f32 = 0.092864126;

    /// The normalized code value of scene-linear 1.0.
    pub const CV_WHITE: f32 = 0.5960273;

    /// The scene-linear value of normalized code value 1.0.
    pub const LINEAR_MAX: f32 = 38.420933;

    /// From scene linear to (normalized) code values.
    ///
    /// For example, to get 10-bit code values do
    /// `from_linear(scene_linear_in) * 1023.0`
    pub fn from_linear(x: f32) -> f32 {
        if x < 0.01125000 {
            (x * (171.2102946929 - 95.0) / 0.01125000 + 95.0) / 1023.0
        } else {
            (420.0 + ((x + 0.01) / (0.18 + 0.01)).log10() * 261.5) / 1023.0
        }
    }

    /// From (normalized) code values to scene linear.
    ///
    /// For example, if using 10-bit code values do
    /// `to_linear(10_bit_cv_in / 1023.0)`
    pub fn to_linear(x: f32) -> f32 {
        if x < (171.2102946929 / 1023.0) {
            (x * 1023.0 - 95.0) * 0.01125000 / (171.2102946929 - 95.0)
        } else {
            (10.0f32.powf((x * 1023.0 - 420.0) / 261.5)) * (0.18 + 0.01) - 0.01
        }
    }

    /// Same as `from_linear()` except remapped so input [0.0, 1.0] ->
    /// output [0.0, 1.0].
    ///
    /// Essentially, this is the non-linearity curve of S-Log3.
    pub fn from_linear_norm(x: f32) -> f32 {
        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        let x = x * LINEAR_MAX;

        let y = from_linear(x);

        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        (y - CV_BLACK) / (1.0 - CV_BLACK)
    }

    /// Inverse of `from_linear_norm()`.
    pub fn to_linear_norm(x: f32) -> f32 {
        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        let x = CV_BLACK + (x * (1.0 - CV_BLACK));

        let y = to_linear(x);

        // Adjustment to map 0.0 to 0.0 and 1.0 to 1.0.
        // Note: this is not part of the official mapping curve.
        y / LINEAR_MAX
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn from_linear_test() {
            // Invariants from page 6 of "Technical Summary for
            // S-Gamut3.Cine/S-Log3 and S-Gamut3/S-Log3", from Sony.
            assert!((from_linear(0.0) - (95.0 / 1023.0)).abs() < 0.001);
            assert!((from_linear(0.18) - (420.0 / 1023.0)).abs() < 0.001);
            assert!((from_linear(0.9) - (598.0 / 1023.0)).abs() < 0.001);
        }

        #[test]
        fn to_linear_test() {
            // Invariants from page 6 of "Technical Summary for
            // S-Gamut3.Cine/S-Log3 and S-Gamut3/S-Log3", from Sony.
            assert!((to_linear(95.0 / 1023.0) - 0.0).abs() < 0.001);
            assert!((to_linear(420.0 / 1023.0) - 0.18).abs() < 0.001);
            assert!((to_linear(598.0 / 1023.0) - 0.9).abs() < 0.001);
        }

        #[test]
        fn from_linear_norm_test() {
            assert!(from_linear_norm(0.0) >= 0.0);
            assert!(from_linear_norm(0.0) <= 0.00001);

            assert!((from_linear_norm(1.0) - 1.0) <= 0.0);
            assert!((from_linear_norm(1.0) - 1.0) >= -0.00001);
        }

        #[test]
        fn to_linear_norm_test() {
            assert!(to_linear_norm(0.0) >= 0.0);
            assert!(to_linear_norm(0.0) <= 0.00001);

            assert!((to_linear_norm(1.0) - 1.0) <= 0.0);
            assert!((to_linear_norm(1.0) - 1.0) >= -0.00001);
        }
    }
}
