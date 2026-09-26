// SPDX-License-Identifier: GPL-3.0-or-later
//! One answer, worked out somewhere that is not the thread drawing the window.
//!
//! # What this is for
//!
//! A panel often needs something only the machine can say: how much room is
//! free, which sound devices exist, whether a file is there. Each of those is
//! one line to write and none of them is quick. Written in the obvious place,
//! inside the function that draws the panel, that line runs **once per frame**,
//! sixty times a second, for as long as the panel is on screen.
//!
//! That is not a theoretical cost. `veilvoice_setup::space::free_bytes` spawns
//! `df` on Unix and `fsutil.exe` on Windows, so the setup tour's last card
//! forked a process per frame. `devices::list` enumerates the audio hardware,
//! which on Windows is a COM call measured in tens of milliseconds. See F-216.
//!
//! # The shape
//!
//! [`Answer`] holds one value that somebody else works out. The drawing
//! function asks for it once, draws whatever has arrived, and never waits:
//!
//! ```ignore
//! self.machine.ask_once(ui.ctx(), measure);   // spawns, returns at once
//! match self.machine.get() {
//!     Some(machine) => draw(ui, machine),
//!     // Still being read. One indicator, from `crate::progress`, which says
//!     // what is happening and honours the reduce-motion setting.
//!     None => crate::progress::strip(ui, "reading this machine", &reach, motion),
//! }
//! ```
//!
//! The thread with the news asks for the repaint, because it is the only thing
//! that knows there is any. That rule is why an untouched window draws nothing
//! at all, and a poller here would have quietly undone it.
//!
//! # What it deliberately does not do
//!
//! **It does not re-ask.** An answer is read once and kept, because the
//! questions it is for have answers that change rarely and are not worth a
//! process per frame to notice. Something that genuinely changes underfoot
//! asks again explicitly, by calling [`Answer::ask`].
//!
//! **It does not report the worker's death as an error.** A worker that
//! panicked drops its sender, and this then reads as "not waiting any more,
//! and the last answer still stands". A panel that showed a number goes on
//! showing the last true one rather than replacing it with a zero, which is
//! what `setup`'s companion probe already decided for itself and for the same
//! reason: an empty list reads as "you have none of these".
//!
//! # Why this is not [`crate::dialog::Pending`]
//!
//! `Pending` is the same shape for file dialogs and it stays separate, because
//! on macOS it must run its work **on** the drawing thread: `NSOpenPanel` can
//! be driven from nowhere else. A type whose whole promise is "not on the
//! drawing thread" cannot have a platform where that is false. The duplication
//! is fifteen lines and the alternative is a promise with an exception in it.

use std::sync::mpsc;

/// One value, worked out off the drawing thread.
///
/// `new` asks nothing. The first [`ask_once`](Answer::ask_once) starts the
/// work; [`get`](Answer::get) collects it when it arrives and never waits.
pub struct Answer<T> {
    waiting: Option<mpsc::Receiver<T>>,
    answer: Option<T>,
    asked: bool,
}

// Written out rather than derived: `derive(Default)` would demand `T: Default`,
// and the whole point is that the value does not exist yet.
impl<T> Default for Answer<T> {
    fn default() -> Self {
        Self {
            waiting: None,
            answer: None,
            asked: false,
        }
    }
}

