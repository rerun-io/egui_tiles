use std::ops::Range;

use egui_table::{AutoSizeMode, CellInfo, Column, Table, TableDelegate, TableState};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TableDemo {
    num_columns: usize,
    num_rows: u64,
    num_sticky_cols: usize,
    default_column: Column,
    auto_size_mode: AutoSizeMode,
    prefetched_row_ranges: Vec<Range<u64>>,
}

impl Default for TableDemo {
    fn default() -> Self {
        Self {
            num_columns: 10,
            num_rows: 100,
            num_sticky_cols: 1,
            default_column: Column::new(100.0, 10.0..=500.0),
            auto_size_mode: AutoSizeMode::default(),
            prefetched_row_ranges: vec![],
        }
    }
}

impl TableDemo {
    fn was_prefetched(&self, row_nr: u64) -> bool {
        self.prefetched_row_ranges
            .iter()
            .any(|range| range.contains(&row_nr))
    }
}

impl TableDelegate for TableDemo {
    fn prefetch_rows(&mut self, row_numbers: std::ops::Range<u64>) {
        self.prefetched_row_ranges.push(row_numbers);
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        let CellInfo { row_nr, col_nr, .. } = *cell;

        ui.add_space(4.0);

        if !self.was_prefetched(row_nr) {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().error_fg_color);
            ui.label("ERROR: row not prefetched");
            log::warn!("Was asked to show row {row_nr} which was not prefetched! This is a bug.");
            return;
        }

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
        egui::Grid::new("settings").show(ui, |ui| {
            ui.label("Columns");
            ui.add(egui::DragValue::new(&mut self.num_columns));
            ui.end_row();

            ui.label("Rows");
            let speed = 1.0 + 0.05 * self.num_rows as f32;
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.num_rows)
                        .speed(speed)
                        .range(0..=10_000),
                );
                ui.weak("(includes header row)");
            });
            ui.end_row();

            ui.label("Sticky columns");
            ui.add(egui::DragValue::new(&mut self.num_sticky_cols));
            ui.end_row();

            ui.label("Default column width");
            ui.add(egui::DragValue::new(&mut self.default_column.current));
            ui.end_row();

            ui.label("Column width range");
            ui.horizontal(|ui| {
                let range = &mut self.default_column.range;
                ui.add(egui::DragValue::new(&mut range.min).range(0.0..=range.max));
                ui.label("to");
                ui.add(egui::DragValue::new(&mut range.max).range(range.min..=1000.0));
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

        let id_salt = egui::Id::new("table_demo");
        let state_id = TableState::id(ui, id_salt); // Note: must be here (in the correct outer `ui` scope) to be correct.

        ui.horizontal(|ui| {
            if ui.button("Reset settings").clicked() {
                *self = Self::default();
            }
            if ui.button("Reset state").clicked() {
                debug_assert!(
                    TableState::load(ui.ctx(), state_id).is_some(),
                    "Wrong state_id"
                );
                TableState::reset(ui.ctx(), state_id);
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Prefetched row ranges:");
            for range in &self.prefetched_row_ranges {
                ui.label(format!("{}..{}", range.start, range.end));
            }
            self.prefetched_row_ranges.clear();
        });

        ui.separator();

        Table {
            columns: vec![self.default_column; self.num_columns],
            id_salt,
            num_sticky_cols: self.num_sticky_cols,
            sticky_row_heights: vec![20.0; 1],
            row_height: 16.0,
            num_rows: self.num_rows,
            auto_size_mode: self.auto_size_mode,
        }
        .show(ui, self);
    }
}
