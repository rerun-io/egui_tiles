use egui_tiles::{Container, Grid, GridLayout, Linear, LinearDir, Tabs, Tile, Tiles, Tree};

#[derive(Debug, PartialEq, Default)]
struct Pane {}

#[test]
/// Testing for the [`layout_helper!`](egui_tiles::layout_helper) macro
///
/// Dev note: I mostly just added a seemingly random combination of tiles to test everything
/// at once. Feel free to modify and add more tests to be a bit more methodical about testing
/// every aspect of the macro.
fn test_layout_helper() {
    let (macro_tiles, macro_root) = {
        let mut tiles = Tiles::<Pane>::default();

        let pane_1 = tiles.insert_pane(Pane::default());
        let pane_2 = tiles.insert_pane(Pane::default());
        let pane_3 = tiles.insert_pane(Pane::default());
        let pane_4 = tiles.insert_pane(Pane::default());
        let pane_5 = tiles.insert_pane(Pane::default());
        let pane_6 = tiles.insert_pane(Pane::default());
        let pane_7 = tiles.insert_pane(Pane::default());
        let pane_8 = tiles.insert_pane(Pane::default());
        let pane_9 = tiles.insert_pane(Pane::default());

        let root = egui_tiles::layout_helper!(tiles,
            tabs {
                row {
                    col [2.0] {
                        pane_1 [2.0],
                        pane_2
                    },
                    pane_3 [0.7],
                    tabs {
                        pane_4,
                        grid {
                            shares: {
                                col: [1.0, 2.0],
                                row: [2.0, 3.0]
                            },
                            [pane_5, pane_6],
                            [pane_7, pane_8],
                        }
                    }
                },
                pane_9,
            }
        );

        (tiles, root)
    };

    let (verify_tiles, verify_root) = {
        let mut tiles = Tiles::<Pane>::default();

        let pane_1 = tiles.insert_pane(Pane::default());
        let pane_2 = tiles.insert_pane(Pane::default());
        let pane_3 = tiles.insert_pane(Pane::default());
        let pane_4 = tiles.insert_pane(Pane::default());
        let pane_5 = tiles.insert_pane(Pane::default());
        let pane_6 = tiles.insert_pane(Pane::default());
        let pane_7 = tiles.insert_pane(Pane::default());
        let pane_8 = tiles.insert_pane(Pane::default());
        let pane_9 = tiles.insert_pane(Pane::default());

        let mut tabs_1 = Tabs::default();

        let mut row = Linear {
            dir: LinearDir::Horizontal,
            ..Default::default()
        };

        let mut col = Linear {
            dir: LinearDir::Vertical,
            ..Default::default()
        };

        col.add_child(pane_1);
        col.add_child(pane_2);

        col.shares.set_share(pane_1, 2.0);
        col.shares.set_share(pane_2, 1.0);

        let col_id = tiles.insert_new(Tile::Container(Container::Linear(col)));

        let mut tabs_2 = Tabs::default();

        tabs_2.add_child(pane_4);

        let mut grid = Grid::default();

        grid.layout = GridLayout::Columns(2);

        grid.col_shares = vec![1.0, 2.0];
        grid.row_shares = vec![2.0, 3.0];

        grid.add_child(pane_5);
        grid.add_child(pane_6);
        grid.add_child(pane_7);
        grid.add_child(pane_8);

        let grid_id = tiles.insert_new(Tile::Container(Container::Grid(grid)));

        tabs_2.add_child(grid_id);

        let tabs_2_id = tiles.insert_new(Tile::Container(Container::Tabs(tabs_2)));

        row.add_child(col_id);
        row.add_child(pane_3);
        row.add_child(tabs_2_id);

        row.shares.set_share(col_id, 2.0);
        row.shares.set_share(pane_3, 0.7);
        row.shares.set_share(tabs_2_id, 1.0);

        let row_id = tiles.insert_new(Tile::Container(Container::Linear(row)));

        tabs_1.add_child(row_id);
        tabs_1.add_child(pane_9);

        let root = tiles.insert_new(Tile::Container(Container::Tabs(tabs_1)));

        (tiles, root)
    };

    assert_eq!(macro_tiles, verify_tiles);

    let macro_tree = Tree::new("tree_name", macro_root, macro_tiles);
    let verify_tree = Tree::new("tree_name", verify_root, verify_tiles);

    assert_eq!(macro_tree, verify_tree);
}
