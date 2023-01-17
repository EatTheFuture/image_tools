use crate::egui::{self, Context};

pub fn ui(ctx: &Context, app: &mut crate::AppMain) {
    // Main area.
    egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
        ui.label("The working color space is what rendering, compositing, and RGB color math are done in.  It is always a linear color space, defined only by a gamut.  It does not have to match the final target display color space, although that's usually a good choice.");
        ui.add_space(8.0);

        let space = &mut app.ui_data.lock_mut().working_color_space;
        crate::colorspace_editor::chromaticity_editor(
            ui,
            space,
        );
        ui.add_space(8.0);
        crate::gamut_graph::graph(ui, space);
    });
}
