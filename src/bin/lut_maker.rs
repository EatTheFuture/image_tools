#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui, epi};

use sensor_analysis::Histogram;
use shared_data::Shared;

use lib::ImageInfo;

fn main() {
    clap::App::new("LUT Maker")
        .version("0.1")
        .author("Nathan Vegdahl")
        .about("Does all things color space")
        .get_matches();

    eframe::run_native(
        Box::new(AppMain {
            job_queue: job_queue::JobQueue::new(),
            last_opened_directory: None,

            bracket_image_sets: Shared::new(Vec::new()),
            lens_cap_images: Shared::new(Vec::new()),
            transfer_function_tables: Shared::new(None),

            ui_data: Shared::new(UIData {
                image_view: ImageViewID::Bracketed,
                advanced_mode: false,
                show_from_linear_graph: false,

                selected_bracket_image_index: (0, 0),
                bracket_thumbnail_sets: Vec::new(),

                selected_lens_cap_image_index: 0,
                lens_cap_thumbnails: Vec::new(),

                sensor_floor: [0.0; 3],
                sensor_ceiling: [1.0; 3],

                transfer_function_type: TransferFunction::Estimated,
                transfer_function_resolution: 4096,
                rounds: 2000,
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
    last_opened_directory: Option<PathBuf>,

    bracket_image_sets: Shared<Vec<Vec<([Histogram; 3], ImageInfo)>>>,
    lens_cap_images: Shared<Vec<[Histogram; 3]>>,
    transfer_function_tables: Shared<Option<([Vec<f32>; 3], f32, f32)>>, // (table, x_min, x_max)

    ui_data: Shared<UIData>,
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
struct UIData {
    image_view: ImageViewID,
    advanced_mode: bool,
    show_from_linear_graph: bool,

    selected_bracket_image_index: (usize, usize), // (set index, image index)
    bracket_thumbnail_sets: Vec<
        Vec<(
            (Vec<egui::Color32>, usize, usize),
            Option<egui::TextureId>,
            ImageInfo,
        )>,
    >,

    selected_lens_cap_image_index: usize,
    lens_cap_thumbnails: Vec<(
        (Vec<egui::Color32>, usize, usize),
        Option<egui::TextureId>,
        ImageInfo,
    )>,

    sensor_floor: [f32; 3],
    sensor_ceiling: [f32; 3],

    transfer_function_type: TransferFunction,
    transfer_function_resolution: usize,
    rounds: usize,
    transfer_function_preview: Option<([Vec<(f32, f32)>; 3], f32)>, // (curves, error)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ImageViewID {
    Bracketed,
    LensCap,
}

impl ImageViewID {
    fn ui_text(&self) -> &'static str {
        match *self {
            ImageViewID::Bracketed => "Bracketed Exposures",
            ImageViewID::LensCap => "Lens Cap Images",
        }
    }
}

impl epi::App for AppMain {
    fn name(&self) -> &str {
        "LUT Maker"
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
        let total_bracket_images: usize = self
            .ui_data
            .lock()
            .bracket_thumbnail_sets
            .iter()
            .map(|s| s.len())
            .sum();
        let total_lens_cap_images: usize = self.ui_data.lock().lens_cap_thumbnails.len();

        // File dialogs used in the UI.
        let mut working_dir = self
            .last_opened_directory
            .clone()
            .unwrap_or_else(|| "".into());
        let add_images_dialog = rfd::FileDialog::new()
            .set_title("Add Images")
            .set_directory(&working_dir)
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
        let save_lut_dialog = rfd::FileDialog::new()
            .set_title("Save LUT")
            .set_directory(&working_dir)
            .add_filter(".cube", &["cube", "CUBE"]);

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
                // View selector.
                ui.add_space(4.0);
                {
                    let image_view = &mut self.ui_data.lock_mut().image_view;
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

                let image_view = self.ui_data.lock().image_view;
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
                                self.add_lens_cap_image_files(
                                    paths.iter().map(|pathbuf| pathbuf.as_path()),
                                );
                                if let Some(parent) =
                                    paths.get(0).map(|p| p.parent().map(|p| p.into())).flatten()
                                {
                                    working_dir = parent;
                                }
                            }
                        }

                        // Image thumbnails.
                        let mut remove_i = None;
                        egui::containers::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let ui_data = &mut *self.ui_data.lock_mut();
                                let thumbnails = &mut ui_data.lens_cap_thumbnails;
                                let selected_image_index =
                                    &mut ui_data.selected_lens_cap_image_index;

                                for (img_i, ((pixels, width, height), ref mut tex_id, _)) in
                                    thumbnails.iter_mut().enumerate()
                                {
                                    let display_height = 64.0;
                                    let display_width =
                                        display_height / *height as f32 * *width as f32;

                                    // Build thumbnail texture if it doesn't already exist.
                                    if tex_id.is_none() {
                                        *tex_id =
                                            Some(frame.tex_allocator().alloc_srgba_premultiplied(
                                                (*width, *height),
                                                &pixels,
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
                                        }
                                        if ui
                                            .add_enabled(
                                                job_count == 0,
                                                egui::widgets::Button::new("🗙"),
                                            )
                                            .clicked()
                                        {
                                            remove_i = Some(img_i);
                                        }
                                    });
                                }
                            });
                        if let Some(img_i) = remove_i {
                            self.remove_lens_cap_image(img_i);
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
                                self.add_bracket_image_files(
                                    paths.iter().map(|pathbuf| pathbuf.as_path()),
                                );
                                if let Some(parent) =
                                    paths.get(0).map(|p| p.parent().map(|p| p.into())).flatten()
                                {
                                    working_dir = parent;
                                }
                            }
                        }

                        // Image thumbnails.
                        let mut remove_i = (None, None); // (set index, image index)
                        egui::containers::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let ui_data = &mut *self.ui_data.lock_mut();
                                let bracket_thumbnail_sets = &mut ui_data.bracket_thumbnail_sets;
                                let (ref mut set_index, ref mut image_index) =
                                    &mut ui_data.selected_bracket_image_index;

                                for set_i in 0..bracket_thumbnail_sets.len() {
                                    ui.add_space(16.0);
                                    ui.horizontal(|ui| {
                                        ui.label(format!("Image Set {}", set_i + 1));
                                        if ui
                                            .add_enabled(
                                                job_count == 0,
                                                egui::widgets::Button::new("🗙"),
                                            )
                                            .clicked()
                                        {
                                            remove_i = (Some(set_i), None);
                                        }
                                    });
                                    ui.add_space(4.0);
                                    let set = &mut bracket_thumbnail_sets[set_i];
                                    for (img_i, ((pixels, width, height), ref mut tex_id, _)) in
                                        set.iter_mut().enumerate()
                                    {
                                        let display_height = 64.0;
                                        let display_width =
                                            display_height / *height as f32 * *width as f32;

                                        // Build thumbnail texture if it doesn't already exist.
                                        if tex_id.is_none() {
                                            *tex_id = Some(
                                                frame.tex_allocator().alloc_srgba_premultiplied(
                                                    (*width, *height),
                                                    &pixels,
                                                ),
                                            );
                                        }

                                        ui.horizontal(|ui| {
                                            if ui
                                                .add(
                                                    egui::widgets::ImageButton::new(
                                                        tex_id.unwrap(),
                                                        egui::Vec2::new(
                                                            display_width,
                                                            display_height,
                                                        ),
                                                    )
                                                    .selected(
                                                        set_i == *set_index
                                                            && img_i == *image_index,
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                *set_index = set_i;
                                                *image_index = img_i;
                                            }
                                            if ui
                                                .add_enabled(
                                                    job_count == 0,
                                                    egui::widgets::Button::new("🗙"),
                                                )
                                                .clicked()
                                            {
                                                remove_i = (Some(set_i), Some(img_i));
                                            }
                                        });
                                    }
                                }
                            });
                        match remove_i {
                            (Some(set_i), Some(img_i)) => self.remove_bracket_image(set_i, img_i),
                            (Some(set_i), None) => self.remove_bracket_image_set(set_i),
                            _ => {}
                        }
                    }
                }
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                if ui
                    .add_enabled(
                        job_count == 0
                            && (self.transfer_function_tables.lock().is_some()
                                || self.ui_data.lock().transfer_function_type
                                    != TransferFunction::Estimated),
                        egui::widgets::Button::new("Export 'to linear' LUT..."),
                    )
                    .clicked()
                {
                    if let Some(path) = save_lut_dialog.clone().save_file() {
                        self.save_lut(&path, true);
                        if let Some(parent) = path.parent().map(|p| p.into()) {
                            working_dir = parent;
                        }
                    }
                }
                if ui
                    .add_enabled(
                        job_count == 0
                            && (self.transfer_function_tables.lock().is_some()
                                || self.ui_data.lock().transfer_function_type
                                    != TransferFunction::Estimated),
                        egui::widgets::Button::new("Export 'from linear' LUT..."),
                    )
                    .clicked()
                {
                    if let Some(path) = save_lut_dialog.clone().save_file() {
                        self.save_lut(&path, false);
                        if let Some(parent) = path.parent().map(|p| p.into()) {
                            working_dir = parent;
                        }
                    }
                }
            });

            ui.add(egui::widgets::Separator::default().spacing(12.0));

            // Advanced/simple mode switch.
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.ui_data.lock_mut().advanced_mode, false, "Simple");
                ui.radio_value(&mut self.ui_data.lock_mut().advanced_mode, true, "Advanced");
            });
            let advanced_mode = self.ui_data.lock().advanced_mode;

            ui.add_space(4.0);

            // Transfer function controls.
            if !advanced_mode {
                // Simple mode.
                ui.horizontal(|ui| {
                    // Rounds slider.
                    ui.add_enabled(
                        job_count == 0,
                        egui::widgets::DragValue::new(&mut self.ui_data.lock_mut().rounds)
                            .clamp_range(100..=200000)
                            .max_decimals(0)
                            .prefix("Estimation rounds: "),
                    );

                    // Estimate transfer function button.
                    if ui
                        .add_enabled(
                            job_count == 0 && total_bracket_images > 0,
                            egui::widgets::Button::new("Estimate Everything"),
                        )
                        .clicked()
                    {
                        self.estimate_everything();
                    }
                });
            } else {
                let area_width = ui.available_width();
                let sub_area_width = (area_width / 3.0).min(230.0);

                // Advanced mode.
                ui.horizontal(|ui| {
                    // Sensor floor controls.
                    ui.vertical(|ui| {
                        ui.set_width(sub_area_width);

                        ui.horizontal(|ui| {
                            ui.label("Sensor Noise Floor");
                            ui.add_space(4.0);
                            if ui
                                .add_enabled(
                                    job_count == 0
                                        && (total_bracket_images > 0 || total_lens_cap_images > 0),
                                    egui::widgets::Button::new("Estimate"),
                                )
                                .clicked()
                            {
                                self.estimate_sensor_floor();
                            }
                        });
                        ui.add_space(4.0);
                        for (label, value) in ["R: ", "G: ", "B: "]
                            .iter()
                            .zip(self.ui_data.lock_mut().sensor_floor.iter_mut())
                        {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.add_enabled(
                                    job_count == 0,
                                    egui::widgets::Slider::new(value, 0.0..=1.0)
                                        .max_decimals(5)
                                        .min_decimals(5),
                                );
                            });
                        }
                    });

                    ui.add_space(16.0);

                    // Sensor ceiling controls.
                    ui.vertical(|ui| {
                        ui.set_width(sub_area_width);

                        ui.horizontal(|ui| {
                            ui.label("Sensor Ceiling");
                            ui.add_space(4.0);
                            if ui
                                .add_enabled(
                                    job_count == 0 && total_bracket_images > 0,
                                    egui::widgets::Button::new("Estimate"),
                                )
                                .clicked()
                            {
                                self.estimate_sensor_ceiling();
                            }
                        });
                        ui.add_space(4.0);
                        for (label, value) in ["R: ", "G: ", "B: "]
                            .iter()
                            .zip(self.ui_data.lock_mut().sensor_ceiling.iter_mut())
                        {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.add_enabled(
                                    job_count == 0,
                                    egui::widgets::Slider::new(value, 0.0..=1.0)
                                        .max_decimals(5)
                                        .min_decimals(5),
                                );
                            });
                        }
                    });

                    ui.add_space(16.0);

                    // Transfer curve controls.
                    ui.vertical(|ui| {
                        let mut ui_data = self.ui_data.lock_mut();

                        ui.label("Transfer Curve");
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_source("Transfer Function Type")
                                .width(150.0)
                                .selected_text(format!(
                                    "{}",
                                    ui_data.transfer_function_type.ui_text()
                                ))
                                .show_ui(ui, |ui| {
                                    for tf in TRANSFER_FUNCTIONS.iter() {
                                        ui.selectable_value(
                                            &mut ui_data.transfer_function_type,
                                            *tf,
                                            tf.ui_text(),
                                        );
                                    }
                                });
                        });
                        ui.add_space(4.0);

                        if ui_data.transfer_function_type == TransferFunction::Estimated {
                            // Estimated curve.
                            // Rounds slider.
                            ui.add_enabled(
                                job_count == 0,
                                egui::widgets::DragValue::new(&mut ui_data.rounds)
                                    .clamp_range(100..=200000)
                                    .max_decimals(0)
                                    .prefix("Rounds: "),
                            );

                            // Estimate transfer function button.
                            if ui
                                .add_enabled(
                                    job_count == 0 && total_bracket_images > 0,
                                    egui::widgets::Button::new("Estimate"),
                                )
                                .clicked()
                            {
                                self.estimate_transfer_curve();
                            }
                        } else {
                            // Fixed curve.
                            ui.add_enabled(
                                job_count == 0,
                                egui::widgets::DragValue::new(
                                    &mut ui_data.transfer_function_resolution,
                                )
                                .clamp_range(2..=(1 << 16))
                                .max_decimals(0)
                                .prefix("LUT resolution: "),
                            );
                        }
                    });
                });
            }

            ui.add_space(16.0);

            // "To linear" / "From linear" view switch.
            if self.ui_data.lock().transfer_function_type != TransferFunction::Estimated
                || self.ui_data.lock().transfer_function_preview.is_some()
            {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.ui_data.lock_mut().show_from_linear_graph,
                        false,
                        "To Linear",
                    );
                    ui.radio_value(
                        &mut self.ui_data.lock_mut().show_from_linear_graph,
                        true,
                        "From Linear",
                    );
                });
            }

            // Transfer function graph.
            {
                use egui::widgets::plot::{Line, Plot, Value, Values};
                let ui_data = self.ui_data.lock();

                let show_from_linear_graph = ui_data.show_from_linear_graph;
                let floor = ui_data.sensor_floor;
                let ceiling = ui_data.sensor_ceiling;
                let range = [
                    ceiling[0] - floor[0],
                    ceiling[1] - floor[1],
                    ceiling[2] - floor[2],
                ];

                if ui_data.transfer_function_type == TransferFunction::Estimated {
                    // Estimated curve.
                    if let Some((transfer_function_curves, err)) =
                        &ui_data.transfer_function_preview
                    {
                        let mut plot = Plot::new("Transfer Function Graph").data_aspect(1.0).text(
                            egui::widgets::plot::Text::new(
                                egui::widgets::plot::Value { x: 0.5, y: -0.05 },
                                format!("Average error: {}", err),
                            ),
                        );
                        for i in 0..3 {
                            plot = plot.line(Line::new(Values::from_values_iter(
                                transfer_function_curves[i].iter().copied().map(|(x, y)| {
                                    if show_from_linear_graph {
                                        Value::new(x, floor[i] + (y * range[i]))
                                    } else {
                                        Value::new(floor[i] + (y * range[i]), x)
                                    }
                                }),
                            )));
                        }
                        ui.add(plot);
                    }
                } else {
                    // Fixed curve.
                    let res = ui_data.transfer_function_resolution;
                    let res_norm = 1.0 / (res - 1) as f32;
                    let function = ui_data.transfer_function_type;
                    let mut plot = Plot::new("Transfer Function Graph").data_aspect(1.0);
                    for chan in 0..3 {
                        if show_from_linear_graph {
                            plot =
                                plot.line(Line::new(Values::from_values_iter((0..res).map(|i| {
                                    let a = function.to_linear(0.0, floor[chan], ceiling[chan]);
                                    let b = function.to_linear(1.0, floor[chan], ceiling[chan]);
                                    let x = a + (i as f32 * res_norm * (b - a));
                                    Value::new(
                                        x,
                                        function.from_linear(x, floor[chan], ceiling[chan]),
                                    )
                                }))));
                        } else {
                            plot =
                                plot.line(Line::new(Values::from_values_iter((0..res).map(|i| {
                                    let x = i as f32 * res_norm;
                                    Value::new(x, function.to_linear(x, floor[chan], ceiling[chan]))
                                }))));
                        }
                    }
                    ui.add(plot);
                }
            }
        });

        self.last_opened_directory = Some(working_dir);

        //----------------
        // Processing.

        // Collect dropped files.
        if !ctx.input().raw.dropped_files.is_empty() {
            let image_view = self.ui_data.lock().image_view;
            match image_view {
                ImageViewID::Bracketed => self.add_bracket_image_files(
                    ctx.input()
                        .raw
                        .dropped_files
                        .iter()
                        .map(|dropped_file| dropped_file.path.as_ref().unwrap().as_path()),
                ),
                ImageViewID::LensCap => self.add_lens_cap_image_files(
                    ctx.input()
                        .raw
                        .dropped_files
                        .iter()
                        .map(|dropped_file| dropped_file.path.as_ref().unwrap().as_path()),
                ),
            }
        }
    }
}

