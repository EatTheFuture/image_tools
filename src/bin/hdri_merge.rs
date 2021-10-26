use std::path::{Path, PathBuf};

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
            images: Vec::new(),
            job: AppJob::None,
            sensor_inv_mapping: None,
            hdri: None,
        }),
        {
            let mut options = eframe::NativeOptions::default();
            options.drag_and_drop_support = true; // Enable drag-and-dropping files on Windows.
            options
        },
    );
}

#[derive(Debug)]
struct HDRIMergeApp {
    images: Vec<SourceImage>,
    job: AppJob,

    sensor_inv_mapping: Option<[Vec<f32>; 3]>,
    hdri: Option<HDRIMerger>,
}

#[derive(Debug)]
enum AppJob {
    None,
    LoadImages {
        image_list: Vec<std::path::PathBuf>,
        total: usize,
    },
    BuildHDRI(HDRIBuildStage),
    SaveHDRI(PathBuf),
}

#[derive(Debug, Copy, Clone)]
enum HDRIBuildStage {
    EstimateLinearization,
    AddImage(usize),
    Finalize,
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
                    self.add_image_files(paths.iter().map(|pathbuf| pathbuf.as_path()));
                }
            }

            // Build HDRI button.
            if self.images.len() >= 2 {
                if ui.add(egui::widgets::Button::new("Build HDRI")).clicked() {
                    match self.job {
                        AppJob::None => {
                            self.job = AppJob::BuildHDRI(HDRIBuildStage::EstimateLinearization);
                        }
                        _ => {}
                    }
                }
            }

            // Save .hdr button.
            if let Some(_) = self.hdri {
                if let AppJob::None = self.job {
                    if ui.add(egui::widgets::Button::new("Save .hdr")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            self.job = AppJob::SaveHDRI(path);
                        }
                    }
                }
            }

            // Image thumbnails.
            egui::containers::ScrollArea::vertical().show(ui, |ui| {
                for src_img in self.images.iter() {
                    let height = 64.0;
                    let width =
                        height / src_img.image.height() as f32 * src_img.image.width() as f32;
                    ui.image(src_img.thumbnail_tex_id, egui::Vec2::new(width, height));
                }
            });
        });

        // Status bar.
        egui::containers::panel::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            // Draw progress bar for any in-progress jobs.
            match self.job {
                AppJob::LoadImages {
                    ref image_list,
                    total,
                } => {
                    ui.add(
                        egui::widgets::ProgressBar::new(
                            1.0 - (image_list.len() as f32 / total as f32),
                        )
                        .text(format!("Loading: {}", image_list[0].to_string_lossy())),
                    );
                }

                AppJob::BuildHDRI(stage) => {
                    let total = 2 + self.images.len();
                    match stage {
                        HDRIBuildStage::EstimateLinearization => {
                            ui.add(
                                egui::widgets::ProgressBar::new(0.0)
                                    .text("Estimating color linearization"),
                            );
                        }
                        HDRIBuildStage::AddImage(img_i) => {
                            ui.add(
                                egui::widgets::ProgressBar::new((img_i + 1) as f32 / total as f32)
                                    .text(format!("Merging image {}", img_i + 1)),
                            );
                        }
                        HDRIBuildStage::Finalize => {
                            ui.add(
                                egui::widgets::ProgressBar::new((total - 1) as f32 / total as f32)
                                    .text("Finalizing"),
                            );
                        }
                    }
                }

                AppJob::SaveHDRI(ref path) => {
                    ui.add(
                        egui::widgets::ProgressBar::new(0.0)
                            .text(format!("Saving: {}", path.to_string_lossy())),
                    );
                }

                AppJob::None => {}
            }
        });

        //----------------
        // Processing.

        match self.job {
            // Load pending images.
            AppJob::LoadImages {
                ref mut image_list, ..
            } => {
                if !image_list.is_empty() {
                    let path = image_list.remove(0);

                    // Load image.
                    let img = image::io::Reader::open(&path)
                        .unwrap()
                        .with_guessed_format()
                        .unwrap()
                        .decode()
                        .unwrap()
                        .to_rgb8();

                    // Get exposure metadata from EXIF data.
                    let img_exif = {
                        let mut file = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
                        exif::Reader::new().read_from_container(&mut file).unwrap()
                    };
                    let exposure_time =
                        match img_exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
                            Some(n) => match n.value {
                                exif::Value::Rational(ref v) => v[0],
                                _ => panic!(),
                            },
                            None => panic!(),
                        };
                    let fstop = match img_exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
                        Some(n) => match n.value {
                            exif::Value::Rational(ref v) => v[0],
                            _ => panic!(),
                        },
                        None => panic!(),
                    };
                    let sensitivity = img_exif
                        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
                        .unwrap()
                        .value
                        .get_uint(0)
                        .unwrap();

                    // Calculate over-all exposure.
                    let total_exposure = sensitivity as f64 * exposure_time.to_f64()
                        / (fstop.to_f64() * fstop.to_f64());

                    // Make a thumbnail texture.
                    let thumbnail_tex_id = {
                        let height = 128;
                        let width = height * img.width() / img.height();
                        let thumbnail = image::imageops::resize(
                            &img,
                            width,
                            height,
                            image::imageops::FilterType::Triangle,
                        );

                        assert_eq!(
                            thumbnail.width() as usize * thumbnail.height() as usize * 3,
                            thumbnail.as_raw().len()
                        );
                        let pixels: Vec<_> = thumbnail
                            .as_raw()
                            .chunks_exact(3)
                            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], 255))
                            .collect();

                        // Allocate the texture.
                        frame.tex_allocator().alloc_srgba_premultiplied(
                            (thumbnail.width() as usize, thumbnail.height() as usize),
                            &pixels,
                        )
                    };

                    // Add image to our list of source images.
                    self.images.push(SourceImage {
                        image: img,
                        exposure: total_exposure as f32,

                        thumbnail_tex_id: thumbnail_tex_id,

                        meta_exposure_time: Some((exposure_time.num, exposure_time.denom)),
                        meta_fstop: Some((fstop.num, fstop.denom)),
                        meta_iso: Some(sensitivity),
                    });
                    self.images
                        .sort_unstable_by(|a, b| a.exposure.partial_cmp(&b.exposure).unwrap());

                    // Clear job if done loading images.
                    if image_list.is_empty() {
                        self.job = AppJob::None;
                    }

                    // Request repaint, to display the thumbnail.
                    ctx.request_repaint();
                }
            }

            // Build HDRI.
            AppJob::BuildHDRI(build_stage) => {
                match build_stage {
                    HDRIBuildStage::EstimateLinearization => {
                        // Estimate sensor response curve from the image-exposure pairs.
                        let (sensor_mapping, _) = estimate_luma_map(&self.images);
                        let mut inv_mapping: Vec<_> =
                            sensor_mapping.iter().map(|m| invert_luma_map(&m)).collect();
                        let mut tmp = [vec![], vec![], vec![]];
                        tmp[0] = inv_mapping.remove(0);
                        tmp[1] = inv_mapping.remove(0);
                        tmp[2] = inv_mapping.remove(0);
                        self.sensor_inv_mapping = Some(tmp);
                        self.job = AppJob::BuildHDRI(HDRIBuildStage::AddImage(0));
                    }
                    HDRIBuildStage::AddImage(img_i) => {
                        // Create the HDRI.
                        if img_i == 0 {
                            let width = self.images[0].image.width() as usize;
                            let height = self.images[0].image.height() as usize;
                            self.hdri = Some(HDRIMerger::new(width, height));
                        }
                        if let (Some(ref mut hdri), Some(ref inv_mapping)) =
                            (&mut self.hdri, &self.sensor_inv_mapping)
                        {
                            hdri.add_image(
                                &self.images[img_i].image,
                                self.images[img_i].exposure,
                                inv_mapping,
                                img_i == 0,
                                img_i == self.images.len() - 1,
                            );
                        }
                        if img_i == self.images.len() - 1 {
                            self.job = AppJob::BuildHDRI(HDRIBuildStage::Finalize);
                        } else {
                            self.job = AppJob::BuildHDRI(HDRIBuildStage::AddImage(img_i + 1));
                        }
                    }
                    HDRIBuildStage::Finalize => {
                        if let Some(ref mut hdri) = self.hdri {
                            hdri.finish();
                        }
                        self.job = AppJob::None;
                    }
                }
            }

            AppJob::SaveHDRI(ref path) => {
                if let Some(ref hdri) = self.hdri {
                    hdr::write_hdr(
                        &mut std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
                        &hdri.pixels,
                        hdri.width,
                        hdri.height,
                    )
                    .unwrap();
                }
                self.job = AppJob::None;
            }

            AppJob::None => {}
        }

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

        // Request a repaint if we still have pending work to do.
        match self.job {
            AppJob::None => {}
            _ => ctx.request_repaint(),
        };
    }
}

