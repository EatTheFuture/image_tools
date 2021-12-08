#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui, epi};

use colorbox::formats;
use ocio_gen;
use sensor_analysis::invert_transfer_function_lut;
use shared_data::Shared;

use lib::{ImageInfo, SourceImage};

fn main() {
    clap::App::new("OCIO Maker")
        .version("0.1")
        .author("Nathan Vegdahl")
        .about("Make OCIO configurations easily")
        .get_matches();

    eframe::run_native(
        Box::new(AppMain {
            job_queue: job_queue::JobQueue::new(),
            last_opened_directory: std::env::current_dir().ok(),

            fast_data: Shared::new(FastData {
                input_spaces: Vec::new(),
                output_spaces: Vec::new(),
                selected_space_index: 0,
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

    fast_data: Shared<FastData>,
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
struct FastData {
    input_spaces: Vec<SpaceTransform>,
    output_spaces: Vec<SpaceTransform>,
    selected_space_index: usize,
}

impl epi::App for AppMain {
    fn name(&self) -> &str {
        "OCIO Maker"
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
        let mut working_dir = self
            .last_opened_directory
            .clone()
            .unwrap_or_else(|| "".into());

        // File dialogs used in the UI.
        let load_1d_lut_dialog = rfd::FileDialog::new()
            .set_directory(&working_dir)
            .set_title("Load 1D LUT")
            .add_filter("All Supported LUTs", &["spi1d", "cube"])
            .add_filter("cube", &["cube"])
            .add_filter("spi1d", &["spi1d"]);

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

        // Color space list (left-side panel).
        egui::containers::panel::SidePanel::left("color_space_list")
            .min_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                let mut remove_i = None;
                let mut add_input_space = false;
                let mut add_output_space = false;

                egui::containers::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let fast_data = &mut *self.fast_data.lock_mut();

                        let mut space_i = 0;
                        let mut selected_i = fast_data.selected_space_index;

                        // Input spaces.
                        ui.horizontal(|ui| {
                            ui.strong("Input Transforms");
                            add_input_space |= ui.button("Add").clicked();
                        });
                        ui.add_space(4.0);
                        for input_space in fast_data.input_spaces.iter() {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                    .clicked()
                                {
                                    remove_i = Some(space_i);
                                }
                                if ui
                                    .add(egui::widgets::SelectableLabel::new(
                                        space_i == fast_data.selected_space_index,
                                        &input_space.name,
                                    ))
                                    .clicked()
                                {
                                    selected_i = space_i;
                                }
                            });

                            space_i += 1;
                        }

                        ui.add_space(16.0);

                        // Output spaces.
                        ui.horizontal(|ui| {
                            ui.strong("Output Transforms");
                            add_output_space |= ui.button("Add").clicked();
                        });
                        ui.add_space(4.0);
                        for output_space in fast_data.output_spaces.iter() {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                    .clicked()
                                {
                                    remove_i = Some(space_i);
                                }
                                if ui
                                    .add(egui::widgets::SelectableLabel::new(
                                        space_i == fast_data.selected_space_index,
                                        &output_space.name,
                                    ))
                                    .clicked()
                                {
                                    selected_i = space_i;
                                }
                            });

                            space_i += 1;
                        }

                        fast_data.selected_space_index = selected_i;
                    });

                if add_input_space {
                    self.add_input_color_space();
                }
                if add_output_space {
                    self.add_output_color_space();
                }
                if let Some(space_i) = remove_i {
                    self.remove_color_space(space_i);
                }
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                if ui
                    .add_enabled(
                        job_count == 0,
                        egui::widgets::Button::new("Export Config..."),
                    )
                    .clicked()
                {
                    self.export_config();
                }
            });

            ui.add(egui::widgets::Separator::default().spacing(12.0));

            // Main UI area.
            {
                let fast_data = &mut *self.fast_data.lock_mut();
                let selected_space_index = fast_data.selected_space_index;
                let space_data = if fast_data.selected_space_index < fast_data.input_spaces.len() {
                    Some((
                        true,
                        fast_data.selected_space_index,
                        &mut fast_data.input_spaces[selected_space_index],
                    ))
                } else if (fast_data.selected_space_index - fast_data.input_spaces.len())
                    < fast_data.output_spaces.len()
                {
                    let i = selected_space_index - fast_data.input_spaces.len();
                    Some((
                        false,
                        fast_data.selected_space_index,
                        &mut fast_data.output_spaces[i],
                    ))
                } else {
                    None
                };

                if let Some((is_input, index, space)) = space_data {
                    // Name.
                    ui.horizontal(|ui| {
                        ui.label("Name: ");
                        ui.add(
                            egui::widgets::TextEdit::singleline(&mut space.name)
                                .id(egui::Id::new(format!("csname{}", index))),
                        );
                    });

                    ui.add_space(8.0);

                    // Chromaticity space.
                    ui.horizontal(|ui| {
                        ui.label(if is_input {
                            "Input Gamut: "
                        } else {
                            "Output Gamut: "
                        });
                        egui::ComboBox::from_id_source("Gamut")
                            .width(256.0)
                            .selected_text(format!("{}", space.chroma_space.ui_text()))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::None,
                                    ChromaSpace::None.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::Rec709,
                                    ChromaSpace::Rec709.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::Rec2020,
                                    ChromaSpace::Rec2020.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::DciP3,
                                    ChromaSpace::DciP3.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AcesAP0,
                                    ChromaSpace::AcesAP0.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AcesAP1,
                                    ChromaSpace::AcesAP1.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AdobeRGB,
                                    ChromaSpace::AdobeRGB.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AdobeWideGamutRGB,
                                    ChromaSpace::AdobeWideGamutRGB.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::ProPhoto,
                                    ChromaSpace::ProPhoto.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::SGamut,
                                    ChromaSpace::SGamut.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::SGamut3Cine,
                                    ChromaSpace::SGamut3Cine.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AlexaWideGamutRGB,
                                    ChromaSpace::AlexaWideGamutRGB.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::RedWideGamutRGB,
                                    ChromaSpace::RedWideGamutRGB.ui_text(),
                                );
                            });
                    });

                    ui.add_space(8.0);

                    // Transfer function.
                    let transfer_lut_label = if is_input {
                        "1D Transfer LUT (to linear):"
                    } else {
                        "1D Transfer LUT (from linear):"
                    };
                    if let Some((ref lut, ref mut inverse)) = space.transfer_lut {
                        ui.horizontal(|ui| {
                            ui.label(transfer_lut_label);
                            ui.strong(
                                if let Some(Some(name)) =
                                    lut.filepath.as_ref().map(|p| p.file_name())
                                {
                                    name.to_string_lossy()
                                } else {
                                    "Unnamed LUT".into()
                                },
                            );
                            if ui
                                .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                .clicked()
                            {
                                self.remove_transfer_function(selected_space_index);
                            }
                        });
                        ui.indent(0, |ui| ui.checkbox(inverse, "Reverse Transfer LUT"));
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(transfer_lut_label);
                            if ui
                                .add_enabled(
                                    job_count == 0,
                                    egui::widgets::Button::new("Load 1D LUT..."),
                                )
                                .clicked()
                            {
                                if let Some(path) = load_1d_lut_dialog.clone().pick_file() {
                                    self.load_transfer_function(&path, selected_space_index);
                                    if let Some(parent) = path.parent().map(|p| p.into()) {
                                        working_dir = parent
                                    }
                                }
                            }
                        });
                    }

                    ui.add_space(8.0);

                    //---------------------------------
                    // Visualizations.

                    if let Some((ref lut, inverse)) = space.transfer_lut {
                        use egui::widgets::plot::{Line, Plot, Value, Values};
                        let aspect = {
                            let min_x = lut
                                .input_range_r
                                .0
                                .min(lut.input_range_g.0.min(lut.input_range_b.0));
                            let max_x = lut
                                .input_range_r
                                .1
                                .max(lut.input_range_g.1.max(lut.input_range_b.1));
                            let min_y = lut.table_r[0].min(lut.table_g[0].min(lut.table_b[0]));
                            let max_y = lut.table_r.last().unwrap().max(
                                lut.table_g
                                    .last()
                                    .unwrap()
                                    .max(*lut.table_b.last().unwrap()),
                            );
                            let extent_x = (max_x - min_x).max(1.0);
                            let extent_y = (max_y - min_y).max(1.0);
                            if inverse {
                                extent_y / extent_x
                            } else {
                                extent_x / extent_y
                            }
                        };
                        ui.add(
                            Plot::new("transfer_function")
                                .line(Line::new(Values::from_values_iter(
                                    lut.table_r.iter().copied().enumerate().map(|(i, y)| {
                                        let a = i as f32 / (lut.table_r.len() - 1).max(1) as f32;
                                        let x = lut.input_range_r.0
                                            + (a * (lut.input_range_r.1 - lut.input_range_r.0));
                                        if inverse {
                                            Value::new(y, x)
                                        } else {
                                            Value::new(x, y)
                                        }
                                    }),
                                )))
                                .line(Line::new(Values::from_values_iter(
                                    lut.table_g.iter().copied().enumerate().map(|(i, y)| {
                                        let a = i as f32 / (lut.table_g.len() - 1).max(1) as f32;
                                        let x = lut.input_range_g.0
                                            + (a * (lut.input_range_g.1 - lut.input_range_g.0));
                                        if inverse {
                                            Value::new(y, x)
                                        } else {
                                            Value::new(x, y)
                                        }
                                    }),
                                )))
                                .line(Line::new(Values::from_values_iter(
                                    lut.table_b.iter().copied().enumerate().map(|(i, y)| {
                                        let a = i as f32 / (lut.table_b.len() - 1).max(1) as f32;
                                        let x = lut.input_range_b.0
                                            + (a * (lut.input_range_b.1 - lut.input_range_b.0));
                                        if inverse {
                                            Value::new(y, x)
                                        } else {
                                            Value::new(x, y)
                                        }
                                    }),
                                )))
                                .data_aspect(aspect),
                        );
                    }
                }
            }
        });

        self.last_opened_directory = Some(working_dir);

        //----------------
        // Processing.

        // Collect dropped files.
        if !ctx.input().raw.dropped_files.is_empty() {
            // self.add_image_files(
            //     ctx.input()
            //         .raw
            //         .dropped_files
            //         .iter()
            //         .map(|dropped_file| dropped_file.path.as_ref().unwrap().as_path()),
            // );
        }
    }
}

