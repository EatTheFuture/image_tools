//! Based on https://github.com/EaryChow/AgX_LUT_Gen
//!
//! Aims to match Blender's AgX implementation.

use colorbox::{
    chroma::{self, Chromaticities},
    matrix::{self, Matrix},
    transforms::{
        ocio::{hsv_to_rgb, rgb_to_hsv},
        rgb_gamut,
    },
};

pub fn agx_base_rec2020() -> AgX {
    const MID_GRAY: f64 = 0.18;
    const NORMALIZED_LOG2_MINIMUM: f64 = -10.0;
    const NORMALIZED_LOG2_MAXIMUM: f64 = 6.5;

    // Sigmoid definition.
    let sigmoid = {
        let x_pivot =
            NORMALIZED_LOG2_MINIMUM.abs() / (NORMALIZED_LOG2_MAXIMUM - NORMALIZED_LOG2_MINIMUM);
        let y_pivot = MID_GRAY.powf(1.0 / 2.4);

        curve::Sigmoid::new(
            [x_pivot, y_pivot],
            2.4,
            [0.0; 2],
            [1.5; 2],
            [[0.0; 2], [1.0; 2]],
        )
    };

    AgX::new(
        chroma::REC709,
        [3.0, -1.0, -1.0],
        [0.4, 0.22, 0.13],
        [0.0, 0.0, 0.0],
        [0.4, 0.22, 0.04],
        MID_GRAY,
        [NORMALIZED_LOG2_MINIMUM, NORMALIZED_LOG2_MAXIMUM],
        sigmoid,
        [0.2658180370250449, 0.59846986045365, 0.1357121025213052],
        40.0,
    )
}

#[derive(Debug, Copy, Clone)]
pub struct AgX {
    log_range: [f64; 2],
    mid_gray: f64,

    inset_matrix: Matrix,
    outset_matrix: Matrix,
    luminance_coeffs: [f64; 3],

    sigmoid: curve::Sigmoid,

    mix_percent: f64,
}

impl AgX {
    pub fn new(
        color_space: Chromaticities,
        inset_rotations: [f64; 3],
        inset_insets: [f64; 3],
        outset_rotations: [f64; 3],
        outset_insets: [f64; 3],
        mid_gray: f64,
        log_range: [f64; 2],
        sigmoid: curve::Sigmoid,
        luminance_coeffs: [f64; 3],
        mix_percent: f64,
    ) -> Self {
        let inset_matrix = matrix::rgb_to_rgb_matrix(
            space::create_working_space(inset_rotations, inset_insets, color_space),
            color_space,
        );
        let outset_matrix = matrix::rgb_to_rgb_matrix(
            color_space,
            space::create_working_space(outset_rotations, outset_insets, color_space),
        );

        Self {
            log_range: log_range,
            mid_gray: mid_gray,
            inset_matrix: inset_matrix,
            outset_matrix: outset_matrix,
            luminance_coeffs: luminance_coeffs,
            sigmoid: sigmoid,
            mix_percent: mix_percent,
        }
    }

    pub fn eval_1d(&self, x: f64) -> f64 {
        let x = curve::log2_encoding(x, self.mid_gray, self.log_range[0], self.log_range[1]);
        let x = self.sigmoid.eval(x);
        x.powf(2.4)
    }

    pub fn eval(&self, col: [f64; 3]) -> [f64; 3] {
        fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
            (a[0] * b[0]) + (a[1] * b[1]) + (a[2] * b[2])
        }

        // Apply open-domain gamut clip.
        // Note: in the original from Eary, this was instead the "lower guard
        // rail" application.  I've substituted a simple luminance-based
        // gamut clip, as I believe that's essentially what "lower guard
        // rail" is, just with an unnecessarily complex implementation.
        let col = rgb_gamut::open_domain_clip(col, dot(col, self.luminance_coeffs).max(0.0), 0.0);

        let col = matrix::transform_color(col, self.inset_matrix);

        let pre_form_hsv = rgb_to_hsv(col);

        let col = [
            self.eval_1d(col[0]),
            self.eval_1d(col[1]),
            self.eval_1d(col[2]),
        ];

        // Record post-sigmoid chroma angle.
        let col = rgb_to_hsv(col);

