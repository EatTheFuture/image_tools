use crate::egui::{self, Context, Ui};
use crate::ShowImage;

pub fn image_view(
    ctx: &Context,
    ui: &mut Ui,
    app: &mut crate::AppMain,
    image_count: usize,
    have_hdri: bool,
    have_hdri_preview_tex: bool,
) {
    ui.horizontal(|ui| {
        let spacing = 16.0;

        ui.vertical(|ui| {
            let show_image = &mut app.ui_data.lock_mut().show_image;
            if ui
                .add_enabled(
                    image_count > 0,
                    egui::widgets::RadioButton::new(
                        *show_image == ShowImage::SelectedImage,
                        "Show Selected Image",
                    ),
                )
                .clicked()
            {
                *show_image = ShowImage::SelectedImage;
            }
            if ui
                .add_enabled(
                    have_hdri,
                    egui::widgets::RadioButton::new(*show_image == ShowImage::HDRI, "Show HDRI"),
                )
                .clicked()
            {
                *show_image = ShowImage::HDRI;
            }
        });

        ui.add_space(spacing);

        if app.ui_data.lock().show_image == ShowImage::HDRI {
            ui.add_space(spacing);
            if ui
                .add(
                    egui::widgets::DragValue::new(&mut app.ui_data.lock_mut().preview_exposure)
                        .speed(0.1)
                        .prefix("Log Exposure: "),
                )
                .changed()
            {
                app.compute_hdri_preview(ctx);
            }
        }

        ui.with_layout(egui::Layout::right_to_left(), |ui| {
            ui.scope(|ui| {
                ui.add_space(6.0);
                ui.spacing_mut().slider_width = 200.0;
                ui.add_enabled(
                    image_count > 0 || have_hdri,
                    egui::widgets::Slider::new(&mut app.ui_data.lock_mut().image_zoom, 0.1..=1.0)
                        .min_decimals(1)
                        .max_decimals(2)
                        // .prefix("Zoom: ")
                        .suffix("x"),
                );
                ui.label("Zoom:");
            });
        });
    });

    let show_image = app.ui_data.lock().show_image;
    let image_zoom = app.ui_data.lock().image_zoom;
    egui::containers::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if show_image == ShowImage::HDRI && have_hdri_preview_tex {
                if let Some((ref tex_handle, width, height)) = app.ui_data.lock().hdri_preview_tex {
                    ui.image(
                        tex_handle,
                        egui::Vec2::new(width as f32 * image_zoom, height as f32 * image_zoom),
                    );
                }
            } else if show_image == ShowImage::SelectedImage && image_count > 0 {
                if let Some((ref tex_handle, width, height)) = app.ui_data.lock().image_preview_tex
                {
                    ui.image(
                        tex_handle,
                        egui::Vec2::new(width as f32 * image_zoom, height as f32 * image_zoom),
                    );
                }
            }
        });
}
