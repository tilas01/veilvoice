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
//! It checks one last time that the program is safe to close, meaning never
//! VeilVoice itself and never anything belonging to the operating system, and it says what
//! it did either way.

use std::process::Command;

/// A program's own name, without the path it was found at.
fn file_name(app: &str) -> String {
    app.trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(app)
        .to_string()
}

/// Whether this process id still belongs to this program.
///
/// Windows asks `tasklist`, filtered by the id, and looks for the name in what
/// comes back.
///
/// This has to exist even though `taskkill` is also given the name as a filter,
/// because **`taskkill` exits 0 whether or not it killed anything.** Measured:
/// with a filter that matches nothing it prints "INFO: No tasks running with
/// the specified criteria" and returns success, exactly as it does after a real
/// termination. Trusting the exit code meant reporting "closed Discord" while
/// Discord carried on running, which is the worst thing a safety catch can say.
#[cfg(windows)]
fn still_named(app: &str, pid: u32) -> bool {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let mut command = Command::new(format!(r"{root}\System32\tasklist.exe"));
    command.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let Ok(output) = command.output() else {
        // Could not tell. Refusing is the safe direction: not closing something
        // is recoverable and closing the wrong thing is not.
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    text.contains(&file_name(app).to_ascii_lowercase())
}

/// The one-letter run state in a line of `/proc/<pid>/stat`.
///
/// The second field is the program's own name in brackets, and a program is
/// free to have brackets and spaces in its name, so the split is on the **last**
/// `)` rather than on whitespace. Splitting on the first one reads the state of
/// a process called `foo)bar` out of the middle of its own name.
#[cfg(not(windows))]
fn run_state(stat: &str) -> Option<char> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .next()?
        .chars()
        .next()
}

