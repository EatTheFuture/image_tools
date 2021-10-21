use clap::{App, Arg};

use sensor_analysis::{eval_luma_map, invert_luma_map};

fn main() {
    let matches = App::new("HDRI Merge")
        .version("1.0")
        .author("Nathan Vegdahl")
        .about("Merges LDR images into an HDRI")
        .arg(
            Arg::with_name("INPUT")
                .help("input image files")
                .required(true)
                .multiple(true)
                .index(1),
        )
        // .arg(
        //     Arg::with_name("config")
        //         .short("c")
        //         .long("config")
        //         .value_name("FILE")
        //         .help("Sets a custom config file")
        //         .takes_value(true),
        // )
        // .arg(
        //     Arg::with_name("v")
        //         .short("v")
        //         .multiple(true)
        //         .help("Sets the level of verbosity"),
        // )
        .get_matches();

    let filenames: Vec<_> = matches.values_of("INPUT").unwrap().collect();

    println!("Loading image files.");
    let mut images = Vec::new();
    for filename in filenames {
        let img = image::io::Reader::open(filename)
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap()
            .to_rgb8();

        let img_exif = {
            let mut file = std::io::BufReader::new(std::fs::File::open(filename).unwrap());
            exif::Reader::new().read_from_container(&mut file).unwrap()
        };

        let exposure_time = match img_exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
            Some(n) => match n.value {
                exif::Value::Rational(ref v) => v[0],
                _ => panic!(),
            },
            None => panic!(),
        };
        let fstop = match img_exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
            Some(n) => match n.value {
                exif::Value::Rational(ref v) => v[0],
                _ => panic!(),
            },
            None => panic!(),
        };
        let sensitivity = img_exif
            .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
            .unwrap()
            .value
            .get_uint(0)
            .unwrap();

        let total_exposure =
            sensitivity as f64 * exposure_time.to_f64() / (fstop.to_f64() * fstop.to_f64());

        images.push((img, total_exposure as f32));
    }

    images.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Estimate sensor response curve from the image-exposure pairs.
    println!("Calculating sensor response curve.");
    let inv_mapping = invert_luma_map(&estimate_luma_map(&images).0);

    // Create the HDR.
    println!("Building HDR image.");
    fn calc_weight(n: f32, is_lowest_exposed: bool) -> f32 {
        // Triangle weight.
        let tri = if is_lowest_exposed && n > 0.5 {
            1.0
        } else {
            (0.5 - (n - 0.5).abs()) * 2.0
        };

        // Smooth step weight.
        tri * tri * (3.0 - 2.0 * tri)
    }
    let width = images[0].0.width() as usize;
    let height = images[0].0.height() as usize;
    let mut hdr_image = vec![[0.0f32; 3]; width * height];
    let mut hdr_weights = vec![[0.0f32; 3]; width * height];
    for (img_i, (image, exposure)) in images.iter().enumerate() {
        let inv_exposure = 1.0 / exposure;
        for (i, pixel) in image.pixels().enumerate() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            let r_linear = eval_luma_map(&inv_mapping[..], r);
            let g_linear = eval_luma_map(&inv_mapping[..], g);
            let b_linear = eval_luma_map(&inv_mapping[..], b);
            let weight = calc_weight(r_linear.max(g_linear).max(b_linear), img_i == 0);
            hdr_image[i][0] += r_linear * inv_exposure * weight;
            hdr_image[i][1] += g_linear * inv_exposure * weight;
            hdr_image[i][2] += b_linear * inv_exposure * weight;
            hdr_weights[i][0] += weight;
            hdr_weights[i][1] += weight;
            hdr_weights[i][2] += weight;
        }
    }
    for i in 0..(width * height) {
        if hdr_weights[i][0] > 0.0 {
            hdr_image[i][0] /= hdr_weights[i][0];
        }
        if hdr_weights[i][1] > 0.0 {
            hdr_image[i][1] /= hdr_weights[i][1];
        }
        if hdr_weights[i][2] > 0.0 {
            hdr_image[i][2] /= hdr_weights[i][2];
        }
    }

    println!("Writing output.");
    hdr::write_hdr(
        &mut std::io::BufWriter::new(std::fs::File::create("test.hdr").unwrap()),
        &hdr_image[..],
        width,
        height,
    )
    .unwrap();
}

/// Estimates a linearizing luma map for the given set of image-exposure
/// pairs.
///
/// Returns the luminance map and the average fitting error.
pub fn estimate_luma_map(images: &[(image::RgbImage, f32)]) -> (Vec<f32>, f32) {
    use sensor_analysis::{estimate_luma_map_emor, ExposureMapping, Histogram};

    assert!(images.len() > 1);

    let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
    for chan in 0..3 {
        for i in 0..images.len() {
            histograms[chan].push(Histogram::from_iter(
                images[i]
                    .0
                    .enumerate_pixels()
                    .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
                256,
            ));
        }
    }

    let mut mappings = Vec::new();
    for chan in 0..3 {
        for i in 0..(images.len() - 1) {
            mappings.push(ExposureMapping::from_histograms(
                &histograms[chan][i],
                &histograms[chan][i + 1],
                images[i].1,
                images[i + 1].1,
            ));
        }
    }

    estimate_luma_map_emor(&mappings)
}
