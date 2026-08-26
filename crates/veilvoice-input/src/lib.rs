// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! What on this machine could be watching the keyboard and the mouse.
//!
//! # This is a heuristic, and the crate is built to keep saying so
//!
//! There is no way to ask an operating system "is anything logging my
//! keystrokes" and get a true answer. The mechanisms a keylogger uses are the
//! same ones accessibility software, password managers, remote-support tools,
//! macro utilities and games legitimately use, and the good ones are written
//! not to be found. A tool that claimed to detect keyloggers would be making a
//! promise nothing can keep.
//!
//! So this does the one thing that *can* be done honestly: it names the
//! programs running right now that are **able** to see your input, says what
//! each one is for, and leaves the judgement where it belongs. Every finding is
//! phrased as capability, never as an accusation, and [`Finding::phrasing`]
//! exists so that no front end has to invent that wording and get it wrong.
//!
//! [`LIMITS`] is the paragraph a front end must show beside any result. It says
//! outright that a clean result proves nothing. That is not a disclaimer bolted
//! on; it is the most important thing this crate outputs, because somebody who
//! reads "nothing found" as "nothing there" has been made *less* safe by
//! running it.
//!
//! # What it does not do, deliberately
//!
//! It does not hook the keyboard, read input, count keystrokes, time them, or
//! watch the mouse. A program that monitored input to detect input monitoring
//! would be the thing it warns about, and on Windows it would need the same
//! `SetWindowsHookEx` that `#![forbid(unsafe_code)]` rules out anyway.
//!
//! It also does not scan memory, inspect other processes' handles or read the
//! registry's autostart keys. `veilvoice-watch` already covers persistence,
//! and duplicating it here would give two answers to one question.
//!
//! # In plain words
//!
//! Software that records what you type is real, and there is no honest way for
//! any program to tell you for certain whether it is on your computer. Anything
//! that claims otherwise is guessing and not admitting it.
//!
//! What this does instead: it looks at which programs are open, and tells you
//! which of them *could* see your typing or your mouse -- remote-support tools,
//! macro recorders, accessibility software, and so on. Most of the time these
//! are things you installed on purpose and there is nothing wrong. The point is
//! that you get to know they are running and decide for yourself.
//!
//! If it finds nothing, that does **not** mean nothing is watching. It means
//! nothing it knows how to recognise is open, which is a much smaller claim,
//! and this crate will keep saying so every time.

/// Why a program is in the table.
///
/// The distinction decides how loudly a front end should speak, and getting it
/// wrong in either direction is a real failure: treating a password manager as
/// a threat trains people to ignore the warning, and treating a remote-access
/// tool as background noise wastes the one finding that mattered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reach {
    /// Seeing input on other programs' windows is what it is for.
    ///
    /// Remote-control software, macro recorders, dedicated key-display tools.
    /// Running one of these is worth a line whatever else is true.
    Purpose,
    /// It reaches input to do its own job, and that job is not watching you.
    ///
    /// Password managers with global hotkeys, screen readers, streaming
    /// overlays. Ordinary, and named rather than hidden, because "ordinary" is
    /// a judgement for the person reading -- not for this table.
    Incidental,
}

impl Reach {
    /// The wording a front end should use, and the reason this is not left to
    /// each caller to phrase.
    ///
    /// Every sentence here is about *capability*. None of them says a program
    /// is doing anything, because this crate cannot know that and neither can
    /// anything else that only reads a process list.
    pub fn phrasing(self) -> &'static str {
        match self {
            Self::Purpose => {
                "is running, and reading input from other programs is what it is for -- \
                 which is not the same as it doing so now"
            }
            Self::Incidental => {
                "is running, and can see input as part of its own job -- which is \
                 usually exactly why you installed it"
            }
        }
    }
}

/// One program able to observe keyboard or mouse input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watcher {
    /// A stable identifier, and what an allowlist entry would name.
    pub key: &'static str,
    /// What to call it.
    pub name: &'static str,
    /// What the program is, in one clause, for somebody who has not met it.
    pub what: &'static str,
    /// Why it can see input at all.
    pub how: &'static str,
    /// Whether watching input is the point of it.
    pub reach: Reach,
    /// Executable names, lower-case and without a path.
    ///
    /// At least one of every Unix name must survive the kernel's fifteen
    /// character truncation of `/proc/<pid>/comm`, or the entry can never match
    /// on Linux. A test holds it.
    pub processes: &'static [&'static str],
}

