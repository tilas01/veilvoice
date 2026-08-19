// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Make a failure that produces no output produce some.
//!
//! # The problem this exists for
//!
//! `veilvoice-gui` is built with `windows_subsystem = "windows"`, so it has no
//! console, and the workspace builds with `panic = "abort"`, so a panic does
//! not unwind into anything that could report it. Put together, **every way
//! this application can fail on Windows produces exactly nothing**: no message,
//! no dialog, no log. The window appears or it does not.
//!
//! That is not hypothetical. A release shipped and the report was "it flashes a
//! command prompt, loads in an unusable state, and crashes" -- which is all a
//! user *can* report, because the program tells them nothing. The console flash
//! turned out to be subprocesses (see `no_window` in [`crate::reduced_motion`]),
//! and the crash could not be diagnosed at all from what was observable.
//!
//! # What this does about it
//!
//! Two failures are caught and written to a file beside the preferences:
//!
//! * **A panic**, through a hook. The hook runs before `abort` even under
//!   `panic = "abort"`, so there is a window in which to write.
//! * **A startup failure from `eframe`**, which is a returned `Err` rather than
//!   a panic and is otherwise printed to a stderr nobody can see.
//!
//! The second is the one worth expecting. This application renders through
//! **glow**, which is OpenGL, and creating a GL context depends on the graphics
//! driver. In a virtual machine, over a remote desktop session, or on a laptop
//! whose hybrid graphics hand the process the wrong adapter, that call fails --
//! and the honest answer to "why did nothing happen?" needs to survive the
//! process exiting.
//!
//! # What it deliberately does not do
//!
//! **It does not report anything anywhere.** The file is written next to the
//! preferences, on the user's own disk, and stays there until they delete it or
//! the application clears it. A privacy tool that phones home about its own
//! crashes would be exactly the thing this project spends its documentation
//! refusing to be, and there is no network code in the dependency graph to do
//! it with even if that changed.
//!
//! **It records no user content.** A panic message, a source location, the
//! version and a timestamp. Not the file being processed, not a path the user
//! chose, not a passphrase -- nothing that is theirs.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The file a failure is written to, beside the preferences.
pub fn default_path() -> Option<PathBuf> {
    crate::prefs::default_path().and_then(|p| p.parent().map(|d| d.join("last-crash.txt")))
}

/// Seconds since the Unix epoch, or 0 if the clock is unreadable.
///
/// No date formatting and no dependency for it. A support conversation needs to
/// know *which* run failed and roughly when, and an epoch second answers that;
/// pulling in a calendar library to render it would be a poor trade in a
/// project whose argument is that its dependency graph can be read.
fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write one failure report. Never panics, whatever happens.
///
/// Called from a panic hook, so it has to be the most defensive code in the
/// crate: a panic in here during a panic is an abort with even less to show for
/// it than before. Every error is swallowed deliberately -- there is nowhere
/// left to report a failure to report a failure.
pub fn write(path: &Path, kind: &str, detail: &str) {
    let mut text = String::new();
    text.push_str("VeilVoice ");
    text.push_str(crate::VERSION);
    text.push_str(" ended unexpectedly.\n\n");
    text.push_str("what:  ");
    text.push_str(kind);
    text.push('\n');
    text.push_str("when:  ");
    text.push_str(&stamp().to_string());
    text.push_str(" (seconds since 1970-01-01 UTC)\n");
    text.push_str("os:    ");
    text.push_str(std::env::consts::OS);
    text.push(' ');
    text.push_str(std::env::consts::ARCH);
    text.push_str("\n\ndetail:\n");
    // Bounded: a panic message can carry a formatted value of any size, and
    // this file is read by a person.
    let detail = if detail.len() > 4000 {
        &detail[..4000]
    } else {
        detail
    };
    text.push_str(detail);
    text.push_str(
        "\n\n\
         This file was written on your machine and sent nowhere. VeilVoice has\n\
         no network code at all. Delete it whenever you like.\n\n\
         If the window never appeared, the most likely cause is that this\n\
         computer could not give the application an OpenGL context -- common\n\
         in a virtual machine, over a remote desktop session, or with hybrid\n\
         graphics. The command-line tool `veilvoice` does the same work and\n\
         needs no graphics at all.\n",
    );

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(text.as_bytes());
        let _ = file.flush();
    }
}