impl AppMain {
    fn remove_color_space(&self, space_i: usize) {
        let fast_data = &mut *self.fast_data.lock_mut();

        if space_i < fast_data.input_spaces.len() {
            fast_data.input_spaces.remove(space_i);
        } else if (space_i - fast_data.input_spaces.len()) < fast_data.output_spaces.len() {
            fast_data
                .output_spaces
                .remove(space_i - fast_data.input_spaces.len());
        }

        if fast_data.selected_space_index > space_i {
            fast_data.selected_space_index = fast_data.selected_space_index.saturating_sub(1);
        }

        let total = fast_data.input_spaces.len() + fast_data.output_spaces.len();
        fast_data.selected_space_index =
            total.saturating_sub(1).min(fast_data.selected_space_index);
    }

    fn add_input_color_space(&self) {
        let fast_data = &mut *self.fast_data.lock_mut();
        fast_data
            .input_spaces
            .push(SpaceTransform::with_name(&format!(
                "New Input Transform #{}",
                fast_data.input_spaces.len() + 1,
            )));
        fast_data.selected_space_index = fast_data.input_spaces.len() - 1;
    }

    fn add_output_color_space(&self) {
        let fast_data = &mut *self.fast_data.lock_mut();
        fast_data
            .output_spaces
            .push(SpaceTransform::with_name(&format!(
                "New Output Transform #{}",
                fast_data.output_spaces.len() + 1,
            )));
        fast_data.selected_space_index =
            fast_data.input_spaces.len() + fast_data.output_spaces.len() - 1;
    }

