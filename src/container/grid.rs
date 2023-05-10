use std::collections::{btree_map, hash_map, BTreeMap};

use egui::{emath::Rangef, pos2, vec2, NumExt as _, Rect};
use itertools::Itertools as _;

use crate::{
    Behavior, ContainerInsertion, DropContext, InsertionPoint, ResizeState, SimplifyAction, TileId,
    Tiles, Tree,
};

/// Where in a grid?
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct GridLoc {
    // Row first for sorting
    pub row: usize,
    pub col: usize,
}

impl GridLoc {
    #[inline]
    pub fn from_col_row(col: usize, row: usize) -> Self {
        Self { col, row }
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum GridLayout {
    /// Place children in a grid, with a dynamic number of columns and rows.
    /// Resizing the window may change the number of columns and rows.
    #[default]
    Auto,

    /// Place children in a grid with this many columns,
    /// and as many rows as needed.
    Columns(usize),
}

/// A grid of tiles.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Grid {
    pub children: Vec<TileId>,

    pub layout: GridLayout,

    /// Where each child is located.
    ///
    /// If a child is missing from this set, it will be assigned a location during layout.
    pub locations: nohash_hasher::IntMap<TileId, GridLoc>,

    /// Share of the available width assigned to each column.
    pub col_shares: Vec<f32>,
    /// Share of the available height assigned to each row.
    pub row_shares: Vec<f32>,

    /// ui point x ranges for each column, recomputed during layout
    #[serde(skip)]
    col_ranges: Vec<Rangef>,

    /// ui point y ranges for each row, recomputed during layout
    #[serde(skip)]
    row_ranges: Vec<Rangef>,
}

impl Grid {
    pub fn new(children: Vec<TileId>) -> Self {
        Self {
            children,
            ..Default::default()
        }
    }

    pub fn add_child(&mut self, child: TileId) {
        self.children.push(child);
    }

    pub fn layout<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        let gap = behavior.gap_width(style);
        let child_ids: nohash_hasher::IntSet<TileId> = self.children.iter().copied().collect();

        let num_cols = match self.layout {
            GridLayout::Auto => behavior.grid_auto_column_count(tiles, &self.children, rect, gap),
            GridLayout::Columns(num_columns) => num_columns.at_least(1),
        };
        let num_rows = (self.children.len() + num_cols - 1) / num_cols;

        // Where to place each tile?
        let mut tile_id_from_location: BTreeMap<GridLoc, TileId> = Default::default();
        self.locations.retain(|&child_id, &mut loc| {
            if child_ids.contains(&child_id) {
                match tile_id_from_location.entry(loc) {
                    btree_map::Entry::Occupied(_) => {
                        false // two tiles assigned to the same position - forget this one for now
                    }
                    btree_map::Entry::Vacant(entry) => {
                        if num_cols <= loc.col || num_rows <= loc.row {
                            false // out of bounds
                        } else {
                            entry.insert(child_id);
                            true
                        }
                    }
                }
            } else {
                false // child no longer exists
            }
        });

        // Find location for tiles that don't have one yet
        let mut next_pos = 0;
        for &child_id in &self.children {
            if let hash_map::Entry::Vacant(entry) = self.locations.entry(child_id) {
                // find a position:
                loop {
                    let loc = GridLoc::from_col_row(next_pos % num_cols, next_pos / num_cols);
                    if tile_id_from_location.contains_key(&loc) {
                        next_pos += 1;
                        continue;
                    }
                    entry.insert(loc);
                    tile_id_from_location.insert(loc, child_id);
                    break;
                }
            }
        }

        // Everything has a location - now we know how many rows we have:
        let num_rows = match tile_id_from_location.keys().last() {
            Some(last_loc) => last_loc.row + 1,
            None => 0,
        };

        // Figure out where each column and row goes:
        self.col_shares.resize(num_cols, 1.0);
        self.row_shares.resize(num_rows, 1.0);

