// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything VeilVoice keeps between runs, in one list.
//!
//! # Why this is here and not in each module
//!
//! Every state file in this project is derived from [`crate::lock::default_path`]
//! and named where it is used: the settings file in the window's `prefs`, the
//! vaults in its `studio`, the allowlist in the command line's `capture`, the
//! mandate in `veilvoice-policy`. That is the right place for each of them and
//! this list does not replace any of it.
//!
//! What it adds is the question none of those can answer on its own: **what is
//! all of it**. Two things ask that. The About tab lists where this copy keeps
//! things, so somebody knows what to back up. The reset offers to remove it, so
//! somebody can start again. Both have to say the same thing about the same
//! files, and a list assembled inside the window cannot be read by the command
//! line, which is where the reset also has to work.
//!
//! # It does not have to be complete for the reset to be
//!
//! That would be a promise this could not keep: a file added tomorrow in a
//! crate that has never heard of this module would be missed, and a reset that
//! silently leaves something behind is worse than no reset.
//!
//! So the reset works by exclusion: it removes everything in the folder and
//! keeps what it was told to keep. This list is what puts a **name** on what is
//! about to go, not what decides that it goes. Something not listed here is
//! still removed; it is simply described as one of the files VeilVoice writes
//! rather than by what it is for.
//!
//! # In plain words
//!
//! One list of the things VeilVoice keeps on your computer, with a line each
//! about what they are, so the About tab and the reset both say the same thing
//! about the same files.

use std::path::PathBuf;

/// One thing VeilVoice keeps between runs.
///
/// Named rather than indexed, so a caller asks for the settings file by asking
/// for the settings file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// The window's own preferences.
    Settings,
    /// Colour schemes somebody wrote.
    Palettes,
    /// The Studio's vaults.
    Vaults,
    /// Policy files, where somebody else decided a setting.
    Policies,
    /// What `veilvoice mandate` insists on.
    Mandate,
    /// What VeilVoice's own files measured, so a change is noticed.
    Integrity,
    /// The report a failure left behind.
    CrashReport,
    /// The marker that says a session was running.
    SessionMarker,
    /// The screen-recorder allowlist.
    Captures,
    /// The canaries and the rate limits.
    Sentry,
    /// The app lock, and everything that opens with it.
    AppLock,
}

/// One entry: what it is called, what it is, and what losing it costs.
pub struct Entry {
    /// Which thing this is.
    pub item: Item,
    /// Its name inside the folder [`dir`] answers with.
    ///
    /// The app lock's entry names its index rather than the lock itself,
    /// because the lock's own two copies are kept under names derived from
    /// that index. See [`crate::vault`], which is the one place that knows
    /// them.
    pub name: &'static str,
    /// Whether it is a folder rather than a file.
    pub folder: bool,
    /// What it is, in the words the About tab uses.
    pub label: &'static str,
    /// One line for somebody deciding whether to remove it.
    pub note: &'static str,
    /// Whether this is part of what "keep the keys" keeps.
    pub keys: bool,
    /// Whether nothing can put it back.
    ///
    /// The distinction the reset asks somebody to type a word about. Settings
    /// have defaults and the integrity record is taken again on the next
    /// launch; a recording and a colour scheme somebody wrote are gone.
    pub irreplaceable: bool,
}

/// Every named thing, in the order a person meets it.
///
/// Ordered by what it costs to lose rather than alphabetically: the things
/// somebody would miss are at the top, where a reader who stops reading has
/// still seen them.
pub const ALL: &[Entry] = &[
    Entry {
        item: Item::AppLock,
        name: "applock.index",
        folder: false,
        label: "app lock",
        note: "the password on VeilVoice itself, and the key to everything \
               sealed with it. Kept under names derived from an index, which \
               is why this shows the folder rather than the files",
        keys: true,
        irreplaceable: true,
    },
    Entry {
        item: Item::Vaults,
        name: "studio",
        folder: true,
        label: "vaults",
        note: "the Studio's recordings, and any decoys made beside them",
        keys: false,
        irreplaceable: true,
    },
    Entry {
        item: Item::Palettes,
        name: "palettes",
        folder: true,
        label: "palettes",
        note: "colour schemes you wrote, read at startup",
        keys: false,
        irreplaceable: true,
    },
    Entry {
        item: Item::Settings,
        name: "settings.conf",
        folder: false,
        label: "settings",
        note: "the palette, the motion setting, the device choices, and what \
               the walkthrough has already shown you",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::Policies,
        name: "policy",
        folder: true,
        label: "policies",
        note: "settings somebody else decided, if any are in force",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::Mandate,
        name: "mandate.conf",
        folder: false,
        label: "the mandate",
        note: "which protections `veilvoice mandate` insists on for this copy",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::Captures,
        name: "capture",
        folder: true,
        label: "screen-recorder allowlist",
        note: "which recorders you have said are yours",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::Sentry,
        name: "sentry",
        folder: true,
        label: "the sentry's own files",
        note: "the canaries it watches and the limits it counts against",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::Integrity,
        name: "integrity.manifest",
        folder: false,
        label: "integrity record",
        note: "what VeilVoice's own files measured, taken again when it is not \
               there",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::CrashReport,
        name: "last-crash.txt",
        folder: false,
        label: "crash report",
        note: "written only by a failure, and offered on the next launch",
        keys: false,
        irreplaceable: false,
    },
    Entry {
        item: Item::SessionMarker,
        name: "session.running",
        folder: false,
        label: "session marker",
        note: "present while a session is running, so the next launch knows \
               the last one did not finish",
        keys: false,
        irreplaceable: false,
    },
];

