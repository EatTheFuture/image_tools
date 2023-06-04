use colorbox::{
    lut::{Lut1D, Lut3D},
    matrix,
    transforms::rgb_gamut_intersect,
};

use crate::config::{ExponentLUTMapper, Interpolation, Transform};

/// A filmic(ish) tonemapping operator.
///
/// - `exposure`: input exposure adjustment before applying the tone mapping.
///   Input color values are simply multiplied by this.  Useful for tuning
///   different tone mappers to match general brightness without altering
///   the actual tone mapping curve.
/// - `contrast`: how "contrasty" the look should be.  Values between 1.0
///   and 4.0 give fairly normal looks, while higher starts to look more
///   high contrast.
/// - `fixed_point`: the luminance that should approximately map to
///   itself.  For example, you might set this to 0.18 (18% gray) so that
///   colors of that brightness remain roughly the same.
/// - `luminance_ceiling`: the luminance level that maps to 1.0 in the
///   output.  Typically you want this to be a large-ish number
///   (e.g. > 30), as it represents the top end of the dynamic range.
///   It can be useful to think in terms of photographic stops: if you
///   want 6 stops of dynamic range above 1.0, then this should be 2^6,
///   or 64.  In practice, this doesn't have much impact on the look
///   beyond maybe 14 stops or so.
#[derive(Debug, Copy, Clone)]
pub struct Tonemapper {
    exposure: f64,
    k: f64,
    fixed_point: f64,
    luminance_ceiling: Option<f64>,

    res_1d: usize,
    res_3d: usize,
    mapper_3d: ExponentLUTMapper,
}

impl Default for Tonemapper {
    fn default() -> Tonemapper {
        Tonemapper {
            exposure: 1.0,
            k: 0.0,
            fixed_point: 0.2,
            luminance_ceiling: None,

            res_1d: 2,
            res_3d: 2,
            mapper_3d: ExponentLUTMapper::new(1.0, 2, 1.0, [false, false, true], true),
        }
    }
}

impl Tonemapper {
    pub fn new(
        exposure: f64,
        contrast: f64,
        fixed_point: f64,
        luminance_ceiling: Option<f64>,
    ) -> Self {
        let k = contrast.abs().sqrt() * contrast.signum();

        let res_1d = 1 << 12;
        let res_3d = 32 + 1;

        Tonemapper {
            exposure: exposure,
            k: k,
            fixed_point: fixed_point,
            luminance_ceiling: luminance_ceiling,

            res_1d: res_1d,
            res_3d: res_3d,
            mapper_3d: ExponentLUTMapper::new(1.5, res_3d, 1.0, [true, true, true], true),
        }
    }

    pub fn eval_1d(&self, x: f64) -> f64 {
        if x <= 0.0 {
            0.0
        } else {
            filmic::curve(
                x * self.exposure,
                self.k,
                self.fixed_point,
                self.luminance_ceiling,
            )
            .min(1.0)
        }
    }

    pub fn eval_1d_inv(&self, y: f64) -> f64 {
        if y <= 0.0 {
            0.0
        } else if y >= 1.0 {
            if let Some(ceil) = self.luminance_ceiling {
                ceil
            } else {
                // Infinity would actually be correct here, but it leads
                // to issues in the generated LUTs.  So instead we just
                // choose an extremely large finite number that fits
                // within an f32 (since later processing may be done in
                // f32).
                (f32::MAX / 2.0) as f64
            }
        } else {
            filmic::curve_inv(y, self.k, self.fixed_point, self.luminance_ceiling) / self.exposure
        }
    }

