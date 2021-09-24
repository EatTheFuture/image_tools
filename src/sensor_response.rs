use crate::histogram::Histogram;

// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

type Curve = Vec<(f32, f32)>;

/// A curve that maps from the pixel values of one exposure
/// of an image to another.  The curve is in [0.0, 1.0] on both axes,
/// representing the min/max pixel values of each image format.
#[derive(Debug, Clone)]
pub struct ExposureMapping {
    pub curve: Curve,
    pub exposure_ratio: f32,
}

impl ExposureMapping {
    /// Generates an exposure mapping from two histograms and accompanying exposure values.
    pub fn from_histograms(
        h1: &Histogram,
        h2: &Histogram,
        exposure_1: f32,
        exposure_2: f32,
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

        ExposureMapping {
            curve: curve,
            exposure_ratio: exposure_2 / exposure_1,
        }
    }

    pub fn resampled(&self, point_count: usize) -> ExposureMapping {
        let min = self.curve[0].0;
        let max = self.curve.last().unwrap().0;
        let inc = 1.0 / (point_count - 1) as f32;
        let mut curve = Vec::new();
        for i in 0..point_count {
            let x = (inc * i as f32) + ((fastrand::f32() - 0.5) * inc);
            if x >= min && x <= max {
                let y = lerp_curve_at_x(&self.curve[..], x);
                curve.push((x, y));
            }
        }
        ExposureMapping {
            curve: curve,
            exposure_ratio: self.exposure_ratio,
        }
    }
}

pub fn estimate_inverse_sensor_response(mappings: &[ExposureMapping]) -> Curve {
    let seed = fastrand::u32(..);
    let segments = 256;
    let target_smoothing_rounds = 8;
    const MAX_SMOOTHING_ROUNDS: usize = 8;

    let mut inv_response_curve = vec![(0.0f32, 0.0f32), (1.0f32, 1.0f32)];
    let mut scratch_curve = Vec::new();

    for round in 0..256 {
        let segs_per_mapping = segments / mappings.len();
        for mapping in mappings.iter().map(|m| m.resampled(segs_per_mapping)) {
            for (x, y) in mapping.curve.iter().copied() {
                let xp = lerp_curve_at_x(&inv_response_curve, x);
                let yp = lerp_curve_at_x(&inv_response_curve, y);

                // let target_ratio = mapping.exposure_ratio.min(1.0 / xp);
                let target_ratio = mapping.exposure_ratio;
                let offset = ((xp * target_ratio) - yp) / (target_ratio + 1.0);

                let xp2 = (xp - (offset * 0.5)).max(0.0).min(1.0);
                let yp2 = (yp + (offset * 0.5)).max(0.0).min(1.0);

                scratch_curve.push((x, xp2));
                scratch_curve.push((y, yp2));
            }
        }
        std::mem::swap(&mut inv_response_curve, &mut scratch_curve);
        scratch_curve.clear();

        // Clean up: make sure things are sorted, deduplicated, and span [0.0, 1.0].
        inv_response_curve.push((0.0, 0.0));
        inv_response_curve.push((1.0, 1.0));
        inv_response_curve.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        inv_response_curve.dedup_by(|a, b| a.0 == b.0);

        // Do smoothing on the current estimated inverse response curve.
        for smooth_round in 0..MAX_SMOOTHING_ROUNDS {
            let mut is_monotonic = true;
            for i in 0..inv_response_curve.len() {
                if i > 0 && i < (inv_response_curve.len() - 1) {
                    // Do smoothing.
                    let a = inv_response_curve[i - 1];
                    let b = inv_response_curve[i];
                    let c = inv_response_curve[i + 1];
                    let alpha = (b.0 - a.0) / (c.0 - a.0);
                    let y_lerp = a.1 + ((c.1 - a.1) * alpha);

                    is_monotonic &= b.1 > a.1 && c.1 > b.1;

                    scratch_curve.push((b.0, b.1 * 0.333 + y_lerp * 0.667));
                } else {
                    scratch_curve.push(inv_response_curve[i]);
                };
            }
            std::mem::swap(&mut inv_response_curve, &mut scratch_curve);
            scratch_curve.clear();

            // // Check if we've reached our smoothing targets.
            // let mut min_slope_ratio = 9999.0f32;
            // for points in inv_response_curve.windows(3) {
            //     let a = points[0];
            //     let b = points[1];
            //     let c = points[2];

            //     // Update max slope diff.
            //     let slope1 = (b.1 - a.1) / (b.0 - a.0);
            //     let slope2 = (c.1 - b.1) / (c.0 - b.0);
            //     let slope_ratio = if slope1 < slope2 { slope1 / slope2 } else { slope2 / slope1 };
            //     min_slope_ratio = if slope_ratio.is_nan() { 0.0 } else { min_slope_ratio.max(slope_ratio) };
            //     // dbg!((slope1, slope2, slope_ratio));
            // }

            if smooth_round >= target_smoothing_rounds && is_monotonic {
                break;
            }
        }
    }

    // Ensure monotonicity on the final curve.
    for (x, y) in inv_response_curve.iter() {
        if *y >= scratch_curve.last().unwrap_or(&(0.0, 0.0)).1 {
            scratch_curve.push((*x, *y));
        }
    }
    std::mem::swap(&mut inv_response_curve, &mut scratch_curve);

    inv_response_curve
}

