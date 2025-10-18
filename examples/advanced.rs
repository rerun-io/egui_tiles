#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_tiles::{Tile, TileId, Tiles};

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_tiles example",
        options,
        Box::new(|_cc| {
            #[cfg_attr(not(feature = "serde"), allow(unused_mut))]
            let mut app = MyApp::default();
            #[cfg(feature = "serde")]
            if let Some(storage) = _cc.storage {
                if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                    app = state;
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Pane {
    nr: usize,
}

impl std::fmt::Debug for Pane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View").field("nr", &self.nr).finish()
    }
}

impl Pane {
    pub fn with_nr(nr: usize) -> Self {
        Self { nr }
    }

    pub fn ui(&self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let color = egui::epaint::Hsva::new(0.103 * self.nr as f32, 0.5, 0.5, 1.0);
        ui.painter().rect_filled(ui.max_rect(), 0.0, color);
        let dragged = ui
            .allocate_rect(ui.max_rect(), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dragged();
        if dragged {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }
}

struct TreeBehavior {
    simplification_options: egui_tiles::SimplificationOptions,
    tab_bar_height: f32,
    gap_width: f32,
    add_child_to: Option<egui_tiles::TileId>,
}

impl Default for TreeBehavior {
    fn default() -> Self {
        Self {
            simplification_options: Default::default(),
            tab_bar_height: 24.0,
            gap_width: 2.0,
            add_child_to: None,
        }
    }
}

impl TreeBehavior {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let Self {
            simplification_options,
            tab_bar_height,
            gap_width,
            add_child_to: _,
        } = self;

        egui::Grid::new("behavior_ui")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("All panes must have tabs:");
                ui.checkbox(&mut simplification_options.all_panes_must_have_tabs, "");
                ui.end_row();

                ui.label("Join nested containers:");
                ui.checkbox(
                    &mut simplification_options.join_nested_linear_containers,
                    "",
                );
                ui.end_row();

                ui.label("Tab bar height:");
                ui.add(
                    egui::DragValue::new(tab_bar_height)
                        .range(0.0..=100.0)
                        .speed(1.0),
                );
                ui.end_row();

                ui.label("Gap width:");
                ui.add(egui::DragValue::new(gap_width).range(0.0..=20.0).speed(1.0));
                ui.end_row();
            });
    }
}

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        view: &mut Pane,
    ) -> egui_tiles::UiResponse {
        view.ui(ui)
    }

    fn tab_title_for_pane(&mut self, view: &Pane) -> egui::WidgetText {
        format!("View {}", view.nr).into()
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<Pane>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if ui.button("➕").clicked() {
            self.add_child_to = Some(tile_id);
        }
    }

    // ---
    // Settings:

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.tab_bar_height
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.gap_width
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        self.simplification_options
    }

    fn is_tab_closable(&self, _tiles: &Tiles<Pane>, _tile_id: TileId) -> bool {
        true
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Pane>, tile_id: TileId) -> bool {
        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                Tile::Pane(pane) => {
                    // Single pane removal
                    let tab_title = self.tab_title_for_pane(pane);
                    log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                }
                Tile::Container(container) => {
                    // Container removal
                    log::debug!("Closing container: {:?}", container.kind());
                    let children_ids = container.children();
                    for child_id in children_ids {
                        if let Some(Tile::Pane(pane)) = tiles.get(*child_id) {
                            let tab_title = self.tab_title_for_pane(pane);
                            log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                        }
                    }
                }
            }
        }

        // Proceed to removing the tab
        true
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct MyApp {
    tree: egui_tiles::Tree<Pane>,
    next_pane_nr: usize,

    #[cfg_attr(feature = "serde", serde(skip))]
    behavior: TreeBehavior,
    #[cfg_attr(feature = "serde", serde(skip))]
    continuous_render: bool,
    #[cfg_attr(feature = "serde", serde(skip, default = "MyApp::fps_history_default"))]
    fps_history: egui::emath::History<f32>,
}

