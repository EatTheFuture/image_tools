use colorbox::{
    lut::{Lut1D, Lut3D},
    matrix,
};

use crate::config::{Interpolation, Transform};

/// A simple filmic tonemapping curve.
///
/// The basic idea behind this is to layer a power function (for the
/// toe) on top of an adjustable Reinhard function (for the shoulder.)
/// This has no real basis in the actual physics of film stock, but in
/// practice produces pleasing results and is reasonably easy to tweak
/// for different looks.
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
/// - `toe_sharpness`: how sharp the toe is.  Reasonable values are
///   typically in [0.0, 1.0].
/// - `shoulder_sharpness`: how sharp the shoulder is.  Reasonable values
///   are typically in [-0.2, 1.0].
///
/// Note that setting both toe and shoulder sharpness to zero creates
/// the classic Reinhard tone mapping curve.
///
/// Returns the tonemapped value, always in the range [0.0, 1.0].
#[derive(Debug, Copy, Clone)]
pub struct FilmicCurve {
    a: f64,
    b: f64,
    scale_x: f64,
    scale_y: f64,
    luminance_ceiling: f64,
}

impl FilmicCurve {
    pub fn new(
        fixed_point: f64,
        luminance_ceiling: f64,
        toe_sharpness: f64,
        shoulder_sharpness: f64,
    ) -> Self {
        let a = toe_sharpness + 1.0;
        let b = shoulder_sharpness + 1.0;
        let scale_x = fixed_point
            / FilmicCurve {
                a: a,
                b: b,
                scale_x: 1.0,
                scale_y: 1.0,
                luminance_ceiling: 0.0,
            }
            .eval_inv(fixed_point);
        let scale_y = 1.0
            / FilmicCurve {
                a: a,
                b: b,
                scale_x: scale_x,
                scale_y: 1.0,
                luminance_ceiling: 0.0,
            }
            .eval(luminance_ceiling);

        FilmicCurve {
            a: a,
            b: b,
            scale_x: scale_x,
            scale_y: scale_y,
            luminance_ceiling: luminance_ceiling,
        }
    }

    pub fn eval(&self, x: f64) -> f64 {
        if x <= 0.0 {
            0.0
        } else {
            ((x / self.scale_x).powf(-self.b) + 1.0).powf(self.a / -self.b) * self.scale_y
        }
    }

    pub fn eval_inv(&self, y: f64) -> f64 {
        if y <= 0.0 {
            0.0
        } else if y >= 1.0 {
            self.luminance_ceiling
        } else {
            ((y / self.scale_y).powf(-self.b / self.a) - 1.0).powf(1.0 / -self.b) * self.scale_x
        }
    }

    /// Generates a 3D and 1D LUT to apply the filmic tone mapping.
    ///
    /// The 3D LUT should be applied in an HSV space generated from RGB
    /// values with an exponent 0.5 applied to them.  The 1D LUT should
    /// be applied as an inverse transform in linear RGB space.
    pub fn generate_luts(&self, res_1d: usize, res_3d: usize) -> (Lut1D, Lut3D) {
        use crate::hsv_lut::make_hsv_lut;
        use colorbox::transforms::ocio::{hsv_to_rgb, rgb_to_hsv};

        let lut_1d = Lut1D::from_fn_1(res_1d, 0.0, 1.0, |n| self.eval_inv(n as f64) as f32);

        let lut_3d = make_hsv_lut(res_3d, (0.0, 1.0), 1.0, |(th, ts, tv)| {
            // To tone mapped rgb.
            let [tr, tg, tb] = hsv_to_rgb([th as f64, ts as f64, tv as f64]);

            // Linear rgb.
            let [lr, lg, lb] = [self.eval_inv(tr), self.eval_inv(tg), self.eval_inv(tb)];

            // Linear hsv.
            let [lh, ls, lv] = rgb_to_hsv([lr, lg, lb]);

            // Gamut-clipped linear rgb.
            let clipped_rgb = crate::gamut_map::rgb_clip(
                [lr, lg, lb],
                Some(self.luminance_ceiling),
                true,
                [0.2, 0.6, 0.2],
                0.0,
            );

            // Gamut-clipped tone-mapped rgb.
            let clipped_hsv = rgb_to_hsv([
                self.eval(clipped_rgb[0]),
                self.eval(clipped_rgb[1]),
                self.eval(clipped_rgb[2]),
            ]);

            // Adjusted tone-mapped saturation based on what it would have
            // been if the linear saturation were lower.
            let sat_cutoff = 0.18;
            let ts2 = if tv >= 1.0 {
                0.0
            } else if tv as f64 > sat_cutoff {
                let sat_fac = {
                    let t = (tv as f64 - sat_cutoff) / (1.0 - sat_cutoff);
                    1.0 - t.powf(4.0)
                };
                let a = hsv_to_rgb([lh, ls * sat_fac, lv]);
                let b = [self.eval(a[0]), self.eval(a[1]), self.eval(a[2])];
                rgb_to_hsv(b)[1] / sat_fac
            } else {
                ts as f64
            };

            // Return adjusted tone-mapped hsv.
            // (lh as f32, ts2 as f32, tv)
            (
                lh as f32,
                clipped_hsv[1].min(ts2) as f32,
                clipped_hsv[2] as f32,
            )
        });

        (lut_1d, lut_3d)
    }

