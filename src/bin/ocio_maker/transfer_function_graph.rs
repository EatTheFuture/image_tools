use std::iter::FromIterator;

use crate::egui::Ui;

use lib::colors::*;

pub fn graph(ui: &mut Ui, space: &mut crate::ColorSpaceSpec) {
    // Visualize transfer function.
    if let Some((ref lut, _, inverse)) = space.transfer_lut {
        use egui_plot::{Line, Plot, PlotPoints};

        let colors: &[_] = if lut.tables.len() == 1 {
            &[WHITE]
        } else if lut.tables.len() <= 4 {
            &[RED, GREEN, BLUE, WHITE]
        } else {
            unreachable!()
        };
        Plot::new("transfer function plot")
            .data_aspect(1.0)
            .show(ui, |plot| {
                for (component, table) in lut.tables.iter().enumerate() {
                    let range = lut.ranges[component.min(lut.ranges.len() - 1)];
                    plot.line(
                        Line::new(PlotPoints::from_iter(
                            table.iter().copied().enumerate().map(|(i, y)| {
                                let a = i as f32 / (table.len() - 1).max(1) as f32;
                                let x = range.0 + (a * (range.1 - range.0));
                                if inverse {
                                    [y as f64, x as f64]
                                } else {
                                    [x as f64, y as f64]
                                }
                            }),
                        ))
                        .color(colors[component]),
                    );
                }
            });
    }
}
