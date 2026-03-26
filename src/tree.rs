use egui::{NumExt as _, Rect, Ui};

/// Rects closer than this (in pixels, sum of min+max distances) are considered converged.
const RECT_CONVERGENCE_THRESHOLD: f32 = 0.5;

/// User-tunable parameters for the animated drag preview.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewOptions {
    /// Whether the animated layout preview is enabled during drag-and-drop.
    ///
    /// When `false`, only a simple highlighted drop zone is shown.
    pub enabled: bool,

    /// How smooth the animation is (0..1, higher = smoother).
    ///
    /// This is the `smoothness` parameter of [`emath::exponential_smooth_factor`].
    pub smoothness: f32,

    /// The duration of the preview animation convergence (in seconds).
    ///
    /// This is the `half_time` parameter of [`emath::exponential_smooth_factor`].
    pub smooth_duration_sec: f32,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            smoothness: 0.9,
            smooth_duration_sec: 0.05,
        }
    }
}

/// Returns true if `a` and `b` are close enough to be considered the same rect.
fn rects_close_enough(a: Rect, b: Rect) -> bool {
    a.min.distance(b.min) + a.max.distance(b.max) < RECT_CONVERGENCE_THRESHOLD
}

use crate::behavior::EditAction;
use crate::{ContainerInsertion, ContainerKind, MoveJournal, UiResponse};

use super::{
    Behavior, Container, DropContext, InsertionPoint, SimplificationOptions, SimplifyAction, Tile,
    TileId, Tiles,
};

/// Transient state for the animated drag preview.
#[derive(Clone, Default)]
struct PreviewState {
    /// The best insertion point from the previous frame's drop context.
    insertion: Option<InsertionPoint>,

    /// Rects for every tile as they would be after the pending move.
    rects: ahash::HashMap<TileId, Rect>,

    lerp_t: f32,

    smoothed_rects: ahash::HashMap<TileId, Rect>,

    /// During preview, the tab children each Tabs container would have after the move.
    tab_children: ahash::HashMap<TileId, Vec<TileId>>,

    /// During preview, which tab should appear active in each Tabs container.
    active_tabs: ahash::HashMap<TileId, Option<TileId>>,
}

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
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tree<Pane> {
    /// The constant, globally unique id of this tree.
    pub(crate) id: egui::Id,

    /// None = empty tree
    pub root: Option<TileId>,

    /// All the tiles in the tree.
    pub tiles: Tiles<Pane>,

    /// When finite, this values contains the exact height of this tree
    #[cfg_attr(
        feature = "serde",
        serde(serialize_with = "serialize_f32_infinity_as_null"),
        serde(deserialize_with = "deserialize_f32_null_as_infinity")
    )]
    height: f32,

    /// When finite, this values contains the exact width of this tree
    #[cfg_attr(
        feature = "serde",
        serde(serialize_with = "serialize_f32_infinity_as_null"),
        serde(deserialize_with = "deserialize_f32_null_as_infinity")
    )]
    width: f32,

    /// Transient state for the animated drag preview.
    #[cfg_attr(feature = "serde", serde(skip))]
    preview: PreviewState,
}

// Workaround for JSON which doesn't support infinity, because JSON is stupid.
#[cfg(feature = "serde")]
fn serialize_f32_infinity_as_null<S: serde::Serializer>(
    t: &f32,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if t.is_infinite() {
        serializer.serialize_none()
    } else {
        serializer.serialize_some(t)
    }
}

#[cfg(feature = "serde")]
fn deserialize_f32_null_as_infinity<'de, D: serde::Deserializer<'de>>(
    des: D,
) -> Result<f32, D::Error> {
    use serde::Deserialize as _;
    Ok(Option::<f32>::deserialize(des)?.unwrap_or(f32::INFINITY))
}

