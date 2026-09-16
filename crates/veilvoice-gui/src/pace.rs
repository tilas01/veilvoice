// SPDX-License-Identifier: GPL-3.0-or-later
//! How often the window draws while something in it is moving, and what that
//! actually came to.
//!
//! # The number this replaces
//!
//! The animations ran at twenty frames a second by design. The mark in the
//! header carried its own constant, the busy path asked for a frame every
//! fifty milliseconds, and the veiling path every sixteen. Each was a
//! reasonable number on its own, and together they meant that on any display
//! somebody had bought in the last five years the window moved at a fraction
//! of the display's rate, with the busy path visibly juddering against the
//! mark beside it.
//!
//! Sixteen milliseconds is the interesting one. It is the number everybody
//! writes for sixty a second, and it is wrong for a window that waits for the
//! display: a display at sixty draws every 16.67 ms, so a request for a frame
//! "no later than sixteen milliseconds from now" wakes the loop just after the
//! frame it could have joined and the drawing lands on the one after. That is
//! thirty a second, asked for as sixty, and it is where "forty frames a second
//! on a good machine" came from.
//!
//! # What this does instead
//!
//! While something is moving, the window asks for the next frame **now**, and
//! lets vsync decide when that is. Under vsync a frame cannot be drawn faster
//! than the display shows it, so this costs one frame per display refresh and
//! not one more, and the rate is the display's own, whatever it is. Somebody
//! who wants fewer frames than that, on a battery or a machine that struggles,
//! sets a target in Settings and the window asks for a frame every `1/target`
//! seconds instead, which is the old behaviour with the number chosen rather
//! than hard-coded. Idle still draws nothing; this only decides the spacing of
//! frames that were going to be drawn anyway.
//!
//! # The display's rate is measured, not asked for
//!
//! Neither `egui` nor `eframe` says what the display's refresh rate is. It can
//! be measured: a run of frames requested back to back under vsync settles at
//! the display's rate, and the median interval over the last thirty-two frames
//! is a number a single slow frame cannot move. That median, rounded and
//! clamped to 30..=240, is what the About tab reports as the display and what
//! "match the display" means in Settings.
//!
//! # Dropped frames
//!
//! A frame that arrives more than one and a half times the expected interval
//! after the one before it is counted as dropped. The count is shown beside
//! the frame rate, and when more than a handful drop inside one second the
//! window says so in the header, with whether it is on software rendering,
//! because that is the first thing to check and the About tab is not where
//! somebody looks while it is happening.
//!
//! # Realtime
//!
//! `frame` runs once per drawn frame on the thread that draws. It allocates
//! nothing, locks nothing and prints nothing: the interval history is a fixed
//! ring, the median is taken over a copy of it on the stack, and the target
//! that other modules read is an atomic. The guard from roadmap item 126 reads this
//! file.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

/// The lowest rate a display is believed to have. Anything measured under this
/// is the window being starved, not a display.
pub const DISPLAY_FLOOR: u32 = 30;

/// The highest rate the window will run at, display or setting.
pub const DISPLAY_CEILING: u32 = 240;

/// The rate assumed until the display has been measured.
pub const ASSUMED: u32 = 60;

/// The targets Settings offers, besides "match the display".
pub const TARGETS: &[u32] = &[30, 60, 90, 120, 144, 165, 240];

/// How many intervals the median is taken over.
const WINDOW: usize = 32;

/// A frame this much later than expected is a dropped one.
const DROPPED_AT: f32 = 1.5;

/// More drops than this inside one second is worth saying out loud.
const NOTICE_AT: u32 = 5;

/// The interval every animation in the window paces itself by, in
/// microseconds. Zero means "ask for the next frame now and let vsync pace it".
///
/// An atomic rather than a field passed down, because the mark is drawn from
/// Settings' preview as well as from the header and neither of those has the
/// application in hand. Written once per frame by `Pace::frame`, read by
/// anything that moves.
static INTERVAL_MICROS: AtomicU32 = AtomicU32::new(0);

/// Set by [`next_frame`], read and cleared once per frame by [`Pace::frame`].
///
/// This is how the measurement knows whether the frame it is looking at was
/// asked for by an animation or by somebody moving the mouse, without every
/// animation in the window having to report itself. The mark in the header
/// asks for its own frames and is drawn from two places, neither of which has
/// the application in hand; this way it counts like everything else.
static ANIMATING: AtomicBool = AtomicBool::new(false);

