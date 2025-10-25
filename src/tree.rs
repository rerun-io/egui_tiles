use egui::{Id, NumExt as _, Pos2, Rect, Ui, Vec2};

use crate::behavior::EditAction;
use crate::{ContainerInsertion, ContainerKind, GridLayout, LinearDir, UiResponse};

use super::{
    Behavior, Container, DropContext, InsertionPoint, SimplificationOptions, SimplifyAction, Tile,
    TileId, Tiles,
    container::{AncestorSplitInfo, PendingLinearResize},
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

    /// Whether the tree is in floating mode, where panes are shown as floating windows.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub floating: bool,

    /// Positions of panes in floating mode.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub floating_positions: ahash::HashMap<TileId, egui::Rect>,

    /// Whether the next floating pass should explicitly reapply stored geometry.
    #[cfg_attr(feature = "serde", serde(skip))]
    floating_reset_pending: bool,

    /// Currently maximized tile, if any.
    #[cfg_attr(feature = "serde", serde(skip))]
    maximized_state: Option<MaximizedState>,

    #[cfg_attr(feature = "serde", serde(skip))]
    pending_linear_resizes: Vec<PendingLinearResize>,

    #[cfg_attr(feature = "serde", serde(skip))]
    cached_perpendicular_splits: ahash::HashMap<Id, AncestorSplitInfo>,

    #[cfg_attr(feature = "serde", serde(skip))]
    active_linear_stack: Vec<ActiveLinearInfo>,
}

#[derive(Clone, PartialEq)]
struct MaximizedState {
    tile: TileId,
    fully_maximized: bool,
    backups: Vec<ContainerBackup>,
    floating_backup: Option<FloatingBackup>,
}

#[derive(Clone, Copy, PartialEq)]
enum FloatingBackup {
    Missing,
    Rect(Rect),
}

