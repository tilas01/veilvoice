// SPDX-License-Identifier: GPL-3.0-or-later
//! The animated mark: a row of bars that rise and fall.
//!
//! # The same mark as the website
//!
//! `website/index.html` draws this in CSS as `.veil` -- a row of `<span>`s with
//! `@keyframes pulse` taking each between 16% and 82% of the height over
//! 1.9 seconds, each with its own delay so the row ripples rather than pumping
//! in unison. The left half is drawn in the accent colour and the right half in
//! the "veiled" secondary, which is the product in one picture and matches the
//! icon.
//!
//! This is that, in egui, with the same period, the same height range and the
//! same delays. Two front-ends showing visibly different marks would be worse
//! than one showing none.
//!
//! # Why it is drawn rather than rendered from a GIF
//!
//! An animated image would be a committed binary blob, and this project's
//! artwork is generated from source precisely so that nothing in the repository
//! has to be taken on trust. Sixty lines of shape drawing is auditable; a GIF
//! is not.
//!
//! # Cost when it is switched off
//!
//! With motion disabled the bars are drawn once, at rest, and **no repaint is
//! requested**. That is the part that matters: an "off" switch that still
//! schedules a frame every 16 ms has turned the animation off visually and left
//! the battery cost behind. The caller decides by passing a [`Motion`], and the
//! only way to animate is to ask for it.
//!
//! # Cost when it is switched on, which is the interesting one
//!
//! Motion is on by default, and this is the only thing in the application that
//! moves without being asked to. Everything else draws when something happens.
//! So with the default settings, on the file tab, doing nothing, the window was
//! redrawing about sixty times a second for ever, and it was this: measured
//! with `Context::repaint_causes`, which named line 117 of this file as the
//! reason for 559 of 566 frames.
//!
//! An animated logo is not worth a permanently busy window, and two of the
//! costs are ones a user actually notices rather than ones a profiler does.
//! A laptop lid left open at this screen never lets the processor idle. And a
//! window being dragged is competing, every frame, with a full redraw it did
//! not need, which is what "it lags when I move it" is made of.
//!
//! So the mark now moves in the three circumstances where somebody can see it
//! moving, and rests otherwise:
//!
//! - **Not while the window is unfocused.** A background window is still.
//! - **Not while the window is being moved or resized.** Detected from the
//!   window's own rectangle changing between frames, and resumed a quarter of
//!   a second after it stops. This is the drag case specifically.
//! - **Not faster than [`FRAMES_PER_SECOND`].** The cycle is 1.9 seconds long
//!   and eased; twenty frames a second is indistinguishable from thirty here,
//!   and costs two thirds as much.
//!
//! Resting is not the same as resetting. Freezing at the midpoint would make
//! every click into another window snap the row flat, so a paused mark holds
//! the shape it had when it paused and picks the cycle up from there.
//!
//! # In plain words
//!
//! The little row of bars in the corner that rises and falls.
//!
//! It is the same mark the website uses, drawn rather than loaded as a picture, so
//! it takes the colours of whichever scheme you have chosen.
//!
//! It stops moving if your system is set to reduce motion. That setting is a
//! request from somebody who has a reason for making it, and animation that
//! ignores it is animation that makes an application unusable for them.

use crate::prefs::Motion;
use crate::theme::palette as p;
use egui::{Color32, CornerRadius, Rect, Sense, Ui, Vec2};

/// Seconds for one full rise and fall. Matches the website's `1.9s`.
const PERIOD: f32 = 1.9;

/// How often the mark is redrawn while it is moving.
///
/// Twenty rather than the thirty it used to ask for. Over a 1.9 second eased
/// cycle the two are not tellable apart, and every one of these frames is a
/// redraw of the whole window, not of the 46 by 22 pixels that changed: egui
/// has no partial repaint, so the cheapest frame is the one not drawn.
pub const FRAMES_PER_SECOND: u64 = 20;

/// How long the window must hold still before the mark starts moving again.
///
/// A drag delivers a new window rectangle every few milliseconds, so anything
/// shorter than this restarts the animation between two frames of the same
/// drag and achieves nothing. A quarter of a second is below the point where
/// somebody notices the mark was waiting.
const SETTLE: f64 = 0.25;

/// Per-bar phase offsets in seconds, matching the `animation-delay` values in
/// `website/index.html`. Twelve bars, deliberately not in order, so the row
/// ripples instead of sweeping.
const DELAYS: [f32; 12] = [
    0.000, 0.120, 0.240, 0.080, 0.300, 0.180, 0.060, 0.420, 0.220, 0.500, 0.140, 0.360,
];

