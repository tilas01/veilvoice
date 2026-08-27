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
use egui::{Color32, Rect, Rounding, Sense, Ui, Vec2};

/// Seconds for one full rise and fall. Matches the website's `1.9s`.
const PERIOD: f32 = 1.9;

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

/// Draw the mark at `size`, returning the response so it can carry a tooltip.
///
/// `time` is the application clock in seconds. When `motion` disallows
/// movement every bar is drawn at its resting height and nothing is scheduled.
pub fn draw(ui: &mut Ui, size: Vec2, motion: Motion, time: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let bars = DELAYS.len();
    // A gap of a quarter of a bar's width, as on the website (5 px bar, 3 px
    // gap is close to a third, but a quarter reads better at icon sizes).
    let unit = rect.width() / (bars as f32 * 1.25);
    let bar_width = unit;
    let gap = unit * 0.25;
    let total = bars as f32 * bar_width + (bars as f32 - 1.0) * gap;
    let left = rect.center().x - total / 2.0;

    let painter = ui.painter();
    let rounding = Rounding::same((bar_width * 0.4).min(2.0));

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
    if motion.icon {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
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
