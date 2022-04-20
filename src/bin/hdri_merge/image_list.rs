use crate::egui::{self, Context, RichText, Ui};

pub fn image_list(ctx: &Context, ui: &mut Ui, app: &mut crate::AppMain, job_count: usize) {
    let mut remove_i = None; // Temp to store index of an image to remove.

    // Selected image info.
    // (Extra scope to contain ui_data's mutex guard.)
    {
        use egui::widgets::Label;
        let ui_data = app.ui_data.lock();
        let spacing = 4.0;

        ui.add_space(spacing + 4.0);
        if ui_data.selected_image_index < ui_data.thumbnails.len() {
            let info = &ui_data.thumbnails[ui_data.selected_image_index].3;
            ui.add(Label::new(RichText::new("Filename:").strong()));
            ui.indent("", |ui| ui.label(format!("{}", info.filename)));

            ui.add_space(spacing);
            ui.add(Label::new(RichText::new("Resolution:").strong()));
            ui.indent("", |ui| {
                ui.label(format!("{} x {}", info.width, info.height))
            });

            ui.add_space(spacing);
            ui.add(Label::new(RichText::new("Log Exposure:").strong()));
            ui.indent("", |ui| {
                ui.label(if let Some(exposure) = info.exposure {
                    format!("{:.1}", exposure.log2())
                } else {
                    "none".into()
                })
            });

            ui.add_space(spacing * 1.5);
            ui.collapsing("more", |ui| {
                ui.add(Label::new("Filepath:"));
                ui.indent("", |ui| ui.label(format!("{}", info.full_filepath)));

                ui.add_space(spacing);
                ui.add(Label::new("Exif:"));
                ui.indent("", |ui| {
                    ui.label(format!(
                        "Shutter speed: {}",
                        if let Some(e) = info.exposure_time {
                            if e.0 < e.1 {
                                format!("{}/{}", e.0, e.1)
                            } else {
                                format!("{}", e.0 as f64 / e.1 as f64)
                            }
                        } else {
                            "none".into()
                        }
                    ))
                });

                ui.indent("", |ui| {
                    ui.label(format!(
                        "F-stop: {}",
                        if let Some(f) = info.fstop {
                            format!("f/{:.1}", f.0 as f64 / f.1 as f64)
                        } else {
                            "none".into()
                        }
                    ))
                });

                ui.indent("", |ui| {
                    ui.label(format!(
                        "ISO: {}",
                        if let Some(iso) = info.iso {
                            format!("{}", iso)
                        } else {
                            "none".into()
                        }
                    ))
                });
            });
        } else {
            ui.label("No images loaded.");
        }
    }

    ui.add(egui::widgets::Separator::default().spacing(16.0));

    // Image thumbnails.
    egui::containers::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let ui_data = &mut *app.ui_data.lock_mut();
            let thumbnails = &ui_data.thumbnails;
            let selected_image_index = &mut ui_data.selected_image_index;

            for (img_i, (ref tex_handle, width, height, _)) in thumbnails.iter().enumerate() {
                let display_height = 64.0;
                let display_width = display_height / *height as f32 * *width as f32;

                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::widgets::ImageButton::new(
                                tex_handle,
                                egui::Vec2::new(display_width, display_height),
                            )
                            .selected(img_i == *selected_image_index),
                        )
                        .clicked()
                    {
                        *selected_image_index = img_i;
                        app.compute_image_preview(img_i, ctx);
                    }
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                        .clicked()
                    {
                        remove_i = Some(img_i);
                    }
                });
            }
        });

    if let Some(img_i) = remove_i {
        app.remove_image(img_i, ctx);
    }
}
