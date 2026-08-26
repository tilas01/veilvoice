// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows detection, via the Capability Access Manager.
//!
//! # Where the answer lives
//!
//! Windows records every application's use of the microphone and camera under
//! `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\
//! ConsentStore\{microphone,webcam}`. Each application gets a subkey holding
//! `LastUsedTimeStart` and `LastUsedTimeStop` as FILETIME values.
//!
//! The rule that makes this a *live* view rather than a history: an application
//! is using the device right now when its start time is non-zero and its stop
//! time is zero. Windows clears the stop value on acquisition and writes it on
//! release. This is the same bookkeeping that drives the taskbar privacy
//! indicator, so what this reports is exactly what the OS itself believes.
//!
//! Desktop programs live under a `NonPackaged` subkey, with their path encoded
//! using `#` in place of each separator. Store apps appear directly, keyed by
//! package family name.
//!
//! # One subprocess per capability, and why that had to be fixed
//!
//! This originally asked `reg.exe` for the subkeys of the store, then spawned
//! `reg.exe` **twice more for every application it found** to read the two
//! timestamps. The count is `2 + 2n` per capability, where `n` is how many
//! applications have ever asked for that device.
//!
//! Measured on the machine this was found on: 7 packaged and 19 desktop
//! applications for the microphone, 6 packaged for the camera — **68 process
//! creations per scan**. One `reg.exe` spawn there costs 6.6 ms at its
//! fastest, so a scan cost **at least 449 ms**, and that is the warm-cache best
//! case rather than the typical one. The desktop application called `scan` on
//! the user-interface thread every two seconds.
//!
//! The result was a window that froze repeatedly, which is what "runs extremely
//! slow and freezes every couple of seconds" meant in the report. Nothing was
//! leaking and nothing was deadlocked: it was doing a great deal of work in the
//! worst possible place.
//!
//! `reg query <key> /s` prints the **whole subtree**, keys and values together,
//! in one go. So the scan is now two spawns — one per capability — and
//! [`parse_consent_dump`] does the rest in memory. Measured on the same
//! machine: 45 ms for the whole scan, against 449 ms, and it no longer grows
//! with the number of applications installed. The parser is a pure function
//! over text, so it is tested against a captured dump on every platform rather
//! than only on the one that can produce it.
//!
//! The front end was fixed too, and separately: a scan that is fast is still
//! not something to do on the thread that paints.
//!
//! # What it cannot give you
//!
//! A PID. Windows tracks this per *application*, not per process, so
//! [`DeviceUse::pid`] is `None` here. The trade is worth it: this sees packaged
//! apps, background services and anything else the OS accounts for, which
//! enumerating process handles would miss.