/// Height as a fraction of the available box, matching `16%` and `82%`.
const MIN_FRACTION: f32 = 0.16;
const MAX_FRACTION: f32 = 0.82;

/// How far along its cycle a bar is, in 0..=1, eased the way CSS
/// `ease-in-out` eases.
fn height_fraction(time: f32, delay: f32) -> f32 {
    // `rem_euclid` so a negative time -- which `Context::input().time` will not
    // produce, but a test can -- still lands inside the period rather than
    // producing a negative phase.
    let phase = (time + delay).rem_euclid(PERIOD) / PERIOD;
    // A triangle wave, then smoothstep, which is close enough to
    // `ease-in-out` that no one could pick them apart side by side.
    let triangle = 1.0 - (phase * 2.0 - 1.0).abs();
    let eased = triangle * triangle * (3.0 - 2.0 * triangle);
    MIN_FRACTION + (MAX_FRACTION - MIN_FRACTION) * eased
}

/// Whether the window is holding still enough for the mark to move.
///
/// Two questions, both asked of the window rather than of the application:
/// does it have focus, and has its rectangle stopped changing.
///
/// The focus answer defaults to *yes* when the platform does not report one.
/// A mark frozen for ever on a system that never says "focused" is a worse
/// failure than one that animates when it did not have to, and there are
/// window managers that do not send focus events at all.
///
/// The rectangle answer is how a drag is detected. There is no "the user is
/// dragging me" signal in `egui` or `winit` to ask for; there is the window's
/// own position and size, which changes on every frame of a drag and on no
/// frame of anything else. Stored in `egui`'s temporary memory, so it lives
/// exactly as long as the context and costs no field on any struct.
fn window_is_settled(ctx: &egui::Context) -> bool {
    let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
    if !focused {
        return false;
    }

    let id = egui::Id::new("veilvoice-soundbar-window-rect");
    let (rect, now) = ctx.input(|i| (i.viewport().outer_rect, i.time));
    let Some(rect) = rect else {
        // A platform that does not report the window rectangle cannot be
        // asked whether it is moving. Treat it as still rather than never
        // animating there.
        return true;
    };

    let last: Option<(egui::Rect, f64)> = ctx.memory(|m| m.data.get_temp(id));
    let moved_at = match last {
        Some((was, when)) if was == rect => when,
        _ => now,
    };
    ctx.memory_mut(|m| m.data.insert_temp(id, (rect, moved_at)));

    let settled = now - moved_at >= SETTLE;
    if !settled {
        // Come back and look again once it could have settled, or the mark
        // stays frozen after the drag ends until something else moves.
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(SETTLE));
    }
    settled
}

/// The clock the bars are drawn against, which is not always the real one.
///
/// While the mark is moving this is the application clock. While it is resting
/// it is the moment it stopped, held in temporary memory, so the row keeps the
/// shape it had rather than snapping to the midpoint the instant the window
/// loses focus. Clicking away from the window and back should not make the
/// logo jump.
fn animation_clock(ctx: &egui::Context, moving: bool, time: f32) -> f32 {
    let id = egui::Id::new("veilvoice-soundbar-clock");
    if moving {
        ctx.memory_mut(|m| m.data.insert_temp(id, time));
        time
    } else {
        ctx.memory(|m| m.data.get_temp(id)).unwrap_or(time)
    }
}

/// Draw the mark at `size`, returning the response so it can carry a tooltip.
///
/// `time` is the application clock in seconds. When `motion` disallows
/// movement every bar is drawn at its resting height and nothing is scheduled.
/// When motion is allowed but the window is unfocused or being dragged, the
/// bars hold their last shape and nothing is scheduled either.
/// The application mark: the bars inside the rounded square the icon uses.
///
/// The lock screen drew the bars alone, which is the header's treatment and
/// reads there because the header is full of other VeilVoice furniture. On an
/// otherwise empty locked window it read as a stray animation rather than as
/// the program identifying itself, so this puts them back in the badge the
/// icon puts them in, and a locked window now shows the logo.
///
/// Drawn rather than decoded, like everything else in this module. The icon on
/// disk is 32x32, and blowing that up to the size wanted here would be visibly
/// soft; the same shape as vectors is crisp at any size and adds no image
/// decoder to the application.
pub fn badge(ui: &mut Ui, side: f32, motion: Motion, time: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    // The icon's proportions: a two-pixel border on a 32-pixel square, corners
    // cut by one. Kept as fractions so it scales.
    let radius = CornerRadius::same((side * 0.19).round().min(255.0) as u8);
    ui.painter()
        .rect_filled(rect, radius, crate::theme::palette::bg_dark());
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new((side * 0.055).max(1.0), crate::theme::palette::border()),
        egui::StrokeKind::Inside,
    );

    // The bars sit in the middle two thirds, as they do in the icon.
    let inner = Rect::from_center_size(rect.center(), Vec2::new(side * 0.66, side * 0.52));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    draw(&mut child, inner.size(), motion, time);

    response
}