/// The folder all of this is in, if this platform says where one is.
///
/// The same answer [`crate::lock::default_dir`] gives, because it is that
/// answer: a second way of working out where the state lives is the defect
/// `tools/audit/state_paths.py` exists to catch.
pub fn dir() -> Option<PathBuf> {
    crate::lock::default_dir()
}

/// The entry for one thing.
pub fn entry(item: Item) -> &'static Entry {
    ALL.iter()
        .find(|entry| entry.item == item)
        // Every variant is in `ALL` and a test refuses a build where one is
        // not, so this cannot be reached. It is stated rather than returned as
        // an `Option` a dozen callers would each have to answer.
        .expect("every item is listed")
}

/// Where one thing is, if this platform says where anything is.
pub fn path(item: Item) -> Option<PathBuf> {
    dir().map(|dir| dir.join(entry(item).name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant is listed, and listed once.
    ///
    /// [`entry`] asserts this at runtime for lack of anywhere better to say
    /// it, so this is what makes that assertion unreachable rather than
    /// hopeful.
    #[test]
    fn every_item_is_listed_exactly_once() {
        for item in [
            Item::Settings,
            Item::Palettes,
            Item::Vaults,
            Item::Policies,
            Item::Mandate,
            Item::Integrity,
            Item::CrashReport,
            Item::SessionMarker,
            Item::Captures,
            Item::Sentry,
            Item::AppLock,
        ] {
            let found = ALL.iter().filter(|entry| entry.item == item).count();
            assert_eq!(found, 1, "{item:?} is listed {found} times");
        }
        assert_eq!(ALL.len(), 11, "something is listed that no item names");
    }

    #[test]
    fn no_two_things_share_a_name() {
        let mut names: Vec<&str> = ALL.iter().map(|entry| entry.name).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "two entries name one file");
    }

    /// A name here is a name on somebody's disk, so it has to be one.
    #[test]
    fn every_name_is_a_plain_name_rather_than_a_path() {
        for entry in ALL {
            assert!(!entry.name.is_empty(), "{:?} has no name", entry.item);
            assert!(
                !entry.name.contains('/') && !entry.name.contains('\\'),
                "{} is a path rather than a name, so it would escape the folder",
                entry.name
            );
            assert!(
                !entry.label.is_empty() && entry.note.len() > 20,
                "{} is listed without saying what it is",
                entry.name
            );
        }
    }

    /// The keys are the app lock and nothing else.
    ///
    /// "Keep the keys" is an offer about one thing, and a second entry
    /// quietly marked `keys` would widen what a reset leaves behind without
    /// anybody deciding to.
    #[test]
    fn the_only_key_is_the_app_lock() {
        let keys: Vec<Item> = ALL
            .iter()
            .filter(|entry| entry.keys)
            .map(|entry| entry.item)
            .collect();
        assert_eq!(keys, vec![Item::AppLock]);
    }

    #[test]
    fn a_path_is_inside_the_folder_it_says_it_is() {
        let Some(dir) = dir() else {
            return; // a platform that does not say where configuration goes
        };
        for entry in ALL {
            let path = path(entry.item).expect("a folder means every path resolves");
            assert_eq!(path.parent(), Some(dir.as_path()));
            assert_eq!(
                path.file_name().map(|n| n.to_string_lossy().to_string()),
                Some(entry.name.to_string())
            );
        }
    }
}
