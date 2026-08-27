// SPDX-License-Identifier: GPL-3.0-or-later
//! The programs this build knows can capture a screen.
//!
//! # A list, and therefore incomplete
//!
//! This is a table of names. Anything not in it is not reported, and the table
//! will never be complete: new recorders appear, people rename executables, and
//! a program written specifically to capture a screen quietly would simply not
//! be called `obs64.exe`. [`crate::SCOPE`] says so, and no front end may present
//! an empty report as "nothing is recording".
//!
//! What it is genuinely good for is the ordinary case, which is also the common
//! one: you have OBS open, VeilVoice notices, and you either wanted that or you
//! did not.
//!
//! # Capable of capturing is not the same as capturing
//!
//! Zoom being open does not mean Zoom is sharing your screen. Discord running
//! does not mean anybody is watching it. This crate reports **what is running**,
//! and the distinction is carried in every string it produces, because the
//! alternative is a privacy tool that cries wolf every time somebody opens a
//! chat application.
//!
//! Telling the difference needs the compositor to say who is holding a capture
//! session, and reaching that is FFI on every platform here. See
//! [`crate`] for what that costs and why it is not paid.
//!
//! # In plain words
//!
//! A list of programs that can record a screen, with what each one is and whether
//! recording is its purpose or merely something it can do.
//!
//! The difference matters. A screen recorder being open is worth telling you
//! about. A chat program being open is worth much less, because it can share a
//! screen and almost never is, and treating the two the same is how a warning
//! becomes noise that everybody learns to ignore.
//!
//! This is a list of names, not a list of threats. Almost everything on it is
//! software somebody installed on purpose.

/// One program known to be able to capture a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Program {
    /// A stable identifier, and what an allowlist entry names.
    pub key: &'static str,
    /// What to call it.
    pub name: &'static str,
    /// What it is, in one clause.
    pub what: &'static str,
    /// Whether capturing is the program's purpose, or merely something it can
    /// do.
    ///
    /// A recorder that is running is worth a line. A chat application that is
    /// running is worth very much less, and conflating the two is how a monitor
    /// becomes noise.
    pub purpose: Purpose,
    /// Executable names, lower-case and without a path.
    pub processes: &'static [&'static str],
}

/// Why a program is in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Recording or streaming the screen is what the program is for.
    Recorder,
    /// The program can share a screen, and mostly is not.
    ///
    /// Reported at a lower weight, and a front end should say "can share"
    /// rather than "is sharing" -- because this crate does not know, and
    /// neither does anything else that only looks at a process list.
    Capable,
}

impl Purpose {
    /// The wording a front end should use for this kind of program.
    pub fn phrasing(&self) -> &'static str {
        match self {
            Purpose::Recorder => "is running, and recording the screen is what it does",
            Purpose::Capable => {
                "is running, and can share a screen -- which is not the \
                                 same as doing it"
            }
        }
    }
}

/// Every program in the table.
///
/// Ordered with the dedicated recorders first, so a front end that shows a few
/// shows the ones that matter.
pub const ALL: &[Program] = &[
    Program {
        key: "obs",
        name: "OBS Studio",
        what: "a screen recorder and streamer",
        purpose: Purpose::Recorder,
        processes: &["obs64.exe", "obs32.exe", "obs.exe", "obs"],
    },
    Program {
        key: "sharex",
        name: "ShareX",
        what: "a screen capture and recording tool",
        purpose: Purpose::Recorder,
        processes: &["sharex.exe"],
    },
    Program {
        key: "bandicam",
        name: "Bandicam",
        what: "a screen recorder",
        purpose: Purpose::Recorder,
        processes: &["bdcam.exe", "bandicam.exe"],
    },
    Program {
        key: "camtasia",
        name: "Camtasia",
        what: "a screen recorder and editor",
        purpose: Purpose::Recorder,
        processes: &["camtasiastudio.exe", "camrecorder.exe"],
    },
    Program {
        key: "snagit",
        name: "Snagit",
        what: "a screen capture tool",
        purpose: Purpose::Recorder,
        processes: &["snagit32.exe", "snagiteditor.exe"],
    },
    Program {
        key: "ffmpeg",
        name: "ffmpeg",
        what: "a general media tool that can record a screen",
        purpose: Purpose::Recorder,
        processes: &["ffmpeg.exe", "ffmpeg"],
    },
    Program {
        key: "screen-recorder-gnome",
        name: "the GNOME screen recorder",
        what: "the desktop's own recorder",
        purpose: Purpose::Recorder,
        // Both spellings on purpose. The Linux kernel truncates
        // `/proc/<pid>/comm` to fifteen characters, so the full name never
        // appears there -- and the full name is still what `ps` and every
        // Windows listing give. Listing one of the two would have matched on
        // one platform and silently not on the other.
        processes: &[
            "gnome-shell-screenshot",
            "gnome-shell-scr",
            "gsr-kms-server",
        ],
    },
    Program {
        key: "simplescreenrecorder",
        name: "SimpleScreenRecorder",
        what: "a screen recorder",
        purpose: Purpose::Recorder,
        // Full name and the fifteen-character form the Linux kernel
        // reports. See the note on the GNOME entry above.
        processes: &["simplescreenrecorder", "simplescreenrec"],
    },
    Program {
        key: "kazam",
        name: "Kazam",
        what: "a screen recorder",
        purpose: Purpose::Recorder,
        processes: &["kazam"],
    },
    Program {
        key: "zoom",
        name: "Zoom",
        what: "a meeting application that can share a screen",
        purpose: Purpose::Capable,
        processes: &["zoom.exe", "cpthost.exe", "zoom", "zoom.us"],
    },
    Program {
        key: "teams",
        name: "Microsoft Teams",
        what: "a meeting application that can share a screen",
        purpose: Purpose::Capable,
        processes: &["ms-teams.exe", "teams.exe", "teams"],
    },
    Program {
        key: "discord",
        name: "Discord",
        what: "a chat application that can share a screen",
        purpose: Purpose::Capable,
        processes: &[
            "discord.exe",
            "discordptb.exe",
            "discordcanary.exe",
            "discord",
        ],
    },
    Program {
        key: "slack",
        name: "Slack",
        what: "a chat application that can share a screen",
        purpose: Purpose::Capable,
        processes: &["slack.exe", "slack"],
    },
    Program {
        key: "meet-webrtc",
        name: "a browser sharing a screen",
        what: "any browser can share a screen, and this cannot tell whether one is",
        purpose: Purpose::Capable,
        // Deliberately empty. A browser is running on almost every machine, so
        // matching one would make the report say "a browser is open" for ever,
        // which is true and useless. The entry stays so the *reason* is
        // documented where somebody would otherwise add it.
        processes: &[],
    },
];

