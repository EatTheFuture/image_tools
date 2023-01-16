use crate::egui::{self, Ui};

pub fn list(ui: &mut Ui, app: &mut crate::AppMain, job_count: usize) {
    let mut remove_i = None;
    let mut add_input_space = false;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        add_input_space |= ui.button("New  ➕").clicked();
    });
    ui.add_space(4.0);

    egui::containers::ScrollArea::vertical()
        .auto_shrink([true, false])
        .show(ui, |ui| {
            let ui_data = &mut *app.ui_data.lock_mut();

            let mut space_i = 0;
            let mut selected_i = ui_data.selected_space_index;

            for input_space in ui_data.color_spaces.iter() {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                        .clicked()
                    {
                        remove_i = Some(space_i);
                    }
                    if ui
                        .add(egui::widgets::SelectableLabel::new(
                            space_i == ui_data.selected_space_index,
                            &input_space.name,
                        ))
                        .clicked()
                    {
                        selected_i = space_i;
                    }
                });

                space_i += 1;
            }

            ui_data.selected_space_index = selected_i;
        });

    if add_input_space {
        app.add_input_color_space();
    }
    if let Some(space_i) = remove_i {
        app.remove_color_space(space_i);
    }
}
