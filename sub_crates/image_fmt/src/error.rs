#[derive(Debug)]
pub enum ReadError {
    IO(std::io::Error),
    UnknownFormat,
    UnsupportedFeature,
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::IO(e) => e.fmt(f),
            ReadError::UnknownFormat => write!(f, "UnknownFormat: could not determine the image file format."),
            ReadError::UnsupportedFeature => write!(f, "UnsupportedFeature: the image file uses a feature that is currently unsupported such that image loading isn't possible."),
        }
    }
}

//-------------------------------------------------------------
// From impls.

impl From<std::io::Error> for ReadError {
    fn from(other: std::io::Error) -> Self {
        Self::IO(other)
    }
}

impl From<tiff::TiffError> for ReadError {
    fn from(other: tiff::TiffError) -> Self {
        use tiff::TiffError::*;
        match other {
            IoError(e) => Self::IO(e),
            FormatError(_) => Self::UnknownFormat,

            IntSizeError | UnsupportedError(_) => Self::UnsupportedFeature,

            LimitsExceeded => panic!(),
            UsageError(_) => panic!(),
        }
    }
}

impl From<png::DecodingError> for ReadError {
    fn from(other: png::DecodingError) -> Self {
        use png::DecodingError::*;
        match other {
            IoError(e) => Self::IO(e),
            Format(_) => Self::UnknownFormat,

            Parameter(_) => panic!(),
            LimitsExceeded => panic!(),
        }
    }
}

impl From<jpeg_decoder::Error> for ReadError {
    fn from(other: jpeg_decoder::Error) -> Self {
        use jpeg_decoder::Error::*;
        match other {
            Io(e) => Self::IO(e),
            Format(_) => Self::UnknownFormat,
            Unsupported(_) => Self::UnsupportedFeature,

            Internal(_) => panic!(),
        }
    }
}
