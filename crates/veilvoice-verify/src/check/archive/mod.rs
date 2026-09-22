// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 164.** Checking every file *inside* a release archive,
//! against the signed contents list, without unpacking anything.
//!
//! # The gap this closes
//!
//! [`crate::check::contents`] answers "is the program in my folder the one
//! that was published" by hashing the files on disk. That is the right answer
//! for somebody who has already extracted the archive, and it is no answer at
//! all for somebody who has not: until they unpack it, there is nothing to
//! hash, and the only thing checkable is the archive as one opaque blob.
//!
//! So the archive is opened and its members are hashed where they sit. The
//! chain is the same one, one link longer at the end:
//!
//! ```text
//! SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file inside the archive
//! ```
//!
//! And it is worth more than the on-disk check rather than less. A folder on
//! disk is whatever the unpacking tool produced and whatever has happened to
//! it since; the members of the archive are what was actually signed. Somebody
//! can now be told the download is sound **before** they extract it, which is
//! the order in which they would like to know.
//!
//! # Nothing is ever written
//!
//! No member is extracted, no path is joined onto a directory, no mode is set
//! and no link is followed. A member's path is a string that gets compared
//! against the signed manifest and then dropped. That is why the usual list of
//! archive-unpacking defects -- a member called `../../etc/profile`, a link
//! pointing out of the tree, a name that is a Windows device -- has no
//! purchase here: this never unpacks, so there is nothing for such a name to
//! reach.
//!
//! The path rules are still applied, in [`crate::check::contents::parse`],
//! because the *manifest* decides what gets compared and it is refused rather
//! than sanitised if it holds a path like that.
//!
//! # What it still does not prove
//!
//! Exactly what [`crate::check`] says, and no more: that these are the files
//! the holder of the signing key published. Not that they are safe, and not
//! that they were built from the source you can read.
//!
//! # In plain words
//!
//! Checks every file inside a downloaded zip or tar.gz against the signed list
//! of what should be in it, without unpacking it first.
//!
//! If anything inside has been changed, added or removed, this says which file
//! and stops you running it.

pub mod tar;
pub mod zip;

use std::collections::BTreeMap;
use std::path::Path;

use crate::check::contents::ArchiveContents;
use crate::check::Error;

/// The archive formats this project publishes.
///
/// Named as a list rather than tested for one at a time, so `.tar.xz` -- which
/// is published and which this cannot read -- is a case with a sentence
/// attached rather than a silent fall-through to "not an archive".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A gzipped tar: every platform but Windows.
    TarGz,
    /// A zip: Windows.
    Zip,
    /// An archive this project publishes and this reader cannot open.
    Unreadable(&'static str),
    /// Not a release archive at all, by its name.
    Unknown,
}

/// Which format a file name says it is.
///
/// A **filename** test, and therefore evidence of nothing, exactly as
/// [`crate::discover::looks_like_archive`] says of its own. It decides which
/// reader to try, never whether anything passes.
pub fn kind_of(name: &str) -> Kind {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Kind::TarGz
    } else if lower.ends_with(".zip") {
        Kind::Zip
    } else if lower.ends_with(".tar.xz") {
        // Published for the platforms whose packagers expect it. Reading it
        // would mean an xz decoder in the one binary whose smallness is a
        // feature, for a format the same release also ships as `.tar.gz`. The
        // honest answer is to say which file to fetch instead.
        Kind::Unreadable(
            "this verifier cannot read .tar.xz. The same release publishes a \
             .tar.gz of the same files, which it can.",
        )
    } else {
        Kind::Unknown
    }
}

/// One file found inside an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inside {
    /// Where it sits inside the archive.
    pub path: String,
    /// Its SHA-256, lowercase hex, or `None` for something that is not a file.
    pub digest: Option<String>,
    /// What it is, when it is not an ordinary file.
    pub what: Option<&'static str>,
}

