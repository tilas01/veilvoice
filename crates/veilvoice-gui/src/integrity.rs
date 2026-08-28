// SPDX-License-Identifier: GPL-3.0-or-later
//! The integrity record, taken and checked by the window rather than by hand.
//!
//! # What this adds to `veilvoice-guard`
//!
//! Nothing, cryptographically. Every hash, every comparison and every honest
//! limit is [`veilvoice_guard`]'s, and [`veilvoice_guard::SCOPE`] is what the
//! interface prints. What this module adds is that it happens at all: the
//! command-line `veilvoice guard init` has always been there and has always
//! been a thing somebody had to know to run.
//!
//! # When it runs
//!
//! At the first launch that finds no record, one is taken. At every launch
//! after that, the record is checked. Both happen on a worker thread, because
//! reading and hashing the installed files is disk work and the drawing thread
//! does none.
//!
//! # Sealing, and the passphrase problem underneath it
//!
//! A record written in the clear beside the files it describes is rewritten by
//! anybody who can change those files. Sealing it under a passphrase raises
//! that to needing the passphrase as well, which is a real improvement and is
//! what [`veilvoice_guard::Manifest::seal`] is for.
//!
//! The awkward part is which passphrase, and when. A record cannot be sealed
//! by a program that has no secret, and at the moment a window opens it has
//! none. So:
//!
//! * With an app lock set, the record is sealed under the **app-lock
//!   passphrase**, and is taken and checked at the moment of unlocking, which
//!   is the one moment that passphrase exists. That is the arrangement worth
//!   having.
//! * With no app lock, the record is written **in the clear** and the interface
//!   says so, in those words. It still catches accidental corruption, a failed
//!   update and a careless overwrite. It does not catch somebody who thought to
//!   rewrite it, and pretending otherwise by sealing it under a key stored
//!   beside it would be a decoration, not a protection.
//!
//! # In plain words
//!
//! VeilVoice writes down what its own files look like the first time it runs,
//! and checks them every time after that.
//!
//! If you have set an app lock, that record is locked with the same passphrase,
//! so changing the files *and* the record needs your passphrase too. If you have
//! not, the record is readable, and it will still spot a file that changed by
//! accident but not one changed by somebody covering their tracks.

use std::path::PathBuf;
use std::sync::mpsc;
use veilvoice_guard::Manifest;
use zeroize::Zeroize;

/// What the record has to say, as far as this window knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Nothing has been asked yet.
    Idle,
    /// A worker thread is reading and hashing.
    Working,
    /// A record was taken for the first time.
    Recorded {
        /// Whether it was sealed under the app-lock passphrase.
        sealed: bool,
    },
    /// Everything on disk matches the record.
    Clean {
        /// How many files matched.
        files: usize,
        /// Whether the record that was consulted was a sealed one.
        sealed: bool,
    },
    /// Something differs. The strings are [`veilvoice_guard::Change::describe`].
    Changed(Vec<String>),
    /// The check could not be run, and why.
    Failed(String),
}

/// The integrity record as the window drives it.
pub struct Integrity {
    state: State,
    pending: Option<mpsc::Receiver<State>>,
}

impl Default for Integrity {
    fn default() -> Self {
        Self {
            state: State::Idle,
            pending: None,
        }
    }
}

impl Integrity {
    /// What the last completed check found.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Whether a check is running, so the window keeps repainting while it is.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether the record found a difference worth showing the user.
    pub fn changed(&self) -> bool {
        matches!(self.state, State::Changed(_))
    }

