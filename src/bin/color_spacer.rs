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
        // let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;

            // Create a new image set.
            image_sets.lock_mut().push(Vec::new());

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

                // // Make a thumbnail texture.
                // let thumbnail = lib::job_helpers::make_image_preview(
                //     &img,
                //     Some(128),
                //     None,
                // );

                // Add image and thumbnail to our lists.
                // ui_data
                //     .lock_mut()
                //     .thumbnails
                //     .push((thumbnail, None, img.info.clone()));
                // ui_data
                //     .lock_mut()
                //     .thumbnails
                //     .sort_unstable_by(|a, b| a.2.exposure.partial_cmp(&b.2.exposure).unwrap());
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
}
