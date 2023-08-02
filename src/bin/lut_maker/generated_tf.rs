use crate::egui::{self, Ui};

pub struct GeneratedTF {
    pub transfer_function_type: TransferFunction,
    pub transfer_function_resolution: usize,
    pub sensor_floor: (bool, [f32; 3]), // The bool is whether to do an adjustment at all.
    pub sensor_ceiling: (bool, [f32; 3]),
}

impl GeneratedTF {
    pub fn new() -> GeneratedTF {
        GeneratedTF {
            transfer_function_type: TransferFunction::sRGB,
            transfer_function_resolution: 4096,
            sensor_floor: (false, [0.0; 3]),
            sensor_ceiling: (false, [1.0; 3]),
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
        });

        ui.add_space(48.0);
        // Sensor floor controls.
        let adjust_floor = app.ui_data.lock().generated.sensor_floor.0;
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut app.ui_data.lock_mut().generated.sensor_floor.0,
                    "Adjust Noise Floor",
                );
                ui.add_space(4.0);
                if ui
                    .add_enabled(
                        job_count == 0
                            && adjust_floor
                            && (total_bracket_images > 0 || total_dark_images > 0),
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
                .zip(app.ui_data.lock_mut().generated.sensor_floor.1.iter_mut())
            {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    ui.add_enabled(
                        job_count == 0 && adjust_floor,
                        egui::widgets::Slider::new(value, 0.0..=1.0)
                            .max_decimals(5)
                            .min_decimals(5),
                    );
                });
            }
        });

        ui.add_space(0.0);

        // Sensor ceiling controls.
        let adjust_ceiling = app.ui_data.lock().generated.sensor_ceiling.0;
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut app.ui_data.lock_mut().generated.sensor_ceiling.0,
                    "Adjust Ceiling",
                );
                ui.add_space(4.0);
                if ui
                    .add_enabled(
                        job_count == 0 && adjust_ceiling && total_bracket_images > 0,
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
                .zip(app.ui_data.lock_mut().generated.sensor_ceiling.1.iter_mut())
            {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    ui.add_enabled(
                        job_count == 0 && adjust_ceiling,
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
    BlackmagicFilmGen5,
    DavinciIntermediate,
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
    RedLog3G10,
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
    TransferFunction::BlackmagicFilmGen5,
    TransferFunction::DavinciIntermediate,
    TransferFunction::CanonLog1,
    TransferFunction::CanonLog2,
    TransferFunction::CanonLog3,
    TransferFunction::DJIDlog,
    TransferFunction::FujifilmFlog,
    TransferFunction::NikonNlog,
    TransferFunction::PanasonicVlog,
    TransferFunction::RedLog3G10,
    TransferFunction::SonySlog1,
    TransferFunction::SonySlog2,
    TransferFunction::SonySlog3,
];

impl TransferFunction {
    pub fn to_linear_fc(
        &self,
        n: f32,
        floor: Option<f32>,
        ceil: Option<f32>,
        normalize: bool,
    ) -> f32 {
        let (nonlinear_black, nonlinear_max, _, linear_top, _) = self.constants();
        let out_floor = self.to_linear(floor.unwrap_or(nonlinear_black));
        let out_ceil = self.to_linear(ceil.unwrap_or(nonlinear_max));

        let mut out = self.to_linear(n);
        out = (out - out_floor) / (out_ceil - out_floor);
        if !normalize {
            out *= linear_top;
        }

        out
    }

    pub fn from_linear_fc(
        &self,
        mut n: f32,
        floor: Option<f32>,
        ceil: Option<f32>,
        normalize: bool,
    ) -> f32 {
        let (nonlinear_black, nonlinear_max, _, linear_top, _) = self.constants();
        let in_floor = self.to_linear(floor.unwrap_or(nonlinear_black));
        let in_ceil = self.to_linear(ceil.unwrap_or(nonlinear_max));

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

            BlackmagicFilmGen5 => blackmagic::film_gen5::to_linear(n),
            DavinciIntermediate => blackmagic::davinci_intermediate::to_linear(n),
            CanonLog1 => canon::log1::to_linear(n),
            CanonLog2 => canon::log2::to_linear(n),
            CanonLog3 => canon::log3::to_linear(n),
            DJIDlog => dji::dlog::to_linear(n),
            FujifilmFlog => fujifilm::flog::to_linear(n),
            HLG => rec2100_hlg::to_linear(n),
            NikonNlog => nikon::nlog::to_linear(n),
            PanasonicVlog => panasonic::vlog::to_linear(n),
            PQ => rec2100_pq::to_linear(n),
            PQ_108 => rec2100_pq::to_linear(n) * (1.0 / 108.0),
            PQ_1000 => rec2100_pq::to_linear(n) * (1.0 / 1000.0),
            Rec709 => rec709::to_linear(n),
            RedLog3G10 => red::log3g10::to_linear(n),
            SonySlog1 => sony::slog1::to_linear(n),
            SonySlog2 => sony::slog2::to_linear(n),
            SonySlog3 => sony::slog3::to_linear(n),
            sRGB => srgb::to_linear(n),
        }
    }

    pub fn from_linear(&self, n: f32) -> f32 {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;
        match *self {
            Linear => n,

            BlackmagicFilmGen5 => blackmagic::film_gen5::from_linear(n),
            DavinciIntermediate => blackmagic::davinci_intermediate::from_linear(n),
            CanonLog1 => canon::log1::from_linear(n),
            CanonLog2 => canon::log2::from_linear(n),
            CanonLog3 => canon::log3::from_linear(n),
            DJIDlog => dji::dlog::from_linear(n),
            FujifilmFlog => fujifilm::flog::from_linear(n),
            HLG => rec2100_hlg::from_linear(n),
            NikonNlog => nikon::nlog::from_linear(n),
            PanasonicVlog => panasonic::vlog::from_linear(n),
            PQ => rec2100_pq::from_linear(n),
            PQ_108 => rec2100_pq::from_linear(n * 108.0),
            PQ_1000 => rec2100_pq::from_linear(n * 1000.0),
            Rec709 => rec709::from_linear(n),
            RedLog3G10 => red::log3g10::from_linear(n),
            SonySlog1 => sony::slog1::from_linear(n),
            SonySlog2 => sony::slog2::from_linear(n),
            SonySlog3 => sony::slog3::from_linear(n),
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

            BlackmagicFilmGen5 => {
                use blackmagic::film_gen5::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            DavinciIntermediate => {
                use blackmagic::davinci_intermediate::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            CanonLog1 => {
                use canon::log1::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            CanonLog2 => {
                use canon::log2::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            CanonLog3 => {
                use canon::log3::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            DJIDlog => {
                use dji::dlog::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            FujifilmFlog => {
                use fujifilm::flog::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            HLG => (0.0, 1.0, 0.0, 1.0, 1.0),
            NikonNlog => {
                use nikon::nlog::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            PanasonicVlog => {
                use panasonic::vlog::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            PQ => (
                0.0,
                1.0,
                0.0,
                rec2100_pq::LUMINANCE_MAX,
                rec2100_pq::LUMINANCE_MAX,
            ),
            PQ_108 => (
                0.0,
                1.0,
                0.0,
                rec2100_pq::LUMINANCE_MAX / 108.0,
                rec2100_pq::LUMINANCE_MAX / 108.0,
            ),
            PQ_1000 => (
                0.0,
                1.0,
                0.0,
                rec2100_pq::LUMINANCE_MAX / 1000.0,
                rec2100_pq::LUMINANCE_MAX / 1000.0,
            ),
            Rec709 => (0.0, 1.0, 0.0, 1.0, 1.0),
            RedLog3G10 => {
                use red::log3g10::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            SonySlog1 => {
                use sony::slog1::*;
                (
                    NONLINEAR_BLACK,
                    NONLINEAR_SATURATION,
                    LINEAR_MIN,
                    LINEAR_MAX,
                    self.to_linear(NONLINEAR_SATURATION),
                )
            }
            SonySlog2 => {
                use sony::slog2::*;
                (
                    NONLINEAR_BLACK,
                    NONLINEAR_SATURATION,
                    LINEAR_MIN,
                    LINEAR_MAX,
                    self.to_linear(NONLINEAR_SATURATION),
                )
            }
            SonySlog3 => {
                use sony::slog3::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX, LINEAR_MAX)
            }
            sRGB => (0.0, 1.0, 0.0, 1.0, 1.0),
        }
    }

    pub fn ui_text(&self) -> &'static str {
        use TransferFunction::*;
        match *self {
            Linear => "Linear",

            BlackmagicFilmGen5 => "BMD Film Gen5",
            DavinciIntermediate => "DaVinci Intermediate",
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
            RedLog3G10 => "RED Log3G10",
            SonySlog1 => "Sony S-Log",
            SonySlog2 => "Sony S-Log2",
            SonySlog3 => "Sony S-Log3",
            sRGB => "sRGB",
        }
    }
}
