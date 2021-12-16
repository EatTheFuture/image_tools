#![windows_subsystem = "windows"] // Don't go through console on Windows.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui, epi};
use egui::Color32;

use colorbox::{formats, lut::Lut1D};
use shared_data::Shared;

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

            ui_data: Shared::new(UIData {
                color_spaces: Vec::new(),
                selected_space_index: 0,
                export_path: String::new(),
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

    ui_data: Shared<UIData>,
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
struct UIData {
    color_spaces: Vec<ColorSpaceSpec>,
    selected_space_index: usize,
    export_path: String,
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
        let current_export_dir = if !self.ui_data.lock().export_path.is_empty() {
            self.ui_data.lock().export_path.clone().into()
        } else {
            working_dir.clone()
        };
        let select_export_directory_dialog = rfd::FileDialog::new()
            .set_directory(&current_export_dir)
            .set_title("Select Export Directory");

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
            .resizable(false)
            .show(ctx, |ui| {
                let mut remove_i = None;
                let mut add_input_space = false;

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.strong("Custom Color Spaces");
                    add_input_space |= ui.button("Add").clicked();
                });
                ui.add_space(4.0);

                egui::containers::ScrollArea::vertical()
                    .auto_shrink([true, false])
                    .show(ui, |ui| {
                        let ui_data = &mut *self.ui_data.lock_mut();

                        let mut space_i = 0;
                        let mut selected_i = ui_data.selected_space_index;

                        for input_space in ui_data.color_spaces.iter() {
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

                        ui_data.selected_space_index = selected_i;
                    });

                if add_input_space {
                    self.add_input_color_space();
                }
                if let Some(space_i) = remove_i {
                    self.remove_color_space(space_i);
                }
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                let mut ui_data = self.ui_data.lock_mut();
                ui.label("Config Directory: ");
                ui.add(
                    egui::widgets::TextEdit::singleline(&mut ui_data.export_path)
                        .id(egui::Id::new("Export Path")),
                );
                if ui.button("Browse...").clicked() {
                    if let Some(path) = select_export_directory_dialog.pick_folder() {
                        ui_data.export_path = path.to_string_lossy().into();
                    }
                }
                ui.add_space(16.0);
                if ui
                    .add_enabled(job_count == 0, egui::widgets::Button::new("Export Config"))
                    .clicked()
                {
                    self.export_config();
                }
            });

            ui.add(egui::widgets::Separator::default().spacing(12.0));

            // Main UI area.
            {
                let ui_data = &mut *self.ui_data.lock_mut();
                let selected_space_index = ui_data.selected_space_index;

                if selected_space_index < ui_data.color_spaces.len() {
                    let space = &mut ui_data.color_spaces[selected_space_index];

                    // Name and Misc.
                    ui.horizontal(|ui| {
                        ui.label("Name: ");
                        ui.add(
                            egui::widgets::TextEdit::singleline(&mut space.name)
                                .id(egui::Id::new(format!("csname{}", selected_space_index))),
                        );

                        ui.add_space(16.0);

                        ui.checkbox(&mut space.include_as_display, "Include as Display");
                    });

                    ui.add_space(8.0);

                    // Graphing colors.
                    use lib::colors::*;

                    // Chromaticity space.
                    ui.horizontal(|ui| {
                        ui.label("Chromaticities / Gamut: ");
                        egui::ComboBox::from_id_source("Chromaticity Space")
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
                    let transfer_lut_label = "Transfer Function (to linear): ";
                    if let Some((_, ref filepath, ref mut inverse)) = space.transfer_lut {
                        ui.horizontal(|ui| {
                            ui.label(transfer_lut_label);
                            ui.strong(if let Some(name) = filepath.file_name() {
                                name.to_string_lossy()
                            } else {
                                "Unnamed LUT".into()
                            });
                            if ui
                                .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                                .clicked()
                            {
                                self.remove_transfer_function(selected_space_index);
                            }
                        });
                        ui.indent(0, |ui| {
                            ui.checkbox(
                                inverse,
                                "Invert Transfer Function (should curve to the lower right)",
                            )
                        });
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

                    // Visualize chromaticities / gamut.
                    if let Some(chroma) = space.chroma_space.chromaticities() {
                        use egui::widgets::plot::{
                            HLine, Line, LineStyle, Plot, VLine, Value, Values,
                        };
                        let mut plot = Plot::new("chromaticities_plot")
                            .data_aspect(1.0)
                            .height(250.0)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .show_x(false)
                            .show_y(false)
                            .show_axes([false, false]);
                        let wp_style = LineStyle::Dashed { length: 10.0 };
                        let r = Value {
                            x: chroma.r.0,
                            y: chroma.r.1,
                        };
                        let g = Value {
                            x: chroma.g.0,
                            y: chroma.g.1,
                        };
                        let b = Value {
                            x: chroma.b.0,
                            y: chroma.b.1,
                        };
                        let w = Value {
                            x: chroma.w.0,
                            y: chroma.w.1,
                        };

                        // Spectral locus and boundary lines.
                        plot = plot.line(
                            Line::new(Values::from_values_iter({
                                use colorbox::tables::cie_1931_xyz::{X, Y, Z};
                                (0..X.len()).chain(0..1).map(|i| Value {
                                    x: (X[i] / (X[i] + Y[i] + Z[i])) as f64,
                                    y: (Y[i] / (X[i] + Y[i] + Z[i])) as f64,
                                })
                            }))
                            .color(GRAY),
                        );
                        plot = plot
                            .hline(HLine::new(0.0).color(Color32::from_rgb(50, 50, 50)))
                            .vline(VLine::new(0.0).color(Color32::from_rgb(50, 50, 50)));

                        // Color space
                        plot = plot
                            .line(
                                Line::new(Values::from_values_iter([r, g].iter().copied()))
                                    .color(YELLOW),
                            )
                            .line(
                                Line::new(Values::from_values_iter([g, b].iter().copied()))
                                    .color(CYAN),
                            )
                            .line(
                                Line::new(Values::from_values_iter([b, r].iter().copied()))
                                    .color(MAGENTA),
                            )
                            .line(
                                Line::new(Values::from_values_iter([r, w].iter().copied()))
                                    .color(RED)
                                    .style(wp_style),
                            )
                            .line(
                                Line::new(Values::from_values_iter([g, w].iter().copied()))
                                    .color(GREEN)
                                    .style(wp_style),
                            )
                            .line(
                                Line::new(Values::from_values_iter([b, w].iter().copied()))
                                    .color(BLUE)
                                    .style(wp_style),
                            );
                        ui.add(plot);

                        ui.add_space(8.0);
                    }

                    // Visualize transfer function.
                    if let Some((ref lut, _, inverse)) = space.transfer_lut {
                        use egui::widgets::plot::{Line, Plot, Value, Values};
                        let aspect = {
                            let range_x = lut
                                .ranges
                                .iter()
                                .fold((0.0f32, 1.0f32), |(a, b), (c, d)| (a.min(*c), b.max(*d)));
                            let range_y =
                                lut.tables.iter().fold((0.0f32, 1.0f32), |(a, b), table| {
                                    (a.min(table[0]), b.max(*table.last().unwrap()))
                                });
                            let extent_x = range_x.1 - range_x.0;
                            let extent_y = range_y.1 - range_y.0;
                            if inverse {
                                extent_y / extent_x
                            } else {
                                extent_x / extent_y
                            }
                        };
                        let mut plot = Plot::new("transfer function plot").data_aspect(aspect);
                        let colors: &[_] = if lut.tables.len() == 1 {
                            &[WHITE]
                        } else if lut.tables.len() <= 4 {
                            &[RED, GREEN, BLUE, WHITE]
                        } else {
                            unreachable!()
                        };
                        for (component, table) in lut.tables.iter().enumerate() {
                            let range = lut.ranges[component.min(lut.ranges.len() - 1)];
                            plot = plot.line(
                                Line::new(Values::from_values_iter(
                                    table.iter().copied().enumerate().map(|(i, y)| {
                                        let a = i as f32 / (table.len() - 1).max(1) as f32;
                                        let x = range.0 + (a * (range.1 - range.0));
                                        if inverse {
                                            Value::new(y, x)
                                        } else {
                                            Value::new(x, y)
                                        }
                                    }),
                                ))
                                .color(colors[component]),
                            );
                        }
                        ui.add(plot);
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
        let ui_data = &mut *self.ui_data.lock_mut();

        if space_i < ui_data.color_spaces.len() {
            ui_data.color_spaces.remove(space_i);
        }

        if ui_data.selected_space_index > space_i {
            ui_data.selected_space_index = ui_data.selected_space_index.saturating_sub(1);
        }

        ui_data.selected_space_index = ui_data
            .color_spaces
            .len()
            .saturating_sub(1)
            .min(ui_data.selected_space_index);
    }

    fn add_input_color_space(&self) {
        let ui_data = &mut *self.ui_data.lock_mut();
        let name = {
            let mut new_name = "New Color Space".into();
            for i in 1..200 {
                let name = format!("{} {}", new_name, i);
                let mut taken = false;
                for space in ui_data.color_spaces.iter() {
                    taken |= space.name == name;
                }
                if !taken {
                    new_name = name;
                    break;
                }
            }
            new_name
        };
        ui_data.color_spaces.push(ColorSpaceSpec::with_name(&name));
        ui_data.selected_space_index = ui_data.color_spaces.len() - 1;
    }

    fn load_transfer_function(&self, lut_path: &Path, color_space_index: usize) {
        let path: PathBuf = lut_path.into();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .add_job("Load Transfer Function LUT", move |status| {
                status
                    .lock_mut()
                    .set_progress(format!("Loading: {}", path.to_string_lossy()), 0.0);

                // Load lut.
                let lut = match lib::job_helpers::load_1d_lut(&path) {
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
                    let mut ui_data = ui_data.lock_mut();
                    if color_space_index < ui_data.color_spaces.len() {
                        ui_data.color_spaces[color_space_index].transfer_lut =
                            Some((lut, path, false));
                    }
                }
            });
    }

    fn remove_transfer_function(&self, color_space_index: usize) {
        let ui_data = self.ui_data.clone_ref();

        self.job_queue
            .add_job("Remove Transfer Function LUT", move |status| {
                status.lock_mut().set_progress("Removing LUT".into(), 0.0);
                let mut ui_data = ui_data.lock_mut();

                if color_space_index < ui_data.color_spaces.len() {
                    ui_data.color_spaces[color_space_index].transfer_lut = None;
                }
            });
    }

    fn export_config(&self) {
        use colorbox::matrix;
        use ocio_gen::config::*;

        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Export Config", move |status| {
            status
                .lock_mut()
                .set_progress("Generating config".into(), 0.0);

            let export_path = ui_data.lock().export_path.clone();
            if export_path.is_empty() {
                status
                    .lock_mut()
                    .log_error("Failed to export: no config directory selected yet.".into());
                return;
            }

            // Template config.
            let mut config = ocio_gen::blender_config::make_blender_3_0();
            let ref_chroma = ocio_gen::blender_config::REFERENCE_SPACE_CHROMA;

            // Prep to add our own stuff.
            let output_dir: &Path = "ocio_maker".as_ref();
            let mut output_files: HashMap<PathBuf, OutputFile> = HashMap::new();
            let space_count = ui_data.lock().color_spaces.len();

            // Add color spaces.
            for i in 0..space_count {
                if let Some(space) = ui_data.lock().color_spaces.get(i).map(|s| s.clone()) {
                    let space_name = space
                        .name
                        .replace("\\", "\\\\")
                        .replace("#", "\\#")
                        .replace("\"", "\\\"")
                        .replace("]", "\\]")
                        .replace("}", "\\}");
                    let mut transforms = Vec::new();
                    let mut transforms_inv = Vec::new();

                    let lut_data = space.transfer_lut.map(|(lut, ref path, inverse)| {(
                        output_files
                            .entry(path.into())
                            .or_insert(
                                OutputFile::Lut1D {
                                    output_path: output_dir.join(format!(
                                        "omkr_{}__{}",
                                        i,
                                        path.file_name()
                                            .map(|f| f.to_str())
                                            .flatten()
                                            .unwrap_or("lut.cube")
                                    )),
                                    lut: lut.clone(),
                                }
                            ),
                        inverse
                    )});

                    let matrix_pair = space.chroma_space.chromaticities().map(|chroma| {
                        let forward = colorbox::matrix_compose!(
                            matrix::rgb_to_xyz_matrix(chroma),
                            matrix::xyz_chromatic_adaptation_matrix(
                                chroma.w,
                                ref_chroma.w,
                                matrix::AdaptationMethod::Bradford,
                            ),
                            matrix::xyz_to_rgb_matrix(ref_chroma),
                        );
                        let inverse = colorbox::matrix::invert(forward).unwrap();
                        (
                            colorbox::matrix::to_4x4_f32(forward),
                            colorbox::matrix::to_4x4_f32(inverse),
                        )
                    });

                    // "To Reference" transform.
                    if let Some((ref lut_file, inverse)) = lut_data {
                        transforms.push(Transform::FileTransform {
                            src: lut_file.path().file_name().unwrap().into(),
                            interpolation: Interpolation::Linear,
                            direction_inverse: inverse,
                        });
                    }
                    if let Some((matrix_forward, _)) = matrix_pair {
                        transforms.push(Transform::MatrixTransform(matrix_forward));
                    }

                    // "From Reference" transform.
                    if let Some((_, matrix_backward)) = matrix_pair {
                        transforms_inv.push(Transform::MatrixTransform(matrix_backward));
                    }
                    if let Some((ref lut_file, inverse)) = lut_data {
                        transforms_inv.push(Transform::FileTransform {
                            src: lut_file.path().file_name().unwrap().into(),
                            interpolation: Interpolation::Linear,
                            direction_inverse: !inverse,
                        });
                    }

                    // Create the colorspace.
                    config.colorspaces.push(ColorSpace {
                        name: space_name.clone(),
                        family: "Custom (OCIO Maker)".into(),
                        bitdepth: Some(BitDepth::F32),
                        isdata: Some(false),
                        to_reference: transforms,
                        from_reference: transforms_inv,
                        ..ColorSpace::default()
                    });

                    if space.include_as_display {
                        config.displays.push(Display {
                            name: space_name.clone(),
                            views: vec![("Standard".into(), space_name.clone())],
                        });
                        config.active_displays.push(space_name.clone());
                    }
                }
            }

            // Add LUT files.
            config.search_path.push(output_dir.into());
            for (_, output_file) in output_files.drain() {
                config.output_files.push(output_file);
            }

            // Check for validation errors.
            use ocio_gen::config::ValidationError::*;
            match config.validate() {
                Err(DuplicateColorSpace(name)) => {
                    status
                        .lock_mut()
                        .log_error(format!("There is a duplicate color space in the config: \"{}\" \
                                            \nNote: this may be a conflict with the built-ins of the \
                                            config template, rather that two visible duplicates in \
                                            your own colorspace list.", name));
                    return;
                },
                Err(DuplicateDisplay(name)) => {
                    status
                        .lock_mut()
                        .log_error(format!("There is a duplicate display in the config: \"{}\" \
                                            \nNote: this may be a conflict with the built-ins of the \
                                            config template, rather that two visible duplicates in \
                                            your own colorspace list.", name));
                    return;
                },
                Err(DuplicateRole(name)) => {
                    status
                        .lock_mut()
                        .log_error(format!("There is a duplicate role in the config: \"{}\"", name));
                    return;
                },
                Err(DuplicateLook(name)) => {
                    status
                        .lock_mut()
                        .log_error(format!("There is a duplicate look in the config: \"{}\"", name));
                    return;
                },
                Err(ReferenceToAbsentColorSpace(name)) => {
                    status
                        .lock_mut()
                        .log_error(format!("There is a reference to a non-existent colorspace in the config: \"{}\"", name));
                    return;
                },
                Ok(()) => {},
            }

            // Write it out to disk.
            status
                .lock_mut()
                .set_progress("Writing config to disk".into(), 0.0);
            config
                .write_to_directory(export_path)
                .expect("Failed to write OCIO config");

            status.lock_mut().log_note("Export successful!".into());
        });
    }
}

//-------------------------------------------------------------

#[derive(Debug, Clone)]
struct ColorSpaceSpec {
    name: String,
    transfer_lut: Option<(Lut1D, PathBuf, bool)>, // The bool is whether to do the inverse transform.
    chroma_space: ChromaSpace,
    include_as_display: bool,
}

impl ColorSpaceSpec {
    fn with_name(name: &str) -> ColorSpaceSpec {
        ColorSpaceSpec {
            name: name.into(),
            transfer_lut: None,
            chroma_space: ChromaSpace::None,
            include_as_display: false,
        }
    }
}

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
            ChromaSpace::Rec709 => "Rec.709 / sRGB",
            ChromaSpace::Rec2020 => "Rec.2020",
            ChromaSpace::DciP3 => "DCI-P3",
            ChromaSpace::AcesAP0 => "ACES APO",
            ChromaSpace::AcesAP1 => "ACES AP1",
            ChromaSpace::AdobeRGB => "Adobe RGB",
            ChromaSpace::AdobeWideGamutRGB => "Adobe Wide Gamut RGB",
            ChromaSpace::ProPhoto => "ProPhoto",
            ChromaSpace::SGamut => "Sony S-Gamut / S-Gamut3",
            ChromaSpace::SGamut3Cine => "Sony S-Gamut3.Cine",
            ChromaSpace::AlexaWideGamutRGB => "Alexa Wide Gamut RGB",
            ChromaSpace::RedWideGamutRGB => "RED Wide Gamut RGB",
        }
    }
}
