use std::path::PathBuf;

use colorbox::formats;

use crate::egui::{self, Ui};

use crate::ChromaSpace;

pub fn editor(
    ui: &mut Ui,
    space: &mut crate::ColorSpaceSpec,
    space_id: &str,
    job_count: usize,
    working_dir: &mut PathBuf,
) -> Result<(), String> {
    let load_1d_lut_dialog = {
        let mut d = rfd::FileDialog::new()
            .set_title("Load 1D LUT")
            .add_filter("All Supported LUTs", &["spi1d", "cube"])
            .add_filter("cube", &["cube"])
            .add_filter("spi1d", &["spi1d"]);
        if !working_dir.as_os_str().is_empty() && working_dir.is_dir() {
            d = d.set_directory(&working_dir);
        }
        d
    };

    // Name and Misc.
    ui.horizontal(|ui| {
        ui.label("Name: ");
        ui.add(egui::widgets::TextEdit::singleline(&mut space.name).id(egui::Id::new(space_id)));

        ui.add_space(16.0);

        ui.checkbox(&mut space.include_as_display, "Include as Display");
    });

    ui.add_space(8.0);

    // Chromaticity space.
    chromaticity_editor(ui, "Chromaticities / Gamut: ", space);
    ui.add_space(8.0);

    // Transfer function.
    let transfer_lut_label = "Transfer Function (to linear): ";
    let mut remove_lut = false;
    if let Some((_, ref filepath, ref mut inverse)) = space.transfer_lut {
        ui.horizontal(|ui| {
            ui.label(transfer_lut_label);
            ui.strong(if let Some(name) = filepath.file_name() {
                let tmp: String = name.to_string_lossy().into();
                tmp
            } else {
                "Unnamed LUT".into()
            });
            if ui
                .add_enabled(job_count == 0, egui::widgets::Button::new("🗙"))
                .clicked()
            {
                remove_lut = true;
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
                .add_enabled(job_count == 0, egui::widgets::Button::new("Load 1D LUT..."))
                .clicked()
            {
                if let Some(path) = load_1d_lut_dialog.clone().pick_file() {
                    if let Some(parent) = path.parent().map(|p| p.into()) {
                        *working_dir = parent;
                    }

                    match lib::job_helpers::load_1d_lut(&path) {
                        Ok(lut) => space.transfer_lut = Some((lut, path.clone(), false)),
                        Err(formats::ReadError::IoErr(_)) => {
                            return Err(format!(
                                "Unable to access file \"{}\".",
                                path.to_string_lossy()
                            ));
                        }
                        Err(formats::ReadError::FormatErr) => {
                            return Err(format!(
                                "Not a 1D LUT file: \"{}\".",
                                path.to_string_lossy()
                            ));
                        }
                    }
                }
            }
            Ok(())
        })
        .inner?;
    }

    if remove_lut {
        space.transfer_lut = None;
    }

    Ok(())
}

pub fn chromaticity_editor(ui: &mut Ui, label: &str, space: &mut crate::ColorSpaceSpec) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_source("Chromaticity Space")
            .width(256.0)
            .selected_text(format!("{}", space.chroma_space.ui_text()))
            .show_ui(ui, |ui| {
                for cs in super::CHROMA_SPACES {
                    ui.selectable_value(&mut space.chroma_space, *cs, cs.ui_text());
                }
            });
    });

    // Custom chromaticity coordinates.
    if space.chroma_space == ChromaSpace::Custom {
        ui.indent("custom_chroma_container", |ui| {
            egui::Grid::new("custom_chroma")
                .min_col_width(4.0)
                .show(ui, |ui| {
                    let precision = 0.0001;

                    ui.label("");
                    ui.label("x");
                    ui.label("y");
                    ui.end_row();

                    ui.label("R");
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.r.0)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.r.1)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.end_row();

                    ui.label("G");
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.g.0)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.g.1)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.end_row();

                    ui.label("B");
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.b.0)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.b.1)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.end_row();

                    ui.label("W");
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.w.0)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.add(
                        egui::widgets::DragValue::new(&mut space.custom_chroma.w.1)
                            .clamp_range(-1.0..=2.0)
                            .speed(precision),
                    );
                    ui.end_row();
                });
        });
        ui.add_space(8.0);
    }
}
