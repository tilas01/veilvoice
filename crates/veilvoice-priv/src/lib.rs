// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! What privilege VeilVoice is running with, and what each level can actually
//! see.
//!
//! # Three levels, and the third one this project does not ship
//!
//! * [`Level::User`] — VeilVoice as you. Everything the de-identifier does
//!   happens here, and nothing about the engine, the container format or the
//!   app lock needs any more than this.
//! * [`Level::Elevated`] — running as administrator or root. The monitoring
//!   features see further: processes belonging to other users, service
//!   accounts, and a few registry and system paths that are unreadable
//!   otherwise.
//! * **Kernel level** — not shipped, and not for want of trying. Loading a
//!   kernel driver on 64-bit Windows needs an EV code-signing certificate
//!   issued to a verified legal entity plus Microsoft's attestation signing;
//!   macOS needs an Apple Developer ID and an entitlement granted case by
//!   case. Both are identity checks, and this project is published under a
//!   pseudonym on purpose. [`NO_KERNEL`] says so in the words a front end
//!   should show.
//!
//! # This crate does not elevate anything
//!
//! It reports. It does not re-launch VeilVoice as administrator, install a
//! service, or ask for a password. Those are changes to somebody's machine and
//! they belong to the person whose machine it is: [`Level::how_to_raise`]
//! prints the command, and they type it.
//!
//! That is not caution for its own sake. A privacy tool that silently acquires
//! administrator rights is a privacy tool nobody can reason about, and one that
//! installs a background service without being asked is worse — a service
//! outlives the window it was started from, and somebody who tried VeilVoice
//! once should not find it still running next month.
//!
//! # Detection is a measurement, and it can fail
//!
//! There is no `am_i_admin()` in the standard library and reaching the real
//! answer is FFI on every platform here. So this asks a tool the system already
//! ships, exactly as `veilvoice-watch` asks the registry — and when the tool
//! cannot be run, the answer is [`Level::Unknown`] rather than a guess.
//!
//! **`Unknown` is not `User`.** Reporting "not elevated" when the truth is "I
//! could not tell" would understate what VeilVoice can see, which sounds like
//! the safe direction and is not: somebody would conclude a feature is
//! unavailable and stop looking at its output.
//!
//! # In plain words
//!
//! Most of VeilVoice needs no special permissions at all — changing a voice is
//! something any program can do with your own account.
//!
//! The parts that *watch* your machine can see more when VeilVoice is run as an
//! administrator: programs belonging to other accounts, and a few places on the
//! system that are otherwise off limits. This tells you which of those you are
//! currently getting, and how to run it the other way if you want to.
//!
//! It will not do that for you. Running as administrator, or installing a
//! background service, is a change to your computer and it should be one you
//! made on purpose. And there is a third level — inside the operating system
//! itself — that VeilVoice does not reach and says so rather than implying it
//! does.

use std::process::Command;

/// What VeilVoice is running with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    /// An ordinary user account.
    #[default]
    User,
    /// Administrator on Windows, root elsewhere.
    Elevated,
    /// The probe could not run.
    ///
    /// Deliberately not [`Level::User`]. "I could not tell" and "you are not
    /// elevated" lead to different actions, and reporting the second when the
    /// first is true understates what VeilVoice can see — which sounds like
    /// the cautious direction and is not.
    Unknown,
}

impl Level {
    /// A short name.
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "your own account",
            Self::Elevated => {
                if cfg!(windows) {
                    "administrator"
                } else {
                    "root"
                }
            }
            Self::Unknown => "could not tell",
        }
    }

    /// What this level can see, and what it cannot.
    pub fn what_it_sees(self) -> &'static str {
        match self {
            Self::User => {
                "Everything VeilVoice does to audio happens here and needs nothing \
                 more. The monitoring features see programs running as you, and not \
                 usually much else -- so something running as another user or as a \
                 service may not appear at all."
            }
            Self::Elevated => {
                "The monitoring features can see processes belonging to other users \
                 and to service accounts, and can read system locations that are \
                 otherwise refused. The de-identifier itself does nothing different: \
                 it never needed this."
            }
            Self::Unknown => {
                "The check could not be run, so this does not know. That is not the \
                 same as running unprivileged: treat the monitoring output as \
                 whatever it says it is, and do not assume a short list means a \
                 quiet machine."
            }
        }
    }

    /// The command that would run VeilVoice at the higher level.
    ///
    /// Returned as text to print, never run. Elevating is a change to somebody's
    /// machine and it belongs to them.
    pub fn how_to_raise(self) -> Option<&'static str> {
        match self {
            Self::Elevated => None,
            _ if cfg!(windows) => Some(
                "Right-click VeilVoice and choose \"Run as administrator\", or from an \
                 elevated PowerShell: Start-Process veilvoice -Verb RunAs",
            ),
            _ => Some("Run it under sudo: sudo veilvoice ..."),
        }
    }

    /// Whether the monitoring features are seeing everything they could.
    pub fn is_full_view(self) -> bool {
        self == Self::Elevated
    }
}

