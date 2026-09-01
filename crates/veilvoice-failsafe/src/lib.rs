// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Failsafe: nothing leaves this machine in your own voice by accident.
//!
//! # The accident this exists to stop
//!
//! You set a calling program to VeilVoice's virtual cable, you start live mode,
//! and everything is veiled. Then you plug in a headset. Windows offers the new
//! microphone, the calling program takes it, and from that moment your **real
//! voice** is going out, with the veiled window still open in front of you,
//! still showing meters moving, still looking exactly as it did a second ago.
//!
//! Nobody notices that. It is not a mistake somebody makes through
//! carelessness; it is a mistake the operating system makes on their behalf.
//! Failsafe is on by default because the cost of it being off is that the one
//! thing this whole project exists to prevent happens silently.
//!
//! # What it can do, and what it cannot
//!
//! It **watches** which applications hold a microphone, and it knows which
//! device VeilVoice is veiling. If another program picks up a *real* microphone
//! while you are veiling, that is the accident, and Failsafe:
//!
//! 1. says so, loudly, because a silent guard is not a guard; and
//! 2. closes that program, if [`Posture::CloseIt`] is set, which it is by
//!    default, because a warning you have not read yet does not stop your voice
//!    going out.
//!
//! It **cannot stop the operating system handing a microphone to another
//! program in the first place.** Doing that needs exclusive-mode capture of
//! every input device, or a driver, and neither is something this project
//! ships. See [`CANNOT_PREVENT`], which is the wording a front end must show.
//! What Failsafe does is notice within a second or so and act. That is a real
//! difference from nothing, and it is a real difference from prevention, and
//! both halves have to be said.
//!
//! # Killing another program is a serious thing
//!
//! So it is bounded rather than general:
//!
//! * **VeilVoice never closes itself**, which would end the veiling it exists
//!   to protect.
//! * **It never closes a system process.** [`PROTECTED`] is a list, checked by
//!   name, and anything on it is reported and left alone.
//! * **It only closes what is actually holding a microphone**, from the watch
//!   feed, never a name from a guess.
//! * **Every close is recorded**, because a program that vanishes with nothing
//!   to explain it is indistinguishable from a crash.
//!
//! # In plain words
//!
//! Failsafe is on by default and it is the safety catch.
//!
//! The danger it guards against is this: you are talking through VeilVoice with
//! your voice disguised, you plug in headphones or a headset, and your computer
//! quietly switches the call over to the *real* microphone. Your actual voice
//! goes out and nothing on screen looks any different.
//!
//! Failsafe watches for exactly that. If another program picks up a real
//! microphone while you are being veiled, it tells you straight away and closes
//! that program so your voice stops going out.
//!
//! What it cannot do is stop your computer from handing the microphone over in
//! the first place, because that needs a level of access this program does not have,
//! and it says so rather than letting you believe otherwise. It notices, and it
//! acts, within about a second.

pub mod act;

use std::time::SystemTime;

/// How hard Failsafe acts when it finds something.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Posture {
    /// Watch, and say something.
    Watch,
    /// Watch, say something, and close the offending program.
    ///
    /// The default. A warning that has not been read yet does not stop a voice
    /// going out, and the whole point of this feature is the case where nobody
    /// is looking at the window.
    #[default]
    CloseIt,
    /// Do nothing.
    ///
    /// Offered, because a guard somebody cannot switch off is a guard they work
    /// around. Turning it off is a decision, and the interface says what it
    /// costs.
    Off,
}

impl Posture {
    /// A short name for a picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Watch => "warn me",
            Self::CloseIt => "warn me and close it",
            Self::Off => "off",
        }
    }

    /// What this choice costs and buys.
    pub fn note(self) -> &'static str {
        match self {
            Self::Watch => {
                "You are told the moment another program picks up a real microphone \
                 while you are being veiled. Nothing is closed, so your voice keeps \
                 going out until you act on the warning."
            }
            Self::CloseIt => {
                "The program that picked up the real microphone is closed, and you are \
                 told which and why. This is the default because a warning nobody has \
                 read yet does not stop a voice going out."
            }
            Self::Off => {
                "Nothing is watched. If your computer switches a call to your real \
                 microphone, your own voice goes out and nothing here will tell you."
            }
        }
    }

    /// Whether anything is being watched at all.
    pub fn is_on(self) -> bool {
        self != Self::Off
    }

    /// Every posture, in the order a picker should offer them.
    pub const ALL: &'static [Posture] = &[Posture::CloseIt, Posture::Watch, Posture::Off];

    /// The identifier written to the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Watch => "warn",
            Self::CloseIt => "close",
            Self::Off => "off",
        }
    }

    /// Read a posture back.
    ///
    /// An unrecognised value becomes the **default**, not `Off`. A settings file
    /// this build cannot read must never be the reason the safety catch is not
    /// on: of the two ways to be wrong, the one that keeps watching cannot let
    /// somebody's voice out.
    pub fn from_key(key: &str) -> Posture {
        Self::ALL
            .iter()
            .copied()
            .find(|posture| posture.key() == key)
            .unwrap_or_default()
    }
}