    /// Take or check the record, off the drawing thread.
    ///
    /// `password` is the app-lock passphrase when there is one. It is consumed
    /// by the worker and dropped there rather than being held by this struct,
    /// so a passphrase does not sit in the window's state for the life of the
    /// session.
    ///
    /// Calling this while a check is already running does nothing. A second
    /// walk of the same files would only race the first to the same answer.
    pub fn start(&mut self, password: Option<String>) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.state = State::Working;
        // Detached on purpose. The window must never join a thread that is
        // reading the disk, and there is nothing to clean up if it outlives a
        // close: it holds no handle the process needs back.
        std::thread::spawn(move || {
            let mut password = password;
            let state = run(password.as_deref());
            if let Some(pw) = &mut password {
                pw.zeroize();
            }
            let _ = tx.send(state);
        });
    }

    /// Collect a finished check. Returns true when the state changed, which is
    /// the window's cue to repaint.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        match rx.try_recv() {
            Ok(state) => {
                self.state = state;
                self.pending = None;
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.state = State::Failed("the integrity check stopped unexpectedly".into());
                self.pending = None;
                true
            }
        }
    }
}

/// Where the record is kept, beside the app lock and under the same rules.
///
/// The same path `veilvoice guard` uses, so the window and the command line
/// read one record rather than two.
pub fn record_path() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|p| p.with_file_name("integrity.manifest"))
}

/// The sealed record sits beside the plain one under the container suffix.
fn sealed_path(base: &std::path::Path) -> PathBuf {
    veilvoice_crypto::container::veil_path(base)
}

/// The files worth watching: the running program, and nothing assumed.
///
/// Deliberately short. A manifest over a directory somebody else installs into
/// reports every legitimate update as a change, and a report that cries wolf on
/// every update is one nobody reads. The binary is the file that matters and it
/// is the file this can name without guessing.
fn targets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        out.push(exe);
    }
    out
}

