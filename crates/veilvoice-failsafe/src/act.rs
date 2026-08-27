// SPDX-License-Identifier: GPL-3.0-or-later
//! Actually closing a program, kept apart from deciding to.
//!
//! # Why this is its own file
//!
//! Everything in [`crate`] is arithmetic over a list: it decides, and it can be
//! tested without a machine. This is the only part that reaches out and ends
//! somebody's program, and separating the two means the decision can be
//! reasoned about on its own and this can be short enough to read in one go.
//!
//! # The check is made twice, on purpose
//!
//! [`crate::Guard::look`] already worked out whether a program may be closed,
//! and [`close`] checks again before it acts. That is not redundancy for its own
//! sake: the two are reached through different paths, a future caller could
//! compute `closeable` wrongly or not at all, and the cost of being wrong here
//! is ending somebody's desktop session. A guard that refuses at the last step
//! as well as the first is a guard that cannot be talked past by a mistake
//! upstream.
//!
//! # In plain words
//!
//! This is the part that closes a program that picked up your real microphone.
//! It checks one last time that the program is safe to close — never VeilVoice
//! itself, never anything belonging to the operating system — and it says what
//! it did either way.

use std::process::Command;

/// Close a program, having decided it should be closed.
///
/// Refuses anything on [`crate::PROTECTED`] whatever the caller believed, and
/// refuses when there is no process to act on: closing "whatever is called
/// Discord" is a different and much worse operation than closing process 4812.
pub fn close(app: &str, pid: Option<u32>) -> Result<String, String> {
    if crate::is_protected(app) {
        return Err(format!(
            "{app} is a system process or VeilVoice itself, and is never closed. \
             Change what it is using, or stop veiling."
        ));
    }
    let Some(pid) = pid else {
        // Windows reports microphone use per *application* rather than per
        // process, so this happens. Closing by name would mean ending every
        // process that shares it, which is not what was decided.
        return Err(format!(
            "{app} holds a microphone but this platform did not say which process, \
             so there is nothing specific to close. Close it yourself, or stop veiling."
        ));
    };

    let (program, args) = if cfg!(windows) {
        // Absolute path, never a bare name: Windows searches the current
        // directory before most of PATH, and this is a security feature
        // deciding what to terminate.
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        (
            format!(r"{root}\System32\taskkill.exe"),
            vec!["/PID".to_string(), pid.to_string(), "/F".to_string()],
        )
    } else {
        // TERM, not KILL. A program given the chance to close its audio device
        // and exit leaves less behind than one shot in the head, and if it
        // ignores that, the next scan finds it again a second later.
        (
            "/bin/kill".to_string(),
            vec!["-TERM".to_string(), pid.to_string()],
        )
    };

    let mut command = Command::new(&program);
    command.args(&args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if output.status.success() {
        Ok(format!("closed {app} (process {pid})"))
    } else {
        Err(format!(
            "could not close {app} (process {pid}): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The last line of defence, and it holds whatever the caller believed.
    #[test]
    fn a_protected_program_is_refused_here_as_well() {
        for system in ["explorer.exe", "lsass.exe", "veilvoice-gui.exe", "systemd"] {
            let refused = close(system, Some(4)).unwrap_err();
            assert!(refused.contains("never closed"), "{system}: {refused}");
            assert!(
                refused.contains("stop veiling"),
                "and what to do: {refused}"
            );
        }
    }

    /// No process, no action. Closing "whatever is called Discord" is a
    /// different and much worse operation than closing one process.
    #[test]
    fn without_a_process_id_nothing_is_closed() {
        let refused = close("Discord.exe", None).unwrap_err();
        assert!(refused.contains("did not say which process"), "{refused}");
        assert!(refused.contains("Close it yourself"), "{refused}");
    }

    /// Nothing here closes anything by name, which would take out every
    /// process sharing it.
    #[test]
    fn nothing_is_ever_closed_by_name() {
        let source = include_str!("act.rs").replace("\r\n", "\n");
        let body = source.split("#[cfg(test)]").next().unwrap();
        for by_name in ["/IM", "pkill", "killall"] {
            assert!(
                !body.contains(by_name),
                "{by_name} ends every process sharing a name"
            );
        }
        assert!(body.contains("/PID"), "Windows closes one process");
        assert!(body.contains("-TERM"), "and elsewhere it asks first");
    }

    /// A real process, closed for real. Started here so nothing else is at
    /// risk, and it is a program that does nothing but wait.
    #[test]
    fn a_process_this_test_started_is_actually_closed() {
        // `ping` rather than `timeout`: timeout.exe refuses to run without a
        // console ("input redirection is not supported") and exits at once, so
        // the first version of this test closed a process that had already
        // gone and reported that as a failure of the code under test.
        let mut child = if cfg!(windows) {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
            Command::new(format!("{root}\\System32\\ping.exe"))
                .args(["-n", "30", "127.0.0.1"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .spawn()
        } else {
            Command::new("/bin/sleep").arg("30").spawn()
        };
        let Ok(child) = child.as_mut() else {
            // No such tool on this machine. Not a failure of the code under
            // test, and better skipped than asserted around.
            return;
        };
        let pid = child.id();
        let told = close("a-test-process", Some(pid));
        assert!(told.is_ok(), "{told:?}");
        assert!(told.unwrap().contains(&pid.to_string()));

        // And it really is gone, rather than merely reported as gone. `wait`
        // returns once it has been reaped, which will not happen if the close
        // silently did nothing -- the test would hang rather than pass.
        let status = child.wait().expect("reap");
        assert!(
            !status.success(),
            "a terminated process should not report success: {status:?}"
        );
    }
}
