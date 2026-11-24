// SPDX-License-Identifier: GPL-3.0-or-later
//! A reader who stops reading is not a crash.
//!
//! `println!` panics when standard output is a pipe whose reader has gone,
//! which is exactly what `veilvoice policy status | head` does after ten
//! lines. The program printed a panic message, and a hint about backtraces,
//! under a command that had done what it was asked. A C program would be
//! ended quietly by `SIGPIPE`, but Rust ignores that signal, and putting it
//! back needs `unsafe`, which this crate forbids. So the panic is recognised
//! here and answered with a quiet exit, and every other panic is reported as
//! it always was.

/// The status a shell reports for a program `SIGPIPE` ended: 128 plus the
/// signal's number. Exiting with it rather than 0 keeps a script that uses
/// `set -o pipefail` able to see that the output was cut short.
const CLOSED_PIPE_STATUS: i32 = 141;

/// Install the hook. Call it once, first thing in `main`.
pub fn quiet_on_closed_pipe() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload_as_str().is_some_and(is_closed_pipe) {
            std::process::exit(CLOSED_PIPE_STATUS);
        }
        default(info);
    }));
}

/// Whether a panic message is the standard library reporting that a write to
/// standard output failed because nothing reads it any more: `EPIPE` on every
/// Unix VeilVoice builds for, and `ERROR_NO_DATA` on Windows.
fn is_closed_pipe(message: &str) -> bool {
    message.starts_with("failed printing to stdout")
        && (message.contains("(os error 32)") || message.contains("(os error 232)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **F-345.** The two messages a closed pipe produces are recognised.
    #[test]
    fn a_closed_pipe_is_recognised_on_unix_and_on_windows() {
        assert!(is_closed_pipe(
            "failed printing to stdout: Broken pipe (os error 32)"
        ));
        assert!(is_closed_pipe(
            "failed printing to stdout: The pipe is being closed. (os error 232)"
        ));
    }

    /// Any other panic still reaches the default hook, including a failed
    /// write for another reason and a closed pipe on standard error.
    #[test]
    fn every_other_panic_is_still_reported() {
        assert!(!is_closed_pipe(
            "failed printing to stdout: No space left on device (os error 28)"
        ));
        assert!(!is_closed_pipe(
            "failed printing to stderr: Broken pipe (os error 32)"
        ));
        assert!(!is_closed_pipe("index out of bounds (os error 32)"));
    }
}
