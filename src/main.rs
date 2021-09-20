use clap::{App, Arg};

mod sensor_response;

use sensor_response::EMOR_TABLE;

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

    let mapping_curves = sensor_response::generate_image_mapping_curves(&images[..]);

    let graph = sensor_response::generate_mapping_graph(&mapping_curves);

    let mut graph_emor = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    for x in 0..1024 {
        let n = 5;
        for i in 0..(n + 1) {
            let i = n - i;
            let y = 0.5 - EMOR_TABLE[i][x];
            let v = (255 / (i + 1)) as u8;

            graph_emor.put_pixel(
                x as u32,
                (y.max(0.0).min(1.0) * 1023.0) as u32,
                image::Rgb([v, v, v]),
            );
        }
    }

    let mut graph_inv = image::RgbImage::from_pixel(1024, 1024, image::Rgb([0u8, 0, 0]));
    for chan in 0..3 {
        let rgb = match chan {
            0 => image::Rgb([128, 0, 0]),
            1 => image::Rgb([0, 128, 0]),
            2 => image::Rgb([0, 0, 128]),
            _ => image::Rgb([0, 0, 0]),
        };
        for (curve, ratio) in mapping_curves[chan].iter() {
            let inv_mapping = sensor_response::estimate_inverse_sensor_response(curve, *ratio);
            draw_line_segments(&mut graph_inv, inv_mapping.iter(), rgb);
            // draw_points(&mut graph_inv, inv_mapping.iter(), rgb);
        }
    }

    graph.save("graph.png").unwrap();
    // graph_emor.save("graph_emor.png").unwrap();
    graph_inv.save("graph_inv.png").unwrap();
}

pub fn draw_line_segments<'a, Itr>(img: &mut image::RgbImage, points: Itr, color: image::Rgb<u8>)
where
    Itr: std::iter::Iterator<Item = &'a (f32, f32)> + 'a,
{
    let (w, h) = (img.width(), img.height());
    let mut points = points.peekable();

    while let Some(p1) = points.next() {
        if let Some(p2) = points.peek() {
            let x1 = (p1.0 * (w - 1) as f32).min(w as f32) as u32;
            let y1 = (p1.1 * (h - 1) as f32).min(h as f32) as u32;
            let x2 = (p2.0 * (w - 1) as f32).min(w as f32) as u32;
            let y2 = (p2.1 * (h - 1) as f32).min(h as f32) as u32;

            if (y2 - y1) < (x2 - x1) {
                let mut y = y1 as f32;
                let dy = (y2 - y1) as f32 / (x2 - x1) as f32;
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
                let mut x = x1 as f32;
                let dx = (x2 - x1) as f32 / (y2 - y1) as f32;
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

pub fn draw_points<'a, Itr>(img: &mut image::RgbImage, points: Itr, color: image::Rgb<u8>)
where
    Itr: std::iter::Iterator<Item = &'a (f32, f32)> + 'a,
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
