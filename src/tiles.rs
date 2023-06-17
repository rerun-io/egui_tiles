use egui::{Pos2, Rect};

use super::{
    Behavior, Container, ContainerInsertion, ContainerKind, GcAction, Grid, InsertionPoint, Linear,
    LinearDir, SimplificationOptions, SimplifyAction, Tabs, Tile, TileId,
};

/// Contains all tile state, but no root.
///
/// ```
/// use egui_tiles::{Tiles, TileId, Tree};
///
/// struct Pane { } // put some state here
///
/// let mut tiles = Tiles::default();
/// let tabs: Vec<TileId> = vec![tiles.insert_pane(Pane { }), tiles.insert_pane(Pane { })];
/// let root: TileId = tiles.insert_tab_tile(tabs);
///
/// let tree = Tree::new(root, tiles);
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tiles<Pane> {
    tiles: nohash_hasher::IntMap<TileId, Tile<Pane>>,

    /// Tiles are visible by default, so we only store the invisible ones.
    invisible: nohash_hasher::IntSet<TileId>,

    /// Filled in by the layout step at the start of each frame.
    #[serde(default, skip)]
    pub(super) rects: nohash_hasher::IntMap<TileId, Rect>,
}

impl<Pane> Default for Tiles<Pane> {
    fn default() -> Self {
        Self {
            tiles: Default::default(),
            invisible: Default::default(),
            rects: Default::default(),
        }
    }
}

// ----------------------------------------------------------------------------

impl<Pane> Tiles<Pane> {
    pub(super) fn try_rect(&self, tile_id: TileId) -> Option<Rect> {
        if self.is_visible(tile_id) {
            self.rects.get(&tile_id).copied()
        } else {
            None
        }
    }

    pub(super) fn rect(&self, tile_id: TileId) -> Rect {
        let rect = self.try_rect(tile_id);
        debug_assert!(rect.is_some(), "Failed to find rect for {tile_id:?}");
        rect.unwrap_or(egui::Rect::from_min_max(Pos2::ZERO, Pos2::ZERO))
    }

    pub fn get(&self, tile_id: TileId) -> Option<&Tile<Pane>> {
        self.tiles.get(&tile_id)
    }

    pub fn get_mut(&mut self, tile_id: TileId) -> Option<&mut Tile<Pane>> {
        self.tiles.get_mut(&tile_id)
    }

