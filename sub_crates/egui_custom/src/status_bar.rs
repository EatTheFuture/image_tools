use eframe::egui::{
    self,
    color::Rgba,
    containers::ScrollArea,
    widgets::{Button, Label, ProgressBar},
    RichText,
};
use job_queue::{JobQueue, LogLevel};

pub fn status_bar(ctx: &egui::Context, job_queue: &JobQueue) {
    egui::containers::panel::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        let log_count = job_queue.log_count();

        let mut log_string = String::new();
        let mut error_count = 0;
        let mut warning_count = 0;
        let mut note_count = 0;

        // Draw unread log messages, if any.
        if log_count > 0 {
            for i in 0..log_count {
                let log_i = (log_count - 1) - i;
                if let Some((message, level)) = job_queue.get_log(log_i) {
                    match level {
                        LogLevel::Error => {
                            error_count += 1;
                            log_string.push_str(&format!("{}:  ERROR: {}", i + 1, message));
                        }
                        LogLevel::Warning => {
                            warning_count += 1;
                            log_string.push_str(&format!("{}:  WARNING: {}", i + 1, message));
                        }
                        LogLevel::Note => {
                            note_count += 1;
                            log_string.push_str(&format!("{}:  {}", i + 1, message));
                        }
                    }
                    if log_i > 0 {
                        log_string.push_str("\n\n")
                    }
                }
            }
            ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(100.0)
                .stick_to_bottom()
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut log_string.as_str())
                            .desired_rows(1)
                            .desired_width(std::f32::INFINITY),
                    );
                });
            ui.add_space(6.0);
        }

        // Draw progress bar for any in-progress jobs.
        let jobs_are_canceling = job_queue.is_canceling();
        if let Some((text, ratio)) = job_queue.progress() {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!jobs_are_canceling, Button::new("Cancel"))
                    .clicked()
                {
                    job_queue.cancel_all_jobs();
                }
                ui.add(
                    ProgressBar::new(ratio)
                        .text(if jobs_are_canceling {
                            "Canceling..."
                        } else {
                            &text
                        })
                        .animate(true),
                );
            });
        } else if log_count > 0 {
            ui.with_layout(egui::Layout::right_to_left(), |ui| {
                if ui.add(Button::new("🗙  Clear Log")).clicked() {
                    job_queue.clear_log();
                }

                ui.add_space(6.0);

                if error_count > 0 {
                    ui.add(Label::new(
                        RichText::new(format!("Errors: {}", error_count))
                            .color(Rgba::from_rgb(1.0, 0.2, 0.1)),
                    ));
                }
                if warning_count > 0 {
                    ui.add(Label::new(
                        RichText::new(format!("Warnings: {}", warning_count))
                            .color(Rgba::from_rgb(0.6, 0.6, 0.05)),
                    ));
                }
                if note_count > 0 {
                    ui.label(format!("Notes: {}", note_count));
                }
            });
        }
    });
}
