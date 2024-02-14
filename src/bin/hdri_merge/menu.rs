use crate::egui::{self, Context};

pub fn menu_bar(
    ctx: &Context,
    app: &mut crate::AppMain,
    add_images_dialog: &rfd::FileDialog,
    save_hdri_dialog: &rfd::FileDialog,
    have_hdri: bool,
    job_count: usize,
) {
    egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                if ui
                    .add_enabled(job_count == 0, egui::widgets::Button::new("Add Images..."))
                    .clicked()
                {
                    if let Some(paths) = add_images_dialog.clone().pick_files() {
                        app.add_image_files(paths, ctx);
                    }
                }

                if ui
                    .add_enabled(
                        have_hdri && job_count == 0,
                        egui::widgets::Button::new("Export HDRI..."),
                    )
                    .clicked()
                {
                    if let Some(path) = save_hdri_dialog.clone().save_file() {
                        app.save_hdri(path);
                    }
                }

                ui.separator();
                if ui.add(egui::widgets::Button::new("Quit")).clicked() {
                    ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Close);
                }
            });
        });
    });
}
