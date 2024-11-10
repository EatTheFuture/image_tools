use colorbox::{
    chroma::Chromaticities,
    lut::{Lut1D, Lut3D},
    matrix::{
        compose, invert, rgb_to_rgb_matrix, rgb_to_xyz_matrix, to_4x4_f32, transform_color,
        xyz_chromatic_adaptation_matrix, xyz_to_rgb_matrix, AdaptationMethod, Matrix,
    },
    transforms::rgb_gamut,
};

use crate::config::{ExponentLUTMapper, Interpolation, Transform};

/// RGB chromaticity coordinates for a custom RGB colorspace that encompasses
/// the spectral locus with a bit of margin all around.  This is used as the
/// space for the LUTs so they can properly handle out-of-gamut colors, which
/// can show up in footage from cameras.
const PARENT_SPACE_RGB_CHROMA: [(f64, f64); 3] = [(0.9, 0.3), (-0.06, 1.04), (0.0, -0.12)];

/// A filmic(ish) tonemapping operator.
///
/// - `exposure`: input exposure adjustment before applying the tone mapping.
///   Input color values are simply multiplied by this, so 1.0 does nothing.
///   Useful for tuning the over-all brightness of tone mappers.
/// - `tone_curve`: the tone mapping curve to use.
/// - `chromaticities`: the RGBW chromaticities of the tone mapping color
///   space (display chromaticities).  This is used for both input and output
///   colors.
/// - `gamut_compression`: how aggressively to compress out-of-gamut colors
///   before tone mapping.  0.0 means a sharp gamut clip, 1.0 means the
///   smoothest roll-off that affects all in-gamut colors.  Reasonable values
///   are between 0.0 and 0.3.
/// - `saturation_preservation`: blends between natural saturation
///   falloff/intensification (0.0) and full saturation preservation from
///   the original image (1.0).  Note that 1.0 looks pretty weird.
///   Reasonable values are between 0.0 and 0.5.
#[derive(Debug, Copy, Clone)]
pub struct Tonemapper {
    exposure: f64,
    tone_curve: ToneCurve,
    saturation_preservation: f64,
    gamut_compression: f64,
    blue_lightness: f64,

    inset_mat: Matrix,
    outset_mat: Matrix,

    // Used for converting to OkLab.
    to_xyz_mat: Matrix,
    from_xyz_mat: Matrix,

    // Used for compressing the original colors to a
    // reasonable space before estimating their hue in OkLab.
    to_rec2020_mat: Matrix,
    from_rec2020_mat: Matrix,

    // Used for LUTs.
    to_parent_mat: Matrix,
    from_parent_mat: Matrix,

    display_rgb_luma_weights: [f64; 3],

    res_1d: usize,
    res_3d: usize,
    mapper_3d: ExponentLUTMapper,
}

const RES_3D_BASE: usize = 48;
const RES_3D_MAX: usize = 97;
const MAPPER_3D_LOW_EXPONENT: f64 = 2.0;
const MAPPER_3D_HIGH_EXPONENT: f64 = 2.0;

impl Default for Tonemapper {
    fn default() -> Tonemapper {
        Tonemapper::new(None, 1.0, ToneCurve::new(1.0, 0.0, 1.0), 0.0, 0.0, 0.0)
    }
}