    pub fn eval_rgb(&self, rgb: [f64; 3]) -> [f64; 3] {
        // TODO: this luma function is poor, just using the general idea that
        // blue has the least influence and green has the most.  In the future,
        // allow the client code to pass its own luma function, so that it can
        // be specific to the given color space and as sophisticated as needed.
        fn luma(rgb: [f64; 3]) -> f64 {
            const LUMA_WEIGHTS: [f64; 3] = [3.0 / 13.0, 9.0 / 13.0, 1.0 / 13.0];
            ((rgb[0] * LUMA_WEIGHTS[0]) + (rgb[1] * LUMA_WEIGHTS[1]) + (rgb[2] * LUMA_WEIGHTS[2]))
                / LUMA_WEIGHTS.iter().sum::<f64>()
        }

        // TODO: make this a user-settable parameter...?
        const DESAT_FACTOR: f64 = 0.85;

        let gray_point1 = {
            let l = luma(rgb);
            [l, l, l]
        };
        let vec1 = vsub(rgb, gray_point1);

        let rgb1 = {
            // Clip to the open-domain color gamut.
            let clipped = rgb_gamut_intersect(rgb, gray_point1, false, true);

            // Desaturate all colors slightly, so that even super saturated
            // colors blow out at the high end with the tone mapping curve.
            vlerp(gray_point1, clipped, DESAT_FACTOR)
        };

        // Apply tone mapping curve.
        let rgb2 = [
            self.eval_1d(rgb1[0]),
            self.eval_1d(rgb1[1]),
            self.eval_1d(rgb1[2]),
        ];

        let gray_point2 = {
            let l = luma(rgb2);
            [l, l, l]
        };

        // Re-saturate to restore colors.
        let resat = vlerp(gray_point2, rgb2, 1.0 / DESAT_FACTOR);

        // Adjust angle to approximately preserve hue.
        let rgb_final = {
            let length1 = gamut_relative_length(vec1, gray_point2, false);
            let length2 = gamut_relative_length(vsub(resat, gray_point2), gray_point2, false);
            vadd(gray_point2, vscale(vec1, length2 / length1))
        };

        // Clip to the closed-domain color gamut.
        rgb_gamut_intersect(rgb_final, gray_point2, true, true)
    }

    /// Generates a 1D and 3D LUT to apply the tone mapping.
    ///
    /// The LUTs should be applied with the transforms yielded by
    /// `tone_map_transforms()` further below.
    pub fn generate_luts(&self) -> (Lut1D, Lut3D) {
        use crate::hsv_lut::make_hsv_lut;
        use colorbox::transforms::ocio::{hsv_to_rgb, rgb_to_hsv};

        let lut_1d = Lut1D::from_fn_1(self.res_1d, 0.0, 1.0, |n| self.eval_1d_inv(n as f64) as f32);

        // The 3d LUT is generated to compensate for the missing bits
        // after just the tone mapping curve is applied per-channel.
        // It's sort of a "diff" that can be applied afterwards to get
        // the full rgb transform.
        let lut_3d = Lut3D::from_fn(
            [self.res_3d; 3],
            [0.0; 3],
            [self.mapper_3d.lut_max() as f32; 3],
            |(a, b, c)| {
                // Convert from LUT space to RGB.
                let rgb = self.mapper_3d.from_lut([a as f64, b as f64, c as f64]);

                // Convert from tonemapped space back to linear.
                let rgb_linear = [
                    self.eval_1d_inv(rgb[0]),
                    self.eval_1d_inv(rgb[1]),
                    self.eval_1d_inv(rgb[2]),
                ];

                // Figure out what it should map to.
                let rgb_adjusted = self.eval_rgb(rgb_linear);

                // Convert back to LUT space.
                let abc_final = self.mapper_3d.to_lut(rgb_adjusted);

                (
                    abc_final[0] as f32,
                    abc_final[1] as f32,
                    abc_final[2] as f32,
                )
            },
        );

        (lut_1d, lut_3d)
    }

    pub fn tone_map_transforms(&self, lut_1d_path: &str, lut_3d_path: &str) -> Vec<Transform> {
        let mut transforms = Vec::new();

        // Clip colors to 1.0 saturation, so they're within the range
        // of our LUTs.
        // TODO: this is a hack.  Replace with a "proper" gamut clip in
        // the future.
        transforms.extend([
            Transform::ToHSV,
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                1.0,
                1.0,
                1.0 / 10_000_000.0,
            ]))),
            Transform::RangeTransform {
                range_in: (0.0, 1.0),
                range_out: (0.0, 1.0),
                clamp: true,
            },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                1.0,
                1.0,
                10_000_000.0,
            ]))),
            Transform::FromHSV,
        ]);

        // Imperceptibly desaturate the colors, for better behavior
        // during tone mapping.  This ensures that even pathological
        // colors eventually blow out to white at high enough exposures.
        transforms.extend([Transform::MatrixTransform(matrix::to_4x4_f32(
            saturation_matrix(0.99),
        ))]);

        // Apply tone map curve.
        transforms.extend([Transform::FileTransform {
            src: lut_1d_path.into(),
            interpolation: Interpolation::Linear,
            direction_inverse: true,
        }]);

        // Apply chroma LUT.
        transforms.extend(self.mapper_3d.transforms_lut_3d(lut_3d_path));

        transforms
    }
}

