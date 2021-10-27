use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::{App, Arg};
use eframe::{egui, epi};

use sensor_analysis::{eval_luma_map, invert_luma_map};

fn main() {
    let matches = App::new("HDRI Merge")
        .version("1.0")
        .author("Nathan Vegdahl")
        .about("Merges LDR images into an HDRI")
        // .arg(
        //     Arg::with_name("INPUT")
        //         .help("input image files")
        //         .required(true)
        //         .multiple(true)
        //         .index(1),
        // )
        .get_matches();

    // let filenames: Vec<_> = matches.values_of("INPUT").unwrap().collect();

    eframe::run_native(
        Box::new(HDRIMergeApp {
            job_queue: job_queue::JobQueue::new(),

            images: Arc::new(Mutex::new(Vec::new())),
            hdri: Arc::new(Mutex::new(None)),
            ui_data: Arc::new(Mutex::new(UIData {
                thumbnails: Vec::new(),
            })),

            log_read: 0,
        }),
        eframe::NativeOptions {
            drag_and_drop_support: true, // Enable drag-and-dropping files on Windows.
            ..eframe::NativeOptions::default()
        },
    );
}

type SharedData<T> = Arc<Mutex<T>>;

struct HDRIMergeApp {
    job_queue: job_queue::JobQueue,

    images: SharedData<Vec<SourceImage>>,
    hdri: SharedData<Option<HDRIMerger>>,
    ui_data: SharedData<UIData>,

    log_read: usize,
}

/// The data that the UI needs realtime access to for responsiveness.
struct UIData {
    thumbnails: Vec<(image::RgbImage, Option<egui::TextureId>, f32)>,
}

impl epi::App for HDRIMergeApp {
    fn name(&self) -> &str {
        "HDRI Merger"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
        // Don't need to do anything.
    }

    // Called before shutdown.
    fn save(&mut self, _storage: &mut dyn epi::Storage) {
        // Don't need to do anything.
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        //----------------
        // GUI.

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            // Image add button.
            if ui.add(egui::widgets::Button::new("Add Image(s)")).clicked() {
                if let Some(paths) = rfd::FileDialog::new().pick_files() {
                    self.add_image_files(
                        Arc::clone(&frame.repaint_signal()),
                        paths.iter().map(|pathbuf| pathbuf.as_path()),
                    );
                }
            }

            // Build HDRI button.
            if self.ui_data.lock().unwrap().thumbnails.len() >= 2 {
                if ui.add(egui::widgets::Button::new("Build HDRI")).clicked() {
                    self.build_hdri(Arc::clone(&frame.repaint_signal()));
                }
            }

            // Save .hdr button.
            match Arc::clone(&self.hdri).try_lock() {
                Ok(hdri) if hdri.is_some() => {
                    if ui.add(egui::widgets::Button::new("Save .hdr")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            self.save_hdri(Arc::clone(&frame.repaint_signal()), path);
                        }
                    }
                }
                _ => {}
            }

            // Image thumbnails.
            egui::containers::ScrollArea::vertical().show(ui, |ui| {
                for (thumbnail, ref mut tex_id, _) in
                    self.ui_data.lock().unwrap().thumbnails.iter_mut()
                {
                    let height = 64.0;
                    let width = height / thumbnail.height() as f32 * thumbnail.width() as f32;

                    // Build thumbnail texture if it doesn't already exist.
                    if tex_id.is_none() {
                        assert_eq!(
                            thumbnail.width() as usize * thumbnail.height() as usize * 3,
                            thumbnail.as_raw().len()
                        );
                        let pixels: Vec<_> = thumbnail
                            .as_raw()
                            .chunks_exact(3)
                            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], 255))
                            .collect();

                        *tex_id = Some(frame.tex_allocator().alloc_srgba_premultiplied(
                            (thumbnail.width() as usize, thumbnail.height() as usize),
                            &pixels,
                        ));
                    }

                    ui.image(tex_id.unwrap(), egui::Vec2::new(width, height));
                }
            });
        });

        // Status bar.
        egui::containers::panel::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            // Draw progress bar for any in-progress jobs.
            if let Some((text, ratio)) = self.job_queue.progress() {
                ui.add(egui::widgets::ProgressBar::new(ratio).text(text));
            }
            // Draw error message if there are any errors.
            else if (self.job_queue.log_count() - self.log_read) > 0 {
                let unread_count = (self.job_queue.log_count() - self.log_read);
                for i in 0..unread_count {
                    let log_i = (unread_count - 1) - i;
                    let (message, level) = self.job_queue.get_log(log_i);
                    ui.add(match level {
                        job_queue::LogLevel::Error => {
                            egui::widgets::Label::new(format!("ERROR: {}", message))
                                .strong()
                                .background_color(egui::Rgba::from_rgb(0.5, 0.05, 0.05))
                        }
                        job_queue::LogLevel::Warning => {
                            egui::widgets::Label::new(format!("Warning: {}", message)).strong()
                        }
                        job_queue::LogLevel::Note => {
                            egui::widgets::Label::new(format!("{}", message))
                        }
                    });
                }
            }
        });

        //----------------
        // Processing.

        // Collect dropped files.
        if !ctx.input().raw.dropped_files.is_empty() {
            self.add_image_files(
                Arc::clone(&frame.repaint_signal()),
                ctx.input()
                    .raw
                    .dropped_files
                    .iter()
                    .map(|dropped_file| dropped_file.path.as_ref().unwrap().as_path()),
            );
        }
    }
}

