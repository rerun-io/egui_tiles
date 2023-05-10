#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        "egui_tile_tree example",
        options,
        Box::new(|cc| {
            let mut app = MyApp::default();
            if let Some(storage) = cc.storage {
                if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                    app = state;
                }
            }
            Box::new(app)
        }),
    )
}

#[derive(serde::Deserialize, serde::Serialize)]
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

    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui_tile_tree::UiResponse {
        let color = egui::epaint::Hsva::new(0.103 * self.nr as f32, 0.5, 0.5, 1.0);
        ui.painter().rect_filled(ui.max_rect(), 0.0, color);
        let dragged = ui
            .allocate_rect(ui.max_rect(), egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dragged();
        if dragged {
            egui_tile_tree::UiResponse::DragStarted
        } else {
            egui_tile_tree::UiResponse::None
        }
    }
}

struct TreeBehavior {
    simplification_options: egui_tile_tree::SimplificationOptions,
    tab_bar_height: f32,
    gap_width: f32,
    add_child_to: Option<egui_tile_tree::TileId>,
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
                    &mut simplification_options.join_nested_linear_containerss,
                    "",
                );
                ui.end_row();

                ui.label("Tab bar height:");
                ui.add(
                    egui::DragValue::new(tab_bar_height)
                        .clamp_range(0.0..=100.0)
                        .speed(1.0),
                );
                ui.end_row();

                ui.label("Gap width:");
                ui.add(
                    egui::DragValue::new(gap_width)
                        .clamp_range(0.0..=20.0)
                        .speed(1.0),
                );
                ui.end_row();
            });
    }
}

impl egui_tile_tree::Behavior<Pane> for TreeBehavior {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tile_tree::TileId,
        view: &mut Pane,
    ) -> egui_tile_tree::UiResponse {
        view.ui(ui)
    }

    fn tab_title_for_pane(&mut self, view: &Pane) -> egui::WidgetText {
        format!("View {}", view.nr).into()
    }

    fn top_bar_rtl_ui(
        &mut self,
        _tiles: &egui_tile_tree::Tiles<Pane>,
        ui: &mut egui::Ui,
        tile_id: egui_tile_tree::TileId,
        _tabs: &egui_tile_tree::Tabs,
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

    fn simplification_options(&self) -> egui_tile_tree::SimplificationOptions {
        self.simplification_options
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct MyApp {
    tree: egui_tile_tree::Tree<Pane>,

    #[serde(skip)]
    behavior: TreeBehavior,

    #[serde(skip)]
    last_tree_debug: String,
}

impl Default for MyApp {
    fn default() -> Self {
        let mut next_view_nr = 0;
        let mut gen_view = || {
            let view = Pane::with_nr(next_view_nr);
            next_view_nr += 1;
            view
        };

        let mut tiles = egui_tile_tree::Tiles::default();

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

        let tree = egui_tile_tree::Tree::new(root, tiles);

        Self {
            tree,
            behavior: Default::default(),
            last_tree_debug: Default::default(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("tree").show(ctx, |ui| {
            if ui.button("Reset").clicked() {
                *self = Default::default();
            }
            self.behavior.ui(ui);
            ui.separator();

            tree_ui(ui, &mut self.behavior, &mut self.tree.tiles, self.tree.root);

            if let Some(parent) = self.behavior.add_child_to.take() {
                let new_child = self.tree.tiles.insert_pane(Pane::with_nr(100));
                if let Some(egui_tile_tree::Tile::Container(egui_tile_tree::Container::Tabs(
                    tabs,
                ))) = self.tree.tiles.get_mut(parent)
                {
                    tabs.add_child(new_child);
                    tabs.set_active(new_child);
                }
            }

            ui.separator();
            ui.style_mut().wrap = Some(false);
            let tree_debug = format!("{:#?}", self.tree);
            ui.monospace(&tree_debug);
            if self.last_tree_debug != tree_debug {
                self.last_tree_debug = tree_debug;
                log::debug!("{}", self.last_tree_debug);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.tree.ui(&mut self.behavior, ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self);
    }
}

fn tree_ui(
    ui: &mut egui::Ui,
    behavior: &mut dyn egui_tile_tree::Behavior<Pane>,
    tiles: &mut egui_tile_tree::Tiles<Pane>,
    tile_id: egui_tile_tree::TileId,
) {
    // Get the name BEFORE we remove the tile below!
    let text = format!(
        "{} - {tile_id:?}",
        behavior.tab_title_for_tile(tiles, tile_id).text()
    );

    let Some(mut tile) = tiles.tiles.remove(&tile_id) else {
        log::warn!("Missing tile {tile_id:?}");
        return;
    };

    egui::CollapsingHeader::new(text)
        .id_source((tile_id, "tree"))
        .default_open(true)
        .show(ui, |ui| match &mut tile {
            egui_tile_tree::Tile::Pane(_) => {}
            egui_tile_tree::Tile::Container(container) => {
                let mut layout = container.layout();
                egui::ComboBox::from_label("Layout")
                    .selected_text(format!("{layout:?}"))
                    .show_ui(ui, |ui| {
                        for typ in egui_tile_tree::Layout::ALL {
                            ui.selectable_value(&mut layout, typ, format!("{typ:?}"))
                                .clicked();
                        }
                    });
                if layout != container.layout() {
                    container.set_layout(layout);
                }

                for &child in container.children() {
                    tree_ui(ui, behavior, tiles, child);
                }
            }
        });

    tiles.tiles.insert(tile_id, tile);
}
