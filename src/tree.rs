use egui::{NumExt as _, Rect, Ui};

use crate::behavior::EditAction;
use crate::{ContainerInsertion, ContainerKind, UiResponse};

use super::{
    Behavior, Container, DropContext, InsertionPoint, SimplificationOptions, SimplifyAction, Tile,
    TileId, Tiles,
};

/// The top level type. Contains all persistent state, including layouts and sizes.
///
/// You'll usually construct this once and then store it, calling [`Tree::ui`] each frame.
///
/// See [the crate-level documentation](crate) for a complete example.
///
/// ## How to construct a [`Tree`]
/// ```
/// use egui_tiles::{Tiles, TileId, Tree};
///
/// struct Pane { } // put some state here
///
/// let mut tiles = Tiles::default();
/// let tabs: Vec<TileId> = vec![tiles.insert_pane(Pane { }), tiles.insert_pane(Pane { })];
/// let root: TileId = tiles.insert_tab_tile(tabs);
///
/// let tree = Tree::new("my_tree", root, tiles);
/// ```
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tree<Pane> {
    /// The constant, globally unique id of this tree.
    pub(crate) id: egui::Id,

    /// None = empty tree
    pub root: Option<TileId>,

    /// All the tiles in the tree.
    pub tiles: Tiles<Pane>,
}

impl<Pane: std::fmt::Debug> std::fmt::Debug for Tree<Pane> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print a hierarchical view of the tree:
        fn format_tile<Pane: std::fmt::Debug>(
            f: &mut std::fmt::Formatter<'_>,
            tiles: &Tiles<Pane>,
            indent: usize,
            tile_id: TileId,
        ) -> std::fmt::Result {
            write!(f, "{} {tile_id:?}: ", "  ".repeat(indent))?;
            if let Some(tile) = tiles.get(tile_id) {
                match tile {
                    Tile::Pane(pane) => writeln!(f, "Pane {pane:?}"),
                    Tile::Container(container) => {
                        writeln!(
                            f,
                            "{}",
                            match container {
                                Container::Tabs(_) => "Tabs",
                                Container::Linear(_) => "Linear",
                                Container::Grid(_) => "Grid",
                            }
                        )?;
                        for &child in container.children() {
                            format_tile(f, tiles, indent + 1, child)?;
                        }
                        Ok(())
                    }
                }
            } else {
                writeln!(f, "DANGLING")
            }
        }

        if let Some(root) = self.root {
            writeln!(f, "Tree {{")?;
            format_tile(f, &self.tiles, 1, root)?;
            write!(f, "}}")
        } else {
            writeln!(f, "Tree {{ }}")
        }
    }
}

// ----------------------------------------------------------------------------

