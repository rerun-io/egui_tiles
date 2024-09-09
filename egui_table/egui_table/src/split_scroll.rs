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
    ///
    /// First to be called.
    fn right_bottom_ui(&mut self, ui: &mut Ui);

    /// Called last.
    fn finish(&mut self, _ui: &mut Ui) {}
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

            let mut rect = ui.cursor();
            rect.max = rect.min + fixed_size + scroll_outer_size;
            ui.shrink_clip_rect(rect);

            let bottom_right_rect = Rect::from_min_max(rect.min + fixed_size, rect.max);

            let scroll_offset = {
                // RIGHT BOTTOM: fully scrollable.

                // The entire thing is a `ScrollArea` that we then paint over.
                // PROBLEM: scroll bars show up at the full rect, instead of just the bottom-right.
                // We could add something like `ScrollArea::with_scroll_bar_rect(bottom_right_rect)`

                let mut scroll_ui = ui.new_child(UiBuilder::new().max_rect(rect));

                egui::ScrollArea::new(scroll_enabled)
                    .auto_shrink(false)
                    .scroll_bar_rect(bottom_right_rect)
                    .show_viewport(&mut scroll_ui, |ui, scroll_offset| {
                        ui.set_min_size(fixed_size + scroll_content_size);

                        let mut shrunk_rect = ui.max_rect();
                        shrunk_rect.min += fixed_size;

                        let mut shrunk_ui = ui.new_child(UiBuilder::new().max_rect(shrunk_rect));
                        shrunk_ui.shrink_clip_rect(bottom_right_rect);
                        delegate.right_bottom_ui(&mut shrunk_ui);

                        // It is very important that the scroll offset is synced between the
                        // right-bottom contents of the real scroll area,
                        // and the fake scroll areas we are painting later.
                        // The scroll offset that `ScrollArea` returns could be a newer one
                        // than was used for rendering, so we use the one _actually_ used for rendering instead:
                        scroll_offset.min
                    })
                    .inner
            };

            {
                // LEFT TOP: Fixed
                let left_top_rect = rect
                    .with_max_x(rect.left() + fixed_size.x)
                    .with_max_y(rect.top() + fixed_size.y);
                let mut left_top_ui = ui.new_child(UiBuilder::new().max_rect(left_top_rect));
                left_top_ui.shrink_clip_rect(left_top_rect);
                delegate.left_top_ui(&mut left_top_ui);
            }

            {
                // RIGHT TOP: Horizontally scrollable
                let right_top_outer_rect = rect
                    .with_min_x(rect.left() + fixed_size.x)
                    .with_max_y(rect.top() + fixed_size.y);
                let right_top_content_rect = Rect::from_min_size(
                    pos2(right_top_outer_rect.min.x - scroll_offset.x, rect.min.y),
                    vec2(scroll_content_size.x, fixed_size.y),
                );
                let mut right_top_ui =
                    ui.new_child(UiBuilder::new().max_rect(right_top_content_rect));
                right_top_ui.shrink_clip_rect(right_top_outer_rect);
                delegate.right_top_ui(&mut right_top_ui);
            }

            {
                // LEFT BOTTOM: Vertically scrollable
                let left_bottom_outer_rect = rect
                    .with_max_x(rect.left() + fixed_size.x)
                    .with_min_y(rect.top() + fixed_size.y);
                let left_bottom_content_rect = Rect::from_min_size(
                    pos2(rect.min.x, left_bottom_outer_rect.min.y - scroll_offset.y),
                    vec2(fixed_size.x, scroll_content_size.y),
                );
                let mut left_bottom_ui =
                    ui.new_child(UiBuilder::new().max_rect(left_bottom_content_rect));
                left_bottom_ui.shrink_clip_rect(left_bottom_outer_rect);
                delegate.left_bottom_ui(&mut left_bottom_ui);
            }

            delegate.finish(ui);
        });
    }
}
