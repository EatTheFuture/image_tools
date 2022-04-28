use std::path::PathBuf;

use sensor_analysis::ExposureMapping;

use crate::egui::{self, Ui};

pub struct ModifiedTF {
    pub loaded_lut: Option<(colorbox::lut::Lut1D, colorbox::lut::Lut1D, PathBuf)>, // (to linear, from linear, path)
    pub sensor_floor: [f32; 3],
    pub sensor_ceiling: [f32; 3],
    pub exposure_mappings: [Vec<ExposureMapping>; 3],
}

impl ModifiedTF {
    pub fn new() -> ModifiedTF {
        ModifiedTF {
            loaded_lut: None,
            sensor_floor: [0.0; 3],
            sensor_ceiling: [1.0; 3],
            exposure_mappings: [Vec::new(), Vec::new(), Vec::new()],
        }
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
            ui.label("LUT");
            if let Some((_, _, ref filepath)) = app.ui_data.lock().modified.loaded_lut {
                ui.horizontal(|ui| {
                    ui.strong(if let Some(name) = filepath.file_name() {
                        let tmp: String = name.to_string_lossy().into();
                        tmp
                    } else {
                        "Unnamed LUT".into()
                    });
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                        .clicked()
                    {
                        // app.remove_loaded_lut();
                    }
                });
                if ui.button("Flip").clicked() {
                    // app.flip_loaded_lut();
                }
            } else {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("Load 1D LUT..."))
                        .clicked()
                    {
                        if let Some(path) = load_1d_lut_dialog.clone().pick_file() {
                            // app.load_lut(&path);
                            if let Some(parent) = path.parent().map(|p| p.into()) {
                                *working_dir = parent;
                            }
                        }
                    }
                });
            }
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
                .zip(app.ui_data.lock_mut().modified.sensor_floor.iter_mut())
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
                .zip(app.ui_data.lock_mut().modified.sensor_ceiling.iter_mut())
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
