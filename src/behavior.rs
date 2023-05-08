use egui::{
    vec2, Color32, Id, Rect, Response, Rgba, Sense, Stroke, TextStyle, Ui, Visuals, WidgetText,
};

use super::{ResizeState, SimplificationOptions, Tile, TileId, Tiles, UiResponse};

/// Trait defining how the [`super::Tree`] and its panes should be shown.
pub trait Behavior<Pane> {
    /// Show this pane tile in the given [`egui::Ui`].
    ///
    /// You can make the pane draggable by returning [`UiResponse::DragStarted`]
    /// when the user drags some handle.
    fn pane_ui(&mut self, _ui: &mut Ui, _tile_id: TileId, _pane: &mut Pane) -> UiResponse;

    /// The title of a pane tab.
    fn tab_title_for_pane(&mut self, pane: &Pane) -> WidgetText;

    /// The title of a general tab.
    ///
    /// The default implementation uses the name of the layout for containers, and
    /// calls [`Self::tab_title_for_pane`] for panes.
    fn tab_title_for_tile(&mut self, tiles: &Tiles<Pane>, tile_id: TileId) -> WidgetText {
        if let Some(tile) = tiles.tiles.get(&tile_id) {
            match tile {
                Tile::Pane(pane) => self.tab_title_for_pane(pane),
                Tile::Container(container) => format!("{:?}", container.layout()).into(),
            }
        } else {
            "MISSING TILE".into()
        }
    }

    /// Show the title of a tab as a button.
    ///
    /// You can override the default implementation to add e.g. a close button.
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

        response
    }

    /// Return `false` if this pane should be removed from its parent.
    fn retain_pane(&mut self, _pane: &Pane) -> bool {
        true
    }

    /// Adds some UI to the top right of each tab bar.
    ///
    /// You can use this to, for instance, add a button for adding new tabs.
    ///
    /// The widgets will be added right-to-left.
    fn top_bar_rtl_ui(&mut self, _ui: &mut Ui, _tile_id: TileId) {
        // if ui.button("➕").clicked() {
        // }
    }

    // --------
    // Settings:

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

    /// What are the rules for simplifying the tree?
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions::default()
    }

    /// The stroke used for the lines in horizontal, vertical, and grid layouts.
    fn resize_stroke(&self, style: &egui::Style, resize_state: ResizeState) -> Stroke {
        match resize_state {
            ResizeState::Idle => Stroke::NONE, // Let the gap speak for itself
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
            Color32::BLACK
        } else {
            (Rgba::from(visuals.window_fill()) * Rgba::from_gray(0.8)).into()
        }
    }

    /// The background color of a tab.
    fn tab_bg_color(&self, visuals: &Visuals, _tile_id: TileId, active: bool) -> Color32 {
        if active {
            visuals.window_fill() // same as the tab contents
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

    /// Show we preview panes that are being dragged,
    /// i.e. show their ui in the region where they will end up?
    fn preview_dragged_panes(&self) -> bool {
        false
    }
}
