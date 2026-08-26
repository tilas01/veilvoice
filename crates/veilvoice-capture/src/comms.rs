// SPDX-License-Identifier: GPL-3.0-or-later
//! Communication programs, and how to put VeilVoice between you and them.
//!
//! # What this does, and the one thing it deliberately does not
//!
//! It finds the calling and messaging programs on this machine and tells you,
//! for each one, exactly where to point it so your voice goes through VeilVoice
//! before it goes to anybody else.
//!
//! It does **not** reach inside any of them. It does not read Signal's or
//! Matrix's traffic, hook their audio, inject into their processes or decrypt
//! anything. That is not a limitation being apologised for — it is the same
//! act this whole project exists to make useless, and a privacy tool that
//! shipped a way to intercept an end-to-end encrypted call would be arguing
//! against itself.
//!
//! # The route, and why it is a virtual cable
//!
//! Every program here lets you choose which microphone it uses. So:
//!
//! ```text
//!   your microphone  ->  VeilVoice  ->  a virtual audio cable
//!                                              |
//!                                              v
//!                                  Discord / Signal / anything,
//!                                  with the cable chosen as its "microphone"
//! ```
//!
//! Nothing has to know VeilVoice exists. The program asks the operating system
//! for a microphone, the operating system hands it the cable, and the cable
//! carries a voice that is not yours. That is why this works with programs
//! nobody here has ever tested, including ones that do not exist yet.
//!
//! # Your voice, not theirs
//!
//! This route veils **what you send**. It does nothing to what you receive:
//! the other people on the call are not going through VeilVoice, and their
//! voices arrive as they always did.
//!
//! Veiling a whole call — everybody, including the people at the other end —
//! means capturing what the program *plays*, which is a different mechanism on
//! every platform and is not built. [`INCOMING`] says so in the words a front
//! end should show, rather than letting somebody assume a recording of a call
//! is veiled on both sides.
//!
//! # In plain words
//!
//! If you want to talk to people through Discord, Signal, Telegram or anything
//! like them without your real voice going out, this tells you how: send your
//! microphone through VeilVoice, out to a virtual cable, and then tell the chat
//! program that the cable *is* your microphone. It never knows the difference.
//!
//! It only changes what **you** send. Everybody else on the call still sounds
//! like themselves, and this does not record or read anything they send you.

use crate::programs::Purpose;

/// One communication program, and where its microphone setting lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comm {
    /// A stable identifier.
    pub key: &'static str,
    /// What to call it.
    pub name: &'static str,
    /// Where the input device is chosen, as a person would navigate to it.
    ///
    /// Written out rather than linked: these menus move between versions, and
    /// a path a reader can recognise survives a redesign better than a
    /// screenshot or a deep link that stops working.
    pub where_to_look: &'static str,
    /// Executable names, lower-case, no path.
    pub processes: &'static [&'static str],
}

/// The programs this build knows where to look in.
///
/// Not a list of what is supported: **anything** that lets you choose a
/// microphone works, because the trick is done at the operating-system level
/// and the program is not consulted. This table exists to save somebody hunting
/// through a settings menu, and nothing depends on a program being in it.
pub const COMMS: &[Comm] = &[
    Comm {
        key: "discord",
        name: "Discord",
        where_to_look: "User Settings → Voice & Video → Input Device",
        processes: &[
            "discord.exe",
            "discordptb.exe",
            "discordcanary.exe",
            "discord",
        ],
    },
    Comm {
        key: "signal",
        name: "Signal",
        where_to_look: "Settings → Calls → Microphone",
        processes: &["signal.exe", "signal-desktop", "signal"],
    },
    Comm {
        key: "telegram",
        name: "Telegram",
        where_to_look: "Settings → Advanced → Calls → Input device",
        processes: &["telegram.exe", "telegram-desktop", "telegram"],
    },
    Comm {
        key: "element",
        name: "Element (Matrix)",
        where_to_look: "Settings → Voice & Video → Microphone",
        processes: &["element.exe", "element-desktop", "element"],
    },
    Comm {
        key: "slack",
        name: "Slack",
        where_to_look: "Preferences → Audio & Video → Microphone",
        processes: &["slack.exe", "slack"],
    },
    Comm {
        key: "zoom",
        name: "Zoom",
        where_to_look: "Settings → Audio → Microphone",
        processes: &["zoom.exe", "zoom"],
    },
    Comm {
        key: "teams",
        name: "Microsoft Teams",
        where_to_look: "Settings → Devices → Microphone",
        processes: &["teams.exe", "ms-teams.exe", "teams"],
    },
    Comm {
        key: "jitsi",
        name: "Jitsi Meet",
        where_to_look: "the microphone menu beside the mute button",
        processes: &["jitsi meet.exe", "jitsi-meet"],
    },
    Comm {
        key: "mumble",
        name: "Mumble",
        where_to_look: "Configure → Settings → Audio Input → Device",
        processes: &["mumble.exe", "mumble"],
    },
];

/// What to do, for one program.
pub fn instructions(comm: &Comm, cable: Option<&str>) -> String {
    let device = cable.unwrap_or("your virtual audio cable");
    format!(
        "In {}, go to {} and choose \"{}\".\n\
         Then run `veilvoice live --output \"{}\"` and talk normally.",
        comm.name, comm.where_to_look, device, device
    )
}

/// The general route, for a program not in the table.
pub const ANY_PROGRAM: &str = "\
This works with anything that lets you pick a microphone, whether or not it is \
listed here. Run VeilVoice's live scrambler with a virtual audio cable as its \
output, then choose that cable as the microphone in the other program. It asks \
the operating system for a microphone and the operating system hands it the \
cable; nothing has to know VeilVoice is there.";