/// Draw the mark at `size`, returning the response so it can carry a tooltip.
///
/// `time` is the application clock in seconds. When `motion` disallows
/// movement every bar is drawn at its resting height and nothing is scheduled.
/// When motion is allowed but the window is unfocused or being dragged, the
/// bars hold their last shape and nothing is scheduled either.
pub fn draw(ui: &mut Ui, size: Vec2, motion: Motion, time: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Asked once per draw, before any bar is positioned, so every bar in the
    // row agrees about which frame this is.
    let moving = motion.icon && window_is_settled(ui.ctx());
    let time = animation_clock(ui.ctx(), moving, time);

    let bars = DELAYS.len();
    // A gap of a quarter of a bar's width, as on the website (5 px bar, 3 px
    // gap is close to a third, but a quarter reads better at icon sizes).
    let unit = rect.width() / (bars as f32 * 1.25);
    let bar_width = unit;
    let gap = unit * 0.25;
    let total = bars as f32 * bar_width + (bars as f32 - 1.0) * gap;
    let left = rect.center().x - total / 2.0;

    let painter = ui.painter();
    let rounding = CornerRadius::same((bar_width * 0.4).min(2.0) as u8);

    for (index, delay) in DELAYS.iter().enumerate() {
        // At rest every bar sits at the midpoint, so a still mark reads as a
        // deliberate shape rather than as an animation caught mid-frame.
        let fraction = if motion.icon {
            height_fraction(time, *delay)
        } else {
            (MIN_FRACTION + MAX_FRACTION) / 2.0
        };
        let height = rect.height() * fraction;
        let x = left + index as f32 * (bar_width + gap);
        let bar = Rect::from_min_size(
            egui::pos2(x, rect.center().y - height / 2.0),
            egui::vec2(bar_width, height),
        );
        painter.rect_filled(bar, rounding, colour_for(index, bars));
    }

    // Only ask for another frame if something will actually differ in it.
    // `moving`, not `motion.icon`: an unfocused or dragging window is not
    // going to look different next frame, and asking anyway is how a window
    // that appears to be doing nothing keeps a processor busy.
    if moving {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1000 / FRAMES_PER_SECOND));
    }

    response
}