impl Tonemapper {
    pub fn new(
        chromaticities: Option<Chromaticities>,
        exposure: f64,
        tone_curve: ToneCurve,
        saturation_preservation: f64,
        gamut_compression: f64,
        blue_lightness: f64,
    ) -> Self {
        let chromaticities = chromaticities.unwrap_or(colorbox::chroma::REC709);

        let to_xyz_mat = compose(&[
            rgb_to_xyz_matrix(chromaticities),
            // Since this is just used for converting to OkLab, we adapt
            // to a D65 white point, which is what OkLab uses.
            xyz_chromatic_adaptation_matrix(
                chromaticities.w,
                colorbox::chroma::illuminant::D65,
                AdaptationMethod::Hunt,
            ),
        ]);

        let to_rec2020_mat = compose(&[
            rgb_to_xyz_matrix(chromaticities),
            xyz_chromatic_adaptation_matrix(
                chromaticities.w,
                colorbox::chroma::REC2020.w,
                AdaptationMethod::Hunt,
            ),
            xyz_to_rgb_matrix(colorbox::chroma::REC2020),
        ]);

        let to_parent_mat = rgb_to_rgb_matrix(
            chromaticities,
            Chromaticities {
                r: PARENT_SPACE_RGB_CHROMA[0],
                g: PARENT_SPACE_RGB_CHROMA[1],
                b: PARENT_SPACE_RGB_CHROMA[2],
                // The parent space always uses the same white point
                // as the display space.
                w: chromaticities.w,
            },
        );

        // The value of 0.73 was arrived at experimentally, and seems
        // to give over-all good results.
        let inset_mat = inset_matrix(0.73);

        Tonemapper {
            exposure: exposure,
            tone_curve: tone_curve,
            saturation_preservation: saturation_preservation,
            gamut_compression: gamut_compression,
            blue_lightness: blue_lightness,

            inset_mat: inset_mat,
            outset_mat: invert(inset_mat).unwrap(),
            to_xyz_mat: to_xyz_mat,
            from_xyz_mat: invert(to_xyz_mat).unwrap(),
            to_rec2020_mat: to_rec2020_mat,
            from_rec2020_mat: invert(to_rec2020_mat).unwrap(),
            to_parent_mat: to_parent_mat,
            from_parent_mat: invert(to_parent_mat).unwrap(),
            display_rgb_luma_weights: rgb_to_xyz_matrix(chromaticities)[1],

            res_1d: 1 << 12,
            res_3d: (((RES_3D_BASE as f64)
                * tone_curve.max_output().powf(1.0 / MAPPER_3D_LOW_EXPONENT))
                as usize)
                .min(RES_3D_MAX),
            mapper_3d: ExponentLUTMapper::new(
                MAPPER_3D_LOW_EXPONENT,
                MAPPER_3D_HIGH_EXPONENT,
                tone_curve.max_output(),
                [true, true, true],
                true,
            ),
        }
    }