        // Mix pre-formation chroma angle with post formation chroma angle.
        let hue = {
            // This looks involved, but is ultimately just a lerp between two
            // hue angles.  The complication is that since hue loops back on
            // itself, we need to ensure that we're interpolating on the
            // shortest path between the two hue angles.
            let h1 = pre_form_hsv[0];
            let mut h2 = col[0];
            while (h1 - h2) > 0.5 {
                h2 += 1.0;
            }
            while (h1 - h2) < 0.5 {
                h2 -= 1.0;
            }
            let t = self.mix_percent / 100.0;
            let mut h3 = (h1 * (1.0 - t)) + (h2 * t);
            while h3 < 0.0 {
                h3 += 1.0;
            }
            while h3 > 1.0 {
                h3 -= 1.0;
            }
            h3
        };

        let col = hsv_to_rgb([hue, col[1], col[2]]);

        // Apply outset to make the result more chroma-laden.
        let col = matrix::transform_color(col, self.outset_matrix);

        // Do a final closed-domain clip to ensure all colors are in-gamut.
        let luma = dot(col, self.luminance_coeffs).max(0.0);
        rgb_gamut::closed_domain_clip(rgb_gamut::open_domain_clip(col, luma, 0.0), luma, 0.0)
    }
}

mod curve {
    pub fn log2_encoding(lin: f64, middle_grey: f64, min_exposure: f64, max_exposure: f64) -> f64 {
        let lg2 = (lin / middle_grey).log2();
        (lg2 - min_exposure) / (max_exposure - min_exposure)
    }

    pub fn log2_decoding(
        log_norm: f64,
        middle_grey: f64,
        min_exposure: f64,
        max_exposure: f64,
    ) -> f64 {
        let lg2 = log_norm * (max_exposure - min_exposure) + min_exposure;
        2.0_f64.powf(lg2) * middle_grey
    }

    #[derive(Debug, Copy, Clone)]
    pub struct Sigmoid {
        // Toe.
        transition_toe: [f64; 2],
        scale_toe: f64,
        power_toe: f64,
        // Linear segment.
        slope: f64,
        intercept: f64,
        // Shoulder.
        transition_shoulder: [f64; 2],
        scale_shoulder: f64,
        power_shoulder: f64,
    }

    impl Sigmoid {
        pub fn new(
            // Pivot coordinates x and y for the fulcrum.
            pivots: [f64; 2],
            // Slope of linear portion.
            slope: f64,
            // Length of transition toward the toe and shoulder.
            lengths: [f64; 2],
            // Exponential power of the toe and shoulder regions.
            powers: [f64; 2],
            // Intersection limit coordinates x and y for the toe and shoulder.
            limits: [[f64; 2]; 2],
        ) -> Self {
            // Toe.
            let transition_toe_x = linear_breakpoint(-lengths[0], slope, pivots[0]);
            let transition_toe_y = linear_breakpoint(slope * -lengths[0], slope, pivots[1]);
            let inverse_transition_toe_x = 1.0 - transition_toe_x;
            let inverse_transition_toe_y = 1.0 - transition_toe_y;
            let inverse_limit_toe_x = 1.0 - limits[0][0];
            let inverse_limit_toe_y = 1.0 - limits[0][1];
            let scale_toe = -scale(
                inverse_limit_toe_x,
                inverse_limit_toe_y,
                inverse_transition_toe_x,
                inverse_transition_toe_y,
                powers[0],
                slope,
            );

            // Linear segment.
            let intercept = transition_toe_y - (slope * transition_toe_x);

            // Shoulder.
            let transition_shoulder_x = linear_breakpoint(lengths[1], slope, pivots[0]);
            let transition_shoulder_y = linear_breakpoint(slope * lengths[1], slope, pivots[1]);
            let scale_shoulder = scale(
                limits[1][0],
                limits[1][1],
                transition_shoulder_x,
                transition_shoulder_y,
                powers[1],
                slope,
            );

            Self {
                // Toe.
                transition_toe: [transition_toe_x, transition_toe_y],
                scale_toe: scale_toe,
                power_toe: powers[0],
                // Linear segment.
                slope: slope,
                intercept: intercept,
                // Shoulder.
                transition_shoulder: [transition_shoulder_x, transition_shoulder_y],
                scale_shoulder: scale_shoulder,
                power_shoulder: powers[1],
            }
        }

