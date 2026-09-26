// SPDX-License-Identifier: GPL-3.0-or-later
//! How far through a job is, in the two cases that exist: the ones that can
//! honestly say, and the ones that cannot.
//!
//! # Why a bar is a claim
//!
//! A progress bar is read as a promise about how much longer. Drawn over a job
//! whose length nobody knows it is a lie that gets worse the longer it is on
//! screen: it crawls to nine tenths, sits there, and the person watching learns
//! that this window's bars mean nothing. Roadmap item 167 says an estimate is
//! given **only where one can honestly be given**, and the example it gives is
//! the distinction this module is built on. Hashing a file of known length can
//! say how far through it is, because the total is a number the operating
//! system will tell you before the work starts. Deriving a key cannot: it is
//! one long computation with no countable middle, and the honest thing is to
//! say that rather than to draw something.
//!
//! So [`Reach`] has two constructors and they are not interchangeable.
//! [`Reach::counting`] is for a job with a total, and draws a bar.
//! [`Reach::unmeasurable`] is for a job without one, takes the reason as an
//! argument, and draws a spinner with that reason beside it. There is no third
//! constructor that guesses.
//!
//! # Why atomics rather than a channel
//!
//! [`crate::offthread::Answer`] is the shape for **one** value that arrives
//! once. This is the other shape: a number that changes thousands of times
//! while the drawing thread reads it every frame. A channel would deliver
//! sixty thousand messages nobody wants, and a `Mutex` would be a lock the
//! drawing thread waits on while a worker holds it, which is the whole class of
//! defect roadmap item 167 exists to remove.
//!
//! `Relaxed` is enough for all of it. Nothing here is ordered against anything
//! else: the worst a stale read can produce is a bar one frame behind, and a
//! bar one frame behind is what every bar is.
//!
//! # Where the repaint request lives
//!
//! In [`strip`], not at the call sites. A window with nothing happening in it
//! stops drawing, so a counter that advanced would not be seen until somebody
//! moved the mouse, which is precisely what a frozen window looks like. Left to
//! the caller that would be one line to forget per panel, and a fix written at
//! the wrong scope is how F-210, F-216, F-219 and F-220 each happened.
//!
//! # One indicator, and the reduce-motion setting
//!
//! Roadmap item 169. [`strip`] is the only thing in this crate that draws "work
//! is happening", and a test refuses a bar or a spinner anywhere else. It takes
//! the resolved [`Motion`] rather than reading the preference itself, because
//! the preference is resolved once per frame against the system setting and the
//! environment override, and a second reader of it is a second answer.
//!
//! Under reduced motion the indeterminate case becomes a still bar rather than a
//! travelling one. A spinner cannot honour that setting at all, which is why
//! there is no spinner here: `ui.spinner()` animates unconditionally, so twelve
//! panels drawing one were twelve panels ignoring somebody who had said movement
//! hurts.
//!
//! # In plain words
//!
//! This is the little indicator that says something is happening.
//!
//! Where the program knows how much work there is, it shows how far through it
//! is. Where it does not know, it says so in a sentence instead of drawing a bar
//! that would be making it up.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use egui::{RichText, Ui};

use crate::prefs::Motion;
use crate::theme::palette as p;

/// Why a key derivation cannot say how far through it is.
///
/// Named here because two panels derive a key, the app lock and the vault, and
/// it is the same fact about the same operation: written out in both it would be
/// two sentences to keep in step, which this project's house rule forbids for
/// exactly the reason that they would not be kept. It is also the roadmap's own
/// example of the case where no honest estimate exists.
pub const KEY_DERIVATION: &str = "deriving a key is one long computation with no \
     countable middle, which is the same property that makes a passphrase \
     expensive to guess";

/// How far through one job is.
///
/// Held in an [`std::sync::Arc`] and shared with the worker: the worker counts,
/// the drawing thread reads, and neither waits for the other.
pub struct Reach {
    /// Units finished. Whatever the unit is, it is the same one as `total`.
    done: AtomicU64,
    /// Units in the whole job, or zero for a job with no honest total.
    total: AtomicU64,
    /// Why no fraction can be given, empty when one can.
    ///
    /// `&'static str` rather than `String` because the reason is a property of
    /// the kind of work, decided where the code is written, not something a
    /// running job discovers about itself.
    because: &'static str,
}

impl Reach {
    /// A job of `total` countable units.
    ///
    /// A total of zero is treated as "cannot say" rather than "already
    /// finished", because a bar over nothing is meaningless either way, and a
    /// job that learns its real total later calls [`set_total`](Reach::set_total).
    pub fn counting(total: u64) -> Self {
        Self {
            done: AtomicU64::new(0),
            total: AtomicU64::new(total),
            because: "",
        }
    }