    fn load_transfer_function(&self, lut_path: &Path, color_space_index: usize) {
        let path: PathBuf = lut_path.into();
        let fast_data = self.fast_data.clone_ref();

        self.job_queue.add_job("Load Transfer LUT", move |status| {
            status
                .lock_mut()
                .set_progress(format!("Loading: {}", path.to_string_lossy()), 0.0);

            // Load lut.
            let lut = match Lut1D::from_file(&path) {
                Ok(lut) => lut,
                Err(formats::ReadError::IoErr(_)) => {
                    status.lock_mut().log_error(format!(
                        "Unable to access file \"{}\".",
                        path.to_string_lossy()
                    ));
                    return;
                }
                Err(formats::ReadError::FormatErr) => {
                    status.lock_mut().log_error(format!(
                        "Not a 1D LUT file: \"{}\".",
                        path.to_string_lossy()
                    ));
                    return;
                }
            };

            // Set this as the lut for the passed color space index.
            {
                let mut fast_data = fast_data.lock_mut();
                let selected_space_index = fast_data.selected_space_index;

                let space = if selected_space_index < fast_data.input_spaces.len() {
                    Some(&mut fast_data.input_spaces[selected_space_index])
                } else if (selected_space_index - fast_data.input_spaces.len())
                    < fast_data.output_spaces.len()
                {
                    let i = selected_space_index - fast_data.input_spaces.len();
                    Some(&mut fast_data.output_spaces[i])
                } else {
                    None
                };

                if let Some(space) = space {
                    space.transfer_lut = Some((lut, false));
                }
            }
        });
    }