        pub fn eval(&self, x: f64) -> f64 {
            if x < self.transition_toe[0] {
                exponential_curve(
                    x,
                    self.scale_toe,
                    self.slope,
                    self.power_toe,
                    self.transition_toe[0],
                    self.transition_toe[1],
                )
            } else if x <= self.transition_shoulder[0] {
                line(x, self.slope, self.intercept)
            } else {
                exponential_curve(
                    x,
                    self.scale_shoulder,
                    self.slope,
                    self.power_shoulder,
                    self.transition_shoulder[0],
                    self.transition_shoulder[1],
                )
            }
        }
    }

    //------------
    // Utilities.

    fn linear_breakpoint(numerator: f64, slope: f64, coordinate: f64) -> f64 {
        let denominator = (slope.powf(2.0) + 1.0).powf(1.0 / 2.0);
        (numerator / denominator) + coordinate
    }

    fn line(x: f64, slope: f64, intercept: f64) -> f64 {
        (slope * x) + intercept
    }

    fn scale(
        limit_x: f64,
        limit_y: f64,
        transition_x: f64,
        transition_y: f64,
        power: f64,
        slope: f64,
    ) -> f64 {
        let term_a = (slope * (limit_x - transition_x)).powf(-power);
        let term_b =
            ((slope * (limit_x - transition_x)) / (limit_y - transition_y)).powf(power) - 1.0;
        (term_a * term_b).powf(-1.0 / power)
    }

    fn exponential(x: f64, power: f64) -> f64 {
        x / (1.0 + x.powf(power)).powf(1.0 / power)
    }

    fn exponential_curve(
        x: f64,
        scale: f64,
        slope: f64,
        power: f64,
        transition_x: f64,
        transition_y: f64,
    ) -> f64 {
        (scale * exponential((slope * (x - transition_x)) / scale, power)) + transition_y
    }
}

mod space {
    use colorbox::chroma::Chromaticities;

    /// Create AgX working color spaces.
    ///
    /// Adapted from:
    /// https://github.com/sobotka/SB2383-Configuration-Generation/blob/e507709c4dc0/working_space.py
    /// by Troy Sobotka.
    pub fn create_working_space(
        primaries_rotate: [f64; 3],
        primaries_inset: [f64; 3],
        colorspace_in: Chromaticities,
    ) -> Chromaticities {
        // Rotate the primaries. Positive values are counter clockwise.
        let rotated_out_red = rotate2d(
            colorspace_in.r,
            colorspace_in.w,
            degrees_to_radians(primaries_rotate[0]),
        );
        let rotated_out_green = rotate2d(
            colorspace_in.g,
            colorspace_in.w,
            degrees_to_radians(primaries_rotate[1]),
        );
        let rotated_out_blue = rotate2d(
            colorspace_in.b,
            colorspace_in.w,
            degrees_to_radians(primaries_rotate[2]),
        );

        // Bisecting lines.
        let rotated_out_lines = [
            [colorspace_in.w, rotated_out_red],
            [colorspace_in.w, rotated_out_green],
            [colorspace_in.w, rotated_out_blue],
        ];

        // Gamut boundary lines.
        let gamut_boundary = [
            [colorspace_in.r, colorspace_in.g],
            [colorspace_in.g, colorspace_in.b],
            [colorspace_in.b, colorspace_in.r],
        ];

        // New primaries.
        let intersections = [
            intersect_lines(
                rotated_out_lines[0],
                if primaries_rotate[0] > 0.0 {
                    gamut_boundary[0]
                } else {
                    gamut_boundary[2]
                },
            ),
            intersect_lines(
                rotated_out_lines[1],
                if primaries_rotate[1] > 0.0 {
                    gamut_boundary[1]
                } else {
                    gamut_boundary[0]
                },
            ),
            intersect_lines(
                rotated_out_lines[2],
                if primaries_rotate[2] > 0.0 {
                    gamut_boundary[2]
                } else {
                    gamut_boundary[1]
                },
            ),
        ];

        // Inset according to the desired inset scales. Insetting controls the rate
        // of attenuation.
        let primaries_inset = [
            scale2d(intersections[0], colorspace_in.w, 1.0 - primaries_inset[0]),
            scale2d(intersections[1], colorspace_in.w, 1.0 - primaries_inset[1]),
            scale2d(intersections[2], colorspace_in.w, 1.0 - primaries_inset[2]),
        ];

        let tmp = Chromaticities {
            r: primaries_inset[0],
            g: primaries_inset[1],
            b: primaries_inset[2],
            w: colorspace_in.w,
        };

        tmp
    }