/// Whether this process id still belongs to this program **and is running**.
///
/// See F-74. Asked immediately before the kill, which narrows the window rather
/// than removing it: that is the best a caller outside the kernel can do, and
/// saying so is better than pretending the problem is gone.
///
/// # F-76: a program that has died is still listed until somebody collects it
///
/// On Unix a process that has exited stays in the table, holding its id and its
/// name, until its parent asks for its exit status. Measured on Linux: after
/// `kill -TERM`, `/proc/<pid>/comm` still reads `sleep`, `ps -p <pid> -o comm=`
/// still prints `sleep` and still exits 0, and the only field that has changed
/// is the run state, which is now `Z`.
///
/// So a check that asks only for the name answers "yes, still there" about a
/// program that is already dead. That is F-74's false report in the other
/// direction: [`close`] would wait out its whole retry loop and then tell
/// somebody to go and close a program by hand that had closed nearly three
/// seconds earlier. The state is read first, and a collected-but-not-yet-reaped
/// process counts as gone, because it is.
#[cfg(not(windows))]
fn still_named(app: &str, pid: u32) -> bool {
    let wanted = file_name(app).to_ascii_lowercase();

    // Linux publishes both, so nothing has to be run.
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        if run_state(&stat) == Some('Z') {
            return false;
        }
    }
    if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
        return comm.trim().to_ascii_lowercase() == wanted;
    }

    let number = pid.to_string();
    let ask = |columns: &str| {
        Command::new("/bin/ps")
            .args(["-p", &number, "-o", columns])
            .output()
            .ok()
            .filter(|output| output.status.success())
    };

    // The state and the name in one line. Asked for first because the name
    // alone cannot tell a running program from a dead one, and asked for
    // separately below because `state` is not a column every `ps` has and a
    // platform without it should lose the extra check rather than the feature.
    if let Some(output) = ask("state=,comm=") {
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.trim();
        let (state, name) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        if state.starts_with('Z') {
            return false;
        }
        return name
            .trim()
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            == wanted;
    }
    match ask("comm=") {
        Some(output) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let name = text
                .trim()
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            name == wanted
        }
        // Could not tell. Refusing is the safe direction: not closing something
        // is recoverable and closing the wrong thing is not.
        _ => false,
    }
}

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

    // F-74. Before anything is run, and on every platform: a process id is not
    // a durable handle to a program. Between the scan that found this and this
    // line, the program can exit and the operating system can hand its id to
    // something else, and closing by number alone would terminate whatever
    // inherited it while reporting the one it meant.
    if !still_named(app, pid) {
        return Err(format!(
            "{app} is no longer process {pid}. Nothing was closed: that id now \
             belongs to something else, or to nothing."
        ));
    }

    let (program, args) = if cfg!(windows) {
        // Absolute path, never a bare name: Windows searches the current
        // directory before most of PATH, and this is a security feature
        // deciding what to terminate.
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        (
            format!(r"{root}\System32\taskkill.exe"),
            vec![
                "/PID".to_string(),
                pid.to_string(),
                // F-74. The name as well as the number.
                //
                // Between the scan that found this and this line, the program
                // can exit and the operating system can hand its id to
                // something else. Closing by number alone would then terminate
                // whatever inherited it and report having closed the one it
                // meant. That window is small and it is not theoretical: this
                // feature fires precisely while somebody is plugging things in
                // and programs are starting and stopping.
                //
                // `taskkill` refuses when the filter does not match the id, so
                // the check and the kill are one operation rather than two
                // with a gap. Measured: with the wrong name it reports "no
                // tasks running with the specified criteria" and the process
                // survives; with the right one it terminates it.
                "/FI".to_string(),
                format!("IMAGENAME eq {}", file_name(app)),
                "/F".to_string(),
            ],
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
    if !output.status.success() {
        return Err(format!(
            "could not close {app} (process {pid}): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    // The exit code is not the answer. `taskkill` returns success whether or
    // not it killed anything, and `kill -TERM` returns as soon as the signal is
    // delivered rather than when the program acts on it. So the process is
    // looked for again.
    //
    // A short wait, because ending is not instant and reporting a failure that
    // resolves itself a hundred milliseconds later would send somebody chasing
    // a program that is already gone.
    for attempt in 0..10 {
        if !still_named(app, pid) {
            return Ok(format!("closed {app} (process {pid})"));
        }
        std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
    }
    Err(format!(
        "{app} (process {pid}) was asked to close and is still running. Close it \
         yourself, or stop veiling."
    ))
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

    /// Wait until a just-started child is wearing its own name.
    ///
    /// **F-96.** `Command::spawn` returns before the program it started is
    /// visible under its own name. Measured on Linux, once in about four
    /// thousand spawns: `/bin/sleep` is started and `/proc/<pid>/comm`, read on
    /// the next line, still says the name of the program that started it. The
    /// kernel releases the parent when the child's address space goes, which
    /// happens inside `begin_new_exec` and before the line that sets the new
    /// name, so there is a short window where the id is live and belongs to the
    /// program that is starting while still answering with somebody else's
    /// name.
    ///
    /// [`still_named`] refuses during that window, which is the safe direction
    /// and is what this program wants: a doubtful answer closes nothing. The
    /// tests below are the ones that have to care, because they start a child
    /// and act on it in the next few microseconds, which no scan ever does.
    /// Without this wait they fail rarely and misleadingly: one CI run reported
    /// "sleep is no longer process 7030" about a `sleep` that was still running
    /// when the job cleaned up after it.
    ///
    /// Waiting on the function under test is deliberate. The precondition these
    /// tests need is exactly "this id now belongs to this program", which is
    /// what it answers, and a child that never arrives fails here with that
    /// said plainly rather than inside an assertion about something else.
    fn wearing_its_own_name(name: &str, pid: u32) -> bool {
        for _ in 0..200 {
            if still_named(name, pid) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }

    /// A name that does not match the process is refused, which is F-74's
    /// whole purpose: the id alone is not enough to know what will be closed.
    #[test]
    fn a_process_whose_name_does_not_match_is_left_alone() {
        let mut child = if cfg!(windows) {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
            Command::new(format!(r"{root}\System32\ping.exe"))
                .args(["-n", "20", "127.0.0.1"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .spawn()
        } else {
            Command::new("/bin/sleep").arg("20").spawn()
        };
        let Ok(child) = child.as_mut() else {
            return;
        };
        let pid = child.id();
        let name = if cfg!(windows) { "ping.exe" } else { "sleep" };
        assert!(
            wearing_its_own_name(name, pid),
            "the child never appeared as {name}, so there is nothing to test"
        );

        // Somebody else's program, wearing this one's number.
        let refused = close("not-what-is-running.exe", Some(pid));
        assert!(refused.is_err(), "{refused:?}");

        // And it really is still running.
        close(name, Some(pid)).expect("the right name still closes it");
        let _ = child.wait();
    }

    /// **F-74.** A process id is not a durable handle to a program.
    ///
    /// Between the scan that found something and the line that closes it, the
    /// program can exit and the operating system can hand its id to a new
    /// process. Closing by number alone would then terminate whatever inherited
    /// it, and report having closed the one it meant.
    ///
    /// On Windows the name travels with the kill as a filter, so the check and
    /// the act are one operation. Elsewhere the name is checked immediately
    /// before, which narrows the window rather than removing it.
    #[test]
    fn a_process_is_never_closed_by_number_alone() {
        let source = include_str!("act.rs").replace(
            "
", "
",
        );
        let body = source.split("#[cfg(test)]").next().unwrap();
        assert!(
            body.contains("IMAGENAME eq"),
            "the Windows kill must carry the name as well as the id"
        );
        assert!(
            body.contains("still_named"),
            "and elsewhere the name must be checked first"
        );
        // And the fallback when it cannot tell is to refuse.
        let start = body.find("fn still_named").unwrap_or(0);
        if start > 0 {
            let tail = &body[start..];
            assert!(
                tail.contains("_ => false"),
                "not being able to tell must mean not closing it"
            );
        }
    }

    /// **F-76.** A program that has died is reported as gone, not as running.
    ///
    /// On Unix a process that has exited keeps its id and its name in the
    /// table until its parent collects its exit status. This test is that
    /// parent, and it deliberately does not collect: after the signal the
    /// child is dead, `/proc/<pid>/comm` still reads `sleep`, and `ps` still
    /// prints `sleep` and still exits 0.
    ///
    /// Before the fix, [`close`] read that as "still running", waited out its
    /// whole retry loop, and then told the reader to go and close by hand a
    /// program that had been dead for nearly three seconds. Measured: the two
    /// tests below this one failed on Linux for exactly that reason while
    /// passing on Windows, where a terminated process leaves the table at once.
    #[cfg(unix)]
    #[test]
    fn a_program_that_has_died_but_not_been_collected_counts_as_gone() {
        let Ok(mut child) = Command::new("/bin/sleep").arg("30").spawn() else {
            // No such tool on this machine. Not a failure of the code under
            // test, and better skipped than asserted around.
            return;
        };
        let pid = child.id();
        assert!(
            wearing_its_own_name("sleep", pid),
            "the child never appeared as sleep, so there is nothing to test"
        );
        let told = close("sleep", Some(pid));
        assert!(
            told.is_ok(),
            "a dead child must not be reported as running: {told:?}"
        );

        // Nothing has collected it yet, so the operating system is still
        // holding the entry this used to be fooled by. Prove that rather than
        // assume it: without it this test would pass for the wrong reason on
        // any platform that reaps early.
        let listed = std::fs::read_to_string(format!("/proc/{pid}/comm"));
        if let Ok(name) = listed {
            assert_eq!(
                name.trim(),
                "sleep",
                "the entry should still be there, which is what makes this a test"
            );
        }
        let _ = child.wait();
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
        // The real name, because F-74 made the name part of the kill. Passing a
        // made-up one here used to work and now correctly does not, which is
        // the whole point: the first run of this after that change sat for the
        // full thirty seconds while taskkill refused and the process outlived
        // it.
        let name = if cfg!(windows) { "ping.exe" } else { "sleep" };
        assert!(
            wearing_its_own_name(name, pid),
            "the child never appeared as {name}, so there is nothing to test"
        );
        let told = close(name, Some(pid));
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
