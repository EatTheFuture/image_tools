use std::path::PathBuf;

use crate::egui::{self, vec2, Align, Color32, Context, Stroke};

pub fn menu_bar(
    ctx: &Context,
    app: &mut crate::AppMain,
    working_dir: &mut PathBuf,
    job_count: usize,
) {
    let load_config_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Load Config")
            .add_filter("OCIO Maker config", &["ocio"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    let select_export_directory_dialog = {
        let mut d = rfd::FileDialog::new().set_title("Select Export Directory");
        let export_path: PathBuf = app.ui_data.lock().export_path.clone().into();
        if !export_path.as_os_str().is_empty() && export_path.is_dir() {
            d = d.set_directory(&export_path);
        } else if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        };
        d
    };

    egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Proper menu items.
            ui.horizontal(|ui| {
                ui.style_mut().spacing.button_padding = vec2(2.0, 0.0);
                ui.style_mut().visuals.widgets.active.bg_stroke = Stroke::NONE;
                ui.style_mut().visuals.widgets.hovered.bg_stroke = Stroke::NONE;
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                ui.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;

                egui::menu::menu_button(ui, "File", |ui| {
                    if ui
                        .add(egui::widgets::Button::new("Load Config..."))
                        .clicked()
                    {
                        if let Some(path) = load_config_dialog.clone().pick_file() {
                            app.load_config(&path);
                            if let Some(parent) = path.parent().map(|p| p.into()) {
                                *working_dir = parent;
                            }
                        }
                    }
                    ui.separator();
                    if ui.add(egui::widgets::Button::new("Quit")).clicked() {
                        ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Close);
                    }
                });
            });

            // Export UI.
            ui.with_layout(egui::Layout::right_to_left(Align::Max), |ui| {
                let mut ui_data = app.ui_data.lock_mut();

                if ui
                    .add_enabled(job_count == 0, egui::widgets::Button::new("Export Config"))
                    .clicked()
                {
                    app.export_config();
                }
                ui.add_space(16.0);
                if ui.button("Browse...").clicked() {
                    if let Some(path) = select_export_directory_dialog.pick_folder() {
                        ui_data.export_path = path.to_string_lossy().into();
                    }
                }
                ui.add(
                    egui::widgets::TextEdit::singleline(&mut ui_data.export_path)
                        .id(egui::Id::new("Export Path")),
                );
                ui.label("Config Directory: ");
            });
        });
    });
}