    /// A job whose length cannot be known, and the reason, which is shown.
    ///
    /// The reason is written as the clause after "because": [`strip`] supplies
    /// the rest of the sentence, so the phrasing is the same in every panel and
    /// the fact is at the one place that knows it.
    pub fn unmeasurable(because: &'static str) -> Self {
        Self {
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            because,
        }
    }

    /// Count `by` more units finished. Called from the worker.
    pub fn advance(&self, by: u64) {
        self.done.fetch_add(by, Ordering::Relaxed);
    }

    /// Say what the total really is, for a job that could only count it once it
    /// had started. Refused on a job declared unmeasurable: a reason on screen
    /// and a bar beside it would be two answers to one question.
    pub fn set_total(&self, total: u64) {
        if self.because.is_empty() {
            self.total.store(total, Ordering::Relaxed);
        }
    }

    /// Units finished so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// The whole job, or zero where there is no honest total.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// How far through, in `[0, 1]`, or `None` where that cannot be said.
    ///
    /// Clamped, because the two numbers are read separately and a worker that
    /// counts one chunk more than it promised must not draw a bar past its own
    /// end. Being slightly wrong about the last percent is ordinary; drawing
    /// outside the box is a bug somebody reports.
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            return None;
        }
        Some((self.done() as f32 / total as f32).clamp(0.0, 1.0))
    }

    /// The reason no estimate is given, for a job that cannot give one.
    pub fn because(&self) -> Option<&'static str> {
        if self.because.is_empty() {
            None
        } else {
            Some(self.because)
        }
    }
}

/// The indicator, beside the control that started the work.
///
/// `saying` is what is happening, in words, in the present tense and lower
/// case, the way the rest of this interface says things: "hashing the
/// download", not "Hashing...". Where a fraction can honestly be given it is
/// added to those words as a percentage, so the number is readable as well as
/// drawn.
///
/// One function rather than a shape each panel draws for itself. Roadmap item
/// 169 asked for that in as many words, and the window had earned the ask: a
/// spinner in some places, a travelling bar in Setup, a changed label in others
/// and nothing at all in most.
pub fn strip(ui: &mut Ui, saying: &str, reach: &Reach, motion: Motion) {
    // Ten frames a second while a job runs, asked for here for the reason in
    // the module note: a window with nothing else happening in it has stopped
    // drawing, and an indicator nobody sees advance is not an indicator.
    ui.ctx().request_repaint_after(Duration::from_millis(100));

    let fraction = reach.fraction();
    ui.horizontal(|ui| {
        bar(ui, fraction, motion);
        let words = match fraction {
            Some(fraction) => format!("{saying}, {}%", (fraction * 100.0).round() as u32),
            None => saying.to_string(),
        };
        ui.label(RichText::new(words).small().color(p::muted()));
    });
    if let Some(because) = reach.because() {
        ui.label(
            RichText::new(format!(
                "How far through cannot be said, because {because}."
            ))
            .small()
            .color(p::muted()),
        );
    }
}

/// How wide the bar is drawn, and how tall.
///
/// Narrow enough that the sentence saying what is happening fits beside it on a
/// window this application can be resized to, which is what "beside the control
/// that started it" needs to mean in a panel rather than a dialog.
const BAR: (f32, f32) = (120.0, 8.0);

