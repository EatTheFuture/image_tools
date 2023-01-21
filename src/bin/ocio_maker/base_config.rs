use crate::egui::{self, Context};

pub fn ui(ctx: &Context, app: &mut crate::AppMain) {
    egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
        // Base config preset.
        ui.horizontal(|ui| {
            let mut ui_data = app.ui_data.lock_mut();
            ui.label("Configuration base: ");
            egui::ComboBox::from_id_source("Base Preset")
                .width(256.0)
                .selected_text(String::from(ui_data.base_preset.ui_text()))
                .show_ui(ui, |ui| {
                    for bp in super::BASE_PRESETS {
                        ui.selectable_value(&mut ui_data.base_preset, *bp, bp.ui_text());
                    }
                });
        });
        ui.add_space(8.0);

        let base_preset = app.ui_data.lock().base_preset;
        match base_preset {
            crate::BasePreset::Custom => {
                ui.label("The working color space is what rendering, compositing, and RGB color math are done in.  It is always a linear color space, defined only by a gamut.  It does not have to match the final target display color space, although that's usually a good choice.");
                ui.add_space(8.0);

                let space = &mut app.ui_data.lock_mut().working_color_space;
                crate::colorspace_editor::chromaticity_editor(
                    ui,
                    "Working color space: ",
                    space,
                );
                ui.add_space(8.0);
                crate::gamut_graph::graph(ui, space);
            }

            _ => {}
        }
    });
}
