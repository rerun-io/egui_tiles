use egui::{emath::Rangef, pos2, vec2, NumExt as _, Rect};
use itertools::Itertools as _;

use crate::{
    Behavior, ContainerInsertion, DropContext, InsertionPoint, ResizeState, SimplifyAction, TileId,
    Tiles, Tree,
};

/// How to lay out the children of a grid.
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
    /// The order of the children, row-major.
    ///
    /// We allow holes (for easier drag-dropping).
    /// We collapse all holes if they become too numerous.
    children: Vec<Option<TileId>>,

    /// Determines the number of columns.
    pub layout: GridLayout,

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

impl PartialEq for Grid {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            children,
            layout,
            col_shares,
            row_shares,
            col_ranges: _, // ignored because they are recomputed each frame
            row_ranges: _, // ignored because they are recomputed each frame
        } = self;

        layout == &other.layout
            && children == &other.children
            && col_shares == &other.col_shares
            && row_shares == &other.row_shares
    }
}

impl Grid {
    pub fn new(children: Vec<TileId>) -> Self {
        Self {
            children: children.into_iter().map(Some).collect(),
            ..Default::default()
        }
    }

    pub fn num_children(&self) -> usize {
        self.children().count()
    }

    pub fn children(&self) -> impl Iterator<Item = &TileId> {
        self.children.iter().filter_map(|c| c.as_ref())
    }

    pub fn add_child(&mut self, child: TileId) {
        self.children.push(Some(child));
    }

    pub fn insert_at(&mut self, index: usize, child: TileId) {
        if let Some(slot) = self.children.get_mut(index) {
            if slot.is_none() {
                // put it in the empty hole
                slot.replace(child);
            } else {
                // put it before
                log::trace!("Inserting {child:?} into Grid at {index}");
                self.children.insert(index, Some(child));
            }
        } else {
            // put it last
            log::trace!("Pushing {child:?} last in Grid");
            self.children.push(Some(child));
        }
    }

    /// Returns the child already at the given index, if any.
    #[must_use]
    pub fn replace_at(&mut self, index: usize, child: TileId) -> Option<TileId> {
        if let Some(slot) = self.children.get_mut(index) {
            slot.replace(child)
        } else {
            // put it last
            self.children.push(Some(child));
            None
        }
    }

    fn collapse_holes(&mut self) {
        log::trace!("Collaping grid holes");
        self.children.retain(|child| child.is_some());
    }

    pub(super) fn layout<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        // clean up any empty holes at the end
        while self.children.last() == Some(&None) {
            self.children.pop();
        }

        let num_visible_children = self
            .children
            .iter()
            .filter_map(|&child| child)
            .filter(|&child_id| tiles.is_visible(child_id))
            .count();

        let gap = behavior.gap_width(style);

        let num_cols = match self.layout {
            GridLayout::Auto => behavior.grid_auto_column_count(num_visible_children, rect, gap),
            GridLayout::Columns(num_columns) => num_columns.at_least(1),
        };
        let num_rows = (num_visible_children + num_cols - 1) / num_cols;

        if self.children.len() > num_cols * num_rows {
            // Too many holes
            self.collapse_holes();
        }

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

        // Layout each child:
        for (i, &child) in self.children.iter().enumerate() {
            if let Some(child) = child {
                let col = i % num_cols;
                let row = i / num_cols;
                let child_rect = Rect::from_x_y_ranges(self.col_ranges[col], self.row_ranges[row]);
                tiles.layout_tile(style, behavior, child_rect, child);
            }
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
            if let Some(child) = child {
                if tree.is_visible(child) {
                    tree.tile_ui(behavior, drop_context, ui, child);
                    crate::cover_tile_if_dragged(tree, behavior, ui, child);
                }
            }
        }

        // Register drop-zones:
        for i in 0..(self.col_ranges.len() * self.row_ranges.len()) {
            let col = i % self.col_ranges.len();
            let row = i / self.col_ranges.len();
            let child_rect = Rect::from_x_y_ranges(self.col_ranges[col], self.row_ranges[row]);
            drop_context.suggest_rect(
                InsertionPoint::new(tile_id, ContainerInsertion::Grid(i)),
                child_rect,
            );
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
        for child_opt in &mut self.children {
            if let Some(child) = *child_opt {
                match simplify(child) {
                    SimplifyAction::Remove => {
                        *child_opt = None;
                    }
                    SimplifyAction::Keep => {}
                    SimplifyAction::Replace(new) => {
                        *child_opt = Some(new);
                    }
                }
            }
        }
    }

    pub(super) fn retain(&mut self, mut retain: impl FnMut(TileId) -> bool) {
        for child_opt in &mut self.children {
            if let Some(child) = *child_opt {
                if !retain(child) {
                    *child_opt = None;
                }
            }
        }
    }

    /// Returns child index, if found.
    pub(crate) fn remove_child(&mut self, needle: TileId) -> Option<usize> {
        let index = self
            .children
            .iter()
            .position(|&child| child == Some(needle))?;
        self.children[index] = None;
        Some(index)
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
