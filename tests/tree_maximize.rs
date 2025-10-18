use egui_tiles::{Container, ContainerKind, GridLayout, Tile, TileId, Tiles, Tree};

#[test]
fn maximize_toggle_and_clear() {
    let mut tiles: Tiles<()> = Tiles::default();
    let pane_a = tiles.insert_pane(());
    let pane_b = tiles.insert_pane(());
    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Horizontal,
        vec![pane_a, pane_b],
    )));

    let mut tree = Tree::new("maximize_test", root, tiles);

    assert!(!tree.is_maximized());
    assert!(tree.maximize_tile(pane_a, true));
    assert!(tree.is_tile_maximized(pane_a));
    assert!(tree.is_maximized());

    assert!(!tree.maximize_tile(TileId::from_u64(999), true));

    assert!(tree.toggle_maximize(pane_b, true));
    assert!(tree.is_tile_maximized(pane_b));

    assert!(!tree.toggle_maximize(pane_b, true));
    assert!(!tree.is_maximized());

    assert!(tree.toggle_maximize(pane_b, true));
    assert!(tree.is_tile_maximized(pane_b));

    tree.clear_maximized();
    assert!(!tree.is_maximized());
}

#[test]
fn maximize_clears_when_tile_removed() {
    let mut tiles: Tiles<()> = Tiles::default();
    let pane_a = tiles.insert_pane(());
    let pane_b = tiles.insert_pane(());
    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Horizontal,
        vec![pane_a, pane_b],
    )));

    let mut tree = Tree::new("maximize_removed", root, tiles);
    assert!(tree.maximize_tile(pane_b, true));

    tree.remove_recursively(pane_b);
    assert!(!tree.is_maximized());
}

#[test]
fn maximize_works_in_floating_mode() {
    let mut tiles: Tiles<()> = Tiles::default();
    let pane = tiles.insert_pane(());
    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Horizontal,
        vec![pane],
    )));

    let mut tree = Tree::new("maximize_floating", root, tiles);
    tree.set_floating(true);
    assert!(tree.maximize_tile(pane, true));
    assert!(tree.is_tile_maximized(pane));
    tree.set_floating(false);
    assert!(tree.maximize_tile(pane, true));
}

#[test]
fn partial_maximize_adjusts_shares_along_path() {
    let mut tiles: Tiles<()> = Tiles::default();
    let top_left = tiles.insert_pane(());
    let bottom_left = tiles.insert_pane(());
    let left_container = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Vertical,
        vec![top_left, bottom_left],
    )));
    let right_pane = tiles.insert_pane(());

    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Horizontal,
        vec![left_container, right_pane],
    )));

    let mut tree = Tree::new("partial_maximize", root, tiles);

    let original_root_shares =
        if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(root) {
            linear.clone_shares()
        } else {
            ahash::HashMap::default()
        };
    let original_left_shares =
        if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(left_container) {
            linear.clone_shares()
        } else {
            ahash::HashMap::default()
        };

    assert!(tree.maximize_tile(bottom_left, false));

    if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(root) {
        assert!(linear.shares[left_container] > linear.shares[right_pane]);
    } else {
        panic!("Root container missing");
    }

    if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(left_container) {
        assert!(linear.shares[bottom_left] > linear.shares[top_left]);
    } else {
        panic!("Left container missing");
    }

    tree.clear_maximized();

    if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(root) {
        assert_eq!(linear.clone_shares(), original_root_shares);
    }
    if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(left_container) {
        assert_eq!(linear.clone_shares(), original_left_shares);
    }
}

#[test]
fn partial_maximize_handles_grid_holes() {
    let mut tiles: Tiles<()> = Tiles::default();
    let pane_a = tiles.insert_pane(());
    let pane_b = tiles.insert_pane(());
    let pane_c = tiles.insert_pane(());
    let pane_d = tiles.insert_pane(());
    let root = tiles.insert_new(Tile::Container(Container::new(
        ContainerKind::Grid,
        vec![pane_a, pane_b, pane_c, pane_d],
    )));

    let mut tree = Tree::new("partial_maximize_grid_holes", root, tiles);

    {
        let Some(Tile::Container(Container::Grid(grid))) = tree.tiles.get_mut(root) else {
            panic!("Root container missing");
        };
        grid.layout = GridLayout::Columns(2);
        grid.col_shares = vec![1.0, 1.0];
        grid.row_shares = vec![1.0, 1.0];
    }

    let expected_cols = vec![1.0, 1.0];
    let expected_rows = vec![1.0, 1.0];

    tree.remove_recursively(pane_a);

    assert!(tree.maximize_tile(pane_b, false));
    assert!(tree.is_tile_maximized(pane_b));

    if let Some(Tile::Container(Container::Grid(grid))) = tree.tiles.get(root) {
        assert_eq!(grid.col_shares.len(), 2);
        assert_eq!(grid.row_shares.len(), 2);
        assert_eq!(grid.col_shares[0], 0.1);
        assert_eq!(grid.col_shares[1], 10.0);
        assert_eq!(grid.row_shares[0], 10.0);
        assert_eq!(grid.row_shares[1], 0.1);
    } else {
        panic!("Root container missing after maximize");
    }

    assert!(tree.clear_maximized());
    assert!(!tree.is_maximized());

    if let Some(Tile::Container(Container::Grid(grid))) = tree.tiles.get(root) {
        assert_eq!(grid.col_shares, expected_cols);
        assert_eq!(grid.row_shares, expected_rows);
    } else {
        panic!("Root container missing after clear");
    }
}