/// The whole of the work, on the worker thread.
fn run(password: Option<&str>) -> State {
    let Some(plain) = record_path() else {
        return State::Failed("no configuration directory on this platform".into());
    };
    let sealed = sealed_path(&plain);
    let files = targets();
    if files.is_empty() {
        return State::Failed("cannot find this program's own file".into());
    }

    // A sealed record is preferred over a plain one wherever both exist. The
    // other order would let anybody who can write the directory downgrade the
    // check by dropping a plain record beside the sealed one.
    let existing = match (password, sealed.exists()) {
        (Some(pw), true) => match std::fs::read(&sealed)
            .ok()
            .and_then(|bytes| Manifest::open_sealed(pw.as_bytes(), &bytes).ok())
        {
            Some(m) => Some((m, true)),
            // A sealed record that will not open is not an absent record.
            // Treating it as absent would overwrite the evidence with a fresh
            // record of whatever is on disk now, which is exactly what somebody
            // who had changed those files would want to happen.
            //
            // The passphrase cannot be the reason: this only runs after an
            // unlock that proved it. So the honest report is that the record
            // will not open, and that a record that will not open is one of the
            // things this is here to notice.
            None => {
                return State::Failed(
                    "the sealed record of VeilVoice's own files will not open. It has been \
                     changed or damaged since it was written."
                        .into(),
                )
            }
        },
        _ => Manifest::load(&plain).ok().map(|m| (m, false)),
    };

    match existing {
        Some((manifest, was_sealed)) => {
            let report = manifest.check::<PathBuf>(&[]);
            if !report.is_clean() {
                return State::Changed(report.changes.iter().map(|c| c.describe()).collect());
            }
            // A plain record found while a passphrase is in hand is upgraded.
            // Somebody who sets an app lock after their first run would
            // otherwise keep the readable record for ever, having done exactly
            // what would earn them the sealed one.
            //
            // Only after the check has come back clean. Sealing a record that
            // no longer matches the files would seal somebody else's version of
            // them and call it authoritative.
            if let (Some(pw), false) = (password, was_sealed) {
                if let Ok(bytes) = manifest.seal(pw.as_bytes()) {
                    if write_private(&sealed, &bytes).is_ok() {
                        let _ = std::fs::remove_file(&plain);
                        return State::Clean {
                            files: report.unchanged,
                            sealed: true,
                        };
                    }
                }
            }
            State::Clean {
                files: report.unchanged,
                sealed: was_sealed,
            }
        }
        None => match Manifest::of(&files) {
            Err(e) => State::Failed(e.to_string()),
            Ok(manifest) => match password {
                Some(pw) => match manifest.seal(pw.as_bytes()) {
                    Err(e) => State::Failed(e.to_string()),
                    Ok(bytes) => match write_private(&sealed, &bytes) {
                        Err(e) => State::Failed(e),
                        Ok(()) => {
                            // A plain record left beside a sealed one is a
                            // downgrade waiting to be used.
                            let _ = std::fs::remove_file(&plain);
                            State::Recorded { sealed: true }
                        }
                    },
                },
                None => match manifest.save(&plain) {
                    Err(e) => State::Failed(e.to_string()),
                    Ok(()) => State::Recorded { sealed: false },
                },
            },
        },
    }
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    veilvoice_crypto::privatefile::write_owner_only(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_record_is_idle_and_not_busy() {
        let guard = Integrity::default();
        assert_eq!(guard.state(), &State::Idle);
        assert!(!guard.is_busy());
        assert!(!guard.changed());
    }

    #[test]
    fn polling_with_nothing_running_reports_no_change() {
        let mut guard = Integrity::default();
        assert!(!guard.poll());
    }

    #[test]
    fn a_disconnected_worker_is_reported_rather_than_waited_for() {
        let mut guard = Integrity::default();
        let (tx, rx) = mpsc::channel();
        guard.pending = Some(rx);
        drop(tx);
        assert!(guard.poll());
        assert!(matches!(guard.state(), State::Failed(_)));
        assert!(!guard.is_busy());
    }

    /// The window must never wait on the disk. This is the same guard the
    /// drawing thread carries, applied to the one module that reads files.
    #[test]
    fn nothing_here_blocks_the_drawing_thread() {
        let source = include_str!("integrity.rs").replace("\r\n", "\n");
        // Everything above the test module. The tests themselves name the very
        // calls they forbid, and the first version of this counted its own
        // assertion as the violation.
        let shipped = source.split("#[cfg(test)]").next().unwrap_or("");
        let body: String = shipped
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let waits = body.matches("recv()").count() - body.matches("try_recv()").count();
        assert_eq!(waits, 0, "a blocking receive reached the window's own code");
        assert!(
            !body.contains(".join()"),
            "the window must not join the worker thread"
        );
    }

    /// A sealed record must win over a plain one wherever both are present, or
    /// dropping a plain file beside the sealed one downgrades the check.
    #[test]
    fn the_sealed_record_is_preferred_over_a_plain_one() {
        let source = include_str!("integrity.rs").replace("\r\n", "\n");
        let start = source.find("fn run(password").expect("run has to exist");
        let body = &source[start..];
        let sealed_at = body.find("Manifest::open_sealed").expect("sealed read");
        let plain_at = body.find("Manifest::load").expect("plain read");
        assert!(
            sealed_at < plain_at,
            "the plain record is consulted before the sealed one"
        );
    }

    #[test]
    fn the_record_sits_beside_the_app_lock() {
        let (Some(record), Some(lock)) = (record_path(), veilvoice_crypto::lock::default_path())
        else {
            return;
        };
        assert_eq!(record.parent(), lock.parent());
        assert!(record.ends_with("integrity.manifest"));
    }

    #[test]
    fn the_sealed_name_is_not_the_plain_one() {
        let plain = PathBuf::from("/somewhere/integrity.manifest");
        assert_ne!(sealed_path(&plain), plain);
    }

    /// The watched set has to name a real file rather than a guess about where
    /// somebody installed this.
    #[test]
    fn the_watched_files_are_ones_that_actually_exist() {
        for path in targets() {
            assert!(
                path.exists(),
                "{} was recorded and is not there",
                path.display()
            );
        }
    }
}