impl HDRIMergeApp {
    fn add_image_files<'a, I: Iterator<Item = &'a Path>>(
        &mut self,
        repaint_signal: Arc<dyn epi::RepaintSignal>,
        paths: I,
    ) {
        let mut image_paths: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let images = Arc::clone(&self.images);
        let ui_data = Arc::clone(&self.ui_data);
        let repaint_signal = std::panic::AssertUnwindSafe(repaint_signal);

        self.log_read = self.job_queue.log_count();
        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;
            for (img_i, path) in image_paths.drain(..).enumerate() {
                if status.lock().unwrap().is_canceled() {
                    break;
                }

                status.lock().unwrap().set_progress(
                    format!("Loading: {}", path.to_string_lossy()),
                    (img_i + 1) as f32 / len,
                );
                repaint_signal.request_repaint();

                // Load image.
                let img = if let Ok(f) = image::io::Reader::open(&path) {
                    if let Some(Some(img)) = f
                        .with_guessed_format()
                        .ok()
                        .map(|f| f.decode().ok().map(|f| f.to_rgb8()))
                    {
                        img
                    } else {
                        status.lock().unwrap().log_error(format!(
                            "Unrecognized image file format: \"{}\".",
                            path.to_string_lossy()
                        ));
                        repaint_signal.request_repaint();
                        return;
                    }
                } else {
                    status.lock().unwrap().log_error(format!(
                        "Unable to access file \"{}\".",
                        path.to_string_lossy()
                    ));
                    repaint_signal.request_repaint();
                    return;
                };

                // Ensure it has the same resolution as the other images.
                if !images.lock().unwrap().is_empty() {
                    let needed_width = images.lock().unwrap()[0].image.width();
                    let needed_height = images.lock().unwrap()[0].image.height();
                    if img.width() != needed_width || img.height() != needed_height {
                        status.lock().unwrap().log_error(format!(
                            "Image has a different resolution: \"{}\".  Not loading.  Note: all images must have the same resolution.",
                            path.to_string_lossy()
                        ));
                        repaint_signal.request_repaint();
                        continue;
                    }
                }

                // Get exposure metadata from EXIF data.
                let (exposure_time, fstop, sensitivity) = {
                    let mut exposure_time = None;
                    let mut fstop = None;
                    let mut sensitivity = None;

                    let mut file = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
                    if let Ok(img_exif) = exif::Reader::new().read_from_container(&mut file) {
                        if let Some(&exif::Value::Rational(ref n)) = img_exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY).map(|n| &n.value) {
                            exposure_time = Some(n[0]);
                        }
                        if let Some(&exif::Value::Rational(ref n)) = img_exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY).map(|n| &n.value) {
                            fstop = Some(n[0]);
                        }
                        if let Some(Some(n)) = img_exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY).map(|n| n.value.get_uint(0)) {
                            sensitivity = Some(n);
                        }
                    }

