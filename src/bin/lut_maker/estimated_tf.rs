use crate::egui::{self, Ui};

pub struct EstimatedTF {
    pub rounds: usize,
    pub transfer_function_preview: Option<([Vec<f32>; 3], f32)>, // (lut, error)
    pub sensor_floor: [f32; 3],
    pub sensor_ceiling: [f32; 3],
}

impl EstimatedTF {
    pub fn new() -> EstimatedTF {
        EstimatedTF {
            rounds: 4000,
            transfer_function_preview: None,
            sensor_floor: [0.0; 3],
            sensor_ceiling: [1.0; 3],
        }
    }
}

pub fn estimated_mode_ui(
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
        // Transfer curve controls.
        ui.vertical(|ui| {
            ui.set_width(sub_area_width);

            ui.label("Transfer Function");
            ui.add_space(4.0);
            ui.add_enabled_ui(job_count == 0, |ui| {
                // Rounds slider.
                ui.add_enabled(
                    job_count == 0,
                    egui::widgets::DragValue::new(&mut app.ui_data.lock_mut().estimated.rounds)
                        .clamp_range(100..=200000)
                        .max_decimals(0)
                        .prefix("Estimation rounds: "),
                );

                if ui
                    .add_enabled(
                        job_count == 0 && total_bracket_images > 0,
                        egui::widgets::Button::new("Estimate"),
                    )
                    .clicked()
                {
                    app.estimate_everything();
                }
            });
            ui.add_space(4.0);
        });

        ui.add_space(8.0);

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
                .zip(app.ui_data.lock_mut().estimated.sensor_floor.iter_mut())
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

        ui.add_space(8.0);

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
                .zip(app.ui_data.lock_mut().estimated.sensor_ceiling.iter_mut())
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
