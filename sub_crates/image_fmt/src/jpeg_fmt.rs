use std::io::{Read, Seek};

use crate::{error::ReadError, Image, ImageData};

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    let mut decoder = jpeg_decoder::Decoder::new(&mut reader);
    decoder.read_info()?;

    let info = decoder.info().unwrap();
    let dimensions = (info.width as usize, info.height as usize);
    let pixel_format = info.pixel_format;

    let pixel_data = decoder.decode()?;

    use jpeg_decoder::PixelFormat::*;
    return match pixel_format {
        //------
        // RGB.
        RGB24 => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageData::Rgb8(pixel_data),
        }),

        //------------
        // Grayscale.
        L8 => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            data: ImageData::Rgb8(pixel_data.iter().map(|&c| [c, c, c]).flatten().collect()),
        }),
        L16 => Ok(Image {
            dimensions: (dimensions.0 as usize, dimensions.0 as usize),
            // NOTE: jpeg-decode doesn't document the endianness of
            // their 16-bit buffers, but examining the code in that
            // crate indicates that it's native endian.  So this
            // transformation should be correct.
            data: ImageData::Rgba16(
                pixel_data
                    .chunks(2)
                    .map(|c| {
                        let v = u16::from_ne_bytes([c[0], c[1]]);
                        [v, v, v]
                    })
                    .flatten()
                    .collect(),
            ),
        }),

        _ => return Err(ReadError::UnsupportedFeature),
    };
}
