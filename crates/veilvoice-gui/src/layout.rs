// SPDX-License-Identifier: GPL-3.0-or-later

//! Centring a row of widgets, which egui does not do by nesting.
//!
//! # The defect this exists to fix
//!
//! The unlock screen drew its mark, its name and the word "locked" inside
//! [`egui::Ui::vertical_centered`], and then drew the password field, the
//! unlock button and the status line underneath in a plain row. The heading
//! sat in the middle of the window and the controls sat against the left edge.
//! The same shape turns up wherever a row is drawn only after setup rather
//! than at launch, because those rows tend to be written later and separately
//! from the ones they end up beside.
//!
//! The obvious repair does not work, and it is worth writing down why, because
//! it looks like it should. Wrapping the row in `vertical_centered` puts it in
//! a `Layout::top_down(Align::Center)`, which centres each child *narrower
//! than the available width*. `Ui::horizontal` is never narrower: it allocates
//!
//! ```text
//! let initial_size = vec2(
//!     self.available_size_before_wrap().x,   // the whole width
//!     self.spacing().interact_size.y,
//! );
//! ```
//!
//! so the row's box is already full width, there is nothing left to centre it
//! within, and its contents start at that box's left edge. A centred layout
//! inside a centred layout changes nothing. `Layout::left_to_right` carries
//! `main_align: Align::Center` and does not help for the same reason.
//!
//! # Measured last frame, drawn this frame
//!
//! The row is drawn once, with a space in front of it worked out from how wide
//! the same row turned out to be on the previous frame, remembered in egui's
//! own temporary memory.
//!
//! **The closure is called once, and that is the whole design constraint.**
//! The tidier-looking approach is [`egui::UiBuilder::sizing_pass`]: lay the
//! row out invisibly, measure it, then lay it out again for real. That needs
//! the closure twice, and these closures are not pure. The unlock row spawns
//! a key derivation when its button reports a click, and a sizing pass runs
//! the body rather than skipping it, so measuring that way would risk
//! spawning the work twice from one press. A row of widgets is not worth a
//! double unlock, so the width comes from the last frame instead.
//!
//! The cost is that the first frame a row appears on is drawn left-aligned,
//! for as long as it takes to ask for another frame, which is done here
//! immediately. Nobody sees a single frame at sixty of them a second; and if
//! it were ever visible, being briefly left-aligned is what the defect looked
//! like permanently.

use egui::{InnerResponse, Ui};

/// Draw a row of widgets centred in the width available.
///
/// A drop-in replacement for [`egui::Ui::horizontal`] wherever a row belongs
/// under centred headings. The returned response covers the row itself, not
/// the padding in front of it.
pub fn centred_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    // Tied to this call site rather than to anything the caller passes, so two
    // rows in one panel cannot share a remembered width.
    let id = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);

    let room = ui.available_width();
    let remembered: Option<f32> = ui.ctx().memory(|m| m.data.get_temp(id));
    // `min(room)` because a row wider than the window is drawn from the left
    // rather than pushed off both edges: clipped on one side beats two.
    let indent = remembered
        .map(|was| ((room - was.min(room)) / 2.0).max(0.0))
        .unwrap_or(0.0);

    let mut inner = None;
    let outer = ui.horizontal(|ui| {
        ui.add_space(indent);
        // Measured by how far the cursor travels, not by any child `Ui`'s
        // rect. A nested `scope` reports the full available width for the same
        // reason `horizontal` does, so measuring one would feed this the window
        // width every frame and the indent would compute to zero: the exact
        // defect being fixed, wearing the fix's own clothes.
        let start = ui.cursor().min.x;
        let value = add(ui);
        let end = ui.cursor().min.x;
        inner = Some(value);
        (start, end)
    });

    // The row's own rect, not the horizontal box it sits in. `ui.horizontal`
    // reports a rect that starts at the left edge and therefore includes the
    // space put in front of the widgets, so a caller reading it back would be
    // told the row is still hard left however far it was actually moved.
    let (start, end) = outer.inner;
    let mut response = outer.response;
    let band = response.rect;
    response.rect = egui::Rect::from_min_max(
        egui::pos2(start, band.top()),
        egui::pos2(end.max(start), band.bottom()),
    );

    // The cursor sits one item-spacing past the last widget, because egui has
    // already made room for whatever might come next. Including that would
    // centre the row plus a trailing gap, which puts the visible row half a
    // spacing left of centre: measured at 4px on an 8px spacing, in a capture
    // of the running window.
    let measured = (end - start - ui.spacing().item_spacing.x).max(0.0);
    if remembered != Some(measured) {
        ui.ctx().memory_mut(|m| m.data.insert_temp(id, measured));
        // Draw again straight away, so the corrected position is on screen on
        // the next frame rather than whenever something else happens to move.
        ui.ctx().request_repaint();
    }

    InnerResponse::new(inner.expect("the row body runs exactly once"), response)
}