impl AppMain {
    fn add_bracket_image_files<'a, I: Iterator<Item = &'a Path>>(&mut self, paths: I) {
        let mut image_paths: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let bracket_image_sets = self.bracket_image_sets.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;

            // Create a new image and thumbnail set.
            bracket_image_sets.lock_mut().push(Vec::new());
            ui_data.lock_mut().bracket_thumbnail_sets.push(Vec::new());

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
                if !bracket_image_sets.lock().last().unwrap().is_empty() {
                    let needed_width = bracket_image_sets.lock().last().unwrap()[0].1.width as u32;
                    let needed_height = bracket_image_sets.lock().last().unwrap()[0].1.height as u32;
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
                let thumbnail = lib::job_helpers::make_image_preview(
                    &img,
                    Some(128),
                    None,
                );

                // Compute histograms.
                let histograms = lib::job_helpers::compute_image_histograms(&img, 256);

                // Add image and thumbnail to our lists.
                {
                    let mut ui_data = ui_data.lock_mut();
                    let set = ui_data.bracket_thumbnail_sets.last_mut().unwrap();
                    set.push((thumbnail, None, img.info.clone()));
                    set.sort_unstable_by(|a, b| a.2.exposure.partial_cmp(&b.2.exposure).unwrap());
                }
                {
                    let mut bracket_image_sets = bracket_image_sets.lock_mut();
                    let set = bracket_image_sets.last_mut().unwrap();
                    set.push((histograms, img.info.clone()));
                    set.sort_unstable_by(|a, b| a.1.exposure.partial_cmp(&b.1.exposure).unwrap());
                }
            }
        });
    }

    fn remove_bracket_image(&mut self, set_index: usize, image_index: usize) {
        if set_index >= self.bracket_image_sets.lock().len() {
            return;
        }
        let image_count = self.bracket_image_sets.lock()[set_index].len();
        if image_index >= image_count {
            return;
        }

        // If there won't be any images after this, just remove the
        // whole set.
        if image_count <= 1 {
            self.remove_bracket_image_set(set_index);
            return;
        }

        // Remove the image.
        self.bracket_image_sets.lock_mut()[set_index].remove(image_index);

        // Remove the thumbnail.
        let mut ui_data = self.ui_data.lock_mut();
        let thumbnail_sets = &mut ui_data.bracket_thumbnail_sets;
        if set_index < thumbnail_sets.len() && image_index < thumbnail_sets[set_index].len() {
            thumbnail_sets[set_index].remove(image_index);
        }

        // Adjust the selected image index appropriately.
        if ui_data.selected_bracket_image_index.0 == set_index
            && ui_data.selected_bracket_image_index.1 > image_index
        {
            ui_data.selected_bracket_image_index.1 -= 1;
        }
    }

    fn remove_bracket_image_set(&mut self, set_index: usize) {
        {
            // Remove the image set.
            let mut image_sets = self.bracket_image_sets.lock_mut();
            if set_index < image_sets.len() {
                image_sets.remove(set_index);
            }
        }
        {
            // Remove the thumbnail set.
            let mut ui_data = self.ui_data.lock_mut();
            let thumbnail_sets = &mut ui_data.bracket_thumbnail_sets;
            if set_index < thumbnail_sets.len() {
                thumbnail_sets.remove(set_index);
            }

            // Adjust the selected image index appropriately.
            if set_index > ui_data.bracket_thumbnail_sets.len() {
                let new_set_index = ui_data.bracket_thumbnail_sets.len().saturating_sub(1);
                let new_image_index = ui_data
                    .bracket_thumbnail_sets
                    .get(new_set_index)
                    .map(|s| s.len().saturating_sub(1))
                    .unwrap_or(0);
                ui_data.selected_bracket_image_index = (new_set_index, new_image_index);
            } else if set_index == ui_data.selected_bracket_image_index.0 {
                ui_data.selected_bracket_image_index.1 = 0;
            } else if set_index < ui_data.selected_bracket_image_index.0 {
                ui_data.selected_bracket_image_index.0 -= 1;
            }
        }
    }

    fn add_lens_cap_image_files<'a, I: Iterator<Item = &'a Path>>(&mut self, paths: I) {
        let mut image_paths: Vec<_> = paths.map(|path| path.to_path_buf()).collect();
        let lens_cap_images = self.lens_cap_images.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Add Image(s)", move |status| {
            let len = image_paths.len() as f32;

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
                    }
                    Err(lib::job_helpers::ImageLoadError::UnknownFormat) => {
                        status.lock_mut().log_error(format!(
                            "Unrecognized image file format: \"{}\".",
                            path.to_string_lossy()
                        ));
                        return;
                    }
                };

                // Make a thumbnail texture.
                let thumbnail = lib::job_helpers::make_image_preview(&img, Some(128), None);

                // Compute histograms.
                let histograms = lib::job_helpers::compute_image_histograms(&img, 256);

                // Add image and thumbnail to our lists.
                ui_data
                    .lock_mut()
                    .lens_cap_thumbnails
                    .push((thumbnail, None, img.info.clone()));
                let mut lens_cap_images = lens_cap_images.lock_mut();
                lens_cap_images.push(histograms);
            }
        });
    }

    fn remove_lens_cap_image(&self, image_index: usize) {
        self.lens_cap_images.lock_mut().remove(image_index);

        let mut ui_data = self.ui_data.lock_mut();
        ui_data.lens_cap_thumbnails.remove(image_index);
        if ui_data.selected_lens_cap_image_index > image_index {
            ui_data.selected_lens_cap_image_index =
                ui_data.selected_lens_cap_image_index.saturating_sub(1);
        }
    }

    fn estimate_sensor_floor(&self) {
        use sensor_analysis::estimate_sensor_floor_ceiling;

        let bracket_image_sets = self.bracket_image_sets.clone_ref();
        let lens_cap_images = self.lens_cap_images.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .add_job("Estimate Sensor Noise Floor", move |status| {
                status
                    .lock_mut()
                    .set_progress(format!("Estimating sensor noise floor"), 0.0);

                if !lens_cap_images.lock().is_empty() {
                    // Collect stats.
                    let mut sum = [0.0f64; 3];
                    let mut sample_count = [0usize; 3];
                    for histograms in lens_cap_images.lock().iter() {
                        for chan in 0..3 {
                            let norm = 1.0 / (histograms[chan].buckets.len() - 1) as f64;
                            for (i, bucket_population) in
                                histograms[chan].buckets.iter().enumerate()
                            {
                                let v = i as f64 * norm;
                                sum[chan] += v * (*bucket_population as f64);
                                sample_count[chan] += *bucket_population;
                            }
                        }
                    }

                    // Compute floor.
                    for chan in 0..3 {
                        let n = sum[chan] / sample_count[chan].max(1) as f64;
                        ui_data.lock_mut().sensor_floor[chan] = n.max(0.0).min(1.0) as f32;
                    }
                } else {
                    let histogram_sets =
                        bracket_images_to_histogram_sets(&*bracket_image_sets.lock());

                    // Estimate sensor floor for each channel.
                    let mut floor: [Option<f32>; 3] = [None; 3];
                    for histograms in histogram_sets.iter() {
                        if status.lock().is_canceled() {
                            return;
                        }
                        for i in 0..3 {
                            let norm = 1.0 / (histograms[i][0].0.buckets.len() - 1) as f32;
                            if let Some((f, _)) = estimate_sensor_floor_ceiling(&histograms[i]) {
                                if let Some(ref mut floor) = floor[i] {
                                    *floor = floor.min(f * norm);
                                } else {
                                    floor[i] = Some(f * norm);
                                }
                            }
                        }
                    }

                    for i in 0..3 {
                        ui_data.lock_mut().sensor_floor[i] = floor[i].unwrap_or(0.0);
                    }
                }
            });
    }

    fn estimate_sensor_ceiling(&self) {
        use sensor_analysis::estimate_sensor_floor_ceiling;

        let bracket_image_sets = self.bracket_image_sets.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .add_job("Estimate Sensor Ceiling", move |status| {
                status
                    .lock_mut()
                    .set_progress(format!("Estimating sensor ceiling"), 0.0);

                let histogram_sets = bracket_images_to_histogram_sets(&*bracket_image_sets.lock());

                // Estimate sensor floor for each channel.
                let mut ceiling: [Option<f32>; 3] = [None; 3];
                for histograms in histogram_sets.iter() {
                    if status.lock().is_canceled() {
                        return;
                    }
                    for i in 0..3 {
                        let norm = 1.0 / (histograms[i][0].0.buckets.len() - 1) as f32;
                        if let Some((_, c)) = estimate_sensor_floor_ceiling(&histograms[i]) {
                            if let Some(ref mut ceiling) = ceiling[i] {
                                *ceiling = ceiling.max(c * norm);
                            } else {
                                ceiling[i] = Some(c * norm);
                            }
                        }
                    }
                }

                for i in 0..3 {
                    ui_data.lock_mut().sensor_ceiling[i] = ceiling[i].unwrap_or(1.0);
                }
            });
    }

    fn estimate_transfer_curve(&self) {
        use sensor_analysis::{emor, ExposureMapping};

        let bracket_image_sets = self.bracket_image_sets.clone_ref();
        let transfer_function_tables = self.transfer_function_tables.clone_ref();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .add_job("Estimate Transfer Function", move |status| {
                ui_data.lock_mut().transfer_function_type = TransferFunction::Estimated;
                let total_rounds = ui_data.lock().rounds;

                let histogram_sets = bracket_images_to_histogram_sets(&*bracket_image_sets.lock());

                let floor = ui_data.lock().sensor_floor;
                let ceiling = ui_data.lock().sensor_ceiling;

                // Compute exposure mappings.
                status
                    .lock_mut()
                    .set_progress(format!("Computing exposure mappings"), 0.2);
                let mut mappings = Vec::new();
                for histograms in histogram_sets.iter() {
                    for chan in 0..histograms.len() {
                        for i in 0..histograms[chan].len() {
                            if status.lock().is_canceled() {
                                return;
                            }
                            for j in 0..1 {
                                let j = j + 1;
                                if (i + j) < histograms[chan].len() {
                                    mappings.push(ExposureMapping::from_histograms(
                                        &histograms[chan][i].0,
                                        &histograms[chan][i + j].0,
                                        histograms[chan][i].1,
                                        histograms[chan][i + j].1,
                                        floor[chan],
                                        ceiling[chan],
                                    ));
                                }
                            }
                        }
                    }
                }

                // Estimate transfer function.
                let rounds_per_update = (1000 / mappings.len()).max(1);
                let mut estimator =
                    emor::EmorEstimator::new(&mappings, histogram_sets[0][0][0].0.buckets.len());
                for round_i in 0..(total_rounds / rounds_per_update) {
                    status.lock_mut().set_progress(
                        format!(
                            "Estimating transfer function, round {}/{}",
                            round_i * rounds_per_update,
                            total_rounds
                        ),
                        0.3,
                    );
                    if status.lock().is_canceled() {
                        return;
                    }

                    estimator.do_rounds(rounds_per_update);
                    let (emor_factors, err) = estimator.current_estimate();
                    let mut curves: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
                    for i in 0..3 {
                        curves[i] =
                            // emor::emor_factors_to_curve(&emor_factors, floor[i], ceiling[i]);
                            emor::emor_factors_to_curve(&emor_factors, 0.0, 1.0);
                    }

                    // Store the curve and the preview.
                    let preview_curves: [Vec<(f32, f32)>; 3] = [
                        curves[0]
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(i, y)| (i as f32 / (curves[0].len() - 1) as f32, y))
                            .collect(),
                        curves[1]
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(i, y)| (i as f32 / (curves[1].len() - 1) as f32, y))
                            .collect(),
                        curves[2]
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(i, y)| (i as f32 / (curves[2].len() - 1) as f32, y))
                            .collect(),
                    ];
                    *transfer_function_tables.lock_mut() = Some((curves, 0.0, 1.0));
                    ui_data.lock_mut().transfer_function_preview = Some((preview_curves, err));
                }
            });
    }

    fn estimate_everything(&self) {
        self.estimate_sensor_floor();
        self.estimate_sensor_ceiling();
        self.estimate_transfer_curve();
    }

    fn save_lut(&self, path: &std::path::Path, to_linear: bool) {
        let transfer_function_tables = self.transfer_function_tables.clone_ref();
        let ui_data = self.ui_data.clone_ref();
        let path = path.to_path_buf();

        self.job_queue.add_job("Save LUT", move |status| {
            status
                .lock_mut()
                .set_progress(format!("Saving LUT: {}", path.to_string_lossy(),), 0.0);

            let (function, floor, ceiling, resolution) = {
                let ui_data = ui_data.lock();
                (
                    ui_data.transfer_function_type,
                    ui_data.sensor_floor,
                    ui_data.sensor_ceiling,
                    ui_data.transfer_function_resolution,
                )
            };

            // Compute the LUT.
            let lut = if function == TransferFunction::Estimated {
                // Estimated function.
                let (tables, range_min, range_max) =
                    transfer_function_tables.lock().clone().unwrap();

                let lut = colorbox::lut::Lut1D {
                    ranges: vec![(range_min, range_max)],
                    tables: tables.to_vec(),
                };

                // Invert the LUT if needed.
                if to_linear {
                    lut.resample_inverted(1024)
                } else {
                    lut
                }
            } else if to_linear {
                // Fixed function, to linear.
                let norm = 1.0 / (resolution - 1) as f32;
                colorbox::lut::Lut1D {
                    ranges: vec![(0.0, 1.0)],
                    tables: (0..3)
                        .map(|chan| {
                            (0..resolution)
                                .map(|i| {
                                    function.to_linear(i as f32 * norm, floor[chan], ceiling[chan])
                                })
                                .collect()
                        })
                        .collect(),
                }
            } else {
                // Fixed function, from linear.
                let ranges: Vec<_> = (0..3)
                    .map(|chan| {
                        (
                            function.to_linear(0.0, floor[chan], ceiling[chan]),
                            function.to_linear(0.0, floor[chan], ceiling[chan]),
                        )
                    })
                    .collect();

                let tables: Vec<Vec<_>> = (0..3)
                    .map(|chan| {
                        let norm = 1.0 / (ranges[chan].1 - ranges[chan].0);
                        (0..resolution)
                            .map(|i| {
                                function.to_linear(
                                    ranges[chan].0 + (i as f32 * norm),
                                    floor[chan],
                                    ceiling[chan],
                                )
                            })
                            .collect()
                    })
                    .collect();

                colorbox::lut::Lut1D {
                    ranges: ranges,
                    tables: tables,
                }
            };

            // Write out the LUT.
            colorbox::formats::cube::write_1d(
                &mut std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
                [(lut.ranges[0].0, lut.ranges[0].1); 3],
                [&lut.tables[0], &lut.tables[1], &lut.tables[2]],
            )
            .unwrap();
        });
    }
}