    fn remove_transfer_function(&self, color_space_index: usize) {
        let fast_data = self.fast_data.clone_ref();

        self.job_queue
            .add_job("Remove Transfer LUT", move |status| {
                status.lock_mut().set_progress("Removing LUT".into(), 0.0);

                // Set this as the lut for the passed color space index.
                {
                    let mut fast_data = fast_data.lock_mut();
                    let selected_space_index = fast_data.selected_space_index;

                    let space = if selected_space_index < fast_data.input_spaces.len() {
                        Some(&mut fast_data.input_spaces[selected_space_index])
                    } else if (selected_space_index - fast_data.input_spaces.len())
                        < fast_data.output_spaces.len()
                    {
                        let i = selected_space_index - fast_data.input_spaces.len();
                        Some(&mut fast_data.output_spaces[i])
                    } else {
                        None
                    };

                    if let Some(space) = space {
                        space.transfer_lut = None;
                    }
                }
            });
    }

    fn export_config(&self) {
        todo!();
    }
}

//-------------------------------------------------------------

#[derive(Debug, Clone)]
struct SpaceTransform {
    name: String,
    transfer_lut: Option<(Lut1D, bool)>, // The bool is whether to do the inverse transform.
    // color_lut: (Option<Lut3D>, Option<Lut3D>), // Forward and reverse.
    chroma_space: ChromaSpace,
}

impl SpaceTransform {
    fn with_name(name: &str) -> SpaceTransform {
        SpaceTransform {
            name: name.into(),
            transfer_lut: None,
            chroma_space: ChromaSpace::None,
        }
    }
}

#[derive(Debug, Clone)]
struct Lut1D {
    filepath: Option<PathBuf>,
    input_range_r: (f32, f32),
    input_range_g: (f32, f32),
    input_range_b: (f32, f32),
    table_r: Vec<f32>,
    table_g: Vec<f32>,
    table_b: Vec<f32>,
}

