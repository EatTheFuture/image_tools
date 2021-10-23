use clap::{App, Arg};

use sensor_analysis::invert_luma_map;

fn main() {
    let matches = App::new("Images to Sensor Response")
        .version("1.0")
        .author("Nathan Vegdahl")
        .about("Estimates sensor luma response curves from a set of differently exposed images of the same scene")
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
    let sensor_mapping = estimate_luma_map(&images).0;
    let inv_mapping = invert_luma_map(&sensor_mapping);

    // Write out senseor response curve lookup tables.
    lut::write_cube_1d(
        &mut std::io::BufWriter::new(std::fs::File::create("linear_to_sensor.cube").unwrap()),
        (0.0, 1.0),
        &sensor_mapping,
        &sensor_mapping,
        &sensor_mapping,
    )
    .unwrap();
    lut::write_cube_1d(
        &mut std::io::BufWriter::new(std::fs::File::create("sensor_to_linear.cube").unwrap()),
        (0.0, 1.0),
        &inv_mapping,
        &inv_mapping,
        &inv_mapping,
    )
    .unwrap();

    // Save out debug sensor mapping graph.
    let mut graph_sensor = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    draw_line_segments(
        &mut graph_sensor,
        sensor_mapping.iter().enumerate().map(|(i, y)| {
            let x = i as f32 / (sensor_mapping.len() - 1) as f32;
            (x, *y)
        }),
        image::Rgb([255, 255, 255]),
    );
    graph_sensor.save("graph_sensor.png").unwrap();

    // // Write out senseor response curve lookup tables for S-Log2.
    // let slog2_mapping: Vec<f32> = (0..1024)
    //     .map(|i| {
    //         let x = i as f32 / 1023.0;
    //         sensor_analysis::known_luma_curves::sony_slog2(x)
    //     })
    //     .collect();
    // lut::write_cube_1d(
    //     &mut std::io::BufWriter::new(std::fs::File::create("linear_to_slog2.cube").unwrap()),
    //     (0.0, 1.0),
    //     &slog2_mapping,
    //     &slog2_mapping,
    //     &slog2_mapping,
    // )
    // .unwrap();
    // let slog2_inv_mapping: Vec<f32> = (0..1024)
    //     .map(|i| {
    //         let x = i as f32 / 1023.0;
    //         sensor_analysis::known_luma_curves::sony_slog2_inv(x)
    //     })
    //     .collect();
    // lut::write_cube_1d(
    //     &mut std::io::BufWriter::new(std::fs::File::create("slog2_to_linear.cube").unwrap()),
    //     (0.0, 1.0),
    //     &slog2_inv_mapping,
    //     &slog2_inv_mapping,
    //     &slog2_inv_mapping,
    // )
    // .unwrap();

    // // Write out sRGB-ified versions of each image.
    // for i in 0..images.len() {
    //     let mut linear = images[i].0.clone();
    //     for pixel in linear.pixels_mut() {
    //         for channel in 0..3 {
    //             let v = pixel[channel] as f32 / 255.0;
    //             let v_linear = lerp_slice(&inv_mapping[..], v);
    //             pixel[channel] = (srgb_gamma(v_linear) * 255.0) as u8;
    //         }
    //     }
    //     linear.save(format!("lin_{:02}.jpg", i + 1)).unwrap();
    // }
}

/// Estimates a linearizing luma map for the given set of image-exposure
/// pairs.
///
/// Returns the luminance map and the average fitting error.
pub fn estimate_luma_map(images: &[(image::RgbImage, f32)]) -> (Vec<f32>, f32) {
    use sensor_analysis::{estimate_luma_map_emor, Histogram};

    assert!(images.len() > 1);

    let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
    for i in 0..images.len() {
        for chan in 0..3 {
            histograms[chan].push((
                Histogram::from_iter(
                    images[i]
                        .0
                        .enumerate_pixels()
                        .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
                    256,
                ),
                images[i].1,
            ));
        }
    }

    estimate_luma_map_emor(&[&histograms[0], &histograms[1], &histograms[2]])
}

pub fn draw_line_segments<Itr>(img: &mut image::RgbImage, points: Itr, color: image::Rgb<u8>)
where
    Itr: std::iter::Iterator<Item = (f32, f32)>,
{
    let (w, h) = (img.width(), img.height());
    let mut points = points.peekable();

    while let Some(p1) = points.next() {
        if let Some(p2) = points.peek() {
            let mut x1 = (p1.0 * (w - 1) as f32).min(w as f32) as u32;
            let mut y1 = (p1.1 * (h - 1) as f32).min(h as f32) as u32;
            let mut x2 = (p2.0 * (w - 1) as f32).min(w as f32) as u32;
            let mut y2 = (p2.1 * (h - 1) as f32).min(h as f32) as u32;

            if (y2 as i32 - y1 as i32).abs() < (x2 as i32 - x1 as i32).abs() {
                if x1 > x2 {
                    std::mem::swap(&mut x1, &mut x2);
                    std::mem::swap(&mut y1, &mut y2);
                }
                let mut y = y1 as f32;
                let dy = (y2 as f32 - y1 as f32) / (x2 as f32 - x1 as f32);
                for x in x1..x2 {
                    let xi = x.min(w - 1);
                    let yi = (h - 1 - y as u32).min(h - 1);
                    let c = *img.get_pixel(xi, yi);
                    img.put_pixel(
                        xi,
                        yi,
                        image::Rgb([
                            (c[0] as u32 + color[0] as u32).min(255) as u8,
                            (c[1] as u32 + color[1] as u32).min(255) as u8,
                            (c[2] as u32 + color[2] as u32).min(255) as u8,
                        ]),
                    );
                    y += dy;
                }
            } else {
                if y1 > y2 {
                    std::mem::swap(&mut x1, &mut x2);
                    std::mem::swap(&mut y1, &mut y2);
                }
                let mut x = x1 as f32;
                let dx = (x2 as f32 - x1 as f32) / (y2 as f32 - y1 as f32);
                for y in y1..y2 {
                    let xi = (x as u32).min(w - 1);
                    let yi = (h - 1 - y).min(h - 1);
                    let c = *img.get_pixel(xi, yi);
                    img.put_pixel(
                        xi,
                        yi,
                        image::Rgb([
                            (c[0] as u32 + color[0] as u32).min(255) as u8,
                            (c[1] as u32 + color[1] as u32).min(255) as u8,
                            (c[2] as u32 + color[2] as u32).min(255) as u8,
                        ]),
                    );
                    x += dx;
                }
            }
        }
    }
}

pub fn draw_points<Itr>(img: &mut image::RgbImage, points: Itr, color: image::Rgb<u8>)
where
    Itr: std::iter::Iterator<Item = (f32, f32)>,
{
    let (w, h) = (img.width(), img.height());

    for point in points {
        let x = (point.0 * (w - 1) as f32).min(w as f32) as u32;
        let y = (point.1 * (h - 1) as f32).min(h as f32) as u32;
        let xi = x.min(w - 1);
        let yi = (h - 1 - y as u32).min(h - 1);
        let c = *img.get_pixel(xi, yi);
        img.put_pixel(
            xi,
            yi,
            image::Rgb([
                (c[0] as u32 + color[0] as u32).min(255) as u8,
                (c[1] as u32 + color[1] as u32).min(255) as u8,
                (c[2] as u32 + color[2] as u32).min(255) as u8,
            ]),
        );
    }
}
