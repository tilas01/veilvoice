// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 164.** Every copy of VeilVoice on this computer, found and
//! checked against what was published.
//!
//! # The question this answers
//!
//! Every other command here is about one download: you have a file, and it
//! either is or is not the published one. That is the right question at the
//! moment of downloading and the wrong one a year later, when there is an
//! installed copy, the folder it was unpacked from, an older release still in
//! Downloads, and one on a memory stick, and no way to say which of them is
//! the one a reader should trust.
//!
//! So: find them all, say where each came from, and check each against the
//! published hashes for the version it claims to be.
//!
//! # The version a copy claims is read from where it sits, never from running it
//!
//! It is tempting to ask each binary its version. It is also the one thing a
//! program in this position must not do. The whole question is whether that
//! binary is the published one, and running it to find out is asking the
//! suspect: a tampered copy will say whatever makes it look right, and the
//! answer arrives after it has already run.
//!
//! So nothing here executes anything it finds. A version comes from the
//! release directory that holds it -- `veilvoice-v0.1.22-linux-x86_64` names
//! its own version, and every archive this project publishes unpacks into such
//! a directory -- or it is not known, and a copy whose version is not known is
//! reported as exactly that.
//!
//! # What it is checked against, and why the answer usually exists
//!
//! Not a list fetched from anywhere: this is entirely offline, like everything
//! else here except `release`. It is checked against the signed contents lists
//! already on this machine.
//!
//! Every `CONTENTS.sha256` found is verified first, the ordinary way -- the
//! detached signature over its `SHA256SUMS`, against the key compiled into
//! this binary, and then the contents list against that hash list -- and only
//! then read. Together they give a set of published hashes, each one attached
//! to a version and a name inside a release.
//!
//! A copy is then looked up **by its own hash**, which is what makes this work
//! for an installed copy that carries no version anywhere in its path. If its
//! digest is in a verified list, the answer is not "it matches" but "this is
//! the `veilvoice` published in v0.1.22", which is a stronger and more useful
//! thing to be told.
//!
//! # Saying plainly what could not be checked
//!
//! A copy whose hash is in no verified list, of a version no list on this
//! machine covers, has not been found wanting. It has not been checked. Those
//! are different facts and this module keeps them apart in the type, because
//! every other part of this program does and because reporting the second as
//! the first is the one failure a verifier cannot come back from.
//!
//! # In plain words
//!
//! Finds every copy of VeilVoice on this computer, installed or just unpacked,
//! old or current, and tells you which of them are exactly what was published.
//!
//! It never runs any of them to ask. A copy that has been tampered with would
//! lie, and by then it would have run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::check::contents::ArchiveContents;
use crate::extracted::PROGRAMS;

/// How a copy came to be where it is.
///
/// Reported because it changes what a reader should do about a bad one. A
/// changed file in an old download is rubbish to delete; a changed file in the
/// installed copy is the program that runs when they type `veilvoice`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Where {
    /// In the directory an installation puts programs.
    Installed,
    /// On this process's `PATH`, so a shell would find it.
    OnPath,
    /// Beside the program that is running now.
    Beside,
    /// In a directory an archive unpacks into.
    Unpacked,
    /// Somewhere else that was looked in, such as Downloads.
    Loose,
}

impl Where {
    /// The words a report uses for it.
    pub fn plainly(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::OnPath => "on your PATH",
            Self::Beside => "beside the running program",
            Self::Unpacked => "unpacked from an archive",
            Self::Loose => "found loose",
        }
    }
}

/// One copy of a VeilVoice program found on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copy {
    /// Where the file is.
    pub path: PathBuf,
    /// How it came to be there.
    pub found: Where,
    /// The version the directory holding it names, if one does.
    ///
    /// `None` is not a failure and not a suspicion. An installed copy sits in
    /// a directory that says nothing about versions, which is ordinary.
    pub claims: Option<String>,
}

