#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui, epi};
use rayon::prelude::*;

use sensor_analysis::{eval_transfer_function_lut, invert_transfer_function_lut};
use shared_data::Shared;

use lib::{ImageInfo, SourceImage};

fn main() {
    clap::App::new("HDRI Merge")
        .version("1.0")
        .author("Nathan Vegdahl")
        .about("Merges LDR images into an HDRI")
        .get_matches();

    eframe::run_native(
        Box::new(AppMain {
            job_queue: job_queue::JobQueue::new(),

            images: Shared::new(Vec::new()),
            hdri_merger: Shared::new(None),
            hdri_preview: Shared::new(None),
            image_preview: Shared::new(None),

            ui_data: Shared::new(UIData {
                preview_exposure: 0.0,
                selected_image_index: 0,
                image_zoom: 1.0,
                show_image: ShowImage::SelectedImage,

                thumbnails: Vec::new(),
                image_preview_tex: None,
                image_preview_tex_needs_update: false,
                hdri_preview_tex: None,
                hdri_preview_tex_needs_update: false,
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

    images: Shared<Vec<SourceImage>>,
    hdri_merger: Shared<Option<HDRIMerger>>,
    hdri_preview: Shared<Option<(Vec<egui::Color32>, usize, usize)>>,
    image_preview: Shared<Option<(Vec<egui::Color32>, usize, usize)>>,

    ui_data: Shared<UIData>,
}

/// The data that the UI needs realtime access to for responsiveness.
struct UIData {
    // Widgets.
    preview_exposure: f32,
    selected_image_index: usize,
    image_zoom: f32,
    show_image: ShowImage,

    // Others.
    thumbnails: Vec<(
        (Vec<egui::Color32>, usize, usize),
        Option<egui::TextureId>,
        ImageInfo,
    )>,
    image_preview_tex: Option<(egui::TextureId, usize, usize)>,
    image_preview_tex_needs_update: bool,
    hdri_preview_tex: Option<(egui::TextureId, usize, usize)>,
    hdri_preview_tex_needs_update: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ShowImage {
    SelectedImage,
    HDRI,
}

impl epi::App for AppMain {
    fn name(&self) -> &str {
        "HDRI Merge"
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
        // Update the HDRI preview texture if needed.
        if self.ui_data.lock().hdri_preview_tex_needs_update {
            let tex_info = self
                .hdri_preview
                .lock()
                .as_ref()
                .map(|(pixels, width, height)| {
                    (
                        frame
                            .tex_allocator()
                            .alloc_srgba_premultiplied((*width, *height), &pixels),
                        *width,
                        *height,
                    )
                });

            if let (Some((tex_id, width, height)), mut ui_data) =
                (tex_info, self.ui_data.lock_mut())
            {
                let old = ui_data.hdri_preview_tex;
                ui_data.hdri_preview_tex = Some((tex_id, width, height));
                if let Some((old_tex_id, _, _)) = old {
                    frame.tex_allocator().free(old_tex_id);
                }

                ui_data.hdri_preview_tex_needs_update = false;
            }
        }

        // Update the image preview texture if needed.
        if self.ui_data.lock().image_preview_tex_needs_update {
            let tex_info = self
                .image_preview
                .lock()
                .as_ref()
                .map(|(pixels, width, height)| {
                    (
                        frame
                            .tex_allocator()
                            .alloc_srgba_premultiplied((*width, *height), &pixels),
                        *width,
                        *height,
                    )
                });

            if let (Some((tex_id, width, height)), mut ui_data) =
                (tex_info, self.ui_data.lock_mut())
            {
                let old = ui_data.image_preview_tex;
                ui_data.image_preview_tex = Some((tex_id, width, height));
                if let Some((old_tex_id, _, _)) = old {
                    frame.tex_allocator().free(old_tex_id);
                }

                ui_data.image_preview_tex_needs_update = false;
            }
        }

        // Some simple queries we use in drawing the UI.
        let image_count = self.ui_data.lock().thumbnails.len();
        let have_hdri = match self.hdri_merger.try_lock() {
            Some(hdri) => hdri.is_some(),
            _ => false,
        };
        let have_hdri_preview_tex = self.ui_data.lock().hdri_preview_tex.is_some();
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
        let save_hdri_dialog = rfd::FileDialog::new()
            .set_title("Save HDRI")
            .add_filter(".hdr", &["hdr", "HDR"]);

        //----------------
        // GUI.

        // Menu bar.
        egui::containers::panel::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui
                        .add_enabled(job_count == 0, egui::widgets::Button::new("Add Images..."))
                        .clicked()
                    {
                        if let Some(paths) = add_images_dialog.clone().pick_files() {
                            self.add_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()));
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
                            self.save_hdri(path);
                        }
                    }

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
                let mut remove_i = None; // Temp to store index of an image to remove.

                // Selected image info.
                // (Extra scope to contain ui_data's mutex guard.)
                {
                    use egui::widgets::Label;
                    let ui_data = self.ui_data.lock();
                    let spacing = 4.0;

                    ui.add_space(spacing + 4.0);
                    if ui_data.selected_image_index < ui_data.thumbnails.len() {
                        let info = &ui_data.thumbnails[ui_data.selected_image_index].2;
                        ui.add(Label::new("Filename:").strong());
                        ui.indent("", |ui| ui.label(format!("{}", info.filename)));

                        ui.add_space(spacing);
                        ui.add(Label::new("Resolution:").strong());
                        ui.indent("", |ui| {
                            ui.label(format!("{} x {}", info.width, info.height))
                        });

                        ui.add_space(spacing);
                        ui.add(Label::new("Log Exposure:").strong());
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
                        let ui_data = &mut *self.ui_data.lock_mut();
                        let thumbnails = &mut ui_data.thumbnails;
                        let selected_image_index = &mut ui_data.selected_image_index;

                        for (img_i, ((pixels, width, height), ref mut tex_id, _)) in
                            thumbnails.iter_mut().enumerate()
                        {
                            let display_height = 64.0;
                            let display_width = display_height / *height as f32 * *width as f32;

                            // Build thumbnail texture if it doesn't already exist.
                            if tex_id.is_none() {
                                *tex_id = Some(make_texture(
                                    (&pixels, *width, *height),
                                    frame.tex_allocator(),
                                ));
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
                                    self.compute_image_preview(img_i);
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
                    self.remove_image(img_i);
                }
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // Image add button.
                if ui
                    .add_enabled(job_count == 0, egui::widgets::Button::new("Add Images..."))
                    .clicked()
                {
                    if let Some(paths) = add_images_dialog.clone().pick_files() {
                        self.add_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()));
                    }
                }

                ui.label(" ➡ ");

                // Build HDRI button.
                if ui
                    .add_enabled(
                        image_count >= 2 && job_count == 0,
                        egui::widgets::Button::new("Build HDRI"),
                    )
                    .clicked()
                {
                    self.build_hdri();
                }

                ui.label(" ➡ ");

                // Save .hdr button.
                if ui
                    .add_enabled(
                        have_hdri && job_count == 0,
                        egui::widgets::Button::new("Save HDRI..."),
                    )
                    .clicked()
                {
                    if let Some(path) = save_hdri_dialog.clone().save_file() {
                        self.save_hdri(path);
                    }
                }
            });

            ui.add(egui::widgets::Separator::default().spacing(12.0));

            ui.horizontal(|ui| {
                let spacing = 16.0;

                ui.vertical(|ui| {
                    let show_image = &mut self.ui_data.lock_mut().show_image;
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
                            egui::widgets::RadioButton::new(
                                *show_image == ShowImage::HDRI,
                                "Show HDRI",
                            ),
                        )
                        .clicked()
                    {
                        *show_image = ShowImage::HDRI;
                    }
                });

                ui.add_space(spacing);

                if self.ui_data.lock().show_image == ShowImage::HDRI {
                    ui.add_space(spacing);
                    if ui
                        .add(
                            egui::widgets::DragValue::new(
                                &mut self.ui_data.lock_mut().preview_exposure,
                            )
                            .speed(0.1)
                            .prefix("Log Exposure: "),
                        )
                        .changed()
                    {
                        self.compute_hdri_preview();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(), |ui| {
                    ui.scope(|ui| {
                        ui.add_space(6.0);
                        ui.spacing_mut().slider_width = 200.0;
                        ui.add_enabled(
                            image_count > 0 || have_hdri,
                            egui::widgets::Slider::new(
                                &mut self.ui_data.lock_mut().image_zoom,
                                0.1..=1.0,
                            )
                            .min_decimals(1)
                            .max_decimals(2)
                            // .prefix("Zoom: ")
                            .suffix("x"),
                        );
                        ui.label("Zoom:");
                    });
                });
            });

            let show_image = self.ui_data.lock().show_image;
            let image_zoom = self.ui_data.lock().image_zoom;
            egui::containers::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if show_image == ShowImage::HDRI && have_hdri_preview_tex {
                        if let Some((tex_id, width, height)) = self.ui_data.lock().hdri_preview_tex
                        {
                            ui.image(
                                tex_id,
                                egui::Vec2::new(
                                    width as f32 * image_zoom,
                                    height as f32 * image_zoom,
                                ),
                            );
                        }
                    } else if show_image == ShowImage::SelectedImage && image_count > 0 {
                        if let Some((tex_id, width, height)) = self.ui_data.lock().image_preview_tex
                        {
                            ui.image(
                                tex_id,
                                egui::Vec2::new(
                                    width as f32 * image_zoom,
                                    height as f32 * image_zoom,
                                ),
                            );
                        }
                    }
                });
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
        let images = self.images.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;
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
                if !images.lock().is_empty() {
                    let needed_width = images.lock()[0].image.width();
                    let needed_height = images.lock()[0].image.height();
                    if img.image.width() != needed_width || img.image.height() != needed_height {
                        status.lock_mut().log_error(format!(
                            "Image has a different resolution: \"{}\".  Not loading.  Note: all images must have the same resolution.",
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
                ui_data
                    .lock_mut()
                    .thumbnails
                    .push((thumbnail, None, img.info.clone()));
                ui_data
                    .lock_mut()
                    .thumbnails
                    .sort_unstable_by(|a, b| a.2.exposure.partial_cmp(&b.2.exposure).unwrap());
                images.lock_mut().push(img);
                images
                    .lock_mut()
                    .sort_unstable_by(|a, b| a.info.exposure.partial_cmp(&b.info.exposure).unwrap());
            }
        });

        let selected_image_index = self.ui_data.lock().selected_image_index;
        self.compute_image_preview(selected_image_index);
    }

    fn remove_image(&self, image_index: usize) {
        let images = self.images.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Remove Image", move |status| {
            status
                .lock_mut()
                .set_progress(format!("Removing image..."), 0.0);

            {
                images.lock_mut().remove(image_index);
                let mut ui_data = ui_data.lock_mut();
                ui_data.thumbnails.remove(image_index);
                if ui_data.selected_image_index > image_index {
                    ui_data.selected_image_index -= 1;
                }
            }
        });

        let selected_image_index = self.ui_data.lock().selected_image_index;
        self.compute_image_preview(selected_image_index);
    }

    fn build_hdri(&mut self) {
        use sensor_analysis::Histogram;

        let images = self.images.clone_ref();
        let hdri = self.hdri_merger.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Build HDRI", move |status| {
            let img_len = images.lock().len();
            let width = images.lock()[0].image.width() as usize;
            let height = images.lock()[0].image.height() as usize;

            status
                .lock_mut()
                .set_progress(format!("Estimating transfer function"), 0.0);

            // Calculate histograms.
            let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
            for img_i in 0..img_len {
                for chan in 0..3 {
                    if status.lock().is_canceled() {
                        return;
                    }
                    let src_img = &images.lock()[img_i];
                    if let Some(exposure) = src_img.info.exposure {
                        histograms[chan].push((
                            Histogram::from_iter(
                                src_img
                                    .image
                                    .enumerate_pixels()
                                    .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
                                256,
                            ),
                            exposure,
                        ));
                    }
                }
            }

            // Estimate linearizating curve.
            let inv_mapping: [Vec<f32>; 3] = {
                let (mapping, _) = sensor_analysis::estimate_transfer_function(&[
                    &histograms[0],
                    &histograms[1],
                    &histograms[2],
                ]);
                [
                    invert_transfer_function_lut(&mapping[0]),
                    invert_transfer_function_lut(&mapping[1]),
                    invert_transfer_function_lut(&mapping[2]),
                ]
            };

            // Merge images.
            let mut hdri_merger = HDRIMerger::new(width, height);
            for img_i in 0..img_len {
                if status.lock().is_canceled() {
                    return;
                }
                status.lock_mut().set_progress(
                    format!("Merging image {}", img_i + 1),
                    (img_i + 1) as f32 / (img_len + 2) as f32,
                );

                let src_img = &images.lock()[img_i];
                hdri_merger.add_image(
                    &src_img.image,
                    src_img.info.exposure.unwrap_or(1.0),
                    &inv_mapping,
                    img_i == 0,
                    img_i == img_len - 1,
                );
            }

            // Finalize.
            if status.lock().is_canceled() {
                return;
            }
            status.lock_mut().set_progress(
                format!("Finalizing"),
                (img_len + 1) as f32 / (img_len + 2) as f32,
            );
            hdri_merger.finish();

            *hdri.lock_mut() = Some(hdri_merger);
            ui_data.lock_mut().show_image = ShowImage::HDRI;
        });

        self.compute_hdri_preview();
    }

    fn save_hdri(&mut self, path: PathBuf) {
        let hdri = self.hdri_merger.clone_ref();

        self.job_queue.add_job("Save HDRI", move |status| {
            status
                .lock_mut()
                .set_progress(format!("Saving: {}", path.to_string_lossy()), 0.0);
            if let Some(ref hdri) = *hdri.lock() {
                hdr::write_hdr(
                    &mut std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
                    &hdri.pixels,
                    hdri.width,
                    hdri.height,
                )
                .unwrap();
            }
        });
    }

    fn compute_hdri_preview(&mut self) {
        let hdri = self.hdri_merger.clone_ref();
        let hdri_preview = self.hdri_preview.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .cancel_pending_jobs_with_name("Update HDRI preview");
        self.job_queue
            .add_job("Update HDRI preview", move |status| {
                status
                    .lock_mut()
                    .set_progress("Updating HDRI preview".to_string(), 0.0);

                let exposure = 2.0f32.powf(ui_data.lock().preview_exposure);
                let srgb_table: Vec<f32> = (0..256)
                    .map(|n| {
                        sensor_analysis::known_transfer_functions::srgb::from_linear(
                            n as f32 / 255.0,
                        )
                    })
                    .collect();
                let preview: Option<(Vec<egui::Color32>, usize, usize)> =
                    hdri.lock().as_ref().map(|hdri| {
                        let map_val = |n: f32| {
                            (eval_transfer_function_lut(
                                &srgb_table,
                                (n * exposure).max(0.0).min(1.0),
                            ) * 255.0)
                                .round() as u8
                        };

                        (
                            hdri.pixels
                                .par_iter()
                                .map(|[r, g, b]| {
                                    let r = map_val(*r);
                                    let g = map_val(*g);
                                    let b = map_val(*b);
                                    egui::Color32::from_rgba_unmultiplied(r, g, b, 255)
                                })
                                .collect(),
                            hdri.width,
                            hdri.height,
                        )
                    });

                if status.lock().is_canceled() {
                    return;
                }

                if preview.is_some() {
                    *hdri_preview.lock_mut() = preview;
                    ui_data.lock_mut().hdri_preview_tex_needs_update = true;
                }
            });
    }

    fn compute_image_preview(&self, image_index: usize) {
        let images = self.images.clone_ref();
        let image_preview = self.image_preview.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.cancel_jobs_with_name("Update image preview");
        self.job_queue
            .add_job("Update image preview", move |status| {
                status
                    .lock_mut()
                    .set_progress("Updating image preview".to_string(), 0.0);

                let preview = images
                    .lock()
                    .get(image_index)
                    .map(|image| lib::job_helpers::make_image_preview(image, None, None));

                if status.lock().is_canceled() {
                    return;
                }

                if preview.is_some() {
                    *image_preview.lock_mut() = preview;
                    ui_data.lock_mut().image_preview_tex_needs_update = true;
                }
            });
    }
}

