use egui_table::{AutoSizeMode, Column, Table, TableDelegate};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TableDemo {
    num_columns: usize,
    num_rows: usize,
    num_sticky_cols: usize,
    default_column: Column,
    auto_size_mode: AutoSizeMode,
}

impl Default for TableDemo {
    fn default() -> Self {
        Self {
            num_columns: 10,
            num_rows: 100,
            num_sticky_cols: 2,
            default_column: Column::new(100.0, 10.0..=500.0),
            auto_size_mode: AutoSizeMode::default(),
        }
    }
}

impl TableDelegate for TableDemo {
    fn cell_ui(&mut self, ui: &mut egui::Ui, row_nr: usize, col_nr: usize) {
        ui.add_space(4.0);

        if row_nr == 0 {
            ui.heading(format!("Column {col_nr}"));
        } else {
            if row_nr % 2 == 1 {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
            }
            ui.label(format!("({row_nr}, {col_nr})"));
        }

        ui.add_space(4.0);
    }
}

impl TableDemo {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
            ui.label("Columns");
            ui.add(egui::DragValue::new(&mut self.num_columns).speed(1.0));
            ui.end_row();

            ui.label("Rows");
            let speed = 1.0 + 0.05 * self.num_rows as f32;
            ui.add(
                egui::DragValue::new(&mut self.num_rows)
                    .speed(speed)
                    .range(0..=10_000),
            );
            ui.end_row();

            ui.label("Sticky columns");
            ui.add(egui::DragValue::new(&mut self.num_sticky_cols).speed(1.0));
            ui.end_row();

            ui.label("Default column width");
            ui.add(egui::DragValue::new(&mut self.default_column.current).speed(1.0));
            ui.end_row();

            ui.label("Column width range");
            ui.horizontal(|ui| {
                let range = &mut self.default_column.range;
                ui.add(
                    egui::DragValue::new(&mut range.min)
                        .speed(1.0)
                        .range(0.0..=range.max),
                );
                ui.label("to");
                ui.add(
                    egui::DragValue::new(&mut range.max)
                        .speed(1.0)
                        .range(range.min..=1000.0),
                );
            });
            ui.end_row();

            ui.label("Auto-size mode");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.auto_size_mode, AutoSizeMode::Never, "Never");
                ui.radio_value(&mut self.auto_size_mode, AutoSizeMode::Always, "Always");
                ui.radio_value(
                    &mut self.auto_size_mode,
                    AutoSizeMode::OnParentResize,
                    "OnParentResize",
                );
            });
            ui.end_row();
        });

        ui.separator();

        Table {
            columns: vec![self.default_column; self.num_columns],
            id_salt: egui::Id::new("table_demo"),
            num_sticky_cols: self.num_sticky_cols,
            sticky_row_heights: vec![20.0; 1],
            row_height: 16.0,
            num_rows: self.num_rows,
            auto_size_mode: self.auto_size_mode,
        }
        .show(ui, self);
    }
}