/// What checking one copy against the published lists found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Its hash is one a verified list publishes, and this is what it is.
    Published {
        /// The release the hash was published in, as the archive names it.
        release: String,
        /// The path it has inside that release.
        inside: String,
    },
    /// A list covering the version it claims exists, and it is not in it.
    ///
    /// The only verdict here that is an accusation, and it is narrow on
    /// purpose: it requires a verified list for the version this copy says it
    /// is, and that list not to publish these bytes under this name.
    Changed {
        /// The version whose list was consulted.
        release: String,
        /// What the file on disk actually hashes to.
        found: String,
        /// What that release published for this name, if it published one.
        expected: Option<String>,
    },
    /// No verified list on this machine covers it.
    ///
    /// Nothing has been proven either way. See the module note: this is not a
    /// quieter kind of failure, it is the absence of a check.
    NoSignedList,
    /// It is there and could not be read.
    Unreadable(String),
}

impl Verdict {
    /// Whether this copy was checked and is what was published.
    pub fn is_good(&self) -> bool {
        matches!(self, Self::Published { .. })
    }

    /// Whether this copy was checked at all.
    ///
    /// Kept apart from [`Self::is_good`] because a caller counting "how many
    /// are fine" and a caller counting "how many do I know about" are asking
    /// different questions, and a single boolean would answer whichever one
    /// the caller happened to assume.
    pub fn was_checked(&self) -> bool {
        matches!(self, Self::Published { .. } | Self::Changed { .. })
    }
}

/// One copy, and what was found out about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The copy.
    pub copy: Copy,
    /// Its SHA-256, when it could be read.
    pub digest: Option<String>,
    /// What the published lists say about it.
    pub verdict: Verdict,
}

/// The version a release directory's name carries.
///
/// `veilvoice-v0.1.22-linux-x86_64` yields `v0.1.22`. Returns `None` rather
/// than a guess for a directory named anything else, because a wrong version
/// here would send a copy to be compared against another release's hashes and
/// reported as changed.
pub fn version_in(directory: &Path) -> Option<String> {
    let name = directory.file_name()?.to_str()?;
    let rest = name.strip_prefix("veilvoice-")?;
    let version = rest.split('-').next()?;
    // A version, not merely the first field: `veilvoice-signing-key` would
    // otherwise claim to be version "signing".
    let digits = version.strip_prefix('v').unwrap_or(version);
    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(version.to_string())
}

/// Every place worth looking for a copy, in order, without duplicates.
///
/// Bounded and named, for the reason [`crate::discover`] gives at length: a
/// verifier that walks a whole home directory looking for something to check
/// is a worse thing than one that asks where to look. Each directory is read
/// one level deep, plus the release directories directly inside it.
pub fn places() -> Vec<(PathBuf, Where)> {
    let mut places: Vec<(PathBuf, Where)> = Vec::new();
    let mut add = |path: Option<PathBuf>, how: Where| {
        if let Some(path) = path {
            if !places.iter().any(|(known, _)| *known == path) {
                places.push((path, how));
            }
        }
    };

    add(veilvoice_setup::install::bin_dir(), Where::Installed);
    add(veilvoice_setup::install::prefix(), Where::Installed);

    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            add(Some(entry), Where::OnPath);
        }
    }

    add(
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf)),
        Where::Beside,
    );
    add(std::env::current_dir().ok(), Where::Loose);

    for home in [
        std::env::var_os("USERPROFILE").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    {
        add(Some(home.join("Downloads")), Where::Loose);
        add(Some(home.join("Desktop")), Where::Loose);
        add(Some(home.join("Applications")), Where::Loose);
        add(Some(home), Where::Loose);
    }
    places
}

/// Find every copy of a VeilVoice program on this machine.
///
/// Nothing found here is executed, and nothing is written. Sorted by path so
/// two runs over an unchanged machine report the same thing in the same order,
/// which is what makes the remembered record worth comparing against.
pub fn find() -> Vec<Copy> {
    let mut copies: Vec<Copy> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    for (directory, how) in places() {
        look(&directory, how, None, &mut copies, &mut seen);
        // And one level into the release directories inside it, which is where
        // an archive unpacks to and where most copies on a machine actually
        // are. Only directories whose names say they are releases: a verifier
        // that descends into everything is the wandering this avoids.
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(version) = version_in(&path) else {
                continue;
            };
            look(
                &path,
                Where::Unpacked,
                Some(version),
                &mut copies,
                &mut seen,
            );
        }
    }
    copies.sort_by(|a, b| a.path.cmp(&b.path));
    copies
}

