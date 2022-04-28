use crate::egui::{self, Ui};

use crate::TRANSFER_FUNCTIONS;

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
