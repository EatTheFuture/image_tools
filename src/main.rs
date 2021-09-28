use clap::{App, Arg};

mod histogram;
mod sensor_response;

use sensor_response::{EMOR_TABLE, INV_EMOR_TABLE};

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

    let mapping_curves = {
        let [mut r, mut g, mut b] = sensor_response::generate_image_mapping_curves(&images[..]);
        let combined: Vec<_> = r.drain(..).chain(g.drain(..)).chain(b.drain(..)).collect();
        combined
    };

    let graph = sensor_response::generate_mapping_graph(&mapping_curves);

    let mut graph_emor = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    let n = 5;
    for i in 0..(n + 1) {
        let table = &INV_EMOR_TABLE[i];
        let v = (255 / (i + 1)) as u8;
        let rgb = image::Rgb([v, v, v]);

        let offset = if i < 2 { 0.0 } else { 0.5 };
        draw_line_segments(
            &mut graph_emor,
            (0..table.len()).zip(table.iter()).map(|(x, y)| {
                (
                    x as f32 / table.len() as f32,
                    (offset + y).max(0.0).min(1.0),
                )
            }),
            rgb,
        );
    }

    println!("Calculating inverse mappings.");
    // let inv_mapping: Vec<f32> = {
    //     let tmp = sensor_response::estimate_inverse_sensor_response(&mapping_curves[..]);
    //     (0..1024).map(|i| {
    //         let x = i as f32 / 1023.0;
    //         sensor_response::lerp_curve_at_x(&tmp, x)
    //     }).collect()
    // };
    let inv_mapping = {
        let inv_emor_factors = sensor_response::estimate_inv_emor(&mapping_curves[..]);
        dbg!(inv_emor_factors);
        sensor_response::inv_emor_factors_to_curve(&inv_emor_factors)
    };

    let mut graph_inv = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    let mut graph_linear = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    for chan in 0..3 {
        let rgb = match chan {
            0 => image::Rgb([255, 0, 0]),
            1 => image::Rgb([0, 255, 0]),
            2 => image::Rgb([0, 0, 255]),
            _ => image::Rgb([0, 0, 0]),
        };

        draw_line_segments(
            &mut graph_inv,
            inv_mapping.iter().enumerate().map(|(i, y)| {
                let x = i as f32 / (inv_mapping.len() - 1) as f32;
                (x, *y)
            }),
            rgb,
        );
        // draw_points(&mut graph_inv, inv_mapping.iter().copied(), rgb);

        for mapping in mapping_curves.iter() {
            draw_line_segments(
                &mut graph_linear,
                mapping.curve.iter().map(|p| {
                    (
                        sensor_response::lerp_slice(&inv_mapping[..], p.0),
                        sensor_response::lerp_slice(&inv_mapping[..], p.1),
                        // sensor_response::lerp_slice(&INV_EMOR_TABLE[1][..], p.0),
                        // sensor_response::lerp_slice(&INV_EMOR_TABLE[1][..], p.1),
                    )
                }),
                rgb,
            );
        }
    }

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

    // Create the HDR.
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
            let r_linear = sensor_response::lerp_slice(&inv_mapping[..], r);
            let g_linear = sensor_response::lerp_slice(&inv_mapping[..], g);
            let b_linear = sensor_response::lerp_slice(&inv_mapping[..], b);
            let r_weight = calc_weight(r, img_i == 0);
            let g_weight = calc_weight(g, img_i == 0);
            let b_weight = calc_weight(b, img_i == 0);
            hdr_image[i][0] += r_linear * inv_exposure * r_weight;
            hdr_image[i][1] += g_linear * inv_exposure * g_weight;
            hdr_image[i][2] += b_linear * inv_exposure * b_weight;
            hdr_weights[i][0] += r_weight;
            hdr_weights[i][1] += g_weight;
            hdr_weights[i][2] += b_weight;
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

    graph.save("graph.png").unwrap();
    graph_emor.save("graph_emor.png").unwrap();
    graph_inv.save("graph_inv.png").unwrap();
    graph_linear.save("graph_linear.png").unwrap();
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