/// What VeilVoice is running with right now.
///
/// Asks the system's own tool. Never elevates, never prompts, changes nothing.
pub fn level() -> Level {
    #[cfg(windows)]
    {
        windows_level()
    }
    #[cfg(not(windows))]
    {
        unix_level()
    }
}

/// On Windows, ask `whoami /groups` for the administrators SID.
///
/// `S-1-5-32-544` is the built-in Administrators group, and the well-known SID
/// is used rather than the group's *name*, which is translated on a localised
/// system and would make this answer "not elevated" on every machine that is
/// not in English.
#[cfg(windows)]
fn windows_level() -> Level {
    // Absolute path, never a bare name: Windows searches the current directory
    // before most of PATH, so a `whoami.exe` in the folder VeilVoice was
    // unpacked into would answer this question instead. This is a security
    // tool asking what privilege it holds.
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let program = format!(r"{root}\System32\whoami.exe");

    let mut command = Command::new(&program);
    command.args(["/groups"]);
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = match command.output() {
        Ok(output) if output.status.success() => output,
        _ => return Level::Unknown,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains("S-1-5-32-544") {
        // The group is not even in the token: an ordinary account.
        return Level::User;
    }
    // Present, but a filtered token lists it as "Group used for deny only",
    // which is what an un-elevated administrator account looks like. Being a
    // member of Administrators is not the same as running as one, and getting
    // this wrong would overstate what VeilVoice can see -- the dangerous
    // direction.
    //
    // Both parts are on one line. `whoami /groups` pads its columns to about
    // 236 characters, so a console displaying it wraps and it *looks* like two
    // rows; reading it that way and matching only the first would report every
    // administrator account as elevated whether or not it was. Measured on a
    // machine in exactly that state: the account is in Administrators, the
    // shell is not elevated, and this reports "your own account".
    for line in text.lines() {
        if line.contains("S-1-5-32-544") {
            if line.contains("Group used for deny only") {
                return Level::User;
            }
            return Level::Elevated;
        }
    }
    Level::User
}

/// Everywhere else, ask `id -u`.
#[cfg(not(windows))]
fn unix_level() -> Level {
    let output = match Command::new("/usr/bin/id").arg("-u").output() {
        Ok(output) if output.status.success() => output,
        // Not every system puts it there. One fallback, through PATH, and then
        // the answer is that this does not know.
        _ => match Command::new("id").arg("-u").output() {
            Ok(output) if output.status.success() => output,
            _ => return Level::Unknown,
        },
    };
    match String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
    {
        Ok(0) => Level::Elevated,
        Ok(_) => Level::User,
        Err(_) => Level::Unknown,
    }
}

/// Whether a background service is installed.
///
/// Always `false` for now, and the function exists so that a front end asking
/// the question gets an answer rather than a missing feature: **VeilVoice does
/// not install a service.** [`NO_SERVICE`] is why.
pub fn service_installed() -> bool {
    false
}

/// Why the opt-in service is not shipped, in the words to show.
pub const NO_SERVICE: &str = "\
VeilVoice does not install a background service. A service outlives the window \
it was started from, starts itself at boot, and runs whether or not anybody is \
using the program -- so somebody who tried VeilVoice once should not find it \
still running next month. Where continuous monitoring is wanted, run VeilVoice \
and leave it open: what it can see is then exactly what it says it can see, and \
closing the window ends it.";

/// What kernel level would need, and why it is not here.
pub const NO_KERNEL: &str = "\
VeilVoice does not reach inside the operating system, and cannot. A kernel \
driver on 64-bit Windows needs an EV code-signing certificate issued to a \
verified legal entity and then Microsoft's attestation signing; macOS needs an \
Apple Developer ID and an entitlement Apple grants case by case. Both are \
identity checks on a named legal person, and this project is published under a \
pseudonym on purpose. So the monitoring here is what a program running as you, \
or as an administrator, can observe from outside -- which is real, and is less \
than a driver would see.";

/// What this crate will not do, and why that is deliberate.
pub const NEVER_ELEVATES: &str = "\
This never raises its own privileges, installs anything, or asks for a \
password. It reports what VeilVoice is running with and prints the command that \
would run it differently. A privacy tool that silently acquires administrator \
rights is a privacy tool nobody can reason about.";

#[cfg(test)]
mod tests {
    use super::*;

    /// "I could not tell" must never be reported as "not elevated".
    #[test]
    fn unknown_is_its_own_answer_and_not_the_unprivileged_one() {
        assert_ne!(Level::Unknown, Level::User);
        assert!(!Level::Unknown.is_full_view());
        let words = Level::Unknown.what_it_sees().to_lowercase();
        assert!(words.contains("does not know"), "{words}");
        assert!(
            words.contains("not the same as running unprivileged"),
            "the distinction has to be stated, not implied: {words}"
        );
        assert!(
            words.contains("do not assume a short list"),
            "and the action it changes: {words}"
        );
    }

    /// Every level says what it can see, in enough words to be useful.
    #[test]
    fn every_level_explains_itself() {
        for level in [Level::User, Level::Elevated, Level::Unknown] {
            assert!(!level.label().is_empty(), "{level:?}");
            assert!(level.what_it_sees().len() > 80, "{level:?}");
        }
        // And the user level has to say the de-identifier needs nothing more,
        // or somebody runs the whole thing as root for no reason.
        assert!(Level::User.what_it_sees().contains("needs nothing more"));
        assert!(Level::Elevated
            .what_it_sees()
            .contains("it never needed this"));
    }

    /// Elevating is a command to print, never an action to take.
    #[test]
    fn raising_privilege_is_something_the_reader_does() {
        assert_eq!(Level::Elevated.how_to_raise(), None, "already there");
        for level in [Level::User, Level::Unknown] {
            let how = level.how_to_raise().expect("a command to print");
            assert!(how.len() > 20, "{how}");
        }

        // Nothing in this module *runs* an elevation. Checked by naming every
        // subprocess it starts rather than by searching for words: the strings
        // above legitimately contain "RunAs" and "sudo" because they are the
        // commands a reader is told to type, and the first version of this test
        // flagged them. An honest failure, and the wrong question -- what
        // matters is what is executed, so that is what is counted.
        let source = include_str!("lib.rs").replace("\r\n", "\n");
        let body = source.split("#[cfg(test)]").next().unwrap().to_string();
        let marker = "Command::new(";
        let started: Vec<String> = body
            .match_indices(marker)
            .map(|(at, _)| {
                let rest = &body[at + marker.len()..];
                rest.split(')').next().unwrap_or("").trim().to_string()
            })
            .collect();
        assert!(!started.is_empty(), "the probe has to run something");
        for program in &started {
            assert!(
                program == "&program" || program.contains("id"),
                "this module starts {program}, which is neither of the two probes"
            );
        }
    }

    /// The three scope notes each say the thing outright rather than hinting.
    #[test]
    fn the_limits_are_stated_rather_than_implied() {
        let kernel = NO_KERNEL.to_lowercase();
        assert!(kernel.contains("does not reach inside"), "{kernel}");
        assert!(kernel.contains("pseudonym"), "{kernel}");
        assert!(
            kernel.contains("less than a driver would see"),
            "the comparison a reader needs: {kernel}"
        );

        let service = NO_SERVICE.to_lowercase();
        assert!(service.contains("does not install a background service"));
        assert!(
            service.contains("still running next month"),
            "the concrete case: {service}"
        );
        assert!(service.contains("closing the window ends it"));

        let never = NEVER_ELEVATES.to_lowercase();
        assert!(never.contains("never raises its own privileges"));
        assert!(never.contains("nobody can reason about"));
    }

    /// No service is installed, and the function that says so is honest about
    /// it rather than absent.
    #[test]
    fn there_is_no_service_and_the_answer_says_so() {
        assert!(!service_installed());
    }

    /// Asking the real machine must not panic, hang, prompt, or change
    /// anything.
    #[test]
    fn asking_is_safe_wherever_this_runs() {
        let level = level();
        assert!(!level.label().is_empty());
        assert!(!level.what_it_sees().is_empty());
        // Whatever it is, the two that mean "not the full view" agree about it.
        assert_eq!(level.is_full_view(), level == Level::Elevated);
    }

    /// The Windows probe keys on the well-known SID, not the group's name.
    ///
    /// The name is translated on a localised system, so matching "Administrators"
    /// would report every non-English machine as unprivileged -- a wrong answer
    /// that would only ever be seen by people this project is unlikely to hear
    /// from.
    #[test]
    fn the_windows_probe_uses_the_sid_rather_than_a_translated_name() {
        let source = include_str!("lib.rs").replace("\r\n", "\n");
        assert!(source.contains("S-1-5-32-544"));
        let code: String = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("///")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("contains(\"Administrators\")"),
            "the group name is localised; the SID is not"
        );
    }
}
