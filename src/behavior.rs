use std::sync::mpsc::Sender;

use egui::{
    vec2, Color32, Id, Rect, Response, Rgba, Sense, Stroke, TextStyle, Ui, Visuals, WidgetText,
};

use super::{ResizeState, SimplificationOptions, Tile, TileId, Tiles, UiResponse};

/// Trait defining how the [`super::Tree`] and its panes should be shown.
pub trait Behavior<Pane> {
    /// Show a pane tile in the given [`egui::Ui`].
    ///
    /// You can make the pane draggable by returning [`UiResponse::DragStarted`]
    /// when the user drags some handle.
    fn pane_ui(&mut self, _ui: &mut Ui, _tile_id: TileId, _pane: &mut Pane) -> UiResponse;

    /// The title of a pane tab.
    fn tab_title_for_pane(&mut self, pane: &Pane) -> WidgetText;

    /// The title of a general tab.
    ///
    /// The default implementation calls [`Self::tab_title_for_pane`] for panes and
    /// uses the name of the [`crate::ContainerKind`] for [`crate::Container`]s.
    fn tab_title_for_tile(&mut self, tiles: &Tiles<Pane>, tile_id: TileId) -> WidgetText {
        if let Some(tile) = tiles.tiles.get(&tile_id) {
            match tile {
                Tile::Pane(pane) => self.tab_title_for_pane(pane),
                Tile::Container(container) => format!("{:?}", container.kind()).into(),
            }
        } else {
            "MISSING TILE".into()
        }
    }

    /// Show the ui for the a tab of some tile.
    ///
    /// The default implementation shows a clickable button with the title for that tile,
    /// gotten with [`Self::tab_title_for_tile`].
    /// The default implementation also calls [`Self::on_tab_button`].
    ///
    /// You can override the default implementation to add e.g. a close button.
    /// Make sure it is sensitive to clicks and drags (if you want to enable drag-and-drop of tabs).
    fn tab_ui(
        &mut self,
        tiles: &Tiles<Pane>,
        ui: &mut Ui,
        id: Id,
        tile_id: TileId,
        active: bool,
        is_being_dragged: bool,
    ) -> Response {
        let text = self.tab_title_for_tile(tiles, tile_id);
        let font_id = TextStyle::Button.resolve(ui.style());
        let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);

        let x_margin = self.tab_title_spacing(ui.visuals());
        let (_, rect) = ui.allocate_space(vec2(
            galley.size().x + 2.0 * x_margin,
            ui.available_height(),
        ));
        let response = ui.interact(rect, id, Sense::click_and_drag());

        // Show a gap when dragged
        if ui.is_rect_visible(rect) && !is_being_dragged {
            let bg_color = self.tab_bg_color(ui.visuals(), tile_id, active);
            let stroke = self.tab_outline_stroke(ui.visuals(), tile_id, active);
            ui.painter().rect(rect.shrink(0.5), 0.0, bg_color, stroke);

            if active {
                // Make the tab name area connect with the tab ui area:
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom(),
                    Stroke::new(stroke.width + 1.0, bg_color),
                );
            }

