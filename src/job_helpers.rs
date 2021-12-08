use std::path::Path;

use eframe::egui;

use colorbox::formats;
use ocio_gen::lut::Lut1D;

use crate::{ImageInfo, SourceImage};

#[derive(Debug, Copy, Clone)]
pub enum ImageLoadError {
    NoAccess, // Unable to access the file (possibly doesn't exist).
    UnknownFormat,
}

pub fn load_image(path: &Path) -> Result<SourceImage, ImageLoadError> {
    // Load image.
    let img = if let Ok(f) = image::io::Reader::open(&path) {
        if let Some(Some(img)) = f
            .with_guessed_format()
            .ok()
            .map(|f| f.decode().ok().map(|f| f.to_rgb8()))
        {
            img
        } else {
            return Err(ImageLoadError::UnknownFormat);
        }
    } else {
        return Err(ImageLoadError::NoAccess);
    };

    // Get exposure metadata from EXIF data.
    let (exposure_time, fstop, sensitivity) = {
        let mut exposure_time = None;
        let mut fstop = None;
        let mut sensitivity = None;

        let mut file = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
        if let Ok(img_exif) = exif::Reader::new().read_from_container(&mut file) {
            if let Some(&exif::Value::Rational(ref n)) = img_exif
                .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
                .map(|n| &n.value)
            {
                exposure_time = Some(n[0]);
            }
            if let Some(&exif::Value::Rational(ref n)) = img_exif
                .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
                .map(|n| &n.value)
            {
                fstop = Some(n[0]);
            }
            if let Some(Some(n)) = img_exif
                .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
                .map(|n| n.value.get_uint(0))
            {
                sensitivity = Some(n);
            }
        }

        (exposure_time, fstop, sensitivity)
    };

    // Calculate over-all exposure.
    let total_exposure =
        if let (Some(exp), Some(fst), Some(sns)) = (exposure_time, fstop, sensitivity) {
            Some((sns as f64 * exp.to_f64() / (fst.to_f64() * fst.to_f64())) as f32)
        } else {
            None
        };

    // Fill in image info.
    let image_info = ImageInfo {
        filename: path
            .file_name()
            .map(|p| p.to_string_lossy().into())
            .unwrap_or_else(|| "".into()),
        full_filepath: path.to_string_lossy().into(),

        width: img.width() as usize,
        height: img.height() as usize,
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
) -> (Vec<egui::Color32>, usize, usize) {
    let old_dim = (img.image.width() as usize, img.image.height() as usize);
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

    if new_dim == old_dim {
        (
            img.image
                .pixels()
                .map(|pix| egui::Color32::from_rgb(pix[0], pix[1], pix[2]))
                .collect(),
            img.image.width() as usize,
            img.image.height() as usize,
        )
    } else {
        let resized_image = image::imageops::resize(
            &img.image,
            new_dim.0 as u32,
            new_dim.1 as u32,
            image::imageops::FilterType::Triangle,
        );
        (
            resized_image
                .pixels()
                .map(|pix| egui::Color32::from_rgb(pix[0], pix[1], pix[2]))
                .collect(),
            resized_image.width() as usize,
            resized_image.height() as usize,
        )
    }
}

pub fn load_1d_lut(path: &Path) -> Result<Lut1D, formats::ReadError> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);

    match path.extension().map(|e| e.to_str()) {
        Some(Some("cube")) => {
            let [(min1, max1, table1), (min2, max2, table2), (min3, max3, table3)] =
                formats::cube::read_1d(file)?;

            Ok(Lut1D {
                ranges: vec![(min1, max1), (min2, max2), (min3, max3)],
                tables: vec![table1, table2, table3],
            })
        }

        Some(Some("spi1d")) => {
            let (min, max, components, [table1, table2, table3]) = formats::spi1d::read(file)?;

            match components {
                1 => Ok(Lut1D {
                    ranges: vec![(min, max)],
                    tables: vec![table1],
                }),
                2 => Ok(Lut1D {
                    ranges: vec![(min, max)],
                    tables: vec![table1, table2],
                }),
                3 => Ok(Lut1D {
                    ranges: vec![(min, max)],
                    tables: vec![table1, table2, table3],
                }),
                _ => unreachable!(),
            }
        }

        _ => Err(formats::ReadError::FormatErr),
    }
}
