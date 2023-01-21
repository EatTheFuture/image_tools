use crate::egui::{self, Context};
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
                        .selectable_label(*selected_tab == Tabs::BaseConfig, "Base Config")
                        .clicked()
                    {
                        *selected_tab = Tabs::BaseConfig;
                    };
                    if ui
                        .selectable_label(
                            *selected_tab == Tabs::InputTransforms,
                            "Input Transforms",
                        )
                        .clicked()
                    {
                        *selected_tab = Tabs::InputTransforms;
                    };
                });
            });
        });
    });
}
