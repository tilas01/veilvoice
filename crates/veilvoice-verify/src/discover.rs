// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding a release to check, without being told where it is.
//!
//! # Why this exists
//!
//! Verifying a download used to require naming three files and knowing a tag.
//! That is fine for somebody who has already read the instructions and is
//! wrong for everybody else, and "everybody else" is precisely the population
//! a verifier exists to serve. Somebody who has just downloaded an archive and
//! wants to know whether it is the real one should be able to run this and be
//! told.
//!
//! So: look in the obvious places, in an obvious order, and say what was found.
//! Nothing here downloads, nothing here guesses at a hash, and nothing here
//! reports "verified" on the strength of a filename.
//!
//! # Where it looks, and why in that order
//!
//! 1. The directory given, if one was.
//! 2. The current working directory, where somebody who has just `cd`-ed to
//!    their downloads will be.
//! 3. The directory the running binary is in, where somebody who unpacked the
//!    archive and double-clicked the verifier inside it will be.
//! 4. The usual download directories for the platform.
//!
//! Each is searched one level deep only. A recursive walk of a home directory
//! is slow, surprising, and would let a verifier wander into places nobody
//! asked it to look at.
//!
//! # What counts as a release archive
//!
//! A file whose name starts `veilvoice-` and ends in one of the archive
//! extensions this project publishes. That is a **filename** test and it proves
//! nothing at all: it is how candidates are found, never how they are judged.
//! Every candidate still has to survive the signature and the hash, and a file
//! that merely looks the part fails exactly as loudly as one that does not.
//!
//! # In plain words
//!
//! Looks for a downloaded release to check, so you can double-click the verifier
//! and have it work.
//!
//! It looks in the folder it is in, the current folder, and your Downloads and
//! Desktop. If it finds nothing it says exactly where it looked, rather than
//! reporting a failure that leaves you guessing.

use std::path::{Path, PathBuf};

/// The extensions this project publishes releases as.
const ARCHIVES: &[&str] = &[".zip", ".tar.gz", ".tgz", ".tar.xz"];

/// The names of the two files a signed release carries beside its archives.
pub const SUMS: &str = "SHA256SUMS";
/// The detached signature over [`SUMS`].
pub const SUMS_SIG: &str = "SHA256SUMS.asc";
/// The list of what is inside each archive, itself covered by [`SUMS`].
///
/// **Marker 97.** Optional, and its absence is not a failure: releases before
/// v0.1.15 do not carry one, and a verifier that refused them would be refusing
/// files it can check perfectly well.
pub const CONTENTS: &str = veilvoice_check::contents::CONTENTS;

/// What was found in one directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The directory looked in.
    pub directory: PathBuf,
    /// Release archives, sorted by name so two runs agree.
    pub archives: Vec<PathBuf>,
    /// The hash list, if it is there.
    pub sums: Option<PathBuf>,
    /// The signature over the hash list, if it is there.
    pub signature: Option<PathBuf>,
    /// The list of what is inside each archive, if the release published one.
    pub contents: Option<PathBuf>,
}

impl Found {
    /// Whether this directory holds everything needed to verify offline.
    pub fn is_complete(&self) -> bool {
        !self.archives.is_empty() && self.sums.is_some() && self.signature.is_some()
    }

    /// Whether anything at all turned up.
    pub fn is_empty(&self) -> bool {
        self.archives.is_empty() && self.sums.is_none() && self.signature.is_none()
    }

    /// What is missing, in words, for a message to the user.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut gaps = Vec::new();
        if self.archives.is_empty() {
            gaps.push("no release archive");
        }
        if self.sums.is_none() {
            gaps.push("no SHA256SUMS");
        }
        if self.signature.is_none() {
            gaps.push("no SHA256SUMS.asc");
        }
        gaps
    }
}

/// Whether a filename looks like one of this project's release archives.
///
/// A filename test, and therefore evidence of nothing. See the module note.
pub fn looks_like_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("veilvoice-") && ARCHIVES.iter().any(|end| lower.ends_with(end))
}

/// Look in one directory, one level deep.
pub fn look_in(directory: &Path) -> Found {
    let mut found = Found {
        directory: directory.to_path_buf(),
        archives: Vec::new(),
        sums: None,
        signature: None,
        contents: None,
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(SUMS) {
            found.sums = Some(path);
        } else if name.eq_ignore_ascii_case(SUMS_SIG) {
            found.signature = Some(path);
        } else if name.eq_ignore_ascii_case(CONTENTS) {
            found.contents = Some(path);
        } else if looks_like_archive(name) {
            found.archives.push(path);
        }
    }
    // Sorted so two runs over the same directory report in the same order, and
    // so a listing shown to a user is stable between them.
    found.archives.sort();
    found
}

/// Every place worth looking, in order, without duplicates.
pub fn places(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    let mut add = |path: Option<PathBuf>| {
        if let Some(path) = path {
            if !places.contains(&path) {
                places.push(path);
            }
        }
    };

    add(explicit.map(Path::to_path_buf));
    add(std::env::current_dir().ok());
    // Where somebody who unpacked the archive and ran the verifier inside it
    // will be. `current_exe` can fail on a deleted or moved binary, which is
    // not worth an error -- it simply means one fewer place to look.
    add(std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf)));

    for home in [
        std::env::var_os("USERPROFILE").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    {
        add(Some(home.join("Downloads")));
        add(Some(home.join("Desktop")));
    }
    places
}

