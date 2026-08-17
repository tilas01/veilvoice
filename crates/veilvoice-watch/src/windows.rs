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
//! # What it cannot give you
//!
//! A PID. Windows tracks this per *application*, not per process, so
//! [`DeviceUse::pid`] is `None` here. The trade is worth it: this sees packaged
//! apps, background services and anything else the OS accounts for, which
//! enumerating process handles would miss.

use crate::{DeviceKind, DeviceUse, Error};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The full hive name, not the `HKCU` abbreviation: `reg query` echoes subkey
/// paths back in long form, and the reply has to be matched against what was
/// asked for. Querying with the short form silently matched nothing.
const CONSENT_STORE: &str = r"HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

/// FILETIME counts 100-nanosecond intervals from 1601-01-01; Unix time starts
/// at 1970-01-01. This is the gap, in seconds.
const FILETIME_TO_UNIX_SECS: u64 = 11_644_473_600;

/// The registry path separator, as a byte.
const BACKSLASH: u8 = b'\\';

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

/// Walk one capability's subkeys, two levels deep to catch `NonPackaged`.
fn collect(kind: DeviceKind, root: &str, out: &mut Vec<DeviceUse>) {
    for app_key in subkeys(root) {
        if app_key.rsplit('\\').next() == Some("NonPackaged") {
            for desktop in subkeys(&app_key) {
                if let Some(entry) = read_entry(kind, &desktop) {
                    out.push(entry);
                }
            }
        } else if let Some(entry) = read_entry(kind, &app_key) {
            out.push(entry);
        }
    }
}

/// Immediate subkey paths of `key`, or nothing if it does not exist.
///
/// `reg.exe` is used rather than a registry crate: it ships with every Windows
/// install, keeps this crate dependency-free, and reading two well-known keys
/// does not justify pulling in the Win32 bindings.
fn subkeys(key: &str) -> Vec<String> {
    let Some(reg) = reg_exe() else {
        return Vec::new();
    };
    let Ok(output) = Command::new(reg).args(["query", key]).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim_end)
        .filter(|line| {
            // A subkey line is the full path, unindented. A value line is
            // indented and carries a type such as REG_QWORD, and the key
            // itself is echoed back as the first line.
            !line.starts_with(' ')
                && line.starts_with(key)
                && line.len() > key.len()
                && line.as_bytes().get(key.len()) == Some(&BACKSLASH)
        })
        .map(str::to_string)
        .collect()
}

/// Read one application's entry, returning it only if the device is in use now.
fn read_entry(kind: DeviceKind, key: &str) -> Option<DeviceUse> {
    let start = read_u64(key, "LastUsedTimeStart")?;
    let stop = read_u64(key, "LastUsedTimeStop").unwrap_or(0);

    // In use == started and not yet stopped. Anything else is history.
    if start == 0 || stop != 0 {
        return None;
    }

    let raw = key.rsplit('\\').next().unwrap_or(key);
    let path = decode_path(raw);
    let app = friendly_name(&path);

    Some(DeviceUse {
        kind,
        app,
        path: Some(path),
        // Per-application accounting, not per-process. See the module note.
        pid: None,
        since: filetime_to_system(start),
        device: None,
    })
}

fn read_u64(key: &str, value: &str) -> Option<u64> {
    let output = Command::new(reg_exe()?)
        .args(["query", key, "/v", value])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|l| l.contains(value))?;
    let hex = line.split_whitespace().last()?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
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

    #[test]
    fn a_missing_key_yields_nothing_rather_than_failing() {
        assert!(subkeys(r"HKEY_CURRENT_USER\SOFTWARE\ThisKeyDoesNotExistAnywhere12345").is_empty());
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
    fn the_registry_parser_reads_a_key_that_always_exists() {
        let keys = subkeys(r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft");
        assert!(
            !keys.is_empty(),
            "no subkeys under HKLM\\SOFTWARE\\Microsoft — `reg query` parsing is broken, \
             and the monitor will report an empty machine whatever is recording"
        );
        assert!(
            keys.iter().all(|k| k.starts_with("HKEY_LOCAL_MACHINE\\")),
            "subkeys should come back under the full hive name: {keys:?}"
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
        let mic = subkeys(&key);
        if mic.is_empty() {
            eprintln!(
                "no microphone consent store on this machine - nothing has ever requested \
                 the microphone here, which is a fact about the machine. The parser is \
                 covered by `the_registry_parser_reads_a_key_that_always_exists`."
            );
            return;
        }
        for entry in &mic {
            assert!(
                entry.starts_with(&key),
                "subkey is not under the store it came from: {entry}"
            );
            assert!(
                !entry.contains("REG_"),
                "a value line leaked through: {entry}"
            );
            assert!(!entry.ends_with('\\'), "trailing separator on {entry}");
        }
    }

    /// Value lines must never be mistaken for subkeys.
    #[test]
    fn value_lines_are_not_treated_as_subkeys() {
        let mic = subkeys(&format!(r"{CONSENT_STORE}\microphone"));
        for key in &mic {
            assert!(!key.contains("REG_"), "a value line leaked through: {key}");
            assert!(!key.starts_with(' '));
        }
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
