use std::io::{Read, Seek};

use crate::{error::ReadError, Image, ImageBuf};

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    let mut decoder = jpeg_decoder::Decoder::new(&mut reader);
    decoder.read_info()?;

    let info = decoder.info().unwrap();
    let dimensions = (info.width as usize, info.height as usize);
    let pixel_count = dimensions.0 * dimensions.1;
    let pixel_format = info.pixel_format;

    let pixel_data = decoder.decode()?;

    use jpeg_decoder::PixelFormat::*;
    return match pixel_format {
        //------
        // RGB.
        RGB24 => {
            assert_eq!(pixel_data.len(), pixel_count * 3);
            Ok(Image {
                dimensions: (dimensions.0 as usize, dimensions.1 as usize),
                data: ImageBuf::Rgb8(pixel_data),
            })
        }

        //------------
        // Grayscale.
        L8 => {
            assert_eq!(pixel_data.len(), pixel_count);
            Ok(Image {
                dimensions: (dimensions.0 as usize, dimensions.1 as usize),
                data: ImageBuf::Rgb8(pixel_data.iter().map(|&c| [c, c, c]).flatten().collect()),
            })
        }
        L16 => {
            assert_eq!(pixel_data.len(), pixel_count * 2);
            Ok(Image {
                dimensions: (dimensions.0 as usize, dimensions.1 as usize),
                // NOTE: jpeg-decode doesn't document the endianness of
                // their 16-bit buffers, but examining the code in that
                // crate indicates that it's native endian.  So this
                // transformation should be correct.
                data: ImageBuf::Rgba16(
                    pixel_data
                        .chunks(2)
                        .map(|c| {
                            let v = u16::from_ne_bytes([c[0], c[1]]);
                            [v, v, v]
                        })
                        .flatten()
                        .collect(),
                ),
            })
        }

        _ => Err(ReadError::UnsupportedFeature),
    };
}