impl<Pane> Tree<Pane> {
    /// Construct an empty tree.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn empty(id: impl Into<egui::Id>) -> Self {
        Self {
            id: id.into(),
            root: None,
            tiles: Default::default(),
        }
    }

    /// The most flexible constructor, allowing you to set up the tiles
    /// however you want.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new(id: impl Into<egui::Id>, root: TileId, tiles: Tiles<Pane>) -> Self {
        Self {
            id: id.into(),
            root: Some(root),
            tiles,
        }
    }

    /// Create a top-level [`crate::Tabs`] container with the given panes.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new_tabs(id: impl Into<egui::Id>, panes: Vec<Pane>) -> Self {
        Self::new_container(id, ContainerKind::Tabs, panes)
    }

    /// Create a top-level horizontal [`crate::Linear`] container with the given panes.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new_horizontal(id: impl Into<egui::Id>, panes: Vec<Pane>) -> Self {
        Self::new_container(id, ContainerKind::Horizontal, panes)
    }

    /// Create a top-level vertical [`crate::Linear`] container with the given panes.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new_vertical(id: impl Into<egui::Id>, panes: Vec<Pane>) -> Self {
        Self::new_container(id, ContainerKind::Vertical, panes)
    }

    /// Create a top-level [`crate::Grid`] container with the given panes.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new_grid(id: impl Into<egui::Id>, panes: Vec<Pane>) -> Self {
        Self::new_container(id, ContainerKind::Grid, panes)
    }

    /// Create a top-level container with the given panes.
    ///
    /// The `id` must be _globally_ unique (!).
    /// This is so that the same tree can be added to different [`egui::Ui`]s (if you want).
    pub fn new_container(id: impl Into<egui::Id>, kind: ContainerKind, panes: Vec<Pane>) -> Self {
        let mut tiles = Tiles::default();
        let tile_ids = panes
            .into_iter()
            .map(|pane| tiles.insert_pane(pane))
            .collect();
        let root = tiles.insert_new(Tile::Container(Container::new(kind, tile_ids)));
        Self::new(id, root, tiles)
    }

    /// Remove the given tile and all child tiles, recursively.
    ///
    /// This also removes the tile id from the parent's list of children.
    ///
    /// All removed tiles are returned in unspecified order.
    pub fn remove_recursively(&mut self, id: TileId) -> Vec<Tile<Pane>> {
        // Remove the top-most tile_id from its parent
        self.remove_tile_id_from_parent(id);

        let mut removed_tiles = vec![];
        self.remove_recursively_impl(id, &mut removed_tiles);
        removed_tiles
    }

    fn remove_recursively_impl(&mut self, id: TileId, removed_tiles: &mut Vec<Tile<Pane>>) {
        // We can safely use the raw `tiles.remove` API here because either the parent was cleaned
        // up explicitly from `remove_recursively` or the parent is also being removed so there's
        // no reason to clean it up.
        if let Some(tile) = self.tiles.remove(id) {
            if let Tile::Container(container) = &tile {
                for &child_id in container.children() {
                    self.remove_recursively_impl(child_id, removed_tiles);
                }
            }
            removed_tiles.push(tile);
        }
    }

    /// The globally unique id used by this `Tree`.
    #[inline]
    pub fn id(&self) -> egui::Id {
        self.id
    }

    /// Check if [`Self::root`] is [`None`].
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    #[inline]
    pub fn root(&self) -> Option<TileId> {
        self.root
    }

    #[inline]
    pub fn is_root(&self, tile: TileId) -> bool {
        self.root == Some(tile)
    }

    /// Tiles are visible by default.
    ///
    /// Invisible tiles still retain their place in the tile hierarchy.
    pub fn is_visible(&self, tile_id: TileId) -> bool {
        self.tiles.is_visible(tile_id)
    }

    /// Tiles are visible by default.
    ///
    /// Invisible tiles still retain their place in the tile hierarchy.
    pub fn set_visible(&mut self, tile_id: TileId, visible: bool) {
        self.tiles.set_visible(tile_id, visible);
    }

    /// All visible tiles.
    ///
    /// This excludes all tiles that invisible or are inactive tabs, recursively.
    ///
    /// The order of the returned tiles is arbitrary.
    pub fn active_tiles(&self) -> Vec<TileId> {
        let mut tiles = vec![];
        if let Some(root) = self.root {
            if self.is_visible(root) {
                self.tiles.collect_acticve_tiles(root, &mut tiles);
            }
        }
        tiles
    }

    /// Show the tree in the given [`Ui`].
    ///
    /// The tree will use upp all the available space - nothing more, nothing less.
    pub fn ui(&mut self, behavior: &mut dyn Behavior<Pane>, ui: &mut Ui) {
        self.simplify(&behavior.simplification_options());

        self.gc(behavior);

        self.tiles.rects.clear();

        // Check if anything is being dragged:
        let mut drop_context = DropContext {
            enabled: true,
            dragged_tile_id: self.dragged_id(ui.ctx()),
            mouse_pos: ui.input(|i| i.pointer.hover_pos()),
            best_dist_sq: f32::INFINITY,
            best_insertion: None,
            preview_rect: None,
        };

        if let Some(root) = self.root {
            self.tiles
                .layout_tile(ui.style(), behavior, ui.available_rect_before_wrap(), root);

            self.tile_ui(behavior, &mut drop_context, ui, root);
        }

        self.preview_dragged_tile(behavior, &drop_context, ui);
    }

    pub(super) fn tile_ui(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &mut Ui,
        tile_id: TileId,
    ) {
        if !self.is_visible(tile_id) {
            return;
        }
        // NOTE: important that we get the rect and tile in two steps,
        // otherwise we could loose the tile when there is no rect.
        let Some(rect) = self.tiles.try_rect(tile_id) else {
            log::debug!("Failed to find rect for tile {tile_id:?} during ui");
            return;
        };
        let Some(mut tile) = self.tiles.remove(tile_id) else {
            log::debug!("Failed to find tile {tile_id:?} during ui");
            return;
        };

        let drop_context_was_enabled = drop_context.enabled;
        if Some(tile_id) == drop_context.dragged_tile_id {
            // Can't drag a tile onto self or any children
            drop_context.enabled = false;
        }
        drop_context.on_tile(behavior, ui.style(), tile_id, rect, &tile);

        // Each tile gets its own `Ui`, nested inside each other, with proper clip rectangles.
        let enabled = ui.is_enabled();
        let mut ui = egui::Ui::new(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.id().with(tile_id),
            rect,
            rect,
        );
        ui.set_enabled(enabled);
        match &mut tile {
            Tile::Pane(pane) => {
                if behavior.pane_ui(&mut ui, tile_id, pane) == UiResponse::DragStarted {
                    ui.ctx().set_dragged_id(tile_id.egui_id(self.id));
                }
            }
            Tile::Container(container) => {
                container.ui(self, behavior, drop_context, &mut ui, rect, tile_id);
            }
        };

        self.tiles.insert(tile_id, tile);
        drop_context.enabled = drop_context_was_enabled;
    }

    /// Recursively "activate" the ancestors of the tiles that matches the given predicate.
    ///
    /// This means making the matching tiles and its ancestors the active tab in any tab layout.
    ///
    /// Returns `true` if a tab was made active.
    pub fn make_active(
        &mut self,
        mut should_activate: impl FnMut(TileId, &Tile<Pane>) -> bool,
    ) -> bool {
        if let Some(root) = self.root {
            self.tiles.make_active(root, &mut should_activate)
        } else {
            false
        }
    }

    fn preview_dragged_tile(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &DropContext,
        ui: &mut Ui,
    ) {
        let (Some(mouse_pos), Some(dragged_tile_id)) =
            (drop_context.mouse_pos, drop_context.dragged_tile_id)
        else {
            return;
        };

        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

        // Preview what is being dragged:
        egui::Area::new(egui::Id::new((dragged_tile_id, "preview")))
            .pivot(egui::Align2::CENTER_CENTER)
            .current_pos(mouse_pos)
            .interactable(false)
            .show(ui.ctx(), |ui| {
                behavior.drag_ui(&self.tiles, ui, dragged_tile_id);
            });

        if let Some(preview_rect) = drop_context.preview_rect {
            let preview_rect = smooth_preview_rect(ui.ctx(), dragged_tile_id, preview_rect);

            let parent_rect = drop_context
                .best_insertion
                .and_then(|insertion_point| self.tiles.try_rect(insertion_point.parent_id));

            behavior.paint_drag_preview(ui.visuals(), ui.painter(), parent_rect, preview_rect);

            if behavior.preview_dragged_panes() {
                // TODO(emilk): add support for previewing containers too.
                if preview_rect.width() > 32.0 && preview_rect.height() > 32.0 {
                    if let Some(Tile::Pane(pane)) = self.tiles.get_mut(dragged_tile_id) {
                        let _ = behavior.pane_ui(
                            &mut ui.child_ui(preview_rect, *ui.layout()),
                            dragged_tile_id,
                            pane,
                        );
                    }
                }
            }
        }

        if ui.input(|i| i.pointer.any_released()) {
            if let Some(insertion_point) = drop_context.best_insertion {
                behavior.on_edit(EditAction::TileDropped);
                self.move_tile(dragged_tile_id, insertion_point, false);
            }
            clear_smooth_preview_rect(ui.ctx(), dragged_tile_id);
        }
    }

    /// Simplify and normalize the tree using the given options.
    ///
    /// This is also called at the start of [`Self::ui`].
    pub fn simplify(&mut self, options: &SimplificationOptions) {
        if let Some(root) = self.root {
            match self.tiles.simplify(options, root, None) {
                SimplifyAction::Keep => {}
                SimplifyAction::Remove => {
                    self.root = None;
                }
                SimplifyAction::Replace(new_root) => {
                    self.root = Some(new_root);
                }
            }

            if options.all_panes_must_have_tabs {
                if let Some(tile_id) = self.root {
                    self.tiles.make_all_panes_children_of_tabs(false, tile_id);
                }
            }
        }
    }

    /// Simplify all of the children of the given container tile recursively.
    pub fn simplify_children_of_tile(&mut self, tile_id: TileId, options: &SimplificationOptions) {
        if let Some(Tile::Container(mut container)) = self.tiles.remove(tile_id) {
            let kind = container.kind();
            container.simplify_children(|child| self.tiles.simplify(options, child, Some(kind)));
            self.tiles.insert(tile_id, Tile::Container(container));
        }
    }

    /// Garbage-collect tiles that are no longer reachable from the root tile.
    ///
    /// This is also called by [`Self::ui`], so usually you don't need to call this yourself.
    pub fn gc(&mut self, behavior: &mut dyn Behavior<Pane>) {
        self.tiles.gc_root(behavior, self.root);
    }

    /// Move a tile to a new container, at the specified insertion index.
    ///
    /// If the insertion index is greater than the current number of children, the tile is appended at the end.
    ///
    /// The grid layout needs a special treatment because it can have holes. When dragging a tile away from a grid, it
    /// leaves behind it a hole. As a result, if the tile is the dropped in the same grid, it there is no need to account
    /// for an insertion index shift (the hole can still occupy the original place of the dragged tile). However, if the
    /// tiles are reordered in a separate, linear representation of the grid (such as the Rerun blueprint tree), the
    /// expectation is that the grid is properly reordered and thus the insertion index must be shifted in case the tile
    /// is moved inside the same grid. The `reflow_grid` parameter controls this behavior.
    ///
    /// TL;DR:
    /// - when drag-and-dropping from a 2D representation of the grid, set `reflow_grid = false`
    /// - when drag-and-dropping from a 1D representation of the grid, set `reflow_grid = true`
    pub fn move_tile_to_container(
        &mut self,
        moved_tile_id: TileId,
        destination_container: TileId,
        mut insertion_index: usize,
        reflow_grid: bool,
    ) {
        // find target container
        if let Some(Tile::Container(target_container)) = self.tiles.get(destination_container) {
            let num_children = target_container.num_children();
            if insertion_index > num_children {
                insertion_index = num_children;
            }

            let container_insertion = match target_container.kind() {
                ContainerKind::Tabs => ContainerInsertion::Tabs(insertion_index),
                ContainerKind::Horizontal => ContainerInsertion::Horizontal(insertion_index),
                ContainerKind::Vertical => ContainerInsertion::Vertical(insertion_index),
                ContainerKind::Grid => ContainerInsertion::Grid(insertion_index),
            };

            self.move_tile(
                moved_tile_id,
                InsertionPoint {
                    parent_id: destination_container,
                    insertion: container_insertion,
                },
                reflow_grid,
            );
        } else {
            log::warn!("Failed to find destination container {destination_container:?} during `move_tile_to_container()`");
        }
    }

    /// Move the given tile to the given insertion point.
    ///
    /// See [`Self::move_tile_to_container()`] for details on `reflow_grid`.
    pub(super) fn move_tile(
        &mut self,
        moved_tile_id: TileId,
        insertion_point: InsertionPoint,
        reflow_grid: bool,
    ) {
        log::trace!(
            "Moving {moved_tile_id:?} into {:?}",
            insertion_point.insertion
        );

        if let Some((prev_parent_id, source_index)) = self.remove_tile_id_from_parent(moved_tile_id)
        {
            // Check to see if we are moving a tile within the same container:

            if prev_parent_id == insertion_point.parent_id {
                let parent_tile = self.tiles.get_mut(prev_parent_id);

                if let Some(Tile::Container(container)) = parent_tile {
                    if container.kind() == insertion_point.insertion.kind() {
                        let dest_index = insertion_point.insertion.index();
                        log::trace!(
                            "Moving within the same parent: {source_index} -> {dest_index}"
                        );
                        // lets swap the two indices

                        let adjusted_index = if source_index < dest_index {
                            // We removed an earlier element, so we need to adjust the index:
                            dest_index - 1
                        } else {
                            dest_index
                        };

                        match container {
                            Container::Tabs(tabs) => {
                                let insertion_index = adjusted_index.min(tabs.children.len());
                                tabs.children.insert(insertion_index, moved_tile_id);
                                tabs.active = Some(moved_tile_id);
                            }
                            Container::Linear(linear) => {
                                let insertion_index = adjusted_index.min(linear.children.len());
                                linear.children.insert(insertion_index, moved_tile_id);
                            }
                            Container::Grid(grid) => {
                                if reflow_grid {
                                    self.tiles.insert_at(insertion_point, moved_tile_id);
                                } else {
                                    let dest_tile = grid.replace_at(dest_index, moved_tile_id);
                                    if let Some(dest) = dest_tile {
                                        grid.insert_at(source_index, dest);
                                    }
                                };
                            }
                        }
                        return; // done
                    }
                }
            }
        }

        // Moving to a new parent
        self.tiles.insert_at(insertion_point, moved_tile_id);
    }

    /// Find the currently dragged tile, if any.
    pub fn dragged_id(&self, ctx: &egui::Context) -> Option<TileId> {
        for tile_id in self.tiles.tile_ids() {
            if self.is_root(tile_id) {
                continue; // not allowed to drag root
            }

            let is_tile_being_dragged = crate::is_being_dragged(ctx, self.id, tile_id);
            if is_tile_being_dragged {
                // Abort drags on escape:
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ctx.stop_dragging();
                    return None;
                }

                return Some(tile_id);
            }
        }
        None
    }

    /// This removes the given tile from the parents list of children.
    ///
    /// The [`Tile`] itself is not removed from [`Self::tiles`].
    ///
    /// Performs no simplifcations.
    ///
    /// If found, the parent tile and the child's index is returned.
    pub(super) fn remove_tile_id_from_parent(
        &mut self,
        remove_me: TileId,
    ) -> Option<(TileId, usize)> {
        let mut result = None;

        for (parent_id, parent) in self.tiles.iter_mut() {
            if let Tile::Container(container) = parent {
                if let Some(child_index) = container.remove_child(remove_me) {
                    result = Some((*parent_id, child_index));
                }
            }
        }

        // Make sure that if we drag away the active some tabs,
        // that the tab container gets assigned another active tab.
        // If the tab is dragged to the same container, then it will become active again,
        // since all tabs become active when dragged, wherever they end up.
        if let Some((parent_id, _)) = result {
            if let Some(mut tile) = self.tiles.remove(parent_id) {
                if let Tile::Container(Container::Tabs(tabs)) = &mut tile {
                    tabs.ensure_active(&self.tiles);
                }
                self.tiles.insert(parent_id, tile);
            }
        }

        result
    }
}

