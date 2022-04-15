use std::path::PathBuf;

use crate::egui::{self, Context};
use crate::epi::Frame;

pub fn menu_bar(ctx: &Context, frame: &Frame, app: &mut crate::AppMain, working_dir: &mut PathBuf) {
    let load_config_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Load Config")
            .add_filter("OCIO Maker config", &["ocio"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
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
                    frame.quit();
                }
            });
        });
    });
}