    /// The main tone mapping function.
    ///
    /// Takes an input open-domain "scene linear" RGB value, and returns
    /// a tone mapped closed-domain "display linear" RGB value.
    pub fn eval(&self, rgb: [f64; 3]) -> [f64; 3] {
        use colorbox::transforms::oklab;

        // Precompute some OkLab color information of the un-tonemapped colors.
        //
        // This is used later to restore the hue of the color.
        let oklab_original = {
            // We gamut clip to Rec2020 first, because OkLab doesn't correctly
            // handle colors that are far outside the spectral locus, and this
            // is a cheap way to ensure that they're inside of it.
            let rec2020 = transform_color(rgb, self.to_rec2020_mat);
            let rec2020_clipped = rgb_gamut::open_domain_clip(rec2020, max_channel(rec2020), 0.8);
            let rgb_clipped = transform_color(rec2020_clipped, self.from_rec2020_mat);
            oklab::from_xyz_d65(transform_color(rgb_clipped, self.to_xyz_mat))
        };

        // Ensure the color is in gamut.
        let rgb_gamut_mapped = {
            let blueness = if rgb[2] > 0.0 {
                ((rgb[2] - rgb[0].max(rgb[1])) / rgb[2].abs()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let blue_fac = blueness * self.blue_lightness;

            let luma_vs_max = lerp(
                dot(rgb, self.display_rgb_luma_weights),
                max_channel(rgb),
                blue_fac,
            );
            let protected = {
                let tmp = self.gamut_compression + (blue_fac * (1.0 - self.gamut_compression));
                1.0 - tmp
            };

            rgb_gamut::open_domain_clip(rgb, luma_vs_max, protected)
        };

        // Apply the tone mapping curve, with an inset gamut.
        //
        // This is the core of the tone mapper.  Everything after this is
        // basically just optional adjustments and fixing the colors up in
        // various ways.
        let rgb_tone_mapped = {
            let rgb_inset = transform_color(rgb_gamut_mapped, self.inset_mat);
            let rgb_compressed = [
                self.eval_1d(rgb_inset[0]),
                self.eval_1d(rgb_inset[1]),
                self.eval_1d(rgb_inset[2]),
            ];
            let rgb_outset = transform_color(rgb_compressed, self.outset_mat);

            rgb_outset
        };

        // Blend between tonemapped saturation and original saturation.
        let rgb_saturation_adjusted = {
            let gray_level_a = dot(rgb_tone_mapped, self.display_rgb_luma_weights);
            let gray_level_b = dot(rgb_gamut_mapped, self.display_rgb_luma_weights);

            if gray_level_a < 1.0e-14 || gray_level_b < 1.0e-14 {
                // If the gray level is too small, bail, since we otherwise divide by it.
                rgb_tone_mapped
            } else {
                // Note: we scale chroma_vector_b so it's relative to the same
                // luminance as chroma_vector_a.
                let chroma_vector_a = vsub(rgb_tone_mapped, [gray_level_a; 3]);
                let chroma_vector_b = vsub(rgb_gamut_mapped, [gray_level_b; 3]);

                let saturation_a = dot(chroma_vector_a, chroma_vector_a).sqrt() / gray_level_a;
                let saturation_b = dot(chroma_vector_b, chroma_vector_b).sqrt() / gray_level_b;

                let rgb_saturated = if saturation_a < 1.0e-10 || saturation_b < 1.0e-10 {
                    // If the saturation is basically zero, then it doesn't
                    // matter, and this avoids numerical instability.
                    rgb_tone_mapped
                } else {
                    // The t factor ensures that colors still blow out towards
                    // white in a pleasing way.
                    let t = if saturation_a < saturation_b {
                        saturation_a / saturation_b
                    } else {
                        0.0
                    };
                    let chroma_scale = lerp(saturation_a, saturation_b, t) / saturation_a;
                    vadd([gray_level_a; 3], vscale(chroma_vector_a, chroma_scale))
                };

                vlerp(rgb_tone_mapped, rgb_saturated, self.saturation_preservation)
            }
        };

        // The inset/outset process as well as re-saturation may have pushed
        // the colors outside of the closed-domain gamut, so we soft-clip them
        // back in.
        let rgb_clipped = {
            // Scale for max output value.
            let scaled = vscale(rgb_saturation_adjusted, 1.0 / self.tone_curve.max_output());

            let open_domain_clipped = rgb_gamut::open_domain_clip(
                scaled,
                dot(scaled, self.display_rgb_luma_weights),
                0.9,
            );

            let closed_domain_clipped = rgb_gamut::closed_domain_clip(
                open_domain_clipped,
                dot(open_domain_clipped, self.display_rgb_luma_weights),
                0.7,
            );

            // Scale for max output value.
            vscale(closed_domain_clipped, self.tone_curve.max_output())
        };

        // Fix hue to match the original input colors.
        let rgb_hue_fixed = {
            let oklab_tonemapped =
                oklab::from_xyz_d65(transform_color(rgb_clipped, self.to_xyz_mat));

            let len1 = ((oklab_tonemapped[1] * oklab_tonemapped[1])
                + (oklab_tonemapped[2] * oklab_tonemapped[2]))
                .sqrt();
            let len2 = ((oklab_original[1] * oklab_original[1])
                + (oklab_original[2] * oklab_original[2]))
                .sqrt();

            let oklab_adjusted = if len2 < 1.0e-10 {
                oklab_tonemapped
            } else {
                [
                    oklab_tonemapped[0],
                    oklab_original[1] * (len1 / len2),
                    oklab_original[2] * (len1 / len2),
                ]
            };

            transform_color(oklab::to_xyz_d65(oklab_adjusted), self.from_xyz_mat)
        };

        // The hue adjustment can slightly push colors out of gamut again.  It's
        // not enough to be visually important, so we just do simple clamping
        // here to push the colors back in.
        [
            rgb_hue_fixed[0].max(0.0).min(self.tone_curve.max_output()),
            rgb_hue_fixed[1].max(0.0).min(self.tone_curve.max_output()),
            rgb_hue_fixed[2].max(0.0).min(self.tone_curve.max_output()),
        ]
    }

    /// Generates a 1D and 3D LUT to apply the tone mapping.
    ///
    /// The LUTs should be applied with the transforms yielded by
    /// `tone_map_transforms()` further below.
    pub fn generate_luts(&self) -> (Lut1D, Lut3D) {
        let lut_1d = Lut1D::from_fn_1(self.res_1d, 0.0, self.tone_curve.max_output() as f32, |n| {
            self.eval_1d_inv(n as f64) as f32
        });

        // The 3d LUT is generated to compensate for the missing bits after just
        // the tone mapping curve is applied per-channel in parent rgb space.
        // It's sort of a "diff" that can be applied afterwards to get the full
        // rgb transform.
        //
        // The generated LUT expects the input values to be in parent space, and
        // produces outputs in display space (the LUT mapping not withstanding).
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

                // NOTE: at this point the color is in linear "parent" colorspace.

                // Figure out what it should map to.
                let rgb_adjusted = self.eval(transform_color(rgb_linear, self.from_parent_mat));

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

    /// Generates the OCIO transforms needed for this tone mapper.
    ///
    /// Should be used together with `generate_luts()`, above.
    pub fn tone_map_transforms(&self, lut_1d_path: &str, lut_3d_path: &str) -> Vec<Transform> {
        let mut transforms = Vec::new();

        // Convert to parent space and gamut clip to that space.
        transforms.extend([Transform::MatrixTransform(to_4x4_f32(self.to_parent_mat))]);
        transforms.extend(crate::gamut_map::hsv_gamut_clip());

        // Apply tone curve.
        transforms.extend([Transform::FileTransform {
            src: lut_1d_path.into(),
            interpolation: Interpolation::Linear,
            direction_inverse: true,
        }]);

        // Apply 3D LUT that does the final adjustments and maps the colors to
        // display space.
        transforms.extend(self.mapper_3d.transforms_lut_3d(lut_3d_path));

        // Gamut clip after
        transforms.extend(crate::gamut_map::hsv_gamut_clip());

        transforms
    }

    //------------
    // Internals.

    fn eval_1d(&self, x: f64) -> f64 {
        if x <= 0.0 {
            0.0
        } else {
            self.tone_curve
                .eval(x * self.exposure)
                .min(self.tone_curve.max_output())
        }
    }

    fn eval_1d_inv(&self, y: f64) -> f64 {
        if y <= 0.0 {
            0.0
        } else if y >= self.tone_curve.max_output() {
            // Infinity would actually be correct here, but it leads
            // to issues in the generated LUTs.  So instead we just
            // choose an extremely large finite number that fits
            // within an f32 (since later processing may be done in
            // f32).
            (f32::MAX / 2.0) as f64
        } else {
            self.tone_curve.eval_inv(y) / self.exposure
        }
    }
}

/// A "filmic" tone mapping curve.
///
/// The basic idea behind this is to layer a toe function underneath
/// a generalized Reinhard function.  This has no particular basis in
/// anything, but in practice produces pleasing results that are easy
/// to adjust for different looks.
///
/// Note: this maps [0.0, inf] to [0.0, 1.0].
///
/// https://www.desmos.com/calculator/pfzvawfekp
#[derive(Debug, Copy, Clone)]
pub struct ToneCurve {
    toe_slope: f64,
    toe_extent: f64,
    shoulder_start: f64,
    shoulder_ceiling: f64,
    shoulder_power: f64,
}

impl ToneCurve {
    /// - `ceiling`: the maximum output channel value.  For example, set
    ///   to 1.0 for SDR, and > 1.0 for HDR.
    /// - `toe_power`: the strength of the toe.  0.0 applies no toe, leaving the
    ///   values completely alone, and larger values compress the darks more and
    ///   more.  0.5 is a reasonable default.
    ///   contrast, 1.0 is neutral, and > 1.0 washes things out.
    /// - `shoulder_power`: the strength of the shoulder.  0.0 is equivalent to
    ///   a linear clamped shoulder, and larger values make the shoulder
    ///   progressively softer and higher dynamic range. 1.0 is a
    ///   reasonable default.
    pub fn new(ceiling: f64, toe_power: f64, shoulder_power: f64) -> ToneCurve {
        assert!(toe_power >= 0.0);
        assert!(shoulder_power >= 0.0);

        ToneCurve {
            toe_slope: (1.0 - toe_power).max(0.0),
            toe_extent: toe_power * 0.1,
            shoulder_start: 0.1,
            shoulder_ceiling: ceiling,
            shoulder_power: shoulder_power,
        }
    }

    pub fn max_output(&self) -> f64 {
        self.shoulder_ceiling
    }

    pub fn eval(&self, x: f64) -> f64 {
        self.shoulder(self.toe(x))
    }

    pub fn eval_inv(&self, x: f64) -> f64 {
        if x >= (self.shoulder_ceiling * 0.999_999_999_999) {
            f64::INFINITY
        } else {
            self.toe_inv(self.shoulder_inv(x))
        }
    }

    //------------
    // Internals.

    const TOE_LINEAR_POINT: f64 = 1.0e+4;

    fn toe(&self, x: f64) -> f64 {
        // Special cases and validation.
        if x < 0.0 {
            // Do a flipped toe for negative values.
            return -self.toe(-x);
        } else if self.toe_extent <= 0.0 || x > Self::TOE_LINEAR_POINT {
            return x;
        }

        let tmp = (1.0 - self.toe_slope) * x * (-x / self.toe_extent).exp2();
        x - tmp
    }

    /// Inverse of `toe()`.  There is no analytic inverse, so we do it
    /// numerically.
    fn toe_inv(&self, y: f64) -> f64 {
        // Special cases and validation.
        if y < 0.0 {
            // Do a flipped toe for negative values.
            return -self.toe_inv(-y);
        } else if y > Self::TOE_LINEAR_POINT {
            // Really far out it's close enough to linear to not matter.
            return y;
        }

        // A binary search with a capped number of iterations.
        // Something like newton iteration would be faster, but I
        // can't be bothered to figure that out right now, and this
        // isn't performance critical.
        const RELATIVE_ERROR_THRESHOLD: f64 = 1.0e-8;
        let mut min = 0.0;
        let mut max = Self::TOE_LINEAR_POINT;
        for _ in 0..64 {
            let x = (min + max) * 0.5;
            let y2 = self.toe(x);
            if y >= y2 {
                min = x;
                if ((y - y2) / y) <= RELATIVE_ERROR_THRESHOLD {
                    break;
                }
            } else {
                max = x;
            }
        }

        min
    }

    fn shoulder(&self, x: f64) -> f64 {
        // Range adjustment for linear segment and ceiling.
        let x = (x - self.shoulder_start) / (self.shoulder_ceiling - self.shoulder_start);

        // Actual curve.
        let y = reinhard(x, self.shoulder_power);

        // Range adjustment for linear segment and ceiling.
        y * (self.shoulder_ceiling - self.shoulder_start) + self.shoulder_start
    }

    /// Inverse of `shoulder()`.
    fn shoulder_inv(&self, y: f64) -> f64 {
        // Range adjustment for linear segment and ceiling.
        let y = (y - self.shoulder_start) / (self.shoulder_ceiling - self.shoulder_start);

        // Actual curve.
        let x = reinhard_inv(y, self.shoulder_power);

        // Range adjustment for linear segment and ceiling.
        x * (self.shoulder_ceiling - self.shoulder_start) + self.shoulder_start
    }
}

/// Computes a matrix that insets/outsets the rgb primaries
/// towards/away from the white point.
///
/// `factor`: the inset/outset amount.  Less than 1.0 is inset,
/// more is outset.  0.0 is total desaturation.
fn inset_matrix(factor: f64) -> Matrix {
    let a = factor * (2.0 / 3.0) + (1.0 / 3.0);
    let b = (1.0 - a) * 0.5;

    [[a, b, b], [b, a, b], [b, b, a]]
}

/// Computes the CIE xy chromaticity coordinates of a pure wavelength of light.
///
/// `wavelength` is given in nanometers.
fn wavelength_to_xy(wavelength: f64) -> (f64, f64) {
    use colorbox::{tables::cie_1931_xyz as xyz, transforms::xyz_to_xyy};

    let t = ((wavelength - xyz::MIN_WAVELENGTH as f64)
        / (xyz::MAX_WAVELENGTH as f64 - xyz::MIN_WAVELENGTH as f64))
        .clamp(0.0, 1.0);
    let ti = t * (xyz::X.len() - 1) as f64;

    let i1 = ti as usize;
    let i2 = (i1 + 1).min(xyz::X.len() - 1);
    let a = if i1 == i2 {
        0.0
    } else {
        (ti - i1 as f64) / (i2 - i1) as f64
    }
    .clamp(0.0, 1.0) as f32;

    let x = (xyz::X[i1] * (1.0 - a)) + (xyz::X[i2] * a);
    let y = (xyz::Y[i1] * (1.0 - a)) + (xyz::Y[i2] * a);
    let z = (xyz::Z[i1] * (1.0 - a)) + (xyz::Z[i2] * a);

    let xyy = xyz_to_xyy([x as f64, y as f64, z as f64]);

    (xyy[0], xyy[1])
}

/// Generalized Reinhard curve.
///
/// Maps [0, infinity] to [0, 1], and leaves < 0 untouched.
///
/// - `p`: a tweaking parameter that affects the shape of the curve, in (0.0,
///   inf].  Larger values make it gentler, lower values make it sharper.  1.0 =
///   standard Reinhard, 0.0 = linear in [0,1].
#[inline(always)]
fn reinhard(x: f64, p: f64) -> f64 {
    // Leave negavite values alone.
    if x <= 0.0 {
        return x;
    }

    // Special case so we get linear at `p == 0` instead of undefined.
    // Negative `p` is unsupported, so treat like zero as well.
    if p <= 0.0 {
        return x.min(1.0);
    }

    // First part of actual generalized Reinhard.
    let tmp = x.powf(-1.0 / p);

    // Special cases for numerical stability.
    if tmp > 1.0e15 {
        return x;
    } else if tmp < 1.0e-15 {
        return 1.0;
    }

    // Second part of actual generalized Reinhard.
    (tmp + 1.0).powf(-p)
}

/// Inverse of `reinhard()`.
#[inline(always)]
fn reinhard_inv(x: f64, p: f64) -> f64 {
    // Make out-of-range numbers do something reasonable.
    if x <= 0.0 {
        // Leave negative values alone.
        return x;
    } else if x >= 1.0 {
        // There isn't really anything meaningful to do beyond 1.0, but this is
        // at least consistent and does the right thing at the boundary.
        return std::f64::INFINITY;
    }

    // Special case so we get linear at `p == 0` instead of undefined.
    // Negative `p` is unsupported, so clamp.
    if p <= 0.0 {
        return x;
    }

    // First part of actual generalized Reinhard inverse.
    let tmp = x.powf(-1.0 / p);

    // Special case for numerical stability.
    if tmp > 1.0e15 {
        return x;
    }

    // Second part of actual generalized Reinhard inverse.
    (tmp - 1.0).powf(-p)
}

/// A [0,1] -> [0,1] mapping, with 0.5 biased up or down.
///
/// `b` is what 0.5 maps to.  Setting it to 0.5 results in a linear
/// mapping.
///
/// Note: `bias()` is its own inverse: simply passing `1.0 - b` instead
/// of `b` gives the inverse.
///
/// https://www.desmos.com/calculator/prxpsydjug
#[inline(always)]
fn bias(x: f64, b: f64) -> f64 {
    if b <= 0.0 {
        0.0
    } else if b >= 1.0 {
        1.0
    } else {
        x / ((((1.0 / b) - 2.0) * (1.0 - x)) + 1.0)
    }
}

fn soft_min(a: f64, b: f64, softness: f64) -> f64 {
    let tmp = -a + b;
    (-a - b + ((tmp * tmp) + (softness * softness)).sqrt()) * -0.5
}

fn soft_max(a: f64, b: f64, softness: f64) -> f64 {
    let tmp = a - b;
    (a + b + ((tmp * tmp) + (softness * softness)).sqrt()) * 0.5
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

fn vlerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

fn max_channel(rgb: [f64; 3]) -> f64 {
    rgb[0].max(rgb[1]).max(rgb[2])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] * b[0]) + (a[1] * b[1]) + (a[2] * b[2])
}

#[inline(always)]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    (a * (1.0 - t)) + (b * t)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tone_toe_round_trip() {
        for power in [0.0, 0.5, 1.0, 1.5, 2.0] {
            let tc = ToneCurve::new(2.0, power, 1.4);

            for i in 0..4096 {
                // Non-linear mapping for x so we test both very
                // small and very large values.
                let x = ((i as f64 / 100.0).exp2() - 1.0) / 10000.0;

                // Forward.
                let y = tc.toe(x);
                let x2 = tc.toe_inv(y);
                if x == 0.0 {
                    assert!(x2 == 0.0);
                } else {
                    assert!(((x - x2).abs() / x) < 0.000_000_1);
                }

                // Reverse.
                let y = tc.toe_inv(x);
                let x2 = tc.toe(y);
                if x == 0.0 {
                    assert!(x2 == 0.0);
                } else {
                    assert!(((x - x2).abs() / x) < 0.000_000_1);
                }

                let x = -x;

                // Forward negative.
                let y = tc.toe(x);
                let x2 = tc.toe_inv(y);
                if x == 0.0 {
                    assert!(x2 == 0.0);
                } else {
                    assert!(((x - x2).abs() / x.abs()) < 0.000_000_1);
                }

                // Reverse negative.
                let y = tc.toe_inv(x);
                let x2 = tc.toe(y);
                if x == 0.0 {
                    assert!(x2 == 0.0);
                } else {
                    assert!(((x - x2).abs() / x.abs()) < 0.000_000_1);
                }
            }
        }
    }

