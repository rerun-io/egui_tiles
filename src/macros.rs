/// Simplifies the creation of a tile layout.
///
/// Takes in a [`Tiles<Pane>`](crate::Tiles) followed by a tile layout.
///
/// This macro will yield a code block that returns the [`TileId`](crate::TileId) of the root
/// [`Tile`](crate::Tile).
///
/// # Syntax
///
/// `layout_helper!` expects a tiles struct as the first argument, followed by a user defined
/// layout. This layout can be made up of 5 components:
/// + `row`: Represents a [`Linear`](crate::Linear) with a [`LinearDir::Horizontal`](crate::LinearDir).
/// + `col`: Represents a [`Linear`](crate::Linear) with a [`LinearDir::Vertical`](crate::LinearDir).
/// + `tabs`: Represents a [`Tabs`](crate::Tabs).
/// + `grid`: Represents a [`Grid`](crate::Grid).
/// + any [`TileId`](crate::TileId): any previously defined tile that should be inserted at that location.
///
/// Each component that is `row`, `col`, or `tabs` *must** be followed by a curly brace block
/// containing that tile's children, in order.
///
/// Each component that is `grid` must be followed by a curly brace block, which will contain
/// the following, in order of appearance:
/// + (Optional) `auto`, which forces the grid to use [`GridLayout::Auto`](crate::GridLayout).
/// `columns:` followed by a [`usize`], which forces the grid to use
/// [`GridLayout::Columns(usize)`](crate::GridLayout), with the given number of columns. If
/// omitted, the grid will default to using [`GridLayout::Columns(usize)`] where the number of
/// columns is decided by the largest child set.
/// + (Optional) `shares:` which must be followed by a curly brace block, which can contain the
/// following in any order:
///   + (Optional) `col:` which must be followed by an array of [`f32`], denoting the `col_shares`
///   + (Optional) `row:` which must be followed by an array of [`f32`], denoting the `row_shares`
/// + Any number of comma separated arrays of children. The largest set of children is used to
/// determine the number of columns automatically. If the number of children on a row is fewer than
/// the number of columns, unfortunately the empty space will not be left empty, but will instead be
/// filled from the next set of children. If the number of columns is set to `auto`, only one set of
/// children may be present.
///
/// Each component that is a *direct* child of a `row` or `col` may *optionally* be followed by an
/// [`f32`] surrounded by square brackets, which denotes that tile's default share. The
/// share **must** be placed *before* anything else, immediately after the component name. If not
/// included, the tile's share defaults to `1.0`.
///
/// The first *direct* child of a `tabs` is always the default active tab.
///
///
/// # Examples
/// ```
/// # #[derive(Default)]
/// # struct Pane {}
/// #
/// # fn create_tree () {
///     let mut tiles = egui_tiles::Tiles::default();
///     let pane_1 = tiles.insert_pane(Pane::default());
///     let pane_2 = tiles.insert_pane(Pane::default());
/// #   let pane_3 = tiles.insert_pane(Pane::default());
/// #   let pane_4 = tiles.insert_pane(Pane::default());
/// #   let pane_5 = tiles.insert_pane(Pane::default());
/// #   let pane_6 = tiles.insert_pane(Pane::default());
/// #   let pane_7 = tiles.insert_pane(Pane::default());
/// #   let pane_8 = tiles.insert_pane(Pane::default());
///     // ...
///     let pane_9 = tiles.insert_pane(Pane::default());
///
///     let root = egui_tiles::layout_helper!(tiles, // Tiles<Pane>
///         tabs { // Tabs, no share value since root tile
///             row { // Linear (Horizontal), active tab
///                 col [2.0] { // Linear (Vertical), with a share of 2.0 in the row
///                     pane_1 [2.0], // Pane with a share of 2.0 in the column
///                     pane_2 // Pane with a share of 1.0 in the column (default)
///                 },
///                 pane_3 [0.7], // Pane with a share of 0.7 in the row (default)
///                 tabs { // Tabs with a share of 1.0 in the row (default)
///                     pane_4, // Pane, active tab
///                     grid { // Grid
///                         shares: {
///                             col: [1.0, 2.0], // col_shares of grid
///                             row: [2.0, 3.0] // row_shares of grid
///                         },
///                         [pane_5, pane_6],
///                         [pane_7, pane_8],
///                     }
///                 }
///             },
///             pane_9,
///         }
///     );
/// }
/// ```
#[macro_export]
macro_rules! layout_helper {
    (@main $tiles:ident, tabs, $($item:ident $({ $($block:tt)*})? $(,)?)+) => {{
        let mut tabs = egui_tiles::Tabs::default();
        $(
            $crate::layout_helper!(@tabs_code $tiles, tabs, $item $(, $($block)*)?);
        )+
        $tiles.insert_new(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs)))
    }};

    (@main $tiles:ident, grid, auto, $(shares: {$($dim_1:ident: [$($dim_1_vals:expr $(,)?)*] $(,)? $($dim_2:ident: [$($dim_2_vals:expr $(,)?)*]$(,)?)?)?},)? [$($item:ident $({ $($block:tt)*})? $(,)?)+]$(,)?) => {{
        let mut grid = egui_tiles::Grid::default();

        $crate::layout_helper!(@grid_setup $tiles, grid, $($($dim_1: [$($dim_1_vals,)*] , $($dim_2: [$($dim_2_vals,)*])?)?)?);

        grid.layout = egui_tiles::GridLayout::Auto;

        $(
            $crate::layout_helper!(@grid_code $tiles, grid, $item $(, $($block)*)?);
        )+

        $tiles.insert_new(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid)))
    }};

    (@main $tiles:ident, grid, columns: $col_count:expr, $(shares: {$($dim_1:ident: [$($dim_1_vals:expr $(,)?)*] $(,)? $($dim_2:ident: [$($dim_2_vals:expr $(,)?)*]$(,)?)?)?},)? [$($item:ident $({ $($block:tt)*})? $(,)?)+]$(,)?) => {{
        let mut grid = egui_tiles::Grid::default();

        $crate::layout_helper!(@grid_setup $tiles, grid, $($($dim_1: [$($dim_1_vals,)*] , $($dim_2: [$($dim_2_vals,)*])?)?)?);

        grid.layout = egui_tiles::GridLayout::Columns($col_count);

        $(
            $crate::layout_helper!(@grid_code $tiles, grid, $item $(, $($block)*)?);
        )+

        $tiles.insert_new(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid)))
    }};

    (@main $tiles:ident, grid, $(shares: {$($dim_1:ident: [$($dim_1_vals:expr $(,)?)*] $(,)? $($dim_2:ident: [$($dim_2_vals:expr $(,)?)*]$(,)?)?)?},)? $([$($item:ident $({ $($block:tt)*})? $(,)?)+]$(,)?)+) => {{
        let mut grid = egui_tiles::Grid::default();

        $crate::layout_helper!(@grid_setup $tiles, grid, $($($dim_1: [$($dim_1_vals,)*] , $($dim_2: [$($dim_2_vals,)*])?)?)?);

        // This get optimized to a single constant value during compilation
        let col_count = vec![$(
            $crate::layout_helper!(@total $($item,)+),
        )*]
        .iter()
        .fold(0, |a, b| if a > *b { a } else { *b });

        grid.layout = egui_tiles::GridLayout::Columns(col_count);

        $(
            $(
                $crate::layout_helper!(@grid_code $tiles, grid, $item $(, $($block)*)?);
            )+
        )+

        $tiles.insert_new(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid)))
    }};

    (@main $tiles:ident, $contain:ident, $($item:ident $([$share:expr])? $({ $($block:tt)*})? $(,)?)+) => {{
        let mut linear = egui_tiles::Linear {
            dir: $crate::layout_helper!(@dir $contain),
            ..Default::default()
        };
        $(
            $crate::layout_helper!(@lin_code $tiles, linear, $item, [$($share)?] $(, $($block)*)?);
        )+
        $tiles.insert_new(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear)))
    }};

    (@tabs_code $tiles:ident, $tabs:ident, $item:ident) => {
        $tabs.add_child($item);
    };

    (@tabs_code $tiles:ident, $tabs:ident, $item:ident, $($block:tt)*) => {
        let id = $crate::layout_helper!(@main $tiles, $item, $($block)*);
        $tabs.add_child(id);
    };

    (@lin_code $tiles:ident, $linear:ident, $item:ident, [$($share:expr)?]) => {
        $linear.add_child($item);
        $linear.shares.set_share($item, $crate::layout_helper!(@share $($share)?));
    };

    (@lin_code $tiles:ident, $linear:ident, $item:ident, [$($share:expr)?], $($block:tt)*) => {
        let id = $crate::layout_helper!(@main $tiles, $item, $($block)*);
        $linear.add_child(id);
        $linear.shares.set_share(id, $crate::layout_helper!(@share $($share)?));
    };

    (@grid_setup $tiles:ident, $grid:ident, $($dim_1:ident: [$($dim_1_vals:expr $(,)?)*] , $($dim_2:ident: [$($dim_2_vals:expr $(,)?)*]$(,)?)?)?) => {
        $(
            $crate::layout_helper!(@grid_share $dim_1, $grid) = vec![$($dim_1_vals,)*];
            $(
                $crate::layout_helper!(@grid_share $dim_2 after $dim_1, $grid) = vec![$($dim_2_vals,)*];
            )?
        )?
    };

    (@grid_code $tiles:ident, $grid:ident, $item:ident) => {
        $grid.add_child($item);
    };

    (@grid_code $tiles:ident, $grid:ident, $item:ident, $($block:tt)*) => {
        let id = $crate::layout_helper!(@main $tiles, $item, $($block)*);
        $grid.add_child(id);
    };

    (@total $item:ident, $($more:ident,)*) => {
        1 + $crate::layout_helper!(@total $($more,)*)
    };

    (@total) => {
        0
    };

    (@share) => {
        1.0
    };

    (@share $share:expr) => {
        $share
    };

    (@dir row) => {
        egui_tiles::LinearDir::Horizontal
    };
    (@dir col) => {
        egui_tiles::LinearDir::Vertical
    };

    (@grid_share col, $grid:ident) => {
        $grid.col_shares
    };

    (@grid_share row, $grid:ident) => {
        $grid.row_shares
    };

    (@grid_share row after col, $grid:ident) => {
        $grid.row_shares
    };

    (@grid_share col after row, $grid:ident) => {
        $grid.col_shares
    };

    ($tiles:ident, $first_con:ident{$($body:tt)*}) => {{
        $crate::layout_helper!(@main $tiles, $first_con, $($body)*)
    }};
}
