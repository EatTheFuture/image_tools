#![windows_subsystem = "windows"] // Don't go through console on Windows.

mod colorspace_editor;
mod colorspace_list;
mod gamut_graph;
mod menu;
mod transfer_function_graph;

use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use eframe::egui;

use colorbox::{formats, lut::Lut1D};
use shared_data::Shared;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    clap::App::new("ETF OCIO Maker")
        .version(VERSION)
        .author("Nathan Vegdahl, Ian Hubert")
        .about("Make OCIO configurations easily")
        .get_matches();

    eframe::run_native(
        "OCIO Maker",
        eframe::NativeOptions {
            drag_and_drop_support: true, // Enable drag-and-dropping files on Windows.
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| Box::new(AppMain::new(cc))),
    );
}

pub struct AppMain {
    job_queue: job_queue::JobQueue,
    last_opened_directory: Option<PathBuf>,

    ui_data: Shared<UIData>,
}

impl AppMain {
    fn new(cc: &eframe::CreationContext) -> AppMain {
        // Dark mode.
        cc.egui_ctx.set_visuals(egui::style::Visuals {
            dark_mode: true,
            ..egui::style::Visuals::default()
        });

        // Update callback for jobs.
        let mut job_queue = job_queue::JobQueue::new();
        let ctx_clone = cc.egui_ctx.clone();
        job_queue.set_update_fn(move || {
            ctx_clone.request_repaint();
        });

        AppMain {
            job_queue: job_queue,
            last_opened_directory: std::env::current_dir().ok(),

            ui_data: Shared::new(UIData {
                color_spaces: Vec::new(),
                selected_space_index: 0,
                export_path: String::new(),
            }),
        }
    }
}

/// The stuff the UI code needs access to for drawing and update.
///
/// Nothing other than the UI should lock this data for non-trivial
/// amounts of time.
pub struct UIData {
    color_spaces: Vec<ColorSpaceSpec>,
    selected_space_index: usize,
    export_path: String,
}

impl eframe::App for AppMain {
    // Called before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Don't need to do anything.
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let job_count = self.job_queue.job_count();
        let mut working_dir = self
            .last_opened_directory
            .clone()
            .unwrap_or_else(|| "".into());