/// Processes Failsafe will never close, whatever they are holding.
///
/// Closing any of these ends the session, the desktop, or the machine. A guard
/// that can take the whole computer down to stop a microphone is a worse
/// problem than the one it was solving.
///
/// Matched on the executable's own name, lower-case, so a program somewhere
/// unusual is still protected.
pub const PROTECTED: &[&str] = &[
    // VeilVoice itself. Closing this stops the veiling.
    "veilvoice.exe",
    "veilvoice-gui.exe",
    "veilvoice",
    "veilvoice-gui",
    // Windows.
    "system",
    "system idle process",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "smss.exe",
    "svchost.exe",
    "dwm.exe",
    "explorer.exe",
    "audiodg.exe",
    // macOS and Linux.
    "launchd",
    "systemd",
    "pipewire",
    "pulseaudio",
    "wireplumber",
    "coreaudiod",
    "windowserver",
    "loginwindow",
];

/// Whether this program is one Failsafe refuses to close.
pub fn is_protected(app: &str) -> bool {
    let name = app
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(app)
        .to_ascii_lowercase();
    PROTECTED.iter().any(|known| *known == name)
}

/// What Failsafe found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// Nothing else holds a real microphone. This is the ordinary answer.
    Clear,
    /// Another program holds a real microphone while veiling is running.
    Foreign {
        /// What to call it.
        app: String,
        /// Its process, where the platform gave one.
        pid: Option<u32>,
        /// The device it took, where known.
        device: Option<String>,
        /// Whether Failsafe is allowed to close it.
        closeable: bool,
    },
    /// Veiling is not running, so there is nothing to protect yet.
    ///
    /// Its own answer rather than [`Finding::Clear`]: "nothing is wrong" and
    /// "there is nothing to be wrong yet" are different, and a front end that
    /// shows the first while live mode is stopped is telling somebody they are
    /// protected when nothing is being protected.
    Idle,
    /// The platform cannot say which applications hold a microphone.
    ///
    /// **Never reported as [`Finding::Clear`].** An empty list from a platform
    /// that cannot see is not good news, and this is the single most dangerous
    /// place in this crate to conflate the two.
    CannotTell {
        /// Why not, in the words to show.
        why: String,
    },
}

impl Finding {
    /// Whether this needs the reader's attention now.
    pub fn is_alarming(&self) -> bool {
        matches!(self, Finding::Foreign { .. })
    }

    /// The sentence to show.
    pub fn phrasing(&self) -> String {
        match self {
            Finding::Clear => {
                "Nothing else is holding a microphone. Your voice is going out veiled.".to_string()
            }
            Finding::Idle => {
                "Live veiling is not running, so there is nothing being protected yet.".to_string()
            }
            Finding::CannotTell { why } => format!(
                "Failsafe cannot see which programs hold a microphone on this system, so \
                 it cannot warn you: {why}"
            ),
            Finding::Foreign {
                app,
                device,
                closeable,
                ..
            } => {
                let where_ = match device {
                    Some(device) => format!(" ({device})"),
                    None => String::new(),
                };
                let action = if *closeable {
                    "It is being closed."
                } else {
                    "It is a system process, so it has been left alone -- change what it \
                     is using, or stop veiling."
                };
                format!(
                    "{app} has picked up a real microphone{where_} while you are being \
                     veiled. Your own voice may be going out. {action}"
                )
            }
        }
    }
}

/// One thing Failsafe did, for the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    /// When, as seconds since the Unix epoch.
    pub at: u64,
    /// Which program.
    pub app: String,
    /// Whether it was actually closed.
    pub closed: bool,
    /// What happened, or why not.
    pub detail: String,
}

