// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Which processes are running, per platform, and what that cannot tell you.
//!
//! # Why this is a crate rather than a module
//!
//! Two of this workspace's security features need the same answer: which
//! programs are running. `veilvoice-capture` asks it about screen recorders,
//! `veilvoice-input` asks it about keyboard and mouse monitors, and the answer
//! is one platform-specific listing with one set of limits.
//!
//! It began inside `veilvoice-capture` as a private module. Leaving it there
//! and depending on that crate would mean a keyboard-monitoring feature pulling
//! in a table of screen recorders it will never look at, which is exactly what
//! the design note in `ROADMAP.md` says these crates must not do: *each is a
//! crate of its own, so that another project can depend on one without taking
//! all of them*. The alternative -- a second copy of the parser -- is worse:
//! this project already extracted `veilvoice-check` out of the verifier so the
//! desktop application and the command line could not drift apart, and the
//! reasoning is the same here.
//!
//! # Linux reads files; the other two ask a tool
//!
//! On Linux every process publishes its own name at `/proc/<pid>/comm`, so the
//! list is a directory walk and nothing is spawned. Windows and macOS have no
//! such file, and their native APIs are FFI -- `#![forbid(unsafe_code)]` holds
//! here as it does everywhere else in the workspace, so this asks a tool the
//! system already ships, exactly as `veilvoice-watch` asks the registry and
//! `veilvoice-drivers` asks `driverquery`.
//!
//! # What this can see, and what it cannot
//!
//! Processes belonging to the user running VeilVoice, and -- depending on the
//! platform and the privileges -- usually not much more. A program running as
//! another user or as a service may not appear at all.
//!
//! It sees a program that is **running**. It does not see what that program is
//! doing. Every caller has to phrase its findings accordingly, and [`SCOPE`]
//! is the wording to show rather than an invitation to invent one.
//!
//! `comm` on Linux is truncated to fifteen characters by the kernel, so any
//! table matched against this must carry a name of fifteen characters or fewer
//! for every program it expects to find there. A sixteen-character executable
//! would otherwise stop matching on one platform only, silently -- and the
//! tables that do this keep their own tests for it, next to the table, because
//! that is where somebody adds a row.
//!
//! # In plain words
//!
//! This asks your computer which programs are open right now, in the way each
//! operating system prefers to be asked. It is used by the parts of VeilVoice
//! that warn you when something able to record your screen, or watch your
//! typing, is running.
//!
//! Two honest limits. It can only see programs running as you -- something
//! hidden well enough, or running as the system, will not appear. And it only
//! knows a program is **open**, never that it is actually recording or
//! watching. Anything built on top of this has to say so in those words.

/// Every process name this build can see, lower-cased and without a path.
///
/// The second value is anything that went wrong. A list that came back short
/// because a tool failed is not a short list, and reporting the difference is
/// the whole reason this returns two things.
pub fn running() -> (Vec<String>, Vec<String>) {
    #[cfg(target_os = "linux")]
    {
        linux()
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        spawned()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (
            Vec::new(),
            vec!["no process reader is written for this platform".to_string()],
        )
    }
}

/// Walk `/proc` and read each process's own name.
#[cfg(target_os = "linux")]
fn linux() -> (Vec<String>, Vec<String>) {
    let mut names = Vec::new();
    let entries = match std::fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(error) => return (names, vec![format!("/proc: {error}")]),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Only the numbered directories are processes. The rest of /proc is
        // the kernel's own bookkeeping.
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        // A process that exits between the walk and the read is ordinary, not
        // an error worth reporting: by the time it could be mentioned it is
        // already not running.
        if let Ok(name) = std::fs::read_to_string(path.join("comm")) {
            let name = name.trim().to_ascii_lowercase();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    (names, Vec::new())
}

/// Ask the system's own process lister.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn spawned() -> (Vec<String>, Vec<String>) {
    #[cfg(target_os = "windows")]
    let (program, arguments): (String, &[&str]) = {
        // Absolute path, never a bare name: Windows searches the current
        // directory before most of PATH, so a `tasklist.exe` sitting in the
        // folder VeilVoice was unpacked into would answer this question
        // instead. This is a security tool asking what is running.
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        (
            format!(r"{root}\System32\tasklist.exe"),
            &["/FO", "CSV", "/NH"],
        )
    };
    #[cfg(target_os = "macos")]
    let (program, arguments): (String, &[&str]) = ("/bin/ps".to_string(), &["-Ao", "comm="]);

    let mut command = std::process::Command::new(&program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // Safe API, so it costs nothing against `#![forbid(unsafe_code)]`, and
        // it is what stops a console flashing every time the monitor polls --
        // the defect v0.1.10 shipped.
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = match command.args(arguments).output() {
        Ok(output) => output,
        Err(error) => return (Vec::new(), vec![format!("{program}: {error}")]),
    };
    if !output.status.success() {
        return (
            Vec::new(),
            vec![format!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )],
        );
    }
    // Lossy on purpose: a process whose name has a byte this cannot decode
    // should still be compared rather than dropped.
    let text = String::from_utf8_lossy(&output.stdout);
    (parse(&text), Vec::new())
}

