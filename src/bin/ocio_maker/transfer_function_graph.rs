use crate::egui::{self, Ui};

use lib::colors::*;

pub fn graph(ui: &mut Ui, app: &mut crate::AppMain) {
    let ui_data = &mut *app.ui_data.lock_mut();
    let selected_space_index = ui_data.selected_space_index;

    let space = &mut ui_data.color_spaces[selected_space_index];

    // Visualize transfer function.
    if let Some((ref lut, _, inverse)) = space.transfer_lut {
        use egui::widgets::plot::{Line, Plot, Value, Values};
        let range_x = lut
            .ranges
            .iter()
            .fold((0.0f32, 1.0f32), |(a, b), (c, d)| (a.min(*c), b.max(*d)));
        let range_y = lut.tables.iter().fold((0.0f32, 1.0f32), |(a, b), table| {
            (a.min(table[0]), b.max(*table.last().unwrap()))
        });
        let aspect = {
            let extent_x = range_x.1 - range_x.0;
            let extent_y = range_y.1 - range_y.0;
            if inverse {
                extent_y / extent_x
            } else {
                extent_x / extent_y
            }
        };
        let colors: &[_] = if lut.tables.len() == 1 {
            &[WHITE]
        } else if lut.tables.len() <= 4 {
            &[RED, GREEN, BLUE, WHITE]
        } else {
            unreachable!()
        };
        Plot::new("transfer function plot")
            .data_aspect(aspect)
            .include_x(range_x.0)
            .include_x(range_x.1)
            .include_y(range_y.0)
            .include_y(range_y.1)
            .show(ui, |plot| {
                for (component, table) in lut.tables.iter().enumerate() {
                    let range = lut.ranges[component.min(lut.ranges.len() - 1)];
                    plot.line(
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
            });
    }
}