impl<T: Send + 'static> Answer<T> {
    /// Nothing asked yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start the work, replacing anything already in flight.
    ///
    /// The context is cloned into the worker so that it can ask for a repaint
    /// when it has something to show. Without that the answer would sit in the
    /// channel until somebody moved the mouse, because a window with nothing
    /// happening in it is a window that has stopped drawing.
    pub fn ask(&mut self, ctx: &egui::Context, work: impl FnOnce() -> T + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.waiting = Some(rx);
        self.asked = true;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Sent before the repaint is asked for, so the frame that wakes up
            // finds the answer already in the channel rather than one frame
            // later. The send fails only if the panel that asked has gone,
            // which is ordinary and not worth reporting to nobody.
            let _ = tx.send(work());
            ctx.request_repaint();
        });
    }

    /// Start the work, unless it has been started before.
    ///
    /// This is what a drawing function calls: it runs every frame and must
    /// start one worker, not sixty a second.
    pub fn ask_once(&mut self, ctx: &egui::Context, work: impl FnOnce() -> T + Send + 'static) {
        if !self.asked {
            self.ask(ctx, work);
        }
    }

    /// The answer, if there is one. Never waits.
    ///
    /// Takes `&mut self` because collecting is what reading is: the value is
    /// pulled out of the channel here, on whichever frame happens to be first
    /// after it arrives.
    pub fn get(&mut self) -> Option<&T> {
        if let Some(rx) = &self.waiting {
            match rx.try_recv() {
                Ok(value) => {
                    self.answer = Some(value);
                    self.waiting = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                // The worker is gone without sending. See the module note: the
                // last answer stands rather than being replaced by nothing.
                Err(mpsc::TryRecvError::Disconnected) => self.waiting = None,
            }
        }
        self.answer.as_ref()
    }

    /// Whether the work is still running.
    ///
    /// For the panel that wants to say so. A button that has just been pressed
    /// going quiet is what reads as a freeze, and saying "still looking" is as
    /// much of the fix as the worker is.
    pub fn is_waiting(&self) -> bool {
        self.waiting.is_some()
    }

    /// Throw the answer away, so the next [`ask_once`](Answer::ask_once) asks
    /// again. For a panel that knows the machine has changed under it.
    pub fn forget(&mut self) {
        self.waiting = None;
        self.answer = None;
        self.asked = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context that is never shown. `request_repaint` on one of these is a
    /// no-op, which is all these tests need it to be.
    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    /// The answer arrives, and is kept once it has.
    #[test]
    fn the_answer_comes_back_and_stays() {
        let mut answer: Answer<u32> = Answer::new();
        assert!(
            answer.get().is_none(),
            "nothing was asked, so nothing is known"
        );
        answer.ask(&ctx(), || 7);
        // Polled rather than waited for, which is the whole point, so this
        // spins the way a window would rather than blocking on the channel.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while answer.get().is_none() && std::time::Instant::now() < deadline {
            std::hint::spin_loop();
        }
        assert_eq!(answer.get().copied(), Some(7));
        assert!(!answer.is_waiting(), "the channel was drained");
        // And again, from the kept value rather than the channel.
        assert_eq!(answer.get().copied(), Some(7));
    }

    /// Asking once is asking once, however many frames go past.
    ///
    /// This is the defect the type exists to prevent, so it is checked rather
    /// than left to the name: the work counts its own calls.
    #[test]
    fn a_thousand_frames_ask_one_worker() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let runs = Arc::new(AtomicUsize::new(0));
        let mut answer: Answer<()> = Answer::new();
        let ctx = ctx();
        for _ in 0..1_000 {
            let runs = Arc::clone(&runs);
            answer.ask_once(&ctx, move || {
                runs.fetch_add(1, Ordering::SeqCst);
            });
            let _ = answer.get();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while runs.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::hint::spin_loop();
        }
        assert_eq!(
            runs.load(Ordering::SeqCst),
            1,
            "the work ran more than once, which is the per-frame defect this \
             type exists to stop"
        );
    }

    /// A worker that dies leaves what was already on screen.
    #[test]
    fn a_dead_worker_leaves_the_last_answer() {
        let mut answer: Answer<u32> = Answer::new();
        let ctx = ctx();
        answer.ask(&ctx, || 3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while answer.get().is_none() && std::time::Instant::now() < deadline {
            std::hint::spin_loop();
        }
        assert_eq!(answer.get().copied(), Some(3));

        // A worker that panics drops its sender without sending.
        answer.ask(&ctx, || panic!("the worker died"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while answer.is_waiting() && std::time::Instant::now() < deadline {
            let _ = answer.get();
            std::hint::spin_loop();
        }
        assert!(!answer.is_waiting(), "the dead worker is not still awaited");
        assert_eq!(
            answer.get().copied(),
            Some(3),
            "the panel would have replaced a true answer with nothing"
        );
    }

    /// Forgetting asks again.
    #[test]
    fn forgetting_asks_again() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let runs = Arc::new(AtomicUsize::new(0));
        let mut answer: Answer<usize> = Answer::new();
        let ctx = ctx();
        for round in 0..2 {
            if round == 1 {
                answer.forget();
            }
            let runs = Arc::clone(&runs);
            answer.ask_once(&ctx, move || runs.fetch_add(1, Ordering::SeqCst) + 1);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while answer.get().is_none() && std::time::Instant::now() < deadline {
                std::hint::spin_loop();
            }
        }
        assert_eq!(runs.load(Ordering::SeqCst), 2, "the second ask never ran");
    }
}