/// Pull process names out of a listing.
///
/// Handles both shapes with one function: `tasklist /FO CSV` quotes its first
/// field, and `ps -Ao comm=` gives a bare path per line. Taking the first
/// comma-separated field, stripping quotes, and then stripping any directory
/// covers both, and one parser cannot drift from the other.
#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn parse(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let first = line.split(',').next().unwrap_or(line);
        let bare = first.trim().trim_matches('"');
        let bare = bare.rsplit(['/', '\\']).next().unwrap_or(bare).trim();
        if bare.is_empty() {
            continue;
        }
        names.push(bare.to_ascii_lowercase());
    }
    names
}

/// What a reader has to be told, in the words to show them.
///
/// Here rather than in each caller so that two features cannot describe the
/// same limit two different ways.
pub const SCOPE: &str = "\
This sees programs running as you. Something running as another user, as a \
system service, or hidden well enough will not appear, so an empty list is not \
proof that nothing is there. It also sees only that a program is *open* -- \
never that it is recording, watching or doing anything at all.";

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `tasklist /FO CSV /NH` output, kept verbatim.
    const TASKLIST: &str = "\
\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"
\"explorer.exe\",\"5512\",\"Console\",\"1\",\"142,336 K\"
\"obs64.exe\",\"18244\",\"Console\",\"1\",\"221,904 K\"
\"veilvoice-gui.exe\",\"9012\",\"Console\",\"1\",\"78,220 K\"
";

    /// Real `ps -Ao comm=` output, kept verbatim.
    const PS: &str = "\
/sbin/launchd
/usr/libexec/logd
/Applications/OBS.app/Contents/MacOS/OBS
/System/Library/CoreServices/Finder.app/Contents/MacOS/Finder
";

    #[test]
    fn a_windows_listing_gives_bare_lower_case_names() {
        let names = parse(TASKLIST);
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"obs64.exe".to_string()));
        assert!(names.contains(&"explorer.exe".to_string()));
        assert!(
            names.contains(&"system idle process".to_string()),
            "a name with spaces must survive: {names:?}"
        );
    }

    #[test]
    fn a_unix_listing_has_its_directories_stripped() {
        let names = parse(PS);
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"obs".to_string()), "{names:?}");
        assert!(names.contains(&"launchd".to_string()));
        assert!(
            !names.iter().any(|name| name.contains('/')),
            "a path survived: {names:?}"
        );
    }

    #[test]
    fn nothing_and_whitespace_produce_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("\n\n   \n").is_empty());
        assert!(parse("\"\"\n").is_empty());
        assert!(parse("/\n").is_empty());
    }

    /// The limit has to be stated outright, not hinted at. A caller that shows
    /// an empty list without this is telling somebody their machine is clean.
    #[test]
    fn the_scope_note_says_what_an_empty_list_does_not_prove() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("not proof that nothing is there"), "{scope}");
        assert!(scope.contains("never that it is recording"), "{scope}");
    }

    /// Asking the real machine must not panic, and must say something when it
    /// cannot answer rather than returning a quiet empty list.
    #[test]
    fn listing_this_machine_does_not_panic() {
        let (names, problems) = running();
        if names.is_empty() {
            assert!(
                !problems.is_empty(),
                "an empty list with no reason given is indistinguishable from an \
                 empty machine"
            );
        }
        for name in &names {
            assert_eq!(name, &name.to_ascii_lowercase());
        }
    }
}
