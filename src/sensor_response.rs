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

        // Build the mapping curve.
        let mut curve = Vec::new();
        let mut i1 = 0;
        let mut i2 = 1;
        let mut acc1 = 0;
        let mut acc2 = h2.buckets[0];
        while i1 < h1.buckets.len() && i2 < h2.buckets.len() {
            if acc1 >= acc2 {
                acc2 += h2.buckets[i2];
                i2 += 1;
                if acc2 >= acc1 {
                    curve.push((
                        i1 as f32 / h1.buckets.len() as f32,
                        i2 as f32 / h2.buckets.len() as f32,
                    ));
                }
            } else {
                acc1 += h1.buckets[i1];
                i1 += 1;
                if acc1 > acc2 {
                    curve.push((
                        i1 as f32 / h1.buckets.len() as f32,
                        i2 as f32 / h2.buckets.len() as f32,
                    ));
                }
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
}

pub fn estimate_inverse_sensor_response(mapping: &ExposureMapping) -> Curve {
    let mut inv_response_curve = vec![(0.0f32, 0.0f32), (1.0f32, 1.0f32)];
    let mut scratch_curve = Vec::new();

    for _ in 0..250 {
        // Do smoothing on the current estimated inverse response curve.
        scratch_curve.clear();
        let smoothing_rounds = 100;
        for _ in 0..smoothing_rounds {
            for i in 0..inv_response_curve.len() {
                let (x, mut y) = inv_response_curve[i];
                if i > 0 && i < (inv_response_curve.len() - 1) {
                    let a = inv_response_curve[i - 1];
                    let b = inv_response_curve[i + 1];
                    let alpha = (x - a.0) / (b.0 - a.0);
                    let y_lerp = a.1 + ((b.1 - a.1) * alpha);
                    y = y * 0.5 + y_lerp * 0.5;
                }
                scratch_curve.push((x, y));
            }
            std::mem::swap(&mut inv_response_curve, &mut scratch_curve);
            scratch_curve.clear();
        }

        // Fix the points of the inverse response curve to be consistent
        // with the points of the mapping curve.
        for (x, y) in mapping.curve.iter() {
            let xp = lerp_curve_at_x(&inv_response_curve, *x);
            let yp = lerp_curve_at_x(&inv_response_curve, *y);

            let offset = ((xp * mapping.exposure_ratio) - yp) / (mapping.exposure_ratio + 1.0);

            let xp2 = (xp - offset).max(0.0).min(1.0);
            let yp2 = (yp + offset).max(0.0).min(1.0);

            scratch_curve.push((*x, xp2));
            scratch_curve.push((*y, yp2));
        }
        std::mem::swap(&mut inv_response_curve, &mut scratch_curve);
        scratch_curve.clear();

        // Clean up: make sure things are sorted, deduplicated, and span [0.0, 1.0].
        inv_response_curve.push((0.0, 0.0));
        inv_response_curve.push((1.0, 1.0));
        inv_response_curve.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        inv_response_curve.dedup_by(|a, b| a.0 == b.0);
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