/// Look everywhere worth looking and return the first directory that holds a
/// complete, checkable set, or, failing that, everything that turned up.
///
/// "Complete" means an archive, a hash list and a signature in one place, which
/// is what an offline check needs. A directory with an archive and no hash list
/// is reported rather than used: it is exactly the situation where somebody
/// needs to be told what else to download, and silently reaching for a hash
/// list from a *different* directory would be checking one release against
/// another's list.
pub fn search(explicit: Option<&Path>) -> (Option<Found>, Vec<Found>) {
    let mut all = Vec::new();
    let mut complete = None;
    for place in places(explicit) {
        let found = look_in(&place);
        if found.is_empty() {
            continue;
        }
        if complete.is_none() && found.is_complete() {
            complete = Some(found.clone());
        }
        all.push(found);
    }
    (complete, all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn the_published_archive_names_are_recognised() {
        for name in [
            "veilvoice-v0.1.12-windows-x86_64.zip",
            "veilvoice-v0.1.12-linux-x86_64.tar.gz",
            "veilvoice-v0.1.11-openbsd-x86_64.tar.gz",
            "VeilVoice-v0.1.12-macos-aarch64.tar.xz",
        ] {
            assert!(looks_like_archive(name), "{name}");
        }
    }

    #[test]
    fn anything_else_is_not_a_candidate() {
        for name in [
            "",
            "notveilvoice.zip",
            "veilvoice.txt",
            "veilvoice-v0.1.12.exe",
            "SHA256SUMS",
            "readme-veilvoice-v1.zip",
        ] {
            assert!(!looks_like_archive(name), "{name}");
        }
    }

    #[test]
    fn a_complete_directory_is_recognised() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "veilvoice-v0.1.12-windows-x86_64.zip");
        touch(dir.path(), SUMS);
        touch(dir.path(), SUMS_SIG);
        let found = look_in(dir.path());
        assert!(found.is_complete());
        assert!(!found.is_empty());
        assert!(found.missing().is_empty());
        assert_eq!(found.archives.len(), 1);
    }

    /// The case somebody actually hits: the archive downloaded, the hash list
    /// forgotten. They need to be told which, not left with a failure.
    #[test]
    fn a_directory_missing_the_hash_list_says_which_part_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "veilvoice-v0.1.12-windows-x86_64.zip");
        let found = look_in(dir.path());
        assert!(!found.is_complete());
        assert!(!found.is_empty());
        let missing = found.missing();
        assert!(missing.contains(&"no SHA256SUMS"), "{missing:?}");
        assert!(missing.contains(&"no SHA256SUMS.asc"), "{missing:?}");
        assert!(!missing.contains(&"no release archive"), "{missing:?}");
    }

    #[test]
    fn several_archives_come_back_in_a_stable_order() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "veilvoice-v0.1.12-windows-x86_64.zip");
        touch(dir.path(), "veilvoice-v0.1.12-linux-x86_64.tar.gz");
        touch(dir.path(), "veilvoice-v0.1.12-macos-aarch64.tar.gz");
        let first = look_in(dir.path());
        let second = look_in(dir.path());
        assert_eq!(first.archives.len(), 3);
        assert_eq!(first.archives, second.archives);
    }

    #[test]
    fn an_empty_or_missing_directory_finds_nothing_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(look_in(dir.path()).is_empty());
        assert!(look_in(&dir.path().join("not-here")).is_empty());
    }

    /// A directory of unrelated files must not produce candidates.
    #[test]
    fn unrelated_files_are_not_candidates() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "holiday.zip");
        touch(dir.path(), "notes.txt");
        assert!(look_in(dir.path()).is_empty());
    }

    /// A directory is only ever searched one level deep. A recursive walk of a
    /// home directory is slow, surprising, and lets a verifier wander into
    /// places nobody asked about.
    #[test]
    fn the_search_does_not_descend() {
        let dir = tempfile::tempdir().unwrap();
        let deeper = dir.path().join("deeper");
        std::fs::create_dir_all(&deeper).unwrap();
        touch(&deeper, "veilvoice-v0.1.12-windows-x86_64.zip");
        assert!(look_in(dir.path()).is_empty());
    }

    #[test]
    fn the_places_list_starts_where_it_was_told_and_has_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(Some(dir.path()));
        assert_eq!(places[0], dir.path());
        let mut sorted = places.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), places.len(), "a place is listed twice");
    }

    /// The whole point of the search: it must find a complete set without
    /// being told where anything is.
    #[test]
    fn a_complete_set_is_found_when_it_is_pointed_at() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "veilvoice-v0.1.12-windows-x86_64.zip");
        touch(dir.path(), SUMS);
        touch(dir.path(), SUMS_SIG);
        let (complete, all) = search(Some(dir.path()));
        let complete = complete.expect("the set should have been found");
        assert_eq!(complete.directory, dir.path());
        assert!(!all.is_empty());
    }

    /// An incomplete directory must be reported rather than silently paired
    /// with a hash list from somewhere else -- which would be checking one
    /// release against another's list.
    #[test]
    fn an_incomplete_directory_is_reported_and_not_completed_from_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "veilvoice-v0.1.12-windows-x86_64.zip");
        let (complete, all) = search(Some(dir.path()));
        let ours = all
            .iter()
            .find(|found| found.directory == dir.path())
            .expect("the directory should be reported");
        assert!(!ours.is_complete());
        // `complete` may be Some from a real Downloads directory on the machine
        // this runs on, but it must never be *this* directory.
        if let Some(complete) = complete {
            assert_ne!(complete.directory, dir.path());
        }
    }

    /// Searching the real machine must not panic whatever is on it.
    #[test]
    fn searching_this_machine_does_not_panic() {
        let (_, all) = search(None);
        for found in all {
            assert!(found.directory.is_absolute() || found.directory.exists());
        }
    }
}
