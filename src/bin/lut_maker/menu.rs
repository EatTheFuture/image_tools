use crate::egui::{self, Context};
use crate::epi::Frame;

/// The top menu bar of the UI.
pub fn menu_bar(ctx: &Context, frame: &Frame) {
    egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                ui.separator();
                if ui.add(egui::widgets::Button::new("Quit")).clicked() {
                    frame.quit();
                }
            });
        });
    });
}
