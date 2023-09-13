use std::path::PathBuf;

use sensor_analysis::utils::lerp_slice;

use crate::egui::{self, Ui};

pub struct ModifiedTF {
    pub loaded_lut: Option<(colorbox::lut::Lut1D, colorbox::lut::Lut1D, PathBuf)>, // (to linear, from linear, path)
    pub sensor_floor: (bool, [f32; 3]), // The bool is whether to do an adjustment at all.
    pub sensor_ceiling: (bool, [f32; 3]),
}

impl ModifiedTF {
    pub fn new() -> ModifiedTF {
        ModifiedTF {
            loaded_lut: None,
            sensor_floor: (false, [0.0; 3]),
            sensor_ceiling: (false, [1.0; 3]),
        }
    }

    /// Returns the LUT with the adjustments made from the modified settings.
    ///
    /// The returned value is an array of (lut, range start, range end) tuples,
    /// one for each channel.
    pub fn adjusted_lut(&self, to_linear: bool) -> Option<[(Vec<f32>, f32, f32); 3]> {
        let floor = self.sensor_floor.1;
        let ceiling = self.sensor_ceiling.1;

        let (to_linear_luts, to_linear_ranges, from_linear_luts, from_linear_ranges) =
            if let Some((ref lut1, ref lut2, _)) = self.loaded_lut {
                (
                    &lut1.tables[..],
                    &lut1.ranges[..],
                    &lut2.tables[..],
                    &lut2.ranges[..],
                )
            } else {
                return None;
            };

        let mut adjusted_luts = [
            (Vec::new(), 0.0, 1.0),
            (Vec::new(), 0.0, 1.0),
            (Vec::new(), 0.0, 1.0),
        ];
        for chan in 0..3 {
            let (out_floor, out_norm) = {
                let (lut, range) = if to_linear_luts.len() >= 3 {
                    (
                        &to_linear_luts[chan],
                        if to_linear_ranges.len() >= 3 {
                            to_linear_ranges[chan]
                        } else {
                            to_linear_ranges[0]
                        },
                    )
                } else {
                    (&to_linear_luts[0], to_linear_ranges[0])
                };

                let out_floor = if self.sensor_floor.0 {
                    let floor = ((floor[chan] - range.0) / (range.1 - range.0))
                        .max(0.0)
                        .min(1.0);
                    lerp_slice(lut, floor)
                } else {
                    0.0
                };
                let out_ceil = if self.sensor_ceiling.0 {
                    let ceil = ((ceiling[chan] - range.0) / (range.1 - range.0))
                        .max(0.0)
                        .min(1.0);
                    lerp_slice(lut, ceil)
                } else {
                    *lut.last().unwrap()
                };
                let out_norm = *lut.last().unwrap() / (out_ceil - out_floor);

                (out_floor, out_norm)
            };

            if to_linear {
                let (lut, range) = if to_linear_luts.len() >= 3 {
                    (
                        &to_linear_luts[chan],
                        if to_linear_ranges.len() >= 3 {
                            to_linear_ranges[chan]
                        } else {
                            to_linear_ranges[0]
                        },
                    )
                } else {
                    (&to_linear_luts[0], to_linear_ranges[0])
                };

                adjusted_luts[chan] = (
                    lut.iter().map(|y| (y - out_floor) * out_norm).collect(),
                    range.0,
                    range.1,
                );
            } else {
                let (lut, range) = if from_linear_luts.len() >= 3 {
                    (
                        &from_linear_luts[chan],
                        if from_linear_ranges.len() >= 3 {
                            from_linear_ranges[chan]
                        } else {
                            from_linear_ranges[0]
                        },
                    )
                } else {
                    (&from_linear_luts[0], from_linear_ranges[0])
                };

                adjusted_luts[chan] = (
                    lut.clone(),
                    (range.0 - out_floor) * out_norm,
                    (range.1 - out_floor) * out_norm,
                );
            }
        }

        Some(adjusted_luts)
    }
}

pub fn modified_mode_ui(
    ui: &mut Ui,
    app: &mut crate::AppMain,
    job_count: usize,
    total_bracket_images: usize,
    total_dark_images: usize,
    working_dir: &mut PathBuf,
) {
    let load_1d_lut_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Load 1D LUT")
            .add_filter("All Supported LUTs", &["spi1d", "cube"])
            .add_filter("cube", &["cube"])
            .add_filter("spi1d", &["spi1d"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    // Transfer function controls.
    let area_width = ui.available_width();
    let sub_area_width = (area_width / 3.0).min(230.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.label("LUT");
            if app.ui_data.lock().modified.loaded_lut.is_some() {
                ui.horizontal(|ui| {
                    ui.strong(
                        if let Some(name) = app
                            .ui_data
                            .lock()
                            .modified
                            .loaded_lut
                            .as_ref()
                            .unwrap()
                            .2
                            .file_name()
                        {
                            let tmp: String = name.to_string_lossy().into();
                            tmp
                        } else {
                            "Unnamed LUT".into()
                        },
                    );
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                        .clicked()
                    {
                        app.ui_data.lock_mut().modified.loaded_lut = None;
                    }
                });
                if ui
                    .add_enabled(job_count == 0, egui::widgets::Button::new("Flip LUT"))
                    .clicked()
                {
                    if let Some((ref mut lut1, ref mut lut2, _)) =
                        app.ui_data.lock_mut().modified.loaded_lut
                    {
                        std::mem::swap(lut1, lut2);
                    }
                }
            } else {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("Load 1D LUT..."))
                        .clicked()
                    {
                        if let Some(path) = load_1d_lut_dialog.clone().pick_file() {
                            app.load_lut(&path);
                            if let Some(parent) = path.parent().map(|p| p.into()) {
                                *working_dir = parent;
                            }
                        }
                    }
                });
            }
        });

        ui.add_space(8.0);

        let have_lut = app.ui_data.lock().modified.loaded_lut.is_some();

        ui.add_enabled_ui(have_lut, |ui| {
            // Sensor floor controls.
            let adjust_floor = app.ui_data.lock().modified.sensor_floor.0;
            ui.vertical(|ui| {
                ui.set_width(sub_area_width);

                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut app.ui_data.lock_mut().modified.sensor_floor.0,
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
                    .zip(app.ui_data.lock_mut().modified.sensor_floor.1.iter_mut())
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

            ui.add_space(8.0);

            // Sensor ceiling controls.
            let adjust_ceiling = app.ui_data.lock().modified.sensor_ceiling.0;
            ui.vertical(|ui| {
                ui.set_width(sub_area_width);

                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut app.ui_data.lock_mut().modified.sensor_ceiling.0,
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
                    .zip(app.ui_data.lock_mut().modified.sensor_ceiling.1.iter_mut())
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
    });
}
