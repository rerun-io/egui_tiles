#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::allow_attributes, unused_mut, clippy::useless_let_if_seq)]

//! Example demonstrating `is_drop_allowed` hook.
//! Each pane kind has different drop restrictions.

use eframe::egui;
use egui_tiles::{ContainerKind, TileId, Tiles};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_tiles — drop filter demo",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

#[derive(Clone, Debug)]
enum PaneKind {
    /// Can be dropped anywhere, including splits
    Free,
    /// Only into existing tabs, no splits
    TabsOnly,
    /// Cannot be dropped into the restricted container
    NoRight,
    /// Not draggable at all
    Pinned,
}

#[derive(Clone, Debug)]
struct Pane {
    nr: usize,
    kind: PaneKind,
}

impl Pane {
    fn color(&self) -> egui::Color32 {
        match self.kind {
            PaneKind::Free => egui::Color32::from_rgb(60, 140, 60),
            PaneKind::TabsOnly => egui::Color32::from_rgb(60, 100, 160),
            PaneKind::NoRight => egui::Color32::from_rgb(160, 100, 40),
            PaneKind::Pinned => egui::Color32::from_rgb(140, 60, 60),
        }
    }

    fn tag(&self) -> &'static str {
        match self.kind {
            PaneKind::Free => "free",
            PaneKind::TabsOnly => "tabs-only",
            PaneKind::NoRight => "no-right",
            PaneKind::Pinned => "pinned",
        }
    }

    fn label(&self) -> String {
        format!("Pane {} ({})", self.nr, self.tag())
    }

    fn rules(&self) -> &'static [&'static str] {
        match self.kind {
            PaneKind::Free => &["Drop anywhere, including splits"],
            PaneKind::TabsOnly => &[
                "Only into existing tabs",
                "No horizontal/vertical splits",
            ],
            PaneKind::NoRight => &[
                "Cannot drop into the right container",
                "Splits and tabs in left — OK",
            ],
            PaneKind::Pinned => &["Cannot be dragged at all"],
        }
    }
}

struct TreeBehavior {
    restricted_container: TileId,
}

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.label().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        ui.painter().rect_filled(ui.max_rect(), 0.0, pane.color());
        let text_color = egui::Color32::WHITE;
        ui.label(egui::RichText::new(pane.label()).color(text_color).strong());
        for rule in pane.rules() {
            ui.label(egui::RichText::new(*rule).color(text_color));
        }

        // Pinned — not draggable, no drag sense
        if matches!(pane.kind, PaneKind::Pinned) {
            return egui_tiles::UiResponse::None;
        }

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

    fn is_drop_allowed(
        &self,
        tiles: &Tiles<Pane>,
        dragged_tile_id: TileId,
        insertion: &egui_tiles::InsertionPoint,
    ) -> bool {
        let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(dragged_tile_id) else {
            return true;
        };
        let kind = insertion.container_kind();
        match pane.kind {
            PaneKind::Free => true,
            PaneKind::TabsOnly => kind == ContainerKind::Tabs,
            PaneKind::NoRight => insertion.parent_id != self.restricted_container,
            // Pinned should never reach here (is_tile_draggable = false),
            // but just in case
            PaneKind::Pinned => false,
        }
    }

    fn is_tile_draggable(
        &self,
        tiles: &Tiles<Pane>,
        tile_id: TileId,
    ) -> bool {
        if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(tile_id) {
            !matches!(pane.kind, PaneKind::Pinned)
        } else {
            true
        }
    }
}

struct MyApp {
    tree: egui_tiles::Tree<Pane>,
    restricted_container: TileId,
}

impl Default for MyApp {
    fn default() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        // Left side: one of each kind
        let left_children: Vec<TileId> = vec![
            tiles.insert_pane(Pane { nr: 0, kind: PaneKind::Free }),
            tiles.insert_pane(Pane { nr: 1, kind: PaneKind::TabsOnly }),
            tiles.insert_pane(Pane { nr: 2, kind: PaneKind::NoRight }),
            tiles.insert_pane(Pane { nr: 3, kind: PaneKind::Pinned }),
        ];
        let left = tiles.insert_tab_tile(left_children);

        // Right side: Free + TabsOnly
        let right_children: Vec<TileId> = vec![
            tiles.insert_pane(Pane { nr: 4, kind: PaneKind::Free }),
            tiles.insert_pane(Pane { nr: 5, kind: PaneKind::TabsOnly }),
        ];
        let right = tiles.insert_tab_tile(right_children);

        let root = tiles.insert_horizontal_tile(vec![left, right]);
        let tree = egui_tiles::Tree::new("drop_filter_tree", root, tiles);

        Self {
            tree,
            restricted_container: right,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Drop filter demo");
        ui.label("Drag tabs between containers. Each pane type has different restrictions.");
        ui.separator();

        let mut behavior = TreeBehavior {
            restricted_container: self.restricted_container,
        };
        self.tree.ui(&mut behavior, ui);
    }
}
