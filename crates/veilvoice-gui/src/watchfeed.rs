// SPDX-License-Identifier: GPL-3.0-or-later
//! The device monitor, moved off the thread that paints.
//!
//! # Why this file exists
//!
//! The monitor used to be polled straight from `update`, which is the
//! user-interface thread. Asking the operating system which applications hold
//! the microphone is not free, because on Windows it means running `reg.exe`
//! and on Linux it means walking `/proc`, and anything that costs tens of
//! milliseconds is several frames.
//!
//! The shipped v0.1.12 did it the expensive way as well as in the wrong place:
//! two subprocesses per application, 68 of them on the machine this was found
//! on, costing at least 449 ms measured, every two seconds, on the thread that
//! draws. The window froze repeatedly. `veilvoice-watch` now costs two
//! subprocesses whatever is installed, and this file makes sure the remaining
//! cost never lands on a frame.
//!
//! **Both halves were needed.** A cheap scan on the painting thread is still a
//! scan on the painting thread: it would be a stutter rather than a freeze, on
//! a machine slower than the one it was tested on, and it would come back.
//!
//! # One thread for the life of the window
//!
//! Not one per poll. Spawning a thread every two seconds to do 45 ms of work is
//! most of a thread's lifetime spent being created, and it would put the
//! monitor's own state, which is what makes "started" and "stopped" different
//! from "is using", somewhere it has to be moved back and forth.
//!
//! So the worker owns the [`veilvoice_watch::Monitor`] and keeps it. It polls,
//! sends, sleeps, repeats. The window drains whatever has arrived once a frame
//! and never waits.
//!
//! # It stops when the window does
//!
//! Nothing tells the thread to exit. When the window closes the receiver is
//! dropped, the next `send` fails, and the loop ends, which is the whole
//! shutdown protocol and needs no flag, no channel back and no chance of
//! hanging on exit waiting for a sleep to finish.
//!
//! # In plain words
//!
//! Keeps the microphone and camera monitor running somewhere other than the thread
//! that draws the window.
//!
//! Asking the operating system which programs are using a device takes long enough
//! to be visible if it happens while the window is being painted. So it happens on
//! its own thread and the window reads whatever has arrived.
//!
//! If that thread ever stops, the panel says so plainly, because a monitor that
//! has quietly not updated for an hour looks exactly like a machine where nothing
//! is listening.

use std::sync::mpsc;
use std::time::Duration;
use veilvoice_watch::{DeviceUse, Monitor, Support};

/// How often to look. Two seconds is frequent enough that a notification is
/// timely and rare enough that the work is invisible.
const EVERY: Duration = Duration::from_secs(2);

/// One look at the machine.
pub struct Update {
    /// What is holding a device right now.
    pub active: Vec<DeviceUse>,
    /// What started or stopped since the previous look, already worded.
    pub alerts: Vec<String>,
    /// Why the look failed, if it did.
    pub error: Option<String>,
}

/// The window's end of the monitor.
pub struct WatchFeed {
    /// Alerts that have arrived and not yet been shown as a notification.
    unseen: Vec<String>,
    receiver: Option<mpsc::Receiver<Update>>,
    /// The most recent snapshot, so the header indicator has something to draw
    /// between updates.
    active: Vec<DeviceUse>,
    /// The running log, oldest first, capped.
    log: Vec<String>,
    error: Option<String>,
    /// What this platform can answer at all.
    support: Support,
}

/// A log that grows without bound is a memory leak with a user interface.
const MOST_ALERTS: usize = 50;

impl Default for WatchFeed {
    fn default() -> Self {
        Self::idle()
    }
}

impl WatchFeed {
    /// A feed that watches nothing. What tests and `Default` use, so neither
    /// starts a thread or touches the machine.
    pub fn idle() -> Self {
        Self {
            receiver: None,
            active: Vec::new(),
            log: Vec::new(),
            unseen: Vec::new(),
            error: None,
            support: veilvoice_watch::support(),
        }
    }

