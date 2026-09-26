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
//! # What is deliberately left to roadmap item 169
//!
//! [`strip`] does not yet consult the reduce-motion setting, so the
//! no-estimate case spins for somebody who has asked their system for no
//! movement. That is roadmap item 169's subject, which is one indicator used
//! everywhere and a static statement in place of a moving one, and it will be
//! added here rather than beside it. The twelve other places that draw a bare
//! `ui.spinner()` are 169's sweep for the same reason.
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

use crate::theme::palette as p;

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
/// download", not "Hashing...".
///
/// One function rather than a shape each panel draws for itself, because five
/// spellings of the same indicator is what roadmap item 169 is for fixing and
/// adding a sixth here would be writing the thing it has to undo.
pub fn strip(ui: &mut Ui, saying: &str, reach: &Reach) {
    // Ten frames a second while a job runs, asked for here for the reason in
    // the module note: a window with nothing else happening in it has stopped
    // drawing, and an indicator nobody sees advance is not an indicator.
    ui.ctx().request_repaint_after(Duration::from_millis(100));

    ui.horizontal(|ui| {
        match reach.fraction() {
            Some(fraction) => {
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(120.0)
                        .show_percentage(),
                );
            }
            None => {
                ui.spinner();
            }
        }
        ui.label(RichText::new(saying).small().color(p::muted()));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Nowhere else in this crate draws a bar.
    ///
    /// The type makes an honest indicator easy and [`strip`] makes it the
    /// default, but neither stops a panel from adding a bar of its own with a
    /// number it made up, and that is the defect this module exists to prevent
    /// rather than to discourage. So it is read out of the source.
    ///
    /// The rule is not that bars are bad. It is that a bar is a claim about how
    /// much longer, and the only code here that knows whether such a claim can
    /// be made is the code that was told a total. A panel that genuinely has one
    /// passes it to [`Reach::counting`] and gets the same bar.
    ///
    /// Written the way the spawn guards are written, for the reason F-210 and
    /// F-220 both gave: a rule enforced over the file that prompted it is a rule
    /// the next file does not have.
    #[test]
    fn only_this_module_may_draw_a_progress_bar() {
        // Split so this test's own source is not what it finds.
        const NEEDLE: &str = concat!("ProgressBar", "::new");
        let mut bars = Vec::new();
        for (module, source) in crate::sources() {
            if module == "progress.rs" {
                continue;
            }
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
    fn a_total_learned_late_turns_the_spinner_into_a_bar() {
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
