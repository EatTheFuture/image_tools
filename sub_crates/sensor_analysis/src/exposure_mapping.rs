use crate::histogram::Histogram;
use crate::utils::{lerp_curve_at_x, lerp_curve_at_y, Curve};

/// A curve that maps from the pixel values of one exposure
/// of an image to another.  The curve is in [0.0, 1.0] on both axes,
/// representing the min/max pixel values of each image format.
#[derive(Debug, Clone)]
pub struct ExposureMapping {
    pub curve: Curve,
    pub x_curve: Vec<f32>,
    pub y_curve: Vec<f32>,
    pub exposure_ratio: f32,
    pub floor: f32,
    pub ceiling: f32,
}

impl ExposureMapping {
    /// Generates an exposure mapping from two histograms and accompanying exposure values.
    pub fn from_histograms(
        h1: &Histogram,
        h2: &Histogram,
        exposure_1: f32,
        exposure_2: f32,
        floor: f32,
        ceiling: f32,
    ) -> Self {
        assert_eq!(h1.total_samples, h2.total_samples);

        let norm1 = 1.0 / (h1.buckets.len() - 1) as f32;
        let norm2 = 1.0 / (h2.buckets.len() - 1) as f32;

        // Build the mapping curve.
        let mut curve = Vec::new();
        let mut i1 = 1;
        let mut i2 = 1;
        let mut seg1 = (0, h1.buckets[0]);
        let mut seg2 = (0, h2.buckets[0]);
        let mut prev_plot = 0;
        while i1 < h1.buckets.len() && i2 < h2.buckets.len() {
            // Plot a point.
            if seg1.1 <= seg2.1 && seg1.1 > seg2.0 {
                if prev_plot != 1 {
                    let alpha = (seg1.1 - seg2.0) as f32 / (seg2.1 - seg2.0) as f32;
                    let x = i1 as f32 * norm1;
                    let y = ((i2 - 1) as f32 + alpha) * norm2;
                    curve.push((x, y));
                }
                prev_plot = 1;
            } else if seg2.1 <= seg1.1 && seg2.1 > seg1.0 {
                if prev_plot != 2 {
                    let alpha = (seg2.1 - seg1.0) as f32 / (seg1.1 - seg1.0) as f32;
                    let x = ((i1 - 1) as f32 + alpha) * norm1;
                    let y = i2 as f32 * norm2;
                    curve.push((x, y));
                }
                prev_plot = 2;
            }

            // Advance forward.
            if seg1.1 >= seg2.1 {
                seg2.0 = seg2.1;
                seg2.1 += h2.buckets.get(i2).unwrap_or(&0);
                i2 += 1;
            } else {
                seg1.0 = seg1.1;
                seg1.1 += h1.buckets.get(i1).unwrap_or(&0);
                i1 += 1;
            }
        }

        // Remove points that are duplicate in either dimension.
        curve.dedup_by_key(|n| n.0);
        curve.dedup_by_key(|n| n.1);

        // Create curves optimized for fast eval.
        let res = 2048;
        let mut x_curve = Vec::with_capacity(res);
        let mut y_curve = Vec::with_capacity(res);
        for i in 0..res {
            let n = i as f32 / (res - 1) as f32;
            x_curve.push(lerp_curve_at_x(&curve, n));
            y_curve.push(lerp_curve_at_y(&curve, n));
        }

        ExposureMapping {
            curve: curve,
            x_curve: x_curve,
            y_curve: y_curve,
            exposure_ratio: exposure_2 / exposure_1,
            floor: floor,
            ceiling: ceiling,
        }
    }

    /// Returns the y coordinate at the given x coordinate.
    ///
    /// Returns `None` if the given x isn't within the extent of the curve.
    /// If the curve isn't monotonic, an unspecified result is returned.
    #[allow(dead_code)]
    pub fn eval_at_x(&self, x: f32) -> Option<f32> {
        if x >= self.curve.get(0)?.0 && x <= self.curve.last()?.0 {
            // Some(lerp_curve_at_x(&self.curve, x))
            Some(crate::utils::lerp_slice(&self.x_curve, x))
        } else {
            None
        }
    }

    /// Returns the x coordinate at the given y coordinate.
    ///
    /// Returns `None` if the given y isn't within the extent of the curve.
    /// If the curve isn't monotonic, an unspecified result is returned.
    #[allow(dead_code)]
    pub fn eval_at_y(&self, y: f32) -> Option<f32> {
        if y >= self.curve.get(0)?.1 && y <= self.curve.last()?.1 {
            // Some(lerp_curve_at_y(&self.curve, y))
            Some(crate::utils::lerp_slice(&self.y_curve, y))
        } else {
            None
        }
    }
}
