use clap::{App, Arg};

mod emor;
mod exposure_mapping;
mod histogram;
mod utils;

use utils::lerp_slice;

fn main() {
    let matches = App::new("My Super Program")
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

        println!(
            "{}\n{} {} {}\n{}\n",
            filename, exposure_time, fstop, sensitivity, total_exposure,
        );

        images.push((img, total_exposure as f32));
    }

    images.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // let pix_ranges: Vec<_> = images.iter().map(|img|
    //     (img.0.pixels().fold((255u8, 255u8, 255u8), |a, b| {
    //         (
    //             a.0.min(b[0]),
    //             a.1.min(b[1]),
    //             a.2.min(b[2]),
    //         )
    //     }),
    //     img.0.pixels().fold((0u8, 0u8, 0u8), |a, b| {
    //         (
    //             a.0.max(b[0]),
    //             a.1.max(b[1]),
    //             a.2.max(b[2]),
    //         )
    //     }))
    // ).collect();
    // dbg!(pix_ranges);

    // // Write out a graph of the EMoR curves.
    // let mut graph_emor = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    // let n = 8;
    // for i in 0..n {
    //     let table = &emor::EMOR_TABLE[i + 1];
    //     let v = (255 - (i * (255 / n))) as u8;
    //     let rgb = image::Rgb([v, v, v]);

    //     draw_line_segments(
    //         &mut graph_emor,
    //         (0..table.len()).zip(table.iter()).map(|(x, y)| {
    //             (
    //                 x as f32 / table.len() as f32,
    //                 (0.5 + y).max(0.0).min(1.0),
    //             )
    //         }),
    //         rgb,
    //     );
    // }
    // graph_emor.save("graph_emor.png").unwrap();

    // Calculate exposure mappings.
    println!("Calculating exposure mappings.");
    let mapping_curves = {
        let [mut r, mut g, mut b] = exposure_mapping::generate_image_mapping_curves(&images[..]);
        let combined: Vec<_> = r.drain(..).chain(g.drain(..)).chain(b.drain(..)).collect();
        combined
    };
    // let graph_mapping = exposure_mapping::generate_mapping_graph(&mapping_curves);
    // graph_mapping.save("graph_mapping.png").unwrap();

    // Estimate sensor response curves from the exposure mappings.
    println!("Calculating sensor mapping.");
    let sensor_mapping = {
        let (emor_factors, err) = emor::estimate_emor(&mapping_curves[..]);
        dbg!(emor_factors, err);
        emor::emor_factors_to_curve(&emor_factors)
    };
    let sensor_mapping_no_srgb: Vec<f32> =
        sensor_mapping.iter().copied().map(srgb_inv_gamma).collect();
    let inv_mapping = utils::flip_slice_xy(&sensor_mapping, 512);
    let inv_mapping_no_srgb = utils::flip_slice_xy(&sensor_mapping_no_srgb, 512);

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
    lut::write_cube_1d(
        &mut std::io::BufWriter::new(
            std::fs::File::create("linear_to_sensor_no_srgb.cube").unwrap(),
        ),
        (0.0, 1.0),
        &sensor_mapping_no_srgb,
        &sensor_mapping_no_srgb,
        &sensor_mapping_no_srgb,
    )
    .unwrap();
    lut::write_cube_1d(
        &mut std::io::BufWriter::new(
            std::fs::File::create("sensor_to_linear_no_srgb.cube").unwrap(),
        ),
        (0.0, 1.0),
        &inv_mapping_no_srgb,
        &inv_mapping_no_srgb,
        &inv_mapping_no_srgb,
    )
    .unwrap();

    // let srgb_gamma_curve: Vec<f32> = (0..4096).map(|n| srgb_gamma(n as f32 / 4095.0)).collect();
    // let srgb_inv_gamma_curve: Vec<f32> = (0..4096)
    //     .map(|n| srgb_inv_gamma(n as f32 / 4095.0))
    //     .collect();

    // lut::write_cube_1d(
    //     &mut std::io::BufWriter::new(std::fs::File::create("srgb_gamma.cube").unwrap()),
    //     (0.0, 1.0),
    //     &srgb_gamma_curve,
    //     &srgb_gamma_curve,
    //     &srgb_gamma_curve,
    // )
    // .unwrap();
    // lut::write_cube_1d(
    //     &mut std::io::BufWriter::new(std::fs::File::create("srgb_gamma_inv.cube").unwrap()),
    //     (0.0, 1.0),
    //     &srgb_inv_gamma_curve,
    //     &srgb_inv_gamma_curve,
    //     &srgb_inv_gamma_curve,
    // )
    // .unwrap();

    // // Save out debug sensor mapping graphs.
    // let mut graph_sensor = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    // draw_line_segments(
    //     &mut graph_sensor,
    //     sensor_mapping.iter().enumerate().map(|(i, y)| {
    //         let x = i as f32 / (sensor_mapping.len() - 1) as f32;
    //         (x, *y)
    //     }),
    //     image::Rgb([255, 255, 255]),
    // );
    // graph_sensor.save("graph_sensor.png").unwrap();

    // let mut graph_sensor_inv = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    // draw_line_segments(
    //     &mut graph_sensor_inv,
    //     inv_mapping.iter().enumerate().map(|(i, y)| {
    //         let x = i as f32 / (inv_mapping.len() - 1) as f32;
    //         (x, *y)
    //     }),
    //     image::Rgb([255, 255, 255]),
    // );
    // graph_sensor_inv.save("graph_sensor_inv.png").unwrap();

    // let mut graph_sensor_inv_no_srgb =
    //     image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    // draw_line_segments(
    //     &mut graph_sensor_inv_no_srgb,
    //     inv_mapping_no_srgb.iter().enumerate().map(|(i, y)| {
    //         let x = i as f32 / (inv_mapping_no_srgb.len() - 1) as f32;
    //         (x, *y)
    //     }),
    //     image::Rgb([255, 255, 255]),
    // );
    // graph_sensor_inv_no_srgb
    //     .save("graph_sensor_inv_no_srgb.png")
    //     .unwrap();

    // let mut graph_mapping_linear = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    // for mapping in mapping_curves.iter() {
    //     draw_line_segments(
    //         &mut graph_mapping_linear,
    //         mapping.curve.iter().map(|p| {
    //             (
    //                 lerp_slice(&inv_mapping[..], p.0),
    //                 lerp_slice(&inv_mapping[..], p.1),
    //             )
    //         }),
    //         image::Rgb([64, 64, 64]),
    //     );
    // }
    // graph_mapping_linear
    //     .save("graph_mapping_linear.png")
    //     .unwrap();

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
            let r_linear = lerp_slice(&inv_mapping[..], r);
            let g_linear = lerp_slice(&inv_mapping[..], g);
            let b_linear = lerp_slice(&inv_mapping[..], b);
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

fn srgb_gamma(n: f32) -> f32 {
    if n < 0.003_130_8 {
        n * 12.92
    } else {
        (1.055 * n.powf(1.0 / 2.4)) - 0.055
    }
}

fn srgb_inv_gamma(n: f32) -> f32 {
    if n < 0.04045 {
        n / 12.92
    } else {
        ((n + 0.055) / 1.055).powf(2.4)
    }
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
