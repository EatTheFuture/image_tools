use crate::egui::{self, Align, Context};
use crate::Tabs;

pub fn top_bar(ctx: &Context, app: &mut crate::AppMain) {
    egui::containers::panel::TopBottomPanel::top("top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Tabs
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    let selected_tab = &mut app.ui_data.lock_mut().selected_tab;
                    if ui
                        .selectable_label(
                            *selected_tab == Tabs::InputTransforms,
                            "Input Transforms",
                        )
                        .clicked()
                    {
                        *selected_tab = Tabs::InputTransforms;
                    };
                    if ui
                        .selectable_label(
                            *selected_tab == Tabs::WorkingColorSpace,
                            "Working Color Space",
                        )
                        .clicked()
                    {
                        *selected_tab = Tabs::WorkingColorSpace;
                    };
                });
            });

            // Preset selector.
            ui.with_layout(egui::Layout::right_to_left(Align::Max), |ui| {
                let mut ui_data = app.ui_data.lock_mut();
                egui::ComboBox::from_id_source("Base Preset")
                    .width(256.0)
                    .selected_text(String::from(ui_data.base_preset.ui_text()))
                    .show_ui(ui, |ui| {
                        for bp in super::BASE_PRESETS {
                            ui.selectable_value(&mut ui_data.base_preset, *bp, bp.ui_text());
                        }
                    });
                ui.label("Configuration base: ");
            });
        });
    });
}