/// Utility function to get histograms into the right order for processing.
fn bracket_images_to_histogram_sets(
    image_sets: &[Vec<([Histogram; 3], ImageInfo)>],
) -> Vec<[Vec<(Histogram, f32)>; 3]> {
    let mut histogram_sets: Vec<[Vec<(Histogram, f32)>; 3]> = Vec::new();
    for images in image_sets.iter() {
        let mut histograms = [Vec::new(), Vec::new(), Vec::new()];
        for src_img in images.iter() {
            for chan in 0..3 {
                if let Some(exposure) = src_img.1.exposure {
                    histograms[chan].push((src_img.0[chan].clone(), exposure));
                }
            }
        }

        histogram_sets.push(histograms);
    }
    histogram_sets
}

//-------------------------------------------------------------

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum TransferFunction {
    Estimated,
    CanonLog1,
    CanonLog2,
    CanonLog3,
    HLG,
    PQ,
    Rec709,
    SonySlog1,
    SonySlog2,
    SonySlog3,
    sRGB,
}

const TRANSFER_FUNCTIONS: &[TransferFunction] = &[
    TransferFunction::Estimated,
    TransferFunction::sRGB,
    TransferFunction::Rec709,
    TransferFunction::HLG,
    TransferFunction::PQ,
    TransferFunction::CanonLog1,
    TransferFunction::CanonLog2,
    TransferFunction::CanonLog3,
    TransferFunction::SonySlog1,
    TransferFunction::SonySlog2,
    TransferFunction::SonySlog3,
];