/// Install the panic hook. Call once, as early in `main` as possible.
///
/// Chained rather than replacing: the default hook prints to stderr, which is
/// useful when there *is* a console (a debug build, or the binary run from a
/// terminal), and this adds the file for when there is not.
pub fn install() {
    let Some(path) = default_path() else {
        return; // No config directory: nowhere to write, and not worth failing over.
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let detail = format!(
            "{}\n\nlocation: {}",
            info.payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(no message)".to_string()),
            info.location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "(unknown)".to_string()),
        );
        write(&path, "a panic", &detail);
        previous(info);
    }));
}

/// Record a startup failure that `eframe` returned rather than panicked.
pub fn record_startup_failure(detail: &str) {
    if let Some(path) = default_path() {
        write(&path, "the window could not be created", detail);
    }
}

/// Read a previous report, if one is there, so the interface can mention it.
pub fn previous() -> Option<(PathBuf, String)> {
    let path = default_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    Some((path, text))
}

/// Forget a previous report.
pub fn clear() {
    if let Some(path) = default_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("veilvoice-crashlog-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir.join("last-crash.txt")
    }

    #[test]
    fn a_report_names_the_version_and_the_detail() {
        let path = scratch("basic");
        write(&path, "a panic", "something went wrong at the seams");
        let text = std::fs::read_to_string(&path).expect("written");
        assert!(text.contains(crate::VERSION), "no version: {text}");
        assert!(text.contains("a panic"));
        assert!(text.contains("something went wrong at the seams"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_report_says_it_was_sent_nowhere() {
        // The sentence matters as much as the file. Somebody whose privacy tool
        // just crashed is entitled to know instantly that nothing left the
        // machine, and a test keeps that sentence from being edited away.
        let path = scratch("privacy");
        write(&path, "a panic", "x");
        let text = std::fs::read_to_string(&path).expect("written");
        assert!(
            text.contains("sent nowhere") && text.contains("no network code"),
            "the report must state that nothing was transmitted: {text}"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_report_points_at_the_command_line_tool() {
        // The most likely cause of an invisible window is a machine that cannot
        // give the process an OpenGL context, and the useful thing to tell that
        // user is that there is a front-end which needs no graphics at all.
        let path = scratch("advice");
        write(&path, "the window could not be created", "no GL context");
        let text = std::fs::read_to_string(&path).expect("written");
        assert!(text.contains("OpenGL"), "no cause named: {text}");
        assert!(
            text.contains("`veilvoice`"),
            "no alternative offered: {text}"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_enormous_panic_message_is_bounded() {
        // A panic message can carry a formatted value of any size; this file is
        // read by a person, and by a text editor that has to open it.
        let path = scratch("huge");
        write(&path, "a panic", &"x".repeat(500_000));
        let text = std::fs::read_to_string(&path).expect("written");
        assert!(
            text.len() < 10_000,
            "the report grew to {} bytes",
            text.len()
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn writing_into_a_directory_that_does_not_exist_creates_it() {
        let base =
            std::env::temp_dir().join(format!("veilvoice-crashlog-{}-deep", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("nested").join("last-crash.txt");
        write(&path, "a panic", "x");
        assert!(path.exists(), "the report was not written");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_unwritable_destination_is_survived_rather_than_panicked_on() {
        // This runs inside a panic hook. A panic here is an abort with even
        // less to show for it, so every failure has to be swallowed.
        let path = Path::new("/this/path/cannot/exist/anywhere/last-crash.txt");
        write(path, "a panic", "x"); // must simply return
    }
}
