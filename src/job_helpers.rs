use std::{fs::File, io::BufReader, path::Path};

use colorbox::{formats, lut::Lut1D};

use sensor_analysis::Histogram;

use crate::{ImageInfo, SourceImage};
use image_fmt::ImageBuf;

pub fn load_image(path: &Path) -> Result<SourceImage, image_fmt::ReadError> {
    // Load image.
    let img = {
        let mut img = image_fmt::load(BufReader::new(File::open(&path)?))?;
        img.data = img.data.to_rgb();
        img
    };

    // Get exposure metadata from EXIF data.
    let (exposure_time, fstop, sensitivity) = {
        let mut exposure_time = None;
        let mut fstop = None;
        let mut sensitivity = None;

        let mut file = std::io::BufReader::new(std::fs::File::open(&path)?);
        if let Ok(img_exif) = exif::Reader::new().read_from_container(&mut file) {
            if let Some(&exif::Value::Rational(ref n)) = img_exif
                .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
                .map(|n| &n.value)
            {
                if n[0].num != 0 && n[0].denom != 0 {
                    exposure_time = Some(n[0]);
                }
            }
            if let Some(&exif::Value::Rational(ref n)) = img_exif
                .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
                .map(|n| &n.value)
            {
                if n[0].num != 0 && n[0].denom != 0 {
                    fstop = Some(n[0]);
                }
            }
            if let Some(Some(n)) = img_exif
                .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
                .map(|n| n.value.get_uint(0))
            {
                if n != 0 {
                    sensitivity = Some(n);
                }
            }
        }

        (exposure_time, fstop, sensitivity)
    };

    // Calculate over-all exposure.
    let total_exposure = match (exposure_time, fstop, sensitivity) {
        (Some(exp), Some(fst), Some(sns)) => {
            Some((sns as f64 * exp.to_f64() / (fst.to_f64() * fst.to_f64())) as f32)
        }
        (Some(exp), None, Some(sns)) => Some((sns as f64 * exp.to_f64()) as f32),
        (Some(exp), Some(fst), None) => Some((exp.to_f64() / (fst.to_f64() * fst.to_f64())) as f32),
        (Some(exp), None, None) => Some(exp.to_f64() as f32),
        _ => None,
    };

    // Fill in image info.
    let image_info = ImageInfo {
        filename: path
            .file_name()
            .map(|p| p.to_string_lossy().into())
            .unwrap_or_else(|| "".into()),
        full_filepath: path.to_string_lossy().into(),

        width: img.width(),
        height: img.height(),
        exposure: total_exposure,

        exposure_time: exposure_time.map(|n| (n.num, n.denom)),
        fstop: fstop.map(|n| (n.num, n.denom)),
        iso: sensitivity,
    };

    // Add image to our list of source images.
    Ok(SourceImage {
        image: img,
        info: image_info,
    })
}

pub fn make_image_preview(
    img: &SourceImage,
    max_width: Option<usize>,
    max_height: Option<usize>,
) -> (Vec<u8>, usize, usize) {
    let old_dim = img.image.dimensions;
    let new_dim = match (max_width, max_height) {
        (None, None) => old_dim,
        (Some(w), None) => {
            if w < old_dim.0 {
                (w, old_dim.1 * w / old_dim.0)
            } else {
                old_dim
            }
        }
        (None, Some(h)) => {
            if h < old_dim.1 {
                (old_dim.0 * h / old_dim.1, h)
            } else {
                old_dim
            }
        }
        (Some(w), Some(h)) => {
            if w >= old_dim.0 && h >= old_dim.1 {
                old_dim
            } else {
                let new_w = old_dim.0 * h / old_dim.1;
                let new_h = old_dim.1 * w / old_dim.0;
                if new_w > w {
                    (w, new_h)
                } else {
                    (new_w, h)
                }
            }
        }
    };

    let preview_img = img
        .image
        .clone()
        .to_8_bit()
        .resized(new_dim.0, new_dim.1)
        .to_rgba();

    (
        if let image_fmt::ImageBuf::Rgba8(buf) = preview_img.data {
            buf
        } else {
            unreachable!()
        },
        new_dim.0,
        new_dim.1,
    )
}

pub fn compute_image_histograms(src_img: &SourceImage) -> [Histogram; 3] {
    let mut histograms = [
        Histogram::default(),
        Histogram::default(),
        Histogram::default(),
    ];

    match src_img.image.data {
        ImageBuf::Rgb8(ref buf) => {
            let bucket_count = 1 << 8;
            for chan in 0..3 {
                histograms[chan] =
                    Histogram::from_iter(buf.chunks(3).map(|c| c[chan]), bucket_count);
            }
        }

        ImageBuf::Rgb16(ref buf) => {
            let bucket_count = 1 << 16;
            for chan in 0..3 {
                histograms[chan] =
                    Histogram::from_iter(buf.chunks(3).map(|c| c[chan]), bucket_count);
            }
        }

        _ => panic!(),
    }

    histograms
}

pub fn load_1d_lut<P: AsRef<Path>>(path: P) -> Result<Lut1D, formats::ReadError> {
    use std::io::Seek;

    let path: &Path = path.as_ref();
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);

    match path.extension().map(|e| e.to_str()) {
        Some(Some("cube")) => {
            // There are actually two different .cube formats, so we try both.
            if let Ok(lut) = formats::cube_iridas::read_1d(&mut file) {
                Ok(lut)
            } else {
                file.rewind()?;
                if let (Some(lut), None) = formats::cube_resolve::read(&mut file)? {
                    Ok(lut)
                } else {
                    Err(formats::ReadError::FormatErr)
                }
            }
        }

        Some(Some("spi1d")) => Ok(formats::spi1d::read(&mut file)?),

        _ => Err(formats::ReadError::FormatErr),
    }
}

/// Ensures that a directory path exists and that we have permission to
/// write to it.  If it doesn't exists, this will attempt to create it.
///
/// Will return an error if:
/// - The path exists, but is not a directory.
/// - The path exists, but we don't have permission to write to it.
/// - The path doesn't exist, and we are unable to create it.
pub fn ensure_dir_exists<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let path: &Path = path.as_ref();

    if !path.exists() {
        std::fs::create_dir_all(path)?;
    } else {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Specified path is not a directory",
            ));
        }
        if metadata.permissions().readonly() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Specified path is read only",
            ));
        }
    }
    Ok(())
}