/// A "filmic" tone mapping curve.
///
/// The basic idea behind this is to layer a sigmoid contrast function
/// on top of a generalized Reinhard function.  This has no particular
/// basis in anything, but in practice produces pleasing results that
/// are easy to adjust for different looks.
///
/// Note: this maps [0.0, inf] to [0.0, 1.0].  So if you want to limit
/// the dynamic range, you should scale up and clamp the result by the
/// appropriate amount.
mod filmic {
    use super::{reinhard, reinhard_inv};

    /// How "gentle" our Reinhard function is.  1.0 is standard Reinhard,
    /// less than 1.0 is sharper, more than 1.0 is gentler.  Gentler
    /// seems to give more flexibility in terms of effective dynamic
    /// range.
    const REINHARD_P: f64 = 2.0;

    /// `c`: contrast.  A value of zero creates the classic Reinhard
    ///      curve, larger values produce a more contrasty look, and
    ///      lower values less.
    /// `fixed_point`: the value of `x` that should map to itself.
    #[inline(always)]
    pub fn curve(x: f64, c: f64, fixed_point: f64, luminance_ceiling: Option<f64>) -> f64 {
        let b = fixed_point.log(0.5);
        let reinhard_scale_x = reinhard_inv(fixed_point, REINHARD_P) / fixed_point;
        let reinhard_scale_y = if let Some(ceil) = luminance_ceiling {
            1.0 / reinhard(ceil, REINHARD_P)
        } else {
            1.0
        };

        // Reinhard.
        let r = reinhard(x * reinhard_scale_x, REINHARD_P) * reinhard_scale_y;

        // Contrast sigmoid.
        let n = r.powf(1.0 / b);
        let m = contrast(n, c);
        m.powf(b)
    }

    #[inline(always)]
    pub fn curve_inv(y: f64, c: f64, fixed_point: f64, luminance_ceiling: Option<f64>) -> f64 {
        let b = fixed_point.log(0.5);
        let reinhard_scale_x = reinhard_inv(fixed_point, REINHARD_P) / fixed_point;
        let reinhard_scale_y = if let Some(ceil) = luminance_ceiling {
            1.0 / reinhard(ceil, REINHARD_P)
        } else {
            1.0
        };

        // Contrast sigmoid.
        let m = y.powf(1.0 / b);
        let n = contrast(m, -c);
        let r = n.powf(b);

        // Reinhard.
        reinhard_inv(r / reinhard_scale_y, REINHARD_P) / reinhard_scale_x
    }

    /// A sigmoid based on the classic logistic function.
    ///
    /// Maps [-inf,+inf] to [-1,+1].
    ///
    /// `k` determines how sharp the mapping is.
    #[inline(always)]
    fn sigmoid(x: f64, k: f64) -> f64 {
        (2.0 / (1.0 + (-k * x).exp())) - 1.0
    }

    #[inline(always)]
    fn sigmoid_inv(x: f64, k: f64) -> f64 {
        if x <= -1.0 {
            -std::f64::INFINITY
        } else if x >= 1.0 {
            std::f64::INFINITY
        } else {
            ((2.0 / (x + 1.0)) - 1.0).ln() / -k
        }
    }

    /// Adjusts contrast in [0.0, 1.0] via a sigmoid function.
    ///
    /// Unlike simply multiplying `v` by some constant with a pivot around
    /// 0.5, this function uses a sigmoid to compress/expand the values near
    /// 0.0 and 1.0, which avoids pushing any values outside of [0.0, 1.0].
    ///
    /// `x`: value to adjust.
    /// `c`: amount of contrast adjustment. 0 is no adjustment, > 0 increases
    ///      contrast, < 0 decreases contrast.
    ///
    /// Note: this function is its own inverse by simply passing the negative
    /// of `c`.
    #[inline(always)]
    fn contrast(x: f64, c: f64) -> f64 {
        if c > -0.00001 && c < 0.00001 {
            // Conceptually this is for when `c == 0.0`, but for numerical
            // stability reasons we do it within a small range around 0.0.
            x
        } else {
            let scale = 2.0 * sigmoid(1.0, c);
            if c > 0.0 {
                // Increase contrast.
                sigmoid(2.0 * x - 1.0, c) / scale + 0.5
            } else {
                // Decrease contrast.
                sigmoid_inv((x - 0.5) * -scale, -c) * 0.5 + 0.5
            }
        }
        .clamp(0.0, 1.0)
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn contrast_round_trip() {
            for i in 0..17 {
                let x = i as f64 / 16.0;
                let x2 = contrast(contrast(x, 2.0), -2.0);
                assert!((x - x2).abs() < 0.000_000_1);
            }
        }

        #[test]
        fn filmic_curve_round_trip() {
            for i in 0..17 {
                let x = i as f64 / 16.0;
                let x2 = curve(
                    curve_inv(x, 2.0, 0.18, Some(2048.0)),
                    2.0,
                    0.18,
                    Some(2048.0),
                );
                assert!((x - x2).abs() < 0.000_001);
            }
        }
    }
}