/// Ask for the next frame the way the current target wants it asked for.
///
/// The one call an animation makes. With the target on the display, this is
/// `request_repaint`, and vsync spaces the frames; with a lower target it is
/// `request_repaint_after` the target's interval.
pub fn next_frame(ctx: &egui::Context) {
    ANIMATING.store(true, Ordering::Relaxed);
    let micros = INTERVAL_MICROS.load(Ordering::Relaxed);
    if micros == 0 {
        ctx.request_repaint();
    } else {
        ctx.request_repaint_after(Duration::from_micros(u64::from(micros)));
    }
}

/// What a person chose in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// As fast as the display shows frames, whatever that is.
    Display,
    /// This many a second, at most.
    Fixed(u32),
}

impl Target {
    /// From the preference as stored: zero is the display.
    pub fn from_setting(value: u32) -> Self {
        if value == 0 {
            Self::Display
        } else {
            Self::Fixed(value.clamp(DISPLAY_FLOOR, DISPLAY_CEILING))
        }
    }

    /// The preference to store.
    pub fn to_setting(self) -> u32 {
        match self {
            Self::Display => 0,
            Self::Fixed(hz) => hz,
        }
    }

    /// The words Settings shows for it.
    pub fn label(self) -> String {
        match self {
            Self::Display => "match the display".to_string(),
            Self::Fixed(hz) => format!("{hz} a second"),
        }
    }
}

/// The measurement, kept across frames.
#[derive(Debug)]
pub struct Pace {
    target: Target,
    /// Seconds between recent frames, oldest overwritten first.
    intervals: [f32; WINDOW],
    filled: usize,
    next: usize,
    last_time: Option<f64>,
    /// The display's rate as measured, once enough frames have been seen.
    display_hz: Option<u32>,
    /// Frames a second over the last whole second, as drawn.
    fps: f32,
    frames_this_second: u32,
    second_started: f64,
    dropped_total: u64,
    dropped_this_second: u32,
    dropped_last_second: u32,
    /// Consecutive whole seconds that dropped more than [`NOTICE_AT`] frames.
    dropping_seconds: u32,
}

impl Default for Pace {
    fn default() -> Self {
        Self::new(Target::Display)
    }
}

impl Pace {
    /// A fresh measurement with this target.
    pub fn new(target: Target) -> Self {
        let pace = Self {
            target,
            intervals: [0.0; WINDOW],
            filled: 0,
            next: 0,
            last_time: None,
            display_hz: None,
            fps: 0.0,
            frames_this_second: 0,
            second_started: 0.0,
            dropped_total: 0,
            dropped_this_second: 0,
            dropped_last_second: 0,
            dropping_seconds: 0,
        };
        pace.publish();
        pace
    }

    /// Change the target, keeping what has been measured.
    pub fn set_target(&mut self, target: Target) {
        self.target = target;
        self.publish();
    }

    /// The target as chosen.
    pub fn target(&self) -> Target {
        self.target
    }

    /// The rate the window is aiming at right now, in frames a second.
    pub fn target_hz(&self) -> u32 {
        match self.target {
            Target::Display => self.display_hz.unwrap_or(ASSUMED),
            Target::Fixed(hz) => hz,
        }
    }

    /// The display's rate as measured, if it has been.
    pub fn display_hz(&self) -> Option<u32> {
        self.display_hz
    }

    /// Frames a second over the last whole second of drawing.
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Every frame counted as dropped since the window opened.
    pub fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    /// Frames dropped in the last whole second.
    pub fn dropped_last_second(&self) -> u32 {
        self.dropped_last_second
    }

    /// Whether frames are being dropped steadily enough to be worth saying.
    ///
    /// Two consecutive seconds rather than one. Opening a window costs a
    /// hitch: fonts are rasterised, the first textures are uploaded, and the
    /// first second of almost any launch drops frames. Telling somebody their
    /// machine is struggling because of that would be crying wolf on every
    /// start.
    pub fn is_dropping(&self) -> bool {
        self.dropping_seconds >= 2
    }

    /// How many consecutive seconds have been dropping frames.
    pub fn dropping_seconds(&self) -> u32 {
        self.dropping_seconds
    }

