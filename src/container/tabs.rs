use egui::{NumExt as _, Rect, Vec2, scroll_area::ScrollBarVisibility, vec2};

use crate::behavior::{EditAction, LayoutContext, TabState};
use crate::{
    Behavior, ContainerInsertion, DropContext, InsertionPoint, SimplifyAction, TileId, Tiles, Tree,
    is_being_dragged,
};

/// A container with tabs. Only one tab is open (active) at a time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tabs {
    /// The tabs, in order.
    pub children: Vec<TileId>,

    /// The currently open tab.
    pub active: Option<TileId>,
}

/// The current tab scrolling state
#[derive(Clone, Copy, Debug, Default)]
struct ScrollState {
    /// The current horizontal scroll offset.
    ///
    /// Positive: scroll right.
    /// Negatie: scroll left.
    pub offset: f32,

    /// Outstanding offset to apply smoothly over the next few frames.
    /// This is what the buttons update.
    pub offset_debt: f32,

    /// The size of all the tabs last frame.
    pub content_size: Vec2,

    /// The available size for the tabs.
    pub available: Vec2,

    /// Show the left scroll-arrow this frame?
    pub show_left_arrow: bool,

    /// Show the right scroll-arrow this frame?
    pub show_right_arrow: bool,

    /// Did we show the left scroll-arrow last frame?
    pub showed_left_arrow_prev: bool,

    /// Did we show the right scroll-arrow last frame?
    pub showed_right_arrow_prev: bool,
}

/// What a wrapped tab bar measured last frame.
///
/// Kept in `egui`'s temporary memory rather than in [`Tabs`], so that adding it breaks no
/// caller's struct literal, and so the layout pass can read it without a tile at hand.
#[derive(Clone, Debug, Default)]
pub(crate) struct WrapState {
    /// How many rows the tabs took.
    pub rows: usize,

    /// The width of each tab, in the order the tabs were drawn.
    pub widths: Vec<f32>,

    /// The width [`Behavior::top_bar_right_ui`] took, which the tabs have to leave free.
    pub right_width: f32,
}

/// What one pass over the tabs produced, whether they wrapped or scrolled.
#[derive(Default)]
struct TabBarOutput {
    /// The tab to open next frame — the one clicked, or the one already open.
    next_active: Option<TileId>,

    /// Where each tab ended up, for the drop zones.
    button_rects: ahash::HashMap<TileId, Rect>,

    /// Index into [`Tabs::children`] of the tab being dragged, if any.
    dragged_index: Option<usize>,

    /// The width of each tab drawn, in order, to wrap by next frame.
    widths: Vec<f32>,

    /// The width [`Behavior::top_bar_right_ui`] took, to wrap by next frame.
    right_width: f32,

    /// How many rows the tabs took.
    rows: usize,
}

struct WrapLayout<'a> {
    tile_id: TileId,
    row_height: f32,
    max_rows: usize,
    previous: &'a WrapState,
}

pub(crate) fn wrap_state_id(tile_id: TileId, tree_id: egui::Id) -> egui::Id {
    tile_id.egui_id(tree_id).with("tab_bar_wrap")
}

/// How many rows the tab bar of `tile_id` took last frame, or 1 if it has not been drawn yet.
pub(crate) fn tab_bar_rows(context: &egui::Context, tile_id: TileId, tree_id: egui::Id) -> usize {
    context
        .data(|data| data.get_temp::<WrapState>(wrap_state_id(tile_id, tree_id)))
        .map_or(1, |state| state.rows.max(1))
}

/// Splits `widths` into rows no wider than `available`, or `None` if that needs more than
/// `max_rows`.
///
/// Greedy, which is what a tab bar wants: tabs keep their order, and a row is filled before the
/// next one is started, so a tab only ever moves down when the one before it no longer fits.
fn wrap_rows(widths: &[f32], available: f32, max_rows: usize) -> Option<Vec<usize>> {
    if widths.is_empty() {
        return Some(vec![0]);
    }
    let mut rows = vec![0usize];
    let mut used = 0.0;
    for &width in widths {
        let fits = used == 0.0 || used + width <= available;
        if !fits {
            if rows.len() >= max_rows {
                return None;
            }
            rows.push(0);
            used = 0.0;
        }
        if let Some(row) = rows.last_mut() {
            *row += 1;
        }
        used += width;
    }
    Some(rows)
}

