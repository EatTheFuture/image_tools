use std::path::PathBuf;

use crate::egui::{self, Context};

pub fn ui(ctx: &Context, app: &mut crate::AppMain, working_dir: &mut PathBuf, job_count: usize) {
    // Color space list (left-side panel).
    egui::containers::panel::SidePanel::left("color_space_list")
        .resizable(false)
        .show(ctx, |ui| {
            crate::colorspace_list::list(ui, app, job_count);
        });

    // Main area.
    egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
        // Main UI area.
        if app.ui_data.lock().selected_space_index < app.ui_data.lock().color_spaces.len() {
            crate::colorspace_editor::editor(ui, app, job_count, working_dir);

            ui.add_space(8.0);

            crate::gamut_graph::graph(ui, app);
            crate::transfer_function_graph::graph(ui, app);
        }
    });
}
