#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use egui::{
    Align2, Stroke,
    emath::{GuiRounding, History},
    pos2,
};

const TREE_ID: &str = "my_tree";

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct Pane {
    nr: usize,
}

#[derive(Clone, Default)]
struct TreeBehavior {
    simplification_options: egui_tiles::SimplificationOptions,
    tab_bar_height: f32,
    gap_width: f32,
    closable_tabs: bool,
    selected_pane: Option<usize>,
    add_child_to: Option<egui_tiles::TileId>,
    floating_borders: bool,
}

impl TreeBehavior {
    fn new(selected_pane: Option<usize>) -> Self {
        Self {
            simplification_options: Default::default(),
            tab_bar_height: 24.0,
            gap_width: 2.0,
            closable_tabs: true,
            selected_pane,
            add_child_to: None,
            floating_borders: true,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("behavior_ui_simple")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("All panes must have tabs:");
                ui.checkbox(
                    &mut self.simplification_options.all_panes_must_have_tabs,
                    "",
                );
                ui.end_row();

                ui.label("Join nested containers:");
                ui.checkbox(
                    &mut self.simplification_options.join_nested_linear_containers,
                    "",
                );
                ui.end_row();

                ui.label("Tab bar height:");
                ui.add(
                    egui::DragValue::new(&mut self.tab_bar_height)
                        .range(0.0..=100.0)
                        .speed(1.0),
                );
                ui.end_row();

                ui.label("Gap width:");
                ui.add(
                    egui::DragValue::new(&mut self.gap_width)
                        .range(0.0..=20.0)
                        .speed(1.0),
                );
                ui.end_row();

                ui.label("Closable tabs:");
                ui.checkbox(&mut self.closable_tabs, "");
                ui.end_row();

                ui.label("Floating borders:");
                ui.checkbox(&mut self.floating_borders, "");
                ui.end_row();
            });
    }
}

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        format!("Pane {}", pane.nr).into()
    }

    fn allow_creating_tabs_on_drop(&self) -> bool {
        false
    }

    fn paint_corner_hint(&self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
        if self.selected_pane.is_some() {
            // For selected panes, paint a custom hint, e.g., a circle in the corner
            let painter = ui.painter();
            let center = egui::Align2::RIGHT_BOTTOM.pos_in_rect(&rect);
            let radius = 6.0;
            let stroke = ui.style().interact(response).fg_stroke;
            painter.circle_stroke(center, radius, stroke);
        } else {
            // For non-selected panes, paint the default diagonal lines
            let style_stroke = ui.style().interact(response).fg_stroke;
            let painter = ui.painter();
            let corner = Align2::RIGHT_BOTTOM;
            let corner_pos = corner
                .pos_in_rect(&rect)
                .round_to_pixels(ui.pixels_per_point());

            let mut w = 2.0;
            let stroke = Stroke {
                width: 1.0,
                color: style_stroke.color,
            };

            while w <= rect.width() && w <= rect.height() {
                let x_dir = corner.x().to_sign();
                let y_dir = corner.y().to_sign();
                painter.line_segment(
                    [
                        pos2(corner_pos.x - w * x_dir, corner_pos.y),
                        pos2(corner_pos.x, corner_pos.y - w * y_dir),
                    ],
                    stroke,
                );
                w += 4.0;
            }
        }
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        // Give each pane a unique color:
        let mut color = egui::epaint::Hsva::new(0.103 * pane.nr as f32, 0.5, 0.5, 1.0);
        if self.selected_pane == Some(pane.nr) {
            color = egui::epaint::Hsva::new(0.103 * pane.nr as f32, 0.8, 0.8, 1.0); // brighter if selected
        }
        ui.painter().rect_filled(ui.max_rect(), 0.0, color);

        ui.label(format!("The contents of pane {}.", pane.nr));
        ui.label(format!("Size: {:?}", ui.max_rect().size()));

        if ui.button("Select").clicked() {
            self.selected_pane = Some(pane.nr);
        }

        // You can make your pane draggable like so:
        if ui
            .add(egui::Button::new("Drag me!").sense(egui::Sense::drag()))
            .drag_started()
        {
            return egui_tiles::UiResponse::DragStarted;
        }

        egui_tiles::UiResponse::None
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<Pane>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if ui.small_button("+").clicked() {
            self.add_child_to = Some(tile_id);
        }
        // rely on built-in close buttons from egui_tiles using is_tab_closable + on_tab_close
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.tab_bar_height
    }
    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.gap_width
    }
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        self.simplification_options
    }
    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
    ) -> bool {
        self.closable_tabs
    }

    fn on_tab_close(
        &mut self,
        tiles: &mut egui_tiles::Tiles<Pane>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                egui_tiles::Tile::Pane(pane) => {
                    let title = self.tab_title_for_pane(pane);
                    log::debug!("Closing pane: {} - {tile_id:?}", title.text());
                    if self.selected_pane == Some(pane.nr) {
                        self.selected_pane = None;
                    }
                }
                egui_tiles::Tile::Container(container) => {
                    log::debug!("Closing container: {:?} - {tile_id:?}", container.kind());
                    for child in container.children() {
                        if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(*child) {
                            let title = self.tab_title_for_pane(pane);
                            log::debug!("Closing pane in container: {} - {child:?}", title.text());
                            if self.selected_pane == Some(pane.nr) {
                                self.selected_pane = None;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    fn floating_pane_border_enabled(&self) -> bool {
        self.floating_borders
    }

    fn floating_pane_border_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        Stroke::new(5.0, egui::Color32::RED)
    }

    fn floating_pane_border_rounding(&self, _visuals: &egui::Visuals) -> f32 {
        0.0 // Remove rounding for now to make it more obvious
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
struct SimpleApp {
    tree: egui_tiles::Tree<Pane>,
    selected_pane: Option<usize>,
    fully_maximize: bool,
    next_pane_nr: usize,
    continuous_render: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    fps_history: History<f32>,
    #[cfg_attr(feature = "serde", serde(skip))]
    behavior: TreeBehavior,
    show_sidebar: bool,
    show_tabs: bool,
}

impl SimpleApp {
    fn new() -> Self {
        let (tree, next_pane_nr) = create_tree();
        Self {
            tree,
            selected_pane: None,
            fully_maximize: false,
            next_pane_nr,
            continuous_render: false,
            fps_history: History::new(5..120, 1.5),
            behavior: TreeBehavior::new(None),
            show_sidebar: true,
            show_tabs: true,
        }
    }
}

impl Default for SimpleApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for SimpleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (fps_instant, now) = ctx.input(|i| {
            let dt = i.stable_dt;
            let fps = if dt > f32::EPSILON { 1.0 / dt } else { 0.0 };
            (fps, i.time)
        });
        if fps_instant.is_finite() && now.is_finite() {
            self.fps_history.add(now, fps_instant);
        }
        let fps_display = self
            .fps_history
            .average()
            .filter(|v| v.is_finite())
            .unwrap_or(fps_instant);

        if self.show_sidebar {
            egui::SidePanel::left("tree_sidebar_simple").show(ctx, |ui| {
                if ui.button("Reset").clicked() {
                    *self = SimpleApp::new();
                }
                if ui.button("Toggle Sidebar").clicked() {
                    self.show_sidebar = !self.show_sidebar;
                }
                ui.checkbox(&mut self.show_tabs, "Show Tabs for all panes");
                self.behavior
                    .simplification_options
                    .all_panes_must_have_tabs = self.show_tabs;
                self.behavior.ui(ui);
                if ui.button("Add pane to root").clicked() {
                    self.add_pane_to_root();
                }
                ui.label(format!("FPS: {:.1}", fps_display));
                ui.checkbox(&mut self.continuous_render, "Continuous render");
                let mut floating = self.tree.floating;
                if ui.checkbox(&mut floating, "Floating mode").changed() {
                    self.tree.set_floating(floating);
                }
                ui.label(if self.tree.floating {
                    "Floating Mode"
                } else {
                    "Tiled Mode"
                });
                ui.separator();
                ui.collapsing("Active tiles", |ui| {
                    let active = self.tree.active_tiles();
                    for tile_id in active {
                        use egui_tiles::Behavior as _;
                        let name = self.behavior.tab_title_for_tile(&self.tree.tiles, tile_id);
                        ui.label(format!("{} - {tile_id:?}", name.text()));
                    }
                });
                ui.separator();
                if let Some(root) = self.tree.root() {
                    tree_ui(ui, &mut self.behavior, &mut self.tree.tiles, root);
                }
                if let Some(parent) = self.behavior.add_child_to.take() {
                    self.add_pane_to_tabs(parent);
                }
            });
        } else {
            egui::TopBottomPanel::top("top_controls_simple").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Show Sidebar").clicked() {
                        self.show_sidebar = true;
                    }
                    ui.checkbox(&mut self.show_tabs, "Show Tabs for all panes");
                    self.behavior
                        .simplification_options
                        .all_panes_must_have_tabs = self.show_tabs;
                    if ui.button("Add Pane").clicked() {
                        self.add_pane_to_root();
                    }
                    ui.label(format!("FPS: {:.1}", fps_display));
                    ui.checkbox(&mut self.continuous_render, "Continuous render");
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Toggle Floating").clicked() {
                    self.tree.set_floating(!self.tree.floating);
                }
                if ui.button("Add Pane").clicked() {
                    self.add_pane_to_root();
                }
                if ui.button("Clear Tiles").clicked() {
                    self.clear_all_tiles();
                }
                ui.label(format!("FPS: {:.1}", fps_display));
                ui.checkbox(&mut self.continuous_render, "Continuous Render");
                ui.label(if self.tree.floating {
                    "Floating Mode"
                } else {
                    "Tiled Mode"
                });
                ui.checkbox(&mut self.fully_maximize, "Fully maximize");
                let button_text = if self.tree.is_maximized() {
                    "Restore Layout"
                } else if self.fully_maximize {
                    "Fully Maximize Selected"
                } else {
                    "Maximize Selected"
                };
                let can_toggle = self.tree.is_maximized() || self.selected_pane.is_some();
                if ui
                    .add_enabled(can_toggle, egui::Button::new(button_text))
                    .clicked()
                {
                    self.toggle_maximize();
                }
            });
            self.handle_resize_keys(ui);
            // Use persistent behavior instance
            self.behavior.selected_pane = self.selected_pane;
            self.tree.ui(&mut self.behavior, ui);
            if let Some(tabs_id) = self.behavior.add_child_to.take() {
                self.add_pane_to_tabs(tabs_id);
            }
            self.selected_pane = self.behavior.selected_pane;
        });

        if self.continuous_render {
            ctx.request_repaint();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        #[cfg(feature = "serde")]
        eframe::set_value(storage, eframe::APP_KEY, &self);
    }
}

