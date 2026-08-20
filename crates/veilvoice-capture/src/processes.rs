// SPDX-License-Identifier: GPL-3.0-or-later
//! Listing the processes that are running, per platform.
//!
//! # Linux reads files; the other two ask a tool
//!
//! On Linux every process publishes its own name at `/proc/<pid>/comm`, so the
//! list is a directory walk and nothing is spawned. Windows and macOS have no
//! such file, and their native APIs are FFI — `#![forbid(unsafe_code)]` holds
//! here as it does everywhere else in the workspace, so this asks a tool the
//! system already ships, exactly as `veilvoice-watch` asks the registry and
//! `veilvoice-drivers` asks `driverquery`.
//!
//! # What this can see
//!
//! Processes belonging to the user running VeilVoice, and — depending on the
//! platform and the privileges — usually not much more. A recorder running as
//! another user or as a service may not appear at all. That is a real limit and
//! [`crate::SCOPE`] states it rather than leaving an empty list to be read as
//! an empty machine.
//!
//! `comm` on Linux is truncated to fifteen characters by the kernel, so a
//! longer name in [`crate::programs`] never appears there. Two entries in that
//! table are longer than fifteen characters and carry **both** spellings for
//! exactly that reason — `ps` and every Windows listing give the full one. A
//! test holds the rule that every program with a Unix name has at least one
//! that survives truncation, because a sixteen-character executable added later
//! would otherwise stop matching on one platform only, silently.

/// Every process name this build can see, lower-cased and without a path.
///
/// The second value is anything that went wrong. A list that came back short
/// because a tool failed is not a short list, and reporting the difference is
/// the whole reason this returns two things.
pub(crate) fn running() -> (Vec<String>, Vec<String>) {
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
/// covers both — and one parser cannot drift from the other.
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

    /// The Linux kernel truncates `/proc/<pid>/comm` to fifteen characters, so
    /// a longer name in the table never matches there -- silently, and on one
    /// platform only, which is the exact shape of bug this project keeps
    /// finding in itself.
    ///
    /// The rule is therefore not "every name is short" but "every program that
    /// has a Unix name has **one** that survives truncation". Both spellings
    /// are listed where they differ, because the full one is still what `ps`
    /// and every Windows listing give.
    #[test]
    fn every_program_has_a_name_that_survives_the_linux_truncation() {
        for program in crate::programs::ALL {
            // A `.exe` name is a Windows name and never appears in `comm`.
            let unix: Vec<&&str> = program
                .processes
                .iter()
                .filter(|process| !process.ends_with(".exe"))
                .collect();
            if unix.is_empty() {
                continue; // a Windows-only program, and there is nothing to check
            }
            assert!(
                unix.iter().any(|process| process.len() <= 15),
                "{} has no name of fifteen characters or fewer, so it can never match \
                 on Linux: {unix:?}",
                program.key
            );
        }
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
