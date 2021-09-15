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

pub fn generate_mapping_matrix(
    images: &[(image::RgbImage, f32)],
) -> [[[(f32, usize); 256]; 256]; 3] {
    // A mapping from one 255 image value to another, with a list of floats
    // indicating the linear luminance ratio estimates between them.
    let mut matrix_r = vec![Vec::new(); 256 * 256];
    let mut matrix_g = vec![Vec::new(); 256 * 256];
    let mut matrix_b = vec![Vec::new(); 256 * 256];

    assert!(images.len() > 1);
    for i in 0..(images.len() - 1) {
        let img1 = &images[i].0;
        let img2 = &images[i + 1].0;
        let exp_ratio = images[i + 1].1 / images[i].1;

        assert_eq!(img1.dimensions(), img2.dimensions());
        for (p1, p2) in img1.enumerate_pixels().zip(img2.enumerate_pixels()) {
            let r1 = p1.2[0] as usize;
            let g1 = p1.2[1] as usize;
            let b1 = p1.2[2] as usize;

            let r2 = p2.2[0] as usize;
            let g2 = p2.2[1] as usize;
            let b2 = p2.2[2] as usize;

            matrix_r[r1 * 256 + r2].push(exp_ratio);
            matrix_g[g1 * 256 + g2].push(exp_ratio);
            matrix_b[b1 * 256 + b2].push(exp_ratio);
        }
    }

    // Collapse each list of luminance ratio estimates into their median value,
    // and store that and the length of the list it came from.
    let mut matrix_med_r = [[(0.0f32, 0usize); 256]; 256];
    let mut matrix_med_g = [[(0.0f32, 0usize); 256]; 256];
    let mut matrix_med_b = [[(0.0f32, 0usize); 256]; 256];
    for in_n in 0..256 {
        for out_n in 0..256 {
            let i = in_n * 256 + out_n;

            if !matrix_r[i].is_empty() {
                matrix_r[i].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                matrix_med_r[in_n][out_n] = (matrix_r[i][matrix_r[i].len() / 2], matrix_r[i].len());
            }
            if !matrix_g[i].is_empty() {
                matrix_g[i].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                matrix_med_g[in_n][out_n] = (matrix_g[i][matrix_g[i].len() / 2], matrix_g[i].len());
            }
            if !matrix_b[i].is_empty() {
                matrix_b[i].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                matrix_med_b[in_n][out_n] = (matrix_b[i][matrix_b[i].len() / 2], matrix_b[i].len());
            }
        }
    }
    drop(matrix_r);
    drop(matrix_g);
    drop(matrix_b);

    [matrix_med_r, matrix_med_g, matrix_med_b]
}
