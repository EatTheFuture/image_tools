use colorbox::{
    lut::{Lut1D, Lut3D},
    matrix,
    transforms::rgb_gamut_intersect,
};

use crate::config::{ExponentLUTMapper, Interpolation, Transform};

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
    fixed_point: f64,
    luminance_ceiling: f64,
    res_1d: usize,
    res_3d: usize,
    mapper_3d: ExponentLUTMapper,
}

impl Default for FilmicCurve {
    fn default() -> FilmicCurve {
        FilmicCurve {
            a: 0.0,
            b: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            fixed_point: 0.18,
            luminance_ceiling: 1.0,
            res_1d: 2,
            res_3d: 2,
            mapper_3d: ExponentLUTMapper::new(1.0, 2, 1.0, [false, false, true]),
        }
    }
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
                ..Default::default()
            }
            .eval_inv(fixed_point);
        let scale_y = 1.0
            / FilmicCurve {
                a: a,
                b: b,
                scale_x: scale_x,
                scale_y: 1.0,
                ..Default::default()
            }
            .eval(luminance_ceiling);

        let exp = 3.0;
        let res_1d = 1 << 14;
        let res_3d = 32 + 1;

        FilmicCurve {
            a: a,
            b: b,
            scale_x: scale_x,
            scale_y: scale_y,
            fixed_point: fixed_point,
            luminance_ceiling: luminance_ceiling,
            res_1d: res_1d,
            res_3d: res_3d,
            mapper_3d: ExponentLUTMapper::new(exp, res_3d, 7.0, [true, true, true]),
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

    /// Generates a 1D and 3D LUT to apply the filmic tone mapping.
    ///
    /// The LUTs should be applied with the transforms yielded by
    /// `tone_map_transforms()` further below.
    pub fn generate_luts(&self) -> (Lut1D, Lut3D) {
        use crate::hsv_lut::make_hsv_lut;
        use colorbox::transforms::ocio::{hsv_to_rgb, rgb_to_hsv};

        let lut_1d = Lut1D::from_fn_1(self.res_1d, 0.0, 1.0, |n| self.eval_inv(n as f64) as f32);

        let lut_3d = Lut3D::from_fn(
            [self.res_3d; 3],
            [0.0; 3],
            [self.mapper_3d.lut_max() as f32; 3],
            |(r, g, b)| {
                const LUMA_WEIGHTS: [f64; 3] = [2.0 / 12.0, 8.0 / 12.0, 2.0 / 12.0];
                let luma = |rgb: [f64; 3]| {
                    ((rgb[0] * LUMA_WEIGHTS[0])
                        + (rgb[1] * LUMA_WEIGHTS[1])
                        + (rgb[2] * LUMA_WEIGHTS[2]))
                        / LUMA_WEIGHTS.iter().sum::<f64>()
                };

                // Convert out of LUT space.
                let rgb = self.mapper_3d.from_lut([r as f64, g as f64, b as f64]);

                // Gray point.
                let gp = {
                    let l = luma(rgb);
                    [l, l, l]
                };

                // HDR gamut-clip to max 1.0 saturation (so no RGB channels are negative).
                let rgb_clipped = rgb_gamut_intersect(rgb, gp, false, false);

                // Vector such that `gp + rgb_vec = rgb_clipped`.
                let rgb_vec = vsub(rgb_clipped, gp);

                // // Desaturate a little, so all colors blow out.
                // let rgb_clipped = {
                //     let bottom = self.luminance_ceiling / 32.0;
                //     let top = self.luminance_ceiling * 8.0;
                //     let power = (vmax(rgb_clipped) - bottom).max(0.0) / (top - bottom);
                //     vadd(gp, vscale(rgb_vec, (1.0 - power).powf(2.0)))
                // };

                //---------------------------------------------
                // Tone mapping space.

                // Tone mapped rgb and gray point.
                let rgb_tm = [
                    self.eval(rgb_clipped[0]),
                    self.eval(rgb_clipped[1]),
                    self.eval(rgb_clipped[2]),
                ];
                let gp_tm = {
                    let l = self.eval(gp[0]);
                    // let l = luma(rgb_tm);
                    [l, l, l]
                };
                let rgb_vec_tm = vsub(rgb_tm, gp_tm);

                let rgb_2 = vadd(gp_tm, vscale(rgb_vec, vlen(rgb_vec_tm) / vlen(rgb_vec)));

                // LDR gamut-clip.
                let rgb_tm_clipped = rgb_gamut_intersect(rgb_2, gp_tm, true, true);

                //---------------------------------------------
                // Back to linear space.

                // Reverse tone-map.
                let rgb_final = [
                    self.eval_inv(rgb_tm_clipped[0]),
                    self.eval_inv(rgb_tm_clipped[1]),
                    self.eval_inv(rgb_tm_clipped[2]),
                ];

                // Back to LUT space.
                let rgb = self.mapper_3d.to_lut(rgb_final);

                (rgb[0] as f32, rgb[1] as f32, rgb[2] as f32)
            },
        );

        (lut_1d, lut_3d)
    }

    pub fn tone_map_transforms(&self, lut_1d_path: &str, lut_3d_path: &str) -> Vec<Transform> {
        let mut transforms = Vec::new();

        // Clip colors to 1.0 saturation, so they're within the range
        // of our LUTs.
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

        // Apply chroma LUT.
        transforms.extend(self.mapper_3d.transforms_lut_3d(lut_3d_path));

        // Apply tone map curve.
        transforms.extend([Transform::FileTransform {
            src: lut_1d_path.into(),
            interpolation: Interpolation::Linear,
            direction_inverse: true,
        }]);

        transforms
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