impl SimpleApp {
    fn handle_resize_keys(&mut self, ui: &mut egui::Ui) {
        let Some(tile_id) = self.selected_tile_id() else {
            return;
        };

        let input = ui.input(|i| i.clone());
        let shift = input.modifiers.shift;
        const LINEAR_STEP: f32 = 0.05;
        const GRID_STEP: f32 = 0.05;

        if input.key_pressed(egui::Key::ArrowRight) {
            self.tree
                .visit_ancestor_containers_mut(tile_id, |_, container, child_id| match container {
                    egui_tiles::Container::Linear(linear)
                        if linear.dir == egui_tiles::LinearDir::Horizontal =>
                    {
                        let Some(index) = linear.children.iter().position(|&c| c == child_id)
                        else {
                            return false;
                        };
                        if index + 1 >= linear.children.len() {
                            return false;
                        }
                        let neighbor = linear.children[index + 1];
                        if shift {
                            linear.transfer_share(child_id, neighbor, LINEAR_STEP)
                        } else {
                            linear.transfer_share(neighbor, child_id, LINEAR_STEP)
                        }
                    }
                    egui_tiles::Container::Grid(grid) => {
                        let Some(index) = grid.child_index(child_id) else {
                            return false;
                        };
                        let num_cols = grid.col_shares.len();
                        if num_cols == 0 {
                            return false;
                        }
                        let col = index % num_cols;
                        if col + 1 >= num_cols {
                            return false;
                        }
                        let neighbor_col = col + 1;
                        if shift {
                            grid.transfer_col_share(col, neighbor_col, GRID_STEP)
                        } else {
                            grid.transfer_col_share(neighbor_col, col, GRID_STEP)
                        }
                    }
                    _ => false,
                });
        }

        if input.key_pressed(egui::Key::ArrowLeft) {
            self.tree
                .visit_ancestor_containers_mut(tile_id, |_, container, child_id| match container {
                    egui_tiles::Container::Linear(linear)
                        if linear.dir == egui_tiles::LinearDir::Horizontal =>
                    {
                        let Some(index) = linear.children.iter().position(|&c| c == child_id)
                        else {
                            return false;
                        };
                        if index == 0 {
                            return false;
                        }
                        let neighbor = linear.children[index - 1];
                        if shift {
                            linear.transfer_share(child_id, neighbor, LINEAR_STEP)
                        } else {
                            linear.transfer_share(neighbor, child_id, LINEAR_STEP)
                        }
                    }
                    egui_tiles::Container::Grid(grid) => {
                        let Some(index) = grid.child_index(child_id) else {
                            return false;
                        };
                        let num_cols = grid.col_shares.len();
                        if num_cols == 0 {
                            return false;
                        }
                        let col = index % num_cols;
                        if col == 0 {
                            return false;
                        }
                        let neighbor_col = col - 1;
                        if shift {
                            grid.transfer_col_share(col, neighbor_col, GRID_STEP)
                        } else {
                            grid.transfer_col_share(neighbor_col, col, GRID_STEP)
                        }
                    }
                    _ => false,
                });
        }

        if input.key_pressed(egui::Key::ArrowUp) {
            self.tree
                .visit_ancestor_containers_mut(tile_id, |_, container, child_id| match container {
                    egui_tiles::Container::Linear(linear)
                        if linear.dir == egui_tiles::LinearDir::Vertical =>
                    {
                        let Some(index) = linear.children.iter().position(|&c| c == child_id)
                        else {
                            return false;
                        };
                        if index == 0 {
                            return false;
                        }
                        let neighbor = linear.children[index - 1];
                        if shift {
                            linear.transfer_share(child_id, neighbor, LINEAR_STEP)
                        } else {
                            linear.transfer_share(neighbor, child_id, LINEAR_STEP)
                        }
                    }
                    egui_tiles::Container::Grid(grid) => {
                        let Some(index) = grid.child_index(child_id) else {
                            return false;
                        };
                        let num_cols = grid.col_shares.len();
                        if num_cols == 0 {
                            return false;
                        }
                        let num_rows = grid.row_shares.len();
                        if num_rows == 0 {
                            return false;
                        }
                        let row = index / num_cols;
                        if row == 0 {
                            return false;
                        }
                        let neighbor_row = row - 1;
                        if shift {
                            grid.transfer_row_share(row, neighbor_row, GRID_STEP)
                        } else {
                            grid.transfer_row_share(neighbor_row, row, GRID_STEP)
                        }
                    }
                    _ => false,
                });
        }

        if input.key_pressed(egui::Key::ArrowDown) {
            self.tree
                .visit_ancestor_containers_mut(tile_id, |_, container, child_id| match container {
                    egui_tiles::Container::Linear(linear)
                        if linear.dir == egui_tiles::LinearDir::Vertical =>
                    {
                        let Some(index) = linear.children.iter().position(|&c| c == child_id)
                        else {
                            return false;
                        };
                        if index + 1 >= linear.children.len() {
                            return false;
                        }
                        let neighbor = linear.children[index + 1];
                        if shift {
                            linear.transfer_share(child_id, neighbor, LINEAR_STEP)
                        } else {
                            linear.transfer_share(neighbor, child_id, LINEAR_STEP)
                        }
                    }
                    egui_tiles::Container::Grid(grid) => {
                        let Some(index) = grid.child_index(child_id) else {
                            return false;
                        };
                        let num_cols = grid.col_shares.len();
                        if num_cols == 0 {
                            return false;
                        }
                        let num_rows = grid.row_shares.len();
                        if num_rows == 0 {
                            return false;
                        }
                        let row = index / num_cols;
                        if row + 1 >= num_rows {
                            return false;
                        }
                        let neighbor_row = row + 1;
                        if shift {
                            grid.transfer_row_share(row, neighbor_row, GRID_STEP)
                        } else {
                            grid.transfer_row_share(neighbor_row, row, GRID_STEP)
                        }
                    }
                    _ => false,
                });
        }

        if let Some(parent_id) = self.tree.tiles.parent_of(tile_id) {
            if let Some(egui_tiles::Tile::Container(container)) = self.tree.tiles.get_mut(parent_id)
            {
                if let egui_tiles::Container::Grid(grid) = container {
                    let Some(index) = grid.child_index(tile_id) else {
                        return;
                    };
                    let num_cols = grid.num_cols();
                    if num_cols == 0 {
                        return;
                    }
                    let row = index / num_cols;
                    let col = index % num_cols;
                    let neighbor_index = if input.key_pressed(egui::Key::H) {
                        if col > 0 { Some(index - 1) } else { None }
                    } else if input.key_pressed(egui::Key::L) {
                        if col + 1 < num_cols {
                            Some(index + 1)
                        } else {
                            None
                        }
                    } else if input.key_pressed(egui::Key::K) {
                        if row > 0 {
                            Some(index - num_cols)
                        } else {
                            None
                        }
                    } else if input.key_pressed(egui::Key::J) {
                        let num_rows = (grid.num_children() + num_cols - 1) / num_cols;
                        if row + 1 < num_rows {
                            Some(index + num_cols)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(neigh) = neighbor_index {
                        grid.swap_children(index, neigh);
                    }
                }
            }
        }

        let key_h = input.key_pressed(egui::Key::H);
        let key_j = input.key_pressed(egui::Key::J);
        let key_k = input.key_pressed(egui::Key::K);
        let key_l = input.key_pressed(egui::Key::L);
        let move_prev = key_h || key_k;
        let move_next = key_l || key_j;

        if let Some(forward) = match (move_prev, move_next) {
            (true, false) => Some(false),
            (false, true) => Some(true),
            _ => None,
        } {
            let horizontal_request = key_h || key_l;
            let vertical_request = key_k || key_j;

            let mut swapped = false;
            if horizontal_request ^ vertical_request {
                let preferred_dir = if horizontal_request {
                    egui_tiles::LinearDir::Horizontal
                } else {
                    egui_tiles::LinearDir::Vertical
                };
                swapped =
                    self.tree
                        .swap_tile_in_linear_ancestors(tile_id, forward, Some(preferred_dir));
            }

            if !swapped {
                self.tree
                    .swap_tile_in_linear_ancestors(tile_id, forward, None);
            }
        }
    }

    fn add_pane_to_tabs(&mut self, tabs_id: egui_tiles::TileId) {
        let pane_nr = self.next_pane_nr;
        self.next_pane_nr += 1;

        let pane_id = self.tree.tiles.insert_pane(Pane { nr: pane_nr });
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            self.tree.tiles.get_mut(tabs_id)
        {
            tabs.add_child(pane_id);
            tabs.set_active(pane_id);
            self.selected_pane = Some(pane_nr);
        } else {
            self.tree.tiles.remove(pane_id);
        }
    }

    fn add_pane_to_root(&mut self) {
        if let Some(root_id) = self.tree.root() {
            let root_is_tabs = matches!(
                self.tree.tiles.get(root_id),
                Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(_)))
            );
            if root_is_tabs {
                self.add_pane_to_tabs(root_id);
                return;
            }
        }

        let pane_nr = self.next_pane_nr;
        self.next_pane_nr += 1;
        let pane_id = self.tree.tiles.insert_pane(Pane { nr: pane_nr });

        match self.tree.root() {
            Some(root_id) => {
                let mut wrap_in_tabs = false;
                let mut handled = false;

                if let Some(tile) = self.tree.tiles.get_mut(root_id) {
                    match tile {
                        egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear)) => {
                            linear.add_child(pane_id);
                            handled = true;
                        }
                        egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid)) => {
                            grid.add_child(pane_id);
                            handled = true;
                        }
                        egui_tiles::Tile::Container(egui_tiles::Container::Tabs(_)) => {
                            // Already handled at the top of the function.
                            handled = true;
                        }
                        egui_tiles::Tile::Pane(_) => {
                            wrap_in_tabs = true;
                        }
                    }
                }

                if wrap_in_tabs {
                    let new_root = self.tree.tiles.insert_tab_tile(vec![root_id, pane_id]);

                    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                        self.tree.tiles.get_mut(new_root)
                    {
                        tabs.set_active(pane_id);
                    }
                    self.tree.root = Some(new_root);
                    handled = true;
                }

                if !handled {
                    // If we couldn't access the existing root tile (e.g. missing), fall back to making
                    // the new pane the root.
                    self.tree.root = Some(pane_id);
                }
            }
            None => {
                self.tree.root = Some(pane_id);
            }
        }

        self.selected_pane = Some(pane_nr);
    }

    fn clear_all_tiles(&mut self) {
        let floating = self.tree.floating;
        self.tree = egui_tiles::Tree::empty(TREE_ID);
        if floating {
            self.tree.set_floating(true);
        }
        self.selected_pane = None;
        self.next_pane_nr = 0;
    }

    fn toggle_maximize(&mut self) {
        if self.tree.is_maximized() {
            self.tree.clear_maximized();
            return;
        }

        if let Some(tile_id) = self.selected_tile_id() {
            self.tree.toggle_maximize(tile_id, self.fully_maximize);
        }
    }

    fn selected_tile_id(&self) -> Option<egui_tiles::TileId> {
        let selected_nr = self.selected_pane?;
        self.tree.tiles.tile_ids().find(|&id| {
            matches!(
                self.tree.tiles.get(id),
                Some(egui_tiles::Tile::Pane(p)) if p.nr == selected_nr
            )
        })
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            let mut app = SimpleApp::new();
            #[cfg(feature = "serde")]
            if let Some(storage) = cc.storage {
                if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                    app = state;
                }
            }
            Ok(Box::new(app))
        }),
    )
}

