use std::io::{Read, Seek};

use crate::{error::ReadError, Image, ImageData};

pub fn load<R: Read + Seek>(mut reader: R) -> Result<Image, ReadError> {
    let mut decoder =
        tiff::decoder::Decoder::new(&mut reader)?.with_limits(tiff::decoder::Limits::unlimited());

    let dimensions = decoder.dimensions()?;
    let colortype = decoder.colortype()?;
    let data = decoder.read_image()?;

    todo!()
}
