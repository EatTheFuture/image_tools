mod error;
mod png_fmt;
mod tiff_fmt;

use std::io::{Read, Seek};

use error::ReadError;

#[derive(Debug, Clone)]
pub struct Image {
    pub dimensions: (usize, usize),
    pub data: ImageData,
}

/// Image data, laid out as `RGBRGBRGB...` or `RGBARGBARGBA...` in scanline order.
#[derive(Debug, Clone)]
pub enum ImageData {
    /// 8-bit unsigned RGB channels.
    Rgb8(Vec<u8>),

    /// 16-bit unsigned RGB channels.
    Rgb16(Vec<u16>),

    /// 8-bit unsigned RGBA channels.
    Rgba8(Vec<u8>),

    /// 16-bit unsigned RGBA channels.
    Rgba16(Vec<u16>),
}

impl ImageData {
    pub fn to_rgb(self) -> Self {
        use ImageData::*;

        match self {
            Rgb8(_) | Rgb16(_) => self,

            Rgba8(mut data) => {
                let mut from_i = 4;
                let mut to_i = 3;

                while from_i < data.len() {
                    for _ in 0..3 {
                        data[to_i] = data[from_i];
                        from_i += 1;
                        to_i += 1;
                    }
                    from_i += 1;
                }

                data.truncate(data.len() / 4 * 3);

                Rgb8(data)
            }

            Rgba16(mut data) => {
                let mut from_i = 4;
                let mut to_i = 3;

                while from_i < data.len() {
                    for _ in 0..3 {
                        data[to_i] = data[from_i];
                        from_i += 1;
                        to_i += 1;
                    }
                    from_i += 1;
                }

                data.truncate(data.len() / 4 * 3);

                Rgb16(data)
            }
        }
    }
}

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    // Try tiff.
    match tiff_fmt::load(&mut reader) {
        Err(ReadError::UnknownFormat) => {} // Continue to try next format.
        r => {
            return r;
        }
    }
    reader.rewind()?;

    // Try png.
    match png_fmt::load(&mut reader) {
        Err(ReadError::UnknownFormat) => {} // Continue to try next format.
        r => {
            return r;
        }
    }
    reader.rewind()?;

    // No formats matched.
    return Err(ReadError::UnknownFormat);
}