/// The bar itself: filled to a fraction, travelling, or still.
///
/// Three cases and each is a different claim.
///
/// **A fraction** fills to it. This keeps moving under reduced motion, and
/// deliberately: the setting is about decoration, and a bar that stops
/// reporting the thing it is for would be answering a request not to animate by
/// withholding information. What changes is that nothing moves except the
/// measurement.
///
/// **No fraction, motion allowed** is a quarter-width highlight travelling left
/// to right. It says the window is alive without claiming to know a percentage,
/// which it does not: `reg.exe` and a package manager report nothing as they go,
/// and a bar that fills to nine tenths and waits is a lie with a shape.
///
/// **No fraction, motion reduced** is a third of the bar, still. Still but not
/// empty, because an empty bar reads as stuck, and somebody who has asked their
/// system for no movement has asked for this rather than for a spinner they
/// cannot stop. That is the whole of roadmap item 169's second half: the
/// setting is somebody saying movement hurts, and there is no reading of that
/// which permits one exception per panel.
fn bar(ui: &mut Ui, fraction: Option<f32>, motion: Motion) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().min(BAR.0), BAR.1),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, p::bg_dark());
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, p::border()),
        egui::StrokeKind::Inside,
    );

    match fraction {
        Some(fraction) => {
            let mut lit = rect;
            lit.set_width(rect.width() * fraction);
            if lit.width() > 0.0 {
                painter.rect_filled(lit, 3.0, p::blue());
            }
        }
        None if motion.enabled => {
            // Read from the frame's own clock rather than a stored instant, so
            // two indicators on screen travel together instead of each keeping
            // time from whenever its panel was first drawn.
            let time = ui.input(|i| i.time) as f32;
            let width = rect.width() * 0.25;
            let travel = rect.width() + width;
            let position = (time * 0.45).fract() * travel - width;
            let mut lit = rect;
            lit.min.x = rect.min.x + position.max(0.0);
            lit.max.x = (rect.min.x + position + width).min(rect.max.x);
            if lit.max.x > lit.min.x {
                painter.rect_filled(lit, 3.0, p::blue());
            }
        }
        None => {
            let mut lit = rect;
            lit.set_width(rect.width() * 0.35);
            painter.rect_filled(lit, 3.0, p::blend(p::blue(), p::bg_dark(), 0.45));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing in this crate draws an egui progress bar.
    ///
    /// The type makes an honest indicator easy and [`strip`] makes it the
    /// default, but neither stops a panel from adding a bar of its own with a
    /// number it made up, and that is the defect this module exists to prevent
    /// rather than to discourage. So it is read out of the source.
    ///
    /// The rule is not that bars are bad. It is that a bar is a claim about how
    /// much longer, and the only code here that knows whether such a claim can
    /// be made is the code that was told a total. A panel that genuinely has one
    /// passes it to [`Reach::counting`] and gets a bar drawn by [`bar`], which is
    /// why this module needs no exception from its own rule: the drawing here is
    /// a painted rectangle, and `egui::ProgressBar` is used nowhere at all.
    ///
    /// Written the way the spawn guards are written, for the reason F-210 and
    /// F-220 both gave: a rule enforced over the file that prompted it is a rule
    /// the next file does not have.
    #[test]
    fn nothing_in_this_crate_draws_a_progress_bar() {
        // Split so this test's own source is not what it finds.
        const NEEDLE: &str = concat!("ProgressBar", "::new");
        let mut bars = Vec::new();
        for (module, source) in crate::sources() {
            // Cut at the test marker: a test that searches for a name contains
            // that name, and a guard that trips over its own reasoning is one
            // somebody deletes.
            let shipped = match source.find("#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            for (number, line) in shipped.lines().enumerate() {
                if line.contains(NEEDLE) {
                    bars.push(format!("{module}:{}: {}", number + 1, line.trim()));
                }
            }
        }
        assert!(
            bars.is_empty(),
            "a bar is drawn outside `crate::progress`, so nothing checked that \
             there is an honest total behind it. Hold a `Reach` and call \
             `progress::strip`:\n{}",
            bars.join("\n")
        );
    }

    /// Nowhere else in this crate draws a spinner.
    ///
    /// **Roadmap item 169.** Twelve panels drew `ui.spinner()`, which animates
    /// whatever the system has been asked for: egui's spinner takes no setting
    /// and reads none, so every one of them was a panel that ignored somebody
    /// who had said movement hurts. There is no spinner in this module either.
    /// The indeterminate case is a bar, travelling or still, because a bar can
    /// hold still and a spinner cannot.
    ///
    /// The whole crate is read, not the files that had one, which is the lesson
    /// of F-210 and F-220 applied before rather than after.
    #[test]
    fn nothing_in_this_crate_draws_a_spinner() {
        // Split so this test's own source is not what it finds.
        const NEEDLE: &str = concat!("ui.spinner", "()");
        let mut spinners = Vec::new();
        for (module, source) in crate::sources() {
            let shipped = match source.find("#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            for (number, line) in shipped.lines().enumerate() {
                let trimmed = line.trim_start();
                // Prose: the reason there is no spinner is discussed in several
                // of these files, and a guard that trips over its own
                // explanation is one somebody deletes.
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                if line.contains(NEEDLE) {
                    spinners.push(format!("{module}:{}: {}", number + 1, trimmed));
                }
            }
        }
        assert!(
            spinners.is_empty(),
            "a spinner is drawn here, and a spinner cannot honour the \
             reduce-motion setting. Call `progress::strip` with a `Reach`, \
             which holds still when movement is not wanted:\n{}",
            spinners.join("\n")
        );
    }

    /// Under reduced motion the indeterminate indicator does not move.
    ///
    /// Driven rather than read, because this is the one property of roadmap item
    /// 169 that a reader of the source can be wrong about: the branch is there
    /// and what it paints is the question. Two frames a second apart, and the
    /// rectangles that came out of them are compared.
    ///
    /// With motion allowed they differ, which is asserted in the same test.
    /// Without that half, a version that painted nothing at all in either case
    /// would pass.
    #[test]
    fn a_reduced_motion_indicator_holds_still() {
        fn rectangles(motion: Motion, time: f64) -> Vec<u32> {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let input = egui::RawInput {
                time: Some(time),
                ..Default::default()
            };
            let output = crate::headless_frame(&ctx, input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    strip(
                        ui,
                        "doing something that cannot be counted",
                        &Reach::unmeasurable("this is a test"),
                        motion,
                    );
                });
            });
            let mut edges = Vec::new();
            for clipped in output.shapes {
                if let egui::Shape::Rect(rect) = &clipped.shape {
                    // Quantised, so floating-point noise does not make two
                    // identical frames look different. Borrowed from the
                    // soundbar's tests, which needed it for the same reason.
                    edges.push((rect.rect.min.x * 100.0) as u32);
                    edges.push((rect.rect.max.x * 100.0) as u32);
                }
            }
            assert!(!edges.is_empty(), "the indicator painted nothing at all");
            edges
        }

        let still = crate::no_motion();
        assert_eq!(
            rectangles(still, 0.0),
            rectangles(still, 1.0),
            "the indicator moved between two frames although the system asked \
             for no movement"
        );

        let moving = Motion {
            enabled: true,
            icon: true,
            system_reduced: false,
        };
        assert_ne!(
            rectangles(moving, 0.0),
            rectangles(moving, 1.0),
            "the indicator is identical in two frames a second apart with \
             motion allowed, so the travelling half draws nothing and the test \
             above proves nothing"
        );
    }

    /// A job with a total says how far through it is, and says nothing else.
    #[test]
    fn a_job_that_can_count_says_how_far() {
        let reach = Reach::counting(1_000);
        assert_eq!(reach.fraction(), Some(0.0));
        reach.advance(250);
        assert_eq!(reach.fraction(), Some(0.25));
        reach.advance(750);
        assert_eq!(reach.fraction(), Some(1.0));
        assert!(
            reach.because().is_none(),
            "a job that can count has no reason to explain"
        );
    }

    /// A job without one says why rather than drawing a bar.
    ///
    /// This is the roadmap's own example, and the assertion is the whole point
    /// of the type: `fraction` returning `None` is what stops [`strip`] from
    /// inventing a number.
    #[test]
    fn a_job_that_cannot_count_says_why_rather_than_drawing_a_bar() {
        let reach = Reach::unmeasurable("a key derivation has no countable middle");
        assert_eq!(
            reach.fraction(),
            None,
            "an unmeasurable job offered a fraction, which is the invented bar"
        );
        assert_eq!(
            reach.because(),
            Some("a key derivation has no countable middle")
        );
        // Counting on one changes nothing: a job declared unmeasurable stays so
        // even if somebody later adds an `advance` call to its worker.
        reach.advance(9_000);
        assert_eq!(reach.fraction(), None);
        reach.set_total(10_000);
        assert_eq!(
            reach.fraction(),
            None,
            "a total was accepted on a job whose reason is already on screen"
        );
    }

    /// A worker that overcounts must not draw past the end of its own bar.
    #[test]
    fn a_bar_never_goes_past_its_end() {
        let reach = Reach::counting(10);
        reach.advance(25);
        assert_eq!(reach.fraction(), Some(1.0));
    }

    /// A total that is only known once the work has started still gets a bar.
    #[test]
    fn a_total_learned_late_turns_a_still_bar_into_a_filling_one() {
        let reach = Reach::counting(0);
        assert_eq!(
            reach.fraction(),
            None,
            "nothing is known yet, so nothing is claimed"
        );
        reach.advance(1);
        reach.set_total(4);
        assert_eq!(reach.fraction(), Some(0.25));
    }

    /// The counting is safe to do from the worker while the window reads it.
    ///
    /// Written as a test rather than left to the word "atomic" because the
    /// alternative shape, a `Mutex` the worker holds while it works, is the one
    /// this module exists to avoid and it would pass every other test here.
    #[test]
    fn a_worker_counts_while_the_window_reads() {
        use std::sync::Arc;
        let reach = Arc::new(Reach::counting(10_000));
        let worker = {
            let reach = Arc::clone(&reach);
            std::thread::spawn(move || {
                for _ in 0..10_000 {
                    reach.advance(1);
                }
            })
        };
        // The drawing thread's half: reads, never waits, and every reading is
        // a number between the two ends.
        while !worker.is_finished() {
            let fraction = reach.fraction().expect("a counting job has a fraction");
            assert!((0.0..=1.0).contains(&fraction));
        }
        worker.join().expect("the counting thread panicked");
        assert_eq!(reach.done(), 10_000);
        assert_eq!(reach.fraction(), Some(1.0));
    }
}