use crate::{DeviceKind, DeviceUse, Error};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Spawn without a console window.
///
/// On Windows a `Command` for a console program creates a console, and when the
/// parent is a GUI process with none of its own, Windows opens a **window** for
/// it -- appearing and vanishing as the child runs. That is what "a cmd prompt
/// flashing randomly" was: once at startup, and again on every poll.
///
/// `CREATE_NO_WINDOW` suppresses it. `creation_flags` is a **safe** API, so
/// this costs nothing against `#![forbid(unsafe_code)]`.
///
/// Every `Command::new` in this crate goes through here, and a test asserts it
/// -- because "no console window appeared" is not observable from a test, which
/// is exactly why the defect shipped.
// `mut` is only needed where the body below is compiled in. Everywhere
// else the parameter is moved straight through, and `-D warnings` in CI
// rejects the unused `mut` -- which only the Linux runner can see.
#[cfg_attr(not(windows), allow(unused_mut))]
fn no_window(mut command: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// The full hive name, not the `HKCU` abbreviation: `reg query` echoes subkey
/// paths back in long form, and the reply has to be matched against what was
/// asked for. Querying with the short form silently matched nothing.
const CONSENT_STORE: &str = r"HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

/// FILETIME counts 100-nanosecond intervals from 1601-01-01; Unix time starts
/// at 1970-01-01. This is the gap, in seconds.
const FILETIME_TO_UNIX_SECS: u64 = 11_644_473_600;

/// The absolute path of `reg.exe`, or `None` if it is not where it should be.
///
/// **Never `Command::new("reg")`.** Rust's `Command` resolves a bare program
/// name through the platform search order, and on Windows that order includes
/// the **current working directory** ahead of most of `PATH`. Running
/// `veilvoice watch` from a directory containing a file named `reg.exe` would
/// have executed it, as the user, with no prompt — a downloads folder is
/// enough. Naming the system directory removes the search.
///
/// Returning `None` rather than falling back to a search is deliberate: this
/// module's failure mode is already "report nothing", and the crate says
/// plainly that an empty list from a blind monitor is not good news. Running an
/// unknown `reg.exe` would be a far worse answer than no answer.
fn reg_exe() -> Option<std::path::PathBuf> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    for directory in [
        format!(r"{root}\System32"),
        // A 32-bit process on 64-bit Windows is redirected away from the real
        // System32; `Sysnative` is the way back to it.
        format!(r"{root}\Sysnative"),
    ] {
        let candidate = std::path::Path::new(&directory).join("reg.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn scan() -> Result<Vec<DeviceUse>, Error> {
    let mut found = Vec::new();
    // "webcam" is what the registry calls it, even though the UI says camera.
    for (kind, key) in [
        (DeviceKind::Microphone, "microphone"),
        (DeviceKind::Camera, "webcam"),
    ] {
        collect(kind, &format!(r"{CONSENT_STORE}\{key}"), &mut found);
    }
    Ok(found)
}

/// Walk one capability's whole subtree, from a single `reg query /s`.
fn collect(kind: DeviceKind, root: &str, out: &mut Vec<DeviceUse>) {
    let Some(dump) = query_tree(root) else {
        return;
    };
    for (key, start, stop) in parse_consent_dump(&dump) {
        // In use == started and not yet stopped. Anything else is history.
        if start == 0 || stop != 0 {
            continue;
        }
        let raw = key.rsplit('\\').next().unwrap_or(&key);
        let path = decode_path(raw);
        let app = friendly_name(&path);
        out.push(DeviceUse {
            kind,
            app,
            path: Some(path),
            // Per-application accounting, not per-process. See the module note.
            pid: None,
            since: filetime_to_system(start),
            device: None,
        });
    }
}

/// One `reg query <key> /s`, printing the whole subtree.
///
/// The single subprocess this module's scan costs per capability. Everything
/// after it is text.
fn query_tree(key: &str) -> Option<String> {
    let reg = reg_exe()?;
    let output = no_window(Command::new(reg))
        .args(["query", key, "/s"])
        .output()
        .ok()?;
    if !output.status.success() {
        // A store that does not exist is the ordinary state on a machine where
        // nothing has ever asked for a microphone, and is not an error.
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Pull `(key, LastUsedTimeStart, LastUsedTimeStop)` out of a `/s` dump.
///
/// The shape `reg query /s` prints is a key path on its own unindented line,
/// then that key's values indented beneath it, then a blank line:
///
/// ```text
/// HKEY_CURRENT_USER\...\microphone\SomeApp
///     LastUsedTimeStart    REG_QWORD    0x1db5f3a1c2d4e5f
///     LastUsedTimeStop    REG_QWORD    0x0
/// ```
///
/// Only keys that carry a `LastUsedTimeStart` come back, which is exactly the
/// set of application entries — packaged ones sit directly under the store and
/// desktop ones under `NonPackaged`, and this needs to know nothing about that
/// distinction to find both.
///
/// A key with a start time and no stop time is reported with a stop of zero:
/// Windows clears the stop value on acquisition, so *absent* and *zero* mean
/// the same thing here, and treating a missing line as "still running" is the
/// reading that errs toward telling the user something is listening.
pub(crate) fn parse_consent_dump(text: &str) -> Vec<(String, u64, u64)> {
    let mut found: Vec<(String, u64, u64)> = Vec::new();
    let mut key: Option<String> = None;
    let mut start: Option<u64> = None;
    let mut stop: Option<u64> = None;

    // Close off whichever key was being read.
    fn flush(
        found: &mut Vec<(String, u64, u64)>,
        key: &mut Option<String>,
        start: &mut Option<u64>,
        stop: &mut Option<u64>,
    ) {
        if let (Some(name), Some(began)) = (key.take(), start.take()) {
            found.push((name, began, stop.take().unwrap_or(0)));
        }
        *start = None;
        *stop = None;
    }

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // A key header is unindented; a value line is indented. `reg` echoes
        // paths under the full hive name, which is why the store is asked for
        // that way -- asking with `HKCU` matched nothing and the monitor
        // reported an empty machine whatever was recording.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            flush(&mut found, &mut key, &mut start, &mut stop);
            if trimmed.starts_with("HKEY_") {
                key = Some(trimmed.trim_end_matches('\\').to_string());
            }
            continue;
        }
        if key.is_none() {
            continue;
        }
        if let Some(value) = hex_value(trimmed, "LastUsedTimeStart") {
            start = Some(value);
        } else if let Some(value) = hex_value(trimmed, "LastUsedTimeStop") {
            stop = Some(value);
        }
    }
    flush(&mut found, &mut key, &mut start, &mut stop);
    found
}

/// `Name    REG_QWORD    0x...` for one named value, as a number.
///
/// Matched by name and then by taking the last field, rather than by splitting
/// into three: the value's *name* is fixed here, so the only thing that can
/// vary is how much whitespace `reg` used, and the last field is the datum
/// whatever it did.
fn hex_value(line: &str, name: &str) -> Option<u64> {
    let rest = line.strip_prefix(name)?;
    // Guard against a value called `LastUsedTimeStartSomethingElse`.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let raw = rest.split_whitespace().next_back()?;
    u64::from_str_radix(raw.trim_start_matches("0x"), 16).ok()
}

/// Registry keys encode a path with `#` where a separator belongs.
fn decode_path(raw: &str) -> String {
    raw.replace('#', "\\")
}

/// The executable name, or the package family name for a Store app.
fn friendly_name(path: &str) -> String {
    let tail = path.rsplit('\\').next().unwrap_or(path);
    let name = tail.strip_suffix(".exe").unwrap_or(tail);
    if name.is_empty() {
        path.to_string()
    } else {
        name.to_string()
    }
}

fn filetime_to_system(filetime: u64) -> Option<SystemTime> {
    let secs = filetime.checked_div(10_000_000)?;
    let unix = secs.checked_sub(FILETIME_TO_UNIX_SECS)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(unix))
}

#[cfg(test)]
mod tests {

    /// Every subprocess in this file must be spawned through `no_window`.
    ///
    /// This reads the file's own source rather than exercising the behaviour,
    /// because "no console window appeared" cannot be observed from a test --
    /// which is precisely why the defect reached a release. A `Command::new`
    /// added later without the wrapper fails here rather than on a desktop.
    #[test]
    fn every_subprocess_is_spawned_without_a_console_window() {
        // A source-reading test, so the line endings have to be settled first.
        // F-72: these searched for "\n}\n" and passed on every machine
        // whose checkout uses LF. GitHub's Windows runners default to
        // core.autocrlf=true, so the file arrives with CRLF, the pattern
        // matches nothing, and three tests failed there and nowhere else --
        // including on the developer machine that had just run them.
        // Normalised here as well as pinned in .gitattributes: a test that
        // depends on a git setting is a test somebody will trip over.
        let source = include_str!("windows.rs").replace("\r\n", "\n");
        let mut bare = Vec::new();
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue; // prose: this rule is discussed in the comments
            }
            if !line.contains("Command::new") || line.contains("no_window(") {
                continue;
            }
            bare.push(format!("line {}: {}", number + 1, trimmed));
        }
        assert!(
            bare.is_empty(),
            "these spawns bypass `no_window`, so each flashes a console window \
             on Windows:\n{}",
            bare.join("\n")
        );
    }
    use super::*;

    #[test]
    fn registry_paths_are_decoded() {
        assert_eq!(
            decode_path("C:#Program Files#Zoom#bin#Zoom.exe"),
            r"C:\Program Files\Zoom\bin\Zoom.exe"
        );
    }

    #[test]
    fn names_are_readable() {
        assert_eq!(friendly_name(r"C:\Program Files\Zoom\bin\Zoom.exe"), "Zoom");
        assert_eq!(
            friendly_name("Microsoft.WindowsCamera_8wekyb3d8bbwe"),
            "Microsoft.WindowsCamera_8wekyb3d8bbwe"
        );
    }

    #[test]
    fn filetime_converts_to_a_sane_instant() {
        // 2020-01-01T00:00:00Z as a FILETIME.
        let ft = (1_577_836_800u64 + FILETIME_TO_UNIX_SECS) * 10_000_000;
        let when = filetime_to_system(ft).expect("should convert");
        let unix = when.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(unix, 1_577_836_800);
    }

    #[test]
    fn a_nonsense_filetime_does_not_panic() {
        assert!(filetime_to_system(0).is_none());
        assert!(filetime_to_system(1).is_none());
        assert!(filetime_to_system(u64::MAX).is_none());
    }

    /// Regression: the registry tool must be an absolute path in the system
    /// directory, never a bare name the OS searches for — the Windows search
    /// order includes the current working directory, so a planted `reg.exe`
    /// would have been run as the user.
    #[test]
    fn the_registry_tool_is_resolved_absolutely_not_searched_for() {
        let path = reg_exe().expect("reg.exe should exist on Windows");
        assert!(path.is_absolute(), "{} is not absolute", path.display());
        assert!(path.is_file());
        let shown = path.to_string_lossy().to_lowercase();
        assert!(
            shown.contains("system32") || shown.contains("sysnative"),
            "resolved outside the system directory: {shown}"
        );
    }

    /// And a decoy named `reg.exe` in the working directory must never become
    /// the tool we run. Written without a temp-directory crate so this crate
    /// stays dependency-free, dev-dependencies included.
    #[test]
    fn a_decoy_in_the_working_directory_is_not_picked_up() {
        let scratch = std::env::temp_dir().join("veilvoice-watch-decoy-test");
        std::fs::create_dir_all(&scratch).unwrap();
        let decoy = scratch.join("reg.exe");
        std::fs::write(&decoy, b"not really reg").unwrap();

        let resolved = reg_exe().expect("reg.exe should exist on Windows");
        assert_ne!(resolved, decoy, "a planted reg.exe was selected");
        assert!(
            !resolved.starts_with(&scratch),
            "resolution reached a writable scratch directory: {}",
            resolved.display()
        );
        let _ = std::fs::remove_file(&decoy);
    }

    /// Real `reg query <store> /s` output, kept verbatim. A parser tested only
    /// against strings written by the person who wrote the parser tests the
    /// assumptions twice and the format never.
    const DUMP: &str = "\
HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone
    Value    REG_SZ    Allow

HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\Microsoft.WindowsCamera_8wekyb3d8bbwe
    Value    REG_SZ    Allow
    LastUsedTimeStart    REG_QWORD    0x1db5f3a1c2d4e5f
    LastUsedTimeStop    REG_QWORD    0x1db5f3a1c2d4f00

HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged

HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged\\C:#Program Files#Zoom#bin#Zoom.exe
    Value    REG_SZ    Allow
    LastUsedTimeStart    REG_QWORD    0x1db6000000000000
    LastUsedTimeStop    REG_QWORD    0x0

HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged\\C:#Windows#System32#WindowsPowerShell#v1.0#powershell.exe
    LastUsedTimeStart    REG_QWORD    0x1db4000000000000
";

    /// The whole subtree comes out of one dump, packaged and desktop alike,
    /// without the parser needing to know which is which.
    #[test]
    fn one_dump_yields_every_application_entry() {
        let found = parse_consent_dump(DUMP);
        assert_eq!(found.len(), 3, "{found:#?}");
        assert!(found.iter().any(|(key, _, _)| key.ends_with("Zoom.exe")));
        assert!(found
            .iter()
            .any(|(key, _, _)| key.ends_with("Microsoft.WindowsCamera_8wekyb3d8bbwe")));
    }

    /// The store key itself and the `NonPackaged` container carry no
    /// timestamps, so they must not become entries.
    #[test]
    fn container_keys_are_not_applications() {
        for (key, _, _) in parse_consent_dump(DUMP) {
            assert!(!key.ends_with("NonPackaged"), "{key}");
            assert!(!key.ends_with("microphone"), "{key}");
        }
    }

    /// The rule the whole feature turns on: started and not yet stopped.
    #[test]
    fn a_started_and_unstopped_entry_is_the_one_in_use() {
        let found = parse_consent_dump(DUMP);
        let zoom = found
            .iter()
            .find(|(key, _, _)| key.ends_with("Zoom.exe"))
            .unwrap();
        assert_eq!(zoom.2, 0, "Zoom is still holding the microphone");
        let camera = found
            .iter()
            .find(|(key, _, _)| key.contains("WindowsCamera"))
            .unwrap();
        assert_ne!(camera.2, 0, "the camera app released it");
    }

    /// Windows clears the stop value on acquisition, so a missing stop line
    /// and a zero mean the same thing -- and the reading that errs toward
    /// telling somebody a microphone is live is the right one.
    #[test]
    fn a_missing_stop_time_reads_as_still_running() {
        let found = parse_consent_dump(DUMP);
        let shell = found
            .iter()
            .find(|(key, _, _)| key.ends_with("powershell.exe"))
            .expect("an entry with no stop line must still be found");
        assert_eq!(shell.2, 0);
    }

    /// A value whose name merely starts with the one being looked for must not
    /// be mistaken for it.
    #[test]
    fn a_similarly_named_value_is_not_confused_for_the_real_one() {
        let dump = "HKEY_CURRENT_USER\\x\\App\n                        LastUsedTimeStartedElsewhere    REG_QWORD    0x99\n";
        assert!(parse_consent_dump(dump).is_empty());
    }

    #[test]
    fn nothing_and_rubbish_produce_nothing_rather_than_panicking() {
        assert!(parse_consent_dump("").is_empty());
        assert!(parse_consent_dump("\n\n   \n").is_empty());
        assert!(parse_consent_dump("not a registry dump at all").is_empty());
        // A value line with no key above it must not be attributed to anything.
        assert!(parse_consent_dump("    LastUsedTimeStart    REG_QWORD    0x1").is_empty());
    }

    /// A malformed number must drop that value rather than the whole entry
    /// silently becoming something else.
    #[test]
    fn an_unparseable_timestamp_is_dropped() {
        let dump = "HKEY_CURRENT_USER\\x\\App\n                        LastUsedTimeStart    REG_QWORD    notanumber\n";
        assert!(parse_consent_dump(dump).is_empty());
    }

    /// Regression, and the important one. `reg query` echoes subkey paths back
    /// under the **full** hive name, so asking with the `HKCU` abbreviation
    /// matched nothing and the monitor reported an empty machine no matter what
    /// was recording — the worst possible failure for this feature, because it
    /// looks like good news.
    ///
    /// Asked against a key every Windows installation has, rather than against
    /// the consent store. The consent store is populated by *applications
    /// having asked for the microphone*, and a headless CI runner with no audio
    /// hardware has legitimately never had one ask — so an empty result there
    /// means "nothing has used the microphone on this machine", which is not the
    /// same claim at all. Conflating the two made CI fail on a machine where the
    /// code was working perfectly, which is its own kind of silently wrong.
    #[test]
    fn the_registry_tool_answers_for_a_key_that_always_exists() {
        // Deliberately shallow: `/s` on HKLM\SOFTWARE\Microsoft would walk an
        // enormous tree, and what is being checked is that the tool runs and
        // echoes paths under the full hive name -- not how big that tree is.
        let dump = query_tree(r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT")
            .expect("reg.exe should answer for a key every Windows install has");
        assert!(
            dump.contains("HKEY_LOCAL_MACHINE\\"),
            "`reg query` parsing is broken, and the monitor would report an empty \
             machine whatever was recording"
        );
    }

    /// The consent store itself, when this machine has one.
    ///
    /// Asserts the *shape* of what comes back, never which applications are in
    /// it. Both of those are facts about the machine rather than about the
    /// code, and both have now failed CI for that reason: first the store was
    /// empty on a runner where nothing had ever asked for a microphone, and
    /// then — after that was allowed for — a runner had entries but no
    /// `NonPackaged` subkey, which only appears once a *desktop* application
    /// has asked. A test that fails depending on what software a machine has
    /// happened to run is not testing this crate.
    ///
    /// What is worth asserting is that every entry is a real subkey path under
    /// the full hive name, because that is what the parser is for. Whether the
    /// parser works at all is covered without any of this ambiguity by
    /// `the_registry_parser_reads_a_key_that_always_exists`.
    #[test]
    fn the_consent_store_is_well_formed_when_this_machine_has_one() {
        let key = format!(r"{CONSENT_STORE}\microphone");
        let Some(dump) = query_tree(&key) else {
            eprintln!(
                "no microphone consent store on this machine - nothing has ever requested \
                 the microphone here, which is a fact about the machine. The parser is \
                 covered by the tests over the captured dump above."
            );
            return;
        };
        for (entry, _, _) in parse_consent_dump(&dump) {
            assert!(
                entry.starts_with(&key),
                "an entry is not under the store it came from: {entry}"
            );
            assert!(
                !entry.contains("REG_"),
                "a value line leaked through: {entry}"
            );
            assert!(!entry.ends_with('\\'), "trailing separator on {entry}");
        }
    }

    /// The whole reason this was rewritten: one spawn per capability, not one
    /// per application. A scan must not take long enough to be noticed.
    ///
    /// Measured as a minimum over several runs, because a single sample on a
    /// machine that happened to be busy says nothing.
    #[test]
    fn a_scan_is_fast_enough_to_run_on_a_timer() {
        let mut best = std::time::Duration::from_secs(3600);
        for _ in 0..5 {
            let started = std::time::Instant::now();
            let _ = scan();
            best = best.min(started.elapsed());
        }
        assert!(
            best < std::time::Duration::from_millis(500),
            "a scan takes {best:?}, which is what made the window freeze"
        );
        eprintln!("fastest scan of this machine: {best:?}");
    }

    /// Reading the real consent store must work on any Windows machine, and
    /// must not report an application that has already released the device.
    #[test]
    fn scanning_the_real_consent_store_is_well_formed() {
        let entries = scan().expect("scan should not fail on Windows");
        for entry in &entries {
            assert!(!entry.app.is_empty());
            assert!(entry.path.is_some());
            assert!(
                matches!(entry.kind, DeviceKind::Microphone | DeviceKind::Camera),
                "unexpected device kind"
            );
        }
    }
}
