// SPDX-License-Identifier: GPL-3.0-or-later
//! Whether the operating system has been asked to reduce motion.
//!
//! # Why this exists rather than an egui call
//!
//! egui does not surface the platform's accessibility preference, so it has to
//! be read here. The website gets this for free -- CSS has
//! `prefers-reduced-motion` and the browser answers it -- and it would be odd
//! for the desktop app to be the one front-end that ignores the setting.
//!
//! # Read once, at startup
//!
//! Every platform answers this through a subprocess, and a subprocess per
//! frame would be indefensible in a paint loop. It is read once when the app
//! starts and cached for the session. Someone who changes the setting while
//! VeilVoice is open sees it on the next launch, which is the same behaviour
//! most applications have.
//!
//! # Absolute paths, always
//!
//! `Command::new("defaults")` is a *search*, and on Windows that search
//! includes the current working directory -- which is precisely the defect
//! (F-13) this project fixed in `veilvoice-watch` and `veilvoice-guard`. Every
//! tool here is named by absolute path, and an unfound tool answers "I do not
//! know" rather than falling back to a search.
//!
//! # When it cannot tell
//!
//! [`Query::Unknown`] means the platform was not asked or did not answer, and
//! the caller treats that as "no reduction requested" -- because defaulting to
//! *off* would silently disable animation for everybody on a platform this
//! cannot read, which is a worse failure than missing the preference for the
//! few who set it. The settings panel only claims the system asked for reduced
//! motion when it actually saw it say so.
//!
//! # In plain words
//!
//! Asks the operating system whether you have said you would rather things did not
//! animate.
//!
//! Some people get motion sickness from moving interfaces, and every system has a
//! setting for it. Honouring it is not decoration: an application that animates
//! regardless is one those people cannot comfortably use.
//!
//! When the answer cannot be determined, animation stays on, and the setting can
//! be overridden by hand either way.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// What the platform said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    /// The system asked for reduced motion.
    Reduce,
    /// The system is happy with animation.
    Allow,
    /// Not asked, or no usable answer.
    Unknown,
}

impl Query {
    /// Whether to treat this as a request to reduce motion.
    ///
    /// `Unknown` is *not* a reduction: see the module note.
    pub fn reduces(self) -> bool {
        matches!(self, Query::Reduce)
    }
}

