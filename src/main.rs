use clap::{App, Arg};

mod sensor_response;

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

    let mats = sensor_response::generate_mapping_matrix(&images[..]);

    use image::Rgb;
    let mut graph = image::RgbImage::from_pixel(256, 256, Rgb([0u8, 0, 0]));
    let scale = 1024.0f32 / (images.len() - 1) as f32;
    for y in 0..256 {
        for x in 0..256 {
            let r = mats[0][y][x] as f32 * scale;
            let g = mats[1][y][x] as f32 * scale;
            let b = mats[2][y][x] as f32 * scale;

            graph.put_pixel(
                x as u32,
                y as u32,
                Rgb([
                    r.max(0.0).min(255.0) as u8,
                    g.max(0.0).min(255.0) as u8,
                    b.max(0.0).min(255.0) as u8,
                ]),
            );
        }
    }

    // println!("{:?}", mats);

    graph.save("graph.png").unwrap();
}
