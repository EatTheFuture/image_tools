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

pub mod colors {
    use eframe::egui::Color32;

    pub const WHITE: Color32 = Color32::from_rgba_premultiplied(220, 220, 220, 0);
    pub const GRAY: Color32 = Color32::from_rgba_premultiplied(128, 128, 128, 0);
    pub const RED: Color32 = Color32::from_rgba_premultiplied(220, 20, 20, 0);
    pub const GREEN: Color32 = Color32::from_rgba_premultiplied(20, 220, 20, 0);
    pub const BLUE: Color32 = Color32::from_rgba_premultiplied(20, 20, 220, 0);
    pub const CYAN: Color32 = Color32::from_rgba_premultiplied(10, 180, 180, 0);
    pub const MAGENTA: Color32 = Color32::from_rgba_premultiplied(220, 20, 220, 0);
    pub const YELLOW: Color32 = Color32::from_rgba_premultiplied(220, 220, 20, 0);
}
