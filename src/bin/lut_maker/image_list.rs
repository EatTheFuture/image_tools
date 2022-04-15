use std::path::PathBuf;

use crate::egui::{self, Context, Ui};

use crate::ImageViewID;

/// The image list in the left-side panel.
pub fn image_list(
    ctx: &Context,
    ui: &mut Ui,
    app: &mut crate::AppMain,
    job_count: usize,
    working_dir: &mut PathBuf,
) {
    let add_images_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Add Images")
            .add_filter(
                "All Images",
                &[
                    "jpg", "JPG", "jpeg", "JPEG", "tiff", "TIFF", "tif", "TIF", "webp", "WEBP",
                    "png", "PNG",
                ],
            )
            .add_filter("jpeg", &["jpg", "JPG", "jpeg", "JPEG"])
            .add_filter("tiff", &["tiff", "TIFF", "tif", "TIF"])
            .add_filter("webp", &["webp", "WEBP"])
            .add_filter("png", &["png", "PNG"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    // View selector.
    ui.add_space(4.0);
    {
        let image_view = &mut app.ui_data.lock_mut().image_view;
        egui::ComboBox::from_id_source("Image View Selector")
            .width(200.0)
            .selected_text(format!("{}", image_view.ui_text()))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    image_view,
                    ImageViewID::Bracketed,
                    ImageViewID::Bracketed.ui_text(),
                );
                ui.selectable_value(
                    image_view,
                    ImageViewID::LensCap,
                    ImageViewID::LensCap.ui_text(),
                );
            });
    }

    ui.add(egui::widgets::Separator::default().spacing(16.0));

    // // Selected image info.
    // // (Extra scope to contain ui_data's mutex guard.)
    // {
    //     use egui::widgets::Label;
    //     let ui_data = app.ui_data.lock();
    //     let spacing = 4.0;

    //     ui.add_space(spacing + 4.0);
    //     if ui_data.selected_image_index < ui_data.thumbnails.len() {
    //         let info = &ui_data.thumbnails[ui_data.selected_image_index].2;
    //         ui.add(Label::new("Filename:").strong());
    //         ui.indent("", |ui| ui.label(format!("{}", info.filename)));

    //         ui.add_space(spacing);
    //         ui.add(Label::new("Resolution:").strong());
    //         ui.indent("", |ui| {
    //             ui.label(format!("{} x {}", info.width, info.height))
    //         });

    //         ui.add_space(spacing);
    //         ui.add(Label::new("Log Exposure:").strong());
    //         ui.indent("", |ui| {
    //             ui.label(if let Some(exposure) = info.exposure {
    //                 format!("{:.1}", exposure.log2())
    //             } else {
    //                 "none".into()
    //             })
    //         });

    //         ui.add_space(spacing * 1.5);
    //         ui.collapsing("more", |ui| {
    //             ui.add(Label::new("Filepath:"));
    //             ui.indent("", |ui| ui.label(format!("{}", info.full_filepath)));

    //             ui.add_space(spacing);
    //             ui.add(Label::new("Exif:"));
    //             ui.indent("", |ui| {
    //                 ui.label(format!(
    //                     "Shutter speed: {}",
    //                     if let Some(e) = info.exposure_time {
    //                         if e.0 < e.1 {
    //                             format!("{}/{}", e.0, e.1)
    //                         } else {
    //                             format!("{}", e.0 as f64 / e.1 as f64)
    //                         }
    //                     } else {
    //                         "none".into()
    //                     }
    //                 ))
    //             });

    //             ui.indent("", |ui| {
    //                 ui.label(format!(
    //                     "F-stop: {}",
    //                     if let Some(f) = info.fstop {
    //                         format!("f/{:.1}", f.0 as f64 / f.1 as f64)
    //                     } else {
    //                         "none".into()
    //                     }
    //                 ))
    //             });

    //             ui.indent("", |ui| {
    //                 ui.label(format!(
    //                     "ISO: {}",
    //                     if let Some(iso) = info.iso {
    //                         format!("{}", iso)
    //                     } else {
    //                         "none".into()
    //                     }
    //                 ))
    //             });
    //         });
    //     } else {
    //         ui.label("No images loaded.");
    //     }
    // }

    // ui.add(egui::widgets::Separator::default().spacing(16.0));

    let image_view = app.ui_data.lock().image_view;
    match image_view {
        // Lens cap images.
        ImageViewID::LensCap => {
            // Image add button.
            if ui
                .add_enabled(
                    job_count == 0,
                    egui::widgets::Button::new("Add Lens Cap Image..."),
                )
                .clicked()
            {
                if let Some(paths) = add_images_dialog.clone().pick_files() {
                    app.add_lens_cap_image_files(
                        paths.iter().map(|pathbuf| pathbuf.as_path()),
                        ctx,
                    );
                    if let Some(parent) =
                        paths.get(0).map(|p| p.parent().map(|p| p.into())).flatten()
                    {
                        *working_dir = parent;
                    }
                }
            }

            // Image thumbnails.
            let mut remove_i = None;
            egui::containers::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let ui_data = &mut *app.ui_data.lock_mut();
                    let thumbnails = &ui_data.lens_cap_thumbnails;
                    let selected_image_index = &mut ui_data.selected_lens_cap_image_index;

                    for (img_i, (ref tex_handle, width, height, _)) in thumbnails.iter().enumerate()
                    {
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
                app.remove_lens_cap_image(img_i);
            }
        }

        // Bracketed exposure image sets.
        ImageViewID::Bracketed => {
            // Image set add button.
            if ui
                .add_enabled(
                    job_count == 0,
                    egui::widgets::Button::new("Add Image Set..."),
                )
                .clicked()
            {
                if let Some(paths) = add_images_dialog.clone().pick_files() {
                    app.add_bracket_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()), ctx);
                    if let Some(parent) =
                        paths.get(0).map(|p| p.parent().map(|p| p.into())).flatten()
                    {
                        *working_dir = parent;
                    }
                }
            }

            // Image thumbnails.
            let mut remove_i = (None, None); // (set index, image index)
            egui::containers::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let ui_data = &mut *app.ui_data.lock_mut();
                    let bracket_thumbnail_sets = &ui_data.bracket_thumbnail_sets;
                    let (ref mut set_index, ref mut image_index) =
                        &mut ui_data.selected_bracket_image_index;

                    for set_i in 0..bracket_thumbnail_sets.len() {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.label(format!("Image Set {}", set_i + 1));
                            if ui
                                .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                .clicked()
                            {
                                remove_i = (Some(set_i), None);
                            }
                        });
                        ui.add_space(4.0);
                        let set = &bracket_thumbnail_sets[set_i];
                        for (img_i, (ref tex_handle, width, height, _)) in set.iter().enumerate() {
                            let display_height = 64.0;
                            let display_width = display_height / *height as f32 * *width as f32;

                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::widgets::ImageButton::new(
                                            tex_handle,
                                            egui::Vec2::new(display_width, display_height),
                                        )
                                        .selected(set_i == *set_index && img_i == *image_index),
                                    )
                                    .clicked()
                                {
                                    *set_index = set_i;
                                    *image_index = img_i;
                                }
                                if ui
                                    .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                    .clicked()
                                {
                                    remove_i = (Some(set_i), Some(img_i));
                                }
                            });
                        }
                    }
                });
            match remove_i {
                (Some(set_i), Some(img_i)) => app.remove_bracket_image(set_i, img_i),
                (Some(set_i), None) => app.remove_bracket_image_set(set_i),
                _ => {}
            }
        }
    }
}