    /// Record that a frame is being drawn at `time`, egui's clock in seconds.
    ///
    /// Called once per frame from the draw path, before anything else reads
    /// the numbers. Whether this frame was asked for by an animation is taken
    /// from the flag [`next_frame`] set at the end of the frame before, and
    /// only those frames are used to measure the display: the interval between
    /// two frames somebody caused by moving the mouse says nothing about how
    /// fast the screen is.
    pub fn frame(&mut self, time: f64) {
        let moving = ANIMATING.swap(false, Ordering::Relaxed);
        if self.second_started == 0.0 {
            self.second_started = time;
        }
        self.frames_this_second += 1;
        let elapsed = time - self.second_started;
        if elapsed >= 1.0 {
            self.fps = self.frames_this_second as f32 / elapsed as f32;
            self.frames_this_second = 0;
            self.second_started = time;
            self.dropped_last_second = self.dropped_this_second;
            if self.dropped_this_second > NOTICE_AT {
                self.dropping_seconds += 1;
            } else {
                self.dropping_seconds = 0;
            }
            self.dropped_this_second = 0;
        }

        let Some(last) = self.last_time else {
            self.last_time = Some(time);
            return;
        };
        self.last_time = Some(time);
        let interval = (time - last) as f32;
        // `is_finite` and a positive test rather than a negated comparison:
        // the clock can hand back the same instant twice, and a NaN here would
        // otherwise be counted as a frame.
        if !moving || !interval.is_finite() || interval <= 0.0 {
            // An idle gap is not a dropped frame, and neither is a clock that
            // did not move.
            return;
        }

        // Late against what was asked for.
        let expected = 1.0 / self.target_hz() as f32;
        if interval > expected * DROPPED_AT {
            self.dropped_total += 1;
            self.dropped_this_second += 1;
        }

        // Only frames paced by vsync say anything about the display. A fixed
        // target below it is the timer being measured, not the screen.
        if self.target == Target::Display {
            self.intervals[self.next] = interval;
            self.next = (self.next + 1) % WINDOW;
            self.filled = (self.filled + 1).min(WINDOW);
            if self.filled == WINDOW {
                let measured = self.median_hz();
                if self.display_hz != Some(measured) {
                    self.display_hz = Some(measured);
                    self.publish();
                }
            }
        }
    }

