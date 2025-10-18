#![allow(clippy::tuple_array_conversions)]

use egui::{NumExt as _, Rect, emath::GuiRounding as _, pos2, vec2};
use itertools::Itertools as _;

use super::Container;
use crate::behavior::EditAction;
use crate::{
    Behavior, ContainerInsertion, DropContext, InsertionPoint, ResizeState, SimplifyAction, Tile,
    TileId, Tiles, Tree, is_being_dragged,
};

// ----------------------------------------------------------------------------

/// How large of a share of space each child has, on a 1D axis.
///
/// Used for [`Linear`] containers (horizontal and vertical).
///
/// Also contains the shares for currently invisible tiles.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Shares {
    /// How large of a share each child has.
    ///
    /// For instance, the shares `[1, 2, 3]` means that the first child gets 1/6 of the space,
    /// the second gets 2/6 and the third gets 3/6.
    shares: ahash::HashMap<TileId, f32>,
}

impl Shares {
    pub fn iter(&self) -> impl Iterator<Item = (&TileId, &f32)> {
        self.shares.iter()
    }

    pub fn replace_with(&mut self, remove: TileId, new: TileId) {
        if let Some(share) = self.shares.remove(&remove) {
            self.shares.insert(new, share);
        }
    }

    pub fn set_share(&mut self, id: TileId, share: f32) {
        self.shares.insert(id, share);
    }

    /// Split the given width based on the share of the children.
    pub fn split(&self, children: &[TileId], available_width: f32) -> Vec<f32> {
        let mut num_shares = 0.0;
        for &child in children {
            num_shares += self[child];
        }
        if num_shares == 0.0 {
            num_shares = 1.0;
        }
        children
            .iter()
            .map(|&child| available_width * self[child] / num_shares)
            .collect()
    }

    pub fn retain(&mut self, keep: impl Fn(TileId) -> bool) {
        self.shares.retain(|&child, _| keep(child));
    }
}

impl<'a> IntoIterator for &'a Shares {
    type Item = (&'a TileId, &'a f32);
    type IntoIter = std::collections::hash_map::Iter<'a, TileId, f32>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.shares.iter()
    }
}

impl std::ops::Index<TileId> for Shares {
    type Output = f32;

    #[inline]
    fn index(&self, id: TileId) -> &Self::Output {
        self.shares.get(&id).unwrap_or(&1.0)
    }
}

impl std::ops::IndexMut<TileId> for Shares {
    #[inline]
    fn index_mut(&mut self, id: TileId) -> &mut Self::Output {
        self.shares.entry(id).or_insert(1.0)
    }
}

// ----------------------------------------------------------------------------

/// The direction of a [`Linear`] container. Either horizontal or vertical.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum LinearDir {
    #[default]
    Horizontal,
    Vertical,
}

