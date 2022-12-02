use std::io::{Read, Seek};

use tiff::{decoder::DecodingResult, ColorType};

use crate::{error::ReadError, Image, ImageBuf};

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    let mut decoder =
        tiff::decoder::Decoder::new(&mut reader)?.with_limits(tiff::decoder::Limits::unlimited());

    let dimensions = decoder.dimensions()?;
    let colortype = decoder.colortype()?;
    let data = decoder.read_image()?;

    return match (colortype, data) {
        //------
        // RGB.
        (ColorType::RGB(_), DecodingResult::U8(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgb8(pixel_data),
        }),
        (ColorType::RGB(_), DecodingResult::U16(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgb16(pixel_data),
        }),

        //-------
        // RGBA.
        (ColorType::RGBA(_), DecodingResult::U8(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgba8(pixel_data),
        }),
        (ColorType::RGBA(_), DecodingResult::U16(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgba16(pixel_data),
        }),

        //------------
        // Grayscale.
        (ColorType::Gray(_), DecodingResult::U8(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgb8(pixel_data.iter().map(|&c| [c, c, c]).flatten().collect()),
        }),
        (ColorType::Gray(_), DecodingResult::U16(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgb16(pixel_data.iter().map(|&c| [c, c, c]).flatten().collect()),
        }),

        //--------------------
        // Grayscale + alpha.
        (ColorType::GrayA(_), DecodingResult::U8(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgba8(
                pixel_data
                    .chunks(2)
                    .map(|c| [c[0], c[0], c[0], c[1]])
                    .flatten()
                    .collect(),
            ),
        }),
        (ColorType::GrayA(_), DecodingResult::U16(pixel_data)) => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageBuf::Rgba16(
                pixel_data
                    .chunks(2)
                    .map(|c| [c[0], c[0], c[0], c[1]])
                    .flatten()
                    .collect(),
            ),
        }),

        _ => return Err(ReadError::UnsupportedFeature),
    };
}