impl Default for MyApp {
    fn default() -> Self {
        let (tree, next_pane_nr) = create_initial_tree();
        Self {
            tree,
            next_pane_nr,
            behavior: Default::default(),
            continuous_render: false,
            fps_history: Self::fps_history_default(),
        }
    }
}

impl eframe::App for MyApp {
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

        egui::SidePanel::left("tree").show(ctx, |ui| {
            if ui.button("Reset").clicked() {
                *self = Default::default();
            }
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

            ui.collapsing("Tree", |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                let tree_debug = format!("{:#?}", self.tree);
                ui.monospace(&tree_debug);
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

        egui::CentralPanel::default().show(ctx, |ui| {
            self.tree.ui(&mut self.behavior, ui);
        });

        if self.continuous_render {
            ctx.request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        #[cfg(feature = "serde")]
        eframe::set_value(_storage, eframe::APP_KEY, &self);
    }
}

fn tree_ui(
    ui: &mut egui::Ui,
    behavior: &mut dyn egui_tiles::Behavior<Pane>,
    tiles: &mut egui_tiles::Tiles<Pane>,
    tile_id: egui_tiles::TileId,
) {
    // Get the name BEFORE we remove the tile below!
    let text = format!(
        "{} - {tile_id:?}",
        behavior.tab_title_for_tile(tiles, tile_id).text()
    );

    // Temporarily remove the tile to circumvent the borrowchecker
    let Some(mut tile) = tiles.remove(tile_id) else {
        log::debug!("Missing tile {tile_id:?}");
        return;
    };

    let default_open = true;
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.id().with((tile_id, "tree")),
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
                        ui.selectable_value(&mut kind, alternative, format!("{alternative:?}"))
                            .clicked();
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

    // Put the tile back
    tiles.insert(tile_id, tile);
}

fn create_initial_tree() -> (egui_tiles::Tree<Pane>, usize) {
    let mut next_view_nr = 0;
    let mut gen_view = || {
        let view = Pane::with_nr(next_view_nr);
        next_view_nr += 1;
        view
    };

    let mut tiles = egui_tiles::Tiles::default();

    let mut tabs = vec![];
    let tab_tile = {
        let children = (0..7).map(|_| tiles.insert_pane(gen_view())).collect();
        tiles.insert_tab_tile(children)
    };
    tabs.push(tab_tile);
    tabs.push({
        let children = (0..7).map(|_| tiles.insert_pane(gen_view())).collect();
        tiles.insert_horizontal_tile(children)
    });
    tabs.push({
        let children = (0..7).map(|_| tiles.insert_pane(gen_view())).collect();
        tiles.insert_vertical_tile(children)
    });
    tabs.push({
        let cells = (0..11).map(|_| tiles.insert_pane(gen_view())).collect();
        tiles.insert_grid_tile(cells)
    });
    tabs.push(tiles.insert_pane(gen_view()));

    let root = tiles.insert_tab_tile(tabs);

    let tree = egui_tiles::Tree::new("my_tree", root, tiles);
    (tree, next_view_nr)
}

impl MyApp {
    fn fps_history_default() -> egui::emath::History<f32> {
        egui::emath::History::new(5..120, 1.5)
    }

    fn add_pane_to_tabs(&mut self, tabs_id: egui_tiles::TileId) {
        let pane_nr = self.next_pane_nr;
        self.next_pane_nr += 1;

        let pane_id = self.tree.tiles.insert_pane(Pane::with_nr(pane_nr));
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            self.tree.tiles.get_mut(tabs_id)
        {
            tabs.add_child(pane_id);
            tabs.set_active(pane_id);
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
        let pane_id = self.tree.tiles.insert_pane(Pane::with_nr(pane_nr));

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
                    self.tree.root = Some(pane_id);
                }
            }
            None => {
                self.tree.root = Some(pane_id);
            }
        }
    }
}
