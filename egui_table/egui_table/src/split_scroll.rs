use egui::{pos2, vec2, Rect, Ui, UiBuilder, Vec2, Vec2b};
/// A scroll area with some portion of its left and/or top side "stuck".
///
/// This produces four quadrants:
///
/// ```text
///               <-------LEFT-------> <---------RIGHT---------->
///
///              ------------------------------------------------
///          ^   |                    |   <----------------->   |
///    TOP   |   |       Fixed        |      Horizontally       |
///          V   |    fixed_size      |       scrollable        |
///              |--------------------|-------------------------|.................
///          ^   | ^                  |           ^             |                .
///  BOTTOM  |   | |   Vertically     | <-  Fully scrollable -> |                .
///          |   | |   scrollable     |    scroll_outer_size    |                .
///          V   | v                  |           v             |                .
///              |____________________|_________________________|                .
///                                   .                                          .
///                                   .                   scroll_content_size    .
///                                   .                                          .
///                                   ............................................
/// ```
///
/// The above shows the initial layout when the scroll offset is zero (no scrolling has occurred yet).
#[derive(Clone, Copy, Debug)]
pub struct SplitScroll {
    pub scroll_enabled: Vec2b,

    /// Width of the fixed left side, and height of the fixed top.
    pub fixed_size: Vec2,

    /// Size of the small container of the right bottom scrollable region.
    pub scroll_outer_size: Vec2,

    /// Size of the large contents of the right bottom region, ignoring the left/top fixed regions.
    pub scroll_content_size: Vec2,
}

/// The contents of a [`SplitScroll`].
pub trait SplitScrollDelegate {
    /// The fixed portion of the top left corner.
    fn left_top_ui(&mut self, ui: &mut Ui);

    /// The horizontally scrollable portion.
    fn right_top_ui(&mut self, ui: &mut Ui);

    /// The vertically scrollable portion.
    fn left_bottom_ui(&mut self, ui: &mut Ui);

    /// The fully scrollable portion.
    fn right_bottom_ui(&mut self, ui: &mut Ui);
}

impl SplitScroll {
    pub fn show(self, ui: &mut Ui, delegate: &mut dyn SplitScrollDelegate) {
        let Self {
            scroll_enabled,
            fixed_size,
            scroll_outer_size,
            scroll_content_size,
        } = self;

        ui.scope(|ui| {
            ui.visuals_mut().clip_rect_margin = 0.0; // Everything else looks awful

            let full_clip_rect = ui.clip_rect();

            let mut rect = ui.cursor();
            rect.max = rect.min + fixed_size + scroll_outer_size;

            let bottom_right_rect = Rect::from_min_max(rect.min + fixed_size, rect.max);

            let scroll_offset = {
                // Entire thing is a scroll region.
                // PROBLEM: scroll bars show up at the full rect, instead of just the bottom-right.
                // We could add something like `ScrollArea::with_scroll_bar_rect(bottom_right_rect)`
                let mut scroll_ui = ui.new_child(UiBuilder::new().max_rect(rect));
                egui::ScrollArea::new(scroll_enabled)
                    .show(&mut scroll_ui, |ui| {
                        ui.set_min_size(fixed_size + scroll_content_size);

                        let mut shrunk_rect = ui.max_rect();
                        shrunk_rect.min += fixed_size;

                        let mut shrunk_ui = ui.new_child(UiBuilder::new().max_rect(shrunk_rect));
                        shrunk_ui.set_clip_rect(full_clip_rect.intersect(bottom_right_rect));
                        delegate.right_bottom_ui(&mut shrunk_ui);
                    })
                    .state
                    .offset
            };

            {
                // Fixed
                let left_top_rect = rect
                    .with_max_x(rect.left() + fixed_size.x)
                    .with_max_y(rect.top() + fixed_size.y);
                let mut left_top_ui = ui.new_child(UiBuilder::new().max_rect(left_top_rect));
                delegate.left_top_ui(&mut left_top_ui);
            }

            {
                // Horizontally scrollable
                let right_top_outer_rect = rect
                    .with_min_x(rect.left() + fixed_size.x)
                    .with_max_y(rect.top() + fixed_size.y);
                let right_top_content_rect = Rect::from_min_size(
                    pos2(right_top_outer_rect.min.x - scroll_offset.x, rect.min.y),
                    vec2(scroll_content_size.x, fixed_size.y),
                );
                let mut right_top_ui =
                    ui.new_child(UiBuilder::new().max_rect(right_top_content_rect));
                right_top_ui.set_clip_rect(full_clip_rect.intersect(right_top_outer_rect));
                delegate.right_top_ui(&mut right_top_ui);
            }

            {
                // Vertically scrollable
                let left_bottom_outer_rect = rect
                    .with_max_x(rect.left() + fixed_size.x)
                    .with_min_y(rect.top() + fixed_size.y);
                let left_bottom_content_rect = Rect::from_min_size(
                    pos2(rect.min.x, left_bottom_outer_rect.min.y - scroll_offset.y),
                    vec2(fixed_size.x, scroll_content_size.y),
                );
                let mut left_bottom_ui =
                    ui.new_child(UiBuilder::new().max_rect(left_bottom_content_rect));
                left_bottom_ui.set_clip_rect(full_clip_rect.intersect(left_bottom_outer_rect));
                delegate.left_bottom_ui(&mut left_bottom_ui);
            }
        });
    }
}
