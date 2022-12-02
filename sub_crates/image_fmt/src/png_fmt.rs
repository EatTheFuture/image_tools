use std::io::Read;

use crate::{error::ReadError, Image, ImageBuf};

pub fn load<R: Read>(mut reader: R) -> Result<Image, ReadError> {
    let decoder = png::Decoder::new_with_limits(
        &mut reader,
        png::Limits {
            bytes: std::usize::MAX,
        },
    );
    let mut reader = decoder.read_info()?;

    let info = reader.info();
    let dimensions = (info.width as usize, info.height as usize);
    let bit_depth = info.bit_depth;
    let color_type = info.color_type;

    if color_type == png::ColorType::Indexed {
        return Err(ReadError::UnsupportedFeature);
    }

    let mut pixel_data = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut pixel_data)?;

    use png::{BitDepth::*, ColorType::*};
    return match (color_type, bit_depth) {
        //------
        // RGB.
        (Rgb, Eight) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgb8(pixel_data),
        }),
        (Rgb, Sixteen) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgb16(
                pixel_data
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect(),
            ),
        }),

        //-------
        // RGBA.
        (Rgba, Eight) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba8(pixel_data),
        }),
        (Rgba, Sixteen) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba16(
                pixel_data
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect(),
            ),
        }),

        //------------
        // Grayscale.
        (Grayscale, Eight) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba8(pixel_data.iter().map(|&c| [c, c, c]).flatten().collect()),
        }),
        (Grayscale, Sixteen) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba16(
                pixel_data
                    .chunks(2)
                    .map(|c| {
                        let v = u16::from_be_bytes([c[0], c[1]]);
                        [v, v, v]
                    })
                    .flatten()
                    .collect(),
            ),
        }),

        //--------------------
        // Grayscale + alpha.
        (GrayscaleAlpha, Eight) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba8(
                pixel_data
                    .chunks(2)
                    .map(|c| [c[0], c[0], c[0], c[1]])
                    .flatten()
                    .collect(),
            ),
        }),
        (GrayscaleAlpha, Sixteen) => Ok(Image {
            dimensions: dimensions,
            data: ImageBuf::Rgba16(
                pixel_data
                    .chunks(4)
                    .map(|c| {
                        let v = u16::from_be_bytes([c[0], c[1]]);
                        let a = u16::from_be_bytes([c[2], c[3]]);
                        [v, v, v, a]
                    })
                    .flatten()
                    .collect(),
            ),
        }),

        _ => return Err(ReadError::UnsupportedFeature),
    };
}