/// Every program this build knows how to recognise.
///
/// **Not a list of keyloggers.** Almost everything here is software somebody
/// installed on purpose and uses every day. It is a list of things that *can*
/// see input, so that a person deciding whether to speak freely knows what is
/// open.
///
/// Ordered with [`Reach::Purpose`] first, so a front end showing a few shows
/// the ones that carry the most information.
pub const ALL: &[Watcher] = &[
    Watcher {
        key: "anydesk",
        name: "AnyDesk",
        what: "Remote desktop software.",
        how: "A remote operator sees the screen and sends keyboard and mouse \
              events, so input is carried by design.",
        reach: Reach::Purpose,
        processes: &["anydesk.exe", "anydesk"],
    },
    Watcher {
        key: "teamviewer",
        name: "TeamViewer",
        what: "Remote desktop and support software.",
        how: "Same as any remote-control tool: input is the channel.",
        reach: Reach::Purpose,
        processes: &["teamviewer.exe", "teamviewer_service.exe", "teamviewer"],
    },
    Watcher {
        key: "rustdesk",
        name: "RustDesk",
        what: "Open-source remote desktop software.",
        how: "Carries keyboard and mouse to whoever is connected.",
        reach: Reach::Purpose,
        processes: &["rustdesk.exe", "rustdesk"],
    },
    Watcher {
        key: "vnc",
        name: "A VNC server",
        what: "Remote screen and input sharing.",
        how: "A VNC server exists to accept keyboard and mouse from elsewhere.",
        reach: Reach::Purpose,
        // `x11vnc` and `tigervncserver` both fit fifteen characters; the longer
        // spellings are here for `ps` and for Windows.
        processes: &[
            "winvnc.exe",
            "tvnserver.exe",
            "vncserver.exe",
            "x11vnc",
            "tigervncserver",
            "vncserver",
        ],
    },
    Watcher {
        key: "autohotkey",
        name: "AutoHotkey",
        what: "A scripting language for automating keyboard and mouse.",
        how: "Its scripts register global hotkeys and can record input. \
              Enormously useful, and the same mechanism a logger would use.",
        reach: Reach::Purpose,
        processes: &["autohotkey.exe", "autohotkey64.exe", "autohotkeyu64.exe"],
    },
    Watcher {
        key: "keyviewer",
        name: "Keystroke display",
        what: "Shows what you type on screen, for demonstrations and streaming.",
        how: "It reads every key globally in order to draw it.",
        reach: Reach::Purpose,
        processes: &["carnac.exe", "keycastr", "screenkey", "showmethekey"],
    },
    Watcher {
        key: "keepass",
        name: "KeePass",
        what: "A password manager.",
        how: "Global auto-type sends your credentials into other programs, so \
              it registers a system-wide hotkey.",
        reach: Reach::Incidental,
        processes: &["keepass.exe", "keepassxc.exe", "keepassxc", "keepass"],
    },
    Watcher {
        key: "1password",
        name: "1Password",
        what: "A password manager.",
        how: "Fills credentials into other programs and listens for a global \
              shortcut to do it.",
        reach: Reach::Incidental,
        processes: &["1password.exe", "1password", "1passwordd"],
    },
    Watcher {
        key: "bitwarden",
        name: "Bitwarden",
        what: "A password manager.",
        how: "Auto-fill and a global shortcut, the same as any of them.",
        reach: Reach::Incidental,
        processes: &["bitwarden.exe", "bitwarden"],
    },
    Watcher {
        key: "nvda",
        name: "NVDA",
        what: "A screen reader.",
        how: "Accessibility software must see input to describe what it does. \
              This is the clearest case where the answer is simply that \
              somebody needs it.",
        reach: Reach::Incidental,
        processes: &["nvda.exe", "nvda"],
    },
    Watcher {
        key: "orca",
        name: "Orca",
        what: "The screen reader shipped with GNOME.",
        how: "Reads input through the accessibility bus, for the same reason.",
        reach: Reach::Incidental,
        processes: &["orca"],
    },
    Watcher {
        key: "streamdeck",
        name: "Stream Deck",
        what: "Elgato's macro keypad software.",
        how: "Binds global shortcuts so a key on the pad can act anywhere.",
        reach: Reach::Incidental,
        processes: &["streamdeck.exe", "stream deck.exe", "streamdeck"],
    },
    Watcher {
        key: "obs",
        name: "OBS Studio",
        what: "Recording and streaming software.",
        how: "Its global hotkeys work while another program has focus, which \
              means it is watching for them. Listed for completeness: \
              `veilvoice-capture` is what has something to say about OBS.",
        reach: Reach::Incidental,
        processes: &["obs64.exe", "obs32.exe", "obs"],
    },
];