impl<Pane: PartialEq> PartialEq for Tree<Pane> {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            id,
            root,
            tiles,
            height,
            width,
            preview: _, // transient, excluded
        } = self;

        *id == other.id
            && *root == other.root
            && *tiles == other.tiles
            && *height == other.height
            && *width == other.width
    }
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

        let Self {
            id,
            root,
            tiles,
            width,
            height,
            ..
        } = self;

        if let Some(root) = root {
            writeln!(f, "Tree {{")?;
            writeln!(f, "    id: {id:?}")?;
            writeln!(f, "    width: {width:?}")?;
            writeln!(f, "    height: {height:?}")?;
            format_tile(f, tiles, 1, *root)?;
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
            width: f32::INFINITY,
            height: f32::INFINITY,
            preview: Default::default(),
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
            width: f32::INFINITY,
            height: f32::INFINITY,
            preview: Default::default(),
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
    /// This excludes all tiles that are invisible or are inactive tabs, recursively.
    ///
    /// The order of the returned tiles is arbitrary.
    pub fn active_tiles(&self) -> Vec<TileId> {
        let mut tiles = vec![];
        if let Some(root) = self.root
            && self.is_visible(root)
        {
            self.tiles.collect_active_tiles(root, &mut tiles);
        }
        tiles
    }

    /// All non-visible tiles.
    ///
    /// This includes all tiles that are invisible or are inactive tabs. Uses `active_tiles`.
    ///
    /// The order of the returned tiles is arbitrary.
    pub fn inactive_tiles(&self) -> Vec<TileId> {
        let active_tiles = self.active_tiles();
        self.tiles
            .tile_ids()
            .filter(|id| !active_tiles.contains(id))
            .collect()
    }

    /// Show the tree in the given [`Ui`].
    ///
    /// The tree will use upp all the available space - nothing more, nothing less.
    pub fn ui(&mut self, behavior: &mut dyn Behavior<Pane>, ui: &mut Ui) {
        self.simplify(&behavior.simplification_options());

        self.gc(behavior);

        self.tiles.rects.clear();

        // Check if anything is being dragged:
        let dragged_id = self.dragged_id(ui);
        let mut drop_context = DropContext {
            enabled: true,
            dragged_tile_id: dragged_id,
            mouse_pos: ui.input(|i| i.pointer.interact_pos()),
            best_dist_sq: f32::INFINITY,
            best_insertion: None,
            preview_rect: None,
        };

        if dragged_id.is_none() {
            // smoothed_preview_rects is kept so
            // update_preview_lerp animates tiles back on cancel.
            self.preview.insertion = None;
            self.preview.rects.clear();
            self.preview.tab_children.clear();
            self.preview.active_tabs.clear();
        }

        let mut rect = ui.available_rect_before_wrap();
        if self.height.is_finite() {
            rect.set_height(self.height);
        }
        if self.width.is_finite() {
            rect.set_width(self.width);
        }

        let preview_options = behavior.preview_options();

        if preview_options.enabled {
            self.compute_preview_rects(dragged_id, behavior, ui.style(), rect);
        }

        if let Some(root) = self.root {
            self.tiles.layout_tile(ui.style(), behavior, rect, root);
        }

        self.update_preview_lerp(ui.ctx(), dragged_id, &preview_options);

        if let Some(root) = self.root {
            self.tile_ui(behavior, &mut drop_context, ui, root);
        }

        self.preview_dragged_tile(behavior, &drop_context, ui);
        ui.advance_cursor_after_rect(rect);
    }

    /// Sets the exact height that can be used by the tree.
    ///
    /// Determines the height that will be used by the tree component.
    /// By default, the tree occupies all the available space in the parent container.
    pub fn set_height(&mut self, height: f32) {
        if height.is_sign_positive() && height.is_finite() {
            self.height = height;
        } else {
            self.height = f32::INFINITY;
        }
    }

    /// Sets the exact width that can be used by the tree.
    ///
    /// Determines the width that will be used by the tree component.
    /// By default, the tree occupies all the available space in the parent container.
    pub fn set_width(&mut self, width: f32) {
        if width.is_sign_positive() && width.is_finite() {
            self.width = width;
        } else {
            self.width = f32::INFINITY;
        }
    }

    pub(super) fn tile_ui(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &Ui,
        tile_id: TileId,
    ) {
        if !self.is_visible(tile_id) {
            return;
        }
        // NOTE: important that we get the rect and tile in two steps,
        // otherwise we could loose the tile when there is no rect.
        let Some(rect) = self.display_rect(tile_id) else {
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
        // Use actual (non-animated) rect for drop zones to prevent a feedback loop
        let drop_rect = self.tiles.rect(tile_id).unwrap_or(rect);
        drop_context.on_tile(behavior, ui.style(), tile_id, drop_rect, &tile);

        // Each tile gets its own `Ui`, nested inside each other, with proper clip rectangles.
        let enabled = ui.is_enabled();
        let mut ui = egui::Ui::new(
            ui.ctx().clone(),
            ui.id().with(tile_id),
            egui::UiBuilder::new()
                .layer_id(ui.layer_id())
                .max_rect(rect),
        );

        let is_being_dragged_tile = Some(tile_id) == drop_context.dragged_tile_id;

        if is_being_dragged_tile && self.is_previewing() {
            self.tiles.insert(tile_id, tile);
            drop_context.enabled = drop_context_was_enabled;
        } else {
            ui.add_enabled_ui(enabled, |ui| {
                match &mut tile {
                    Tile::Pane(pane) => {
                        if behavior.pane_ui(ui, tile_id, pane) == UiResponse::DragStarted
                            && behavior.is_tile_draggable(&self.tiles, tile_id)
                        {
                            ui.set_dragged_id(tile_id.egui_id(self.id));
                        }
                    }
                    Tile::Container(container) => {
                        container.ui(self, behavior, drop_context, ui, rect, tile_id);
                    }
                }

                behavior.paint_on_top_of_tile(ui.painter(), ui.style(), tile_id, rect);

                self.tiles.insert(tile_id, tile);
                drop_context.enabled = drop_context_was_enabled;
            });
        }
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

        // Store for next frame's speculative layout.
        self.preview.insertion = drop_context.best_insertion;

        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

        // Preview what is being dragged:
        egui::Area::new(ui.id().with((dragged_tile_id, "preview")))
            .pivot(egui::Align2::CENTER_CENTER)
            .current_pos(mouse_pos)
            .interactable(false)
            .show(ui, |ui| {
                behavior.drag_ui(&self.tiles, ui, dragged_tile_id);
            });

        // Use the dragged tile's smoothed rect so the
        // highlight matches the animated layout
        let preview_rect = self
            .preview
            .smoothed_rects
            .get(&dragged_tile_id)
            .copied()
            .or_else(|| {
                drop_context.preview_rect.map(|r| {
                    smooth_preview_rect(ui, dragged_tile_id, r, &behavior.preview_options())
                })
            });

        if let Some(preview_rect) = preview_rect {
            let parent_rect = drop_context
                .best_insertion
                .and_then(|insertion_point| self.display_rect(insertion_point.parent_id));

            behavior.paint_drag_preview(ui.visuals(), ui.painter(), parent_rect, preview_rect);

            if behavior.preview_dragged_panes() {
                // TODO(emilk): add support for previewing containers too.
                if preview_rect.width() > 32.0
                    && preview_rect.height() > 32.0
                    && let Some(Tile::Pane(pane)) = self.tiles.get_mut(dragged_tile_id)
                {
                    // Intentionally ignore the response, since the user cannot possibly
                    // begin a drag on the preview pane.
                    let _ignored: UiResponse = behavior.pane_ui(
                        &mut ui.new_child(egui::UiBuilder::new().max_rect(preview_rect)),
                        dragged_tile_id,
                        pane,
                    );
                }
            }
        }

        if ui.input(|i| i.pointer.any_released()) {
            if let Some(insertion_point) = drop_context.best_insertion {
                behavior.on_edit(EditAction::TileDropped);
                let _journal = self.move_tile(dragged_tile_id, insertion_point, false);
            }
            clear_smooth_preview_rect(ui, dragged_tile_id);
            self.clear_preview_state();
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

            if options.all_panes_must_have_tabs
                && let Some(tile_id) = self.root
            {
                self.tiles.make_all_panes_children_of_tabs(false, tile_id);
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

            let _journal = self.move_tile(
                moved_tile_id,
                InsertionPoint {
                    parent_id: destination_container,
                    insertion: container_insertion,
                },
                reflow_grid,
            );
        } else {
            log::warn!(
                "Failed to find destination container {destination_container:?} during `move_tile_to_container()`"
            );
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
    ) -> MoveJournal {
        let mut journal = MoveJournal::new();

        log::trace!(
            "Moving {moved_tile_id:?} into {:?}",
            insertion_point.insertion
        );

        if let Some((prev_parent_id, source_index)) = self.remove_tile_id_from_parent(moved_tile_id)
        {
            // Check to see if we are moving a tile within the same container:
            if prev_parent_id == insertion_point.parent_id {
                let parent_tile = self.tiles.get_mut(prev_parent_id);

                if let Some(Tile::Container(container)) = parent_tile
                    && container.kind() == insertion_point.insertion.kind()
                {
                    let dest_index = insertion_point.insertion.index();
                    log::trace!("Moving within the same parent: {source_index} -> {dest_index}");
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
                                self.tiles
                                    .insert_at(insertion_point, moved_tile_id, &mut journal);
                            } else {
                                let dest_tile = grid.replace_at(dest_index, moved_tile_id);
                                if let Some(dest) = dest_tile {
                                    grid.insert_at(source_index, dest);
                                }
                            }
                        }
                    }
                    return journal; // done
                }
            }
        }

        // Moving to a new parent
        self.tiles
            .insert_at(insertion_point, moved_tile_id, &mut journal);

        journal
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
    /// Performs no simplifications.
    ///
    /// If found, the parent tile and the child's index is returned.
    pub(super) fn remove_tile_id_from_parent(
        &mut self,
        remove_me: TileId,
    ) -> Option<(TileId, usize)> {
        let mut result = None;

        for (parent_id, parent) in self.tiles.iter_mut() {
            if let Tile::Container(container) = parent
                && let Some(child_index) = container.remove_child(remove_me)
            {
                result = Some((*parent_id, child_index));
            }
        }

        // Make sure that if we drag away the active some tabs,
        // that the tab container gets assigned another active tab.
        // If the tab is dragged to the same container, then it will become active again,
        // since all tabs become active when dragged, wherever they end up.
        if let Some((parent_id, _)) = result
            && let Some(mut tile) = self.tiles.remove(parent_id)
        {
            if let Tile::Container(Container::Tabs(tabs)) = &mut tile {
                tabs.ensure_active(&self.tiles);
            }
            self.tiles.insert(parent_id, tile);
        }

        result
    }

    /// Exponentially smooth each tile's displayed rect toward its target.
    fn update_preview_lerp(
        &mut self,
        ctx: &egui::Context,
        dragged_id: Option<TileId>,
        options: &PreviewOptions,
    ) {
        if self.preview.rects.is_empty() && self.preview.smoothed_rects.is_empty() {
            self.preview.lerp_t = 0.0;
            return;
        }

        let dt = ctx.input(|input| input.stable_dt).at_most(0.1);
        let t = egui::emath::exponential_smooth_factor(
            options.smoothness,
            options.smooth_duration_sec,
            dt,
        );

        // Start tracking tiles that appear in preview_rects but aren't smoothed yet.
        let new_tiles: Vec<(TileId, Rect)> = self
            .preview
            .rects
            .keys()
            .filter(|id| !self.preview.smoothed_rects.contains_key(id))
            .filter_map(|&id| {
                let start = if Some(id) == dragged_id {
                    self.preview.rects.get(&id).copied()
                } else {
                    self.tiles.rect(id)
                };
                start.map(|r| (id, r))
            })
            .collect();
        for (tile_id, start) in new_tiles {
            self.preview.smoothed_rects.insert(tile_id, start);
        }

        // Smooth each tracked tile toward its target.
        let mut any_animating = false;
        #[allow(clippy::iter_over_hash_type)] // Order doesn't matter; each tile is independent.
        for (&tile_id, smoothed) in &mut self.preview.smoothed_rects {
            let target = self
                .preview
                .rects
                .get(&tile_id)
                .copied()
                .or_else(|| self.tiles.rect(tile_id));
            let Some(target) = target else { continue };

            *smoothed = smoothed.lerp_towards(&target, t);

            if rects_close_enough(*smoothed, target) {
                *smoothed = target;
            } else {
                any_animating = true;
            }
        }

        // Remove entries that have converged to their actual rect and have no preview target.
        self.preview.smoothed_rects.retain(|tile_id, smoothed| {
            if self.preview.rects.contains_key(tile_id) {
                return true;
            }
            let Some(actual) = self.tiles.rect(*tile_id) else {
                return false;
            };
            !rects_close_enough(*smoothed, actual)
        });

        if self.preview.smoothed_rects.is_empty() {
            self.preview.lerp_t = 0.0;
        } else if any_animating {
            self.preview.lerp_t = 0.5; // non-zero to gate resize handles
            ctx.request_repaint();
        } else {
            self.preview.lerp_t = 1.0;
        }
    }

    /// Speculatively apply the pending move, run simplification and layout,
    /// capture the resulting rects as preview targets, then fully restore the tree.
    fn compute_preview_rects(
        &mut self,
        dragged_id: Option<TileId>,
        behavior: &mut dyn Behavior<Pane>,
        style: &egui::Style,
        rect: Rect,
    ) {
        let (Some(dragged_id), Some(insertion)) = (dragged_id, self.preview.insertion) else {
            self.preview.rects.clear();
            return;
        };

        self.preview.rects.clear();

        // Save full state for restoration after speculative pass.
        // NOTE: must restore all state mutated by move_tile/simplify/layout_tile.
        let saved_root = self.root;
        let saved_next_tile_id = self.tiles.next_tile_id();
        let original_tile_ids: ahash::HashSet<TileId> = self.tiles.tile_ids().collect();
        let saved_containers: Vec<(TileId, Container)> = self
            .tiles
            .iter()
            .filter_map(|(&id, tile)| match tile {
                Tile::Container(c) => Some((id, c.clone())),
                Tile::Pane(_) => None,
            })
            .collect();

        let journal = self.move_tile(dragged_id, insertion, false);
        self.simplify(&behavior.simplification_options());

        if let Some(root) = self.root {
            self.tiles.layout_tile(style, behavior, rect, root);
        }

        // Grids defers hole collapse to the end of each pass. Run a
        // second pass of simplify + layout to reach the true state.
        self.simplify(&behavior.simplification_options());
        if let Some(root) = self.root {
            self.tiles.layout_tile(style, behavior, rect, root);
        }

        self.preview.rects = self.tiles.rects.clone();

        // Remap displaced tile rects to their original IDs.
        for &(original_id, displaced_to_id) in journal.displaced_tiles() {
            if let Some(rect) = self.preview.rects.remove(&displaced_to_id) {
                self.preview.rects.insert(original_id, rect);
            }
        }
        self.preview
            .rects
            .retain(|id, _| original_tile_ids.contains(id));

        // Snapshot tabs from the speculative state
        self.preview.tab_children.clear();
        self.preview.active_tabs.clear();
        let displaced_map: ahash::HashMap<TileId, TileId> = journal
            .displaced_tiles()
            .iter()
            .map(|&(orig, disp)| (disp, orig))
            .collect();

        #[allow(clippy::iter_over_hash_type)]
        for (&tile_id, tile) in self.tiles.iter() {
            if let Tile::Container(Container::Tabs(tabs)) = tile {
                let real_id = displaced_map.get(&tile_id).copied().unwrap_or(tile_id);
                let children: Vec<TileId> = tabs
                    .children
                    .iter()
                    .map(|&c| displaced_map.get(&c).copied().unwrap_or(c))
                    .collect();
                let active = tabs
                    .active
                    .map(|a| displaced_map.get(&a).copied().unwrap_or(a));
                if original_tile_ids.contains(&real_id) {
                    self.preview.tab_children.insert(real_id, children);
                    self.preview.active_tabs.insert(real_id, active);
                }
            }
        }

        // Ensure containers that were simplified away in
        // the speculative state still get entries.
        for (id, container) in &saved_containers {
            if let Container::Tabs(tabs) = container {
                if !self.preview.tab_children.contains_key(id) {
                    let children: Vec<TileId> = tabs
                        .children
                        .iter()
                        .copied()
                        .filter(|&c| c != dragged_id)
                        .collect();
                    self.preview.tab_children.insert(*id, children.clone());
                    // Pick the first remaining child as active, or keep original
                    let active = tabs
                        .active
                        .filter(|a| children.contains(a))
                        .or_else(|| children.first().copied());
                    self.preview.active_tabs.insert(*id, active);
                }
            }
        }

        // Full restore
        self.root = saved_root;
        self.tiles.set_next_tile_id(saved_next_tile_id);

        for &(original_id, displaced_to_id) in journal.displaced_tiles() {
            if let Some(tile) = self.tiles.remove(displaced_to_id) {
                self.tiles.insert(original_id, tile);
            }
        }

        for tile_id in self.tiles.tile_ids().collect::<Vec<_>>() {
            if !original_tile_ids.contains(&tile_id) {
                self.tiles.remove(tile_id);
            }
        }

        for (id, container) in saved_containers {
            self.tiles.insert(id, Tile::Container(container));
        }

        self.tiles.rects.clear();
    }

    fn clear_preview_state(&mut self) {
        self.preview = Default::default();
    }

    pub(crate) fn display_rect(&self, tile_id: TileId) -> Option<Rect> {
        let actual = self.tiles.rect(tile_id)?;
        // Use the smoothed rect if this tile is being animated
        Some(
            self.preview
                .smoothed_rects
                .get(&tile_id)
                .copied()
                .unwrap_or(actual),
        )
    }

    pub(crate) fn display_rect_or_die(&self, tile_id: TileId) -> Rect {
        self.display_rect(tile_id)
            .unwrap_or(Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO))
    }

    /// Whether tiles are currently being animated for a drag preview.
    pub(crate) fn is_previewing(&self) -> bool {
        self.preview.lerp_t != 0.0
    }

    /// Get the preview tab children for a container, if any.
    pub(crate) fn preview_tab_children(&self, tile_id: TileId) -> Option<&Vec<TileId>> {
        self.preview.tab_children.get(&tile_id)
    }

    /// Check if a child is the preview-active tab in a container.
    /// Returns `None` if there is no preview for this container.
    pub(crate) fn is_preview_active_tab(
        &self,
        container_id: TileId,
        child_id: TileId,
    ) -> Option<bool> {
        self.preview
            .active_tabs
            .get(&container_id)
            .map(|active| *active == Some(child_id))
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
fn smooth_preview_rect(
    ctx: &egui::Context,
    dragged_tile_id: TileId,
    new_rect: Rect,
    options: &PreviewOptions,
) -> Rect {
    let data_id = smooth_preview_rect_id(dragged_tile_id);

    let dt = ctx.input(|input| input.stable_dt).at_most(0.1);

    let mut requires_repaint = false;

    let smoothed = ctx.data_mut(|data| {
        let smoothed: &mut Rect = data.get_temp_mut_or(data_id, new_rect);

        let t = egui::emath::exponential_smooth_factor(
            options.smoothness,
            options.smooth_duration_sec,
            dt,
        );

        *smoothed = smoothed.lerp_towards(&new_rect, t);

        if rects_close_enough(*smoothed, new_rect) {
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
