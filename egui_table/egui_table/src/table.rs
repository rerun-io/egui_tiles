use egui::{vec2, Id, IdMap, NumExt as _, Rect, Ui, UiBuilder, Vec2, Vec2b};
use vec1::Vec1;

use crate::{columns::Column, SplitScroll, SplitScrollDelegate};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum AutoSizeMode {
    /// Never auto-size the columns.
    Never,

    /// Always auto-size the columns
    Always,

    /// Auto-size the columns if the parents' width changes
    #[default]
    OnParentResize,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct TableState {
    // Maps columns ids to their widths.
    pub col_widths: IdMap<f32>,

    pub parent_width: Option<f32>,
}

impl TableState {
    pub fn load(ctx: &egui::Context, id: Id) -> Option<Self> {
        ctx.data_mut(|d| d.get_persisted(id))
    }

    pub fn store(self, ctx: &egui::Context, id: Id) {
        ctx.data_mut(|d| d.insert_persisted(id, self));
    }
}

pub struct Table {
    pub columns: Vec<Column>,

    pub id_salt: Id,

    /// Which columns are sticky (non-scrolling)?
    pub num_sticky_cols: usize,

    /// The count and heights of the sticky (non-scrolling) rows.
    /// Usually used for a single header row.
    pub sticky_row_heights: Vec<f32>,

    /// Height of the non-sticky rows.
    pub row_height: f32,

    /// Total number of rows (sticky + non-sticky).
    pub num_rows: usize,

    pub auto_size_mode: AutoSizeMode,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            columns: vec![],
            id_salt: Id::new("table"),
            num_sticky_cols: 1,
            sticky_row_heights: vec![16.0],
            row_height: 16.0,
            num_rows: 0,
            auto_size_mode: AutoSizeMode::default(),
        }
    }
}

pub trait TableDelegate {
    fn cell_ui(&mut self, ui: &mut Ui, row_nr: usize, col_nr: usize);
}

impl Table {
    pub fn show(mut self, ui: &mut Ui, table_delegate: &mut dyn TableDelegate) {
        self.num_sticky_cols = self.num_sticky_cols.at_most(self.columns.len());
        self.num_rows = self.num_rows.at_least(self.sticky_row_heights.len());
        let num_scroll_rows = self.num_rows - self.sticky_row_heights.len();

        let id = ui.make_persistent_id(self.id_salt);
        let mut state: TableState = TableState::load(ui.ctx(), id).unwrap_or_default();

        for (i, column) in self.columns.iter_mut().enumerate() {
            let column_id = column.id(i);
            if let Some(existing_width) = state.col_widths.get(&column_id) {
                column.current = *existing_width;
            }
            column.current = column.range.clamp(column.current);
        }

        let parent_width = ui.available_width();
        let auto_size = match self.auto_size_mode {
            AutoSizeMode::Never => false,
            AutoSizeMode::Always => true,
            AutoSizeMode::OnParentResize => state.parent_width.map_or(true, |w| w != parent_width),
        };
        if auto_size {
            Column::auto_size(&mut self.columns, parent_width);
        }
        state.parent_width = Some(parent_width);

        let col_x = {
            let mut x = ui.cursor().min.x;
            let mut col_x = Vec1::with_capacity(x, self.columns.len() + 1);
            for column in &self.columns {
                x += column.current;
                col_x.push(x);
            }
            col_x
        };

        let sticky_row_y = {
            let mut y = ui.cursor().min.y;
            let mut sticky_row_y = Vec1::with_capacity(y, self.sticky_row_heights.len() + 1);
            for height in &self.sticky_row_heights {
                y += *height;
                sticky_row_y.push(y);
            }
            sticky_row_y
        };

        let sticky_size = Vec2::new(
            self.columns[..self.num_sticky_cols]
                .iter()
                .map(|c| c.current)
                .sum(),
            self.sticky_row_heights.iter().sum(),
        );

        ui.scope(|ui| {
            // Don't wrap text in the table cells.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            let num_columns = self.columns.len();

            SplitScroll {
                scroll_enabled: Vec2b::new(true, true),
                fixed_size: sticky_size,
                scroll_outer_size: (ui.available_size() - sticky_size).at_least(Vec2::ZERO),
                scroll_content_size: Vec2::new(
                    self.columns[self.num_sticky_cols..]
                        .iter()
                        .map(|c| c.current)
                        .sum(),
                    self.row_height * num_scroll_rows as f32,
                ),
            }
            .show(
                ui,
                &mut TableSplitScrollDelegate {
                    table_delegate,
                    state: &mut state,
                    table: &mut self,
                    col_x,
                    sticky_row_y,
                    max_column_widths: vec![0.0; num_columns],
                },
            );
        });

        state.store(ui.ctx(), id);
    }
}