impl ScrollState {
    /// Returns the space left for the tabs after the scroll arrows.
    pub fn update(&mut self, ui: &egui::Ui, arrow_size: Vec2) -> f32 {
        let mut scroll_area_width = ui.available_width();

        let button_and_spacing_width = arrow_size.x + ui.spacing().item_spacing.x;

        let margin = 0.1;

        self.show_left_arrow = arrow_size.x < self.offset;

        if self.show_left_arrow {
            scroll_area_width -= button_and_spacing_width;
        }

        self.show_right_arrow = self.offset + scroll_area_width + margin < self.content_size.x;

        // Compensate for showing/hiding of arrow:
        self.offset += button_and_spacing_width
            * ((self.show_left_arrow as i32 as f32) - (self.showed_left_arrow_prev as i32 as f32));

        if self.show_right_arrow {
            scroll_area_width -= button_and_spacing_width;
        }

        self.showed_left_arrow_prev = self.show_left_arrow;
        self.showed_right_arrow_prev = self.show_right_arrow;

        if self.offset_debt != 0.0 {
            const SPEED: f32 = 500.0;

            let dt = ui.input(|i| i.stable_dt).min(0.1);
            let max_movement = dt * SPEED;
            if self.offset_debt.abs() <= max_movement {
                self.offset += self.offset_debt;
                self.offset_debt = 0.0;
            } else {
                let movement = self.offset_debt.signum() * max_movement;
                self.offset += movement;
                self.offset_debt -= movement;
                ui.request_repaint();
            }
        }

        scroll_area_width
    }

    fn scroll_increment(&self) -> f32 {
        (self.available.x / 3.0).at_least(20.0)
    }

    fn arrow_button(ui: &mut egui::Ui, arrow_size: Vec2, id: egui::Id, glyph: &str) -> bool {
        let glyph_size = arrow_size.y * 0.5;
        ui.scope_builder(egui::UiBuilder::new().id(id), |ui| {
            ui.add_sized(
                arrow_size,
                egui::Button::new(egui::RichText::new(glyph).size(glyph_size)),
            )
        })
        .inner
        .clicked()
    }

    fn hidden_arrow_marker(ui: &egui::Ui, arrow_size: Vec2, id: egui::Id) {
        let rect = ui
            .layout()
            .align_size_within_rect(arrow_size, ui.available_rect_before_wrap());
        ui.interact(rect, id, egui::Sense::hover());
    }

    pub fn left_arrow(&mut self, ui: &mut egui::Ui, arrow_size: Vec2, id: egui::Id) {
        if !self.show_left_arrow {
            Self::hidden_arrow_marker(ui, arrow_size, id);
            return;
        }

        if Self::arrow_button(ui, arrow_size, id, "⏴") {
            self.offset_debt -= self.scroll_increment();
        }
    }

    pub fn right_arrow(&mut self, ui: &mut egui::Ui, arrow_size: Vec2, id: egui::Id) {
        if !self.show_right_arrow {
            Self::hidden_arrow_marker(ui, arrow_size, id);
            return;
        }

        if Self::arrow_button(ui, arrow_size, id, "⏵") {
            self.offset_debt += self.scroll_increment();
        }
    }
}

impl Tabs {
    pub fn new(children: Vec<TileId>) -> Self {
        let active = children.first().copied();
        Self { children, active }
    }

    pub fn add_child(&mut self, child: TileId) {
        self.children.push(child);
    }

    /// Swap out one tab for another, keeping its position and whether it was the open one.
    ///
    /// Returns the index of the tab that was swapped,
    /// or `None` if `old` was not a tab of this container.
    #[must_use]
    pub(super) fn replace_child(&mut self, old: TileId, new: TileId) -> Option<usize> {
        let index = self.children.iter().position(|child| *child == old)?;
        self.children[index] = new;
        if self.active == Some(old) {
            self.active = Some(new);
        }
        Some(index)
    }

    pub fn set_active(&mut self, child: TileId) {
        self.active = Some(child);
    }

    pub fn is_active(&self, child: TileId) -> bool {
        Some(child) == self.active
    }