/// What the watch feed says about one application holding a device.
///
/// A plain copy rather than a dependency on `veilvoice-watch`: this crate is
/// arithmetic over a list, and keeping it that way means its tests need no
/// machine, no microphone and no platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    /// What to call it.
    pub app: String,
    /// Its process, where known.
    pub pid: Option<u32>,
    /// The device it holds, where known.
    pub device: Option<String>,
}

/// The state Failsafe needs to decide anything.
///
/// `Default` gives the default **posture**, which is on -- so a guard that
/// nobody has configured still watches.
#[derive(Clone, Debug, Default)]
pub struct Guard {
    /// How hard to act.
    pub posture: Posture,
    /// The device VeilVoice is itself veiling, lower-case.
    ///
    /// A program holding *this* is not the accident: it is the arrangement
    /// working. Compared case-insensitively and by containment, because the
    /// same endpoint is spelled differently by different interfaces.
    pub veiling: Option<String>,
    /// Whether live veiling is running at all.
    pub live: bool,
    /// Everything Failsafe has done.
    log: Vec<Action>,
}

impl Guard {
    /// A guard in its default posture, watching nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a device name is the one being veiled.
    fn is_ours(&self, device: Option<&str>) -> bool {
        let (Some(ours), Some(theirs)) = (self.veiling.as_deref(), device) else {
            return false;
        };
        let ours = ours.trim().to_ascii_lowercase();
        let theirs = theirs.trim().to_ascii_lowercase();
        !ours.is_empty() && (theirs.contains(&ours) || ours.contains(&theirs))
    }

    /// Decide, given who holds a microphone right now.
    ///
    /// `problems` is whatever went wrong looking. A non-empty `problems` with an
    /// empty `holders` is [`Finding::CannotTell`], never [`Finding::Clear`].
    pub fn look(&self, holders: &[Holder], problems: &[String]) -> Finding {
        if !self.live {
            return Finding::Idle;
        }
        if holders.is_empty() && !problems.is_empty() {
            return Finding::CannotTell {
                why: problems.join("; "),
            };
        }
        for holder in holders {
            // Our own cable is the arrangement working, not the accident.
            if self.is_ours(holder.device.as_deref()) {
                continue;
            }
            // VeilVoice holding the real microphone is what veiling *is*.
            if is_veilvoice(&holder.app) {
                continue;
            }
            return Finding::Foreign {
                app: holder.app.clone(),
                pid: holder.pid,
                device: holder.device.clone(),
                closeable: self.posture == Posture::CloseIt && !is_protected(&holder.app),
            };
        }
        Finding::Clear
    }

    /// Record what was done about a finding.
    pub fn record(&mut self, at: SystemTime, app: &str, closed: bool, detail: &str) {
        let seconds = at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.log.push(Action {
            at: seconds,
            app: app.to_string(),
            closed,
            detail: detail.to_string(),
        });
    }

    /// Everything Failsafe has done, oldest first.
    pub fn log(&self) -> &[Action] {
        &self.log
    }
}

/// Whether this is one of VeilVoice's own programs.
fn is_veilvoice(app: &str) -> bool {
    let name = app
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(app)
        .to_ascii_lowercase();
    name.starts_with("veilvoice")
}

/// What Failsafe cannot do, in the words a front end must show.
pub const CANNOT_PREVENT: &str = "\
Failsafe cannot stop your computer handing a microphone to another program. \
Doing that needs exclusive control of every input device or a system driver, \
and VeilVoice ships neither. What it does is notice, within about a second, \
and act -- so there is a moment between another program taking a real \
microphone and Failsafe reacting to it. That moment is short and it is not \
zero, and anything that told you otherwise would be lying to you about how \
safe you are.";

/// What Failsafe refuses to close, and why.
pub const NEVER_CLOSES: &str = "\
Failsafe never closes VeilVoice itself, and never closes a system process. \
Closing either would end the veiling, the desktop or the session -- a guard \
that can take the whole computer down to stop a microphone is a worse problem \
than the one it was solving. Those are reported and left alone, with what to \
do instead.";

#[cfg(test)]
mod tests {
    use super::*;

    fn holder(app: &str, device: Option<&str>) -> Holder {
        Holder {
            app: app.to_string(),
            pid: Some(42),
            device: device.map(str::to_string),
        }
    }