impl TransferFunction {
    fn to_linear(&self, mut n: f32, floor: f32, ceil: f32) -> f32 {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;

        let (nl_black, nl_sat, _, _) = self.constants();
        n = nl_black + (n * (nl_sat - nl_black));
        n = (n - floor) / (ceil - floor);

        match *self {
            Estimated => panic!("No built-in function for an estimated transfer function."),
            CanonLog1 => canon_log1::to_linear(n),
            CanonLog2 => canon_log2::to_linear(n),
            CanonLog3 => canon_log3::to_linear(n),
            HLG => hlg::to_linear(n),
            PQ => pq::to_linear(n),
            Rec709 => rec709::to_linear(n),
            SonySlog1 => sony_slog1::to_linear(n),
            SonySlog2 => sony_slog2::to_linear(n),
            SonySlog3 => sony_slog3::to_linear(n),
            sRGB => srgb::to_linear(n),
        }
    }

    fn from_linear(&self, n: f32, floor: f32, ceil: f32) -> f32 {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;

        let (nl_black, nl_sat, _, _) = self.constants();

        let mut out = match *self {
            Estimated => panic!("No built-in function for an estimated transfer function."),
            CanonLog1 => canon_log1::from_linear(n),
            CanonLog2 => canon_log2::from_linear(n),
            CanonLog3 => canon_log3::from_linear(n),
            HLG => hlg::from_linear(n),
            PQ => pq::from_linear(n),
            Rec709 => rec709::from_linear(n),
            SonySlog1 => sony_slog1::from_linear(n),
            SonySlog2 => sony_slog2::from_linear(n),
            SonySlog3 => sony_slog3::from_linear(n),
            sRGB => srgb::from_linear(n),
        };

        out = (out - nl_black) / (nl_sat - nl_black);
        out = floor + (out * (ceil - floor));

        out
    }