    /// The display's rate from the median interval, clamped to sense.
    fn median_hz(&self) -> u32 {
        // A copy on the stack: the ring must not be reordered.
        //
        // `select_nth_unstable_by` rather than a sort, and that is not a
        // micro-optimisation, it is the difference between allocating and
        // not. The stable `sort_by` takes a scratch buffer for a slice this
        // long, which would put an allocation in a function this file's own
        // notes promise does not allocate; the unstable selection is in
        // place and only has to get the middle element right, which is all a
        // median needs.
        let mut window = self.intervals;
        let (_, median, _) = window.select_nth_unstable_by(WINDOW / 2, |a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let median = *median;
        if !median.is_finite() || median <= 0.0 {
            return ASSUMED;
        }
        let hz = (1.0 / median).round();
        (hz as u32).clamp(DISPLAY_FLOOR, DISPLAY_CEILING)
    }

    /// Tell the animations what to ask for.
    fn publish(&self) {
        let micros = match self.target {
            Target::Display => 0,
            Target::Fixed(hz) => 1_000_000 / hz.max(1),
        };
        INTERVAL_MICROS.store(micros, Ordering::Relaxed);
    }

    /// The interval animations currently pace by, for tests and the About
    /// tab. `None` is "vsync decides".
    pub fn interval(&self) -> Option<Duration> {
        match self.target {
            Target::Display => None,
            Target::Fixed(hz) => Some(Duration::from_micros(1_000_000 / u64::from(hz.max(1)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames an animation asked for, at `hz`.
    fn run(pace: &mut Pace, hz: f64, frames: usize) {
        let mut t = 0.0;
        for _ in 0..frames {
            ANIMATING.store(true, Ordering::Relaxed);
            pace.frame(t);
            t += 1.0 / hz;
        }
    }

    /// One frame nothing asked for: somebody moved the mouse.
    fn idle_frame(pace: &mut Pace, t: f64) {
        ANIMATING.store(false, Ordering::Relaxed);
        pace.frame(t);
    }

    #[test]
    fn the_display_is_measured_from_the_frames_it_paced() {
        for hz in [60.0, 120.0, 144.0, 165.0, 240.0] {
            let mut pace = Pace::new(Target::Display);
            run(&mut pace, hz, WINDOW + 2);
            assert_eq!(pace.display_hz(), Some(hz as u32), "{hz}");
            assert_eq!(pace.target_hz(), hz as u32);
        }
    }

    #[test]
    fn one_slow_frame_does_not_move_the_measurement() {
        let mut pace = Pace::new(Target::Display);
        run(&mut pace, 144.0, WINDOW);
        let t = WINDOW as f64 / 144.0;
        ANIMATING.store(true, Ordering::Relaxed);
        pace.frame(t + 0.2);
        assert_eq!(pace.display_hz(), Some(144));
        assert_eq!(pace.dropped_total(), 1, "the slow frame is a dropped one");
    }

    #[test]
    fn a_fixed_target_paces_by_its_own_interval_and_measures_nothing() {
        let mut pace = Pace::new(Target::Fixed(30));
        assert_eq!(pace.interval(), Some(Duration::from_micros(33_333)));
        run(&mut pace, 30.0, WINDOW + 2);
        assert_eq!(pace.display_hz(), None, "a timer is not a display");
        assert_eq!(pace.target_hz(), 30);
        assert_eq!(pace.dropped_total(), 0);
    }

    #[test]
    fn the_setting_round_trips_and_is_clamped() {
        assert_eq!(Target::from_setting(0), Target::Display);
        assert_eq!(Target::from_setting(144), Target::Fixed(144));
        assert_eq!(Target::from_setting(1), Target::Fixed(DISPLAY_FLOOR));
        assert_eq!(Target::from_setting(1000), Target::Fixed(DISPLAY_CEILING));
        for hz in TARGETS {
            assert_eq!(Target::from_setting(*hz).to_setting(), *hz);
        }
        assert_eq!(Target::Display.to_setting(), 0);
    }

    #[test]
    fn idle_gaps_are_not_dropped_frames() {
        let mut pace = Pace::new(Target::Display);
        run(&mut pace, 60.0, 8);
        // The window sat idle for a while, then drew because the mouse moved.
        idle_frame(&mut pace, 30.0);
        assert_eq!(pace.dropped_total(), 0);
    }

    #[test]
    fn a_second_of_late_frames_is_worth_saying() {
        let mut pace = Pace::new(Target::Fixed(60));
        let mut t = 0.0;
        // Every frame at half the rate asked for, for more than two seconds:
        // one bad second is a hitch and deliberately does not raise this.
        for _ in 0..70 {
            ANIMATING.store(true, Ordering::Relaxed);
            pace.frame(t);
            t += 1.0 / 30.0;
        }
        assert!(pace.dropped_last_second() > NOTICE_AT);
        assert!(
            pace.is_dropping(),
            "two seconds of late frames is worth saying"
        );
        assert!((pace.fps() - 30.0).abs() < 2.0, "{}", pace.fps());
    }

    #[test]
    fn one_bad_second_at_launch_is_not_an_alarm() {
        // Opening a window costs a hitch. One second of it must not tell
        // somebody their machine is struggling.
        let mut pace = Pace::new(Target::Fixed(60));
        let mut t = 0.0;
        // One whole second of late frames and a little more, which is what a
        // launch looks like.
        for _ in 0..40 {
            ANIMATING.store(true, Ordering::Relaxed);
            pace.frame(t);
            t += 1.0 / 30.0;
        }
        assert!(
            pace.dropped_last_second() > NOTICE_AT,
            "the second did drop frames"
        );
        assert_eq!(pace.dropping_seconds(), 1);
        assert!(!pace.is_dropping(), "one second is a hitch, not a struggle");
    }

    #[test]
    fn the_published_interval_follows_the_target() {
        let mut pace = Pace::new(Target::Display);
        assert_eq!(INTERVAL_MICROS.load(Ordering::Relaxed), 0);
        pace.set_target(Target::Fixed(120));
        assert_eq!(INTERVAL_MICROS.load(Ordering::Relaxed), 8_333);
        pace.set_target(Target::Display);
        assert_eq!(INTERVAL_MICROS.load(Ordering::Relaxed), 0);
    }
}