            let text_color = self.tab_text_color(ui.visuals(), tile_id, active);
            ui.painter().galley_with_color(
                egui::Align2::CENTER_CENTER
                    .align_size_within_rect(galley.size(), rect)
                    .min,
                galley.galley,
                text_color,
            );
        }

        self.on_tab_button(tiles, tile_id, &response);

        response
    }

    /// Called by the default implementation of [`Self::tab_ui`] for each added button
    fn on_tab_button(
        &mut self,
        _tiles: &Tiles<Pane>,
        _tile_id: TileId,
        _button_response: &Response,
    ) {
    }

    /// Return `false` if a given pane should be removed from its parent.
    fn retain_pane(&mut self, _pane: &Pane) -> bool {
        true
    }

    /// Adds some UI to the top right of each tab bar.
    ///
    /// You can use this to, for instance, add a button for adding new tabs.
    ///
    /// The widgets will be added right-to-left.
    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<Pane>,
        _ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &crate::Tabs,
        _offset: f32,
        _scroll: &mut f32,
    ) {
        // if ui.button("➕").clicked() {
        // }
    }

    fn top_bar_left_ui(
        &mut self,
        _tiles: &Tiles<Pane>,
        _ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &crate::Tabs,
        _offset: f32,
        _scroll: &mut f32,
    ) {
        // if ui.button("➕").clicked() {
        // }
    }

    /// The height of the bar holding tab titles.
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        24.0
    }

    /// Width of the gap between tiles in a horizontal or vertical layout,
    /// and between rows/columns in a grid layout.
    fn gap_width(&self, _style: &egui::Style) -> f32 {
        1.0
    }

    /// No child should shrink below this width nor height.
    fn min_size(&self) -> f32 {
        32.0
    }

    /// Show we preview panes that are being dragged,
    /// i.e. show their ui in the region where they will end up?
    fn preview_dragged_panes(&self) -> bool {
        false
    }

    /// Cover the tile that is being dragged with this color.
    fn dragged_overlay_color(&self, visuals: &Visuals) -> Color32 {
        visuals.panel_fill.gamma_multiply(0.5)
    }

    /// What are the rules for simplifying the tree?
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions::default()
    }

    /// The stroke used for the lines in horizontal, vertical, and grid layouts.
    fn resize_stroke(&self, style: &egui::Style, resize_state: ResizeState) -> Stroke {
        match resize_state {
            ResizeState::Idle => {
                Stroke::new(self.gap_width(style), self.tab_bar_color(&style.visuals))
            }
            ResizeState::Hovering => style.visuals.widgets.hovered.fg_stroke,
            ResizeState::Dragging => style.visuals.widgets.active.fg_stroke,
        }
    }

    /// Extra spacing to left and right of tab titles.
    fn tab_title_spacing(&self, _visuals: &Visuals) -> f32 {
        8.0
    }

    /// The background color of the tab bar.
    fn tab_bar_color(&self, visuals: &Visuals) -> Color32 {
        if visuals.dark_mode {
            visuals.extreme_bg_color
        } else {
            (Rgba::from(visuals.panel_fill) * Rgba::from_gray(0.8)).into()
        }
    }

    /// The background color of a tab.
    fn tab_bg_color(&self, visuals: &Visuals, _tile_id: TileId, active: bool) -> Color32 {
        if active {
            visuals.panel_fill // same as the tab contents
        } else {
            Color32::TRANSPARENT // fade into background
        }
    }

    /// Stroke of the outline around a tab title.
    fn tab_outline_stroke(&self, visuals: &Visuals, _tile_id: TileId, active: bool) -> Stroke {
        if active {
            Stroke::new(1.0, visuals.widgets.active.bg_fill)
        } else {
            Stroke::NONE
        }
    }

    /// Stroke of the line separating the tab title bar and the content of the active tab.
    fn tab_bar_hline_stroke(&self, visuals: &Visuals) -> Stroke {
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
    }

    /// The color of the title text of the tab.
    fn tab_text_color(&self, visuals: &Visuals, _tile_id: TileId, active: bool) -> Color32 {
        if active {
            visuals.widgets.active.text_color()
        } else {
            visuals.widgets.noninteractive.text_color()
        }
    }

    /// When drag-and-dropping a tile, how do we preview what is about to happen?
    fn paint_drag_preview(
        &self,
        visuals: &Visuals,
        painter: &egui::Painter,
        parent_rect: Option<Rect>,
        preview_rect: Rect,
    ) {
        let preview_stroke = visuals.selection.stroke;
        let preview_color = preview_stroke.color;

        if let Some(parent_rect) = parent_rect {
            // Show which parent we will be dropped into
            painter.rect_stroke(parent_rect, 1.0, preview_stroke);
        }

        painter.rect(
            preview_rect,
            1.0,
            preview_color.gamma_multiply(0.5),
            preview_stroke,
        );
    }

    /// How many columns should we use for a [`crate::Grid`] put into [`crate::GridLayout::Auto`]?
    ///
    /// The default heuristic tried to find a good column count that results in a per-tile aspect-ratio
    /// of [`Self::ideal_tile_aspect_ratio`].
    ///
    /// The `rect` is the available space for the grid,
    /// and `gap` is the distance between each column and row.
    fn grid_auto_column_count(
        &self,
        _tiles: &Tiles<Pane>,
        children: &[TileId],
        rect: Rect,
        gap: f32,
    ) -> usize {
        num_columns_heuristic(children.len(), rect, gap, self.ideal_tile_aspect_ratio())
    }

    /// When using [`crate::GridLayout::Auto`], what is the ideal aspect ratio of a tile?
    fn ideal_tile_aspect_ratio(&self) -> f32 {
        4.0 / 3.0
    }
}

/// How many columns should we use to fit `n` children in a grid?
fn num_columns_heuristic(n: usize, rect: Rect, gap: f32, desired_aspect: f32) -> usize {
    let mut best_loss = f32::INFINITY;
    let mut best_num_columns = 1;

    for ncols in 1..=n {
        let nrows = (n + ncols - 1) / ncols;

        let cell_width = (rect.width() - gap * (ncols as f32 - 1.0)) / (ncols as f32);
        let cell_height = (rect.height() - gap * (nrows as f32 - 1.0)) / (nrows as f32);

        let cell_aspect = cell_width / cell_height;
        let aspect_diff = (desired_aspect - cell_aspect).abs();
        let num_empty_cells = ncols * nrows - n;

        let loss = aspect_diff + 0.1 * num_empty_cells as f32; // TODO(emilk): weight differently?

        if loss < best_loss {
            best_loss = loss;
            best_num_columns = ncols;
        }
    }

    best_num_columns
}