/// Look in one directory for the programs a release carries.
fn look(
    directory: &Path,
    how: Where,
    claims: Option<String>,
    into: &mut Vec<Copy>,
    seen: &mut Vec<PathBuf>,
) {
    for program in PROGRAMS {
        for name in [program.to_string(), format!("{program}.exe")] {
            let path = directory.join(&name);
            // `symlink_metadata`, not `metadata`: a link is a name somebody
            // else may be able to repoint, and hashing what it points at today
            // would report on a file this one need not be tomorrow. The same
            // rule `contents::check` follows on disk.
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            // The same file reachable two ways is one copy. Resolved where the
            // system can, compared by path where it cannot.
            let identity = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.contains(&identity) {
                continue;
            }
            seen.push(identity);
            into.push(Copy {
                claims: claims.clone().or_else(|| version_in(directory)),
                path,
                found: how,
            });
            break;
        }
    }
}

/// What the signed lists on this machine publish, gathered in one place.
///
/// Built only from lists that have been verified: the signature over
/// `SHA256SUMS` against the embedded key, then `CONTENTS.sha256` against that
/// hash list. An unverified contents list is a text file somebody sent you,
/// and building this from one would make every answer below worthless.
#[derive(Clone, Debug, Default)]
pub struct Published {
    /// Hash to the release and the path it was published under.
    by_digest: BTreeMap<String, (String, String)>,
    /// Which releases are covered at all, so "no list for that version" can be
    /// told from "changed".
    releases: BTreeMap<String, Vec<ArchiveContents>>,
}

impl Published {
    /// Whether any signed list at all was found.
    pub fn is_empty(&self) -> bool {
        self.by_digest.is_empty()
    }

    /// How many releases are covered.
    pub fn releases(&self) -> usize {
        self.releases.len()
    }

    /// The versions covered, for a report that says what it could consult.
    pub fn versions(&self) -> Vec<String> {
        self.releases.keys().cloned().collect()
    }

    /// Add one verified contents list.
    ///
    /// **Only call this with a list that has already been checked against its
    /// signed hash list.** The whole value of this structure is that every
    /// hash in it came through the signature.
    pub fn add_verified(&mut self, all: Vec<ArchiveContents>) {
        for archive in &all {
            let Some(version) = version_of_archive(&archive.archive) else {
                continue;
            };
            for member in &archive.members {
                self.by_digest
                    .entry(member.digest.to_ascii_lowercase())
                    .or_insert((version.clone(), member.path.clone()));
            }
            self.releases
                .entry(version)
                .or_default()
                .push(archive.clone());
        }
    }

    /// What a digest was published as, if anything.
    fn what_is(&self, digest: &str) -> Option<&(String, String)> {
        self.by_digest.get(&digest.to_ascii_lowercase())
    }

    /// The hash a release published for a file of this name, if it published
    /// one at all.
    fn expected_for(&self, release: &str, name: &str) -> Option<String> {
        let archives = self.releases.get(release)?;
        for archive in archives {
            for member in &archive.members {
                if member.path.rsplit('/').next() == Some(name) {
                    return Some(member.digest.clone());
                }
            }
        }
        None
    }
}

/// The version an archive's own name carries.
fn version_of_archive(archive: &str) -> Option<String> {
    let rest = archive.strip_prefix("veilvoice-")?;
    let version = rest.split('-').next()?;
    let digits = version.strip_prefix('v').unwrap_or(version);
    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(version.to_string())
}

