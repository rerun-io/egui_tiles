use egui::{Align2, Margin, NumExt};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TableDemo {
    num_columns: usize,
    num_rows: u64,
    num_sticky_cols: usize,
    default_column: egui_table::Column,
    auto_size_mode: egui_table::AutoSizeMode,
    top_row_height: f32,
    row_height: f32,
    prefetched: Vec<egui_table::PrefetchInfo>,
}

impl Default for TableDemo {
    fn default() -> Self {
        Self {
            num_columns: 20,
            num_rows: 10_000,
            num_sticky_cols: 1,
            default_column: egui_table::Column::new(100.0, 10.0..=500.0),
            auto_size_mode: egui_table::AutoSizeMode::default(),
            top_row_height: 24.0,
            row_height: 20.0,
            prefetched: vec![],
        }
    }
}

impl TableDemo {
    fn was_row_prefetched(&self, row_nr: u64) -> bool {
        self.prefetched
            .iter()
            .any(|info| info.visible_rows.contains(&row_nr))
    }
}

impl egui_table::TableDelegate for TableDemo {
    fn prefetch_columns_and_rows(&mut self, info: &egui_table::PrefetchInfo) {
        for row in info.visible_rows.clone() {
            assert!(
                row < self.num_rows,
                "Was asked to prefetch rows {:?}, but we only have {} rows. This is a bug in egui_table.",
                info.visible_rows,
                self.num_rows
            );
        }
        self.prefetched.push(info.clone());
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell_inf: &egui_table::HeaderCellInfo) {
        let egui_table::HeaderCellInfo {
            group_index,
            col_range,
            row_nr,
            ..
        } = cell_inf;

        egui::Frame::none()
            .inner_margin(Margin::symmetric(4.0, 0.0))
            .show(ui, |ui| {
                #[allow(clippy::collapsible_else_if)]
                if *row_nr == 0 {
                    if 0 < col_range.start {
                        // Our special grouped column.
                        let sticky = true;
                        let text = format!("This is group {group_index}");
                        if sticky {
                            // Put the text leftmost in the clip rect (so it is always visible)
                            let font_id = egui::TextStyle::Heading.resolve(ui.style());
                            let text_color = ui.visuals().text_color();
                            let galley =
                                ui.painter()
                                    .layout(text, font_id, text_color, f32::INFINITY);
                            let mut pos = Align2::LEFT_CENTER
                                .anchor_size(ui.clip_rect().left_center(), galley.size())
                                .min;

                            // … but not so far to the right that it doesn't fit.
                            pos.x = pos.x.at_most(ui.max_rect().right() - galley.size().x);

                            ui.painter().galley(pos, galley, text_color);
                        } else {
                            ui.heading(text);
                        }
                    }
                } else {
                    if col_range.start == 0 {
                        egui::Sides::new().height(ui.available_height()).show(
                            ui,
                            |ui| {
                                ui.heading("Row");
                            },
                            |ui| {
                                ui.label("⬇");
                            },
                        );
                    } else {
                        ui.heading(format!("Column {group_index}"));
                    }
                }
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell_info: &egui_table::CellInfo) {
        let egui_table::CellInfo { row_nr, col_nr, .. } = *cell_info;

        if row_nr % 2 == 1 {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        egui::Frame::none()
            .inner_margin(Margin::symmetric(4.0, 0.0))
            .show(ui, |ui| {
                if !self.was_row_prefetched(row_nr) {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 0.0, ui.visuals().error_fg_color);
                    ui.label("ERROR: row not prefetched");
                    log::warn!(
                        "Was asked to show row {row_nr} which was not prefetched! This is a bug in egui_table."
                    );
                    return;
                }

                if col_nr == 0 {
                    ui.label(row_nr.to_string());
                } else {
                    ui.label(format!("({row_nr}, {col_nr})"));

                    if (row_nr + col_nr as u64) % 27 == 0 {
                        if !ui.is_sizing_pass() {
                            // During a sizing pass we don't truncate!
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        }
                        ui.label("Extra long cell!");
                    }
                }
            });
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
            ui.add(
                egui::DragValue::new(&mut self.num_rows)
                    .speed(speed)
                    .range(0..=10_000),
            );
            ui.end_row();

            ui.label("Height of top row");
            ui.add(egui::DragValue::new(&mut self.top_row_height).range(0.0..=100.0));
            ui.end_row();

            ui.label("Height of other rows");
            ui.add(egui::DragValue::new(&mut self.row_height).range(0.0..=100.0));
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
                ui.radio_value(
                    &mut self.auto_size_mode,
                    egui_table::AutoSizeMode::Never,
                    "Never",
                );
                ui.radio_value(
                    &mut self.auto_size_mode,
                    egui_table::AutoSizeMode::Always,
                    "Always",
                );
                ui.radio_value(
                    &mut self.auto_size_mode,
                    egui_table::AutoSizeMode::OnParentResize,
                    "OnParentResize",
                );
            });
            ui.end_row();
        });

        let id_salt = egui::Id::new("table_demo");
        let state_id = egui_table::TableState::id(ui, id_salt); // Note: must be here (in the correct outer `ui` scope) to be correct.

        ui.horizontal(|ui| {
            if ui.button("Reset settings").clicked() {
                *self = Self::default();
            }
            if ui.button("Reset state").clicked() {
                debug_assert!(
                    egui_table::TableState::load(ui.ctx(), state_id).is_some(),
                    "Wrong state_id"
                );
                egui_table::TableState::reset(ui.ctx(), state_id);
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            for info in &self.prefetched {
                ui.label("Visible columns:");
                ui.label(format!(
                    "{}..{}",
                    info.visible_columns.start, info.visible_columns.end
                ));

                ui.label("rows:");
                ui.label(format!(
                    "{}..{}",
                    info.visible_rows.start, info.visible_rows.end
                ));
            }
            self.prefetched.clear();
        });

        ui.separator();

        egui_table::Table {
            columns: vec![self.default_column; self.num_columns],
            id_salt,
            num_sticky_cols: self.num_sticky_cols,
            headers: vec![
                egui_table::HeaderRow {
                    height: self.top_row_height,
                    groups: vec![0..1, 1..4, 4..8, 8..12],
                },
                egui_table::HeaderRow::new(self.top_row_height),
            ],
            row_height: self.row_height,
            num_rows: self.num_rows,
            auto_size_mode: self.auto_size_mode,
        }
        .show(ui, self);
    }
}