/// The program with this identifier.
pub fn by_key(key: &str) -> Option<&'static Watcher> {
    ALL.iter().find(|watcher| watcher.key == key)
}

/// The entry a process name belongs to, if any.
pub fn matching(process: &str) -> Option<&'static Watcher> {
    let name = process.trim().to_ascii_lowercase();
    ALL.iter()
        .find(|watcher| watcher.processes.iter().any(|known| *known == name))
}

/// One program found running, and how to describe it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Which entry matched.
    pub watcher: &'static Watcher,
}

impl Finding {
    /// The whole sentence to show, capability and all.
    pub fn phrasing(&self) -> String {
        format!("{} {}", self.watcher.name, self.watcher.reach.phrasing())
    }
}

/// What was found, and everything that qualifies it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Programs running that can see input, most significant first.
    pub findings: Vec<Finding>,
    /// Anything that went wrong while looking.
    ///
    /// A short list because a tool failed is **not** a short list. Without
    /// this, "nothing found" and "I could not look" are the same output, and
    /// they mean opposite things.
    pub problems: Vec<String>,
}

impl Report {
    /// Whether anything at all could be established.
    ///
    /// False when the process listing itself failed. A caller must not print a
    /// reassuring summary in that case, and this is how it knows.
    pub fn is_answerable(&self) -> bool {
        self.problems.is_empty() || !self.findings.is_empty()
    }

    /// A one-line summary, phrased so it cannot be read as a clean bill of
    /// health.
    pub fn summary(&self) -> String {
        if !self.is_answerable() {
            return "Could not read the list of running programs, so nothing was \
                    checked."
                .to_string();
        }
        match self.findings.len() {
            0 => "Nothing this build recognises is running. That is not the same \
                  as nothing watching."
                .to_string(),
            1 => "1 running program can see keyboard or mouse input.".to_string(),
            many => format!("{many} running programs can see keyboard or mouse input."),
        }
    }
}

/// Look, and report. Changes nothing and reads no input.
pub fn look() -> Report {
    let (names, problems) = veilvoice_proc::running();
    let mut findings: Vec<Finding> = Vec::new();
    for name in &names {
        if let Some(watcher) = matching(name) {
            // One line per program, not per process: several helper processes
            // of one application would otherwise read as several findings.
            if !findings.iter().any(|f| f.watcher.key == watcher.key) {
                findings.push(Finding { watcher });
            }
        }
    }
    // Purpose before incidental, then by name, so the order is stable and the
    // informative entries come first.
    findings.sort_by(|a, b| {
        a.watcher
            .reach
            .cmp(&b.watcher.reach)
            .then_with(|| a.watcher.name.cmp(b.watcher.name))
    });
    Report { findings, problems }
}

/// What a reader must be told, in the words to tell them.
///
/// Shown beside every result rather than behind a link. The sentence that
/// matters most is the one about a clean result: somebody who reads "nothing
/// found" as "nothing there" has been made less safe by running this.
pub const LIMITS: &str = "\
This cannot tell you whether anything is logging your keystrokes, and nothing \
can. The ways a program reads input are the same ways accessibility software, \
password managers and remote-support tools read it, and software written to \
hide is written to hide from this too. So a result of nothing found does not \
mean nothing is watching -- it means nothing this build recognises is running, \
which is a much smaller claim. What is listed is what is *able* to see input, \
never what is doing so; most of it is software you installed on purpose.";

