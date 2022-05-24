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

        // Hack to work around egui bug:
        // https://github.com/emilk/egui/issues/1649
        let (min_co, max_co) = {
            let mut min_co = (range_x.0, range_y.0);
            let mut max_co = (range_x.1, range_y.1);
            let extent_co = (max_co.0 - min_co.0, max_co.1 - min_co.1);
            let needed_x_extent = {
                let extent_ui = (ui.available_width(), ui.available_height());
                let aspect_ui = extent_ui.0 / extent_ui.1;
                extent_co.1 * aspect_ui
            };
            if needed_x_extent > extent_co.0 {
                let pad = (needed_x_extent - extent_co.0) * 0.5;
                min_co.0 -= pad;
                max_co.0 += pad;
            }
            (min_co, max_co)
        };

        let colors: &[_] = if lut.tables.len() == 1 {
            &[WHITE]
        } else if lut.tables.len() <= 4 {
            &[RED, GREEN, BLUE, WHITE]
        } else {
            unreachable!()
        };
        Plot::new("transfer function plot")
            .data_aspect(1.0)
            .include_x(min_co.0)
            .include_x(max_co.0)
            .include_y(min_co.1)
            .include_y(max_co.1)
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
