//! A tab bar narrower than its tabs hides them behind a scroll arrow, which is the one thing a tab
//! bar is meant not to do. [`Behavior::max_tab_bar_rows`] lets it break into further rows instead.
//!
//! The rows are measured a frame late — the layout pass runs before anything knows how wide a tab
//! turned out — so these tests run several frames before looking.

use std::cell::Cell;

use egui_kittest::{Harness, kittest::Queryable as _};
use egui_tiles::{Behavior, Container, Tile, TileId, Tiles, Tree, UiResponse};

struct Panes {
    max_rows: usize,
}

impl Behavior<String> for Panes {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut String) -> UiResponse {
        ui.label(format!("inside {pane}"));
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &String) -> egui::WidgetText {
        pane.clone().into()
    }

    fn max_tab_bar_rows(&self) -> usize {
        self.max_rows
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        24.0
    }
}

const TITLES: [&str; 6] = ["Sounds", "Layers", "Motifs", "Tunings", "Waves", "Imports"];

const ROW_HEIGHT: f32 = 24.0;

fn tree() -> Tree<String> {
    let mut tiles = Tiles::default();
    let panes = TITLES
        .iter()
        .map(|title| tiles.insert_pane((*title).to_owned()))
        .collect();
    let root = tiles.insert_tab_tile(panes);
    Tree::new("bar", root, tiles)
}

/// Runs the tree in a bar too narrow for one row, and returns how tall the tab bar came out.
fn tab_bar_height(max_rows: usize) -> f32 {
    let mut tree = tree();
    let mut behavior = Panes { max_rows };
    let height = Cell::new(0.0);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(220.0, 300.0))
        .build_ui(|ui| {
            tree.ui(&mut behavior, ui);
            let pane = tree.root.and_then(|root| match tree.tiles.get(root) {
                Some(Tile::Container(Container::Tabs(tabs))) => tabs.active,
                _ => None,
            });
            if let (Some(root), Some(pane)) = (tree.root, pane)
                && let (Some(bar), Some(pane)) = (tree.tiles.rect(root), tree.tiles.rect(pane))
            {
                height.set(pane.top() - bar.top());
            }
        });
    harness.run();
    harness.run();
    height.get()
}

#[test]
fn a_crowded_bar_wraps_into_further_rows() {
    let wrapped = tab_bar_height(3);
    assert!(
        wrapped >= ROW_HEIGHT * 2.0,
        "six tabs in a 220 px bar should take more than one row, got {wrapped}"
    );
}

#[test]
fn one_row_is_the_default_and_keeps_scrolling() {
    let height = tab_bar_height(1);
    assert!(
        (height - ROW_HEIGHT).abs() < 0.5,
        "an unwrapped bar should stay one row tall, got {height}"
    );
}

#[test]
fn every_tab_is_reachable_once_the_bar_wraps() {
    let mut tree = tree();
    let mut behavior = Panes { max_rows: 3 };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(220.0, 300.0))
        .build_ui(|ui| {
            tree.ui(&mut behavior, ui);
        });
    harness.run();
    harness.run();

    for title in TITLES {
        assert!(
            harness.query_by_label(title).is_some(),
            "{title} should be visible in a wrapped tab bar"
        );
    }
}
