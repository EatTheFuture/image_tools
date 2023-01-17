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
        let selected_space_index = app.ui_data.lock().selected_space_index;
        if selected_space_index < app.ui_data.lock().color_spaces.len() {
            let mut ui_data = app.ui_data.lock_mut();
            let space = &mut ui_data.color_spaces[selected_space_index];

            if let Err(message) = crate::colorspace_editor::editor(
                ui,
                space,
                &format!("input_space{}", selected_space_index),
                job_count,
                working_dir,
            ) {
                app.job_queue.log_error(message);
            };

            ui.add_space(8.0);

            crate::gamut_graph::graph(ui, space);
            crate::transfer_function_graph::graph(ui, space);
        }
    });
}