    fn veiling_guard() -> Guard {
        Guard {
            posture: Posture::CloseIt,
            veiling: Some("CABLE Input (VB-Audio Virtual Cable)".into()),
            live: true,
            log: Vec::new(),
        }
    }

    /// **The accident this crate exists for.** Another program takes a real
    /// microphone while veiling is running, and it is caught.
    #[test]
    fn a_program_taking_a_real_microphone_while_veiling_is_the_alarm() {
        let guard = veiling_guard();
        let found = guard.look(&[holder("Discord.exe", Some("Headset Microphone"))], &[]);
        match &found {
            Finding::Foreign { app, closeable, .. } => {
                assert_eq!(app, "Discord.exe");
                assert!(closeable, "an ordinary program is closeable");
            }
            other => panic!("{other:?}"),
        }
        assert!(found.is_alarming());
        let words = found.phrasing();
        assert!(
            words.contains("your own voice may be going out")
                || words.contains("Your own voice may be going out")
        );
        assert!(words.contains("being closed"));
    }

    /// A program on our own cable is the arrangement working, not the accident.
    /// Reporting it would make the alarm fire constantly and be ignored.
    #[test]
    fn a_program_on_our_own_cable_is_not_the_accident() {
        let guard = veiling_guard();
        assert_eq!(
            guard.look(
                &[holder(
                    "Discord.exe",
                    Some("CABLE Input (VB-Audio Virtual Cable)")
                )],
                &[]
            ),
            Finding::Clear
        );
        // Spelled differently by different interfaces, and still ours.
        assert_eq!(
            guard.look(&[holder("Discord.exe", Some("cable input"))], &[]),
            Finding::Clear
        );
    }

    /// VeilVoice holding the real microphone is what veiling *is*.
    #[test]
    fn veilvoice_holding_the_microphone_is_not_reported_against_itself() {
        let guard = veiling_guard();
        assert_eq!(
            guard.look(
                &[holder("veilvoice-gui.exe", Some("Headset Microphone"))],
                &[]
            ),
            Finding::Clear
        );
        assert!(is_veilvoice("C:\\x\\veilvoice.exe"));
        assert!(!is_veilvoice("discord.exe"));
    }

    /// **The most dangerous conflation in the crate.** A platform that cannot
    /// see must never read as a quiet machine.
    #[test]
    fn a_platform_that_cannot_see_is_never_reported_as_clear() {
        let guard = veiling_guard();
        let found = guard.look(&[], &["no way to ask on this system".into()]);
        assert_ne!(found, Finding::Clear);
        match &found {
            Finding::CannotTell { why } => assert!(why.contains("no way to ask")),
            other => panic!("{other:?}"),
        }
        assert!(found.phrasing().contains("cannot warn you"));
        // And an empty list with nothing wrong really is clear.
        assert_eq!(guard.look(&[], &[]), Finding::Clear);
    }

    /// "Nothing is wrong" and "nothing is being protected" are different, and
    /// showing the first while live mode is stopped is a lie of reassurance.
    #[test]
    fn not_veiling_is_its_own_answer_rather_than_all_clear() {
        let mut guard = veiling_guard();
        guard.live = false;
        let found = guard.look(&[holder("Discord.exe", Some("Headset Microphone"))], &[]);
        assert_eq!(found, Finding::Idle);
        assert!(!found.is_alarming());
        assert!(found.phrasing().contains("nothing being protected"));
    }

    /// A guard that can close the desktop is worse than the problem.
    #[test]
    fn system_processes_and_veilvoice_itself_are_never_closeable() {
        let guard = veiling_guard();
        for system in ["explorer.exe", "audiodg.exe", "lsass.exe", "pipewire"] {
            let found = guard.look(&[holder(system, Some("Headset Microphone"))], &[]);
            match found {
                Finding::Foreign { closeable, .. } => {
                    assert!(!closeable, "{system} must never be closed")
                }
                other => panic!("{system}: {other:?}"),
            }
        }
        for ours in ["veilvoice.exe", "veilvoice-gui.exe", "VeilVoice-GUI.exe"] {
            assert!(is_protected(ours), "{ours}");
        }
        // Checked on the file name, so an unusual location is still protected.
        assert!(is_protected("C:\\Windows\\System32\\lsass.exe"));
        assert!(is_protected("/usr/lib/systemd"));
        assert!(!is_protected("discord.exe"));
    }

