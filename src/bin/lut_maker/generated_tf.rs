use crate::egui::{self, Ui};

use sensor_analysis::ExposureMapping;

pub struct GeneratedTF {
    pub transfer_function_type: TransferFunction,
    pub transfer_function_resolution: usize,
    pub normalize_transfer_function: bool,
    pub sensor_floor: [f32; 3],
    pub sensor_ceiling: [f32; 3],
    pub exposure_mappings: [Vec<ExposureMapping>; 3],
}

impl GeneratedTF {
    pub fn new() -> GeneratedTF {
        GeneratedTF {
            transfer_function_type: TransferFunction::sRGB,
            transfer_function_resolution: 4096,
            normalize_transfer_function: false,
            sensor_floor: [0.0; 3],
            sensor_ceiling: [1.0; 3],
            exposure_mappings: [Vec::new(), Vec::new(), Vec::new()],
        }
    }
}

pub fn generated_mode_ui(
    ui: &mut Ui,
    app: &mut crate::AppMain,
    job_count: usize,
    total_bracket_images: usize,
    total_dark_images: usize,
) {
    // Transfer function controls.
    let area_width = ui.available_width();
    let sub_area_width = (area_width / 3.0).min(230.0);
    ui.horizontal(|ui| {
        // Transfer function controls.
        ui.vertical(|ui| {
            let mut ui_data = app.ui_data.lock_mut();

            ui.label("Transfer Function");
            ui.add_space(4.0);
            ui.add_enabled_ui(job_count == 0, |ui| {
                egui::ComboBox::from_id_source("Transfer Function Type")
                    .width(180.0)
                    .selected_text(format!(
                        "{}",
                        ui_data.generated.transfer_function_type.ui_text()
                    ))
                    .show_ui(ui, |ui| {
                        for tf in TRANSFER_FUNCTIONS.iter() {
                            ui.selectable_value(
                                &mut ui_data.generated.transfer_function_type,
                                *tf,
                                tf.ui_text(),
                            );
                        }
                    })
            });
            ui.add_space(4.0);
            ui.add_enabled(
                job_count == 0,
                egui::widgets::DragValue::new(&mut ui_data.generated.transfer_function_resolution)
                    .clamp_range(2..=(1 << 16))
                    .max_decimals(0)
                    .prefix("LUT resolution: "),
            );
            ui.add_enabled(
                job_count == 0,
                egui::widgets::Checkbox::new(
                    &mut ui_data.generated.normalize_transfer_function,
                    "Normalize",
                ),
            );
        });

        ui.add_space(48.0);

        // Sensor floor controls.
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.horizontal(|ui| {
                ui.label("Sensor Noise Floor");
                ui.add_space(4.0);
                if ui
                    .add_enabled(
                        job_count == 0 && (total_bracket_images > 0 || total_dark_images > 0),
                        egui::widgets::Button::new("Estimate"),
                    )
                    .clicked()
                {
                    app.estimate_sensor_floor();
                }
            });
            ui.add_space(4.0);
            for (label, value) in ["R: ", "G: ", "B: "]
                .iter()
                .zip(app.ui_data.lock_mut().generated.sensor_floor.iter_mut())
            {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    ui.add_enabled(
                        job_count == 0,
                        egui::widgets::Slider::new(value, 0.0..=1.0)
                            .max_decimals(5)
                            .min_decimals(5),
                    );
                });
            }
        });

        ui.add_space(0.0);

        // Sensor ceiling controls.
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.horizontal(|ui| {
                ui.label("Sensor Ceiling");
                ui.add_space(4.0);
                if ui
                    .add_enabled(
                        job_count == 0 && total_bracket_images > 0,
                        egui::widgets::Button::new("Estimate"),
                    )
                    .clicked()
                {
                    app.estimate_sensor_ceiling();
                }
            });
            ui.add_space(4.0);
            for (label, value) in ["R: ", "G: ", "B: "]
                .iter()
                .zip(app.ui_data.lock_mut().generated.sensor_ceiling.iter_mut())
            {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    ui.add_enabled(
                        job_count == 0,
                        egui::widgets::Slider::new(value, 0.0..=1.0)
                            .max_decimals(5)
                            .min_decimals(5),
                    );
                });
            }
        });
    });
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TransferFunction {
    Linear,
    CanonLog1,
    CanonLog2,
    CanonLog3,
    DJIDlog,
    FujifilmFlog,
    HLG,
    NikonNlog,
    PanasonicVlog,
    PQ,
    PQ_108,
    PQ_1000,
    Rec709,
    SonySlog1,
    SonySlog2,
    SonySlog3,
    sRGB,
}