    /// All tiles, in arbitrary order
    pub fn iter(&self) -> impl Iterator<Item = (&TileId, &Tile<Pane>)> + '_ {
        self.tiles.iter()
    }

    /// All [`TileId`]s, in arbitrary order
    pub fn tile_ids(&self) -> impl Iterator<Item = TileId> + '_ {
        self.tiles.keys().copied()
    }

    /// All [`Tile`]s in arbitrary order
    pub fn tiles(&self) -> impl Iterator<Item = &Tile<Pane>> + '_ {
        self.tiles.values()
    }

    /// All [`Tile`]s in arbitrary order
    pub fn tiles_mut(&mut self) -> impl Iterator<Item = &mut Tile<Pane>> + '_ {
        self.tiles.values_mut()
    }

    /// Tiles are visible by default.
    ///
    /// Invisible tiles still retain their place in the tile hierarchy.
    pub fn is_visible(&self, tile_id: TileId) -> bool {
        !self.invisible.contains(&tile_id)
    }

    /// Tiles are visible by default.
    ///
    /// Invisible tiles still retain their place in the tile hierarchy.
    pub fn set_visible(&mut self, tile_id: TileId, visible: bool) {
        if visible {
            self.invisible.remove(&tile_id);
        } else {
            self.invisible.insert(tile_id);
        }
    }

    pub fn insert(&mut self, id: TileId, tile: Tile<Pane>) {
        self.tiles.insert(id, tile);
    }

    pub fn remove(&mut self, id: TileId) -> Option<Tile<Pane>> {
        self.tiles.remove(&id)
    }

    #[must_use]
    pub fn insert_tile(&mut self, tile: Tile<Pane>) -> TileId {
        let id = TileId::random();
        self.tiles.insert(id, tile);
        id
    }

    #[must_use]
    pub fn insert_pane(&mut self, pane: Pane) -> TileId {
        self.insert_tile(Tile::Pane(pane))
    }

    #[must_use]
    pub fn insert_container(&mut self, container: impl Into<Container>) -> TileId {
        self.insert_tile(Tile::Container(container.into()))
    }

    #[must_use]
    pub fn insert_tab_tile(&mut self, children: Vec<TileId>) -> TileId {
        self.insert_tile(Tile::Container(Container::new_tabs(children)))
    }

    #[must_use]
    pub fn insert_horizontal_tile(&mut self, children: Vec<TileId>) -> TileId {
        self.insert_tile(Tile::Container(Container::new_linear(
            LinearDir::Horizontal,
            children,
        )))
    }

    #[must_use]
    pub fn insert_vertical_tile(&mut self, children: Vec<TileId>) -> TileId {
        self.insert_tile(Tile::Container(Container::new_linear(
            LinearDir::Vertical,
            children,
        )))
    }

    #[must_use]
    pub fn insert_grid_tile(&mut self, children: Vec<TileId>) -> TileId {
        self.insert_tile(Tile::Container(Container::new_grid(children)))
    }

    pub fn parent_of(&self, child_id: TileId) -> Option<TileId> {
        for (tile_id, tile) in &self.tiles {
            if let Tile::Container(container) = tile {
                if container.children().contains(&child_id) {
                    return Some(*tile_id);
                }
            }
        }
        None
    }

    pub fn is_root(&self, tile_id: TileId) -> bool {
        self.parent_of(tile_id).is_none()
    }

    pub(super) fn insert_at(&mut self, insertion_point: InsertionPoint, child_id: TileId) {
        let InsertionPoint {
            parent_id,
            insertion,
        } = insertion_point;

        let Some(mut tile) = self.tiles.remove(&parent_id) else {
            log::warn!("Failed to insert: could not find parent {parent_id:?}");
            return;
        };

        match insertion {
            ContainerInsertion::Tabs(index) => {
                if let Tile::Container(Container::Tabs(tabs)) = &mut tile {
                    let index = index.min(tabs.children.len());
                    tabs.children.insert(index, child_id);
                    tabs.set_active(child_id);
                    self.tiles.insert(parent_id, tile);
                } else {
                    let new_tile_id = self.insert_tile(tile);
                    let mut tabs = Tabs::new(vec![new_tile_id]);
                    tabs.children.insert(index.min(1), child_id);
                    tabs.set_active(child_id);
                    self.tiles
                        .insert(parent_id, Tile::Container(Container::Tabs(tabs)));
                }
            }
            ContainerInsertion::Horizontal(index) => {
                if let Tile::Container(Container::Linear(Linear {
                    dir: LinearDir::Horizontal,
                    children,
                    ..
                })) = &mut tile
                {
                    let index = index.min(children.len());
                    children.insert(index, child_id);
                    self.tiles.insert(parent_id, tile);
                } else {
                    let new_tile_id = self.insert_tile(tile);
                    let mut linear = Linear::new(LinearDir::Horizontal, vec![new_tile_id]);
                    linear.children.insert(index.min(1), child_id);
                    self.tiles
                        .insert(parent_id, Tile::Container(Container::Linear(linear)));
                }
            }
            ContainerInsertion::Vertical(index) => {
                if let Tile::Container(Container::Linear(Linear {
                    dir: LinearDir::Vertical,
                    children,
                    ..
                })) = &mut tile
                {
                    let index = index.min(children.len());
                    children.insert(index, child_id);
                    self.tiles.insert(parent_id, tile);
                } else {
                    let new_tile_id = self.insert_tile(tile);
                    let mut linear = Linear::new(LinearDir::Vertical, vec![new_tile_id]);
                    linear.children.insert(index.min(1), child_id);
                    self.tiles
                        .insert(parent_id, Tile::Container(Container::Linear(linear)));
                }
            }
            ContainerInsertion::Grid(insert_location) => {
                if let Tile::Container(Container::Grid(grid)) = &mut tile {
                    grid.locations.retain(|_, pos| *pos != insert_location);
                    grid.locations.insert(child_id, insert_location);
                    grid.children.push(child_id);
                    self.tiles.insert(parent_id, tile);
                } else {
                    let new_tile_id = self.insert_tile(tile);
                    let mut grid = Grid::new(vec![new_tile_id, child_id]);
                    grid.locations.insert(child_id, insert_location);
                    self.tiles
                        .insert(parent_id, Tile::Container(Container::Grid(grid)));
                }
            }
        }
    }

    /// Detect cycles, duplications, and other invalid state, and fix it.
    ///
    /// Will also call [`Behavior::retain_pane`] to check if a users wants to remove a pane.
    ///
    /// Finally free up any tiles that are no longer reachable from the root.
    pub(super) fn gc_root(&mut self, behavior: &mut dyn Behavior<Pane>, root_id: Option<TileId>) {
        let mut visited = Default::default();

        if let Some(root_id) = root_id {
            // We ignore the returned root action, because we will never remove the root.
            let _root_action = self.gc_tile_id(behavior, &mut visited, root_id);
        }

        if visited.len() < self.tiles.len() {
            // This should only happen if the user set up the tree in a bad state,
            // or if it was restored from a bad state via serde.
            // …or if there is a bug somewhere 😜
            log::warn!(
                "GC collecting tiles: {:?}",
                self.tiles
                    .keys()
                    .filter(|id| !visited.contains(id))
                    .collect::<Vec<_>>()
            );
        }

        self.invisible.retain(|tile_id| visited.contains(tile_id));
        self.tiles.retain(|tile_id, _| visited.contains(tile_id));
    }

    /// Detect cycles, duplications, and other invalid state, and remove them.
    fn gc_tile_id(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        visited: &mut nohash_hasher::IntSet<TileId>,
        tile_id: TileId,
    ) -> GcAction {
        let Some(mut tile) = self.tiles.remove(&tile_id) else { return GcAction::Remove; };
        if !visited.insert(tile_id) {
            log::warn!("Cycle or duplication detected");
            return GcAction::Remove;
        }

        match &mut tile {
            Tile::Pane(pane) => {
                if !behavior.retain_pane(pane) {
                    return GcAction::Remove;
                }
            }
            Tile::Container(container) => {
                container
                    .retain(|child| self.gc_tile_id(behavior, visited, child) == GcAction::Keep);
            }
        }
        self.tiles.insert(tile_id, tile);
        GcAction::Keep
    }

    pub(super) fn layout_tile(
        &mut self,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
        tile_id: TileId,
    ) {
        let Some(mut tile) = self.tiles.remove(&tile_id) else {
            log::warn!("Failed to find tile {tile_id:?} during layout");
            return;
        };
        self.rects.insert(tile_id, rect);

        if let Tile::Container(container) = &mut tile {
            container.layout(self, style, behavior, rect);
        }

        self.tiles.insert(tile_id, tile);
    }

    /// Simplify the tree, perhaps culling empty containers,
    /// and/or merging single-child containers into their parent.
    ///
    /// Drag-dropping tiles can often leave containers empty, or with only a single child.
    /// This is often undersired, so this function can be used to clean up the tree.
    ///
    /// What simplifcations are allowed is controlled by the [`SimplificationOptions`].
    pub(super) fn simplify(
        &mut self,
        options: &SimplificationOptions,
        it: TileId,
        parent_kind: Option<ContainerKind>,
    ) -> SimplifyAction {
        let Some(mut tile) = self.tiles.remove(&it) else {
            log::warn!("Failed to find tile {it:?} during simplify");
            return SimplifyAction::Remove;
        };

        if let Tile::Container(container) = &mut tile {
            let kind = container.kind();
            container.simplify_children(|child| self.simplify(options, child, Some(kind)));

            if kind == ContainerKind::Tabs {
                if options.prune_empty_tabs && container.is_empty() {
                    log::debug!("Simplify: removing empty tabs container");
                    return SimplifyAction::Remove;
                }

                if options.prune_single_child_tabs && container.children().len() == 1 {
                    let child_is_pane =
                        matches!(self.get(container.children()[0]), Some(Tile::Pane(_)));

                    if options.all_panes_must_have_tabs
                        && child_is_pane
                        && parent_kind != Some(ContainerKind::Tabs)
                    {
                        // Keep it, even though we only one child
                    } else {
                        log::debug!("Simplify: collapsing single-child tabs container");
                        return SimplifyAction::Replace(container.children()[0]);
                    }
                }
            } else {
                if options.join_nested_linear_containerss {
                    if let Container::Linear(parent) = container {
                        let mut new_children = Vec::with_capacity(parent.children.len());
                        for child_id in parent.children.drain(..) {
                            if let Some(Tile::Container(Container::Linear(child))) =
                                &mut self.get_mut(child_id)
                            {
                                if parent.dir == child.dir {
                                    // absorb the child
                                    log::debug!(
                                        "Simplify: absorbing nested linear container with {} children",
                                        child.children.len()
                                    );

                                    let mut child_share_sum = 0.0;
                                    for &grandchild in &child.children {
                                        child_share_sum += child.shares[grandchild];
                                    }
                                    let share_normalizer =
                                        parent.shares[child_id] / child_share_sum;
                                    for &grandchild in &child.children {
                                        new_children.push(grandchild);
                                        parent.shares[grandchild] =
                                            child.shares[grandchild] * share_normalizer;
                                    }

                                    self.tiles.remove(&child_id);
                                } else {
                                    // keep the child
                                    new_children.push(child_id);
                                }
                            } else {
                                new_children.push(child_id);
                            }
                        }
                        parent.children = new_children;
                    }
                }

                if options.prune_empty_containers && container.is_empty() {
                    log::debug!("Simplify: removing empty container tile");
                    return SimplifyAction::Remove;
                }
                if options.prune_single_child_containers && container.children().len() == 1 {
                    log::debug!("Simplify: collapsing single-child container tile");
                    return SimplifyAction::Replace(container.children()[0]);
                }
            }
        }

        self.tiles.insert(it, tile);
        SimplifyAction::Keep
    }

    pub(super) fn make_all_panes_children_of_tabs(&mut self, parent_is_tabs: bool, it: TileId) {
        let Some(mut tile) = self.tiles.remove(&it) else {
            log::warn!("Failed to find tile {it:?} during make_all_panes_children_of_tabs");
            return;
        };

        match &mut tile {
            Tile::Pane(_) => {
                if !parent_is_tabs {
                    // Add tabs to this pane:
                    log::debug!("Auto-adding Tabs-parent to pane {it:?}");
                    let new_id = TileId::random();
                    self.tiles.insert(new_id, tile);
                    self.tiles
                        .insert(it, Tile::Container(Container::new_tabs(vec![new_id])));
                    return;
                }
            }
            Tile::Container(container) => {
                let is_tabs = container.kind() == ContainerKind::Tabs;
                for &child in container.children() {
                    self.make_all_panes_children_of_tabs(is_tabs, child);
                }
            }
        }

        self.tiles.insert(it, tile);
    }

    pub(super) fn make_active(
        &mut self,
        it: TileId,
        should_activate: &dyn Fn(&Tile<Pane>) -> bool,
    ) -> bool {
        let Some(mut tile) = self.tiles.remove(&it) else {
            log::warn!("Failed to find tile {it:?} during make_active");
            return false;
        };

        let mut activate = should_activate(&tile);

        if let Tile::Container(container) = &mut tile {
            let mut active_child = None;
            for &child in container.children() {
                if self.make_active(child, should_activate) {
                    active_child = Some(child);
                }
            }

            if let Some(active_child) = active_child {
                if let Container::Tabs(tabs) = container {
                    tabs.set_active(active_child);
                }
            }

            activate |= active_child.is_some();
        }

        self.tiles.insert(it, tile);
        activate
    }
}