struct TableSplitScrollDelegate<'a> {
    table_delegate: &'a mut dyn TableDelegate,
    table: &'a mut Table,
    state: &'a mut TableState,

    /// The x coordinate for the start of each column, plus the end of the last column.
    col_x: Vec1<f32>,

    /// The y coordinate for the start of each sticky row, plus the end of the last sticky row.
    sticky_row_y: Vec1<f32>,

    /// Actual width of the widest element in each column
    max_column_widths: Vec<f32>,
}

impl<'a> TableSplitScrollDelegate<'a> {
    fn col_idx_at(&self, x: f32) -> usize {
        self.col_x.partition_point(|&x0| x0 < x).saturating_sub(1)
    }

    fn row_idx_at(&self, y: f32) -> usize {
        if y < *self.sticky_row_y.last() {
            self.sticky_row_y
                .partition_point(|&y0| y0 < y)
                .saturating_sub(1)
        } else {
            let y = y - self.sticky_row_y.last();
            let row_nr = (y / self.table.row_height).floor() as usize;
            self.table.sticky_row_heights.len() + row_nr
        }
    }

    fn cell_rect(&self, col: usize, row: usize) -> Rect {
        let x_range = self.col_x[col]..=self.col_x[col + 1];
        let y_range = if row < self.table.sticky_row_heights.len() {
            self.sticky_row_y[row]..=self.sticky_row_y[row + 1]
        } else {
            let row = row - self.table.sticky_row_heights.len();
            let y = self.sticky_row_y.last() + row as f32 * self.table.row_height;
            y..=y + self.table.row_height
        };
        Rect::from_x_y_ranges(x_range, y_range)
    }