/// What this route does **not** cover, in the words a front end should show.
pub const INCOMING: &str = "\
This changes what you send and nothing else. The other people on the call are \
not going through VeilVoice, so their voices arrive exactly as they always did \
-- and if you record the call, their half is not veiled. Veiling a whole call \
means capturing what the program plays back, which is a different mechanism on \
every operating system and is not built here.";

/// What this crate will not do, and why that is not an oversight.
pub const NO_INTERCEPTION: &str = "\
VeilVoice does not reach inside any of these programs. It does not read their \
traffic, hook their audio, inject into their processes or decrypt anything. \
That is deliberate rather than missing: intercepting an end-to-end encrypted \
call is the act this project exists to make useless, and a privacy tool that \
shipped a way to do it would be arguing against itself. The route above works \
entirely outside them, through a microphone they choose to use.";

/// Which of these are running now, and anything that went wrong looking.
///
/// The same reading of the process list [`crate::programs`] uses, and with the
/// same two limits. It sees a program that is **running**, not one that is in
/// a call -- a front end must say "is running" and not "is on a call". And the
/// second value is why the list may be short: a list that came back empty
/// because a tool failed is not an empty list, and saying so is the difference
/// between "nothing is running" and "I could not tell".
pub fn running() -> (Vec<&'static Comm>, Vec<String>) {
    let (names, problems) = crate::processes::running();
    let found = COMMS
        .iter()
        .filter(|comm| {
            comm.processes
                .iter()
                .any(|process| names.iter().any(|seen| seen == process))
        })
        .collect();
    (found, problems)
}

/// The program with this identifier.
pub fn by_key(key: &str) -> Option<&'static Comm> {
    COMMS.iter().find(|comm| comm.key == key)
}

/// How a running communication program should be described.
///
/// Deliberately [`Purpose::Capable`]'s weight rather than a recorder's: a chat
/// program being open says almost nothing, and treating it as an event is how a
/// monitor becomes noise nobody reads.
pub fn weight() -> Purpose {
    Purpose::Capable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_complete_and_uniquely_keyed() {
        let mut keys: Vec<&str> = COMMS.iter().map(|c| c.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two programs share a key");

        for comm in COMMS {
            assert!(!comm.name.is_empty(), "{}", comm.key);
            assert!(!comm.where_to_look.is_empty(), "{}", comm.key);
            assert!(!comm.processes.is_empty(), "{}", comm.key);
            for process in comm.processes {
                assert_eq!(
                    *process,
                    process.to_lowercase(),
                    "{}: process names are matched lower-case",
                    comm.key
                );
                assert!(!process.contains('/') && !process.contains('\\'));
            }
            assert_eq!(by_key(comm.key), Some(comm));
        }
        assert_eq!(by_key("nothing-like-this"), None);
    }

    /// The four the request named, plus the ones people actually use.
    #[test]
    fn the_programs_that_were_asked_about_are_there() {
        for key in ["discord", "telegram", "signal", "element"] {
            assert!(by_key(key).is_some(), "{key} should be known");
        }
    }

    #[test]
    fn instructions_name_the_menu_and_the_device() {
        let discord = by_key("discord").unwrap();
        let text = instructions(discord, Some("CABLE Input (VB-Audio Virtual Cable)"));
        assert!(text.contains("Discord"), "{text}");
        assert!(text.contains("Voice & Video"), "{text}");
        assert!(text.contains("CABLE Input"), "{text}");
        assert!(text.contains("veilvoice live"), "{text}");
    }

    /// With no cable found, the instructions still make sense rather than
    /// naming an empty string.
    #[test]
    fn instructions_without_a_cable_still_read_as_english() {
        let text = instructions(by_key("signal").unwrap(), None);
        assert!(text.contains("your virtual audio cable"), "{text}");
        assert!(!text.contains("\"\""), "{text}");
    }

    /// The two notes that keep this honest have to say the thing, not hint at
    /// it. A reader who skims must not come away thinking a recorded call is
    /// veiled on both sides.
    #[test]
    fn the_scope_notes_state_the_limit_outright() {
        let incoming = INCOMING.to_lowercase();
        assert!(
            incoming.contains("what you send and nothing else"),
            "{incoming}"
        );
        assert!(incoming.contains("their half is not veiled"), "{incoming}");
        assert!(incoming.contains("is not built here"), "{incoming}");

        let no = NO_INTERCEPTION.to_lowercase();
        assert!(no.contains("does not reach inside"), "{no}");
        assert!(no.contains("deliberate rather than missing"), "{no}");
    }

    /// The table is a convenience, and the crate has to say so -- otherwise
    /// somebody whose program is missing concludes it will not work.
    #[test]
    fn the_general_route_says_the_table_is_not_a_list_of_what_is_supported() {
        let any = ANY_PROGRAM.to_lowercase();
        assert!(any.contains("whether or not it is listed here"), "{any}");
        assert!(
            any.contains("nothing has to know veilvoice is there"),
            "{any}"
        );
    }

    /// A chat program being open is not an event. Weighting it like a screen
    /// recorder is how a monitor becomes noise nobody reads.
    #[test]
    fn a_running_chat_program_is_reported_at_the_lower_weight() {
        assert_eq!(weight(), Purpose::Capable);
        assert!(weight().phrasing().contains("which is not the"));
    }

    /// Reading the process list must not panic or hang, whatever is running.
    #[test]
    fn asking_what_is_running_is_safe_on_any_machine() {
        let (found, problems) = running();
        for comm in &found {
            assert!(by_key(comm.key).is_some());
        }
        // A short list because a tool failed is not a short list. Whatever the
        // answer, the reasons come back rather than being swallowed.
        for problem in &problems {
            assert!(!problem.is_empty());
        }
    }
}
