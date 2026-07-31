//! A tab bar narrower than its tabs hides them behind a scroll arrow, which is the one thing a tab
//! bar is meant not to do. [`Behavior::max_tab_bar_rows`] lets it break into further rows instead.
//!
//! The rows are measured a frame late — the layout pass runs before anything knows how wide a tab
//! turned out — so these tests run several frames before looking.

use std::{cell::Cell, rc::Rc};

use egui_kittest::{Harness, kittest::Queryable as _};
use egui_tiles::{Behavior, Container, Tabs, Tile, TileId, Tiles, Tree, UiResponse};

struct Panes {
    max_rows: usize,
    right_ui_width: f32,
    right_ui_calls: Rc<Cell<usize>>,
}

impl Panes {
    fn new(max_rows: usize) -> Self {
        Self {
            max_rows,
            right_ui_width: 16.0,
            right_ui_calls: Rc::default(),
        }
    }
}

impl Behavior<String> for Panes {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut String) -> UiResponse {
        ui.label(format!("inside {pane}"));
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &String) -> egui::WidgetText {
        pane.clone().into()
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<String>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        _tabs: &Tabs,
        _scroll_offset: &mut f32,
    ) {
        self.right_ui_calls.set(self.right_ui_calls.get() + 1);
        ui.add_sized(
            egui::vec2(self.right_ui_width, ui.available_height()),
            egui::Label::new("➕"),
        );
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
    let mut behavior = Panes::new(max_rows);
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

/// The frame in which wrapping is measured but does not fit falls back to the scrolling row, and
/// only one of the two may draw the widgets the caller put in the bar.
#[test]
fn the_bar_is_drawn_by_one_path_per_frame() {
    let mut tree = tree();
    let mut behavior = Panes::new(3);
    let calls = Rc::clone(&behavior.right_ui_calls);
    let most_calls_in_a_frame = Rc::new(Cell::new(0));
    let watched = Rc::clone(&most_calls_in_a_frame);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(220.0, 300.0))
        .build_ui(|ui| {
            calls.set(0);
            tree.ui(&mut behavior, ui);
            watched.set(watched.get().max(calls.get()));
        });
    for _ in 0..4 {
        harness.run();
    }

    assert_eq!(
        most_calls_in_a_frame.get(),
        1,
        "top_bar_right_ui should run once per frame, not once per layout attempt"
    );
}

/// Wrapping plans its rows from what the previous frame measured, so the width it leaves for
/// [`Behavior::top_bar_right_ui`] has to survive the trip through that memory.
#[test]
fn wrapped_tabs_leave_room_for_the_top_bar_right_ui() {
    let mut tree = tree();
    let mut behavior = Panes {
        right_ui_width: 80.0,
        ..Panes::new(4)
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(220.0, 300.0))
        .build_ui(|ui| {
            tree.ui(&mut behavior, ui);
        });
    for _ in 0..4 {
        harness.run();
    }

    let right_ui = harness
        .query_by_label("➕")
        .expect("the top bar right ui should be drawn")
        .rect();
    for title in TITLES {
        let tab = harness
            .query_by_label(title)
            .expect("every tab should be drawn")
            .rect();
        assert!(
            tab.right() <= right_ui.left() + 0.5,
            "{title} ends at {}, overlapping the top bar right ui at {}",
            tab.right(),
            right_ui.left()
        );
    }
}

#[test]
fn every_tab_is_reachable_once_the_bar_wraps() {
    let mut tree = tree();
    let mut behavior = Panes::new(3);
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
