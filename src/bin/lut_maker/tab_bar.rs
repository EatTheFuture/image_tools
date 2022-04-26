use std::path::PathBuf;

use crate::egui::{self, Ui};

use crate::{AppMode, TransferFunction};

/// Mode tabs and export buttons.
pub fn tab_bar(ui: &mut Ui, app: &mut crate::AppMain, job_count: usize, working_dir: &mut PathBuf) {
    let save_lut_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Save LUT")
            .add_filter(".spi1d", &["spi1d", "SPI1D"])
            .add_filter(".cube", &["cube", "CUBE"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    ui.with_layout(egui::Layout::right_to_left(), |ui| {
        // Export buttons.
        ui.add_space(8.0);
        if ui
            .add_enabled(
                job_count == 0
                    && (app.transfer_function_tables.lock().is_some()
                        || app.ui_data.lock().transfer_function_type
                            != TransferFunction::Estimated),
                egui::widgets::Button::new("Export 'from linear' LUT..."),
            )
            .clicked()
        {
            if let Some(path) = save_lut_dialog.clone().save_file() {
                app.save_lut(&path, false);
                if let Some(parent) = path.parent().map(|p| p.into()) {
                    *working_dir = parent;
                }
            }
        }
        if ui
            .add_enabled(
                job_count == 0
                    && (app.transfer_function_tables.lock().is_some()
                        || app.ui_data.lock().transfer_function_type
                            != TransferFunction::Estimated),
                egui::widgets::Button::new("Export 'to linear' LUT..."),
            )
            .clicked()
        {
            if let Some(path) = save_lut_dialog.clone().save_file() {
                app.save_lut(&path, true);
                if let Some(parent) = path.parent().map(|p| p.into()) {
                    *working_dir = parent;
                }
            }
        }

        // Mode tabs.
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let mode = &mut app.ui_data.lock_mut().mode;
                if ui
                    .selectable_label(*mode == AppMode::Generate, "Generate")
                    .clicked()
                {
                    *mode = AppMode::Generate;
                };
                if ui
                    .selectable_label(*mode == AppMode::Estimate, "Estimate")
                    .clicked()
                {
                    *mode = AppMode::Estimate;
                };
                if ui
                    .selectable_label(*mode == AppMode::Modify, "Modify")
                    .clicked()
                {
                    *mode = AppMode::Modify;
                };
            });
        });
    });
}