    /// The protected program is reported, and told what to do instead, rather
    /// than silently skipped.
    #[test]
    fn a_protected_program_still_warns_and_says_what_to_do() {
        let guard = veiling_guard();
        let found = guard.look(&[holder("explorer.exe", Some("Headset Microphone"))], &[]);
        let words = found.phrasing();
        assert!(words.contains("system process"), "{words}");
        assert!(words.contains("left alone"), "{words}");
        assert!(words.contains("stop veiling"), "{words}");
    }

    /// Watching without closing still warns, and says the voice keeps going.
    #[test]
    fn the_watch_only_posture_warns_but_does_not_close() {
        let mut guard = veiling_guard();
        guard.posture = Posture::Watch;
        match guard.look(&[holder("Discord.exe", Some("Headset Microphone"))], &[]) {
            Finding::Foreign { closeable, .. } => assert!(!closeable),
            other => panic!("{other:?}"),
        }
        assert!(Posture::Watch.note().contains("keeps going out"));
    }

    /// Off is off, and the interface says what that costs.
    #[test]
    fn off_watches_nothing_and_says_so_plainly() {
        assert!(!Posture::Off.is_on());
        assert!(Posture::CloseIt.is_on());
        assert!(Posture::Watch.is_on());
        let note = Posture::Off.note().to_lowercase();
        assert!(note.contains("your own voice goes out"), "{note}");
        assert!(note.contains("nothing here will tell you"), "{note}");
    }

    /// **On by default**, and a settings file this build cannot read must not
    /// be the reason the safety catch is off.
    #[test]
    fn the_default_is_on_and_an_unreadable_setting_stays_on() {
        assert_eq!(Posture::default(), Posture::CloseIt);
        assert!(Posture::default().is_on());
        assert_eq!(Posture::from_key("something-new"), Posture::CloseIt);
        assert_eq!(Posture::from_key(""), Posture::CloseIt);
        assert_ne!(
            Posture::from_key("nonsense"),
            Posture::Off,
            "an unreadable setting must never silence the safety catch"
        );
        // But a value that really says off is honoured.
        assert_eq!(Posture::from_key("off"), Posture::Off);
        for posture in Posture::ALL {
            assert_eq!(Posture::from_key(posture.key()), *posture);
        }
    }

    /// The limit is stated outright. This is the sentence that stops somebody
    /// believing they are protected in the gap.
    #[test]
    fn the_limits_say_it_notices_rather_than_prevents() {
        let cannot = CANNOT_PREVENT.to_lowercase();
        assert!(
            cannot.contains("cannot stop your computer handing"),
            "{cannot}"
        );
        assert!(cannot.contains("short and it is not zero"), "{cannot}");
        assert!(!cannot.contains("prevents another program"));

        let never = NEVER_CLOSES.to_lowercase();
        assert!(never.contains("never closes veilvoice itself"), "{never}");
        assert!(
            never.contains("worse problem than the one it was solving"),
            "{never}"
        );
    }

    /// Nothing this crate says claims to have prevented anything.
    #[test]
    fn nothing_here_claims_to_have_prevented_anything() {
        let mut sentences = vec![CANNOT_PREVENT.to_string(), NEVER_CLOSES.to_string()];
        for posture in Posture::ALL {
            sentences.push(posture.note().to_string());
        }
        for finding in [
            Finding::Clear,
            Finding::Idle,
            Finding::CannotTell { why: "x".into() },
            Finding::Foreign {
                app: "a".into(),
                pid: None,
                device: None,
                closeable: true,
            },
        ] {
            sentences.push(finding.phrasing());
        }
        for sentence in &sentences {
            let lower = sentence.to_lowercase();
            for claim in [
                "cannot be revealed",
                "impossible for your voice",
                "guarantees",
                "blocks other programs",
            ] {
                assert!(!lower.contains(claim), "\"{claim}\" in:\n{sentence}");
            }
        }
    }

    /// Every action is recorded. A program that vanishes with nothing to
    /// explain it is indistinguishable from a crash.
    #[test]
    fn every_action_is_written_down() {
        let mut guard = veiling_guard();
        assert!(guard.log().is_empty());
        guard.record(
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(99),
            "Discord.exe",
            true,
            "closed: it took Headset Microphone while veiling was running",
        );
        assert_eq!(guard.log().len(), 1);
        assert_eq!(guard.log()[0].at, 99);
        assert!(guard.log()[0].closed);
        assert!(guard.log()[0].detail.contains("Headset Microphone"));
    }
}