    pub(super) fn layout<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        layout: &LayoutContext<'_>,
        rect: Rect,
        tile_id: TileId,
    ) {
        let prev_active = self.active;
        self.ensure_active(tiles);
        if prev_active != self.active {
            layout.tab_auto_selected.set(true);
        }

        let mut active_rect = rect;
        active_rect.min.y += layout.tab_bar_height * (layout.tab_bar_rows)(tile_id) as f32;

        if let Some(active) = self.active {
            // Only lay out the active tab (saves CPU):
            tiles.layout_tile(layout, active_rect, active);
        }
    }

    pub fn next_active<Pane>(&self, tiles: &Tiles<Pane>) -> Option<TileId> {
        self.active
            .filter(|active| self.children.contains(active) && tiles.is_visible(*active))
            .or_else(|| {
                self.children
                    .iter()
                    .copied()
                    .find(|&child_id| tiles.is_visible(child_id))
            })
    }

    /// Make sure we have an active tab (or no visible tabs).
    pub fn ensure_active<Pane>(&mut self, tiles: &Tiles<Pane>) {
        self.active = self.next_active(tiles);
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
        let next_active = self.tab_bar_ui(tree, behavior, ui, rect, drop_context, tile_id);

        if let Some(active) = self.active {
            tree.tile_ui(behavior, drop_context, ui, active);
            crate::cover_tile_if_dragged(tree, behavior, ui, active);
        }

        // We have only laid out the active tab, so we need to switch active tab _after_ the ui pass above:
        self.active = next_active;
    }

    /// Returns the next active tab (e.g. the one clicked, or the current).
    fn tab_bar_ui<Pane>(
        &self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        rect: Rect,
        drop_context: &mut DropContext,
        tile_id: TileId,
    ) -> Option<TileId> {
        let row_height = behavior.tab_bar_height(ui.style());
        let max_rows = behavior.max_tab_bar_rows().max(1);

        let wrap_id = wrap_state_id(tile_id, tree.id);
        let previous = ui
            .data(|data| data.get_temp::<WrapState>(wrap_id))
            .unwrap_or_default();
        let rows_reserved = if max_rows > 1 {
            previous.rows.clamp(1, max_rows)
        } else {
            1
        };

        let tab_bar_rect = rect
            .split_top_bottom_at_y(rect.top() + row_height * rows_reserved as f32)
            .0;
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(tab_bar_rect));

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, behavior.tab_bar_color(ui.visuals()));

        let mut output = TabBarOutput {
            next_active: self.active,
            ..TabBarOutput::default()
        };

        let wrapped = max_rows > 1
            && self.wrapping_tab_bar_ui(
                tree,
                behavior,
                &mut ui,
                drop_context,
                &WrapLayout {
                    tile_id,
                    row_height,
                    max_rows,
                    previous: &previous,
                },
                &mut output,
            );

        if !wrapped {
            self.scrolling_tab_bar_ui(
                tree,
                behavior,
                &mut ui,
                drop_context,
                tile_id,
                row_height,
                &mut output,
            );
            output.rows = 1;
        }

        if output.rows != rows_reserved {
            ui.ctx().request_repaint();
        }
        let widths = std::mem::take(&mut output.widths);
        let rows = output.rows;
        let right_width = output.right_width;
        ui.data_mut(|data| {
            data.insert_temp(
                wrap_id,
                WrapState {
                    rows,
                    widths,
                    right_width,
                },
            );
        });

        self.tab_drop_zones(&ui, drop_context, tile_id, &output);

        output.next_active
    }

    fn tab_drop_zones(
        &self,
        ui: &egui::Ui,
        drop_context: &mut DropContext,
        tile_id: TileId,
        output: &TabBarOutput,
    ) {
        let preview_thickness = 6.0;
        let dragged_index = output.dragged_index;
        let button_rects = &output.button_rects;
        let after_rect = |rect: Rect| {
            let dragged_size = if let Some(dragged_index) = dragged_index {
                button_rects[&self.children[dragged_index]].size()
            } else {
                rect.size()
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
            |tile_id| button_rects.get(&tile_id).copied(),
            |rect, i| {
                drop_context.suggest_rect(
                    InsertionPoint::new(tile_id, ContainerInsertion::Tabs(i)),
                    rect,
                );
            },
            after_rect,
        );
    }

    /// Draws every tab in one row per line, when they fit within the allowed number of rows.
    ///
    /// Returns `false` without drawing anything if they do not, which is the caller's signal to
    /// fall back to a single scrolling row. Both the tab widths and the width of
    /// [`Behavior::top_bar_right_ui`] come from what the previous frame measured, so that deciding
    /// whether the tabs fit draws nothing that the fallback would then draw a second time.
    fn wrapping_tab_bar_ui<Pane>(
        &self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        drop_context: &DropContext,
        wrap: &WrapLayout<'_>,
        output: &mut TabBarOutput,
    ) -> bool {
        let tile_id = wrap.tile_id;
        let bar_rect = ui.max_rect();
        let available = bar_rect.width() - wrap.previous.right_width;

        let visible: Vec<(usize, TileId)> = self
            .children
            .iter()
            .enumerate()
            .filter(|&(_, &child_id)| tree.is_visible(child_id))
            .map(|(index, &child_id)| (index, child_id))
            .collect();

        let Some(plan) = wrap_rows(&wrap.previous.widths, available, wrap.max_rows) else {
            return false;
        };
        if plan.iter().sum::<usize>() != visible.len() {
            return false;
        }

        let first_row = bar_rect
            .split_top_bottom_at_y(bar_rect.top() + wrap.row_height)
            .0;
        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(first_row)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let mut unused_offset = 0.0;
        behavior.top_bar_right_ui(
            &tree.tiles,
            &mut right_ui,
            tile_id,
            self,
            &mut unused_offset,
        );
        output.right_width = right_ui.min_rect().width();

        Self::drag_background(tree, behavior, ui, tile_id);

        let mut drawn = 0;
        for (row_index, count) in plan.iter().enumerate() {
            let top = bar_rect.top() + wrap.row_height * row_index as f32;
            let row_rect = Rect::from_min_size(
                egui::pos2(bar_rect.left(), top),
                vec2(available, wrap.row_height),
            );
            let mut row_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(row_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            row_ui.spacing_mut().item_spacing.x = 0.0;
            self.tabs_ui(
                tree,
                behavior,
                &mut row_ui,
                drop_context,
                &visible[drawn..drawn + count],
                output,
            );
            if row_index + 1 == plan.len() {
                behavior.tab_bar_trailing_ui(&tree.tiles, &mut row_ui, tile_id, self);
            }
            drawn += count;
        }
        output.rows = plan.len();
        true
    }

    fn drag_background<Pane>(
        tree: &Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &egui::Ui,
        tile_id: TileId,
    ) {
        if tree.is_root(tile_id) || !behavior.is_tile_draggable(&tree.tiles, tile_id) {
            return;
        }
        let sense = egui::Sense::click_and_drag();
        if ui
            .interact(ui.max_rect(), ui.id().with("background"), sense)
            .on_hover_cursor(egui::CursorIcon::Grab)
            .drag_started()
        {
            behavior.on_edit(EditAction::TileDragged);
            ui.set_dragged_id(tile_id.egui_id(tree.id));
        }
    }

    fn tabs_ui<Pane>(
        &self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        drop_context: &DropContext,
        children: &[(usize, TileId)],
        output: &mut TabBarOutput,
    ) {
        for &(index, child_id) in children {
            let is_being_dragged = is_being_dragged(ui, tree.id, child_id);
            let tab_state = TabState {
                active: self.is_active(child_id),
                is_being_dragged,
                closable: behavior.is_tab_closable(&tree.tiles, child_id),
            };
            let id = child_id.egui_id(tree.id);
            let response = behavior.tab_ui(&mut tree.tiles, ui, id, child_id, &tab_state);

            if response.clicked() {
                behavior.on_edit(EditAction::TabSelected);
                output.next_active = Some(child_id);
            }

            if let Some(mouse_pos) = drop_context.mouse_pos
                && drop_context.dragged_tile_id.is_some()
                && response.rect.contains(mouse_pos)
            {
                behavior.on_edit(EditAction::TabSelected);
                output.next_active = Some(child_id);
            }

            output.widths.push(response.rect.width());
            output.button_rects.insert(child_id, response.rect);
            if is_being_dragged {
                output.dragged_index = Some(index);
            }
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn scrolling_tab_bar_ui<Pane>(
        &self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        drop_context: &DropContext,
        tile_id: TileId,
        tab_bar_height: f32,
        output: &mut TabBarOutput,
    ) {
        let arrow_size = egui::Vec2::splat(tab_bar_height);
        let visible: Vec<(usize, TileId)> = self
            .children
            .iter()
            .enumerate()
            .filter(|&(_, &child_id)| tree.is_visible(child_id))
            .map(|(index, &child_id)| (index, child_id))
            .collect();

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let scroll_state_id = ui.make_persistent_id(tile_id);
            let mut scroll_state = ui.memory_mut(|m| {
                m.data
                    .get_temp::<ScrollState>(scroll_state_id)
                    .unwrap_or_default()
            });

            // Allow user to add buttons such as "add new tab".
            // They can also read and modify the scroll state if they want.
            let before_right_ui = ui.min_rect().width();
            behavior.top_bar_right_ui(&tree.tiles, ui, tile_id, self, &mut scroll_state.offset);
            output.right_width = ui.min_rect().width() - before_right_ui;

            let scroll_area_width = scroll_state.update(ui, arrow_size);

            // We're in a right-to-left layout, so start with the right scroll-arrow:
            let right_arrow_id = ui.make_persistent_id((tile_id, "right_scroll_arrow"));
            scroll_state.right_arrow(ui, arrow_size, right_arrow_id);

            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    // Left custom slot — first call in this LTR child layout
                    // means leftmost on screen, so it sits to the left of the
                    // left scroll-arrow.
                    behavior.tab_bar_left_ui(&tree.tiles, ui, tile_id, self);

                    let left_arrow_id = ui.make_persistent_id((tile_id, "left_scroll_arrow"));
                    scroll_state.left_arrow(ui, arrow_size, left_arrow_id);

                    // Clamp the precomputed width so it can't exceed what's
                    // left after the leading slot + left arrow consumed space
                    // inside this LTR child ui.
                    let scroll_area_width = scroll_area_width.min(ui.available_width()).max(0.0);

                    // Prepare to show the scroll area with the tabs:

                    scroll_state.offset = scroll_state
                        .offset
                        .at_most(scroll_state.content_size.x - ui.available_width());
                    scroll_state.offset = scroll_state.offset.at_least(0.0);

                    let scroll_area = egui::ScrollArea::horizontal()
                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                        .max_width(scroll_area_width)
                        .auto_shrink([false; 2])
                        .horizontal_scroll_offset(scroll_state.offset);

                    let scrolled = scroll_area.show(ui, |ui| {
                        Self::drag_background(tree, behavior, ui, tile_id);

                        ui.spacing_mut().item_spacing.x = 0.0; // Tabs have spacing built-in

                        self.tabs_ui(tree, behavior, ui, drop_context, &visible, output);

                        // Allow the user to add a trailing widget after the last tab
                        // (e.g. a "➕" button), inside the tab scroll area's flow.
                        behavior.tab_bar_trailing_ui(&tree.tiles, ui, tile_id, self);
                    });

                    scroll_state.offset = scrolled.state.offset.x;
                    scroll_state.content_size = scrolled.content_size;
                    scroll_state.available = scrolled.inner_rect.size();
                },
            );

            ui.data_mut(|data| data.insert_temp(scroll_state_id, scroll_state));
        });
    }

    pub(super) fn simplify_children(&mut self, mut simplify: impl FnMut(TileId) -> SimplifyAction) {
        self.children.retain_mut(|child| match simplify(*child) {
            SimplifyAction::Remove => false,
            SimplifyAction::Keep => true,
            SimplifyAction::Replace(new) => {
                if self.active == Some(*child) {
                    self.active = Some(new);
                }
                *child = new;
                true
            }
        });
    }

    /// Returns child index, if found.
    pub(crate) fn remove_child(&mut self, needle: TileId) -> Option<usize> {
        let index = self.children.iter().position(|&child| child == needle)?;
        self.children.remove(index);
        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::wrap_rows;

    #[test]
    fn tabs_that_fit_stay_on_one_row() {
        assert_eq!(wrap_rows(&[30.0, 30.0, 30.0], 100.0, 2), Some(vec![3]));
    }

    #[test]
    fn a_row_is_filled_before_the_next_one_is_started() {
        assert_eq!(wrap_rows(&[60.0, 60.0, 30.0], 100.0, 2), Some(vec![1, 2]));
    }

    #[test]
    fn tabs_that_need_more_rows_than_allowed_do_not_wrap() {
        assert_eq!(wrap_rows(&[60.0, 60.0, 60.0], 100.0, 2), None);
    }

    #[test]
    fn a_tab_wider_than_the_bar_still_gets_its_own_row() {
        assert_eq!(wrap_rows(&[300.0, 30.0], 100.0, 2), Some(vec![1, 1]));
    }

    #[test]
    fn an_empty_bar_is_one_row() {
        assert_eq!(wrap_rows(&[], 100.0, 2), Some(vec![0]));
    }
}