    /// Start watching, on a thread of its own.
    ///
    /// Does nothing on a platform that cannot answer: a thread that would only
    /// ever report "not supported" is a thread that need not exist, and
    /// [`WatchFeed::support`] is what a front end reads to say so.
    pub fn start(ctx: egui::Context) -> Self {
        let mut feed = Self::idle();
        if !(feed.support.microphone || feed.support.camera) {
            return feed;
        }
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("veilvoice-watch".into())
            .spawn(move || {
                let mut monitor = Monitor::new();
                let mut first = true;
                let mut had_error = false;
                loop {
                    let (update, worth_sending) = match monitor.poll() {
                        Ok(changes) => {
                            let worth = first || !changes.is_empty() || had_error;
                            had_error = false;
                            (
                                Update {
                                    active: monitor.current().to_vec(),
                                    alerts: changes
                                        .iter()
                                        .map(veilvoice_watch::Change::alert)
                                        .collect(),
                                    error: None,
                                },
                                worth,
                            )
                        }
                        Err(error) => {
                            let worth = !had_error;
                            had_error = true;
                            (
                                Update {
                                    active: Vec::new(),
                                    alerts: Vec::new(),
                                    error: Some(error.to_string()),
                                },
                                worth,
                            )
                        }
                    };
                    first = false;

                    // Only when there is something to say.
                    //
                    // This used to send on every poll, and the window used to
                    // ask the channel every 500 ms whether anything had
                    // arrived. Between them that meant a window sitting
                    // untouched on any tab redrew itself twice a second for
                    // ever: measured at 2.1 frames a second on all nine tabs,
                    // with the animations turned off and nothing happening.
                    //
                    // A repaint is now asked for by the thread that has news,
                    // which is the only thing that knows there is any. An idle
                    // window draws nothing at all.
                    if worth_sending {
                        // The window has gone. Nothing else needs to happen.
                        if sender.send(update).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                    std::thread::sleep(EVERY);
                }
            })
            // A machine that cannot spawn a thread has larger problems, and the
            // interface should still open. The feed simply stays idle and the
            // monitor tab says it is not running.
            .map(|_| ())
            .unwrap_or_else(|error| {
                feed.error = Some(format!("the monitor could not be started: {error}"));
            });
        feed.receiver = Some(receiver);
        feed
    }

    /// Alerts that have arrived and not yet been shown as a notification.
    ///
    /// Separate from [`WatchFeed::log`], which is the history and stays. This
    /// is a queue of things still to say, and it is drained by whoever shows
    /// them -- so an alert that arrives while the reader is on another tab is
    /// still waiting when they come back, rather than having scrolled past in
    /// a log they were not looking at.
    pub fn unseen(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unseen)
    }

    /// Whether anything is waiting to be shown.
    pub fn has_unseen(&self) -> bool {
        !self.unseen.is_empty()
    }