    //------------
    // Utilities.

    fn scale2d(p: (f64, f64), pivot: (f64, f64), factor: f64) -> (f64, f64) {
        let p1 = (p.0 - pivot.0, p.1 - pivot.1);
        let p2 = (p1.0 * factor, p1.1 * factor);
        (p2.0 + pivot.0, p2.1 + pivot.1)
    }

    fn rotate2d(p: (f64, f64), pivot: (f64, f64), angle: f64) -> (f64, f64) {
        let p1 = (p.0 - pivot.0, p.1 - pivot.1);
        let p2 = (
            (angle.cos() * p1.0) - (angle.sin() * p1.1),
            (angle.sin() * p1.0) + (angle.cos() * p1.1),
        );
        (p2.0 + pivot.0, p2.1 + pivot.1)
    }

    fn degrees_to_radians(n: f64) -> f64 {
        n / 180.0 * std::f64::consts::PI
    }

    fn intersect_lines(a: [(f64, f64); 2], b: [(f64, f64); 2]) -> (f64, f64) {
        let slope_a = (a[0].1 - a[1].1) / (a[0].0 - a[1].0);
        let offset_a = a[0].1 - (a[0].0 * slope_a);
        let slope_b = (b[0].1 - b[1].1) / (b[0].0 - b[1].0);
        let offset_b = b[0].1 - (b[0].0 * slope_b);

        let tmp = (offset_b - offset_a) / (slope_a - slope_b);
        (tmp, slope_a * tmp + offset_a)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use colorbox::chroma;

        fn is_close(a: f64, b: f64, thresh: f64) -> bool {
            (a - b).abs() < thresh
        }

        #[test]
        fn test1() {
            let c1 = chroma::Chromaticities {
                r: (0.49696405145569406, 0.33922148854989),
                g: (0.30657700760943823, 0.5373758468983858),
                b: (0.17958017322958988, 0.12531502362652353),
                w: (0.3127, 0.329),
            };
            let c2 = create_working_space([3.0, -1.0, -2.0], [0.4, 0.22, 0.13], chroma::REC709);

            assert!(is_close(c1.r.0, c2.r.0, 0.0000001));
            assert!(is_close(c1.r.1, c2.r.1, 0.0000001));
            assert!(is_close(c1.g.0, c2.g.0, 0.0000001));
            assert!(is_close(c1.g.1, c2.g.1, 0.0000001));
            assert!(is_close(c1.b.0, c2.b.0, 0.0000001));
            assert!(is_close(c1.b.1, c2.b.1, 0.0000001));
            assert!(is_close(c1.w.0, c2.w.0, 0.0000001));
            assert!(is_close(c1.w.1, c2.w.1, 0.0000001));
        }

        #[test]
        fn test2() {
            let c1 = chroma::Chromaticities {
                r: (0.50908, 0.3296),
                g: (0.302794, 0.5403799999999997),
                b: (0.156508, 0.07075999999999999),
                w: (0.3127, 0.329),
            };
            let c2 = create_working_space([0.0, 0.0, 0.0], [0.4, 0.22, 0.04], chroma::REC709);

            assert!(is_close(c1.r.0, c2.r.0, 0.0000001));
            assert!(is_close(c1.r.1, c2.r.1, 0.0000001));
            assert!(is_close(c1.g.0, c2.g.0, 0.0000001));
            assert!(is_close(c1.g.1, c2.g.1, 0.0000001));
            assert!(is_close(c1.b.0, c2.b.0, 0.0000001));
            assert!(is_close(c1.b.1, c2.b.1, 0.0000001));
            assert!(is_close(c1.w.0, c2.w.0, 0.0000001));
            assert!(is_close(c1.w.1, c2.w.1, 0.0000001));
        }
    }
}
