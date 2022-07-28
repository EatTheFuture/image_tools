use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use job_queue::JobQueue;
use sensor_analysis::Histogram;
use shared_data::Shared;

use lib::ImageInfo;

use crate::egui::{self, Context, TextureFilter, Ui};

pub struct ImageList {
    pub histogram_sets: Shared<Vec<Vec<([Histogram; 3], ImageInfo)>>>,
    ui_data: Shared<UiData>,
    multiple_sets: AtomicBool,
}

struct UiData {
    thumbnail_sets: Vec<Vec<(egui::TextureHandle, usize, usize, ImageInfo)>>, // (tex_handle, width, height, ImageInfo)
    selected_idx: (usize, usize), // (set index, image index)
}

impl ImageList {
    pub fn new(multiple_sets: bool) -> ImageList {
        ImageList {
            histogram_sets: Shared::new(Vec::new()),
            ui_data: Shared::new(UiData {
                thumbnail_sets: Vec::new(),
                selected_idx: (0, 0),
            }),
            multiple_sets: AtomicBool::new(multiple_sets),
        }
    }

    pub fn total_image_count(&self) -> usize {
        self.ui_data
            .lock()
            .thumbnail_sets
            .iter()
            .map(|s| s.len())
            .sum()
    }

    // Returns whether any data was changed or not.
    pub fn draw(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        jq: &JobQueue,
        enable_changes: bool,
        working_dir: &mut PathBuf,
    ) -> bool {
        let mut was_changed = false;
        let use_sets = self.uses_sets();

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

        // Image set add button.
        if ui
            .add_enabled(
                enable_changes,
                if use_sets {
                    egui::widgets::Button::new("Add Image Set...")
                } else {
                    egui::widgets::Button::new("Add Image...")
                },
            )
            .clicked()
        {
            if let Some(paths) = add_images_dialog.clone().pick_files() {
                self.add_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()), ctx, jq);
                if let Some(parent) = paths.get(0).map(|p| p.parent().map(|p| p.into())).flatten() {
                    *working_dir = parent;
                }
                was_changed = true;
            }
        }

        // Image thumbnails.
        let mut remove_i = (None, None); // (set index, image index)
        egui::containers::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let ui_data = &mut *self.ui_data.lock_mut();
                let thumbnail_sets = &ui_data.thumbnail_sets;
                let (ref mut set_index, ref mut image_index) = &mut ui_data.selected_idx;

                for set_i in 0..thumbnail_sets.len() {
                    ui.add_space(16.0);
                    if use_sets {
                        ui.horizontal(|ui| {
                            ui.label(format!("Image Set {}", set_i + 1));
                            if ui
                                .add_enabled(enable_changes, egui::widgets::Button::new("🗙"))
                                .clicked()
                            {
                                remove_i = (Some(set_i), None);
                            }
                        });
                        ui.add_space(4.0);
                    }
                    let set = &thumbnail_sets[set_i];
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
                                .add_enabled(enable_changes, egui::widgets::Button::new("🗙"))
                                .clicked()
                            {
                                remove_i = (Some(set_i), Some(img_i));
                            }
                        });
                    }
                }
            });
        match remove_i {
            (Some(set_i), Some(img_i)) => {
                self.remove_image(set_i, img_i);
                was_changed = true;
            }

            (Some(set_i), None) => {
                self.remove_image_set(set_i);
                was_changed = true;
            }
            _ => {}
        }

        was_changed
    }

    fn uses_sets(&self) -> bool {
        use std::sync::atomic::Ordering;
        self.multiple_sets.load(Ordering::Acquire)
    }

    pub fn add_image_files<'a, I: Iterator<Item = &'a Path>>(
        &mut self,
        paths: I,
        ctx: &egui::Context,
        job_queue: &JobQueue,
    ) {
        let use_sets = self.uses_sets();

        let mut image_paths: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let histogram_sets = self.histogram_sets.clone_ref();
        let ui_data = self.ui_data.clone_ref();
        let ctx = ctx.clone();

        job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;

            // Create a new image and thumbnail set.
            if use_sets || histogram_sets.lock().is_empty() {
                histogram_sets.lock_mut().push(Vec::new());
                ui_data.lock_mut().thumbnail_sets.push(Vec::new());
            }

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
                if !histogram_sets.lock().last().unwrap().is_empty() {
                    let needed_width = histogram_sets.lock().last().unwrap()[0].1.width as u32;
                    let needed_height = histogram_sets.lock().last().unwrap()[0].1.height as u32;
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
                        "Image file lacks Exif data needed to compute exposure value: \"{}\".  Transfer function estimation will not work correctly.",
                        path.to_string_lossy()
                    ));
                }

                // Make a thumbnail texture.
                let (thumbnail_tex_handle, thumbnail_width, thumbnail_height) = {
                    let (pixels, width, height) = lib::job_helpers::make_image_preview(&img, Some(128), None);
                    let tex_handle = ctx.load_texture("",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [width, height],
                                &pixels,
                            ),
                            TextureFilter::Linear,
                        );
                    (tex_handle, width, height)
                };

                // Compute histograms.
                let histograms = lib::job_helpers::compute_image_histograms(&img);

                // Add image and thumbnail to our lists.
                {
                    let mut ui_data = ui_data.lock_mut();
                    let set = ui_data.thumbnail_sets.last_mut().unwrap();
                    set.push((thumbnail_tex_handle, thumbnail_width, thumbnail_height, img.info.clone()));
                    set.sort_unstable_by(|a, b| a.3.exposure.partial_cmp(&b.3.exposure).unwrap());
                }
                {
                    let mut histogram_sets = histogram_sets.lock_mut();
                    let set = histogram_sets.last_mut().unwrap();
                    set.push((histograms, img.info.clone()));
                    set.sort_unstable_by(|a, b| a.1.exposure.partial_cmp(&b.1.exposure).unwrap());
                }
            }
        });

        // // Update the exposure mappings.
        // self.compute_exposure_mappings();
    }

    fn remove_image(&mut self, set_index: usize, image_index: usize) {
        if set_index >= self.histogram_sets.lock().len() {
            return;
        }
        let image_count = self.histogram_sets.lock()[set_index].len();
        if image_index >= image_count {
            return;
        }

        // If there won't be any images after this, just remove the
        // whole set.
        if image_count <= 1 {
            self.remove_image_set(set_index);
            return;
        }

        // Remove the image.
        self.histogram_sets.lock_mut()[set_index].remove(image_index);

        // Remove the thumbnail.
        let mut ui_data = self.ui_data.lock_mut();
        let thumbnail_sets = &mut ui_data.thumbnail_sets;
        if set_index < thumbnail_sets.len() && image_index < thumbnail_sets[set_index].len() {
            let _ = thumbnail_sets[set_index].remove(image_index);
        }

        // Adjust the selected image index appropriately.
        if ui_data.selected_idx.0 == set_index && ui_data.selected_idx.1 > image_index {
            ui_data.selected_idx.1 -= 1;
        }

        // // Update the exposure mappings.
        // self.compute_exposure_mappings();
    }

    fn remove_image_set(&mut self, set_index: usize) {
        {
            // Remove the image set.
            let mut image_sets = self.histogram_sets.lock_mut();
            if set_index < image_sets.len() {
                image_sets.remove(set_index);
            }
        }
        {
            // Remove the thumbnail set.
            let mut ui_data = self.ui_data.lock_mut();
            let thumbnail_sets = &mut ui_data.thumbnail_sets;
            if set_index < thumbnail_sets.len() {
                thumbnail_sets.remove(set_index);
            }

            // Adjust the selected image index appropriately.
            if set_index > thumbnail_sets.len() {
                let new_set_index = thumbnail_sets.len().saturating_sub(1);
                let new_image_index = thumbnail_sets
                    .get(new_set_index)
                    .map(|s| s.len().saturating_sub(1))
                    .unwrap_or(0);
                ui_data.selected_idx = (new_set_index, new_image_index);
            } else if set_index == ui_data.selected_idx.0 {
                ui_data.selected_idx.1 = 0;
            } else if set_index < ui_data.selected_idx.0 {
                ui_data.selected_idx.0 -= 1;
            }
        }

        // // Update the exposure mappings.
        // self.compute_exposure_mappings();
    }
}
