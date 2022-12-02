pub mod job_helpers;

pub use image_fmt::ImageBuf;

#[derive(Debug)]
pub struct SourceImage {
    pub image: image_fmt::Image,
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

// Returns the y value at the given x value.
pub fn lerp_curve_at_x(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.0.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].1, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.0) / (p2.0 - p1.0);
    p1.1 + ((p2.1 - p1.1) * alpha)
}

// Returns the x value at the given y value.
pub fn lerp_curve_at_y(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.1.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].0, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.1) / (p2.1 - p1.1);
    p1.0 + ((p2.0 - p1.0) * alpha)
}