/// Find the program a process name belongs to, if this build knows it.
///
/// The comparison is on the bare executable name, lower-cased. A path is
/// stripped first: process listings differ about whether they give one, and a
/// match that worked on one platform and not another would be the sort of bug
/// nobody notices until a user is relying on it.
pub fn matching(process: &str) -> Option<&'static Program> {
    let bare = process
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(process)
        .trim()
        .to_ascii_lowercase();
    if bare.is_empty() {
        return None;
    }
    ALL.iter().find(|program| {
        program
            .processes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&bare))
    })
}

/// Find a program by its [`Program::key`].
pub fn by_key(key: &str) -> Option<&'static Program> {
    ALL.iter().find(|program| program.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for program in ALL {
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

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<&str> = ALL.iter().map(|program| program.key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two programs share a key");
    }

    /// One executable name must not belong to two programs, or which one is
    /// reported depends on the order of the table.
    #[test]
    fn no_process_name_belongs_to_two_programs() {
        let mut seen: Vec<String> = Vec::new();
        for program in ALL {
            for process in program.processes {
                let lowered = process.to_ascii_lowercase();
                assert!(
                    !seen.contains(&lowered),
                    "{process} is claimed by two programs"
                );
                seen.push(lowered);
            }
        }
    }

    #[test]
    fn a_known_recorder_is_found_however_it_is_spelled() {
        for spelling in [
            "obs64.exe",
            "OBS64.EXE",
            r"C:\Program Files\obs-studio\bin\64bit\obs64.exe",
            "/usr/bin/obs",
            "  obs64.exe  ",
        ] {
            let found = matching(spelling).unwrap_or_else(|| panic!("{spelling}"));
            assert_eq!(found.key, "obs");
        }
    }

    #[test]
    fn an_unknown_process_matches_nothing() {
        for nothing in ["", "   ", "notepad.exe", "veilvoice-gui.exe", "/", "\\"] {
            assert!(matching(nothing).is_none(), "{nothing:?} matched something");
        }
    }

    /// VeilVoice must never report itself. It draws a window; it does not
    /// capture one.
    #[test]
    fn veilvoice_is_not_in_its_own_table() {
        for program in ALL {
            for process in program.processes {
                assert!(
                    !process.to_ascii_lowercase().contains("veilvoice"),
                    "{process} is VeilVoice"
                );
            }
        }
    }

    /// A recorder and a chat application must not be described alike.
    #[test]
    fn a_program_that_merely_can_share_is_not_described_as_recording() {
        let capable = Purpose::Capable.phrasing();
        assert!(capable.contains("can share"), "{capable}");
        assert!(
            capable.contains("not the same as doing it"),
            "the distinction must be in the sentence: {capable}"
        );
        assert!(Purpose::Recorder.phrasing().contains("what it does"));
        assert_ne!(Purpose::Recorder.phrasing(), capable);
    }

    #[test]
    fn every_program_says_what_it_is() {
        for program in ALL {
            assert!(!program.name.trim().is_empty(), "{}", program.key);
            assert!(!program.what.trim().is_empty(), "{}", program.key);
            assert!(!program.key.trim().is_empty());
            assert_eq!(by_key(program.key), Some(program));
        }
        assert!(by_key("not-a-program").is_none());
    }

    /// The browser entry is deliberately unmatched, and must stay that way: a
    /// browser is open on nearly every machine, so matching one would make the
    /// report permanently true and permanently useless.
    #[test]
    fn the_browser_entry_matches_nothing_on_purpose() {
        let browser = by_key("meet-webrtc").unwrap();
        assert!(browser.processes.is_empty());
        assert!(browser.what.contains("cannot tell"));
    }

    /// Dedicated recorders come first, so a front end showing a few shows the
    /// ones that matter.
    #[test]
    fn recorders_are_listed_before_the_merely_capable() {
        let first_capable = ALL
            .iter()
            .position(|program| program.purpose == Purpose::Capable)
            .expect("there is at least one");
        assert!(
            ALL[first_capable..]
                .iter()
                .all(|program| program.purpose == Purpose::Capable),
            "a recorder is listed after a chat application"
        );
    }
}