// ----------------------------------------------------------------------------

/// We store the preview rect in egui temp storage so that it is not serialized,
/// and so that a user could re-create the [`Tree`] each frame and still get smooth previews.
fn smooth_preview_rect_id(dragged_tile_id: TileId) -> egui::Id {
    egui::Id::new((dragged_tile_id, "smoothed_preview_rect"))
}

fn clear_smooth_preview_rect(ctx: &egui::Context, dragged_tile_id: TileId) {
    let data_id = smooth_preview_rect_id(dragged_tile_id);
    ctx.data_mut(|data| data.remove::<Rect>(data_id));
}

/// Take the preview rectangle and smooth it over time.
fn smooth_preview_rect(ctx: &egui::Context, dragged_tile_id: TileId, new_rect: Rect) -> Rect {
    let data_id = smooth_preview_rect_id(dragged_tile_id);

    let dt = ctx.input(|input| input.stable_dt).at_most(0.1);

    let mut requires_repaint = false;

    let smoothed = ctx.data_mut(|data| {
        let smoothed: &mut Rect = data.get_temp_mut_or(data_id, new_rect);

        let t = egui::emath::exponential_smooth_factor(0.9, 0.05, dt);

        *smoothed = smoothed.lerp_towards(&new_rect, t);

        let diff = smoothed.min.distance(new_rect.min) + smoothed.max.distance(new_rect.max);
        if diff < 0.5 {
            *smoothed = new_rect;
        } else {
            requires_repaint = true;
        }
        *smoothed
    });

    if requires_repaint {
        ctx.request_repaint();
    }

    smoothed
}
