use crate::egui::{self, Color32, Ui};

use lib::colors::*;

pub fn graph(ui: &mut Ui, app: &mut crate::AppMain) {
    let ui_data = &mut *app.ui_data.lock_mut();
    let selected_space_index = ui_data.selected_space_index;

    let space = &mut ui_data.color_spaces[selected_space_index];

    // Visualize chromaticities / gamut.
    if let Some(chroma) = space.chroma_space.chromaticities(space.custom_chroma) {
        use egui::widgets::plot::{HLine, Line, LineStyle, Plot, VLine, Value, Values};
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

        Plot::new("chromaticities_plot")
            .data_aspect(1.0)
            .height(250.0)
            .width(250.0)
            .include_x(0.0)
            .include_x(1.0)
            .include_y(0.0)
            .include_y(1.0)
            .allow_drag(false)
            .allow_zoom(false)
            .show_x(false)
            .show_y(false)
            .show_axes([false, false])
            .show(ui, |plot| {
                // Spectral locus and boundary lines.
                plot.line(
                    Line::new(Values::from_values_iter({
                        use colorbox::tables::cie_1931_xyz::{X, Y, Z};
                        (0..X.len()).chain(0..1).map(|i| Value {
                            x: (X[i] / (X[i] + Y[i] + Z[i])) as f64,
                            y: (Y[i] / (X[i] + Y[i] + Z[i])) as f64,
                        })
                    }))
                    .color(GRAY),
                );
                plot.hline(HLine::new(0.0).color(Color32::from_rgb(50, 50, 50)));
                plot.vline(VLine::new(0.0).color(Color32::from_rgb(50, 50, 50)));

                // Color space
                plot.line(
                    Line::new(Values::from_values_iter([r, g].iter().copied())).color(YELLOW),
                );
                plot.line(Line::new(Values::from_values_iter([g, b].iter().copied())).color(CYAN));
                plot.line(
                    Line::new(Values::from_values_iter([b, r].iter().copied())).color(MAGENTA),
                );
                plot.line(
                    Line::new(Values::from_values_iter([r, w].iter().copied()))
                        .color(RED)
                        .style(wp_style),
                );
                plot.line(
                    Line::new(Values::from_values_iter([g, w].iter().copied()))
                        .color(GREEN)
                        .style(wp_style),
                );
                plot.line(
                    Line::new(Values::from_values_iter([b, w].iter().copied()))
                        .color(BLUE)
                        .style(wp_style),
                );
            });

        ui.add_space(8.0);
    }
}