pub const TRANSFER_FUNCTIONS: &[TransferFunction] = &[
    TransferFunction::Linear,
    TransferFunction::sRGB,
    TransferFunction::Rec709,
    TransferFunction::HLG,
    TransferFunction::PQ,
    TransferFunction::PQ_108,
    TransferFunction::PQ_1000,
    TransferFunction::CanonLog1,
    TransferFunction::CanonLog2,
    TransferFunction::CanonLog3,
    TransferFunction::DJIDlog,
    TransferFunction::FujifilmFlog,
    TransferFunction::NikonNlog,
    TransferFunction::PanasonicVlog,
    TransferFunction::SonySlog1,
    TransferFunction::SonySlog2,
    TransferFunction::SonySlog3,
];

impl TransferFunction {
    pub fn to_linear_fc(&self, n: f32, floor: f32, ceil: f32, normalize: bool) -> f32 {
        let (_, _, _, linear_top, _) = self.constants();
        let out_floor = self.to_linear(floor);
        let out_ceil = self.to_linear(ceil);

        let mut out = self.to_linear(n);
        out = (out - out_floor) / (out_ceil - out_floor);
        if !normalize {
            out *= linear_top;
        }

        out
    }

    pub fn from_linear_fc(&self, mut n: f32, floor: f32, ceil: f32, normalize: bool) -> f32 {
        let (_, _, _, linear_top, _) = self.constants();
        let in_floor = self.to_linear(floor);
        let in_ceil = self.to_linear(ceil);

        if !normalize {
            n /= linear_top;
        }
        n = in_floor + (n * (in_ceil - in_floor));

        self.from_linear(n)
    }

    pub fn to_linear(&self, n: f32) -> f32 {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;
        match *self {
            Linear => n,

            CanonLog1 => canon_log1::to_linear(n),
            CanonLog2 => canon_log2::to_linear(n),
            CanonLog3 => canon_log3::to_linear(n),
            DJIDlog => dji_dlog::to_linear(n),
            FujifilmFlog => fujifilm_flog::to_linear(n),
            HLG => hlg::to_linear(n),
            NikonNlog => nikon_nlog::to_linear(n),
            PanasonicVlog => panasonic_vlog::to_linear(n),
            PQ => pq::to_linear(n),
            PQ_108 => pq::to_linear(n) * (1.0 / 108.0),
            PQ_1000 => pq::to_linear(n) * (1.0 / 1000.0),
            Rec709 => rec709::to_linear(n),
            SonySlog1 => sony_slog1::to_linear(n),
            SonySlog2 => sony_slog2::to_linear(n),
            SonySlog3 => sony_slog3::to_linear(n),
            sRGB => srgb::to_linear(n),
        }
    }

    pub fn from_linear(&self, n: f32) -> f32 {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;
        match *self {
            Linear => n,

            CanonLog1 => canon_log1::from_linear(n),
            CanonLog2 => canon_log2::from_linear(n),
            CanonLog3 => canon_log3::from_linear(n),
            DJIDlog => dji_dlog::from_linear(n),
            FujifilmFlog => fujifilm_flog::from_linear(n),
            HLG => hlg::from_linear(n),
            NikonNlog => nikon_nlog::from_linear(n),
            PanasonicVlog => panasonic_vlog::from_linear(n),
            PQ => pq::from_linear(n),
            PQ_108 => pq::from_linear(n * 108.0),
            PQ_1000 => pq::from_linear(n * 1000.0),
            Rec709 => rec709::from_linear(n),
            SonySlog1 => sony_slog1::from_linear(n),
            SonySlog2 => sony_slog2::from_linear(n),
            SonySlog3 => sony_slog3::from_linear(n),
            sRGB => srgb::from_linear(n),
        }
    }