/// Why this crate does not watch input in order to detect input watching.
pub const WHY_NOT_HOOKING: &str = "\
VeilVoice does not hook the keyboard, read what you type, or time your \
keystrokes. Detecting input monitoring by monitoring input would make this the \
thing it warns about, and it would need exactly the mechanism a logger uses.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_complete_and_uniquely_keyed() {
        let mut keys: Vec<&str> = ALL.iter().map(|w| w.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two programs share a key");

        for watcher in ALL {
            assert!(!watcher.name.is_empty(), "{}", watcher.key);
            assert!(watcher.what.len() > 10, "{}", watcher.key);
            assert!(
                watcher.how.len() > 25,
                "{}: say why it can see input at all",
                watcher.key
            );
            assert!(!watcher.processes.is_empty(), "{}", watcher.key);
            for process in watcher.processes {
                assert_eq!(
                    *process,
                    process.to_lowercase(),
                    "{}: names are matched lower-case",
                    watcher.key
                );
                assert!(!process.contains('/') && !process.contains('\\'));
            }
            assert_eq!(by_key(watcher.key), Some(watcher));
        }
        assert_eq!(by_key("nothing-like-this"), None);
    }

    /// The same fifteen-character rule `veilvoice-proc` documents, tested next
    /// to the table it applies to -- because the table is where somebody adds a
    /// row, and a sixteen-character name would stop matching on Linux only.
    #[test]
    fn every_entry_has_a_name_that_survives_the_linux_truncation() {
        for watcher in ALL {
            let unix: Vec<&&str> = watcher
                .processes
                .iter()
                .filter(|process| !process.ends_with(".exe"))
                .collect();
            if unix.is_empty() {
                continue;
            }
            assert!(
                unix.iter().any(|process| process.len() <= 15),
                "{} has no name of fifteen characters or fewer, so it can never \
                 match on Linux: {unix:?}",
                watcher.key
            );
        }
    }

    /// **The most important test in this crate.**
    ///
    /// Every sentence a front end shows has to describe a capability. The
    /// moment one of them says a program *is* watching, this crate is making a
    /// claim it cannot support, and somebody acts on it.
    #[test]
    fn nothing_this_crate_says_accuses_a_program_of_anything() {
        // Deliberately **not** LIMITS. That paragraph has to name the thing
        // it cannot detect -- "cannot tell you whether anything is logging your
        // keystrokes" -- and a blunt search for "is logging" flags the denial
        // as though it were the accusation. It was written that way first and
        // failed on its own honesty. LIMITS has its own test below, which
        // checks it for the denials rather than against them.
        let mut sentences: Vec<String> = vec![
            Reach::Purpose.phrasing().to_string(),
            Reach::Incidental.phrasing().to_string(),
        ];
        for watcher in ALL {
            sentences.push(Finding { watcher }.phrasing());
            sentences.push(watcher.how.to_string());
        }
        sentences.push(Report::default().summary());

        for sentence in &sentences {
            let lower = sentence.to_lowercase();
            for accusation in [
                "is logging",
                "is recording your",
                "is watching you",
                "is stealing",
                "is spying",
                "keylogger detected",
            ] {
                assert!(
                    !lower.contains(accusation),
                    "\"{accusation}\" is a claim this crate cannot support:\n{sentence}"
                );
            }
        }

        // And the two that describe a kind of program have to say so outright.
        assert!(Reach::Purpose
            .phrasing()
            .contains("not the same as it doing so"));
        assert!(Reach::Incidental
            .phrasing()
            .contains("why you installed it"));
    }

    /// A clean result must never read as a clean machine. This is the sentence
    /// that decides whether running this makes somebody safer or less safe.
    #[test]
    fn a_clean_result_states_plainly_that_it_proves_nothing() {
        let empty = Report::default();
        let summary = empty.summary().to_lowercase();
        assert!(
            summary.contains("not the same as nothing watching"),
            "{summary}"
        );

        let limits = LIMITS.to_lowercase();
        assert!(limits.contains("and nothing can"), "{limits}");
        assert!(
            limits.contains("does not mean nothing is watching"),
            "{limits}"
        );
        assert!(limits.contains("never what is doing so"), "{limits}");
        assert!(
            limits.contains("installed on purpose"),
            "most of it is ordinary software and the note has to say so: {limits}"
        );
    }

    /// "I could not look" and "I looked and found nothing" are opposite
    /// answers. Reporting the first as the second is the failure this whole
    /// crate exists to avoid making.
    #[test]
    fn a_failed_look_is_not_reported_as_a_clean_one() {
        let broken = Report {
            findings: Vec::new(),
            problems: vec!["tasklist.exe: not found".to_string()],
        };
        assert!(!broken.is_answerable());
        let summary = broken.summary().to_lowercase();
        assert!(summary.contains("nothing was checked"), "{summary}");
        assert!(
            !summary.contains("not the same as nothing watching"),
            "a failed look must not borrow the clean-result wording: {summary}"
        );

        // A partial answer -- some findings *and* a problem -- is still an
        // answer, and must not be thrown away.
        let partial = Report {
            findings: vec![Finding {
                watcher: by_key("anydesk").unwrap(),
            }],
            problems: vec!["one reader failed".to_string()],
        };
        assert!(partial.is_answerable());
        assert!(partial.summary().contains("1 running program"));
    }

    #[test]
    fn a_process_name_finds_its_entry_however_it_is_written() {
        assert_eq!(matching("AnyDesk.exe").map(|w| w.key), Some("anydesk"));
        assert_eq!(matching("  anydesk  ").map(|w| w.key), Some("anydesk"));
        assert_eq!(matching("orca").map(|w| w.key), Some("orca"));
        assert_eq!(matching("notepad.exe"), None);
        assert_eq!(matching(""), None);
    }

    /// The dedicated tools sort first, so a front end showing three shows the
    /// three worth showing.
    #[test]
    fn the_informative_findings_come_first() {
        assert!(Reach::Purpose < Reach::Incidental);

        let mut report = Report {
            findings: vec![
                Finding {
                    watcher: by_key("keepass").unwrap(),
                },
                Finding {
                    watcher: by_key("anydesk").unwrap(),
                },
            ],
            problems: Vec::new(),
        };
        report.findings.sort_by(|a, b| {
            a.watcher
                .reach
                .cmp(&b.watcher.reach)
                .then_with(|| a.watcher.name.cmp(b.watcher.name))
        });
        assert_eq!(report.findings[0].watcher.key, "anydesk");
    }

    /// Looking at the real machine must not panic, hang, or change anything.
    #[test]
    fn looking_is_safe_wherever_this_runs() {
        let report = look();
        for finding in &report.findings {
            assert!(by_key(finding.watcher.key).is_some());
            assert!(!finding.phrasing().is_empty());
        }
        for problem in &report.problems {
            assert!(!problem.is_empty());
        }
        assert!(!report.summary().is_empty());
    }

    /// This crate must not become the thing it warns about. Checked against the
    /// source, because the argument for not doing it is only as good as the
    /// code continuing not to.
    #[test]
    fn this_crate_never_reads_input_itself() {
        let source = include_str!("lib.rs").replace("\r\n", "\n");
        // Code only. The module documentation *names* these mechanisms in order
        // to say it does not use them, and the first version of this test read
        // that explanation and reported it as the offence.
        let body: String = source
            .split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("///")
            })
            .collect::<Vec<_>>()
            .join("\n");
        for mechanism in [
            "SetWindowsHookEx",
            "GetAsyncKeyState",
            "XGrabKeyboard",
            "CGEventTap",
            "/dev/input",
            "evdev",
        ] {
            assert!(
                !body.contains(mechanism),
                "{mechanism} is how a logger works, not how one is found"
            );
        }
        assert!(
            WHY_NOT_HOOKING.contains("would make this the thing it warns about"),
            "the reason has to be written down, not just followed"
        );
    }
}