/// The left half in the accent colour, the right in the veiled secondary --
/// the same split the website and the icon use.
fn colour_for(index: usize, bars: usize) -> Color32 {
    if index * 2 < bars {
        p::blue()
    } else {
        // `0.85` matches the website's `opacity: 0.85` on the veiled half,
        // applied as a blend towards the background so it does not depend on
        // what is painted behind it.
        p::blend(p::bg(), p::purple(), 0.85)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::Prefs;

    fn moving() -> Motion {
        Motion::resolve(
            &Prefs {
                animations: true,
                animated_icon: true,
                ..Default::default()
            },
            false,
        )
    }

    /// A frame of input describing a window: is it focused, where is it, and
    /// what time is it. `None` for the rectangle stands for a platform that
    /// does not report one.
    fn frame(focused: Option<bool>, rect: Option<egui::Rect>, time: f64) -> egui::RawInput {
        let mut input = egui::RawInput {
            time: Some(time),
            ..Default::default()
        };
        let id = egui::ViewportId::ROOT;
        let viewport = input.viewports.entry(id).or_default();
        viewport.focused = focused;
        viewport.outer_rect = rect;
        input
    }

    /// The bar heights drawn for one frame of that input.
    fn heights_for(
        ctx: &egui::Context,
        input: egui::RawInput,
        motion: Motion,
        time: f32,
    ) -> Vec<u32> {
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw(ui, egui::vec2(120.0, 40.0), motion, time);
            });
        });
        let mut heights = Vec::new();
        for clipped in output.shapes {
            if let egui::Shape::Rect(r) = &clipped.shape {
                heights.push((r.rect.height() * 100.0) as u32);
            }
        }
        heights
    }

    /// The soonest repaint the context asked for after that frame.
    fn soonest_after(
        ctx: &egui::Context,
        input: egui::RawInput,
        motion: Motion,
        time: f32,
    ) -> std::time::Duration {
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw(ui, egui::vec2(120.0, 40.0), motion, time);
            });
        });
        output
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(std::time::Duration::MAX)
    }

    const SOMEWHERE: fn() -> egui::Rect =
        || egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(800.0, 600.0));

    fn still() -> Motion {
        Motion::resolve(
            &Prefs {
                animations: true,
                animated_icon: false,
                ..Default::default()
            },
            false,
        )
    }

    #[test]
    fn a_bar_stays_inside_the_documented_height_range() {
        // Walk several periods at a fine step: nothing may leave 16%..82%,
        // because a bar taller than its box is a bar drawn over the text
        // beside it.
        let mut lowest = f32::MAX;
        let mut highest = f32::MIN;
        for step in 0..2000 {
            let time = step as f32 * 0.01;
            for delay in DELAYS {
                let f = height_fraction(time, delay);
                assert!(f.is_finite(), "non-finite height at t={time}");
                lowest = lowest.min(f);
                highest = highest.max(f);
            }
        }
        assert!(
            lowest >= MIN_FRACTION - 1e-4,
            "went below the floor: {lowest}"
        );
        assert!(
            highest <= MAX_FRACTION + 1e-4,
            "went above the ceiling: {highest}"
        );
        // And it must actually use the range, or the animation is invisible.
        assert!(lowest < MIN_FRACTION + 0.02, "never reached the floor");
        assert!(highest > MAX_FRACTION - 0.02, "never reached the ceiling");
    }

    #[test]
    fn the_cycle_repeats_exactly_once_per_period() {
        for delay in DELAYS {
            for t in [0.0f32, 0.3, 1.1, 7.7] {
                let a = height_fraction(t, delay);
                let b = height_fraction(t + PERIOD, delay);
                assert!((a - b).abs() < 1e-4, "not periodic at t={t}");
            }
        }
    }

    /// The bars must not all be at the same height, or the row pumps as one
    /// block instead of rippling.
    #[test]
    fn the_bars_are_out_of_phase_with_each_other() {
        let heights: Vec<f32> = DELAYS.iter().map(|d| height_fraction(0.0, *d)).collect();
        let spread = heights.iter().cloned().fold(f32::MIN, f32::max)
            - heights.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread > 0.1, "the bars move together: spread {spread}");
    }

    /// A negative clock must not produce a negative phase.
    #[test]
    fn a_negative_time_is_handled() {
        for t in [-0.5f32, -100.0] {
            let f = height_fraction(t, 0.0);
            assert!((MIN_FRACTION..=MAX_FRACTION).contains(&f), "t={t} gave {f}");
        }
    }

    /// The point of the toggle: still means still, and identical at every
    /// moment. A "still" mark that still differed frame to frame would mean
    /// the switch had not actually turned anything off.
    #[test]
    fn a_stilled_mark_is_the_same_at_every_moment() {
        let ctx = egui::Context::default();
        let a = render_heights(&ctx, still(), 0.0);
        let b = render_heights(&ctx, still(), 12.34);
        assert_eq!(a, b, "the stilled mark changed between frames");

        let c = render_heights(&ctx, moving(), 0.0);
        let d = render_heights(&ctx, moving(), 0.6);
        assert_ne!(c, d, "the moving mark did not move");
    }

    /// And it must not keep the CPU awake once it is off.
    ///
    /// An "off" switch that still schedules a frame every 33 ms has turned the
    /// animation off visually and left the battery cost behind, which on a
    /// laptop is most of the reason somebody turned it off. Compared against
    /// the moving case rather than against an absolute sentinel, because what
    /// egui uses to mean "nothing pending" is its business and not a contract.
    #[test]
    fn a_stilled_mark_requests_no_repaint() {
        // Measured from a *settled* context. egui asks for an immediate
        // repaint on the first frames whatever is drawn, because layout needs
        // a second pass to converge -- so a single `run` reports zero for the
        // still case too, and the test would pass or fail for a reason that
        // has nothing to do with the mark.
        let soonest = |motion: Motion| {
            let ctx = egui::Context::default();
            let mut delay = std::time::Duration::MAX;
            for _ in 0..4 {
                let output = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        draw(ui, egui::vec2(120.0, 32.0), motion, 1.0);
                    });
                });
                delay = output
                    .viewport_output
                    .values()
                    .map(|v| v.repaint_delay)
                    .min()
                    .unwrap_or(std::time::Duration::MAX);
            }
            delay
        };

        let moving_delay = soonest(moving());
        let still_delay = soonest(still());

        assert!(
            moving_delay <= std::time::Duration::from_millis(100),
            "a moving mark did not ask to be repainted promptly: {moving_delay:?}"
        );
        assert!(
            still_delay > moving_delay,
            "a stilled mark asked to be repainted as eagerly as a moving one \
             ({still_delay:?} vs {moving_delay:?})"
        );
        assert!(
            still_delay >= std::time::Duration::from_secs(1),
            "a stilled mark is still driving the frame rate: {still_delay:?}"
        );
    }

    /// Drawing must not panic at any size, including degenerate ones a layout
    /// can genuinely produce while a window is being resized.
    #[test]
    fn any_size_can_be_drawn() {
        let ctx = egui::Context::default();
        for size in [
            egui::vec2(0.0, 0.0),
            egui::vec2(1.0, 1.0),
            egui::vec2(120.0, 32.0),
            egui::vec2(4000.0, 2.0),
        ] {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw(ui, size, moving(), 3.0);
                });
            });
        }
    }

    /// The delays are copied from the website's markup; if that list changes
    /// and this one does not, the two marks stop matching.

    #[test]
    fn an_unfocused_window_does_not_animate() {
        // The case that makes a laptop warm: the application is open behind
        // something else and nobody can see the mark at all.
        let ctx = egui::Context::default();
        let rect = SOMEWHERE();
        // Settle it first, focused, so the "unmoved for a quarter second"
        // condition is satisfied and focus is the only thing being tested.
        for step in 0..4 {
            heights_for(
                &ctx,
                frame(Some(true), Some(rect), step as f64 * 0.2),
                moving(),
                step as f32 * 0.2,
            );
        }

        let first = heights_for(&ctx, frame(Some(false), Some(rect), 2.0), moving(), 2.0);
        let later = heights_for(&ctx, frame(Some(false), Some(rect), 2.9), moving(), 2.9);
        assert_eq!(
            first, later,
            "the mark moved with the window unfocused, half a period apart"
        );

        let delay = soonest_after(&ctx, frame(Some(false), Some(rect), 3.5), moving(), 3.5);
        assert!(
            delay >= std::time::Duration::from_secs(1),
            "an unfocused window is still driving the frame rate: {delay:?}"
        );
    }

    #[test]
    fn a_window_being_dragged_does_not_animate() {
        // The reported symptom: dragging the window is jerky. Every frame of
        // a drag delivers a new rectangle, and each one used to come with a
        // full redraw this animation had asked for.
        let ctx = egui::Context::default();
        for step in 0..4 {
            heights_for(
                &ctx,
                frame(Some(true), Some(SOMEWHERE()), step as f64 * 0.2),
                moving(),
                step as f32 * 0.2,
            );
        }
        let settled = heights_for(
            &ctx,
            frame(Some(true), Some(SOMEWHERE()), 1.0),
            moving(),
            1.0,
        );

        // Now move it, one frame at a time, as a drag does.
        let mut dragged = Vec::new();
        for step in 0..6 {
            let moved = SOMEWHERE().translate(egui::vec2(step as f32 * 7.0, 0.0));
            let time = 1.0 + step as f64 * 0.05;
            dragged.push(heights_for(
                &ctx,
                frame(Some(true), Some(moved), time),
                moving(),
                time as f32,
            ));
        }
        for (index, shape) in dragged.iter().enumerate() {
            assert_eq!(
                shape, &settled,
                "the mark advanced on drag frame {index}, which is the redraw \
                 that competes with the window move"
            );
        }
    }

    #[test]
    fn the_mark_starts_again_once_the_window_stops() {
        // Pausing during a drag is only correct if it resumes afterwards. A
        // permanently frozen logo is a different bug, not a fix.
        let ctx = egui::Context::default();
        let rect = SOMEWHERE();
        let paused = heights_for(
            &ctx,
            frame(Some(true), Some(rect.translate(egui::vec2(9.0, 0.0))), 1.0),
            moving(),
            1.0,
        );

        // Hold still for longer than SETTLE, then look again half a period on.
        let mut latest = paused.clone();
        for step in 1..8 {
            let time = 1.0 + step as f64 * 0.2;
            latest = heights_for(
                &ctx,
                frame(Some(true), Some(rect), time),
                moving(),
                time as f32,
            );
        }
        assert_ne!(
            latest, paused,
            "the mark never resumed after the window stopped moving"
        );
    }

    #[test]
    fn a_paused_mark_keeps_the_shape_it_had() {
        // Freezing at the resting midpoint would make every click into another
        // window snap the row flat, which is a new visible glitch traded for
        // the old invisible one.
        let ctx = egui::Context::default();
        let rect = SOMEWHERE();
        let mut running = Vec::new();
        for step in 0..6 {
            let time = step as f64 * 0.2;
            running = heights_for(
                &ctx,
                frame(Some(true), Some(rect), time),
                moving(),
                time as f32,
            );
        }
        let frozen = heights_for(&ctx, frame(Some(false), Some(rect), 1.4), moving(), 1.4);
        assert_eq!(
            frozen, running,
            "losing focus changed the shape of the mark rather than holding it"
        );
    }

    #[test]
    fn a_platform_that_reports_nothing_still_animates() {
        // No focus and no window rectangle: a mark frozen for ever there would
        // be a worse failure than one that animates when it need not.
        let ctx = egui::Context::default();
        let mut delay = std::time::Duration::MAX;
        for step in 0..4 {
            delay = soonest_after(
                &ctx,
                frame(None, None, step as f64 * 0.1),
                moving(),
                step as f32 * 0.1,
            );
        }
        assert!(
            delay <= std::time::Duration::from_millis(1000 / FRAMES_PER_SECOND + 10),
            "a platform reporting neither focus nor geometry stopped the mark: {delay:?}"
        );
    }

    #[test]
    fn the_frame_rate_is_the_documented_one() {
        let ctx = egui::Context::default();
        let rect = SOMEWHERE();
        let mut delay = std::time::Duration::MAX;
        for step in 0..6 {
            delay = soonest_after(
                &ctx,
                frame(Some(true), Some(rect), step as f64 * 0.2),
                moving(),
                step as f32 * 0.2,
            );
        }
        // What comes back is not what was asked for, and the difference is
        // deliberate on egui's side: it subtracts one predicted frame from
        // every delay so a repaint does not land late. So the reported figure
        // is up to one frame short of the request, and a test comparing them
        // for equality is asserting the frame rate of the machine it runs on.
        let requested = std::time::Duration::from_millis(1000 / FRAMES_PER_SECOND);
        let one_frame = std::time::Duration::from_millis(17);
        assert!(
            delay <= requested,
            "asked to be repainted later than the rate this module documents: \
             {delay:?} against {requested:?}"
        );
        assert!(
            delay + one_frame >= requested,
            "asked to be repainted far sooner than this module documents: \
             {delay:?} against {requested:?}"
        );
        // A constant, so the compiler settles it: the whole point of this
        // change was to ask for fewer whole-window redraws than the thirty a
        // second it used to.
        const { assert!(FRAMES_PER_SECOND < 30) };
    }

    #[test]
    fn the_delays_match_the_website_markup() {
        let html = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/index.html"),
        )
        .expect("index.html should be readable");
        let start = html
            .find("class=\"veil\"")
            .expect("no veil mark on the page");
        let end = html[start..].find("</div>").expect("unterminated veil") + start;
        let block = &html[start..end];

        let mut found: Vec<f32> = Vec::new();
        for piece in block.split("animation-delay:").skip(1) {
            let ms: String = piece.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(v) = ms.parse::<f32>() {
                found.push(v / 1000.0);
            }
        }
        assert_eq!(
            found.len(),
            DELAYS.len(),
            "the website has {} bars, the app has {}",
            found.len(),
            DELAYS.len()
        );
        for (i, (site, app)) in found.iter().zip(DELAYS.iter()).enumerate() {
            assert!(
                (site - app).abs() < 1e-4,
                "bar {i}: website {site}s, app {app}s"
            );
        }
    }

    /// Render once and read the bar heights back out of the paint list.
    fn render_heights(ctx: &egui::Context, motion: Motion, time: f32) -> Vec<u32> {
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw(ui, egui::vec2(120.0, 40.0), motion, time);
            });
        });
        let mut heights = Vec::new();
        for clipped in output.shapes {
            if let egui::Shape::Rect(r) = &clipped.shape {
                // Quantised, so floating-point noise does not make two
                // identical frames look different.
                heights.push((r.rect.height() * 100.0) as u32);
            }
        }
        heights
    }
}
