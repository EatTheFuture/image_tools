use std::path::PathBuf;

use crate::egui::{self, Align, Ui};

use crate::{AppMode, ExportFormat};

/// Mode tabs and export buttons.
pub fn bar(ui: &mut Ui, app: &mut crate::AppMain, job_count: usize, working_dir: &mut PathBuf) {
    let export_lut_dialog = {
        let exp_fmt = app.ui_data.lock().export_format;
        let mut d = rfd::FileDialog::new()
            .set_title("Save LUT")
            .set_file_name(&format!(".{}", exp_fmt.ext()))
            .add_filter(
                exp_fmt.ui_text(),
                &[exp_fmt.ext(), &exp_fmt.ext().to_uppercase()],
            );
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    ui.with_layout(egui::Layout::right_to_left(Align::Max), |ui| {
        // Export buttons.
        let export_enabled = {
            let mode = app.ui_data.lock().mode;
            job_count == 0
                && ((mode == AppMode::Estimate && app.transfer_function_tables.lock().is_some())
                    || (mode == AppMode::Modify
                        && app.ui_data.lock().modified.loaded_lut.is_some())
                    || mode == AppMode::Generate)
        };
        ui.add_space(8.0);
        if ui
            .add_enabled(
                export_enabled,
                egui::widgets::Button::new("Export 'from linear' LUT..."),
            )
            .clicked()
        {
            if let Some(path) = export_lut_dialog.clone().save_file() {
                app.export_lut(&path, false);
                if let Some(parent) = path.parent().map(|p| p.into()) {
                    *working_dir = parent;
                }
            }
        }
        if ui
            .add_enabled(
                export_enabled,
                egui::widgets::Button::new("Export 'to linear' LUT..."),
            )
            .clicked()
        {
            if let Some(path) = export_lut_dialog.clone().save_file() {
                app.export_lut(&path, true);
                if let Some(parent) = path.parent().map(|p| p.into()) {
                    *working_dir = parent;
                }
            }
        }
        ui.add_space(8.0);
        {
            let exp_fmt = &mut app.ui_data.lock_mut().export_format;
            egui::ComboBox::from_label("Export format:")
                .selected_text(exp_fmt.ui_text())
                .show_ui(ui, |ui| {
                    ui.selectable_value(exp_fmt, ExportFormat::Cube, ExportFormat::Cube.ui_text());
                    ui.selectable_value(
                        exp_fmt,
                        ExportFormat::Spi1D,
                        ExportFormat::Spi1D.ui_text(),
                    );
                });
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
