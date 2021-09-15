/// A mapping from input image rgb values to output linear rgb values.
#[derive(Debug, Clone)]
pub struct SensorResponseTable {
    r: Vec<(u8, f32)>,
    g: Vec<(u8, f32)>,
    b: Vec<(u8, f32)>,
}

/// Takes a list of images and their exposure values, and
/// returns an estimate of the camera's sensor response.
///
/// The images are assumed to be sorted in order of exposure,
/// from least exposed to most, and are assumed to be the same
/// resolution.
pub fn estimate_sensor_response(images: &[(image::RgbImage, f32)]) -> SensorResponseTable {
    todo!()
}

pub fn generate_mapping_matrix(images: &[(image::RgbImage, f32)]) -> [[[usize; 256]; 256]; 3] {
    // Build the histograms for each image.
    // One per channel.
    let mut histograms = [
        vec![[0usize; 256]; images.len()], // r
        vec![[0usize; 256]; images.len()], // g
        vec![[0usize; 256]; images.len()], // b
    ];
    for i in 0..images.len() {
        for (_, _, rgb) in images[i].0.enumerate_pixels() {
            for chan in 0..3 {
                histograms[chan][i][rgb[chan] as usize] += 1;
            }
        }
    }

    // Build the mapping points.
    assert!(images.len() > 1);
    let mut mappings: [Vec<(Vec<(u8, u8)>, f32)>; 3] = [
        vec![(Vec::new(), 0.0); images.len() - 1], // r
        vec![(Vec::new(), 0.0); images.len() - 1], // g
        vec![(Vec::new(), 0.0); images.len() - 1], // b
    ];
    for i in 0..(images.len() - 1) {
        let relative_exposure = images[i + 1].1 / images[i].1;
        for chan in 0..3 {
            mappings[chan][i].1 = relative_exposure;
            let hist_1 = &histograms[chan][i];
            let hist_2 = &histograms[chan][i + 1];

            let mut i1 = 0;
            let mut i2 = 1;
            let mut acc1 = 0;
            let mut acc2 = hist_2[0];

            while i1 < 256 && i2 < 256 {
                if acc1 > acc2 {
                    acc2 += hist_2[i2];
                    i2 += 1;
                    if acc2 >= acc1 {
                        mappings[chan][i].0.push((i1 as u8, i2 as u8));
                    }
                } else {
                    acc1 += hist_1[i1];
                    i1 += 1;
                    if acc1 > acc2 {
                        mappings[chan][i].0.push((i1 as u8, i2 as u8));
                    }
                }
            }

            mappings[chan][i].0.dedup_by_key(|n| n.0);
            mappings[chan][i].0.dedup_by_key(|n| n.1);
        }
    }

    // Graph it!
    let mut graph = [[[0usize; 256]; 256]; 3];
    for chan in 0..3 {
        for i in 0..(images.len() - 1) {
            for (in_v, out_v) in &mappings[chan][i].0 {
                graph[chan][*in_v as usize][*out_v as usize] += 1;
            }
        }
    }

    graph
}