impl HDRIMergeApp {
    fn add_image_files<'a, I: Iterator<Item = &'a Path>>(&mut self, paths: I) {
        let mut images: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let len = images.len();
        if let AppJob::None = self.job {
            // Start a new job.
            self.job = AppJob::LoadImages {
                image_list: images,
                total: len,
            };
        } else if let AppJob::LoadImages {
            ref mut image_list,
            ref mut total,
        } = self.job
        {
            // Add to the existing job.
            image_list.extend(images.drain(..));
            *total += len;
        } else {
            // We're in the middle of another job, so ignore.
        }
    }
}

/// Estimates a linearizing luma map for the given set of source images..
///
/// Returns the luminance map and the average fitting error.
fn estimate_luma_map(images: &[SourceImage]) -> (Vec<Vec<f32>>, f32) {
    use sensor_analysis::{estimate_luma_map_emor, Histogram};

    assert!(images.len() > 1);

    let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
    for i in 0..images.len() {
        for chan in 0..3 {
            histograms[chan].push((
                Histogram::from_iter(
                    images[i]
                        .image
                        .enumerate_pixels()
                        .map(|p: (u32, u32, &image::Rgb<u8>)| p.2[chan]),
                    256,
                ),
                images[i].exposure,
            ));
        }
    }

    estimate_luma_map_emor(&[&histograms[0], &histograms[1], &histograms[2]])
}

#[derive(Debug)]
struct SourceImage {
    image: image::RgbImage,
    exposure: f32,

    thumbnail_tex_id: egui::TextureId,

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