/// Every member of an archive, hashed.
///
/// The archive is read from disk and nothing is written anywhere. A member
/// that is not an ordinary file is returned with no digest and a word for what
/// it is, rather than dropped: the contents list holds only regular files, so
/// a link at a published path is a finding and not an absence.
pub fn members(archive: &Path) -> Result<Vec<Inside>, Error> {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match kind_of(&name) {
        Kind::Unreadable(why) => Err(Error::Malformed(why.to_string())),
        Kind::Unknown => Err(Error::Malformed(format!(
            "{name} is not a name this project publishes a release under"
        ))),
        Kind::TarGz => {
            let file = std::fs::File::open(archive)
                .map_err(|e| Error::Io(format!("cannot open {}: {e}", archive.display())))?;
            let stream = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
            Ok(tar::members(stream)?
                .into_iter()
                .map(|member| match member {
                    tar::Member::File { path, digest } => Inside {
                        path,
                        digest: Some(digest),
                        what: None,
                    },
                    tar::Member::NotAFile { path, what } => Inside {
                        path,
                        digest: None,
                        what: Some(what),
                    },
                })
                .collect())
        }
        Kind::Zip => {
            let mut file = std::fs::File::open(archive)
                .map_err(|e| Error::Io(format!("cannot open {}: {e}", archive.display())))?;
            Ok(zip::members(&mut file)?
                .into_iter()
                .map(|member| match member {
                    zip::Member::File { path, digest } => Inside {
                        path,
                        digest: Some(digest),
                        what: None,
                    },
                    zip::Member::NotAFile { path, what } => Inside {
                        path,
                        digest: None,
                        what: Some(what),
                    },
                })
                .collect())
        }
    }
}

/// What checking one published file against the archive found.
///
/// The same shape as [`crate::check::contents::Verdict`], for the same
/// findings in the other place. Kept as its own type rather than shared,
/// because two of the disk verdicts -- unreadable, and a symbolic link on
/// disk -- cannot arise inside an archive that is never unpacked, and a reader
/// of this code should not have to work out which cases are dead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It is in the archive and its hash is the published one.
    Matches,
    /// It is in the archive and it is not the published one.
    Differs {
        /// What it actually hashes to.
        found: String,
    },
    /// The list publishes it and the archive does not hold it.
    Missing,
    /// Something is in the archive under that name and it is not a plain file.
    NotAFile(&'static str),
}

/// One published file, checked against the archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The path as the manifest gives it.
    pub path: String,
    /// What was found.
    pub verdict: Verdict,
}

impl Outcome {
    /// Whether this one is as published.
    pub fn is_good(&self) -> bool {
        self.verdict == Verdict::Matches
    }
}

/// What comparing a whole archive against its section of the list found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comparison {
    /// One per file the release published, in the order the list gives them.
    pub outcomes: Vec<Outcome>,
    /// Files the archive holds that the release never published.
    ///
    /// A signed list says what is inside; a file inside that is not on it is
    /// as much a difference as one that is missing, and the more interesting
    /// of the two. This is the inside-the-archive twin of
    /// [`crate::check::contents::Sweep::extras`], and it cannot have that
    /// one's blind spot: a directory that could not be read has no equivalent
    /// here, because the archive's own index lists everything it holds.
    pub extras: Vec<String>,
}

impl Comparison {
    /// How many files are exactly as published.
    pub fn as_published(&self) -> usize {
        self.outcomes.iter().filter(|o| o.is_good()).count()
    }

    /// How many things did not check out, extras counted among them.
    pub fn wrong(&self) -> usize {
        self.outcomes.len() - self.as_published() + self.extras.len()
    }

    /// Whether the archive holds exactly what was published and nothing else.
    pub fn is_clean(&self) -> bool {
        self.wrong() == 0
    }
}

/// Compare what is inside an archive against the section of the list that
/// covers it.
///
/// Both directions, and that is the point of it: every published file is
/// looked for, and every file found is checked against the published set. A
/// comparison that only walked the list would pass an archive with an extra
/// program added to it.
pub fn compare(inside: &[Inside], section: &ArchiveContents) -> Comparison {
    let found: BTreeMap<&str, &Inside> = inside
        .iter()
        .map(|member| (member.path.as_str(), member))
        .collect();

    let outcomes = section
        .members
        .iter()
        .map(|member| {
            let verdict = match found.get(member.path.as_str()) {
                None => Verdict::Missing,
                Some(entry) => match (&entry.digest, entry.what) {
                    (Some(digest), _) if crate::check::digests_match(digest, &member.digest) => {
                        Verdict::Matches
                    }
                    (Some(digest), _) => Verdict::Differs {
                        found: digest.clone(),
                    },
                    (None, Some(what)) => Verdict::NotAFile(what),
                    // No digest and no word for why. Unreachable as the two
                    // readers are written, and treated as a failure rather
                    // than a pass if it ever becomes reachable: an unchecked
                    // file is not a checked one.
                    (None, None) => Verdict::NotAFile("not an ordinary file"),
                },
            };
            Outcome {
                path: member.path.clone(),
                verdict,
            }
        })
        .collect();

    let published: BTreeMap<&str, ()> = section
        .members
        .iter()
        .map(|member| (member.path.as_str(), ()))
        .collect();
    let extras = inside
        .iter()
        .filter(|member| !published.contains_key(member.path.as_str()))
        .map(|member| member.path.clone())
        .collect();

    Comparison { outcomes, extras }
}