#[derive(Debug)]
struct HDRIMerger {
    pixels: Vec<[f32; 3]>, // Vec<[r, g, b]>
    pixel_weights: Vec<f32>,
    width: usize,
    height: usize,
}

impl HDRIMerger {
    fn new(width: usize, height: usize) -> HDRIMerger {
        HDRIMerger {
            pixels: vec![[0.0; 3]; width * height],
            pixel_weights: vec![0.0; width * height],
            width: width,
            height: height,
        }
    }

    fn add_image(
        &mut self,
        img: &image::RgbImage,
        exposure: f32,
        linearizing_curves: &[Vec<f32>],
        is_lowest_exposed: bool,
        is_highest_exposed: bool,
    ) {
        debug_assert_eq!(self.width, img.width() as usize);
        debug_assert_eq!(self.height, img.height() as usize);

        let calc_weight = |n: f32| -> f32 {
            // Triangle weight.
            let tri = if (is_lowest_exposed && n > 0.5) || (is_highest_exposed && n < 0.5) {
                // For highest/lowest exposed image, make the appropriate
                // half constant 1.0 instead of sloping down to zero.
                1.0
            } else {
                (0.5 - (n - 0.5).abs()) * 2.0
            };

            // Triangle -> smooth step weight.
            tri * tri * (3.0 - 2.0 * tri)
        };

        let inv_exposure = 1.0 / exposure;
        for (i, pixel) in img.pixels().enumerate() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            let r_linear = eval_transfer_function_lut(&linearizing_curves[0][..], r);
            let g_linear = eval_transfer_function_lut(&linearizing_curves[1][..], g);
            let b_linear = eval_transfer_function_lut(&linearizing_curves[2][..], b);
            let weight =
                calc_weight(r.max(g).max(b)) + calc_weight(r_linear.max(g_linear).max(b_linear));

            self.pixels[i][0] += r_linear * inv_exposure * weight;
            self.pixels[i][1] += g_linear * inv_exposure * weight;
            self.pixels[i][2] += b_linear * inv_exposure * weight;
            self.pixel_weights[i] += weight;
        }
    }

    fn finish(&mut self) {
        for (pixel, weight) in self.pixels.iter_mut().zip(self.pixel_weights.iter()) {
            if *weight > 0.0 {
                pixel[0] /= weight;
                pixel[1] /= weight;
                pixel[2] /= weight;
            }
        }
    }
}

fn make_texture(
    img: (&[egui::Color32], usize, usize),
    tex_allocator: &mut dyn epi::TextureAllocator,
) -> egui::TextureId {
    assert_eq!(img.0.len(), img.1 * img.2);
    tex_allocator.alloc_srgba_premultiplied((img.1, img.2), img.0)
}