#[derive(Clone, PartialEq)]
enum ContainerBackup {
    Linear {
        container_id: TileId,
        shares: ahash::HashMap<TileId, f32>,
    },
    Grid {
        container_id: TileId,
        col_shares: Vec<f32>,
        row_shares: Vec<f32>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActiveLinearInfo {
    pub(crate) tile_id: TileId,
    pub(crate) dir: LinearDir,
    pub(crate) visible_children: Vec<TileId>,
}

const MAXIMIZED_PRIMARY_SHARE: f32 = 10.0;
const MAXIMIZED_SECONDARY_SHARE: f32 = 0.1;

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
            floating: _,
            floating_positions: _,
            maximized_state: _,
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
            floating: false,
            floating_positions: Default::default(),
            floating_reset_pending: false,
            maximized_state: None,
            pending_linear_resizes: Vec::new(),
            cached_perpendicular_splits: Default::default(),
            active_linear_stack: Vec::new(),
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
            floating: false,
            floating_positions: Default::default(),
            floating_reset_pending: false,
            maximized_state: None,
            pending_linear_resizes: Vec::new(),
            cached_perpendicular_splits: Default::default(),
            active_linear_stack: Vec::new(),
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
        if let Some(state_tile) = self.maximized_state.as_ref().map(|s| s.tile) {
            if state_tile == id || self.is_descendant_of(state_tile, id) {
                self.clear_maximized();
            }
        }
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

    fn is_descendant_of(&self, mut tile: TileId, potential_ancestor: TileId) -> bool {
        while let Some(parent) = self.tiles.parent_of(tile) {
            if parent == potential_ancestor {
                return true;
            }
            tile = parent;
        }
        false
    }

    fn parent_chain(&self, mut tile: TileId) -> Vec<(TileId, TileId)> {
        let mut chain = Vec::new();
        while let Some(parent) = self.tiles.parent_of(tile) {
            chain.push((parent, tile));
            tile = parent;
        }
        chain
    }

    /// Visit the ancestor containers of `tile_id`, starting with its direct parent.
    ///
    /// The provided closure receives the current parent `TileId`, a mutable reference to that
    /// container, and the child `TileId` that led to it. Returning `true` from the closure stops
    /// the traversal early. Returns `true` if the traversal was stopped early.
    pub fn visit_ancestor_containers_mut(
        &mut self,
        mut tile_id: TileId,
        mut visitor: impl FnMut(TileId, &mut Container, TileId) -> bool,
    ) -> bool {
        while let Some(parent_id) = self.tiles.parent_of(tile_id) {
            let Some(tile) = self.tiles.get_mut(parent_id) else {
                tile_id = parent_id;
                continue;
            };
            let Tile::Container(container) = tile else {
                tile_id = parent_id;
                continue;
            };
            if visitor(parent_id, container, tile_id) {
                return true;
            }
            tile_id = parent_id;
        }
        false
    }

    pub fn swap_tile_in_linear_ancestors(
        &mut self,
        tile_id: TileId,
        forward: bool,
        preferred_dir: Option<LinearDir>,
    ) -> bool {
        if let Some(dir) = preferred_dir {
            if self.swap_tile_in_linear_ancestors_with_filter(tile_id, forward, Some(dir)) {
                return true;
            }
        }
        self.swap_tile_in_linear_ancestors_with_filter(tile_id, forward, None)
    }

    fn swap_tile_in_linear_ancestors_with_filter(
        &mut self,
        tile_id: TileId,
        forward: bool,
        dir_filter: Option<LinearDir>,
    ) -> bool {
        self.visit_ancestor_containers_mut(tile_id, |_, container, child_id| {
            let Container::Linear(linear) = container else {
                return false;
            };

            if let Some(required_dir) = dir_filter {
                if linear.dir != required_dir {
                    return false;
                }
            }

            let Some(index) = linear.children.iter().position(|&c| c == child_id) else {
                return false;
            };

            let neighbor_index = if forward {
                if index + 1 < linear.children.len() {
                    Some(index + 1)
                } else {
                    None
                }
            } else {
                index.checked_sub(1)
            };

            if let Some(neigh) = neighbor_index {
                linear.swap_children(index, neigh);
                true
            } else {
                false
            }
        })
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

    /// Returns the currently maximized tile, if any.
    #[inline]
    pub fn maximized(&self) -> Option<TileId> {
        self.maximized_state.as_ref().map(|state| state.tile)
    }

    /// Is any tile currently maximized?
    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.maximized_state.is_some()
    }

    /// Returns `true` if the given tile is currently maximized.
    #[inline]
    pub fn is_tile_maximized(&self, tile_id: TileId) -> bool {
        self.maximized() == Some(tile_id)
    }

    /// Clear the maximized state, if any. Returns `true` if the layout changed.
    #[inline]
    pub fn clear_maximized(&mut self) -> bool {
        if let Some(state) = self.maximized_state.take() {
            for backup in state.backups.into_iter().rev() {
                match backup {
                    ContainerBackup::Linear {
                        container_id,
                        shares,
                    } => {
                        if let Some(Tile::Container(Container::Linear(linear))) =
                            self.tiles.get_mut(container_id)
                        {
                            linear.set_shares(shares);
                        }
                    }
                    ContainerBackup::Grid {
                        container_id,
                        col_shares,
                        row_shares,
                    } => {
                        if let Some(Tile::Container(Container::Grid(grid))) =
                            self.tiles.get_mut(container_id)
                        {
                            grid.set_col_shares(col_shares);
                            grid.set_row_shares(row_shares);
                        }
                    }
                }
            }
            if let Some(floating_backup) = state.floating_backup {
                match floating_backup {
                    FloatingBackup::Rect(rect) => {
                        self.floating_positions.insert(state.tile, rect);
                    }
                    FloatingBackup::Missing => {
                        self.floating_positions.remove(&state.tile);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Maximize the given tile so it fills the entire tree.
    ///
    /// Returns `true` if the tile exists and is now maximized.
    pub fn maximize_tile(&mut self, tile_id: TileId, fully_maximize: bool) -> bool {
        if self.tiles.get(tile_id).is_none() {
            log::debug!("Ignoring maximize request for missing tile {tile_id:?}");
            return false;
        }

        if !self.tiles.is_visible(tile_id) {
            log::debug!("Ignoring maximize request for invisible tile {tile_id:?}");
            return false;
        }

        let target_full_maximize = if self.floating { true } else { fully_maximize };

        if let Some(state) = &self.maximized_state {
            if state.tile == tile_id && state.fully_maximized == target_full_maximize {
                return true;
            }
        }

        self.clear_maximized();

        // Ensure that all ancestors are activated (e.g. tab containers).
        self.make_active(|id, _| id == tile_id);

        let backups = if target_full_maximize {
            Vec::new()
        } else {
            self.apply_partial_maximize(tile_id)
        };

        let floating_backup = if self.floating {
            Some(
                if let Some(rect) = self.floating_positions.get(&tile_id).copied() {
                    FloatingBackup::Rect(rect)
                } else {
                    FloatingBackup::Missing
                },
            )
        } else {
            None
        };

        self.maximized_state = Some(MaximizedState {
            tile: tile_id,
            fully_maximized: target_full_maximize,
            backups,
            floating_backup,
        });
        true
    }

    /// Toggle maximizing the given tile. Returns `true` if the tile is maximized after the call.
    pub fn toggle_maximize(&mut self, tile_id: TileId, fully_maximize: bool) -> bool {
        if self.is_tile_maximized(tile_id) {
            self.clear_maximized();
            false
        } else {
            self.maximize_tile(tile_id, fully_maximize);
            self.is_tile_maximized(tile_id)
        }
    }

    /// Set the maximized tile.
    ///
    /// Returns `true` if the maximized state changed.
    pub fn set_maximized(&mut self, tile_id: Option<TileId>, fully_maximize: bool) -> bool {
        match tile_id {
            Some(tile_id) => {
                let prev = self.maximized();
                if self.maximize_tile(tile_id, fully_maximize) {
                    prev != Some(tile_id)
                } else {
                    false
                }
            }
            None => self.clear_maximized(),
        }
    }

    fn apply_partial_maximize(&mut self, tile_id: TileId) -> Vec<ContainerBackup> {
        let mut backups = Vec::new();
        let chain = self.parent_chain(tile_id);

        enum ParentKind {
            Linear,
            Grid { visible_slots: Vec<Option<TileId>> },
            Tabs,
        }

        for (parent_id, child_id) in chain {
            let parent_kind = match self.tiles.get(parent_id) {
                Some(Tile::Container(Container::Linear(_))) => ParentKind::Linear,
                Some(Tile::Container(Container::Grid(grid))) => ParentKind::Grid {
                    visible_slots: grid.visible_children_and_holes(&self.tiles),
                },
                Some(Tile::Container(Container::Tabs(_))) => ParentKind::Tabs,
                _ => continue,
            };

            let Some(Tile::Container(container)) = self.tiles.get_mut(parent_id) else {
                continue;
            };

            match (parent_kind, container) {
                (ParentKind::Linear, Container::Linear(linear)) => {
                    if !linear.children.iter().any(|&id| id == child_id) {
                        continue;
                    }
                    backups.push(ContainerBackup::Linear {
                        container_id: parent_id,
                        shares: linear.clone_shares(),
                    });

                    let siblings = linear.children.clone();
                    for sibling in siblings {
                        let share = if sibling == child_id {
                            MAXIMIZED_PRIMARY_SHARE
                        } else {
                            MAXIMIZED_SECONDARY_SHARE
                        };
                        linear.set_share(sibling, share);
                    }
                }
                (ParentKind::Grid { visible_slots }, Container::Grid(grid)) => {
                    let Some(slot_index) = visible_slots
                        .iter()
                        .position(|slot| slot == &Some(child_id))
                    else {
                        continue;
                    };

                    let mut num_cols = grid.col_shares.len();
                    if num_cols == 0 {
                        num_cols = match grid.layout {
                            GridLayout::Columns(cols) => cols.max(1),
                            GridLayout::Auto => visible_slots.len().max(1),
                        };
                    }
                    let num_cols = num_cols.max(1);
                    let num_rows = (visible_slots.len().max(1) + num_cols - 1) / num_cols;

                    if grid.col_shares.len() < num_cols {
                        grid.col_shares.resize(num_cols, 1.0);
                    }
                    if grid.row_shares.len() < num_rows {
                        grid.row_shares.resize(num_rows, 1.0);
                    }

                    let row = slot_index / num_cols;
                    let col = slot_index % num_cols;

                    backups.push(ContainerBackup::Grid {
                        container_id: parent_id,
                        col_shares: grid.clone_col_shares(),
                        row_shares: grid.clone_row_shares(),
                    });

                    for c in 0..grid.col_shares.len() {
                        let share = if c == col {
                            MAXIMIZED_PRIMARY_SHARE
                        } else {
                            MAXIMIZED_SECONDARY_SHARE
                        };
                        grid.set_col_share(c, share);
                    }

                    for r in 0..grid.row_shares.len() {
                        let share = if r == row {
                            MAXIMIZED_PRIMARY_SHARE
                        } else {
                            MAXIMIZED_SECONDARY_SHARE
                        };
                        grid.set_row_share(r, share);
                    }
                }
                (ParentKind::Tabs, Container::Tabs(tabs)) => {
                    tabs.set_active(child_id);
                }
                _ => {}
            }
        }
        backups
    }

    /// All visible tiles.
    ///
    /// This excludes all tiles that are invisible or are inactive tabs, recursively.
    ///
    /// The order of the returned tiles is arbitrary.
    pub fn active_tiles(&self) -> Vec<TileId> {
        let mut tiles = vec![];
        if let Some(root) = self.root {
            if self.is_visible(root) {
                self.tiles.collect_active_tiles(root, &mut tiles);
            }
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

    /// Tiles to show in floating mode.
    ///
    /// Collects panes and tab containers in tree order, keeping tabbed layouts intact.
    pub fn floating_tiles(&self) -> Vec<TileId> {
        if let Some(state) = &self.maximized_state {
            if state.fully_maximized && self.tiles.get(state.tile).is_some() {
                return vec![state.tile];
            }
        }
        let mut tiles = vec![];
        if let Some(root) = self.root {
            if self.is_visible(root) {
                self.collect_floating_tiles(root, &mut tiles, false);
            }
        }
        tiles
    }

    fn collect_floating_tiles(
        &self,
        tile_id: TileId,
        tiles: &mut Vec<TileId>,
        parent_is_tabs: bool,
    ) {
        if let Some(tile) = self.tiles.get(tile_id) {
            match tile {
                Tile::Pane(_) => {
                    if !parent_is_tabs {
                        tiles.push(tile_id);
                    }
                }
                Tile::Container(container) => {
                    let is_tabs = matches!(container, Container::Tabs(_));
                    if is_tabs && !parent_is_tabs {
                        tiles.push(tile_id);
                    }

                    let next_parent_is_tabs = parent_is_tabs || is_tabs;
                    for &child in container.active_children() {
                        self.collect_floating_tiles(child, tiles, next_parent_is_tabs);
                    }
                }
            }
        }
    }

    fn retain_floating_positions(&mut self, allowed_tiles: &ahash::HashSet<TileId>) {
        let mut removed: Vec<(TileId, egui::Rect)> = Vec::new();
        self.floating_positions.retain(|tile_id, rect| {
            if allowed_tiles.contains(tile_id) {
                true
            } else {
                removed.push((*tile_id, *rect));
                false
            }
        });

        for (tile_id, rect) in removed {
            let Some(tile) = self.tiles.get(tile_id) else {
                continue;
            };
            let Tile::Container(container) = tile else {
                continue;
            };

            let mut visible_children = container
                .active_children()
                .filter(|child| self.is_visible(**child))
                .copied();

            if let Some(single_child) = visible_children.next() {
                if visible_children.next().is_none()
                    && allowed_tiles.contains(&single_child)
                    && !self.floating_positions.contains_key(&single_child)
                {
                    self.floating_positions.insert(single_child, rect);
                }
            }
        }
    }

    /// Show the tree in the given [`Ui`].
    ///
    /// The tree will use upp all the available space - nothing more, nothing less.
    pub fn ui(&mut self, behavior: &mut dyn Behavior<Pane>, ui: &mut Ui) {
        self.simplify(&behavior.simplification_options());

        self.gc(behavior);

        if self.floating {
            self.show_floating(behavior, ui);
            return;
        }

        self.tiles.rects.clear();

        // Check if anything is being dragged:
        let mut drop_context = DropContext {
            enabled: true,
            dragged_tile_id: self.dragged_id(ui.ctx()),
            mouse_pos: ui.input(|i| i.pointer.interact_pos()),
            best_dist_sq: f32::INFINITY,
            best_insertion: None,
            preview_rect: None,
            tabs_only_dragging: false,
        };

        let mut rect = ui.available_rect_before_wrap();
        if self.height.is_finite() {
            rect.set_height(self.height);
        }
        if self.width.is_finite() {
            rect.set_width(self.width);
        }
        let mut fully_maximized_tile = None;
        if let Some((tile_id, fully_maximized)) = self
            .maximized_state
            .as_ref()
            .map(|state| (state.tile, state.fully_maximized))
        {
            if self.tiles.get(tile_id).is_some() {
                if fully_maximized {
                    fully_maximized_tile = Some(tile_id);
                }
            } else {
                self.clear_maximized();
            }
        }

        if let Some(tile_id) = fully_maximized_tile {
            self.tiles.layout_tile(ui.style(), behavior, rect, tile_id);
            self.tile_ui(behavior, &mut drop_context, ui, tile_id);
        } else if let Some(root) = self.root {
            self.tiles.layout_tile(ui.style(), behavior, rect, root);
            self.tile_ui(behavior, &mut drop_context, ui, root);
        }

        self.preview_dragged_tile(behavior, &drop_context, ui);
        ui.advance_cursor_after_rect(rect);
    }

    /// Show panes and tab containers as floating windows.
    fn show_floating(&mut self, behavior: &mut dyn Behavior<Pane>, ui: &mut Ui) {
        self.tiles.rects.clear();

        let mut drop_context = DropContext {
            enabled: false,
            dragged_tile_id: self.dragged_id(ui.ctx()),
            mouse_pos: ui.input(|i| i.pointer.interact_pos()),
            best_insertion: None,
            best_dist_sq: f32::INFINITY,
            preview_rect: None,
            tabs_only_dragging: true,
        };

        let mut available_rect = ui.available_rect_before_wrap();
        if !available_rect.is_finite() {
            available_rect = ui.ctx().content_rect();
        }
        let default_size = {
            let size = available_rect.size();
            let default_width = if size.x.is_finite() && size.x > 0.0 {
                size.x.clamp(200.0, 360.0)
            } else {
                320.0
            };
            let default_height = if size.y.is_finite() && size.y > 0.0 {
                size.y.clamp(160.0, 300.0)
            } else {
                240.0
            };
            Vec2::new(default_width, default_height)
        };

        let mut fully_maximized_tile = None;
        if let Some((tile_id, fully_maximized)) = self
            .maximized_state
            .as_ref()
            .map(|state| (state.tile, state.fully_maximized))
        {
            if self.tiles.get(tile_id).is_some() {
                if fully_maximized {
                    fully_maximized_tile = Some(tile_id);
                }
            } else {
                self.clear_maximized();
            }
        }

        let floating_tiles = self.floating_tiles();
        if drop_context.tabs_only_dragging && fully_maximized_tile.is_none() {
            let allowed_tiles: ahash::HashSet<TileId> = floating_tiles.iter().copied().collect();
            self.retain_floating_positions(&allowed_tiles);
        }

        if let Some(tile_id) = fully_maximized_tile {
            self.tiles
                .layout_tile(ui.style(), behavior, available_rect, tile_id);
            self.tile_ui(behavior, &mut drop_context, ui, tile_id);
            self.preview_dragged_tile(behavior, &drop_context, ui);
            ui.advance_cursor_after_rect(available_rect);
            return;
        }

        let mut defaults_assigned = 0usize;

        for &tile_id in &floating_tiles {
            let area = egui::Area::new(tile_id.egui_id(self.id)).constrain_to(available_rect);
            let (rect_opt, mut area) = if let Some(rect) = self.floating_positions.get(&tile_id) {
                let rect = Self::clamp_rect_to_bounds(*rect, available_rect);
                (
                    Some(rect),
                    area.current_pos(rect.min).default_size(rect.size()),
                )
            } else {
                let offset_factor = self.floating_positions.len() + defaults_assigned;
                let offset = Vec2::splat(24.0) * (offset_factor as f32);
                defaults_assigned += 1;

                let mut position = available_rect.min + offset;
                if !position.x.is_finite() {
                    position.x = 0.0;
                }
                if !position.y.is_finite() {
                    position.y = 0.0;
                }

                let rect = Self::clamp_rect_to_bounds(
                    Rect::from_min_size(position, default_size),
                    available_rect,
                );
                (
                    Some(rect),
                    area.current_pos(rect.min).default_size(rect.size()),
                )
            };
            if self.floating_reset_pending && rect_opt.is_some() {
                area = area.sizing_pass(true);
            }

            let response = area.show(ui.ctx(), |area_ui| {
                if let Some(rect) = rect_opt {
                    area_ui.set_min_size(rect.size());
                }

                let mut rect = area_ui.available_rect_before_wrap();

                // Apply inner margin equal to gap width when borders are enabled
                if behavior.floating_pane_border_enabled() {
                    let gap = behavior.gap_width(area_ui.style());
                    rect = rect.shrink(gap);
                }

                self.tiles
                    .layout_tile(area_ui.style(), behavior, rect, tile_id);

                self.tile_ui(behavior, &mut drop_context, area_ui, tile_id);

                // Add resize handle in bottom-right corner
                let resize_handle_size = 16.0;
                let handle_rect = if behavior.floating_pane_border_enabled() {
                    // Position handle relative to the full area when borders are enabled
                    area_ui.available_rect_before_wrap()
                } else {
                    rect
                };
                let resize_handle_rect = Rect::from_min_size(
                    handle_rect.max - Vec2::splat(resize_handle_size),
                    Vec2::splat(resize_handle_size),
                );

                let resize_id = area_ui.id().with("resize_handle");
                let resize_response =
                    area_ui.interact(resize_handle_rect, resize_id, egui::Sense::drag());

                // Change cursor when hovering over resize handle
                if resize_response.hovered() {
                    area_ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }

                // Paint corner hint for resize handle
                behavior.paint_corner_hint(area_ui, &resize_response, resize_handle_rect);

                // Draw border AFTER content to ensure it's visible
                if behavior.floating_pane_border_enabled() {
                    let stroke = behavior.floating_pane_border_stroke(area_ui.visuals());
                    let rounding = behavior.floating_pane_border_rounding(area_ui.visuals());
                    let border_rect = area_ui.available_rect_before_wrap();
                    area_ui.painter().rect_stroke(
                        border_rect,
                        rounding,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }

                resize_response
            });

            // Handle resizing by updating the stored position for next frame
            let resize_response = response.inner;
            if resize_response.dragged() && resize_response.drag_delta() != Vec2::ZERO {
                let delta = resize_response.drag_delta();
                let current_rect = response.response.rect;
                let mut new_size =
                    (current_rect.size() + delta).max(Vec2::splat(behavior.min_size()));
                let max_width =
                    (available_rect.max.x - current_rect.min.x).at_least(behavior.min_size());
                let max_height =
                    (available_rect.max.y - current_rect.min.y).at_least(behavior.min_size());
                new_size.x = new_size.x.min(max_width);
                new_size.y = new_size.y.min(max_height);
                let new_rect = Rect::from_min_size(current_rect.min, new_size);
                let clamped = Self::clamp_rect_to_bounds(new_rect, available_rect);
                self.floating_positions.insert(tile_id, clamped);
            } else {
                // Normal case: update with actual area rect
                let clamped = Self::clamp_rect_to_bounds(response.response.rect, available_rect);
                self.floating_positions.insert(tile_id, clamped);
            }
        }

        if self.floating_reset_pending {
            self.floating_reset_pending = false;
        }

        self.preview_dragged_tile(behavior, &drop_context, ui);
    }

    fn clamp_rect_to_bounds(rect: Rect, bounds: Rect) -> Rect {
        if !bounds.is_finite() {
            return rect;
        }

        let clamp_axis = |min: f32, length: f32, bounds_min: f32, bounds_max: f32| -> (f32, f32) {
            let bounds_len = (bounds_max - bounds_min).at_least(0.0);
            let length = length.clamp(0.0, bounds_len);
            let max_min = bounds_max - length;
            if bounds_len == 0.0 {
                (bounds_min, 0.0)
            } else {
                (min.clamp(bounds_min, max_min), length)
            }
        };

        let (min_x, width) = clamp_axis(rect.min.x, rect.width(), bounds.min.x, bounds.max.x);
        let (min_y, height) = clamp_axis(rect.min.y, rect.height(), bounds.min.y, bounds.max.y);

        Rect::from_min_size(Pos2::new(min_x, min_y), Vec2::new(width, height))
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

    /// Sets whether the tree is in floating mode.
    ///
    /// In floating mode, panes are shown as movable windows instead of tiled layout.
    /// When entering floating mode, panes are positioned at their current tiled locations.
    pub fn set_floating(&mut self, floating: bool) {
        if floating {
            self.clear_maximized();
        }

        if floating && !self.floating {
            let floating_tiles = self.floating_tiles();

            let previous_positions = std::mem::take(&mut self.floating_positions);
            let mut new_positions: ahash::HashMap<TileId, Rect> = ahash::HashMap::default();
            new_positions.reserve(floating_tiles.len());

            for tile_id in floating_tiles {
                if let Some(rect) = self.tiles.rect(tile_id) {
                    new_positions.insert(tile_id, rect);
                } else if let Some(rect) = previous_positions.get(&tile_id) {
                    new_positions.insert(tile_id, *rect);
                }
            }

            self.floating_positions = new_positions;
            self.floating_reset_pending = true;
        } else {
            self.floating_reset_pending = false;
        }
        self.floating = floating;
    }

    /// Adjust the ratio of a child in a linear container.
    ///
    /// This allows resizing tiles via keyboard or other means.
    /// `delta_ratio` is added to the child's ratio, clamped to positive values.
    pub fn adjust_linear_ratio(
        &mut self,
        linear_tile_id: TileId,
        child_index: usize,
        delta_ratio: f32,
    ) {
        if let Some(tile) = self.tiles.get_mut(linear_tile_id) {
            if let Tile::Container(Container::Linear(linear)) = tile {
                if child_index < linear.children.len() {
                    let child_id = linear.children[child_index];
                    linear.adjust_share(child_id, delta_ratio);
                }
            }
        }
    }

    pub(crate) fn enqueue_pending_linear_resize(&mut self, pending: PendingLinearResize) {
        self.pending_linear_resizes.push(pending);
    }

    pub(crate) fn cached_perpendicular_split(&mut self, id: Id) -> Option<AncestorSplitInfo> {
        if let Some(split) = self.cached_perpendicular_splits.get(&id) {
            if self.is_cached_split_valid(split) {
                return Some(split.clone());
            }
            self.cached_perpendicular_splits.remove(&id);
        }
        None
    }

    pub(crate) fn store_perpendicular_split(&mut self, id: Id, split: AncestorSplitInfo) {
        self.cached_perpendicular_splits.insert(id, split);
    }

    pub(crate) fn clear_perpendicular_split(&mut self, id: Id) {
        self.cached_perpendicular_splits.remove(&id);
    }

    pub(crate) fn active_linear_stack(&self) -> &[ActiveLinearInfo] {
        &self.active_linear_stack
    }

    fn is_cached_split_valid(&self, split: &AncestorSplitInfo) -> bool {
        let Some(Tile::Container(Container::Linear(linear))) = self.tiles.get(split.container_id)
        else {
            return false;
        };

        if linear.dir != split.dir {
            return false;
        }

        let current_visible = linear.visible_children(&self.tiles);
        if split.index + 1 >= current_visible.len() {
            return false;
        }

        current_visible == split.visible_children
    }

    fn apply_pending_linear_resizes(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        tile_id: TileId,
        tile: &mut Tile<Pane>,
    ) {
        if self.pending_linear_resizes.is_empty() {
            return;
        }

        let mut i = 0;
        while i < self.pending_linear_resizes.len() {
            if self.pending_linear_resizes[i].container_id == tile_id {
                let pending = self.pending_linear_resizes.remove(i);
                if let Tile::Container(Container::Linear(linear)) = tile {
                    pending.apply(self, behavior, linear);
                }
            } else {
                i += 1;
            }
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
        let Some(rect) = self.tiles.rect(tile_id) else {
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
            ui.id().with(tile_id),
            egui::UiBuilder::new()
                .layer_id(ui.layer_id())
                .max_rect(rect),
        );

        ui.add_enabled_ui(enabled, |ui| {
            match &mut tile {
                Tile::Pane(pane) => {
                    if behavior.pane_ui(ui, tile_id, pane) == UiResponse::DragStarted {
                        let allow_drag = if self.floating {
                            self.tiles.parent_of(tile_id).map_or(false, |parent_id| {
                                matches!(
                                    self.tiles.get(parent_id),
                                    Some(Tile::Container(Container::Tabs(_)))
                                )
                            })
                        } else {
                            true
                        };

                        if allow_drag {
                            ui.ctx().set_dragged_id(tile_id.egui_id(self.id));
                        }
                    }
                }
                Tile::Container(container) => {
                    let mut pushed_linear = false;
                    if let Container::Linear(linear) = container {
                        let visible_children = linear.visible_children(&self.tiles);
                        self.active_linear_stack.push(ActiveLinearInfo {
                            tile_id,
                            dir: linear.dir,
                            visible_children,
                        });
                        pushed_linear = true;
                    }

                    if drop_context.tabs_only_dragging
                        && !drop_context_was_enabled
                        && drop_context.dragged_tile_id != Some(tile_id)
                        && matches!(container, Container::Tabs(_))
                    {
                        drop_context.enabled = true;
                    }

                    container.ui(self, behavior, drop_context, ui, rect, tile_id);

                    if pushed_linear {
                        self.active_linear_stack.pop();
                    }
                }
            };

            behavior.paint_on_top_of_tile(ui.painter(), ui.style(), tile_id, rect);

            self.apply_pending_linear_resizes(behavior, tile_id, &mut tile);
            self.tiles.insert(tile_id, tile);
            drop_context.enabled = drop_context_was_enabled;
        });
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

        let overlay_layer_id =
            egui::LayerId::new(egui::Order::Tooltip, self.id.with("drag_overlay"));

        // Preview what is being dragged (the floating tile under the cursor):
        egui::Area::new(self.id.with((dragged_tile_id, "preview")))
            .order(egui::Order::Tooltip)
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
                .and_then(|insertion_point| self.tiles.rect(insertion_point.parent_id));

            let overlay_painter = ui.ctx().layer_painter(overlay_layer_id);
            behavior.paint_drag_preview(ui.visuals(), &overlay_painter, parent_rect, preview_rect);

            if behavior.preview_dragged_panes() {
                // TODO(emilk): add support for previewing containers too.
                if preview_rect.width() > 32.0 && preview_rect.height() > 32.0 {
                    if let Some(Tile::Pane(pane)) = self.tiles.get_mut(dragged_tile_id) {
                        // Intentionally ignore the response, since the user cannot possibly
                        // begin a drag on the preview pane.
                        let mut preview_ui = egui::Ui::new(
                            ui.ctx().clone(),
                            self.id.with((dragged_tile_id, "pane_preview_ui")),
                            egui::UiBuilder::new()
                                .layer_id(overlay_layer_id)
                                .max_rect(preview_rect),
                        );
                        let _ignored: UiResponse =
                            behavior.pane_ui(&mut preview_ui, dragged_tile_id, pane);
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
    /// Performs no simplifications.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContainerKind, Tile, Tiles};
    use ahash::HashSet;
    use egui::{Rect as EguiRect, Vec2, pos2};

    #[derive(Clone, Debug, PartialEq)]
    struct Pane;

    fn rect() -> EguiRect {
        EguiRect::from_min_size(pos2(0.0, 0.0), Vec2::new(10.0, 10.0))
    }

    #[test]
    fn floating_tiles_keep_tab_children_together() {
        let mut tiles = Tiles::default();
        let pane_a = tiles.insert_pane(Pane);
        let pane_b = tiles.insert_pane(Pane);
        let tab = tiles.insert_tab_tile(vec![pane_a, pane_b]);
        let other = tiles.insert_pane(Pane);
        let root = tiles.insert_horizontal_tile(vec![tab, other]);

        let tree = Tree::new("floating_tiles_keep_tab_children_together", root, tiles);

        assert_eq!(tree.floating_tiles(), vec![tab, other]);
    }

    #[test]
    fn floating_positions_do_not_track_tab_children() {
        let mut tiles = Tiles::default();
        let pane_a = tiles.insert_pane(Pane);
        let pane_b = tiles.insert_pane(Pane);
        let tab = tiles.insert_tab_tile(vec![pane_a, pane_b]);

        let mut tree = Tree::new("floating_positions_do_not_track_tab_children", tab, tiles);

        let saved = rect();
        tree.floating_positions.insert(pane_a, rect());
        tree.floating_positions.insert(pane_b, rect());
        tree.floating_positions.insert(tab, saved);

        let floating_tiles = tree.floating_tiles();
        let allowed: HashSet<TileId> = floating_tiles.iter().copied().collect();
        tree.retain_floating_positions(&allowed);

        assert!(tree.floating_positions.contains_key(&tab));
        assert!(!tree.floating_positions.contains_key(&pane_a));
        assert!(!tree.floating_positions.contains_key(&pane_b));
        assert_eq!(tree.floating_positions.get(&tab), Some(&saved));
    }

    #[test]
    fn floating_position_transfers_when_tabs_becomes_linear() {
        let mut tiles = Tiles::default();
        let pane = tiles.insert_pane(Pane);
        let tab = tiles.insert_tab_tile(vec![pane]);

        let mut tree = Tree::new(
            "floating_position_transfers_when_tabs_becomes_linear",
            tab,
            tiles,
        );

        let saved = rect();
        tree.floating_positions.insert(tab, saved);

        {
            let Some(Tile::Container(container)) = tree.tiles.get_mut(tab) else {
                panic!("Expected container tile");
            };
            container.set_kind(ContainerKind::Horizontal);
        }

        let floating_tiles = tree.floating_tiles();
        let allowed: HashSet<TileId> = floating_tiles.iter().copied().collect();
        tree.retain_floating_positions(&allowed);

        assert!(!tree.floating_positions.contains_key(&tab));
        assert_eq!(tree.floating_positions.get(&pane), Some(&saved));
    }
}