/// Check one copy against what the verified lists publish.
pub fn examine(copy: &Copy, published: &Published) -> Checked {
    let digest = match crate::check::sha256_file(&copy.path) {
        Ok(digest) => digest,
        Err(why) => {
            return Checked {
                copy: copy.clone(),
                digest: None,
                verdict: Verdict::Unreadable(why.to_string()),
            }
        }
    };

    // By hash first, and deliberately so. It is the question that has an
    // answer for an installed copy, whose path says nothing about versions,
    // and the answer it gives -- "this is the veilvoice published in v0.1.22"
    // -- is more than a comparison against one expected value would be.
    if let Some((release, inside)) = published.what_is(&digest) {
        return Checked {
            copy: copy.clone(),
            digest: Some(digest),
            verdict: Verdict::Published {
                release: release.clone(),
                inside: inside.clone(),
            },
        };
    }

    // Not published anywhere this machine can verify. Whether that is a
    // finding depends entirely on whether a list for its version was
    // available, which is the distinction the rest of this function is.
    let name = copy
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let verdict = match &copy.claims {
        Some(release) if published.releases.contains_key(release) => Verdict::Changed {
            release: release.clone(),
            found: digest.clone(),
            expected: published.expected_for(release, &name),
        },
        _ => Verdict::NoSignedList,
    };
    Checked {
        copy: copy.clone(),
        digest: Some(digest),
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::contents::Member;

    /// A release directory names its own version, and nothing else does.
    #[test]
    fn a_release_directory_names_the_version_it_holds() {
        for (name, expected) in [
            ("veilvoice-v0.1.22-linux-x86_64", Some("v0.1.22")),
            ("veilvoice-v0.1.23-windows-x86_64", Some("v0.1.23")),
            ("veilvoice-0.1.14-macos-aarch64", Some("0.1.14")),
        ] {
            assert_eq!(version_in(Path::new(name)).as_deref(), expected, "{name}");
        }
        // Nothing that is not a version. A wrong answer here sends a copy to
        // be compared against another release's hashes and reported as
        // changed, which is an accusation made by a parsing mistake.
        for name in [
            "veilvoice-signing-key",
            "veilvoice",
            "veilvoice-",
            "Downloads",
            "veilvoice-gui",
        ] {
            assert_eq!(version_in(Path::new(name)), None, "{name}");
        }
    }

    /// A release with one file in it, for the tests below.
    fn published_with(archive: &str, path: &str, digest: &str) -> Published {
        let mut published = Published::default();
        published.add_verified(vec![ArchiveContents {
            archive: archive.to_string(),
            members: vec![Member {
                path: path.to_string(),
                digest: digest.to_string(),
            }],
        }]);
        published
    }

    /// A copy on disk, hashed for real.
    fn copy_of(dir: &Path, name: &str, bytes: &[u8], claims: Option<&str>) -> (Copy, String) {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let digest = crate::check::sha256_bytes(bytes);
        (
            Copy {
                path,
                found: Where::Installed,
                claims: claims.map(str::to_string),
            },
            digest,
        )
    }

    /// A copy whose hash is published is named as what it is.
    ///
    /// Looked up by hash rather than by path, which is the whole reason this
    /// works for an installed copy: `~/.local/bin/veilvoice` carries no
    /// version anywhere, and the answer is still the release it came from.
    #[test]
    fn an_installed_copy_with_no_version_in_its_path_is_still_identified() {
        let dir = tempfile::tempdir().unwrap();
        let (copy, digest) = copy_of(dir.path(), "veilvoice", b"the published one", None);
        let published = published_with(
            "veilvoice-v0.1.22-linux-x86_64.tar.gz",
            "veilvoice-v0.1.22-linux-x86_64/veilvoice",
            &digest,
        );

        let checked = examine(&copy, &published);
        assert_eq!(checked.digest.as_deref(), Some(digest.as_str()));
        match &checked.verdict {
            Verdict::Published { release, inside } => {
                assert_eq!(release, "v0.1.22");
                assert_eq!(inside, "veilvoice-v0.1.22-linux-x86_64/veilvoice");
            }
            other => panic!("expected Published, got {other:?}"),
        }
        assert!(checked.verdict.is_good() && checked.verdict.was_checked());
    }

    /// A copy claiming a version whose list is here, and not in it, is named.
    ///
    /// The one verdict here that is an accusation, and it is reached only when
    /// a verified list for that copy's own claimed version exists and does not
    /// publish these bytes.
    #[test]
    fn a_copy_that_is_not_in_the_list_for_the_version_it_claims_is_a_finding() {
        let dir = tempfile::tempdir().unwrap();
        let (copy, digest) = copy_of(
            dir.path(),
            "veilvoice",
            b"something else entirely",
            Some("v0.1.22"),
        );
        let published = published_with(
            "veilvoice-v0.1.22-linux-x86_64.tar.gz",
            "veilvoice-v0.1.22-linux-x86_64/veilvoice",
            &crate::check::sha256_bytes(b"the published one"),
        );

        match examine(&copy, &published).verdict {
            Verdict::Changed {
                release,
                found,
                expected,
            } => {
                assert_eq!(release, "v0.1.22");
                assert_eq!(found, digest);
                assert_eq!(
                    expected,
                    Some(crate::check::sha256_bytes(b"the published one")),
                    "the report names what that release did publish under this name"
                );
            }
            other => panic!("expected Changed, got {other:?}"),
        }
    }

    /// A copy no list here covers has not been found wanting.
    ///
    /// The distinction the whole module is built around. A verifier that
    /// reported this as a failure would tell somebody their old download had
    /// been tampered with, when the truth is that nothing was compared.
    #[test]
    fn a_copy_no_signed_list_covers_is_unchecked_and_not_condemned() {
        let dir = tempfile::tempdir().unwrap();
        let published = published_with(
            "veilvoice-v0.1.22-linux-x86_64.tar.gz",
            "veilvoice-v0.1.22-linux-x86_64/veilvoice",
            &crate::check::sha256_bytes(b"the published one"),
        );

        // A version this machine has no list for.
        let (older, _) = copy_of(dir.path(), "veilvoice", b"an old build", Some("v0.1.9"));
        assert_eq!(examine(&older, &published).verdict, Verdict::NoSignedList);

        // And one that says nothing about its version at all.
        let elsewhere = tempfile::tempdir().unwrap();
        let (nameless, _) = copy_of(elsewhere.path(), "veilvoice-gui", b"who knows", None);
        assert_eq!(
            examine(&nameless, &published).verdict,
            Verdict::NoSignedList
        );

        for verdict in [
            examine(&older, &published).verdict,
            examine(&nameless, &published).verdict,
        ] {
            assert!(!verdict.is_good(), "it is not a pass");
            assert!(!verdict.was_checked(), "and it is not a failure either");
        }
    }

    /// With no lists at all, nothing is a finding.
    #[test]
    fn a_machine_with_no_signed_lists_condemns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let published = Published::default();
        assert!(published.is_empty());
        let (copy, _) = copy_of(dir.path(), "veilvoice", b"anything", Some("v0.1.22"));
        assert_eq!(examine(&copy, &published).verdict, Verdict::NoSignedList);
    }

    /// A file that cannot be read is reported as unreadable, not as changed.
    #[test]
    fn a_copy_that_cannot_be_read_is_not_reported_as_changed() {
        let dir = tempfile::tempdir().unwrap();
        let copy = Copy {
            path: dir.path().join("veilvoice-that-is-not-there"),
            found: Where::Loose,
            claims: Some("v0.1.22".to_string()),
        };
        let checked = examine(&copy, &Published::default());
        assert!(matches!(checked.verdict, Verdict::Unreadable(_)));
        assert!(checked.digest.is_none());
        assert!(!checked.verdict.was_checked());
    }

    /// Only lists whose archive names a version contribute.
    ///
    /// A contents list naming an archive this parser cannot place is skipped
    /// rather than filed under a guessed version, for the same reason
    /// `version_in` refuses to guess.
    #[test]
    fn a_list_whose_archive_names_no_version_is_not_filed_under_a_guess() {
        let mut published = Published::default();
        published.add_verified(vec![ArchiveContents {
            archive: "something-else.tar.gz".to_string(),
            members: vec![Member {
                path: "whatever/veilvoice".to_string(),
                digest: "aa".to_string(),
            }],
        }]);
        assert!(published.is_empty());
        assert_eq!(published.releases(), 0);
        assert!(published.versions().is_empty());
    }

    /// The places looked in are named, and the list has no duplicates.
    #[test]
    fn every_place_is_looked_in_once() {
        let places = places();
        for (at, (path, _)) in places.iter().enumerate() {
            assert!(
                !places[..at].iter().any(|(earlier, _)| earlier == path),
                "{} is searched twice",
                path.display()
            );
        }
    }
}