// pub fn calc_error(mapping_curve: &[(f32, f32)], exposure_ratio: f32, emor_factors: &[f32]) -> f32 {
//     let target_curve = |x: f32| (x * exposure_ratio).min(1.0);
//     let eval_inv_emor = |x: f32| {
//         let mut y = lerp_slice(INV_EMOR_TABLE[0], x);
//         for i in 0..emor_factors.len() {
//             y += lerp_slice(INV_EMOR_TABLE[i + 1], x) * emor_factors[i];
//         }
//     };

//     let mut err_sum = 0.0f32;
//     for (x, y) in mapping_curve {
//         let x_inv = eval_inv_emor(x);
//         let y_inv = eval_inv_emor(y);
//         let err = y_inv - (x * exposure_ratio);
//         err_sum += err * err;
//     }

//     err_sum / mapping_curve.len() as f32
// }

// Returns the y value at the given x value.
pub fn lerp_curve_at_x(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.0.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].1, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.0) / (p2.0 - p1.0);
    p1.1 + ((p2.1 - p1.1) * alpha)
}

pub fn lerp_slice(slice: &[f32], t: f32) -> f32 {
    let i1 = ((slice.len() - 1) as f32 * t) as usize;
    let alpha = ((slice.len() - 1) as f32 * t) - i1 as f32;

    if i1 == (slice.len() - 1) {
        *slice.last().unwrap()
    } else {
        let v1 = slice[i1];
        let v2 = slice[i1 + 1];
        v1 + ((v2 - v1) * alpha)
    }
}

// Returns the x value at the given y value.
pub fn lerp_curve_at_y(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.1.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].1, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.1) / (p2.1 - p1.1);
    p1.0 + ((p2.0 - p1.0) * alpha)
}

pub fn generate_image_mapping_curves(
    images: &[(image::RgbImage, f32)],
) -> [Vec<ExposureMapping>; 3] {
    assert!(images.len() > 1);

    let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
    for chan in 0..3 {
        for i in 0..images.len() {
            histograms[chan].push(Histogram::from_u8s(
                images[i]
                    .0
                    .enumerate_pixels()
                    .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
            ));
        }
    }

    let mut mappings = [Vec::new(), Vec::new(), Vec::new()];
    for chan in 0..3 {
        for i in 0..(images.len() - 1) {
            mappings[chan].push(ExposureMapping::from_histograms(
                &histograms[chan][i],
                &histograms[chan][i + 1],
                images[i].1,
                images[i + 1].1,
            ));
        }
    }

    mappings
}

pub fn generate_mapping_graph(mappings: &[Vec<ExposureMapping>; 3]) -> image::RgbImage {
    // Graph it!
    let mut graph = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    for chan in 0..3 {
        for i in 0..mappings[0].len() {
            crate::draw_line_segments(
                &mut graph,
                mappings[chan][i].curve.iter().copied(),
                image::Rgb(match chan {
                    0 => [128, 0, 0],
                    1 => [0, 128, 0],
                    2 => [0, 0, 128],
                    _ => [128, 128, 128],
                }),
            );
        }
    }

    graph
}
