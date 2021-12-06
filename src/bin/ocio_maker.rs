#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui, epi};

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

            ui_data: Shared::new(UIData {
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

    ui_data: Shared<UIData>,
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
struct UIData {
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
                        let ui_data = &mut *self.ui_data.lock_mut();

                        let mut space_i = 0;
                        let mut selected_i = ui_data.selected_space_index;

                        // Input spaces.
                        ui.horizontal(|ui| {
                            ui.label("Input Spaces");
                            add_input_space |= ui.button("Add").clicked();
                        });
                        for input_space in ui_data.input_spaces.iter() {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                    .clicked()
                                {
                                    remove_i = Some(space_i);
                                }
                                if ui
                                    .add(egui::widgets::SelectableLabel::new(
                                        space_i == ui_data.selected_space_index,
                                        &input_space.name,
                                    ))
                                    .clicked()
                                {
                                    selected_i = space_i;
                                }
                            });

                            space_i += 1;
                        }

                        // Output spaces.
                        ui.horizontal(|ui| {
                            ui.label("Output Spaces");
                            add_output_space |= ui.button("Add").clicked();
                        });
                        for output_space in ui_data.output_spaces.iter() {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                    .clicked()
                                {
                                    remove_i = Some(space_i);
                                }
                                if ui
                                    .add(egui::widgets::SelectableLabel::new(
                                        space_i == ui_data.selected_space_index,
                                        &output_space.name,
                                    ))
                                    .clicked()
                                {
                                    selected_i = space_i;
                                }
                            });

                            space_i += 1;
                        }

                        ui_data.selected_space_index = selected_i;
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
                let ui_data = &mut *self.ui_data.lock_mut();
                let space_data = if ui_data.selected_space_index < ui_data.input_spaces.len() {
                    let i = ui_data.selected_space_index;
                    Some((
                        true,
                        ui_data.selected_space_index,
                        &mut ui_data.input_spaces[i],
                    ))
                } else if (ui_data.selected_space_index - ui_data.input_spaces.len())
                    < ui_data.output_spaces.len()
                {
                    let i = ui_data.selected_space_index - ui_data.input_spaces.len();
                    Some((
                        false,
                        ui_data.selected_space_index,
                        &mut ui_data.output_spaces[i],
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

                    // Transfer function.
                    ui.horizontal(|ui| {
                        ui.label(if is_input {
                            "Transfer LUT (to linear):"
                        } else {
                            "Transfer LUT (from linear):"
                        });

                        if let Some((ref mut lut, ref mut inverse)) = space.transfer_lut {
                            // TODO.
                        } else {
                            // TODO: load LUT.
                        }
                    });

                    // Color LUT.
                    ui.horizontal(|ui| {
                        ui.label("3D LUT: ");

                        if let Some(ref lut) = space.color_lut.0 {
                            // TODO.
                        } else {
                            // TODO: load LUT.
                        }
                    });
                    if space.color_lut.0.is_some() {
                        ui.horizontal(|ui| {
                            ui.label("Reverse 3D LUT (optional): ");

                            if let Some(ref lut) = space.color_lut.1 {
                                // TODO.
                            } else {
                                // TODO: load LUT.
                            }
                        });
                    }

                    // Chromaticity space.
                    ui.horizontal(|ui| {
                        ui.label("Gamut: ");
                        egui::ComboBox::from_id_source("Gamut")
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
                                    ChromaSpace::AcesAP0,
                                    ChromaSpace::AcesAP0.ui_text(),
                                );
                                ui.selectable_value(
                                    &mut space.chroma_space,
                                    ChromaSpace::AcesAP1,
                                    ChromaSpace::AcesAP1.ui_text(),
                                );
                            });
                    });
                }
            }
        });

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
    fn remove_color_space(&mut self, space_i: usize) {
        let ui_data = &mut *self.ui_data.lock_mut();

        if space_i < ui_data.input_spaces.len() {
            ui_data.input_spaces.remove(space_i);
        } else if (space_i - ui_data.input_spaces.len()) < ui_data.output_spaces.len() {
            ui_data
                .output_spaces
                .remove(space_i - ui_data.input_spaces.len());
        }

        if ui_data.selected_space_index > space_i {
            ui_data.selected_space_index = ui_data.selected_space_index.saturating_sub(1);
        }

        let total = ui_data.input_spaces.len() + ui_data.output_spaces.len();
        ui_data.selected_space_index = total.saturating_sub(1).min(ui_data.selected_space_index);
    }

    fn add_input_color_space(&mut self) {
        let ui_data = &mut *self.ui_data.lock_mut();
        ui_data.input_spaces.push(SpaceTransform {
            name: format!("New Input Color Space {}", ui_data.input_spaces.len()),
            transfer_lut: None,
            color_lut: (None, None),
            chroma_space: ChromaSpace::None,
        });
    }

    fn add_output_color_space(&mut self) {
        let ui_data = &mut *self.ui_data.lock_mut();
        ui_data.output_spaces.push(SpaceTransform {
            name: format!("New Output Color Space {}", ui_data.output_spaces.len()),
            transfer_lut: None,
            color_lut: (None, None),
            chroma_space: ChromaSpace::None,
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
    color_lut: (Option<Lut3D>, Option<Lut3D>), // Forward and reverse.
    chroma_space: ChromaSpace,
}

#[derive(Debug, Clone)]
struct Lut1D {
    filepath: Option<PathBuf>,
    input_range: (f32, f32),
    r: Vec<f32>,
    g: Vec<f32>,
    b: Vec<f32>,
}

#[derive(Debug, Clone)]
struct Lut3D {
    filepath: Option<PathBuf>,
    input_range_x: (f32, f32),
    input_range_y: (f32, f32),
    input_range_z: (f32, f32),
    resolution: (usize, usize, usize),
    table: Vec<[f32; 3]>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ChromaSpace {
    None,
    Rec709,
    Rec2020,
    AcesAP0,
    AcesAP1,
}

impl ChromaSpace {
    fn chromaticities(&self) -> Option<colorbox::chroma::Chromaticities> {
        match *self {
            ChromaSpace::None => None,
            ChromaSpace::Rec709 => Some(colorbox::chroma::REC709),
            ChromaSpace::Rec2020 => Some(colorbox::chroma::REC2020),
            ChromaSpace::AcesAP0 => Some(colorbox::chroma::ACES_AP0),
            ChromaSpace::AcesAP1 => Some(colorbox::chroma::ACES_AP1),
        }
    }

    fn ui_text(&self) -> &'static str {
        match *self {
            ChromaSpace::None => "None",
            ChromaSpace::Rec709 => "Rec.709",
            ChromaSpace::Rec2020 => "Rec.2020",
            ChromaSpace::AcesAP0 => "ACES APO",
            ChromaSpace::AcesAP1 => "ACES AP1",
        }
    }
}