/// A fixed-width column inside a row, so what follows it starts at one x.
///
/// # Why this is not `ui.label(format!("{label:<16}"))`
///
/// Padding a label with trailing spaces is the obvious way to make a column
/// and it aligns nothing outside a terminal. The interface font is
/// proportional, so a space is not the width of a letter and eight letters
/// plus two spaces is not the width of ten; and egui gives trailing
/// whitespace no reliable width at all. Rows padded that way sat at slightly
/// different places on different screens, and everything lined up beneath
/// them inherited the drift.
///
/// This has now been the cause of two findings in two different files, which
/// is why it is here rather than written out a third time.
///
/// # Why `set_min_width` is not optional
///
/// [`egui::Ui::allocate_ui_with_layout`] asks for a width and then gives back
/// only what the contents actually used, so a short label would take a short
/// column and the next widget would move left again, which is the whole
/// problem. `set_min_width` is what turns the request into a column.
pub fn column<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            add(ui)
        },
    )
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lay the same content out twice through one `Ui`, so the second call
    /// sees what the first remembered. Returns where the row starts each time.
    /// A context whose window really is `width` across.
    ///
    /// `Ui::set_width` does not do this: it sets a minimum, and the panel goes
    /// on offering the whole screen, which in a default test context is nearly
    /// ten thousand points. Centring inside that is centring inside the wrong
    /// number, so the screen itself is sized here.
    fn input(width: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(width, 400.0),
            )),
            ..Default::default()
        }
    }

    fn twice(width: f32, mut body: impl FnMut(&mut Ui)) -> (f32, f32, f32) {
        let ctx = egui::Context::default();
        let mut lefts = Vec::new();
        let mut available = 0.0_f32;
        for _ in 0..2 {
            let _ = ctx.run(input(width), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let origin = ui.min_rect().left();
                    available = ui.available_width();
                    let left = centred_row(ui, |ui| body(ui)).response.rect.left();
                    lefts.push(left - origin);
                });
            });
        }
        (lefts[0], lefts[1], available)
    }

    /// The row lands in the middle once its width is known.
    ///
    /// Both halves matter. Asserting only that the centred row is centred
    /// would pass just as happily if egui had been centring rows all along,
    /// and the helper would be dead weight nobody could tell was dead. So the
    /// same content is drawn as a plain row too, and the two must differ.
    #[test]
    fn a_centred_row_is_centred_and_a_plain_row_is_not() {
        let (first, second, available) = twice(600.0, |ui| {
            ui.label("password");
            let _ = ui.button("unlock");
        });

        assert!(
            available > 100.0,
            "the test ui has no room to centre within"
        );

        // First frame has nothing remembered, so it sits at the left. That is
        // the documented cost, asserted rather than hoped for.
        assert!(
            first < 1.0,
            "the first frame should be left-aligned, and started at {first}"
        );

        // Second frame knows the width and centres it.
        assert!(
            second > 100.0,
            "the second frame did not centre the row: it starts at {second} \
             of {available} available"
        );

        // And the row is as far from the right edge as from the left.
        let ctx = egui::Context::default();
        let mut gaps = (0.0_f32, 0.0_f32);
        for _ in 0..2 {
            let _ = ctx.run(input(600.0), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let origin = ui.min_rect().left();
                    let room = ui.available_width();
                    let rect = centred_row(ui, |ui| {
                        ui.label("password");
                        let _ = ui.button("unlock");
                    })
                    .response
                    .rect;
                    gaps = (rect.left() - origin, room - (rect.right() - origin));
                });
            });
        }
        assert!(
            (gaps.0 - gaps.1).abs() < 12.0,
            "not centred: {} to the left, {} to the right",
            gaps.0,
            gaps.1
        );
    }

    /// A row too wide for the window starts at the left, not off both edges.
    #[test]
    fn a_row_wider_than_the_window_is_not_pushed_off_both_edges() {
        let (_, second, _) = twice(40.0, |ui| {
            ui.label("a label a great deal wider than forty points across");
        });
        assert!(
            second >= -0.5,
            "the row was pushed to {second}, off the left edge"
        );
    }

    /// The body runs once per frame, however many times the row is measured.
    ///
    /// This is the constraint the whole design exists for: the unlock row
    /// spawns a key derivation when its button reports a click, and a helper
    /// that ran its closure twice could spawn it twice from one press.
    #[test]
    fn the_row_body_runs_exactly_once_per_frame() {
        let ctx = egui::Context::default();
        let mut runs = 0_u32;
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    centred_row(ui, |ui| {
                        runs += 1;
                        ui.label("once");
                    });
                });
            });
        }
        assert_eq!(runs, 3, "the body ran {runs} times across three frames");
    }
}