    /// Returns (NONLINEAR_BLACK, NONLINEAR_MAX, LINEAR_MIN, LINEAR_MAX) for the
    /// transfer function.
    ///
    /// - NONLINEAR_BLACK is the non-linear value of linear = 0.0.
    /// - NONLINEAR_MAX is the maximum nonlinear value that should be
    ///   reportable by a camera sensor.  Usually 1.0, but some transfer
    ///   functions are weird.
    /// - LINEAR_MIN/MAX are the linear values when the encoded value is
    ///   0.0 and 1.0.
    fn constants(&self) -> (f32, f32, f32, f32) {
        use colorbox::transfer_functions::*;
        use TransferFunction::*;
        match *self {
            Estimated => panic!("No built-in function for an estimated transfer function."),
            CanonLog1 => {
                use canon_log1::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX)
            }
            CanonLog2 => {
                use canon_log2::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX)
            }
            CanonLog3 => {
                use canon_log3::*;
                (NONLINEAR_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX)
            }
            HLG => (0.0, 1.0, 0.0, 1.0),
            PQ => {
                use pq::*;
                (0.0, 1.0, 0.0, LUMINANCE_MAX)
            }
            Rec709 => (0.0, 1.0, 0.0, 1.0),
            SonySlog1 => {
                use sony_slog1::*;
                (CV_BLACK, CV_SATURATION, LINEAR_MIN, LINEAR_MAX)
            }
            SonySlog2 => {
                use sony_slog2::*;
                (CV_BLACK, CV_SATURATION, LINEAR_MIN, LINEAR_MAX)
            }
            SonySlog3 => {
                use sony_slog3::*;
                (CV_BLACK, 1.0, LINEAR_MIN, LINEAR_MAX)
            }
            sRGB => (0.0, 1.0, 0.0, 1.0),
        }
    }

    fn ui_text(&self) -> &'static str {
        use TransferFunction::*;
        match *self {
            Estimated => "Estimated",
            CanonLog1 => "Canon Log",
            CanonLog2 => "Canon Log 2",
            CanonLog3 => "Canon Log 3",
            HLG => "Rec.2100 - HLG",
            PQ => "Rec.2100 - PQ",
            Rec709 => "Rec.709",
            SonySlog1 => "Sony S-Log",
            SonySlog2 => "Sony S-Log2",
            SonySlog3 => "Sony S-Log3",
            sRGB => "sRGB",
        }
    }
}