    fn region_ui(&mut self, ui: &mut Ui, is_lower_half: bool, offset: Vec2) {
        // Find the visible range of columns and rows:
        let viewport = ui.clip_rect().translate(offset);

        let first_col = self.col_idx_at(viewport.min.x);
        let last_col = self.col_idx_at(viewport.max.x);
        let first_row = self.row_idx_at(viewport.min.y);
        let last_row = self.row_idx_at(viewport.max.y);

        for col_nr in first_col..=last_col {
            let Some(column) = self.table.columns.get_mut(col_nr) else {
                continue;
            };
            if !column.resizable {
                continue;
            }
            let column_id = column.id(col_nr);
            let column_resize_id = column_id.with("resize");
            if let Some(response) = ui.ctx().read_response(column_resize_id) {
                if response.double_clicked() {
                    column.auto_size_this_frame = true;
                }
            }
        }

        for row_nr in first_row..=last_row {
            if self.table.num_rows <= row_nr {
                break;
            }
            for col_nr in first_col..=last_col {
                let Some(column) = self.table.columns.get(col_nr) else {
                    continue;
                };

                let mut cell_rect = self.cell_rect(col_nr, row_nr).translate(-offset);
                if column.auto_size_this_frame {
                    cell_rect.max.x = cell_rect.min.x + column.range.min;
                }

                let mut ui_builder = UiBuilder::new()
                    .max_rect(cell_rect)
                    .id_salt((row_nr, col_nr))
                    .layout(egui::Layout::left_to_right(egui::Align::Center));
                if column.auto_size_this_frame {
                    ui_builder = ui_builder.sizing_pass();
                }
                let mut cell_ui = ui.new_child(ui_builder);
                cell_ui.set_clip_rect(ui.clip_rect().intersect(cell_rect));

                self.table_delegate.cell_ui(&mut cell_ui, row_nr, col_nr);

                let width = &mut self.max_column_widths[col_nr];
                *width = width.max(cell_ui.min_size().x);
            }
        }

        if is_lower_half {
            // Resize interaction:
            for col_nr in first_col..=last_col {
                let Some(column) = self.table.columns.get(col_nr) else {
                    continue;
                };
                if !column.resizable {
                    continue;
                }
                let column_id = column.id(col_nr);
                let used_width = column.range.clamp(self.max_column_widths[col_nr]);

                let column_width = self
                    .state
                    .col_widths
                    .entry(column_id)
                    .or_insert(column.current);

                if ui.is_sizing_pass() || column.auto_size_this_frame {
                    // Shrink to fit the widest element in the column:
                    *column_width = used_width;
                } else {
                    // Grow to fit the widest element in the column:
                    *column_width = column_width.max(used_width);
                }

                let column_resize_id = column_id.with("resize");

                let x = self.col_x[col_nr + 1] - offset.x; // Right side of the column
                let mut p0 = egui::pos2(x, ui.clip_rect().top());
                let mut p1 = egui::pos2(x, ui.clip_rect().bottom());
                let line_rect = egui::Rect::from_min_max(p0, p1)
                    .expand(ui.style().interaction.resize_grab_radius_side);

                let resize_response =
                    ui.interact(line_rect, column_resize_id, egui::Sense::click_and_drag());

                if resize_response.dragged() {
                    if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                        let mut new_width = *column_width + pointer.x - x;

                        // We don't want to shrink below the size that was actually used.
                        // However, we still want to allow content that shrinks when you try
                        // to make the column less wide, so we allow some small shrinkage each frame:
                        // big enough to allow shrinking over time, small enough not to look ugly when
                        // shrinking fails. This is a bit of a HACK around immediate mode.
                        let max_shrinkage_per_frame = 8.0;
                        new_width = new_width.at_least(used_width - max_shrinkage_per_frame);

                        new_width = column.range.clamp(new_width);

                        let x = x - *column_width + new_width;
                        (p0.x, p1.x) = (x, x);

                        *column_width = new_width;
                    }
                }

                let dragging_something_else =
                    ui.input(|i| i.pointer.any_down() || i.pointer.any_pressed());
                let resize_hover = resize_response.hovered() && !dragging_something_else;

                if resize_hover || resize_response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                }

                let stroke = if resize_response.dragged() {
                    ui.style().visuals.widgets.active.bg_stroke
                } else if resize_hover {
                    ui.style().visuals.widgets.hovered.bg_stroke
                } else {
                    // ui.visuals().widgets.inactive.bg_stroke
                    ui.visuals().widgets.noninteractive.bg_stroke
                };

                ui.painter().line_segment([p0, p1], stroke);
            }
        }
    }
}

impl<'a> SplitScrollDelegate for TableSplitScrollDelegate<'a> {
    fn left_top_ui(&mut self, ui: &mut Ui) {
        self.region_ui(ui, false, Vec2::ZERO);
    }

    fn right_top_ui(&mut self, ui: &mut Ui) {
        self.region_ui(
            ui,
            false,
            vec2(ui.clip_rect().min.x - ui.min_rect().min.x, 0.0),
        );
    }

    fn left_bottom_ui(&mut self, ui: &mut Ui) {
        self.region_ui(
            ui,
            true,
            vec2(0.0, ui.clip_rect().min.y - ui.min_rect().min.y),
        );
    }

    fn right_bottom_ui(&mut self, ui: &mut Ui) {
        self.region_ui(ui, true, ui.clip_rect().min - ui.min_rect().min);
    }
}
