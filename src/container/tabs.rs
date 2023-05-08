use std::collections::HashMap;

use egui::{vec2, Rect};

use crate::{
    is_being_dragged, Behavior, DropContext, InsertionPoint, LayoutInsertion, TileId, Tiles,
};

/// A container with tabs. Only one tab is open (active) at a time.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Tabs {
    /// The tabs, in order.
    pub children: Vec<TileId>,

    /// The currently open tab.
    pub active: TileId,
}

impl Tabs {
    pub fn new(children: Vec<TileId>) -> Self {
        let active = children.first().copied().unwrap_or_default();
        Self { children, active }
    }

    pub fn add_child(&mut self, child: TileId) {
        self.children.push(child);
    }

    pub fn set_active(&mut self, child: TileId) {
        self.active = child;
    }

    pub(super) fn layout<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        if !self.children.iter().any(|&child| child == self.active) {
            // Make sure something is active:
            self.active = self.children.first().copied().unwrap_or_default();
        }

        let mut active_rect = rect;
        active_rect.min.y += behavior.tab_bar_height(style);

        // Only lay out the active tab (saves CPU):
        tiles.layout_tile(style, behavior, active_rect, self.active);
    }

    pub(super) fn ui<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        drop_context: &mut DropContext,
        ui: &mut egui::Ui,
        rect: Rect,
        tile_id: TileId,
    ) {
        let next_active = self.tab_bar_ui(behavior, ui, rect, tiles, drop_context, tile_id);

        // When dragged, don't show it (it is "being held")
        let is_active_being_dragged = is_being_dragged(ui.ctx(), self.active);
        if !is_active_being_dragged {
            tiles.tile_ui(behavior, drop_context, ui, self.active);
        }

        // We have only laid out the active tab, so we need to switch active tab _after_ the ui pass above:
        self.active = next_active;
    }

    /// Returns the next active tab (e.g. the one clicked, or the current).
    fn tab_bar_ui<Pane>(
        &self,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        rect: Rect,
        tiles: &mut Tiles<Pane>,
        drop_context: &mut DropContext,
        tile_id: TileId,
    ) -> TileId {
        let mut next_active = self.active;

        let tab_bar_height = behavior.tab_bar_height(ui.style());
        let tab_bar_rect = rect.split_top_bottom_at_y(rect.top() + tab_bar_height).0;
        let mut ui = ui.child_ui(tab_bar_rect, *ui.layout());

        let mut button_rects = HashMap::new();
        let mut dragged_index = None;

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, behavior.tab_bar_color(ui.visuals()));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Add buttons such as "add new tab"
            behavior.top_bar_rtl_ui(ui, tile_id);

            ui.spacing_mut().item_spacing.x = 0.0; // Tabs have spacing built-in

            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_clip_rect(ui.max_rect()); // Don't cover the `rtl_ui` buttons.

                for (i, &child_id) in self.children.iter().enumerate() {
                    let is_being_dragged = is_being_dragged(ui.ctx(), child_id);

                    let selected = child_id == self.active;
                    let id = child_id.id();

                    let response =
                        behavior.tab_ui(tiles, ui, id, child_id, selected, is_being_dragged);
                    let response = response.on_hover_cursor(egui::CursorIcon::Grab);
                    if response.clicked() {
                        next_active = child_id;
                    }

                    if let Some(mouse_pos) = drop_context.mouse_pos {
                        if drop_context.dragged_tile_id.is_some()
                            && response.rect.contains(mouse_pos)
                        {
                            // Expand this tab - maybe the user wants to drop something into it!
                            next_active = child_id;
                        }
                    }

                    button_rects.insert(child_id, response.rect);
                    if is_being_dragged {
                        dragged_index = Some(i);
                    }
                }
            });
        });

        // -----------
        // Drop zones:

        let preview_thickness = 6.0;
        let after_rect = |rect: Rect| {
            let dragged_size = if let Some(dragged_index) = dragged_index {
                // We actually know the size of this thing
                button_rects[&self.children[dragged_index]].size()
            } else {
                rect.size() // guess that the size is the same as the last button
            };
            Rect::from_min_size(
                rect.right_top() + vec2(ui.spacing().item_spacing.x, 0.0),
                dragged_size,
            )
        };
        super::linear::drop_zones(
            preview_thickness,
            &self.children,
            dragged_index,
            super::LinearDir::Horizontal,
            |tile_id| button_rects[&tile_id],
            |rect, i| {
                drop_context
                    .suggest_rect(InsertionPoint::new(tile_id, LayoutInsertion::Tabs(i)), rect);
            },
            after_rect,
        );

        next_active
    }
}