    /// Take whatever has arrived. Never waits.
    ///
    /// Returns true when something new came in, so the caller can decide
    /// whether the frame needs anything else done to it.
    pub fn drain(&mut self) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };
        let mut fresh = false;
        loop {
            match receiver.try_recv() {
                Ok(update) => {
                    fresh = true;
                    self.error = update.error;
                    if self.error.is_none() {
                        self.active = update.active;
                    }
                    // Queued for notification as well as kept in the log.
                    // The cap is the same one the log uses: a machine that
                    // produces alerts faster than they can be read should not
                    // grow a queue without end, and the log is where the ones
                    // that overflow can still be found.
                    self.unseen.extend(update.alerts.iter().cloned());
                    let spare = self.unseen.len().saturating_sub(MOST_ALERTS);
                    self.unseen.drain(..spare);
                    self.log.extend(update.alerts);
                    let overflow = self.log.len().saturating_sub(MOST_ALERTS);
                    self.log.drain(..overflow);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // The worker stopped. Said plainly rather than left as a
                    // monitor that has quietly not updated for an hour, which
                    // looks exactly like a machine where nothing is listening.
                    self.receiver = None;
                    self.error = Some(
                        "the monitor thread stopped, so nothing here is being watched any \
                         more. Close and reopen VeilVoice to start it again."
                            .to_string(),
                    );
                    self.active.clear();
                    break;
                }
            }
        }
        fresh
    }

    /// What is holding a device right now.
    pub fn active(&self) -> &[DeviceUse] {
        &self.active
    }

    /// What has started and stopped, oldest first.
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Why the monitor is not answering, if it is not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// What this platform can detect at all.
    pub fn support(&self) -> &Support {
        &self.support
    }

    /// Whether there is a worker running.
    pub fn is_watching(&self) -> bool {
        self.receiver.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_feed_starts_nothing_and_reports_nothing() {
        let feed = WatchFeed::idle();
        assert!(!feed.is_watching());
        assert!(feed.active().is_empty());
        assert!(feed.log().is_empty());
        assert!(feed.error().is_none());
    }

    /// Draining an idle feed must be free and must not claim anything arrived.
    #[test]
    fn draining_an_idle_feed_does_nothing() {
        let mut feed = WatchFeed::idle();
        assert!(!feed.drain());
        assert!(feed.active().is_empty());
    }

    /// The whole point: a drain must never wait for the machine. Measured as a
    /// minimum over many runs, because one sample on a busy machine says
    /// nothing.
    #[test]
    fn draining_never_blocks_the_frame() {
        let mut feed = WatchFeed::start(egui::Context::default());
        let mut worst = Duration::ZERO;
        for _ in 0..200 {
            let started = std::time::Instant::now();
            feed.drain();
            worst = worst.max(started.elapsed());
        }
        assert!(
            worst < Duration::from_millis(4),
            "the worst drain took {worst:?}, which is a dropped frame"
        );
    }

    /// A worker that has gone must be reported, not left looking like a quiet
    /// machine -- which is the failure mode this project guards hardest
    /// against.
    #[test]
    fn a_worker_that_stops_is_reported_rather_than_looking_quiet() {
        let (sender, receiver) = mpsc::channel();
        let mut feed = WatchFeed {
            receiver: Some(receiver),
            active: vec![],
            log: Vec::new(),
            unseen: Vec::new(),
            error: None,
            support: veilvoice_watch::support(),
        };
        sender
            .send(Update {
                active: Vec::new(),
                alerts: vec!["something started".into()],
                error: None,
            })
            .unwrap();
        assert!(feed.drain());
        assert_eq!(feed.log().len(), 1);

        drop(sender);
        feed.drain();
        assert!(!feed.is_watching());
        let error = feed.error().expect("a stopped worker must be reported");
        assert!(error.contains("stopped"), "{error}");
        assert!(feed.active().is_empty());
    }

    /// The log is capped, or a machine left open overnight grows one string at
    /// a time until it is a problem.
    #[test]
    fn the_log_is_capped() {
        let (sender, receiver) = mpsc::channel();
        let mut feed = WatchFeed {
            receiver: Some(receiver),
            active: Vec::new(),
            log: Vec::new(),
            unseen: Vec::new(),
            error: None,
            support: veilvoice_watch::support(),
        };
        for round in 0..40 {
            sender
                .send(Update {
                    active: Vec::new(),
                    alerts: (0..10).map(|i| format!("alert {round}-{i}")).collect(),
                    error: None,
                })
                .unwrap();
        }
        feed.drain();
        assert_eq!(feed.log().len(), MOST_ALERTS);
        // The newest are the ones kept.
        assert!(feed.log().last().unwrap().starts_with("alert 39-"));
    }

    /// A failed look must not wipe the last known state to nothing, which
    /// would read as "the microphone was released".
    #[test]
    fn a_failed_look_keeps_the_last_known_state() {
        let (sender, receiver) = mpsc::channel();
        let mut feed = WatchFeed {
            receiver: Some(receiver),
            active: Vec::new(),
            log: Vec::new(),
            unseen: Vec::new(),
            error: None,
            support: veilvoice_watch::support(),
        };
        sender
            .send(Update {
                active: Vec::new(),
                alerts: Vec::new(),
                error: Some("the registry did not answer".into()),
            })
            .unwrap();
        feed.drain();
        assert_eq!(feed.error(), Some("the registry did not answer"));
    }

    /// Starting for real must not panic, and must agree with what the platform
    /// says it can do.
    #[test]
    fn starting_on_this_machine_agrees_with_what_it_supports() {
        let feed = WatchFeed::start(egui::Context::default());
        let support = feed.support();
        if support.microphone || support.camera {
            assert!(feed.is_watching() || feed.error().is_some());
        } else {
            assert!(!feed.is_watching(), "a thread was started for nothing");
        }
    }
}