    #[test]
    fn tone_curve_round_trip() {
        let tc = ToneCurve::new(2.0, 0.25, 1.4);
        for i in 0..4096 {
            // Forward.
            let x = i as f64 / 64.0;
            let y = tc.eval(x);
            let x2 = tc.eval_inv(y);
            assert!((x - x2).abs() < 0.000_001);

            // Reverse.
            let x = i as f64 / 4096.0;
            let y = tc.eval_inv(x);
            let x2 = tc.eval(y);
            assert!((x - x2).abs() < 0.000_001);

            let x = -x;

            // Forward negative.
            let x = i as f64 / 64.0;
            let y = tc.eval(x);
            let x2 = tc.eval_inv(y);
            assert!((x - x2).abs() < 0.000_001);

            // Reverse negative.
            let x = i as f64 / 4096.0;
            let y = tc.eval_inv(x);
            let x2 = tc.eval(y);
            assert!((x - x2).abs() < 0.000_001);
        }
    }

    #[test]
    fn tonemap_1d_round_trip() {
        let tone_curve = ToneCurve::new(2.0, 0.25, 1.4);
        let satfx = (0.4, 0.6);
        let min_smooth = 0.25;
        let tm = Tonemapper::new(1.1, tone_curve, None, satfx, min_smooth);
        for i in 0..=32 {
            let x = (i as f64 - 8.0) / 4.0;
            let x = i as f64 / 16.0;
            let x2 = tm.eval_1d(tm.eval_1d_inv(x));
            assert!((x - x2).abs() < 0.000_001);
        }
    }

    #[test]
    fn reinhard_round_trip() {
        for i in 0..=32 {
            for p in 0..4 {
                let x = (i as f64 - 8.0) / 4.0;
                let p = p as f64 / 8.0;
                if p <= 0.0 && x >= 1.0 {
                    continue;
                }
                let x2 = reinhard_inv(reinhard(x, p), p);
                assert!((x - x2).abs() < 0.000_001);
            }
        }
    }
}