/// Resolve a tool to an absolute path. Never searches `PATH`.
fn tool(directories: &[&str], name: &str) -> Option<PathBuf> {
    for directory in directories {
        if directory.is_empty() {
            continue;
        }
        let candidate = Path::new(directory).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Ask the operating system. Called once, at startup.
pub fn query() -> Query {
    #[cfg(windows)]
    {
        windows_query()
    }
    #[cfg(target_os = "macos")]
    {
        macos_query()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        unix_query()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Query::Unknown
    }
}

/// Windows: "Show animations in Windows" lives in the `UserPreferencesMask`
/// under `HKCU\Control Panel\Desktop`. It is a little-endian bit field, and
/// **bit 1 of byte 0** is `CLIENTAREAANIMATION` -- set when animation is
/// wanted, clear when the user has switched it off.
#[cfg(windows)]
fn windows_query() -> Query {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let Some(reg) = tool(
        &[&format!(r"{root}\System32"), &format!(r"{root}\Sysnative")],
        "reg.exe",
    ) else {
        return Query::Unknown;
    };

    let Ok(output) = no_window(Command::new(reg))
        .args([
            "query",
            r"HKCU\Control Panel\Desktop",
            "/v",
            "UserPreferencesMask",
        ])
        .output()
    else {
        return Query::Unknown;
    };
    if !output.status.success() {
        return Query::Unknown;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_user_preferences_mask(&text)
}

/// Pull the mask out of `reg query` output and read the animation bit.
#[cfg(windows)]
fn parse_user_preferences_mask(text: &str) -> Query {
    let Some(line) = text.lines().find(|l| l.contains("UserPreferencesMask")) else {
        return Query::Unknown;
    };
    // `... REG_BINARY    9E3E078012000000`
    let Some(hex) = line.split_whitespace().last() else {
        return Query::Unknown;
    };
    if hex.len() < 2 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Query::Unknown;
    }
    let Ok(first_byte) = u8::from_str_radix(&hex[..2], 16) else {
        return Query::Unknown;
    };
    // Bit 1 set means client-area animation is enabled.
    if first_byte & 0b10 != 0 {
        Query::Allow
    } else {
        Query::Reduce
    }
}

/// macOS: the Accessibility "Reduce motion" switch.
#[cfg(target_os = "macos")]
fn macos_query() -> Query {
    let Some(defaults) = tool(&["/usr/bin", "/bin"], "defaults") else {
        return Query::Unknown;
    };
    let Ok(output) = no_window(Command::new(defaults))
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .output()
    else {
        return Query::Unknown;
    };
    if !output.status.success() {
        // The key is absent until the switch has been touched, and `defaults`
        // exits non-zero for that. Absent means not reduced.
        return Query::Allow;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "1" | "true" => Query::Reduce,
        "0" | "false" => Query::Allow,
        _ => Query::Unknown,
    }
}

/// Linux and the BSDs: GNOME's `enable-animations`, which the other major
/// desktops have largely adopted as the common key.
#[cfg(all(unix, not(target_os = "macos")))]
fn unix_query() -> Query {
    let Some(gsettings) = tool(&["/usr/bin", "/bin", "/usr/local/bin"], "gsettings") else {
        return Query::Unknown;
    };
    let Ok(output) = no_window(Command::new(gsettings))
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
    else {
        return Query::Unknown;
    };
    if !output.status.success() {
        return Query::Unknown;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "false" => Query::Reduce,
        "true" => Query::Allow,
        _ => Query::Unknown,
    }
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
        let source = include_str!("reduced_motion.rs").replace("\r\n", "\n");
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

    /// Whatever this machine says, it must say it without panicking and
    /// without hanging.
    #[test]
    fn the_platform_can_be_asked_without_incident() {
        let started = std::time::Instant::now();
        let answer = query();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the probe took too long to be run at startup"
        );
        // Any of the three is a legitimate answer; the point is that we got one.
        assert!(matches!(
            answer,
            Query::Reduce | Query::Allow | Query::Unknown
        ));
    }

    /// "I do not know" must not switch animation off for everybody on a
    /// platform this cannot read.
    #[test]
    fn not_knowing_is_not_a_request_to_reduce() {
        assert!(!Query::Unknown.reduces());
        assert!(!Query::Allow.reduces());
        assert!(Query::Reduce.reduces());
    }

    #[test]
    fn a_missing_tool_yields_unknown_rather_than_a_search() {
        assert!(tool(&["/definitely/not/here"], "gsettings").is_none());
        assert!(tool(&[""], "reg.exe").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_mask_is_read_from_the_right_bit() {
        // Bit 1 of the first byte is CLIENTAREAANIMATION.
        let allow = "    UserPreferencesMask    REG_BINARY    9E3E078012000000";
        assert_eq!(parse_user_preferences_mask(allow), Query::Allow);

        // 0x9C has bit 1 clear: animation switched off.
        let reduce = "    UserPreferencesMask    REG_BINARY    9C3E078012000000";
        assert_eq!(parse_user_preferences_mask(reduce), Query::Reduce);

        // Anything unrecognisable is Unknown, never a guess.
        for bad in [
            "",
            "UserPreferencesMask",
            "    UserPreferencesMask    REG_BINARY    ",
            "    UserPreferencesMask    REG_BINARY    zzzz",
            "    SomethingElse    REG_BINARY    9E3E0780",
        ] {
            assert_eq!(
                parse_user_preferences_mask(bad),
                Query::Unknown,
                "{bad:?} should be Unknown"
            );
        }
    }

    /// The real registry value, on the machine running the test, must parse.
    #[cfg(windows)]
    #[test]
    fn the_real_windows_setting_parses() {
        // Not asserting *which* answer: that is a fact about this machine.
        // Asserting that we did not fall through to Unknown, which would mean
        // the parser no longer understands the format.
        let answer = windows_query();
        assert!(
            matches!(answer, Query::Allow | Query::Reduce),
            "could not read UserPreferencesMask on a real Windows install"
        );
    }
}
