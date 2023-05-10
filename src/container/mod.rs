use egui::Rect;

use crate::Tree;

use super::{Behavior, DropContext, SimplifyAction, TileId, Tiles};

mod grid;
mod linear;
mod tabs;

pub use grid::{Grid, GridLoc};
pub use linear::{Linear, LinearDir, Shares};
pub use tabs::Tabs;

// ----------------------------------------------------------------------------

/// The layout of a [`Container`].
///
/// This is used to describe a [`Container`], and to change it to a different layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Layout {
    #[default]
    Tabs,
    Horizontal,
    Vertical,
    Grid,
}

impl Layout {
    pub const ALL: [Self; 4] = [Self::Tabs, Self::Horizontal, Self::Vertical, Self::Grid];
}

// ----------------------------------------------------------------------------

/// A container of several [`super::Tile`]s.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Container {
    Tabs(Tabs),
    Linear(Linear),
    Grid(Grid),
}

impl From<Tabs> for Container {
    #[inline]
    fn from(tabs: Tabs) -> Self {
        Self::Tabs(tabs)
    }
}

impl From<Linear> for Container {
    #[inline]
    fn from(linear: Linear) -> Self {
        Self::Linear(linear)
    }
}

impl From<Grid> for Container {
    #[inline]
    fn from(grid: Grid) -> Self {
        Self::Grid(grid)
    }
}

impl Container {
    pub fn new(layout: Layout, children: Vec<TileId>) -> Self {
        match layout {
            Layout::Tabs => Self::new_tabs(children),
            Layout::Horizontal => Self::new_horizontal(children),
            Layout::Vertical => Self::new_vertical(children),
            Layout::Grid => Self::new_grid(children),
        }
    }

    pub fn new_linear(dir: LinearDir, children: Vec<TileId>) -> Self {
        Self::Linear(Linear::new(dir, children))
    }

    pub fn new_horizontal(children: Vec<TileId>) -> Self {
        Self::new_linear(LinearDir::Horizontal, children)
    }

    pub fn new_vertical(children: Vec<TileId>) -> Self {
        Self::new_linear(LinearDir::Vertical, children)
    }

    pub fn new_tabs(children: Vec<TileId>) -> Self {
        Self::Tabs(Tabs::new(children))
    }

    pub fn new_grid(children: Vec<TileId>) -> Self {
        Self::Grid(Grid::new(children))
    }

    pub fn is_empty(&self) -> bool {
        self.children().is_empty()
    }

    pub fn children(&self) -> &[TileId] {
        match self {
            Self::Tabs(tabs) => &tabs.children,
            Self::Linear(linear) => &linear.children,
            Self::Grid(grid) => &grid.children,
        }
    }

    pub fn add_child(&mut self, child: TileId) {
        match self {
            Self::Tabs(tabs) => tabs.add_child(child),
            Self::Linear(linear) => linear.add_child(child),
            Self::Grid(grid) => grid.add_child(child),
        }
    }

    pub fn layout(&self) -> Layout {
        match self {
            Self::Tabs(_) => Layout::Tabs,
            Self::Linear(linear) => match linear.dir {
                LinearDir::Horizontal => Layout::Horizontal,
                LinearDir::Vertical => Layout::Vertical,
            },
            Self::Grid(_) => Layout::Grid,
        }
    }

    pub fn set_layout(&mut self, layout: Layout) {
        if layout == self.layout() {
            return;
        }

        *self = match layout {
            Layout::Tabs => Self::Tabs(Tabs::new(self.children().to_vec())),
            Layout::Horizontal => {
                Self::Linear(Linear::new(LinearDir::Horizontal, self.children().to_vec()))
            }
            Layout::Vertical => {
                Self::Linear(Linear::new(LinearDir::Vertical, self.children().to_vec()))
            }
            Layout::Grid => Self::Grid(Grid::new(self.children().to_vec())),
        };
    }

    pub(super) fn retain(&mut self, mut retain: impl FnMut(TileId) -> bool) {
        let retain = |tile_id: &TileId| retain(*tile_id);
        match self {
            Self::Tabs(tabs) => tabs.children.retain(retain),
            Self::Linear(linear) => linear.children.retain(retain),
            Self::Grid(grid) => grid.children.retain(retain),
        }
    }

    pub(super) fn simplify_children(&mut self, simplify: impl FnMut(TileId) -> SimplifyAction) {
        match self {
            Self::Tabs(tabs) => tabs.simplify_children(simplify),
            Self::Linear(linear) => linear.simplify_children(simplify),
            Self::Grid(grid) => grid.simplify_children(simplify),
        }
    }

    pub(super) fn layout_recursive<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        if self.is_empty() {
            return;
        }

        match self {
            Container::Tabs(tabs) => tabs.layout(tiles, style, behavior, rect),
            Container::Linear(linear) => {
                linear.layout(tiles, style, behavior, rect);
            }
            Container::Grid(grid) => grid.layout(tiles, style, behavior, rect),
        }
    }

    pub(super) fn ui<Pane>(
        &mut self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &mut egui::Ui,
        rect: Rect,
        tile_id: TileId,
    ) {
        match self {
            Container::Tabs(tabs) => {
                tabs.ui(tree, behavior, drop_context, ui, rect, tile_id);
            }
            Container::Linear(linear) => {
                linear.ui(tree, behavior, drop_context, ui, tile_id);
            }
            Container::Grid(grid) => {
                grid.ui(tree, behavior, drop_context, ui, tile_id);
            }
        }
    }
}
