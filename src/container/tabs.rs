use egui::{scroll_area::ScrollBarVisibility, vec2, NumExt, Rect, Vec2};

use crate::{
    is_being_dragged, Behavior, ContainerInsertion, DropContext, InsertionPoint, SimplifyAction,
    TileId, Tiles, Tree,
};

/// Fixed size icons for `⏴` and `⏵`
const SCROLL_ARROW_SIZE: Vec2 = Vec2::splat(20.0);

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

impl ScrollState {
    /// Returns the space left for the tabs after the scroll arrows.
    pub fn update(&mut self, ui: &egui::Ui) -> f32 {
        let mut scroll_area_width = ui.available_width();

        let button_and_spacing_width = SCROLL_ARROW_SIZE.x + ui.spacing().item_spacing.x;

        let margin = 0.1;

        self.show_left_arrow = SCROLL_ARROW_SIZE.x < self.offset;

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
                ui.ctx().request_repaint();
            }
        }

        scroll_area_width
    }

    fn scroll_increment(&self) -> f32 {
        (self.available.x / 3.0).at_least(20.0)
    }

    pub fn left_arrow(&mut self, ui: &mut egui::Ui) {
        if !self.show_left_arrow {
            return;
        }

        if ui
            .add_sized(SCROLL_ARROW_SIZE, egui::Button::new("⏴"))
            .clicked()
        {
            self.offset_debt -= self.scroll_increment();
        }
    }

    pub fn right_arrow(&mut self, ui: &mut egui::Ui) {
        if !self.show_right_arrow {
            return;
        }

        if ui
            .add_sized(SCROLL_ARROW_SIZE, egui::Button::new("⏵"))
            .clicked()
        {
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

    pub fn set_active(&mut self, child: TileId) {
        self.active = Some(child);
    }

    pub fn is_active(&self, child: TileId) -> bool {
        Some(child) == self.active
    }

    pub(super) fn layout<Pane>(
        &mut self,
        tiles: &mut Tiles<Pane>,
        style: &egui::Style,
        behavior: &mut dyn Behavior<Pane>,
        rect: Rect,
    ) {
        if let Some(active) = self.active {
            if !tiles.is_visible(active) {
                self.active = None;
            }
        }

        if !self.children.iter().any(|&child| self.is_active(child)) {
            // Make sure something is active:
            self.active = self
                .children
                .iter()
                .copied()
                .find(|&child_id| tiles.is_visible(child_id));
        }

        let mut active_rect = rect;
        active_rect.min.y += behavior.tab_bar_height(style);

        if let Some(active) = self.active {
            // Only lay out the active tab (saves CPU):
            tiles.layout_tile(style, behavior, active_rect, active);
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
        let next_active = self.tab_bar_ui(tree, behavior, ui, rect, drop_context, tile_id);

        if let Some(active) = self.active {
            tree.tile_ui(behavior, drop_context, ui, active);
            crate::cover_tile_if_dragged(tree, behavior, ui, active);
        }

        // We have only laid out the active tab, so we need to switch active tab _after_ the ui pass above:
        self.active = next_active;
    }

    /// Returns the next active tab (e.g. the one clicked, or the current).
    #[allow(clippy::too_many_lines)]
    fn tab_bar_ui<Pane>(
        &self,
        tree: &mut Tree<Pane>,
        behavior: &mut dyn Behavior<Pane>,
        ui: &mut egui::Ui,
        rect: Rect,
        drop_context: &mut DropContext,
        tile_id: TileId,
    ) -> Option<TileId> {
        let mut next_active = self.active;

        let tab_bar_height = behavior.tab_bar_height(ui.style());
        let tab_bar_rect = rect.split_top_bottom_at_y(rect.top() + tab_bar_height).0;
        let mut ui = ui.child_ui(tab_bar_rect, *ui.layout());

        let mut button_rects = ahash::HashMap::default();
        let mut dragged_index = None;

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, behavior.tab_bar_color(ui.visuals()));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let scroll_state_id = ui.make_persistent_id(tile_id);
            let mut scroll_state = ui.ctx().memory_mut(|m| {
                m.data
                    .get_temp::<ScrollState>(scroll_state_id)
                    .unwrap_or_default()
            });

            // Allow user to add buttons such as "add new tab".
            // They can also read and modify the scroll state if they want.
            behavior.top_bar_right_ui(&tree.tiles, ui, tile_id, self, &mut scroll_state.offset);

            let scroll_area_width = scroll_state.update(ui);

            // We're in a right-to-left layout, so start with the right scroll-arrow:
            scroll_state.right_arrow(ui);

            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    scroll_state.left_arrow(ui);

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

                    let output = scroll_area.show(ui, |ui| {
                        if !tree.is_root(tile_id) {
                            // Make the background behind the buttons draggable (to drag the parent container tile):
                            if ui
                                .interact(
                                    ui.max_rect(),
                                    ui.id().with("background"),
                                    egui::Sense::drag(),
                                )
                                .on_hover_cursor(egui::CursorIcon::Grab)
                                .drag_started()
                            {
                                behavior.on_edit();
                                ui.memory_mut(|mem| mem.set_dragged_id(tile_id.egui_id()));
                            }
                        }

                        ui.spacing_mut().item_spacing.x = 0.0; // Tabs have spacing built-in

                        for (i, &child_id) in self.children.iter().enumerate() {
                            if !tree.is_visible(child_id) {
                                continue;
                            }

                            let is_being_dragged = is_being_dragged(ui.ctx(), child_id);

                            let selected = self.is_active(child_id);
                            let id = child_id.egui_id();

                            let response = behavior.tab_ui(
                                &tree.tiles,
                                ui,
                                id,
                                child_id,
                                selected,
                                is_being_dragged,
                            );
                            let response = response.on_hover_cursor(egui::CursorIcon::Grab);
                            if response.clicked() {
                                behavior.on_edit();
                                next_active = Some(child_id);
                            }

                            if let Some(mouse_pos) = drop_context.mouse_pos {
                                if drop_context.dragged_tile_id.is_some()
                                    && response.rect.contains(mouse_pos)
                                {
                                    // Expand this tab - maybe the user wants to drop something into it!
                                    behavior.on_edit();
                                    next_active = Some(child_id);
                                }
                            }

                            button_rects.insert(child_id, response.rect);
                            if is_being_dragged {
                                dragged_index = Some(i);
                            }
                        }
                    });

                    scroll_state.offset = output.state.offset.x;
                    scroll_state.content_size = output.content_size;
                    scroll_state.available = output.inner_rect.size();
                },
            );

            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(scroll_state_id, scroll_state));
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
            |tile_id| button_rects.get(&tile_id).copied(),
            |rect, i| {
                drop_context.suggest_rect(
                    InsertionPoint::new(tile_id, ContainerInsertion::Tabs(i)),
                    rect,
                );
            },
            after_rect,
        );

        next_active
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