        let col_widths = sizes_from_shares(&self.col_shares, rect.width(), gap);
        let row_heights = sizes_from_shares(&self.row_shares, rect.height(), gap);

        {
            let mut x = rect.left();
            self.col_ranges.clear();
            for &width in &col_widths {
                self.col_ranges.push(Rangef::new(x, x + width));
                x += width + gap;
            }
        }
        {
            let mut y = rect.top();
            self.row_ranges.clear();
            for &height in &row_heights {
                self.row_ranges.push(Rangef::new(y, y + height));
                y += height + gap;
            }
        }

        // Each child now has a location. Use this to order them, in case we will later do auto-layouts:
        self.children.sort_by_key(|&child| self.locations[&child]);

        // Place each child:
        for &child in &self.children {
            let loc = self.locations[&child];
            let child_rect =
                Rect::from_x_y_ranges(self.col_ranges[loc.col], self.row_ranges[loc.row]);
            tiles.layout_tile(style, behavior, child_rect, child);
        }
    }

    pub(super) fn ui<Pane>(
        &mut self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &mut egui::Ui,
        tile_id: TileId,
    ) {
        for &child in &self.children {
            tree.tile_ui(behavior, drop_context, ui, child);
        }

        // Register drop-zones:
        for (col, &x_range) in self.col_ranges.iter().enumerate() {
            for (row, &y_range) in self.row_ranges.iter().enumerate() {
                let cell_rect = Rect::from_x_y_ranges(x_range, y_range);
                drop_context.suggest_rect(
                    InsertionPoint::new(
                        tile_id,
                        ContainerInsertion::Grid(GridLoc::from_col_row(col, row)),
                    ),
                    cell_rect,
                );
            }
        }

        self.resize_columns(&mut tree.tiles, behavior, ui, tile_id);
        self.resize_rows(&mut tree.tiles, behavior, ui, tile_id);
    }

    fn resize_columns<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        parent_id: TileId,
    ) {
        let parent_rect = tiles.rect(parent_id);
        for (i, (left, right)) in self.col_ranges.iter().copied().tuple_windows().enumerate() {
            let resize_id = egui::Id::new((parent_id, "resize_col", i));

            let x = egui::lerp(left.max..=right.min, 0.5);

            let mut resize_state = ResizeState::Idle;
            if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                let line_rect = Rect::from_center_size(
                    pos2(x, parent_rect.center().y),
                    vec2(
                        2.0 * ui.style().interaction.resize_grab_radius_side,
                        parent_rect.height(),
                    ),
                );
                let response = ui.interact(line_rect, resize_id, egui::Sense::click_and_drag());
                resize_state = resize_interaction(
                    behavior,
                    &self.col_ranges,
                    &mut self.col_shares,
                    &response,
                    ui.painter().round_to_pixel(pointer.x) - x,
                    i,
                );

                if resize_state != ResizeState::Idle {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }

            let stroke = behavior.resize_stroke(ui.style(), resize_state);
            ui.painter().vline(x, parent_rect.y_range(), stroke);
        }
    }

    fn resize_rows<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        parent_id: TileId,
    ) {
        let parent_rect = tiles.rect(parent_id);
        for (i, (top, bottom)) in self.row_ranges.iter().copied().tuple_windows().enumerate() {
            let resize_id = egui::Id::new((parent_id, "resize_row", i));

            let y = egui::lerp(top.max..=bottom.min, 0.5);

            let mut resize_state = ResizeState::Idle;
            if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                let line_rect = Rect::from_center_size(
                    pos2(parent_rect.center().x, y),
                    vec2(
                        parent_rect.width(),
                        2.0 * ui.style().interaction.resize_grab_radius_side,
                    ),
                );
                let response = ui.interact(line_rect, resize_id, egui::Sense::click_and_drag());
                resize_state = resize_interaction(
                    behavior,
                    &self.row_ranges,
                    &mut self.row_shares,
                    &response,
                    ui.painter().round_to_pixel(pointer.y) - y,
                    i,
                );

                if resize_state != ResizeState::Idle {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
            }

            let stroke = behavior.resize_stroke(ui.style(), resize_state);
            ui.painter().hline(parent_rect.x_range(), y, stroke);
        }
    }

    pub(super) fn simplify_children(&mut self, mut simplify: impl FnMut(TileId) -> SimplifyAction) {
        self.children.retain_mut(|child| match simplify(*child) {
            SimplifyAction::Remove => false,
            SimplifyAction::Keep => true,
            SimplifyAction::Replace(new) => {
                if let Some(loc) = self.locations.remove(child) {
                    self.locations.insert(new, loc);
                }
                *child = new;
                true
            }
        });
    }
}