    /// Returns (NONLINEAR_BLACK, NONLINEAR_MAX, LINEAR_MIN, LINEAR_MAX,
    /// LINEAR_SATURATE) for the transfer function.
    ///
    /// - NONLINEAR_BLACK is the non-linear value of linear = 0.0.
    /// - NONLINEAR_MAX is the maximum nonlinear value that should be
    ///   reportable by a camera sensor.  Usually 1.0, but some transfer
    ///   functions are weird.
    /// - LINEAR_MIN/MAX are the linear values when the encoded value is
    ///   0.0 and 1.0.
    /// - LINEAR_SATURATE is the linear value when the encoded value is
    ///   NONLINEAR_MAX.  Usually the same as LINEAR_MAX, but some
    ///   transfer functions are weird.
    #[inline(always)]
    pub fn constants(&self) -> (f32, f32, f32, f32, f32) {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;
        match *self {
            Linear => (0.0, 1.0, 0.0, 1.0, 1.0),

            CanonLog1 => {
                use canon_log1::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            CanonLog2 => {
                use canon_log2::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            CanonLog3 => {
                use canon_log3::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            DJIDlog => {
                use dji_dlog::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            FujifilmFlog => {
                use fujifilm_flog::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            HLG => (0.0, 1.0, 0.0, 1.0, 1.0),
            NikonNlog => {
                use nikon_nlog::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            PanasonicVlog => {
                use panasonic_vlog::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            PQ => (0.0, 1.0, 0.0, pq::LUMINANCE_MAX, pq::LUMINANCE_MAX),
            PQ_108 => (
                0.0,
                1.0,
                0.0,
                pq::LUMINANCE_MAX / 108.0,
                pq::LUMINANCE_MAX / 108.0,
            ),
            PQ_1000 => (
                0.0,
                1.0,
                0.0,
                pq::LUMINANCE_MAX / 1000.0,
                pq::LUMINANCE_MAX / 1000.0,
            ),
            Rec709 => (0.0, 1.0, 0.0, 1.0, 1.0),
            SonySlog1 => {
                use sony_slog1::*;
                (
                    CV_BLACK,
                    CV_SATURATION,
                    LINEAR_MIN,
                    LINEAR_MAX,
                    self.to_linear(CV_SATURATION),
                )
            }
            SonySlog2 => {
                use sony_slog2::*;
                (
                    CV_BLACK,
                    CV_SATURATION,
                    LINEAR_MIN,
                    LINEAR_MAX,
                    self.to_linear(CV_SATURATION),
                )
            }
            SonySlog3 => {
                use sony_slog3::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            sRGB => (0.0, 1.0, 0.0, 1.0, 1.0),
        }
    }

    pub fn ui_text(&self) -> &'static str {
        use TransferFunction::*;
        match *self {
            Linear => "Linear",

            CanonLog1 => "Canon Log",
            CanonLog2 => "Canon Log 2",
            CanonLog3 => "Canon Log 3",
            DJIDlog => "DJI D-Log",
            FujifilmFlog => "Fujifilm F-Log",
            HLG => "Rec.2100 - HLG",
            NikonNlog => "Nikon N-Log",
            PanasonicVlog => "Panasonic V-Log",
            PQ => "Rec.2100 - PQ",
            PQ_108 => "Rec.2100 - PQ - 108 nits",
            PQ_1000 => "Rec.2100 - PQ - 1000 nits",
            Rec709 => "Rec.709",
            SonySlog1 => "Sony S-Log",
            SonySlog2 => "Sony S-Log2",
            SonySlog3 => "Sony S-Log3",
            sRGB => "sRGB",
        }
    }
}
