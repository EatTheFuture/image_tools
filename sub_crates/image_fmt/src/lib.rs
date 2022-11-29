mod error;
mod tiff_fmt;

use std::io::{Read, Seek};

use error::ReadError;

#[derive(Debug, Clone)]
pub enum ImageData {
    /// 8-bit unsigned RGB channels.
    Rgb8(Vec<[u8; 3]>),

    /// 16-bit unsigned RGB channels.
    Rgb16(Vec<[u16; 3]>),
}

#[derive(Debug, Clone)]
pub struct Image {
    pub dimensions: (usize, usize),
    pub data: ImageData,
}

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    // Try tiff.
    match tiff_fmt::load(&mut reader) {
        Err(ReadError::UnknownFormat) => {} // Continue to try next format.
        r => return r,
    }

    // No formats matched.
    return Err(ReadError::UnknownFormat);
}