    pub fn tone_map_transforms(lut_1d_path: &str, lut_3d_path: &str) -> Vec<Transform> {
        vec![
            // Clip colors to 1.0 saturation, so they they blow out properly.
            // TODO: unfortunately, this distorts the luminance of
            // those clipped colors.  Replace this with a 3D LUT
            // transform that does proper HDR gamut clipping once
            // issue #1763 is fixed in OCIO and released.
            Transform::ToHSV,
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                1.0,
                1.0,
                1.0 / 1_000_000_0.0,
            ]))),
            Transform::RangeTransform {
                range_in: (0.0, 1.0),
                range_out: (0.0, 1.0),
                clamp: true,
            },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                1.0,
                1.0,
                1_000_000_0.0,
            ]))),
            Transform::FromHSV,
            // Apply tone map curve.
            Transform::FileTransform {
                src: lut_1d_path.into(),
                interpolation: Interpolation::Linear,
                direction_inverse: true,
            },
            // Apply chroma post-adjustment.
            Transform::ToHSV,
            Transform::FileTransform {
                src: lut_3d_path.into(),
                interpolation: Interpolation::Linear,
                direction_inverse: false,
            },
            Transform::FromHSV,
        ]
    }
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

// /// A tweakable sigmoid function that maps [0.0, 1.0] to [0.0, 1.0].
// ///
// /// - `transition`: the value of `x` where the toe transitions to the shoulder.
// /// - `toe_exp`: the exponent used for the toe part of the curve.
// ///   1.0 = linear, 2.0 = quadratic, etc.
// /// - `shoulder_exp`: the exponent used for the shoulder part of the curve.
// fn s_curve(x: f64, transition: f64, toe_exp: f64, shoulder_exp: f64) -> f64 {
//     // Early-out for off-the-end values.
//     if x <= 0.0 {
//         return 0.0;
//     } else if x >= 1.0 {
//         return 1.0;
//     }

//     // Toe and shoulder curve functions.
//     let toe = |x: f64, scale: f64| -> f64 { x.powf(toe_exp) * scale };
//     let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

//     // Toe and shoulder slopes at the transition.
//     let toe_slope = toe_exp * transition.powf(toe_exp - 1.0);
//     let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

//     // Vertical scale factors needed to make the toe and shoulder meet
//     // at the transition with equal slopes.
//     let s1 = shoulder_slope / toe_slope;
//     let s2 = 1.0 / (1.0 + toe(transition, s1) - shoulder(transition, 1.0));

//     // The full curve output.
//     if x < transition {
//         toe(x, s1 * s2)
//     } else {
//         shoulder(x, s2)
//     }
//     .clamp(0.0, 1.0)
// }

// /// Inverse of `s_curve()`.
// fn s_curve_inv(x: f64, transition: f64, toe_exp: f64, shoulder_exp: f64) -> f64 {
//     // Early-out for off-the-end values.
//     if x <= 0.0 {
//         return 0.0;
//     } else if x >= 1.0 {
//         return 1.0;
//     }

//     // Toe and shoulder curve functions.
//     let toe = |x: f64, scale: f64| -> f64 { x.powf(toe_exp) * scale };
//     let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

//     // Toe and shoulder slopes at the transition.
//     let toe_slope = toe_exp * transition.powf(toe_exp - 1.0);
//     let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

//     // Vertical scale factors needed to make the toe and shoulder meet
//     // at the transition with equal slopes.
//     let s1 = shoulder_slope / toe_slope;
//     let s2 = 1.0 / (1.0 + toe(transition, s1) - shoulder(transition, 1.0));

//     //-------------------------

//     let transition_inv = toe(transition, s1 * s2);

//     let toe_inv = |x: f64, scale: f64| -> f64 {
//         // x.powf(toe_exp) * scale
//         (x / scale).powf(1.0 / toe_exp)
//     };
//     let shoulder_inv = |x: f64, scale: f64| -> f64 {
//         // 1.0 - ((1.0 - x).powf(shoulder_exp) * scale)
//         1.0 - ((1.0 - x) / scale).powf(1.0 / shoulder_exp)
//     };

//     // The full curve output.
//     if x < transition_inv {
//         toe_inv(x, s1 * s2)
//     } else {
//         shoulder_inv(x, s2)
//     }
//     .clamp(0.0, 1.0)
// }

#[cfg(test)]
mod test {
    use super::*;

    // #[test]
    // fn s_curve_round_trip() {
    //     for i in 0..17 {
    //         let t = 0.25;
    //         let p1 = 2.3;
    //         let p2 = 4.5;

    //         let x = i as f64 / 16.0;
    //         let x2 = s_curve_inv(s_curve(x, t, p1, p2), t, p1, p2);
    //         assert!((x - x2).abs() < 0.000_000_1);
    //     }
    // }

    #[test]
    fn filmic_curve_round_trip() {
        let curve = FilmicCurve::new(0.18, 64.0, 0.4, 0.4);
        for i in 0..17 {
            let x = i as f64 / 16.0;
            let x2 = curve.eval_inv(curve.eval(x));
            assert!((x - x2).abs() < 0.000_000_1);
        }
    }
}
