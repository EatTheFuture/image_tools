mod error;
mod jpeg_fmt;
mod png_fmt;
mod tiff_fmt;

use std::io::{Read, Seek};

pub use error::ReadError;

#[derive(Debug, Clone)]
pub struct Image {
    pub dimensions: (usize, usize),
    pub data: ImageBuf,
}

impl Image {
    #[inline(always)]
    pub fn width(&self) -> usize {
        self.dimensions.0
    }

    #[inline(always)]
    pub fn height(&self) -> usize {
        self.dimensions.1
    }

    pub fn resized(&self, width: usize, height: usize) -> Self {
        use fast_image_resize as fir;
        use ImageBuf::*;

        match self.data {
            Rgb8(ref data) => {
                assert_eq!(data.len(), self.dimensions.0 * self.dimensions.1 * 3);
                let src_view = fir::DynamicImageView::U8x3(
                    fir::ImageView::from_buffer(
                        (self.dimensions.0 as u32).try_into().unwrap(),
                        (self.dimensions.1 as u32).try_into().unwrap(),
                        &data,
                    )
                    .unwrap(),
                );

                let mut new_data = vec![0u8; width * height * 3];
                let mut dst_view = fir::DynamicImageViewMut::U8x3(
                    fir::ImageViewMut::from_buffer(
                        (width as u32).try_into().unwrap(),
                        (height as u32).try_into().unwrap(),
                        &mut new_data,
                    )
                    .unwrap(),
                );

                let mut resizer =
                    fir::Resizer::new(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));
                resizer.resize(&src_view, &mut dst_view).unwrap();

                Image {
                    dimensions: (width, height),
                    data: Rgb8(new_data),
                }
            }

            _ => todo!(),
        }
    }

    pub fn to_8_bit(self) -> Self {
        Image {
            dimensions: self.dimensions,
            data: self.data.to_8_bit(),
        }
    }

    pub fn to_rgba(self) -> Self {
        Image {
            dimensions: self.dimensions,
            data: self.data.to_rgba(),
        }
    }
}

/// Image data, laid out as `RGBRGBRGB...` or `RGBARGBARGBA...` in scanline order.
#[derive(Debug, Clone)]
pub enum ImageBuf {
    /// 8-bit unsigned RGB channels.
    Rgb8(Vec<u8>),

    /// 16-bit unsigned RGB channels.
    Rgb16(Vec<u16>),

    /// 8-bit unsigned RGBA channels.
    Rgba8(Vec<u8>),

    /// 16-bit unsigned RGBA channels.
    Rgba16(Vec<u16>),
}

impl ImageBuf {
    pub fn to_rgb(self) -> Self {
        use ImageBuf::*;

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

    pub fn to_rgba(self) -> Self {
        use ImageBuf::*;

        match self {
            Rgba8(_) | Rgba16(_) => self,

            Rgb8(mut data) => {
                let mut from_i = data.len() - 1;
                data.resize(data.len() / 3 * 4, 0);
                let mut to_i = data.len() - 1;

                while from_i > 2 {
                    data[to_i] = std::u8::MAX;
                    to_i -= 1;
                    for _ in 0..3 {
                        data[to_i] = data[from_i];
                        from_i -= 1;
                        to_i -= 1;
                    }
                }

                Rgba8(data)
            }

            Rgb16(mut data) => {
                let mut from_i = data.len() - 1;
                data.resize(data.len() / 3 * 4, 0);
                let mut to_i = data.len() - 1;

                while from_i > 2 {
                    data[to_i] = std::u16::MAX;
                    to_i -= 1;
                    for _ in 0..3 {
                        data[to_i] = data[from_i];
                        from_i -= 1;
                        to_i -= 1;
                    }
                }

                Rgba16(data)
            }
        }
    }

    pub fn to_8_bit(self) -> Self {
        use ImageBuf::*;
        match self {
            Rgb8(_) | Rgba8(_) => self,
            Rgb16(data) => Rgb8(data.iter().map(|&v| (v >> 8) as u8).collect()),
            Rgba16(data) => Rgba8(data.iter().map(|&v| (v >> 8) as u8).collect()),
        }
    }

    pub fn to_16_bit(self) -> Self {
        use ImageBuf::*;
        match self {
            Rgb16(_) | Rgba16(_) => self,
            Rgb8(data) => Rgb16(data.iter().map(|&v| (v as u16) << 8).collect()),
            Rgba8(data) => Rgba16(data.iter().map(|&v| (v as u16) << 8).collect()),
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

    // Try jpeg.
    match jpeg_fmt::load(&mut reader) {
        Err(ReadError::UnknownFormat) => {} // Continue to try next format.
        r => {
            return r;
        }
    }
    reader.rewind()?;

    // No formats matched.
    return Err(ReadError::UnknownFormat);
}