        // File dialogs used in the UI.
        let select_export_directory_dialog = {
            let mut d = rfd::FileDialog::new().set_title("Select Export Directory");
            let export_path: PathBuf = self.ui_data.lock().export_path.clone().into();
            if !export_path.as_os_str().is_empty() && export_path.is_dir() {
                d = d.set_directory(&export_path);
            } else if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
                d = d.set_directory(&working_dir);
            };
            d
        };

        //----------------
        // GUI.

        // Menu bar.
        menu::menu_bar(ctx, frame, self, &mut working_dir);

        // Status bar and log (footer).
        egui_custom::status_bar(ctx, &self.job_queue);

        // Color space list (left-side panel).
        egui::containers::panel::SidePanel::left("color_space_list")
            .resizable(false)
            .show(ctx, |ui| {
                colorspace_list::list(ui, self, job_count);
            });

        // Main area.
        egui::containers::panel::CentralPanel::default().show(ctx, |ui| {
            // Config exporting.
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
            if self.ui_data.lock().selected_space_index < self.ui_data.lock().color_spaces.len() {
                colorspace_editor::editor(ui, self, job_count, &mut working_dir);

                ui.add_space(8.0);

                gamut_graph::graph(ui, self);
                transfer_function_graph::graph(ui, self);
            }
        });

        self.last_opened_directory = Some(working_dir);

        //----------------
        // Processing.

        // Collect dropped files.
        if !ctx.input().raw.dropped_files.is_empty() {
            // let file_list: Vec<PathBuf> = ctx
            //     .input()
            //     .raw
            //     .dropped_files
            //     .iter()
            //     .map(|dropped_file| dropped_file.path.clone().unwrap())
            //     .collect();

            // self.add_image_files(file_list, ctx);
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
            // We ignore the result here because we'll encounter the same one later
            // anyway, where it is handled properly.
            let _ = lib::job_helpers::ensure_dir_exists(&export_path);

            // Template config.
            let mut config = ocio_gen::blender_config::make_blender_3_0();
            let ref_chroma = ocio_gen::blender_config::REFERENCE_SPACE_CHROMA;

            // Initial header comment.
            config.header_comment.push_str(&format!("Generated by ETF OCIO Maker v{}\n\n", VERSION));
            config.header_comment.push_str("Do not edit this file by hand if you want to continue managing\n");
            config.header_comment.push_str("this configuration with ETF OCIO Maker.\n\n");
            config.header_comment.push_str("----\n");
            config.header_comment.push_str("template: blender_3_0\n");

            // Prep to add our own stuff.
            let output_dir: &Path = "ocio_maker".as_ref();
            let mut output_files: HashMap<PathBuf, OutputFile> = HashMap::new();
            let space_count = ui_data.lock().color_spaces.len();

            // Add color spaces.
            for i in 0..space_count {
                if let Some(space) = ui_data.lock().color_spaces.get(i).map(|s| s.clone()) {
                    // Add text version to header comment.
                    config.header_comment.push_str("----\n");
                    config.header_comment.push_str(&space.to_string(Some(&export_path)));

                    // Actual export.
                    let space_name = space
                        .name
                        .trim()
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

                    let matrix_pair = space.chroma_space.chromaticities(space.custom_chroma).map(|chroma| {
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

            // Add final separator to the end of the header comment.
            config.header_comment.push_str("----\n");

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

    fn load_config(&self, config_file_path: &Path) {
        let config_file_path: PathBuf = config_file_path.into();
        let ui_data = self.ui_data.clone_ref();

        self.job_queue.add_job("Load Config", move |status| {
            status.lock_mut().set_progress("Loading config".into(), 0.0);

            let mut color_spaces = Vec::new();

            // Parse.
            if let Ok(file) = std::fs::File::open(&config_file_path) {
                let file = BufReader::new(file);

                let mut in_data = false;
                let mut chunk = String::new();
                for (num, line) in file.lines().take_while(|l| l.is_ok()).map(|l| l.unwrap()).enumerate() {
                    if num == 0 && !line.starts_with("# Generated by ETF OCIO Maker") {
                        status.lock_mut().log_error("Failed to load configuration: is not an ETF OCIO Maker-generated config.".into());
                        return;
                    } else if !line.trim().starts_with("#") {
                        break;
                    } else if !in_data && line.trim() == "# ----" {
                        in_data = true;
                    } else if in_data && line.trim() == "# ----" {
                        // Main parsing code, that handles each chunk.
                        if chunk.starts_with("color_space:") {
                            let (color_space, result) = ColorSpaceSpec::from_str(&chunk, config_file_path.parent());
                            match result {
                                Ok(_) => {},
                                Err(ConfigLoadErr::FileUnloadable(s)) => {
                                    status.lock_mut().log_error(format!(
                                        "Unable to load LUT file: \"{}\".  Colorspace \"{}\" is incomplete.",
                                        s, color_space.name
                                    ));
                                },
                            };
                            color_spaces.push(color_space);
                        }
                        chunk.clear();
                    } else {
                        chunk.push_str(&line[1..].trim());
                        chunk.push_str("\n");
                    }
                }
            } else {
                status.lock_mut().log_error(format!(
                    "Unable to access config file: \"{}\"",
                    config_file_path.to_string_lossy()
                ));
                return;
            }

            // Set in-memory config to the same as the parsed one.
            {
                let mut ui_data = ui_data.lock_mut();
                ui_data.color_spaces = color_spaces;
                ui_data.selected_space_index = 0;
                if let Some(parent) = config_file_path.parent().map(|p| p.to_string_lossy()) {
                    ui_data.export_path = parent.into();
                }
            }
        });
    }
}

//-------------------------------------------------------------

#[derive(Debug, Clone)]
struct ColorSpaceSpec {
    name: String,
    transfer_lut: Option<(Lut1D, PathBuf, bool)>, // The bool is whether to do the inverse transform.
    chroma_space: ChromaSpace,
    custom_chroma: colorbox::chroma::Chromaticities,
    include_as_display: bool,
}

impl ColorSpaceSpec {
    fn with_name(name: &str) -> ColorSpaceSpec {
        ColorSpaceSpec {
            name: name.into(),
            ..ColorSpaceSpec::default()
        }
    }

    /// If `base_path` is specified, then all paths will be written as relative to that path.
    fn to_string<P: AsRef<Path>>(&self, base_path: Option<P>) -> String {
        let mut s = String::new();

        s.push_str(&format!("color_space: {}\n", self.name.trim()));
        s.push_str(&format!(
            "include_as_display: {:?}\n",
            self.include_as_display
        ));
        if self.chroma_space != ChromaSpace::None {
            s.push_str(&format!("chroma_space: {}\n", self.chroma_space.as_str()));
            if self.chroma_space == ChromaSpace::Custom {
                s.push_str(&format!(
                    "custom_chroma: {} {} {} {} {} {} {} {}\n",
                    self.custom_chroma.r.0,
                    self.custom_chroma.r.1,
                    self.custom_chroma.g.0,
                    self.custom_chroma.g.1,
                    self.custom_chroma.b.0,
                    self.custom_chroma.b.1,
                    self.custom_chroma.w.0,
                    self.custom_chroma.w.1,
                ));
            }
        }
        if let Some((_, ref path, use_inverse)) = self.transfer_lut {
            // Convert path to be relative, if a base path is available.
            let path = path.canonicalize().unwrap_or_else(|_| path.into());
            let path = if let Some(base) = base_path {
                let base: &Path = base.as_ref();
                let base = base.canonicalize().unwrap_or_else(|_| base.into());
                pathdiff::diff_paths(&path, &base).unwrap_or_else(|| path)
            } else {
                path
            };

            s.push_str(&format!("transfer_lut_path: {}\n", path.to_string_lossy()));
            s.push_str(&format!("transfer_lut_use_inverse: {:?}\n", use_inverse));
        }

        s
    }

    /// If `base_path` is specified, then all relative paths will be interpretted in terms of it.
    fn from_str<P: AsRef<Path>>(
        text: &str,
        base_path: Option<P>,
    ) -> (ColorSpaceSpec, Result<(), ConfigLoadErr>) {
        let mut color_space = ColorSpaceSpec::default();
        let mut result = Ok(());

        for line in text.lines() {
            if let Some((param, value)) = line.split_once(":") {
                let param = param.trim();
                let value = value.trim();
                match param {
                    "color_space" => color_space.name = value.into(),
                    "include_as_display" => {
                        color_space.include_as_display = if value == "true" { true } else { false }
                    }
                    "chroma_space" => {
                        color_space.chroma_space =
                            ChromaSpace::from_str(value).unwrap_or(ChromaSpace::None)
                    }
                    "custom_chroma" => {
                        let values: Vec<f64> = value
                            .split_whitespace()
                            .map(|v| v.parse::<f64>())
                            .filter(|v| v.is_ok())
                            .map(|v| v.unwrap())
                            .collect();
                        if values.len() == 8 {
                            color_space.custom_chroma = colorbox::chroma::Chromaticities {
                                r: (values[0], values[1]),
                                g: (values[2], values[3]),
                                b: (values[4], values[5]),
                                w: (values[6], values[7]),
                            }
                        }
                    }
                    "transfer_lut_path" => {
                        let path: PathBuf = if let Some(ref base_path) = base_path {
                            base_path.as_ref().join(value)
                        } else {
                            value.into()
                        };
                        match lib::job_helpers::load_1d_lut(path) {
                            Ok(lut) => color_space.transfer_lut = Some((lut, value.into(), false)),
                            Err(_) => result = Err(ConfigLoadErr::FileUnloadable(value.into())),
                        }
                    }
                    "transfer_lut_use_inverse" => {
                        if let Some((_, _, ref mut use_inverse)) = color_space.transfer_lut {
                            *use_inverse = if value == "true" { true } else { false };
                        }
                    }
                    _ => {}
                }
            }
        }

        (color_space, result)
    }
}

impl Default for ColorSpaceSpec {
    fn default() -> ColorSpaceSpec {
        ColorSpaceSpec {
            name: "".into(),
            transfer_lut: None,
            chroma_space: ChromaSpace::None,
            custom_chroma: colorbox::chroma::Chromaticities {
                // Default to Rec.2020, just to have a starting point.
                r: (0.708, 0.292),
                g: (0.170, 0.797),
                b: (0.131, 0.046),
                w: (0.3127, 0.3290),
            },
            include_as_display: false,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ChromaSpace {
    None,
    Custom,
    Rec709,
    Rec2020,
    DciP3,
    AcesAP0,
    AcesAP1,
    AdobeRGB,
    AdobeWideGamutRGB,
    AlexaWideGamutRGB,
    BlackmagicWideGamutGen4,
    DJIDGamut,
    PanasonicVGamut,
    ProPhoto,
    RedWideGamutRGB,
    SGamut,
    SGamut3Cine,
}

impl ChromaSpace {
    fn chromaticities(
        &self,
        custom: colorbox::chroma::Chromaticities,
    ) -> Option<colorbox::chroma::Chromaticities> {
        match *self {
            ChromaSpace::None => None,
            ChromaSpace::Custom => Some(custom),
            ChromaSpace::Rec709 => Some(colorbox::chroma::REC709),
            ChromaSpace::Rec2020 => Some(colorbox::chroma::REC2020),
            ChromaSpace::DciP3 => Some(colorbox::chroma::DCI_P3),
            ChromaSpace::AcesAP0 => Some(colorbox::chroma::ACES_AP0),
            ChromaSpace::AcesAP1 => Some(colorbox::chroma::ACES_AP1),
            ChromaSpace::AdobeRGB => Some(colorbox::chroma::ADOBE_RGB),
            ChromaSpace::AdobeWideGamutRGB => Some(colorbox::chroma::ADOBE_WIDE_GAMUT_RGB),
            ChromaSpace::AlexaWideGamutRGB => Some(colorbox::chroma::ALEXA_WIDE_GAMUT_RGB),
            ChromaSpace::BlackmagicWideGamutGen4 => {
                Some(colorbox::chroma::blackmagic::BMD_WIDE_GAMUT_GEN4)
            }
            ChromaSpace::DJIDGamut => Some(colorbox::chroma::DJI_D_GAMUT),
            ChromaSpace::PanasonicVGamut => Some(colorbox::chroma::PANASONIC_V_GAMUT),
            ChromaSpace::ProPhoto => Some(colorbox::chroma::PROPHOTO),
            ChromaSpace::RedWideGamutRGB => Some(colorbox::chroma::RED_WIDE_GAMUT_RGB),
            ChromaSpace::SGamut => Some(colorbox::chroma::sony::S_GAMUT),
            ChromaSpace::SGamut3Cine => Some(colorbox::chroma::sony::S_GAMUT3_CINE),
        }
    }

    fn ui_text(&self) -> &'static str {
        match *self {
            ChromaSpace::None => "None",
            ChromaSpace::Custom => "Custom",
            ChromaSpace::Rec709 => "Rec.709 / sRGB",
            ChromaSpace::Rec2020 => "Rec.2020",
            ChromaSpace::DciP3 => "DCI-P3",
            ChromaSpace::AcesAP0 => "ACES APO",
            ChromaSpace::AcesAP1 => "ACES AP1",
            ChromaSpace::AdobeRGB => "Adobe RGB",
            ChromaSpace::AdobeWideGamutRGB => "Adobe Wide Gamut RGB",
            ChromaSpace::AlexaWideGamutRGB => "Alexa Wide Gamut RGB",
            ChromaSpace::BlackmagicWideGamutGen4 => "BMD Wide Gamut Gen4/Gen5",
            ChromaSpace::DJIDGamut => "DJI D-Gamut",
            ChromaSpace::PanasonicVGamut => "Panasonic V-Gamut",
            ChromaSpace::ProPhoto => "ProPhoto",
            ChromaSpace::RedWideGamutRGB => "RED Wide Gamut RGB",
            ChromaSpace::SGamut => "Sony S-Gamut / S-Gamut3",
            ChromaSpace::SGamut3Cine => "Sony S-Gamut3.Cine",
        }
    }

    fn as_str(&self) -> &'static str {
        match *self {
            ChromaSpace::None => "None",
            ChromaSpace::Custom => "Custom",
            ChromaSpace::Rec709 => "Rec709",
            ChromaSpace::Rec2020 => "Rec2020",
            ChromaSpace::DciP3 => "DciP3",
            ChromaSpace::AcesAP0 => "AcesAPO",
            ChromaSpace::AcesAP1 => "AcesAP1",
            ChromaSpace::AdobeRGB => "AdobeRGB",
            ChromaSpace::AdobeWideGamutRGB => "AdobeWideGamutRGB",
            ChromaSpace::AlexaWideGamutRGB => "AlexaWideGamutRGB",
            ChromaSpace::BlackmagicWideGamutGen4 => "BlackmagicWideGamutGen4",
            ChromaSpace::DJIDGamut => "DJIDGamut",
            ChromaSpace::PanasonicVGamut => "PanasonicVGamut",
            ChromaSpace::ProPhoto => "ProPhoto",
            ChromaSpace::RedWideGamutRGB => "RedWideGamutRGB",
            ChromaSpace::SGamut => "SGamut",
            ChromaSpace::SGamut3Cine => "SGamut3Cine",
        }
    }

    fn from_str(text: &str) -> Option<ChromaSpace> {
        match text {
            "None" => Some(ChromaSpace::None),
            "Custom" => Some(ChromaSpace::Custom),
            "Rec709" => Some(ChromaSpace::Rec709),
            "Rec2020" => Some(ChromaSpace::Rec2020),
            "DciP3" => Some(ChromaSpace::DciP3),
            "AcesAPO" => Some(ChromaSpace::AcesAP0),
            "AcesAP1" => Some(ChromaSpace::AcesAP1),
            "AdobeRGB" => Some(ChromaSpace::AdobeRGB),
            "AdobeWideGamutRGB" => Some(ChromaSpace::AdobeWideGamutRGB),
            "AlexaWideGamutRGB" => Some(ChromaSpace::AlexaWideGamutRGB),
            "BlackmagicWideGamutGen4" => Some(ChromaSpace::BlackmagicWideGamutGen4),
            "DJIDGamut" => Some(ChromaSpace::DJIDGamut),
            "PanasonicVGamut" => Some(ChromaSpace::PanasonicVGamut),
            "ProPhoto" => Some(ChromaSpace::ProPhoto),
            "RedWideGamutRGB" => Some(ChromaSpace::RedWideGamutRGB),
            "SGamut" => Some(ChromaSpace::SGamut),
            "SGamut3Cine" => Some(ChromaSpace::SGamut3Cine),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum ConfigLoadErr {
    FileUnloadable(String),
}
