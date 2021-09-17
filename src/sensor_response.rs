// Provides `EMOR_TABLE` and `INV_EMOR_TABLE`;
include!(concat!(env!("OUT_DIR"), "/emor.inc"));

/// Takes a list of exposure-pair mapping curves and their exposure ratios,
/// and returns an estimate of the camera's sensor response as a combination
/// of EMoR factors.
///
/// The images are assumed to be sorted in order of exposure,
/// from least exposed to most, and are assumed to be the same
/// resolution.
pub fn estimate_sensor_response(mapping_curves: &[(Vec<(f32, f32)>, f32)]) -> Vec<f32> {
    todo!()
}

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

// Given sets of pixel values (in [0, u16::MAX]) from images with two different exposures,
// produces a mapping curve that converts from the pixel values in the first image to
// the pixel values in the second.
//
// The returned curve uses [0.0, 1.0).
pub fn generate_mapping_curve<Itr1, Itr2>(
    pixel_values_1: Itr1,
    pixel_values_2: Itr2,
) -> Vec<(f32, f32)>
where
    Itr1: std::iter::Iterator<Item = u16>,
    Itr2: std::iter::Iterator<Item = u16>,
{
    const BUCKET_COUNT: usize = 1 << 12;
    const TO_BUCKET_RATIO: usize = (std::u16::MAX as usize + 1) / BUCKET_COUNT;

    // Build histograms of the pixel values.
    let mut histogram_1 = vec![0usize; BUCKET_COUNT];
    let mut histogram_2 = vec![0usize; BUCKET_COUNT];
    for v in pixel_values_1 {
        let bucket = (v as usize / TO_BUCKET_RATIO).min(BUCKET_COUNT - 1);
        histogram_1[bucket] += 1;
    }
    for v in pixel_values_2 {
        let bucket = (v as usize / TO_BUCKET_RATIO).min(BUCKET_COUNT - 1);
        histogram_2[bucket] += 1;
    }

    // Build the mapping curve.
    let mut mapping = Vec::new();
    let mut i1 = 0;
    let mut i2 = 1;
    let mut acc1 = 0;
    let mut acc2 = histogram_2[0];
    while i1 < BUCKET_COUNT && i2 < BUCKET_COUNT {
        if acc1 > acc2 {
            acc2 += histogram_2[i2];
            i2 += 1;
            if acc2 >= acc1 {
                mapping.push((
                    i1 as f32 / BUCKET_COUNT as f32,
                    i2 as f32 / BUCKET_COUNT as f32,
                ));
            }
        } else {
            acc1 += histogram_1[i1];
            i1 += 1;
            if acc1 > acc2 {
                mapping.push((
                    i1 as f32 / BUCKET_COUNT as f32,
                    i2 as f32 / BUCKET_COUNT as f32,
                ));
            }
        }
    }

    // Remove points that are duplicate in either dimension.
    mapping.dedup_by_key(|n| n.0);
    mapping.dedup_by_key(|n| n.1);

    mapping
}

pub fn generate_mapping_graph(images: &[(image::RgbImage, f32)]) -> [[[usize; 256]; 256]; 3] {
    // Build the mapping curves.
    assert!(images.len() > 1);
    let mut mappings = [Vec::new(), Vec::new(), Vec::new()];
    for chan in 0..3 {
        for i in 0..(images.len() - 1) {
            let relative_exposure = images[i + 1].1 / images[i].1;
            let mapping_curve = generate_mapping_curve(
                images[i]
                    .0
                    .enumerate_pixels()
                    .map(|p: (u32, u32, &image::Rgb<u8>)| (p.2[chan] as u16) << 8),
                images[i + 1]
                    .0
                    .enumerate_pixels()
                    .map(|p: (u32, u32, &image::Rgb<u8>)| (p.2[chan] as u16) << 8),
            );
            mappings[chan].push((mapping_curve, relative_exposure));
        }
    }

    // Graph it!
    let mut graph = [[[0usize; 256]; 256]; 3];
    for chan in 0..3 {
        for i in 0..(images.len() - 1) {
            for i2 in 0..256 {
                let x = ((lerp_curve_at_y(&mappings[chan][i].0, i2 as f32 / 255.0) * 255.0)
                    as usize)
                    .min(255);
                let y = ((lerp_curve_at_x(&mappings[chan][i].0, i2 as f32 / 255.0) * 255.0)
                    as usize)
                    .min(255);
                graph[chan][i2][y] += 1;
                graph[chan][x][i2] += 1;
            }
        }
    }

    graph
}
