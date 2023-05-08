use egui::{Id, NumExt as _, Rect, Ui};

use crate::Layout;

use super::{
    is_possible_drag, Behavior, Container, DropContext, InsertionPoint, SimplificationOptions,
    SimplifyAction, Tile, TileId, Tiles,
};

/// The top level type. Contains all persistent state, including layouts and sizes.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Tree<Pane> {
    pub root: TileId,
    pub tiles: Tiles<Pane>,

    /// Smoothed avaerage of preview
    #[serde(skip)]
    pub smoothed_preview_rect: Option<Rect>,
}

impl<Pane> Default for Tree<Pane> {
    fn default() -> Self {
        Self {
            root: Default::default(),
            tiles: Default::default(),
            smoothed_preview_rect: None,
        }
    }
}

impl<Pane: std::fmt::Debug> std::fmt::Debug for Tree<Pane> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print a hiearchical view of the tree:
        fn format_tile<Pane: std::fmt::Debug>(
            f: &mut std::fmt::Formatter<'_>,
            tiles: &Tiles<Pane>,
            indent: usize,
            tile_id: TileId,
        ) -> std::fmt::Result {
            write!(f, "{} {tile_id:?} ", "  ".repeat(indent))?;
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
                write!(f, "DANGLING {tile_id:?}")
            }
        }

        writeln!(f, "Tree {{")?;
        format_tile(f, &self.tiles, 1, self.root)?;
        write!(f, "\n}}")
    }
}

// ----------------------------------------------------------------------------

impl<Pane> Tree<Pane> {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(root: TileId, tiles: Tiles<Pane>) -> Self {
        Self {
            root,
            tiles,
            smoothed_preview_rect: None,
        }
    }

    pub fn new_tabs(panes: Vec<Pane>) -> Self {
        Self::new_layout(Layout::Tabs, panes)
    }

    pub fn new_horizontal(panes: Vec<Pane>) -> Self {
        Self::new_layout(Layout::Horizontal, panes)
    }

    pub fn new_vertical(panes: Vec<Pane>) -> Self {
        Self::new_layout(Layout::Vertical, panes)
    }

    pub fn new_grid(panes: Vec<Pane>) -> Self {
        Self::new_layout(Layout::Grid, panes)
    }

    pub fn new_layout(layout: Layout, panes: Vec<Pane>) -> Self {
        let mut tiles = Tiles::default();
        let tile_ids = panes
            .into_iter()
            .map(|pane| tiles.insert_pane(pane))
            .collect();
        let root = tiles.insert_tile(Tile::Container(Container::new(layout, tile_ids)));
        Self::new(root, tiles)
    }

    pub fn root(&self) -> TileId {
        self.root
    }

    /// Show the tree in the given [`Ui`].
    ///
    /// The tree will use upp all the available space - nothing more, nothing less.
    pub fn ui(&mut self, behavior: &mut dyn Behavior<Pane>, ui: &mut Ui) {
        let options = behavior.simplification_options();
        self.simplify(&options);
        if options.all_panes_must_have_tabs {
            self.tiles.make_all_panes_children_of_tabs(false, self.root);
        }

        self.tiles.gc_root(behavior, self.root);

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

        self.tiles.layout_tile(
            ui.style(),
            behavior,
            ui.available_rect_before_wrap(),
            self.root,
        );

        self.tiles
            .tile_ui(behavior, &mut drop_context, ui, self.root);

        self.preview_dragged_tile(behavior, &drop_context, ui);
    }

    /// Recursively "activate" the ancestors of the tiles that matches the given predicate.
    ///
    /// This means making the matching tiles and its ancestors the active tab in any tab layout.
    pub fn make_active(&mut self, should_activate: impl Fn(&Tile<Pane>) -> bool) {
        self.tiles.make_active(self.root, &should_activate);
    }

    fn preview_dragged_tile(
        &mut self,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &DropContext,
        ui: &mut Ui,
    ) {
        if let (Some(mouse_pos), Some(dragged_tile_id)) =
            (drop_context.mouse_pos, drop_context.dragged_tile_id)
        {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

            // Preview what is being dragged:
            egui::Area::new(Id::new((dragged_tile_id, "preview")))
                .pivot(egui::Align2::CENTER_CENTER)
                .current_pos(mouse_pos)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    let mut frame = egui::Frame::popup(ui.style());
                    frame.fill = frame.fill.gamma_multiply(0.5); // Make see-through
                    frame.show(ui, |ui| {
                        // TODO(emilk): preview contents?
                        let text = behavior.tab_title_for_tile(&self.tiles, dragged_tile_id);
                        ui.label(text);
                    });
                });

            if let Some(preview_rect) = drop_context.preview_rect {
                let preview_rect = self.smooth_preview_rect(ui.ctx(), preview_rect);

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
                ui.memory_mut(|mem| mem.stop_dragging());
                if let Some(insertion_point) = drop_context.best_insertion {
                    self.move_tile(dragged_tile_id, insertion_point);
                }
                self.smoothed_preview_rect = None;
            }
        } else {
            self.smoothed_preview_rect = None;
        }
    }

    /// Take the preview rectangle and smooth it over time.
    fn smooth_preview_rect(&mut self, ctx: &egui::Context, new_rect: Rect) -> Rect {
        let dt = ctx.input(|input| input.stable_dt).at_most(0.1);
        let t = egui::emath::exponential_smooth_factor(0.9, 0.05, dt);

        let smoothed = self.smoothed_preview_rect.get_or_insert(new_rect);
        *smoothed = smoothed.lerp_towards(&new_rect, t);

        let diff = smoothed.min.distance(new_rect.min) + smoothed.max.distance(new_rect.max);
        if diff < 0.5 {
            *smoothed = new_rect;
        } else {
            ctx.request_repaint();
        }
        *smoothed
    }

    fn simplify(&mut self, options: &SimplificationOptions) {
        match self.tiles.simplify(options, self.root) {
            SimplifyAction::Remove => {
                log::warn!("Tried to simplify root tile!"); // TODO: handle this
            }
            SimplifyAction::Keep => {}
            SimplifyAction::Replace(new_root) => {
                self.root = new_root;
            }
        }
    }

    /// Move the given tile to the given insertion point.
    pub(super) fn move_tile(&mut self, moved_tile_id: TileId, insertion_point: InsertionPoint) {
        log::debug!(
            "Moving {moved_tile_id:?} into {:?}",
            insertion_point.insertion
        );
        self.remove_tile_id_from_parent(moved_tile_id);
        self.tiles.insert(insertion_point, moved_tile_id);
    }

    /// Find the currently dragged tile, if any.
    pub fn dragged_id(&self, ctx: &egui::Context) -> Option<TileId> {
        if !is_possible_drag(ctx) {
            // We're not sure we're dragging _at all_ yet.
            return None;
        }

        for &tile_id in self.tiles.tiles.keys() {
            if tile_id == self.root {
                continue; // not allowed to drag root
            }

            let id = tile_id.id();
            let is_tile_being_dragged = ctx.memory(|mem| mem.is_being_dragged(id));
            if is_tile_being_dragged {
                // Abort drags on escape:
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ctx.memory_mut(|mem| mem.stop_dragging());
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
    pub(super) fn remove_tile_id_from_parent(&mut self, remove_me: TileId) {
        for parent in self.tiles.tiles.values_mut() {
            if let Tile::Container(container) = parent {
                container.retain(|child| child != remove_me);
            }
        }
    }
}
