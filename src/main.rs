use clap::{App, Arg};

fn main() {
    let matches = App::new("My Super Program")
        .version("1.0")
        .author("Nathan Vegdahl")
        .about("Merges LDR images into an HDRI")
        .arg(
            Arg::with_name("INPUT")
                .help("input image file")
                .required(true)
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

    let filename = matches.value_of("INPUT").unwrap();

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

    let upper_left = img.get_pixel(0, 0);

    println!(
        "Hello, {}! {:?}\n{} {} {}\n{}",
        filename, upper_left, exposure_time, fstop, sensitivity, total_exposure,
    );
}