impl Lut1D {
    fn from_file(path: &Path) -> Result<Lut1D, formats::ReadError> {
        let file = std::io::BufReader::new(std::fs::File::open(path)?);

        match path.extension().map(|e| e.to_str()) {
            Some(Some("cube")) => {
                let [(min1, max1, table1), (min2, max2, table2), (min3, max3, table3)] =
                    formats::cube::read_1d(file)?;

                Ok(Lut1D {
                    filepath: Some(path.into()),
                    input_range_r: (min1, max1),
                    input_range_g: (min2, max2),
                    input_range_b: (min3, max3),
                    table_r: table1,
                    table_g: table2,
                    table_b: table3,
                })
            }

            Some(Some("spi1d")) => {
                let (min, max, _, [table1, table2, table3]) = formats::spi1d::read(file)?;

                Ok(Lut1D {
                    filepath: Some(path.into()),
                    input_range_r: (min, max),
                    input_range_g: (min, max),
                    input_range_b: (min, max),
                    table_r: table1,
                    table_g: table2,
                    table_b: table3,
                })
            }

            _ => Err(formats::ReadError::FormatErr),
        }
    }
}

// #[derive(Debug, Clone)]
// struct Lut3D {
//     filepath: Option<PathBuf>,
//     input_range_r: (f32, f32),
//     input_range_g: (f32, f32),
//     input_range_b: (f32, f32),
//     resolution: (usize, usize, usize),
//     table: Vec<[f32; 3]>,
// }

// impl Lut3D {
//     fn from_file(path: &Path) -> Result<Lut3D, formats::ReadError> {
//         todo!()
//     }
// }

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ChromaSpace {
    None,
    Rec709,
    Rec2020,
    DciP3,
    AcesAP0,
    AcesAP1,
    AdobeRGB,
    AdobeWideGamutRGB,
    ProPhoto,
    SGamut,
    SGamut3Cine,
    AlexaWideGamutRGB,
    RedWideGamutRGB,
}

impl ChromaSpace {
    fn chromaticities(&self) -> Option<colorbox::chroma::Chromaticities> {
        match *self {
            ChromaSpace::None => None,
            ChromaSpace::Rec709 => Some(colorbox::chroma::REC709),
            ChromaSpace::Rec2020 => Some(colorbox::chroma::REC2020),
            ChromaSpace::DciP3 => Some(colorbox::chroma::DCI_P3),
            ChromaSpace::AcesAP0 => Some(colorbox::chroma::ACES_AP0),
            ChromaSpace::AcesAP1 => Some(colorbox::chroma::ACES_AP1),
            ChromaSpace::AdobeRGB => Some(colorbox::chroma::ADOBE_RGB),
            ChromaSpace::AdobeWideGamutRGB => Some(colorbox::chroma::ADOBE_WIDE_GAMUT_RGB),
            ChromaSpace::ProPhoto => Some(colorbox::chroma::PROPHOTO),
            ChromaSpace::SGamut => Some(colorbox::chroma::S_GAMUT),
            ChromaSpace::SGamut3Cine => Some(colorbox::chroma::S_GAMUT3_CINE),
            ChromaSpace::AlexaWideGamutRGB => Some(colorbox::chroma::ALEXA_WIDE_GAMUT_RGB),
            ChromaSpace::RedWideGamutRGB => Some(colorbox::chroma::RED_WIDE_GAMUT_RGB),
        }
    }

    fn ui_text(&self) -> &'static str {
        match *self {
            ChromaSpace::None => "None",
            ChromaSpace::Rec709 => "Rec.709",
            ChromaSpace::Rec2020 => "Rec.2020",
            ChromaSpace::DciP3 => "DCI-P3",
            ChromaSpace::AcesAP0 => "ACES APO",
            ChromaSpace::AcesAP1 => "ACES AP1",
            ChromaSpace::AdobeRGB => "Adobe RGB",
            ChromaSpace::AdobeWideGamutRGB => "Adobe Wide Gamut RGB",
            ChromaSpace::ProPhoto => "ProPhoto",
            ChromaSpace::SGamut => "Sony S-Gamut/S-Gamut3",
            ChromaSpace::SGamut3Cine => "Sony S-Gamut3.Cine",
            ChromaSpace::AlexaWideGamutRGB => "Alexa Wide Gamut RGB",
            ChromaSpace::RedWideGamutRGB => "RED Wide Gamut RGB",
        }
    }
}
