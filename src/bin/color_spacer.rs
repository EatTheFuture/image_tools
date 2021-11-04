#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{path::Path, sync::Arc};

use eframe::{egui, epi};

use shared_data::Shared;
// use sensor_analysis::invert_luma_map;

use lib::{ImageInfo, SourceImage};

fn main() {
    clap::App::new("Color Spacer")
        .version("0.1")
        .author("Nathan Vegdahl")
        .about("Does all things color space")
        .get_matches();

    eframe::run_native(
        Box::new(AppMain {
            job_queue: job_queue::JobQueue::new(),

            image_sets: Shared::new(Vec::new()),
            transfer_function_table: Shared::new(None),

            ui_data: Shared::new(UIData {
                active_image_set: 0,
                selected_image_index: 0,

                thumbnail_sets: vec![vec![]],
                image_preview_tex: None,
                image_preview_tex_needs_update: false,
                transfer_function_preview: None,
            }),
        }),
        eframe::NativeOptions {
            drag_and_drop_support: true, // Enable drag-and-dropping files on Windows.
            ..eframe::NativeOptions::default()
        },
    );
}

struct AppMain {
    job_queue: job_queue::JobQueue,

    image_sets: Shared<Vec<Vec<SourceImage>>>,
    transfer_function_table: Shared<Option<(Vec<f32>, f32, f32)>>, // (table, x_min, x_max)

    ui_data: Shared<UIData>,
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
struct UIData {
    // Widgets.
    active_image_set: usize,
    selected_image_index: usize,

    // Other stuff.
    thumbnail_sets: Vec<
        Vec<(
            (Vec<egui::Color32>, usize, usize),
            Option<egui::TextureId>,
            ImageInfo,
        )>,
    >,
    image_preview_tex: Option<(egui::TextureId, usize, usize)>,
    image_preview_tex_needs_update: bool,
    transfer_function_preview: Option<Vec<(f32, f32)>>,
}

impl epi::App for AppMain {
    fn name(&self) -> &str {
        "Color Spacer"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
        let repaint_signal = Arc::clone(&frame.repaint_signal());
        self.job_queue.set_update_fn(move || {
            repaint_signal.request_repaint();
        });
    }

    // Called before shutdown.
    fn save(&mut self, _storage: &mut dyn epi::Storage) {
        // Don't need to do anything.
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        let job_count = self.job_queue.job_count();

        // File dialogs used in the UI.
        let add_images_dialog = rfd::FileDialog::new()
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

        //----------------
        // GUI.

        // Menu bar.
        egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    ui.separator();
                    if ui.add(egui::widgets::Button::new("Quit")).clicked() {
                        frame.quit();
                    }
                });
            });
        });

        // Status bar and log (footer).
        egui_custom::status_bar(ctx, &self.job_queue);

        // Image list (left-side panel).
        egui::containers::panel::SidePanel::left("image_list")
            .min_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                // let mut remove_i = None; // Temp to store index of an image to remove.

                // // Selected image info.
                // // (Extra scope to contain ui_data's mutex guard.)
                // {
                //     use egui::widgets::Label;
                //     let ui_data = self.ui_data.lock();
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

                // Image thumbnails.
                egui::containers::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let ui_data = &mut *self.ui_data.lock_mut();
                        let thumbnail_sets = &mut ui_data.thumbnail_sets;
                        let active_image_set = &mut ui_data.active_image_set;
                        let selected_image_index = &mut ui_data.selected_image_index;

                        for set_i in 0..thumbnail_sets.len() {
                            let set = &mut thumbnail_sets[set_i];
                            for (img_i, ((pixels, width, height), ref mut tex_id, _)) in
                                set.iter_mut().enumerate()
                            {
                                let display_height = 64.0;
                                let display_width = display_height / *height as f32 * *width as f32;

                                // Build thumbnail texture if it doesn't already exist.
                                if tex_id.is_none() {
                                    *tex_id = Some(
                                        frame
                                            .tex_allocator()
                                            .alloc_srgba_premultiplied((*width, *height), &pixels),
                                    );
                                }

                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::widgets::ImageButton::new(
                                                tex_id.unwrap(),
                                                egui::Vec2::new(display_width, display_height),
                                            )
                                            .selected(img_i == *selected_image_index),
                                        )
                                        .clicked()
                                    {
                                        *selected_image_index = img_i;
                                        // self.compute_image_preview(img_i);
                                    }
                                    if ui
                                        .add_enabled(
                                            job_count == 0,
                                            egui::widgets::Button::new("🗙"),
                                        )
                                        .clicked()
                                    {
                                        // remove_i = Some(img_i);
                                    }
                                });
                            }
                        }
                    });

                // if let Some(img_i) = remove_i {
                //     self.remove_image(img_i);
                // }
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // Image set add button.
                if ui
                    .add_enabled(
                        job_count == 0,
                        egui::widgets::Button::new("Add Image Set..."),
                    )
                    .clicked()
                {
                    if let Some(paths) = add_images_dialog.clone().pick_files() {
                        self.add_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()));
                    }
                }

                ui.label(" ➡ ");

                // Estimate transfer function button.
                if ui
                    .add_enabled(
                        job_count == 0,
                        egui::widgets::Button::new("Estimate Transfer Function"),
                    )
                    .clicked()
                {
                    self.estimate_transfer_function();
                }
            });

            ui.add(egui::widgets::Separator::default().spacing(12.0));

            if let Some(transfer_function_curve) = &self.ui_data.lock().transfer_function_preview {
                use egui::widgets::plot::{Line, Plot, Value, Values};
                ui.add(
                    Plot::new("transfer_function")
                        .line(Line::new(Values::from_values_iter(
                            transfer_function_curve
                                .iter()
                                .copied()
                                .map(|(x, y)| Value::new(x, y)),
                        )))
                        .view_aspect(1.0),
                );
            }
        });

        //----------------
        // Processing.

        // Collect dropped files.
        if !ctx.input().raw.dropped_files.is_empty() {
            self.add_image_files(
                ctx.input()
                    .raw
                    .dropped_files
                    .iter()
                    .map(|dropped_file| dropped_file.path.as_ref().unwrap().as_path()),
            );
        }
    }
}

