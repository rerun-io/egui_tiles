use egui_tiles::{
    Behavior, Container, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree,
    UiResponse,
};

struct TestBehavior;

impl Behavior<()> for TestBehavior {
    fn pane_ui(&mut self, _ui: &mut egui::Ui, _tile_id: TileId, _pane: &mut ()) -> UiResponse {
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, _pane: &()) -> egui::WidgetText {
        "pane".into()
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions::OFF
    }
}

fn run_tree_ui(ctx: &egui::Context, tree: &mut Tree<()>, behavior: &mut impl Behavior<()>) {
    ctx.begin_pass(Default::default());
    egui::CentralPanel::default().show(ctx, |ui| {
        tree.ui(behavior, ui);
    });
    let _ = ctx.end_pass();
}

#[test]
fn new_floating_panes_restore_tiled_geometry() {
    let mut tiles = Tiles::default();
    let pane_a = tiles.insert_pane(());
    let pane_b = tiles.insert_pane(());
    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Horizontal,
        vec![pane_a, pane_b],
    )));

    let mut tree = Tree::new("floating_restore", root, tiles);
    let mut behavior = TestBehavior;
    let ctx = egui::Context::default();

    // Start in floating mode and run one frame to populate floating positions.
    tree.set_floating(true);
    run_tree_ui(&ctx, &mut tree, &mut behavior);

    // Add new panes while in floating mode.
    let mut new_panes = Vec::new();
    for offset in [egui::vec2(123.0, -77.0), egui::vec2(-45.0, 96.0)] {
        let pane_id = tree.tiles.insert_pane(());
        let Tile::Container(Container::Linear(linear)) = tree
            .tiles
            .get_mut(root)
            .expect("root container should exist")
        else {
            panic!("expected linear container");
        };
        linear.add_child(pane_id);

        run_tree_ui(&ctx, &mut tree, &mut behavior);

        let rect = tree
            .floating_positions
            .get(&pane_id)
            .copied()
            .expect("pane should have a floating rect after ui");
        tree.floating_positions
            .insert(pane_id, rect.translate(offset));

        run_tree_ui(&ctx, &mut tree, &mut behavior);
        new_panes.push(pane_id);
    }

    // Switch to tiled mode and lay out the tree to obtain rects.
    tree.set_floating(false);
    run_tree_ui(&ctx, &mut tree, &mut behavior);

    let expected_rects: Vec<(TileId, egui::Rect)> = new_panes
        .iter()
        .map(|&pane_id| {
            let rect = tree
                .tiles
                .rect(pane_id)
                .expect("pane should have a rect in tiled mode");
            (pane_id, rect)
        })
        .collect();

    // Return to floating mode.
    tree.set_floating(true);

    // After running the floating UI pass, the areas should adopt the tiled rects.
    run_tree_ui(&ctx, &mut tree, &mut behavior);

    fn rects_close(a: egui::Rect, b: egui::Rect) -> bool {
        let eps = 1e-3;
        (a.min.x - b.min.x).abs() <= eps
            && (a.min.y - b.min.y).abs() <= eps
            && (a.max.x - b.max.x).abs() <= eps
            && (a.max.y - b.max.y).abs() <= eps
    }

    for (pane_id, expected_rect) in expected_rects {
        let actual_rect = tree
            .floating_positions
            .get(&pane_id)
            .copied()
            .expect("pane should have a floating position");

        if !rects_close(actual_rect, expected_rect) {
            let diff_min = actual_rect.min - expected_rect.min;
            let diff_max = actual_rect.max - expected_rect.max;
            panic!(
                "rect mismatch: pane {:?}, expected {:?}, actual {:?}, diff_min {:?}, diff_max {:?}",
                pane_id, expected_rect, actual_rect, diff_min, diff_max
            );
        }
    }
}
