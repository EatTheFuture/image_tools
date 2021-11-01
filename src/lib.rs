pub mod job_helpers;

#[derive(Debug)]
pub struct SourceImage {
    pub image: image::RgbImage,
    pub info: ImageInfo,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub filename: String,
    pub full_filepath: String,

    pub width: usize,
    pub height: usize,
    pub exposure: Option<f32>,

    pub exposure_time: Option<(u32, u32)>, // Ratio.
    pub fstop: Option<(u32, u32)>,         // Ratio.
    pub iso: Option<u32>,
}