                    (exposure_time, fstop, sensitivity)
                };

                // Calculate over-all exposure.
                let total_exposure = if let (Some(exp), Some(fst), Some(sns)) = (exposure_time, fstop, sensitivity) {
                    sns as f64 * exp.to_f64() / (fst.to_f64() * fst.to_f64())
                } else {
                    status.lock().unwrap().log_warning(format!(
                        "Image file lacks Exif data needed to compute exposure value: \"{}\".  Defaulting to 1.0.",
                        path.to_string_lossy()
                    ));
                    repaint_signal.request_repaint();
                    1.0
                };

                // Make a thumbnail texture.
                let thumbnail = {
                    let height = 128;
                    let width = height * img.width() / img.height();
                    let thumbnail = image::imageops::resize(
                        &img,
                        width,
                        height,
                        image::imageops::FilterType::Triangle,
                    );
                    thumbnail
                };

                // Add image to our list of source images.
                images.lock().unwrap().push(SourceImage {
                    image: img,
                    exposure: total_exposure as f32,

                    meta_exposure_time: exposure_time.map(|n| (n.num, n.denom)),
                    meta_fstop: fstop.map(|n| (n.num, n.denom)),
                    meta_iso: sensitivity,
                });
                ui_data
                    .lock()
                    .unwrap()
                    .thumbnails
                    .push((thumbnail, None, total_exposure as f32));
                images
                    .lock()
                    .unwrap()
                    .sort_unstable_by(|a, b| a.exposure.partial_cmp(&b.exposure).unwrap());
                ui_data
                    .lock()
                    .unwrap()
                    .thumbnails
                    .sort_unstable_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
            }
            repaint_signal.request_repaint();
        });
    }

    fn build_hdri(&mut self, repaint_signal: Arc<dyn epi::RepaintSignal>) {
        use sensor_analysis::Histogram;

        let images = Arc::clone(&self.images);
        let hdri = Arc::clone(&self.hdri);
        let repaint_signal = std::panic::AssertUnwindSafe(repaint_signal);

        self.log_read = self.job_queue.log_count();
        self.job_queue.add_job("Build HDRI", move |status| {
            let img_len = images.lock().unwrap().len();
            let width = images.lock().unwrap()[0].image.width() as usize;
            let height = images.lock().unwrap()[0].image.height() as usize;

            status
                .lock()
                .unwrap()
                .set_progress(format!("Estimating linearization curve"), 0.0);
            repaint_signal.request_repaint();

            // Calculate histograms.
            let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
            for img_i in 0..img_len {
                for chan in 0..3 {
                    if status.lock().unwrap().is_canceled() {
                        return;
                    }
                    let src_img = &images.lock().unwrap()[img_i];
                    histograms[chan].push((
                        Histogram::from_iter(
                            src_img
                                .image
                                .enumerate_pixels()
                                .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
                            256,
                        ),
                        src_img.exposure,
                    ));
                }
            }

            // Estimate linearizating curve.
            let inv_mapping: [Vec<f32>; 3] = {
                let (mapping, _) = sensor_analysis::estimate_luma_map_emor(&[
                    &histograms[0],
                    &histograms[1],
                    &histograms[2],
                ]);
                [
                    invert_luma_map(&mapping[0]),
                    invert_luma_map(&mapping[1]),
                    invert_luma_map(&mapping[2]),
                ]
            };

            // Merge images.
            let mut hdri_merger = HDRIMerger::new(width, height);
            for img_i in 0..img_len {
                if status.lock().unwrap().is_canceled() {
                    return;
                }
                status.lock().unwrap().set_progress(
                    format!("Merging image {}", img_i + 1),
                    (img_i + 1) as f32 / (img_len + 2) as f32,
                );
                repaint_signal.request_repaint();

                let src_img = &images.lock().unwrap()[img_i];
                hdri_merger.add_image(
                    &src_img.image,
                    src_img.exposure,
                    &inv_mapping,
                    img_i == 0,
                    img_i == img_len - 1,
                );
            }

            // Finalize.
            if status.lock().unwrap().is_canceled() {
                return;
            }
            status.lock().unwrap().set_progress(
                format!("Finalizing"),
                (img_len + 1) as f32 / (img_len + 2) as f32,
            );
            repaint_signal.request_repaint();
            hdri_merger.finish();

            *hdri.lock().unwrap() = Some(hdri_merger);
            repaint_signal.request_repaint();
        });
    }

    fn save_hdri(&mut self, repaint_signal: Arc<dyn epi::RepaintSignal>, path: PathBuf) {
        let hdri = Arc::clone(&self.hdri);
        let repaint_signal = std::panic::AssertUnwindSafe(repaint_signal);

        self.log_read = self.job_queue.log_count();
        self.job_queue.add_job("Save HDRI", move |status| {
            status
                .lock()
                .unwrap()
                .set_progress(format!("Saving: {}", path.to_string_lossy()), 0.0);
            repaint_signal.request_repaint();
            if let Some(ref hdri) = *hdri.lock().unwrap() {
                hdr::write_hdr(
                    &mut std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
                    &hdri.pixels,
                    hdri.width,
                    hdri.height,
                )
                .unwrap();
            }
            repaint_signal.request_repaint();
        });
    }
}

#[derive(Debug)]
struct SourceImage {
    image: image::RgbImage,
    exposure: f32,

    meta_exposure_time: Option<(u32, u32)>, // Ratio.
    meta_fstop: Option<(u32, u32)>,         // Ratio.
    meta_iso: Option<u32>,
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
            let r_linear = eval_luma_map(&linearizing_curves[0][..], r);
            let g_linear = eval_luma_map(&linearizing_curves[1][..], g);
            let b_linear = eval_luma_map(&linearizing_curves[2][..], b);
            let weight = calc_weight(r_linear.max(g_linear).max(b_linear));

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