fn resize_interaction<Pane>(
    behavior: &mut dyn Behavior<Pane>,
    ranges: &[Rangef],
    shares: &mut [f32],
    splitter_response: &egui::Response,
    dx: f32,
    i: usize,
) -> ResizeState {
    assert_eq!(ranges.len(), shares.len());
    let num = ranges.len();
    let tile_width = |i: usize| ranges[i].span();

    let left = i;
    let right = i + 1;

    if splitter_response.double_clicked() {
        // double-click to center the split between left and right:
        let mean = 0.5 * (shares[left] + shares[right]);
        shares[left] = mean;
        shares[right] = mean;
        ResizeState::Hovering
    } else if splitter_response.dragged() {
        if dx < 0.0 {
            // Expand right, shrink stuff to the left:
            shares[right] += shrink_shares(
                behavior,
                shares,
                &(0..=i).rev().collect_vec(),
                dx.abs(),
                tile_width,
            );
        } else {
            // Expand the left, shrink stuff to the right:
            shares[left] += shrink_shares(
                behavior,
                shares,
                &(i + 1..num).collect_vec(),
                dx.abs(),
                tile_width,
            );
        }
        ResizeState::Dragging
    } else if splitter_response.hovered() {
        ResizeState::Hovering
    } else {
        ResizeState::Idle
    }
}

/// Try shrink the children by a total of `target_in_points`,
/// making sure no child gets smaller than its minimum size.
fn shrink_shares<Pane>(
    behavior: &dyn Behavior<Pane>,
    shares: &mut [f32],
    children: &[usize],
    target_in_points: f32,
    size_in_point: impl Fn(usize) -> f32,
) -> f32 {
    if children.is_empty() {
        return 0.0;
    }

    let mut total_shares = 0.0;
    let mut total_points = 0.0;
    for &child in children {
        total_shares += shares[child];
        total_points += size_in_point(child);
    }

    let shares_per_point = total_shares / total_points;

    let min_size_in_shares = shares_per_point * behavior.min_size();

    let target_in_shares = shares_per_point * target_in_points;
    let mut total_shares_lost = 0.0;

    for &child in children {
        let share = &mut shares[child];
        let spare_share = (*share - min_size_in_shares).at_least(0.0);
        let shares_needed = (target_in_shares - total_shares_lost).at_least(0.0);
        let shrink_by = f32::min(spare_share, shares_needed);

        *share -= shrink_by;
        total_shares_lost += shrink_by;
    }

    total_shares_lost
}

fn sizes_from_shares(shares: &[f32], available_size: f32, gap_width: f32) -> Vec<f32> {
    if shares.is_empty() {
        return vec![];
    }

    let available_size = available_size - gap_width * (shares.len() - 1) as f32;
    let available_size = available_size.at_least(0.0);

    let total_share: f32 = shares.iter().sum();
    if total_share <= 0.0 {
        vec![available_size / shares.len() as f32; shares.len()]
    } else {
        shares
            .iter()
            .map(|&share| share / total_share * available_size)
            .collect()
    }
}
