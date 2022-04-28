use crate::egui::{self, Ui};

pub fn estimated_mode_ui(
    ui: &mut Ui,
    app: &mut crate::AppMain,
    job_count: usize,
    total_bracket_images: usize,
) {
    // Transfer function controls.
    ui.horizontal(|ui| {
        // Rounds slider.
        ui.add_enabled(
            job_count == 0,
            egui::widgets::DragValue::new(&mut app.ui_data.lock_mut().estimated.rounds)
                .clamp_range(100..=200000)
                .max_decimals(0)
                .prefix("Estimation rounds: "),
        );

        // Estimate transfer function button.
        if ui
            .add_enabled(
                job_count == 0 && total_bracket_images > 0,
                egui::widgets::Button::new("Estimate Everything"),
            )
            .clicked()
        {
            app.estimate_everything();
        }
    });
}