#[inline(always)]
fn gamut_relative_length(vec: [f64; 3], gray_point: [f64; 3], closed_domain: bool) -> f64 {
    let len = vlen(vec);
    if len > 0.00000001 {
        let distant = vadd(gray_point, vscale(vec, 65536.0));
        let projected = rgb_gamut_intersect(distant, gray_point, closed_domain, true);
        vlen(vec) / vlen(vsub(projected, gray_point))
    } else {
        0.0
    }
}

/// Generates a matrix that does a simplistic saturation adjustment.
///
/// Note: only use this for very tiny adjustments.  It's poorly suited
/// for anything else.
fn saturation_matrix(factor: f64) -> matrix::Matrix {
    let a = (factor * 2.0 + 1.0) / 3.0;
    let b = (1.0 - a) / 2.0;

    [[a, b, b], [b, a, b], [b, b, a]]
}

/// Generalized Reinhard curve.
///
/// `p`: a tweaking parameter that affects the shape of the curve,
///      in (0.0, inf].  Larger values make it gentler, lower values
///      make it sharper.  1.0 = standard Reinhard, 0.0 = linear
///      in [0,1].
#[inline(always)]
fn reinhard(x: f64, p: f64) -> f64 {
    // Make out-of-range numbers do something reasonable and predictable.
    if x <= 0.0 {
        return x;
    }

    // Special case so we get linear at `p == 0` instead of undefined.
    // Negative `p` is unsupported, so clamp.
    if p <= 0.0 {
        if x >= 1.0 {
            return 1.0;
        } else {
            return x;
        }
    }

    let tmp = x.powf(-1.0 / p);

    // Special cases for numerical stability.
    // Note that for the supported values of `p`, `tmp > 1.0` implies
    // `x < 1.0` and vice versa.
    if tmp > 1.0e15 {
        return x;
    } else if tmp < 1.0e-15 {
        return 1.0;
    }

    // Actual generalized Reinhard.
    (tmp + 1.0).powf(-p)
}

/// Inverse of `reinhard()`.
#[inline(always)]
fn reinhard_inv(x: f64, p: f64) -> f64 {
    // Make out-of-range numbers do something reasonable and predictable.
    if x <= 0.0 {
        return x;
    } else if x >= 1.0 {
        return std::f64::INFINITY;
    }

    // Special case so we get linear at `p == 0` instead of undefined.
    // Negative `p` is unsupported, so clamp.
    if p <= 0.0 {
        return x;
    }

    let tmp = x.powf(-1.0 / p);

    // Special case for numerical stability.
    if tmp > 1.0e15 {
        return x;
    }

    // Actual generalized Reinhard inverse.
    (tmp - 1.0).powf(-p)
}

fn smoothstep(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        (3.0 * x * x) - (2.0 * x * x * x)
    }
}

fn smootherstep(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        (6.0 * x * x * x * x * x) - (15.0 * x * x * x * x) + (10.0 * x * x * x)
    }
}

fn vadd(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn vsub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vscale(a: [f64; 3], scale: f64) -> [f64; 3] {
    [a[0] * scale, a[1] * scale, a[2] * scale]
}

fn vlen(a: [f64; 3]) -> f64 {
    ((a[0] * a[0]) + (a[1] * a[1]) + (a[2] * a[2])).sqrt()
}

fn vmax(a: [f64; 3]) -> f64 {
    a[0].max(a[1]).max(a[2])
}

fn vlerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        (a[0] * (1.0 - t)) + (b[0] * t),
        (a[1] * (1.0 - t)) + (b[1] * t),
        (a[2] * (1.0 - t)) + (b[2] * t),
    ]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tonemap_1d_round_trip() {
        let curve = Tonemapper::new(1.1, 2.0, 0.18, Some(5.0_f64.exp2()));
        for i in 0..17 {
            let x = i as f64 / 16.0;
            let x2 = curve.eval_1d(curve.eval_1d_inv(x));
            assert!((x - x2).abs() < 0.000_001);
        }
    }

    #[test]
    fn reinhard_round_trip() {
        for i in 0..17 {
            for p in 0..17 {
                let x = (i - 8) as f64 / 4.0;
                let p = i as f64 / 8.0;
                let x2 = reinhard_inv(reinhard(x, p), p);
                assert!((x - x2).abs() < 0.000_001);
            }
        }
    }
}
