use colorbox::{
    lut::{Lut1D, Lut3D},
    matrix,
    transforms::rgb_gamut_intersect,
};

use crate::config::{ExponentLUTMapper, Interpolation, Transform};

const K_LIMIT: f64 = 0.00001;

/// A simple filmic tonemapping curve.
///
/// The basic idea behind this is to layer a sigmoid contrast function
/// on top of the Reinhard function.  This has no real basis in the
/// actual physics of film stock, but in practice produces pleasing
/// results that are easy to adjust for different looks.
///
/// - `contrast`: how "contrasty" the look should be.  Values between 1.0
///   and 4.0 give fairly normal looks, while higher starts to look more
///   high contrast.  Values less than 0.0 aren't meaningful.
/// - `fixed_point`: the value of `x` that should (approximately) map to
///   itself.  For example, you might set this to 0.18 (18% gray) so that
///   colors of that brightness remain the same.
/// - `luminance_ceiling`: the luminance level that maps to 1.0 in the
///   output.  Typically you want this to be a large-ish number
///   (e.g. > 30), as it represents the top end of the dynamic range.
///   It can be useful to think in terms of photographic stops: if you
///   want 6 stops of dynamic range above 1.0, then this should be 2^6,
///   or 64.  In practice, this doesn't have much impact on the look
///   beyond maybe 14 stops or so.
/// - `exposure`: input exposure adjustment before applying the tone mapping.
///   Input color values are just multiplied by this.  Useful for tuning
///   different tone mappers to match general brightness without altering
///   the actual tone mapping curve.
///
/// Note that setting `contrast` to zero creates the classic Reinhard curve.
///
/// Returns the tonemapped value, always in the range [0.0, 1.0].
#[derive(Debug, Copy, Clone)]
pub struct FilmicCurve {
    k: f64,
    b: f64,
    sigmoid_scale_y: f64,
    reinhard_scale_x: f64,
    scale_y: f64,
    luminance_ceiling: f64,
    exposure: f64,

    res_1d: usize,
    res_3d: usize,
    mapper_3d: ExponentLUTMapper,
}

impl Default for FilmicCurve {
    fn default() -> FilmicCurve {
        FilmicCurve {
            k: 0.0,
            b: 1.0,
            sigmoid_scale_y: 1.0,
            reinhard_scale_x: 1.0,
            scale_y: 1.0,
            luminance_ceiling: 4096.0,
            exposure: 1.0,

            res_1d: 2,
            res_3d: 2,
            mapper_3d: ExponentLUTMapper::new(1.0, 2, 1.0, [false, false, true]),
        }
    }
}

impl FilmicCurve {
    pub fn new(contrast: f64, fixed_point: f64, luminance_ceiling: f64, exposure: f64) -> Self {
        let k = contrast.sqrt();
        let b = fixed_point.log(0.5);
        let sigmoid_scale_y = if k < K_LIMIT {
            1.0
        } else {
            1.0 / sigmoid(1.0, k)
        };
        let reinhard_scale_x = reinhard_inv(fixed_point) / fixed_point;
        let scale_y =
            1.0 / unscaled_filmic(luminance_ceiling, k, sigmoid_scale_y, reinhard_scale_x, b);

        let res_1d = 1 << 14;
        let res_3d = 32 + 1;

        FilmicCurve {
            k: k,
            b: b,
            sigmoid_scale_y: sigmoid_scale_y,
            reinhard_scale_x: reinhard_scale_x,
            scale_y: scale_y,
            luminance_ceiling: luminance_ceiling,
            exposure: exposure,

            res_1d: res_1d,
            res_3d: res_3d,
            mapper_3d: ExponentLUTMapper::new(3.0, res_3d, 7.0, [true, true, true]),
        }
    }

    pub fn eval(&self, x: f64) -> f64 {
        if x <= 0.0 {
            0.0
        } else {
            unscaled_filmic(
                x * self.exposure,
                self.k,
                self.sigmoid_scale_y,
                self.reinhard_scale_x,
                self.b,
            ) * self.scale_y
        }
    }

    pub fn eval_inv(&self, y: f64) -> f64 {
        if y <= 0.0 {
            0.0
        } else if y >= 1.0 {
            self.luminance_ceiling
        } else {
            unscaled_filmic_inv(
                y / self.scale_y,
                self.k,
                self.sigmoid_scale_y,
                self.reinhard_scale_x,
                self.b,
            ) / self.exposure
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

#[inline(always)]
fn reinhard(x: f64) -> f64 {
    x / (1.0 + x)
}

#[inline(always)]
fn reinhard_inv(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        std::f64::INFINITY
    } else {
        (1.0 / (1.0 - x)) - 1.0
    }
}

/// A sigmoid that maps -inf,+inf to -1,+1.
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

#[inline(always)]
fn unscaled_filmic(x: f64, k: f64, sigmoid_scale_y: f64, reinhard_scale_x: f64, b: f64) -> f64 {
    // Reinhard.
    let r = reinhard(x * reinhard_scale_x);

    // Contrast sigmoid.
    let n = (2.0 * r.powf(1.0 / b)) - 1.0;
    let m = if k < K_LIMIT {
        n
    } else {
        sigmoid(n, k) * sigmoid_scale_y
    };
    ((m + 1.0) / 2.0).powf(b)
}

#[inline(always)]
fn unscaled_filmic_inv(x: f64, k: f64, sigmoid_scale_y: f64, reinhard_scale_x: f64, b: f64) -> f64 {
    // Contrast sigmoid.
    let m = (x.powf(1.0 / b) * 2.0) - 1.0;
    let n = if k < K_LIMIT {
        m
    } else {
        sigmoid_inv(m / sigmoid_scale_y, k)
    };
    let r = ((n + 1.0) / 2.0).powf(b);

    // Reinhard.
    reinhard_inv(r) / reinhard_scale_x
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