fn tree_ui(
    ui: &mut egui::Ui,
    behavior: &mut dyn egui_tiles::Behavior<Pane>,
    tiles: &mut egui_tiles::Tiles<Pane>,
    tile_id: egui_tiles::TileId,
) {
    let text = format!(
        "{} - {tile_id:?}",
        behavior.tab_title_for_tile(tiles, tile_id).text()
    );
    let Some(mut tile) = tiles.remove(tile_id) else {
        return;
    };
    let default_open = true;
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.id().with((tile_id, "tree_simple")),
        default_open,
    )
    .show_header(ui, |ui| {
        ui.label(text);
        let mut visible = tiles.is_visible(tile_id);
        ui.checkbox(&mut visible, "Visible");
        tiles.set_visible(tile_id, visible);
    })
    .body(|ui| match &mut tile {
        egui_tiles::Tile::Pane(_) => {}
        egui_tiles::Tile::Container(container) => {
            let mut kind = container.kind();
            egui::ComboBox::from_label("Kind")
                .selected_text(format!("{kind:?}"))
                .show_ui(ui, |ui| {
                    for alternative in egui_tiles::ContainerKind::ALL {
                        ui.selectable_value(&mut kind, alternative, format!("{alternative:?}"));
                    }
                });
            if kind != container.kind() {
                container.set_kind(kind);
            }
            for &child in container.children() {
                tree_ui(ui, behavior, tiles, child);
            }
        }
    });
    tiles.insert(tile_id, tile);
}

fn create_tree() -> (egui_tiles::Tree<Pane>, usize) {
    let mut next_view_nr = 0;
    let mut gen_pane = || {
        let pane = Pane { nr: next_view_nr };
        next_view_nr += 1;
        pane
    };

    let mut tiles = egui_tiles::Tiles::default();

    let mut tabs = vec![];
    tabs.push({
        let children = (0..7).map(|_| tiles.insert_pane(gen_pane())).collect();
        tiles.insert_horizontal_tile(children)
    });
    tabs.push({
        let cells = (0..11).map(|_| tiles.insert_pane(gen_pane())).collect();
        tiles.insert_grid_tile(cells)
    });
    tabs.push(tiles.insert_pane(gen_pane()));

    let root = tiles.insert_tab_tile(tabs);

    let tree = egui_tiles::Tree::new(TREE_ID, root, tiles);
    (tree, next_view_nr)
}