impl LinearDir {
    pub fn perpendicular(self) -> Self {
        match self {
            LinearDir::Horizontal => LinearDir::Vertical,
            LinearDir::Vertical => LinearDir::Horizontal,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PendingResizeAction {
    Reset,
    Drag { delta: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingLinearResize {
    pub(crate) container_id: TileId,
    pub(crate) dir: LinearDir,
    pub(crate) visible_children: Vec<TileId>,
    pub(crate) index: usize,
    pub(crate) action: PendingResizeAction,
    pub(crate) notify_edit: bool,
}

impl PendingLinearResize {
    pub(crate) fn apply<Pane>(
        &self,
        tree: &Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        linear: &mut Linear,
    ) {
        let left = self.visible_children[self.index];
        let right = self.visible_children[self.index + 1];

        let size_lookup = |tile_id: TileId| {
            let rect = tree.tiles.rect_or_die(tile_id);
            match self.dir {
                LinearDir::Horizontal => rect.width(),
                LinearDir::Vertical => rect.height(),
            }
        };

        match self.action {
            PendingResizeAction::Reset => {
                if self.notify_edit {
                    behavior.on_edit(EditAction::TileResized);
                }

                let mean = 0.5 * (linear.shares[left] + linear.shares[right]);
                linear.shares[left] = mean;
                linear.shares[right] = mean;
            }
            PendingResizeAction::Drag { delta } => {
                if self.notify_edit {
                    behavior.on_edit(EditAction::TileResized);
                }

                if delta < 0.0 {
                    let affected: Vec<TileId> = self.visible_children[..=self.index]
                        .iter()
                        .copied()
                        .rev()
                        .collect();
                    let gained = shrink_shares(
                        behavior,
                        &mut linear.shares,
                        &affected,
                        delta.abs(),
                        size_lookup,
                    );
                    linear.shares[right] += gained;
                } else {
                    let affected: Vec<TileId> = self.visible_children[self.index + 1..].to_vec();
                    let gained = shrink_shares(
                        behavior,
                        &mut linear.shares,
                        &affected,
                        delta.abs(),
                        size_lookup,
                    );
                    linear.shares[left] += gained;
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AncestorSplitInfo {
    pub(crate) container_id: TileId,
    pub(crate) dir: LinearDir,
    pub(crate) visible_children: Vec<TileId>,
    pub(crate) index: usize,
}

impl AncestorSplitInfo {
    fn into_pending(&self, action: PendingResizeAction, notify_edit: bool) -> PendingLinearResize {
        PendingLinearResize {
            container_id: self.container_id,
            dir: self.dir,
            visible_children: self.visible_children.clone(),
            index: self.index,
            action,
            notify_edit,
        }
    }
}

/// Horizontal or vertical container.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Linear {
    pub children: Vec<TileId>,
    pub dir: LinearDir,
    pub shares: Shares,
}

impl Linear {
    pub fn new(dir: LinearDir, children: Vec<TileId>) -> Self {
        Self {
            children,
            dir,
            ..Default::default()
        }
    }

    /// Adjust the share of a child by scaling, clamped to positive.
    pub fn adjust_share(&mut self, child_id: TileId, delta: f32) {
        let share = self.shares.shares.entry(child_id).or_insert(1.0);
        if delta > 0.0 {
            *share *= 1.5;
        } else if delta < 0.0 {
            *share *= 1.0 / 1.5;
        }
        *share = share.max(0.01);
    }

    /// Set the share of a child to a specific value, clamped to positive.
    pub fn set_share(&mut self, child_id: TileId, value: f32) {
        self.shares.shares.insert(child_id, value.max(0.01));
    }

    /// Transfer a share amount from one child to another, clamped to keep shares positive.
    pub fn transfer_share(&mut self, from: TileId, to: TileId, amount: f32) -> bool {
        if from == to {
            return false;
        }

        let transfer = amount.abs();
        if transfer == 0.0 {
            return false;
        }

        let delta = {
            let from_entry = self.shares.shares.entry(from).or_insert(1.0);
            let available = (*from_entry - 0.01).max(0.0);
            if available <= 0.0 {
                return false;
            }
            let delta = transfer.min(available);
            if delta <= 0.0 {
                return false;
            }
            *from_entry -= delta;
            delta
        };

        let to_entry = self.shares.shares.entry(to).or_insert(1.0);
        *to_entry += delta;
        true
    }

    /// Swap two children by index.
    pub fn swap_children(&mut self, i: usize, j: usize) {
        if i < self.children.len() && j < self.children.len() {
            self.children.swap(i, j);
        }
    }

    /// Clone the shares map.
    pub fn clone_shares(&self) -> ahash::HashMap<TileId, f32> {
        self.shares.shares.clone()
    }

    /// Set the shares map.
    pub fn set_shares(&mut self, shares: ahash::HashMap<TileId, f32>) {
        self.shares.shares = shares;
    }

    pub(crate) fn visible_children<Pane>(&self, tiles: &Tiles<Pane>) -> Vec<TileId> {
        self.children
            .iter()
            .copied()
            .filter(|&child_id| tiles.is_visible(child_id))
            .collect()
    }

    /// Create a binary split with the given split ratio in the 0.0 - 1.0 range.
    ///
    /// The `fraction` is the fraction of the total width that the first child should get.
    pub fn new_binary(dir: LinearDir, children: [TileId; 2], fraction: f32) -> Self {
        debug_assert!(
            (0.0..=1.0).contains(&fraction),
            "Fraction should be in 0.0..=1.0"
        );
        let mut slf = Self {
            children: children.into(),
            dir,
            ..Default::default()
        };
        // We multiply the shares with 2.0 because the default share size is 1.0,
        // and so we want the total share to be the same as the number of children.
        slf.shares[children[0]] = 2.0 * (fraction);
        slf.shares[children[1]] = 2.0 * (1.0 - fraction);
        slf
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
        // GC:
        let child_set: ahash::HashSet<TileId> = self.children.iter().copied().collect();
        self.shares.retain(|id| child_set.contains(&id));

        match self.dir {
            LinearDir::Horizontal => {
                self.layout_horizontal(tiles, style, behavior, rect);
            }
            LinearDir::Vertical => self.layout_vertical(tiles, style, behavior, rect),
        }
    }

    fn layout_horizontal<Pane>(
        &self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        let visible_children = self.visible_children(tiles);

        let num_gaps = visible_children.len().saturating_sub(1);
        let gap_width = behavior.gap_width(style);
        let total_gap_width = gap_width * num_gaps as f32;
        let available_width = (rect.width() - total_gap_width).at_least(0.0);

        let widths = self.shares.split(&visible_children, available_width);

        let mut x = rect.min.x;
        for (child, width) in visible_children.iter().zip(widths) {
            let child_rect = Rect::from_min_size(pos2(x, rect.min.y), vec2(width, rect.height()));
            tiles.layout_tile(style, behavior, child_rect, *child);
            x += width + gap_width;
        }
    }

    fn layout_vertical<Pane>(
        &self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        let visible_children = self.visible_children(tiles);

        let num_gaps = visible_children.len().saturating_sub(1);
        let gap_height = behavior.gap_width(style);
        let total_gap_height = gap_height * num_gaps as f32;
        let available_height = (rect.height() - total_gap_height).at_least(0.0);

        let heights = self.shares.split(&visible_children, available_height);

        let mut y = rect.min.y;
        for (child, height) in visible_children.iter().zip(heights) {
            let child_rect = Rect::from_min_size(pos2(rect.min.x, y), vec2(rect.width(), height));
            tiles.layout_tile(style, behavior, child_rect, *child);
            y += height + gap_height;
        }
    }

    pub(super) fn ui<Pane>(
        &mut self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &egui::Ui,
        tile_id: TileId,
    ) {
        match self.dir {
            LinearDir::Horizontal => self.horizontal_ui(tree, behavior, drop_context, ui, tile_id),
            LinearDir::Vertical => self.vertical_ui(tree, behavior, drop_context, ui, tile_id),
        }
    }

    fn horizontal_ui<Pane>(
        &mut self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &egui::Ui,
        parent_id: TileId,
    ) {
        let visible_children = self.visible_children(&tree.tiles);

        for &child in &visible_children {
            tree.tile_ui(behavior, drop_context, ui, child);
            crate::cover_tile_if_dragged(tree, behavior, ui, child);
        }

        linear_drop_zones(ui.ctx(), tree, &self.children, self.dir, |rect, i| {
            drop_context.suggest_rect(
                InsertionPoint::new(parent_id, ContainerInsertion::Horizontal(i)),
                rect,
            );
        });

        // ------------------------
        // resizing:

        let parent_rect = tree.tiles.rect_or_die(parent_id);
        for (i, (left, right)) in visible_children.iter().copied().tuple_windows().enumerate() {
            let resize_id = ui.id().with((parent_id, "resize", i));

            let left_rect = tree.tiles.rect_or_die(left);
            let right_rect = tree.tiles.rect_or_die(right);
            let x = egui::lerp(left_rect.right()..=right_rect.left(), 0.5);

            let mut resize_state = ResizeState::Idle;
            let line_rect = Rect::from_center_size(
                pos2(x, parent_rect.center().y),
                vec2(
                    2.0 * ui.style().interaction.resize_grab_radius_side,
                    parent_rect.height(),
                ),
            );
            let response = ui.interact(line_rect, resize_id, egui::Sense::click_and_drag());
            // NOTE: Check for interaction with line_rect BEFORE entering the 'IF block' below,
            // otherwise we miss the start of a drag event in certain cases (e.g. touchscreens).
            if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                resize_state = resize_interaction(
                    behavior,
                    &mut self.shares,
                    &visible_children,
                    &response,
                    [left, right],
                    pointer.round_to_pixels(ui.pixels_per_point()).x - x,
                    i,
                    |tile_id: TileId| tree.tiles.rect_or_die(tile_id).width(),
                    true,
                );

                if resize_state != ResizeState::Idle {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }

            let stroke = behavior.resize_stroke(ui.style(), resize_state);
            ui.painter().vline(x, parent_rect.y_range(), stroke);
        }

        if !tree.is_maximized() {
            self.resize_diagonal_corners(behavior, tree, ui, parent_id, &visible_children);
        }
    }

    fn resize_diagonal_corners<Pane>(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        tree: &mut Tree<Pane>,
        ui: &egui::Ui,
        parent_id: TileId,
        visible_children: &[TileId],
    ) {
        if visible_children.is_empty() || !behavior.allow_diagonal_resize() || tree.is_maximized() {
            return;
        }

        let handle_extent = ui.style().interaction.resize_grab_radius_corner;
        if handle_extent <= 0.0 {
            return;
        }

        let handle_size = vec2(handle_extent, handle_extent);
        let perp_dir = self.dir.perpendicular();

        for (index, &child) in visible_children.iter().enumerate() {
            let next_index = index + 1;
            if next_index >= visible_children.len() {
                continue;
            }

            let child_rect = tree.tiles.rect_or_die(child);
            let corner_rect = Rect::from_min_size(child_rect.max - handle_size, handle_size);
            if !ui.is_rect_visible(corner_rect) {
                continue;
            }

            let corner_id = ui.id().with((parent_id, child, "resize_corner"));
            let cache_id = corner_id.with("perp_split_cache");
            let response = ui.interact(corner_rect, corner_id, egui::Sense::click_and_drag());
            let anchor_pointer = corner_rect.center().round_to_pixels(ui.pixels_per_point());

            if response.hovered() || response.dragged() || response.double_clicked() {
                if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                    let pointer = pointer.round_to_pixels(ui.pixels_per_point());

                    let mut perp_split = tree.cached_perpendicular_split(cache_id);
                    if perp_split.is_none() {
                        let descendant_split =
                            find_descendant_split(tree, child, perp_dir, anchor_pointer);
                        let ancestor_split =
                            find_ancestor_split(tree, child, perp_dir, Some((self, parent_id)));
                        perp_split = choose_perpendicular_split(
                            tree,
                            descendant_split,
                            ancestor_split,
                            anchor_pointer,
                        );

                        if let Some(split) = perp_split.as_ref() {
                            if split_distance_to_point(tree, split, anchor_pointer) > handle_extent
                            {
                                perp_split = None;
                            }
                        }
                    }
                    if let Some(split) = perp_split.clone() {
                        tree.store_perpendicular_split(cache_id, split);
                    } else {
                        tree.clear_perpendicular_split(cache_id);
                    }

                    let neighbor = visible_children[next_index];
                    let neighbor_rect = tree.tiles.rect_or_die(neighbor);

                    let split_primary = match self.dir {
                        LinearDir::Horizontal => {
                            egui::lerp(child_rect.right()..=neighbor_rect.left(), 0.5)
                        }
                        LinearDir::Vertical => {
                            egui::lerp(child_rect.bottom()..=neighbor_rect.top(), 0.5)
                        }
                    };

                    let delta_primary = match self.dir {
                        LinearDir::Horizontal => pointer.x - split_primary,
                        LinearDir::Vertical => pointer.y - split_primary,
                    };

                    let size_lookup_primary = |tile_id: TileId| match self.dir {
                        LinearDir::Horizontal => tree.tiles.rect_or_die(tile_id).width(),
                        LinearDir::Vertical => tree.tiles.rect_or_die(tile_id).height(),
                    };

                    let local_children = &visible_children[index..=next_index];
                    let primary_state = resize_interaction(
                        behavior,
                        &mut self.shares,
                        local_children,
                        &response,
                        [child, neighbor],
                        delta_primary,
                        0,
                        size_lookup_primary,
                        true,
                    );

                    let primary_ancestor_split = if primary_state == ResizeState::Idle {
                        find_ancestor_split(tree, parent_id, self.dir, Some((self, parent_id)))
                    } else {
                        None
                    };

                    let ancestor_primary_state = if let Some(primary_split) =
                        primary_ancestor_split.as_ref()
                    {
                        let ancestor_child = primary_split.visible_children[primary_split.index];
                        let ancestor_neighbor =
                            primary_split.visible_children[primary_split.index + 1];
                        let ancestor_child_rect = tree.tiles.rect_or_die(ancestor_child);
                        let ancestor_neighbor_rect = tree.tiles.rect_or_die(ancestor_neighbor);

                        let split_primary_ancestor = match primary_split.dir {
                            LinearDir::Horizontal => egui::lerp(
                                ancestor_child_rect.right()..=ancestor_neighbor_rect.left(),
                                0.5,
                            ),
                            LinearDir::Vertical => egui::lerp(
                                ancestor_child_rect.bottom()..=ancestor_neighbor_rect.top(),
                                0.5,
                            ),
                        };

                        let delta_primary_ancestor = match primary_split.dir {
                            LinearDir::Horizontal => pointer.x - split_primary_ancestor,
                            LinearDir::Vertical => pointer.y - split_primary_ancestor,
                        };

                        apply_resize_for_split(
                            tree,
                            behavior,
                            primary_split,
                            &response,
                            delta_primary_ancestor,
                            true,
                        )
                    } else {
                        ResizeState::Idle
                    };

                    let secondary_state = if let Some(perp_split) = perp_split.as_ref() {
                        let secondary_child = perp_split.visible_children[perp_split.index];
                        let secondary_neighbor = perp_split.visible_children[perp_split.index + 1];
                        let secondary_child_rect = tree.tiles.rect_or_die(secondary_child);
                        let secondary_neighbor_rect = tree.tiles.rect_or_die(secondary_neighbor);

                        let split_secondary = match perp_split.dir {
                            LinearDir::Horizontal => egui::lerp(
                                secondary_child_rect.right()..=secondary_neighbor_rect.left(),
                                0.5,
                            ),
                            LinearDir::Vertical => egui::lerp(
                                secondary_child_rect.bottom()..=secondary_neighbor_rect.top(),
                                0.5,
                            ),
                        };

                        let delta_secondary = match perp_split.dir {
                            LinearDir::Horizontal => pointer.x - split_secondary,
                            LinearDir::Vertical => pointer.y - split_secondary,
                        };

                        apply_resize_for_split(
                            tree,
                            behavior,
                            perp_split,
                            &response,
                            delta_secondary,
                            primary_state == ResizeState::Idle,
                        )
                    } else {
                        ResizeState::Idle
                    };

                    let primary_combined =
                        combine_resize_states(primary_state, ancestor_primary_state);
                    let corner_state = combine_resize_states(primary_combined, secondary_state);
                    if corner_state != ResizeState::Idle {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                    }
                }
            } else {
                tree.clear_perpendicular_split(cache_id);
            }

            behavior.paint_corner_hint(ui, &response, corner_rect);
        }
    }

    fn vertical_ui<Pane>(
        &mut self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &egui::Ui,
        parent_id: TileId,
    ) {
        let visible_children = self.visible_children(&tree.tiles);

        for &child in &visible_children {
            tree.tile_ui(behavior, drop_context, ui, child);
            crate::cover_tile_if_dragged(tree, behavior, ui, child);
        }

        linear_drop_zones(ui.ctx(), tree, &self.children, self.dir, |rect, i| {
            drop_context.suggest_rect(
                InsertionPoint::new(parent_id, ContainerInsertion::Vertical(i)),
                rect,
            );
        });

        // ------------------------
        // resizing:

        let parent_rect = tree.tiles.rect_or_die(parent_id);
        for (i, (top, bottom)) in visible_children.iter().copied().tuple_windows().enumerate() {
            let resize_id = ui.id().with((parent_id, "resize", i));

            let top_rect = tree.tiles.rect_or_die(top);
            let bottom_rect = tree.tiles.rect_or_die(bottom);
            let y = egui::lerp(top_rect.bottom()..=bottom_rect.top(), 0.5);

            let mut resize_state = ResizeState::Idle;
            let line_rect = Rect::from_center_size(
                pos2(parent_rect.center().x, y),
                vec2(
                    parent_rect.width(),
                    2.0 * ui.style().interaction.resize_grab_radius_side,
                ),
            );
            let response = ui.interact(line_rect, resize_id, egui::Sense::click_and_drag());
            // NOTE: Check for interaction with line_rect BEFORE entering the 'IF block' below,
            // otherwise we miss the start of a drag event in certain cases (e.g. touchscreens).
            if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                resize_state = resize_interaction(
                    behavior,
                    &mut self.shares,
                    &visible_children,
                    &response,
                    [top, bottom],
                    pointer.round_to_pixels(ui.pixels_per_point()).y - y,
                    i,
                    |tile_id: TileId| tree.tiles.rect_or_die(tile_id).height(),
                    true,
                );

                if resize_state != ResizeState::Idle {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
            }

            let stroke = behavior.resize_stroke(ui.style(), resize_state);
            ui.painter().hline(parent_rect.x_range(), y, stroke);
        }

        if !tree.is_maximized() {
            self.resize_diagonal_corners(behavior, tree, ui, parent_id, &visible_children);
        }
    }

    pub(super) fn simplify_children(&mut self, mut simplify: impl FnMut(TileId) -> SimplifyAction) {
        self.children.retain_mut(|child| match simplify(*child) {
            SimplifyAction::Remove => false,
            SimplifyAction::Keep => true,
            SimplifyAction::Replace(new) => {
                self.shares.replace_with(*child, new);
                *child = new;
                true
            }
        });
    }

    /// Returns child index, if found.
    pub(crate) fn remove_child(&mut self, needle: TileId) -> Option<usize> {
        let index = self.children.iter().position(|&child| child == needle)?;
        self.children.remove(index);
        Some(index)
    }
}

fn find_descendant_split<Pane>(
    tree: &Tree<Pane>,
    start_tile: TileId,
    desired_dir: LinearDir,
    pointer: egui::Pos2,
) -> Option<AncestorSplitInfo> {
    let mut current_tile = start_tile;

    loop {
        let tile = tree.tiles.get(current_tile)?;
        let Tile::Container(container) = tile else {
            return None;
        };

        let Container::Linear(linear) = container else {
            return None;
        };

        let visible_children = linear.visible_children(&tree.tiles);

        if linear.dir == desired_dir {
            if let Some(index) =
                find_split_index_for_pointer(tree, &visible_children, desired_dir, pointer)
            {
                return Some(AncestorSplitInfo {
                    container_id: current_tile,
                    dir: desired_dir,
                    visible_children,
                    index,
                });
            }
        }

        let Some(next_child) = child_containing_pointer(tree, &visible_children, pointer) else {
            return None;
        };

        current_tile = next_child;
    }
}

fn find_split_index_for_pointer<Pane>(
    tree: &Tree<Pane>,
    visible_children: &[TileId],
    dir: LinearDir,
    pointer: egui::Pos2,
) -> Option<usize> {
    if visible_children.len() < 2 {
        return None;
    }

    let pointer_value = match dir {
        LinearDir::Horizontal => pointer.x,
        LinearDir::Vertical => pointer.y,
    };

    let first_rect = tree.tiles.rect_or_die(*visible_children.first().unwrap());
    let last_rect = tree.tiles.rect_or_die(*visible_children.last().unwrap());
    let (min_bound, max_bound) = match dir {
        LinearDir::Horizontal => (first_rect.min.x, last_rect.max.x),
        LinearDir::Vertical => (first_rect.min.y, last_rect.max.y),
    };

    if pointer_value < min_bound || pointer_value > max_bound {
        return None;
    }

    let mut best_index = None;
    let mut best_distance = f32::INFINITY;

    for (index, window) in visible_children.windows(2).enumerate() {
        let first_rect = tree.tiles.rect_or_die(window[0]);
        let boundary = match dir {
            LinearDir::Horizontal => first_rect.max.x,
            LinearDir::Vertical => first_rect.max.y,
        };
        let distance = (pointer_value - boundary).abs();

        if distance < best_distance {
            best_distance = distance;
            best_index = Some(index);
        }
    }

    best_index
}

fn child_containing_pointer<Pane>(
    tree: &Tree<Pane>,
    children: &[TileId],
    pointer: egui::Pos2,
) -> Option<TileId> {
    if children.is_empty() {
        return None;
    }

    for &child in children {
        let rect = tree.tiles.rect_or_die(child);
        if rect.contains(pointer) {
            return Some(child);
        }
    }

    children.iter().copied().min_by(|&a, &b| {
        let rect_a = tree.tiles.rect_or_die(a);
        let rect_b = tree.tiles.rect_or_die(b);
        distance_to_rect(pointer, rect_a)
            .partial_cmp(&distance_to_rect(pointer, rect_b))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn distance_to_rect(point: egui::Pos2, rect: egui::Rect) -> f32 {
    let dx = if point.x < rect.min.x {
        rect.min.x - point.x
    } else if point.x > rect.max.x {
        point.x - rect.max.x
    } else {
        0.0
    };

    let dy = if point.y < rect.min.y {
        rect.min.y - point.y
    } else if point.y > rect.max.y {
        point.y - rect.max.y
    } else {
        0.0
    };

    (dx * dx + dy * dy).sqrt()
}

fn ancestor_split_from_linear<Pane>(
    tree: &Tree<Pane>,
    container_id: TileId,
    linear: &Linear,
    desired_dir: LinearDir,
    child: TileId,
) -> Option<AncestorSplitInfo> {
    if linear.dir != desired_dir {
        return None;
    }
    let visible_children = linear.visible_children(&tree.tiles);
    let Some(index) = visible_children.iter().position(|&id| id == child) else {
        return None;
    };
    if index + 1 >= visible_children.len() {
        return None;
    }

    Some(AncestorSplitInfo {
        container_id,
        dir: desired_dir,
        visible_children,
        index,
    })
}

fn find_ancestor_split<Pane>(
    tree: &Tree<Pane>,
    start_child: TileId,
    desired_dir: LinearDir,
    local_container: Option<(&Linear, TileId)>,
) -> Option<AncestorSplitInfo> {
    let mut current_child = start_child;

    if let Some((linear, container_id)) = local_container {
        if let Some(split) =
            ancestor_split_from_linear(tree, container_id, linear, desired_dir, current_child)
        {
            return Some(split);
        }

        current_child = container_id;
    }

    for info in tree.active_linear_stack().iter().rev() {
        if let Some(index) = info
            .visible_children
            .iter()
            .position(|&id| id == current_child)
        {
            if index + 1 < info.visible_children.len() && info.dir == desired_dir {
                return Some(AncestorSplitInfo {
                    container_id: info.tile_id,
                    dir: info.dir,
                    visible_children: info.visible_children.clone(),
                    index,
                });
            }
            current_child = info.tile_id;
        }
    }

    while let Some(parent_id) = tree.tiles.parent_of(current_child) {
        let Some(parent_tile) = tree.tiles.get(parent_id) else {
            current_child = parent_id;
            continue;
        };
        let Tile::Container(container) = parent_tile else {
            current_child = parent_id;
            continue;
        };
        if let Container::Linear(linear) = container {
            if let Some(split) =
                ancestor_split_from_linear(tree, parent_id, linear, desired_dir, current_child)
            {
                return Some(split);
            }
        }
        current_child = parent_id;
    }
    None
}

fn choose_perpendicular_split<Pane>(
    tree: &Tree<Pane>,
    descendant: Option<AncestorSplitInfo>,
    ancestor: Option<AncestorSplitInfo>,
    anchor: egui::Pos2,
) -> Option<AncestorSplitInfo> {
    match (descendant, ancestor) {
        (Some(desc), Some(anc)) => {
            let desc_distance = split_distance_to_point(tree, &desc, anchor);
            let ancestor_distance = split_distance_to_point(tree, &anc, anchor);
            if desc_distance <= ancestor_distance {
                Some(desc)
            } else {
                Some(anc)
            }
        }
        (Some(desc), None) => Some(desc),
        (None, Some(anc)) => Some(anc),
        (None, None) => None,
    }
}

fn split_distance_to_point<Pane>(
    tree: &Tree<Pane>,
    split: &AncestorSplitInfo,
    point: egui::Pos2,
) -> f32 {
    let child_rect = tree.tiles.rect_or_die(split.visible_children[split.index]);
    match split.dir {
        LinearDir::Horizontal => (point.x - child_rect.max.x).abs(),
        LinearDir::Vertical => (point.y - child_rect.max.y).abs(),
    }
}

fn apply_resize_for_split<Pane>(
    tree: &mut Tree<Pane>,
    _behavior: &mut dyn Behavior<Pane>,
    split: &AncestorSplitInfo,
    response: &egui::Response,
    delta: f32,
    notify_edit: bool,
) -> ResizeState {
    schedule_pending_resize(tree, split, response, delta, notify_edit)
}

fn schedule_pending_resize<Pane>(
    tree: &mut Tree<Pane>,
    split: &AncestorSplitInfo,
    response: &egui::Response,
    delta: f32,
    notify_edit: bool,
) -> ResizeState {
    if response.double_clicked() {
        tree.enqueue_pending_linear_resize(
            split.into_pending(PendingResizeAction::Reset, notify_edit),
        );
        ResizeState::Hovering
    } else if response.dragged() {
        tree.enqueue_pending_linear_resize(
            split.into_pending(PendingResizeAction::Drag { delta }, notify_edit),
        );
        ResizeState::Dragging
    } else if response.hovered() {
        ResizeState::Hovering
    } else {
        ResizeState::Idle
    }
}

fn combine_resize_states(a: ResizeState, b: ResizeState) -> ResizeState {
    use ResizeState::*;

    if matches!(a, Dragging) || matches!(b, Dragging) {
        Dragging
    } else if matches!(a, Hovering) || matches!(b, Hovering) {
        Hovering
    } else {
        Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Container, Tile, Tiles};
    use egui::{Rect, pos2};

    #[test]
    fn find_descendant_split_locates_nested_linear_split() {
        let mut tiles = Tiles::default();
        let top = tiles.insert_pane(());
        let bottom = tiles.insert_pane(());
        let right = tiles.insert_pane(());

        let vertical_id = tiles.insert_container(Container::new_vertical(vec![top, bottom]));
        let horizontal_id =
            tiles.insert_container(Container::new_horizontal(vec![vertical_id, right]));

        let mut tree = Tree::new("descendant_split", horizontal_id, tiles);

        tree.tiles.rects.insert(
            horizontal_id,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 200.0)),
        );
        tree.tiles.rects.insert(
            vertical_id,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(120.0, 200.0)),
        );
        tree.tiles
            .rects
            .insert(top, Rect::from_min_max(pos2(0.0, 0.0), pos2(120.0, 100.0)));
        tree.tiles.rects.insert(
            bottom,
            Rect::from_min_max(pos2(0.0, 100.0), pos2(120.0, 200.0)),
        );
        tree.tiles.rects.insert(
            right,
            Rect::from_min_max(pos2(120.0, 0.0), pos2(200.0, 200.0)),
        );

        let pointer = pos2(118.0, 105.0);
        let split =
            find_descendant_split(&tree, vertical_id, LinearDir::Vertical, pointer).unwrap();

        assert_eq!(split.container_id, vertical_id);
        assert_eq!(split.dir, LinearDir::Vertical);
        assert_eq!(split.visible_children, vec![top, bottom]);
        assert_eq!(split.index, 0);
    }

    #[test]
    fn find_ancestor_split_remains_available() {
        let mut tiles = Tiles::default();
        let top = tiles.insert_pane(());
        let bottom = tiles.insert_pane(());
        let right = tiles.insert_pane(());

        let vertical_id = tiles.insert_container(Container::new_vertical(vec![top, bottom]));
        let horizontal_id =
            tiles.insert_container(Container::new_horizontal(vec![vertical_id, right]));

        let tree = Tree::new("ancestor_split", horizontal_id, tiles);

        let Some(Tile::Container(Container::Linear(vertical_linear))) = tree.tiles.get(vertical_id)
        else {
            panic!("missing vertical container");
        };

        let split = find_ancestor_split(
            &tree,
            top,
            LinearDir::Horizontal,
            Some((vertical_linear, vertical_id)),
        )
        .expect("expected ancestor split");

        assert_eq!(split.container_id, horizontal_id);
        assert_eq!(split.dir, LinearDir::Horizontal);
        assert_eq!(split.visible_children, vec![vertical_id, right]);
        assert_eq!(split.index, 0);
    }

    #[test]
    fn find_descendant_split_prefers_previous_child_when_pointer_on_shared_edge() {
        let mut tiles = Tiles::default();
        let top = tiles.insert_pane(());
        let middle = tiles.insert_pane(());
        let bottom = tiles.insert_pane(());

        let vertical_id =
            tiles.insert_container(Container::new_vertical(vec![top, middle, bottom]));

        let mut tree = Tree::new("descendant_split_edge_case", vertical_id, tiles);

        tree.tiles.rects.insert(
            vertical_id,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 300.0)),
        );
        tree.tiles
            .rects
            .insert(top, Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0)));
        tree.tiles.rects.insert(
            middle,
            Rect::from_min_max(pos2(0.0, 100.0), pos2(100.0, 200.0)),
        );
        tree.tiles.rects.insert(
            bottom,
            Rect::from_min_max(pos2(0.0, 200.0), pos2(100.0, 300.0)),
        );

        let pointer = pos2(50.0, 100.0);
        let split =
            find_descendant_split(&tree, vertical_id, LinearDir::Vertical, pointer).unwrap();

        assert_eq!(split.container_id, vertical_id);
        assert_eq!(split.dir, LinearDir::Vertical);
        assert_eq!(split.index, 0);
        assert_eq!(split.visible_children[split.index], top);
        assert_eq!(split.visible_children[split.index + 1], middle);
    }

    #[test]
    fn find_descendant_split_stops_when_pointer_leaves_container() {
        let mut tiles = Tiles::default();
        let top = tiles.insert_pane(());
        let bottom = tiles.insert_pane(());

        let vertical_id = tiles.insert_container(Container::new_vertical(vec![top, bottom]));

        let mut tree = Tree::new("descendant_split_outside", vertical_id, tiles);

        tree.tiles.rects.insert(
            vertical_id,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 200.0)),
        );
        tree.tiles
            .rects
            .insert(top, Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0)));
        tree.tiles.rects.insert(
            bottom,
            Rect::from_min_max(pos2(0.0, 100.0), pos2(100.0, 200.0)),
        );

        let pointer_below = pos2(50.0, 250.0);
        assert!(
            find_descendant_split(&tree, vertical_id, LinearDir::Vertical, pointer_below).is_none()
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn resize_interaction<Pane>(
    behavior: &mut dyn Behavior<Pane>,
    shares: &mut Shares,
    children: &[TileId],
    splitter_response: &egui::Response,
    [left, right]: [TileId; 2],
    dx: f32,
    i: usize,
    tile_width: impl Fn(TileId) -> f32,
    notify_edit: bool,
) -> ResizeState {
    if splitter_response.double_clicked() {
        if notify_edit {
            behavior.on_edit(EditAction::TileResized);
        }

        // double-click to center the split between left and right:
        let mean = 0.5 * (shares[left] + shares[right]);
        shares[left] = mean;
        shares[right] = mean;
        ResizeState::Hovering
    } else if splitter_response.dragged() {
        if notify_edit {
            behavior.on_edit(EditAction::TileResized);
        }

        if dx < 0.0 {
            // Expand right, shrink stuff to the left:
            shares[right] += shrink_shares(
                behavior,
                shares,
                &children[0..=i].iter().copied().rev().collect_vec(),
                dx.abs(),
                tile_width,
            );
        } else {
            // Expand the left, shrink stuff to the right:
            shares[left] +=
                shrink_shares(behavior, shares, &children[i + 1..], dx.abs(), tile_width);
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
    shares: &mut Shares,
    children: &[TileId],
    target_in_points: f32,
    size_in_point: impl Fn(TileId) -> f32,
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

fn linear_drop_zones<Pane>(
    egui_ctx: &egui::Context,
    tree: &Tree<Pane>,
    children: &[TileId],
    dir: LinearDir,
    add_drop_drect: impl FnMut(Rect, usize),
) {
    let preview_thickness = 12.0;
    let dragged_index = children
        .iter()
        .position(|&child| is_being_dragged(egui_ctx, tree.id, child));

    let after_rect = |rect: Rect| match dir {
        LinearDir::Horizontal => Rect::from_min_max(
            rect.right_top() - vec2(preview_thickness, 0.0),
            rect.right_bottom(),
        ),
        LinearDir::Vertical => Rect::from_min_max(
            rect.left_bottom() - vec2(0.0, preview_thickness),
            rect.right_bottom(),
        ),
    };

    drop_zones(
        preview_thickness,
        children,
        dragged_index,
        dir,
        |tile_id| tree.tiles.rect(tile_id),
        add_drop_drect,
        after_rect,
    );
}

/// Register drop-zones for a linear container.
///
/// `get_rect`: return `None` for invisible tiles.
pub(super) fn drop_zones(
    preview_thickness: f32,
    children: &[TileId],
    dragged_index: Option<usize>,
    dir: LinearDir,
    get_rect: impl Fn(TileId) -> Option<Rect>,
    mut add_drop_drect: impl FnMut(Rect, usize),
    after_rect: impl Fn(Rect) -> Rect,
) {
    let before_rect = |rect: Rect| match dir {
        LinearDir::Horizontal => Rect::from_min_max(
            rect.left_top(),
            rect.left_bottom() + vec2(preview_thickness, 0.0),
        ),
        LinearDir::Vertical => Rect::from_min_max(
            rect.left_top(),
            rect.right_top() + vec2(0.0, preview_thickness),
        ),
    };
    let between_rects = |a: Rect, b: Rect| match dir {
        LinearDir::Horizontal => Rect::from_center_size(
            a.right_center().lerp(b.left_center(), 0.5),
            vec2(preview_thickness, a.height()),
        ),
        LinearDir::Vertical => Rect::from_center_size(
            a.center_bottom().lerp(b.center_top(), 0.5),
            vec2(a.width(), preview_thickness),
        ),
    };

    let mut prev_rect: Option<Rect> = None;

    for (i, &child) in children.iter().enumerate() {
        let Some(rect) = get_rect(child) else {
            // skip invisible child
            continue;
        };

        if Some(i) == dragged_index {
            // Suggest hole as a drop-target:
            add_drop_drect(rect, i);
        } else if let Some(prev_rect) = prev_rect {
            if Some(i - 1) != dragged_index {
                // Suggest dropping between the rects:
                add_drop_drect(between_rects(prev_rect, rect), i);
            }
        } else {
            // Suggest dropping before the first child:
            add_drop_drect(before_rect(rect), 0);
        }

        prev_rect = Some(rect);
    }

    if let Some(last_rect) = prev_rect {
        // Suggest dropping after the last child (unless that's the one being dragged):
        if dragged_index != Some(children.len() - 1) {
            add_drop_drect(after_rect(last_rect), children.len());
        }
    }
}