impl AppMain {
    fn add_image_files<'a, I: Iterator<Item = &'a Path>>(&mut self, paths: I) {
        let mut image_paths: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let image_sets = self.image_sets.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;

            // Create a new image and thumbnail set.
            image_sets.lock_mut().push(Vec::new());
            ui_data.lock_mut().thumbnail_sets.push(Vec::new());

            // Load and add images.
            for (img_i, path) in image_paths.drain(..).enumerate() {
                if status.lock().is_canceled() {
                    break;
                }

                status.lock_mut().set_progress(
                    format!("Loading: {}", path.to_string_lossy()),
                    (img_i + 1) as f32 / len,
                );

                // Load image.
                let img = match lib::job_helpers::load_image(&path) {
                    Ok(img) => img,
                    Err(lib::job_helpers::ImageLoadError::NoAccess) => {
                        status.lock_mut().log_error(format!(
                            "Unable to access file \"{}\".",
                            path.to_string_lossy()
                        ));
                        return;
                    },
                    Err(lib::job_helpers::ImageLoadError::UnknownFormat) => {
                        status.lock_mut().log_error(format!(
                            "Unrecognized image file format: \"{}\".",
                            path.to_string_lossy()
                        ));
                        return;
                    }
                };

                // Ensure it has the same resolution as the other images.
                if !image_sets.lock().last().unwrap().is_empty() {
                    let needed_width = image_sets.lock().last().unwrap()[0].image.width();
                    let needed_height = image_sets.lock().last().unwrap()[0].image.height();
                    if img.image.width() != needed_width || img.image.height() != needed_height {
                        status.lock_mut().log_error(format!(
                            "Image has a different resolution that the others in the set: \"{}\".  Not loading.  Note: all images in a set must have the same resolution.",
                            path.to_string_lossy()
                        ));
                        continue;
                    }
                }

                // Check if we got exposure data from it.
                if img.info.exposure.is_none() {
                    status.lock_mut().log_warning(format!(
                        "Image file lacks Exif data needed to compute exposure value: \"{}\".  HDRI merging will not work correctly.",
                        path.to_string_lossy()
                    ));
                }

                // Make a thumbnail texture.
                let thumbnail = lib::job_helpers::make_image_preview(
                    &img,
                    Some(128),
                    None,
                );

                // Add image and thumbnail to our lists.
                {
                    let mut ui_data = ui_data.lock_mut();
                    let set = ui_data.thumbnail_sets.last_mut().unwrap();
                    set.push((thumbnail, None, img.info.clone()));
                    set.sort_unstable_by(|a, b| a.2.exposure.partial_cmp(&b.2.exposure).unwrap());
                }
                {
                    let mut image_sets = image_sets.lock_mut();
                    let set = image_sets.last_mut().unwrap();
                    set.push(img);
                    set.sort_unstable_by(|a, b| a.info.exposure.partial_cmp(&b.info.exposure).unwrap());
                }
            }
        });

        // let selected_image_index = self.ui_data.lock().selected_image_index;
        // self.compute_image_preview(selected_image_index);
    }

    fn estimate_transfer_function(&self) {
        todo!()
    }
}
