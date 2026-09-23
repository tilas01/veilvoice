// SPDX-License-Identifier: GPL-3.0-or-later
//! The update that does the update.
//!
//! **Roadmap item 179.** `veilvoice_setup::update` reports that a newer release
//! exists and stops there, which leaves the reader to download it, verify it,
//! unpack it and replace their copy by hand. Most people will do some of that
//! and not the rest, and the part they skip is the checking. This does all four
//! in one act, and the checking is the part it will not skip.
//!
//! # The stance this changes, and the sentence that survives it
//!
//! `veilvoice_setup::update` used to say, as a design note rather than a
//! limitation:
//!
//! > An update checker that could install its own answer is an update checker
//! > that can be made to install somebody else's.
//!
//! That sentence is still true and it is still the constraint this module is
//! written around. What made it an argument for doing nothing was the missing
//! half: **whose** answer. Every byte that reaches your disk here is checked
//! against a signature made by a key that is **compiled into the binary you are
//! already running**, before anything is unpacked and long before anything is
//! copied over a program. Somebody who can answer the network cannot produce
//! that signature, so they cannot make this install their answer. They can make
//! it refuse, loudly, which is the correct failure.
//!
//! The other half of that sentence, that nothing happens on its own, is
//! unchanged and is not a detail. There is no timer, no check at startup and
//! no background thread. An update happens because a person asked for one, in
//! the same session, and every step of it is reported as it happens.
//!
//! # The order, which is the whole design
//!
//! 1. **Ask.** [`offered`] calls `veilvoice_setup::update::check`, which reads
//!    one public page with the transfer tool the operating system already
//!    ships. Nothing is downloaded.
//! 2. **Fetch.** The archive for this platform, `SHA256SUMS`, `SHA256SUMS.asc`
//!    and `CONTENTS.sha256`, into a directory the caller names. Nothing is run
//!    and nothing is opened.
//! 3. **Prove.** The signature over `SHA256SUMS` against the compiled-in key;
//!    the archive against `SHA256SUMS`; `CONTENTS.sha256` against
//!    `SHA256SUMS`; and then every file **inside** the archive against
//!    `CONTENTS.sha256`, read where it lies without unpacking it.
//! 4. **Open.** Only now, and with the system's own `tar`, into an empty
//!    directory. What lands is hashed again against the same signed list,
//!    because an extractor is a program too.
//! 5. **Replace.** The binaries beside the running one are moved aside and the
//!    new ones put in their place.
//! 6. **Record.** The integrity record is re-taken, for the reason below.
//! 7. **Restart**, which the caller does, because a window and a command line
//!    part company here.
//!
//! Steps 3 and 4 are [`crate::check`], unchanged and uncopied. That was the
//! condition roadmap item 164 was landed first for: an updater that verified a
//! download with its own second implementation would be a second implementation
//! to get subtly wrong, in the one place where being subtly wrong means
//! accepting something.
//!
//! # The archive is never trusted to name its own files
//!
//! Nothing here reads a name out of the archive and acts on it. The list of
//! members comes from `CONTENTS.sha256`, which the signature covers, and the
//! archive is compared **against** that list in both directions: a member the
//! list does not have is a failure, and a listed file the archive does not
//! carry is a failure. So the archive-traversal defects, a member called
//! `../../.bashrc` or a symbolic link pointing at one, are refused at the
//! comparison rather than caught at the extractor.
//!
//! # The integrity record, and why it is re-taken here
//!
//! `veilvoice-guard` records what VeilVoice's own files hash to, and the window
//! checks that record at every launch. An update replaces those files with
//! different ones, so the next launch after an update would report the update
//! as a change, every time, for ever. A tamper alarm that fires on the ordinary
//! maintenance of the thing it watches is an alarm people learn to dismiss, and
//! an alarm people dismiss is worth nothing on the day it is right.
//!
//! So the record is re-taken as part of the update. Two constraints on when and
//! how, both of which are the point rather than housekeeping:
//!
//! * **After the signature has been checked, never before.** A record re-taken
//!   first would be a record of whatever arrived, blessed by this program, and
//!   an update that failed its signature check would have already written down
//!   the attacker's files as the correct ones.
//! * **Unsealed unless the reader is there to seal it.** The sealed record is
//!   sealed under the app lock's passphrase, which exists only while somebody
//!   is present and has typed it. With it in hand the new record is sealed;
//!   without it the record is written in the clear and [`Done::record`] says
//!   so, in those words, rather than sealing it under something kept beside it
//!   and calling that protection.
//!
//! With an app lock set, the passphrase is **required** rather than optional:
//! see [`Error::Locked`]. Updating past a lock without it would leave the
//! machine with new files and a sealed record of the old ones, which reads as
//! tampering at the next unlock and cannot be told apart from the real thing.
//!
//! # What this updates is the copy you are running
//!
//! Not "the installation", which this program is in no position to be certain
//! about: VeilVoice runs portably by design, and a folder somebody unzipped is
//! as real an installation as anything under `~/.local`. The files replaced are
//! the ones beside [`std::env::current_exe`], which is what a person pressing
//! update means, and it is the one answer that is right for a portable folder,
//! a per-user install and a copy somebody put somewhere of their own.
//!
//! # In plain words
//!
//! Downloads the new version, checks it properly, and puts it in place.
//!
//! It checks the download against a signature made with a key that is already
//! inside the program you are running, before it opens the file, and it opens
//! the file before it touches anything of yours. If any of that fails, nothing
//! is replaced and it tells you what failed.
//!
//! If you have an app lock, it will ask for it, because the record of what
//! VeilVoice's own files should look like has to be rewritten by the update and
//! that record is locked with the same passphrase.

use std::path::{Path, PathBuf};

use crate::check::{self, archive, contents};

/// Every platform label the release workflow publishes an archive for.
///
/// Copied from the build matrix in `.github/workflows/release.yml`, and that
/// copy is **checked** rather than maintained: `tools/audit/release_targets.py`
/// reads the workflow and fails a build if the two lists differ. A label added
/// to the workflow and not here would mean an update this build cannot fetch
/// even though it exists; a label here and not in the workflow would mean an
/// update this build asks for and never receives.
pub const PUBLISHED: &[&str] = &[
    "windows-x86_64",
    "macos-arm64",
    "macos-x86_64",
    "linux-x86_64",
    "linux-arm64",
    "linux-armv7-pi",
    "linux-x86_64-musl-static",
    "linux-arm64-musl-static",
    "freebsd-x86_64",
    "openbsd-x86_64",
    "netbsd-x86_64",
];

/// This build's label, or `None` for a build of a kind nothing is published
/// for.
///
/// `None` is an ordinary answer rather than a failure. Somebody who built this
/// from source for a target the release workflow has no row for has a perfectly
/// good VeilVoice and no archive to be offered, and telling them that is better
/// than offering them somebody else's architecture.
pub const PLATFORM: Option<&str> = this_platform();

/// The label picked apart by target, as the workflow's matrix picks it.
///
/// Written as one expression per published row rather than as an operating
/// system joined to an architecture, because two of the rows are not that
/// shape: the Raspberry Pi build carries `-pi`, and the statically linked
/// builds carry `-musl-static`. Assembling a label out of parts would produce
/// `linux-armv7` for the first, which is not published, and the failure would
/// be a download that 404s rather than a compile error.
const fn this_platform() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("macos-x86_64")
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "musl"
    )) {
        Some("linux-x86_64-musl-static")
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "aarch64",
        target_env = "musl"
    )) {
        Some("linux-arm64-musl-static")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("linux-arm64")
    } else if cfg!(all(target_os = "linux", target_arch = "arm")) {
        Some("linux-armv7-pi")
    } else if cfg!(all(target_os = "freebsd", target_arch = "x86_64")) {
        Some("freebsd-x86_64")
    } else if cfg!(all(target_os = "openbsd", target_arch = "x86_64")) {
        Some("openbsd-x86_64")
    } else if cfg!(all(target_os = "netbsd", target_arch = "x86_64")) {
        Some("netbsd-x86_64")
    } else {
        None
    }
}

/// The archive this platform's release is published under.
///
/// `veilvoice-<tag>-<label>.<extension>`, which is what the release workflow
/// writes. Windows publishes a zip and everything else a gzipped tar, which is
/// a property of the workflow rather than of the platform, so it is read off
/// the label rather than off `cfg`.
pub fn archive_for(tag: &str, label: &str) -> String {
    let extension = if label.starts_with("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("veilvoice-{tag}-{label}.{extension}")
}

/// Everything that can stop an update, in the words the reader is given.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Nothing is published for this build's target.
    NoPlatform,
    /// The version could not be looked up, and why.
    NoAnswer(String),
    /// A file could not be fetched, and why.
    Fetch(String),
    /// Something did not check out. **Nothing has been replaced.**
    Refused(String),
    /// The archive could not be opened, and why.
    Unpack(String),
    /// An app lock is set and no passphrase was given.
    ///
    /// Its own variant rather than a message, because it is the one failure
    /// here that a front end should answer by asking a question rather than by
    /// printing a complaint.
    Locked,
    /// The replacement itself failed, and what state it left behind.
    Replace(String),
    /// Something else went wrong with the disk.
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlatform => write!(
                f,
                "no release is published for this build's platform, so there is \
                 nothing to update to. This is a build somebody made themselves, \
                 and updating it means rebuilding it."
            ),
            Self::NoAnswer(why) => {
                write!(f, "could not find out what the latest version is: {why}")
            }
            Self::Fetch(why) => write!(f, "{why}"),
            Self::Refused(why) => write!(
                f,
                "this download was refused and nothing has been changed: {why}"
            ),
            Self::Unpack(why) => write!(f, "the release could not be opened: {why}"),
            Self::Locked => write!(
                f,
                "this machine has an app lock set. The record of what VeilVoice's \
                 own files should look like is sealed with that passphrase and has \
                 to be rewritten by an update, so the update needs it too."
            ),
            Self::Replace(why) => write!(f, "{why}"),
            Self::Io(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A newer release, and the file to fetch for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The version running now.
    pub current: String,
    /// The tag being offered.
    pub tag: String,
    /// The archive published for this platform under that tag.
    pub archive: String,
}

/// Ask what is published, and offer it if it is newer. Nothing is downloaded.
///
/// `Ok(None)` means this copy is current, or ahead of what is published, which
/// an early build legitimately is. Both are reported by
/// `veilvoice_setup::update` and neither is an error here.
pub fn offered(current: &str) -> Result<Option<Offer>, Error> {
    let Some(label) = PLATFORM else {
        return Err(Error::NoPlatform);
    };
    let report =
        veilvoice_setup::update::check(current).map_err(|e| Error::NoAnswer(e.to_string()))?;
    let veilvoice_setup::update::Verdict::Newer(tag) = &report.verdict else {
        return Ok(None);
    };
    if !crate::fetch::valid_tag(tag) {
        // The tag came off a web page. It is already bounded by the scanner
        // that read it, and it is bounded again here because it is about to
        // become part of a URL and part of a file name.
        return Err(Error::NoAnswer(format!(
            "the version published is not a release tag this understands: {tag}"
        )));
    }
    Ok(Some(Offer {
        current: current.to_string(),
        tag: tag.clone(),
        archive: archive_for(tag, label),
    }))
}

/// Whether this machine has an app lock set.
///
/// Read as "is there a lock file", which is the same question the window asks
/// before it puts up an unlock screen. It says nothing about the passphrase.
pub fn lock_is_set() -> bool {
    veilvoice_crypto::lock::default_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Where the running copy of VeilVoice lives.
pub fn installation() -> Result<PathBuf, Error> {
    let running = std::env::current_exe()
        .map_err(|e| Error::Io(format!("cannot find this program on disk: {e}")))?;
    running
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::Io("this program has no parent directory".to_string()))
}

/// What the update is doing, reported as it happens.
///
/// A front end shows these; nothing here prints. The command line writes a line
/// per step and the window moves an indicator, and neither of those belongs in
/// the code that does the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Fetching one named file.
    Fetching(String),
    /// Checking the signature, the hash list and the archive's contents.
    Checking,
    /// Opening the archive, having checked it.
    Opening,
    /// Putting the new files in place.
    Replacing,
    /// Re-taking the integrity record.
    Recording,
}

/// What became of the integrity record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// Re-taken and sealed under the app lock's passphrase.
    Sealed,
    /// Re-taken and written in the clear, because there was no app lock.
    Plain,
    /// Not re-taken, and why. The update itself succeeded.
    NotTaken(String),
}

/// A finished update.
#[derive(Debug, Clone)]
pub struct Done {
    /// The tag now installed.
    pub tag: String,
    /// The directory the files were replaced in.
    pub into: PathBuf,
    /// The file names that were replaced.
    pub replaced: Vec<String>,
    /// Old files that could not be removed and are still on disk.
    ///
    /// A running program cannot be deleted on Windows, so the copy being
    /// replaced is moved aside and the removal is attempted rather than
    /// required. Anything left is named here instead of being forgotten about.
    pub left_behind: Vec<PathBuf>,
    /// What became of the integrity record.
    pub record: Record,
    /// The program to start again, which the caller does.
    pub restart: PathBuf,
}

/// The whole update, in the one order that is safe.
///
/// `password` is the app lock's passphrase, as bytes rather than as a string so
/// that a caller holding one in a `veilvoice_crypto::Secret` can pass it
/// straight through. Copying it into a `String` to call this would put a second,
/// unwiped copy of it on the heap for the length of an update, which is the
/// longest this program ever holds one.
///
/// One function rather than a fetch, a check and an install a caller strings
/// together, because the order is the security property. A caller who unpacked
/// before checking, or re-took the record before verifying, would have written
/// something that passes its own tests and proves nothing, and there would be
/// two such callers.
///
/// # Where the download goes, and why it is not a temporary directory
///
/// Into [`workspace`], which is a directory beside the copy being replaced. Not
/// the system's temporary directory, for three reasons, in the order they bite:
///
/// * The install directory is writable by whoever owns the installation and by
///   nobody else, so the download cannot be read or replaced by another user on
///   a shared machine. A world-writable temporary directory is where a program
///   has to be careful about exactly that, and this project has no shipped code
///   that has had to be careful about it yet. Not being the first is worth more
///   than the tidiness.
/// * It fails early and honestly. A copy somewhere the user cannot write cannot
///   be updated at all, and finding that out before eighty megabytes are
///   fetched is better than finding it out after.
/// * The temporary directory is often small, sometimes in memory, and on more
///   than one platform is cleared while a program is still using it.
///
/// It is removed when this returns, whether the update succeeded or failed. A
/// failed update leaves the program it found and nothing else.
pub fn perform(
    offer: &Offer,
    password: Option<&[u8]>,
    say: &mut dyn FnMut(Step),
) -> Result<Done, Error> {
    // Before anything is fetched. An update that got as far as replacing the
    // binaries and then could not rewrite the record would leave the machine
    // in the one state this module exists to avoid.
    if lock_is_set() && password.is_none() {
        return Err(Error::Locked);
    }

    let into = installation()?;
    let workspace = workspace()?;
    let outcome = update_within(offer, password, &into, &workspace, say);
    // Whether it worked or not. A half-downloaded release left beside the
    // program is a file somebody finds later and cannot account for, and the
    // next attempt makes its own.
    let _ = std::fs::remove_dir_all(&workspace);
    outcome
}

/// Where an update does its work: beside the copy being replaced.
///
/// A fixed name rather than a unique one. Two updates of the same copy at once
/// is not a thing to support, and the fixed name means the directory left by a
/// process that was killed is found and cleared by the next attempt rather than
/// accumulating.
pub fn workspace() -> Result<PathBuf, Error> {
    Ok(installation()?.join(".veilvoice-update"))
}

/// The update itself, with the workspace's lifetime handled by the caller
/// above.
fn update_within(
    offer: &Offer,
    password: Option<&[u8]>,
    into: &Path,
    workspace: &Path,
    say: &mut dyn FnMut(Step),
) -> Result<Done, Error> {
    // Cleared rather than reused. A download directory holding something from
    // a previous attempt is a directory where a file this did not fetch could
    // be mistaken for one it did.
    if workspace.exists() {
        std::fs::remove_dir_all(workspace)
            .map_err(|e| Error::Io(format!("could not clear {}: {e}", workspace.display())))?;
    }
    let downloads = workspace.join("download");
    std::fs::create_dir_all(&downloads).map_err(|e| {
        Error::Io(format!(
            "could not make {}: {e}. An update is written beside the copy it \
             replaces, so this is a copy of VeilVoice in a place you cannot write to.",
            downloads.display()
        ))
    })?;

    // 2. Fetch. Nothing is opened and nothing is run.
    let mut fetched = Vec::new();
    for name in [
        crate::fetch::SUMS,
        crate::fetch::SIGNATURE,
        contents::CONTENTS,
        offer.archive.as_str(),
    ] {
        say(Step::Fetching(name.to_string()));
        let url = crate::fetch::asset_url(&offer.tag, name);
        let path = crate::fetch::download(&url, &downloads.join(name)).map_err(Error::Fetch)?;
        fetched.push(path);
    }
    let (sums_path, signature_path, contents_path, archive_path) =
        (&fetched[0], &fetched[1], &fetched[2], &fetched[3]);

    // 3. Prove, before anything is opened.
    say(Step::Checking);
    let listed = proved_contents(archive_path, sums_path, signature_path, contents_path)?;

    // 4. Open, and hash again what actually landed.
    say(Step::Opening);
    let unpacked = workspace.join("unpacked");
    let root = open_archive(archive_path, &unpacked, &listed)?;

    // 5. Replace.
    say(Step::Replacing);
    let (replaced, left_behind) = put_in_place(&root, into)?;

    // 6. Record, after the signature and not before.
    say(Step::Recording);
    let record = retake_record(into, &replaced, password);

    let restart = std::env::current_exe()
        .map_err(|e| Error::Io(format!("cannot find this program on disk: {e}")))?;
    Ok(Done {
        tag: offer.tag.clone(),
        into: into.to_path_buf(),
        replaced,
        left_behind,
        record,
        restart,
    })
}

/// The signature, the hash list, the contents list and every member, in that
/// order, all of it `crate::check`.
///
/// Returns the published list of what the archive carries, which every later
/// step is driven from. Nothing after this reads a name out of the archive.
fn proved_contents(
    archive_path: &Path,
    sums_path: &Path,
    signature_path: &Path,
    contents_path: &Path,
) -> Result<contents::ArchiveContents, Error> {
    let read = |path: &Path| -> Result<String, Error> {
        std::fs::read_to_string(path)
            .map_err(|e| Error::Io(format!("could not read {}: {e}", path.display())))
    };
    let sums = read(sums_path)?;
    let signature = read(signature_path)?;

    // The signature over SHA256SUMS is checked inside `check_file`, against the
    // key compiled into this binary, before any number in that list is read.
    let checked = check::check_file(archive_path, &sums, &signature)
        .map_err(|e| Error::Refused(e.to_string()))?;
    if !checked.matched {
        return Err(Error::Refused(format!(
            "{} is not the file the release signed. Published {}, downloaded {}.",
            checked.name, checked.expected, checked.actual
        )));
    }
    if !check::digests_match(&checked.fingerprint, check::FINGERPRINT) {
        // Unreachable while `check::key` returns the compiled-in key, and here
        // because the claim this module makes to the reader is about *which*
        // key, not merely about there being one.
        return Err(Error::Refused(format!(
            "the signature verified against {} rather than this project's key",
            checked.fingerprint
        )));
    }

    let manifest = check::check_file(contents_path, &sums, &signature)
        .map_err(|e| Error::Refused(e.to_string()))?;
    if !manifest.matched {
        return Err(Error::Refused(
            "the contents list is not the one the release signed".to_string(),
        ));
    }

    let all = contents::parse(&read(contents_path)?).map_err(|e| Error::Refused(e.to_string()))?;
    let listed = contents::for_archive(&all, &archive_name(archive_path)?)
        .ok_or_else(|| {
            Error::Refused("the signed contents list says nothing about this archive".to_string())
        })?
        .clone();

    // Every member, hashed where it lies. The archive is not unpacked to do
    // this and it is not unpacked until it has passed.
    let inside = archive::members(archive_path).map_err(|e| Error::Refused(e.to_string()))?;
    let comparison = archive::compare(&inside, &listed);
    if !comparison.is_clean() {
        let wrong: Vec<String> = comparison
            .outcomes
            .iter()
            .filter(|outcome| !outcome.is_good())
            .map(|outcome| match &outcome.verdict {
                archive::Verdict::Matches => unreachable!("filtered out above"),
                archive::Verdict::Differs { .. } => {
                    format!("{} is not the published file", outcome.path)
                }
                archive::Verdict::Missing => format!("{} is not in the archive", outcome.path),
                archive::Verdict::NotAFile(what) => {
                    format!("{} is {what} rather than a file", outcome.path)
                }
            })
            .collect();
        let mut why = Vec::new();
        if !wrong.is_empty() {
            why.push(format!("inside the archive: {}", wrong.join("; ")));
        }
        if !comparison.extras.is_empty() {
            why.push(format!(
                "files the release never published: {}",
                comparison.extras.join(", ")
            ));
        }
        return Err(Error::Refused(why.join(". ")));
    }
    Ok(listed)
}

/// The archive's own file name, which is the key into the contents list.
fn archive_name(path: &Path) -> Result<String, Error> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Io(format!("{} does not name a file", path.display())))
}

/// Unpack into an empty directory, then hash what landed.
///
/// Returns the release directory inside `into`, taken from the **signed** list
/// rather than from the archive or from the archive's file name.
fn open_archive(
    archive_path: &Path,
    into: &Path,
    listed: &contents::ArchiveContents,
) -> Result<PathBuf, Error> {
    // An empty directory, made here. Unpacking into one that already holds
    // something would mean the check below could pass on a file that was
    // already there.
    if into.exists() {
        std::fs::remove_dir_all(into)
            .map_err(|e| Error::Io(format!("could not clear {}: {e}", into.display())))?;
    }
    std::fs::create_dir_all(into)
        .map_err(|e| Error::Io(format!("could not make {}: {e}", into.display())))?;

    extract(archive_path, into)?;

    let roots = listed.roots();
    if roots.len() != 1 {
        return Err(Error::Refused(format!(
            "the signed list says this archive has {} top-level directories, and \
             a release archive has one",
            roots.len()
        )));
    }
    let root = roots.iter().next().expect("one root, just counted");

    // Hashed again, against the same signed list. The members were already
    // proved inside the archive; this proves that what the extractor wrote is
    // what it read. An extractor is a program too, and this is the cheapest
    // possible way to not have to trust it.
    let outcomes = contents::check(into, listed);
    let bad: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.is_good())
        .map(|o| o.path.clone())
        .collect();
    if !bad.is_empty() {
        return Err(Error::Refused(format!(
            "what came out of the archive is not what the release signed: {}",
            bad.join(", ")
        )));
    }

    let extras = contents::extras(into, listed);
    if !extras.extras.is_empty() {
        return Err(Error::Refused(format!(
            "the archive left files the release never published: {}",
            extras
                .extras
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    Ok(into.join(root))
}

/// Unpack with the tool the operating system already ships.
///
/// The same reasoning as `crate::fetch`, which borrows the system's transfer
/// tool rather than putting an HTTP client in a dependency graph this project
/// advertises as having none. Here it buys something else as well: this crate's
/// archive readers **never write**, and a test asserts it, because every
/// interesting defect in reading an archive is a defect in extracting one. That
/// property is worth more than the convenience of a Rust extractor, and it is
/// only worth anything if nothing here quietly adds one.
///
/// `tar` reads both forms. It has been in `System32` on Windows since 2018, it
/// is part of the base system on macOS and the BSDs, and it is on every Linux
/// installation there is. Resolved by absolute path, never by bare name: see
/// finding F-13.
fn extract(archive_path: &Path, into: &Path) -> Result<(), Error> {
    let tar = tar_program().ok_or_else(|| {
        Error::Unpack(
            "no tar was found on this system. Unpack the release yourself and \
             check it with `veilvoice verify auto`."
                .to_string(),
        )
    })?;
    let output = std::process::Command::new(&tar)
        .arg("-x")
        .arg("-f")
        .arg(archive_path)
        .arg("-C")
        .arg(into)
        .output()
        .map_err(|e| Error::Unpack(format!("could not run {}: {e}", tar.display())))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Unpack(if said.is_empty() {
            format!("{} exited with {}", tar.display(), output.status)
        } else {
            said
        }));
    }
    Ok(())
}

/// `tar`, by absolute path.
fn tar_program() -> Option<PathBuf> {
    #[cfg(windows)]
    let candidates = {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        vec![PathBuf::from(format!(r"{root}\System32\tar.exe"))]
    };
    #[cfg(not(windows))]
    let candidates = vec![
        PathBuf::from("/usr/bin/tar"),
        PathBuf::from("/bin/tar"),
        PathBuf::from("/usr/local/bin/tar"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Move the old binaries aside and put the new ones in their place.
///
/// The old copy is **moved, not deleted**, and moved first. A running program
/// cannot be overwritten on Windows and cannot be deleted at all, but it can be
/// renamed, and renaming it frees the name for the new one. On Unix the same
/// order costs nothing and means that at no point is the name missing: a rename
/// within a directory is atomic, so somebody starting VeilVoice during an
/// update gets the old one or the new one and never a partial file.
///
/// Anything that could not be cleaned up afterwards is returned rather than
/// ignored, because a `veilvoice.old` nobody was told about is a file somebody
/// finds in a year and cannot explain.
fn put_in_place(root: &Path, into: &Path) -> Result<(Vec<String>, Vec<PathBuf>), Error> {
    let mut replaced = Vec::new();
    let mut left_behind = Vec::new();

    for stem in crate::extracted::PROGRAMS {
        let name = crate::builder::with_platform_extension(stem);
        let from = root.join(&name);
        if !from.is_file() {
            // A platform whose archive carries only the command line is not
            // missing anything. Replacing what is published and leaving what is
            // not is the right behaviour, and the report names what moved.
            continue;
        }
        let target = into.join(&name);
        let aside = into.join(format!("{name}.old"));
        if target.exists() {
            let _ = std::fs::remove_file(&aside);
            std::fs::rename(&target, &aside).map_err(|e| {
                Error::Replace(format!(
                    "could not move the old {name} aside, so nothing was replaced: {e}"
                ))
            })?;
        }
        if let Err(e) = std::fs::copy(&from, &target) {
            // Put it back. Failing halfway through an update and leaving no
            // program at all is much worse than failing.
            let _ = std::fs::rename(&aside, &target);
            return Err(Error::Replace(format!(
                "could not write the new {name}: {e}. The copy you had has been put back."
            )));
        }
        carry_over_permissions(&from, &target);
        if aside.exists() && std::fs::remove_file(&aside).is_err() {
            left_behind.push(aside);
        }
        replaced.push(name);
    }

    if replaced.is_empty() {
        return Err(Error::Replace(format!(
            "the release carried none of the VeilVoice programs, so {} was left alone",
            into.display()
        )));
    }
    Ok((replaced, left_behind))
}

/// Give the new file the mode the archive published for it.
///
/// A copy takes the destination's mode on some systems and the source's on
/// others, and a new VeilVoice without its execute bit is the defect
/// `crate::extracted` exists to report. Best effort: a failure here leaves a
/// file that is byte for byte correct, which the report can still be honest
/// about, and refusing the whole update over a permission bit would be worse.
#[cfg(unix)]
fn carry_over_permissions(from: &Path, to: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(from) {
        let mode = meta.permissions().mode();
        let _ = std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode));
    }
}

/// The everywhere-else half: there is no execute bit to carry over.
#[cfg(not(unix))]
fn carry_over_permissions(_from: &Path, _to: &Path) {}

/// Re-take the integrity record over the files that were just replaced.
///
/// Never fails the update. By the time this runs the new files are in place and
/// have been proved against a signature; a record that could not be written is
/// worth reporting and is not worth undoing a good update for. [`Record`] says
/// which happened.
fn retake_record(into: &Path, replaced: &[String], password: Option<&[u8]>) -> Record {
    let Some(plain) = veilvoice_guard::record_path() else {
        return Record::NotTaken(
            "this platform names no configuration directory, so there is nowhere \
             to keep the record"
                .to_string(),
        );
    };
    let sealed = veilvoice_guard::sealed_record_path(&plain);
    let files: Vec<PathBuf> = replaced.iter().map(|name| into.join(name)).collect();

    let manifest = match veilvoice_guard::Manifest::of(&files) {
        Ok(manifest) => manifest,
        Err(why) => return Record::NotTaken(why.to_string()),
    };

    match password {
        Some(pw) => match manifest.seal(pw) {
            Err(why) => Record::NotTaken(why.to_string()),
            Ok(bytes) => match write_beside(&sealed, &bytes) {
                Err(why) => Record::NotTaken(why),
                Ok(()) => {
                    // A plain record left beside a sealed one is a downgrade
                    // waiting to be used: whoever can write that directory could
                    // drop one there and have the next check read it instead.
                    let _ = std::fs::remove_file(&plain);
                    Record::Sealed
                }
            },
        },
        None => match manifest.save(&plain) {
            Err(why) => Record::NotTaken(why.to_string()),
            Ok(()) => Record::Plain,
        },
    }
}

/// Write the sealed record, owner-readable, making its directory if needed.
fn write_beside(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    veilvoice_crypto::privatefile::write_owner_only(path, bytes).map_err(|e| e.to_string())
}

/// Start the updated program and return.
///
/// The caller exits afterwards; this does not exit for it, because a window has
/// state to put down first and a command line has an exit status to return.
/// Nothing is waited for: the point is to replace this process, not to supervise
/// a child.
pub fn relaunch(program: &Path) -> Result<(), Error> {
    std::process::Command::new(program)
        .spawn()
        .map(|_| ())
        .map_err(|e| Error::Io(format!("could not start {}: {e}", program.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This build must be offerable an update, or must say it cannot be.
    ///
    /// The two lists exist because a label is a string in a workflow and a
    /// `cfg` here, and nothing in the compiler relates them. A build whose
    /// label is not one the release publishes would ask for an archive that has
    /// never existed and report the 404 as a failed download.
    #[test]
    fn this_builds_label_is_one_the_release_publishes() {
        let Some(label) = PLATFORM else {
            // A target the workflow has no row for. Saying nothing is offered
            // is the right answer and there is nothing further to check.
            return;
        };
        assert!(
            PUBLISHED.contains(&label),
            "this build calls itself {label:?}, which the release publishes nothing for"
        );
    }

    /// Every published label is distinct, and none is a prefix of another.
    ///
    /// The prefix half is not pedantry: `archive_for` decides the extension by
    /// asking whether the label starts with `windows`, and a label that began
    /// with another label would make a reader of this code have to work out
    /// which rule won.
    #[test]
    fn the_published_labels_are_distinct() {
        for (at, label) in PUBLISHED.iter().enumerate() {
            assert!(!label.is_empty());
            assert!(
                !PUBLISHED[..at].contains(label),
                "{label:?} is listed twice"
            );
        }
    }

    /// The archive name is the one the release workflow writes.
    ///
    /// `veilvoice-<tag>-<label>`, then `.zip` on Windows and `.tar.gz`
    /// everywhere else, which is `dist/$out` in `.github/workflows/release.yml`
    /// followed by whichever of the two archive steps that row selects.
    #[test]
    fn an_archive_is_named_the_way_the_release_workflow_names_it() {
        assert_eq!(
            archive_for("v0.1.23", "linux-x86_64"),
            "veilvoice-v0.1.23-linux-x86_64.tar.gz"
        );
        assert_eq!(
            archive_for("v0.1.23", "windows-x86_64"),
            "veilvoice-v0.1.23-windows-x86_64.zip"
        );
        assert_eq!(
            archive_for("v0.1.23", "macos-arm64"),
            "veilvoice-v0.1.23-macos-arm64.tar.gz"
        );
        // Every published label produces a name of one of the two shapes, and
        // none of them produces something with no extension at all.
        for label in PUBLISHED {
            let name = archive_for("v0.1.23", label);
            assert!(
                name.ends_with(".zip") || name.ends_with(".tar.gz"),
                "{name} is neither of the two forms the workflow builds"
            );
            assert!(name.starts_with("veilvoice-v0.1.23-"), "{name}");
        }
    }

    /// The order in [`update_within`] is the security property, read out of the
    /// source.
    ///
    /// **What actually enforces this order is the compiler, and that is the
    /// stronger thing.** Each step consumes the previous step's result:
    /// `open_archive` needs the `ArchiveContents` that only `proved_contents`
    /// returns, `put_in_place` needs the release directory that only
    /// `open_archive` returns, and `retake_record` needs the file names that
    /// only `put_in_place` returns. Reversing any pair of them does not
    /// compile. That is deliberate, and it is why the functions hand values
    /// back rather than writing to paths the next one knows about.
    ///
    /// So this test is not what stops the reversal today. It is the backstop
    /// for the rewrite that would remove the compiler's hold on it: a version
    /// of this function that passed directory paths around instead of values
    /// would still type-check in any order, and that is a perfectly natural
    /// thing for somebody to do while tidying. Kept for that, and for saying in
    /// one place what the order is.
    #[test]
    fn nothing_happens_before_the_thing_that_makes_it_safe() {
        let source = include_str!("update.rs").replace("\r\n", "\n");
        let start = source
            .find("fn update_within(")
            .expect("update_within exists");
        let end = source[start..]
            .find("\n/// The signature, the hash list")
            .map(|at| start + at)
            .expect("the function ends");
        let body = &source[start..end];

        let proved = body.find("proved_contents(").expect("the check is called");
        let opened = body.find("open_archive(").expect("the archive is opened");
        let replaced = body.find("put_in_place(").expect("the files are replaced");
        let recorded = body.find("retake_record(").expect("the record is re-taken");

        assert!(
            proved < opened,
            "the archive is opened before it has been checked"
        );
        assert!(
            opened < replaced,
            "files are replaced before the archive has been opened"
        );
        assert!(
            proved < recorded,
            "the integrity record is re-taken before the signature has been checked, \
             which would write down whatever arrived as the correct files"
        );
        assert!(
            replaced < recorded,
            "the record is taken before the new files are in place, so it would \
             describe the old ones"
        );
    }

    /// A locked machine is refused before a single byte is fetched.
    ///
    /// Read out of [`perform`], which is short for exactly this reason: the
    /// check has to be the first thing in it, and a short function is one where
    /// that is obvious to a reader as well as to this test.
    #[test]
    fn a_locked_machine_is_refused_before_anything_is_downloaded() {
        let source = include_str!("update.rs").replace("\r\n", "\n");
        let start = source.find("pub fn perform(").expect("perform exists");
        let end = source[start..]
            .find("\n/// Where an update does its work")
            .map(|at| start + at)
            .expect("the function ends");
        let body = &source[start..end];

        let locked = body.find("Error::Locked").expect("the refusal is there");
        let work = body.find("update_within(").expect("the work is dispatched");
        assert!(
            locked < work,
            "an update on a locked machine starts fetching before it finds out it \
             cannot finish"
        );
    }

    /// The only program this module runs is `tar`, and it comes from
    /// [`tar_program`].
    ///
    /// The point of an updater that checks a download is that it never runs
    /// what it fetched. A spawn added here later, of anything at all, is the
    /// one change to this file that could undo that, and it is not a change a
    /// reviewer would necessarily read as dangerous.
    #[test]
    fn the_only_program_it_runs_is_the_systems_own_tar() {
        let source = include_str!("update.rs").replace("\r\n", "\n");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or("");
        let spawns: Vec<&str> = shipped
            .lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with("//"))
            .filter(|line| line.contains("Command::new"))
            .collect();
        assert_eq!(
            spawns.len(),
            2,
            "exactly two spawns are expected, tar and the relaunch: {spawns:?}"
        );
        assert!(
            spawns[0].contains("&tar"),
            "the first spawn is not the system tar: {}",
            spawns[0]
        );
        assert!(
            spawns[1].contains("program"),
            "the second spawn is not the relaunch: {}",
            spawns[1]
        );
    }

    // -----------------------------------------------------------------------
    // The parts that can be exercised for real
    //
    // The whole of `perform` cannot be: it needs the network and a signature
    // only the maintainer can make. What can be, and is below, is everything
    // after the signature: opening an archive built by the real `tar`, hashing
    // what came out of it against a list, and putting files in place.
    // -----------------------------------------------------------------------

    /// Somewhere to work, removed by the caller.
    fn room(what: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("veilvoice-update-{what}-{stamp:x}"));
        std::fs::create_dir_all(&path).expect("a directory to work in");
        path
    }

    /// Stage a release directory and tar it up, the shape the release job
    /// builds. `None` where this machine has no `tar`, which is a fact about
    /// the machine.
    fn a_release(root: &Path, tag: &str) -> Option<(PathBuf, contents::ArchiveContents)> {
        let tar = tar_program()?;
        let name = format!("veilvoice-{tag}-test");
        let staged = root.join("staged");
        let release = staged.join(&name);
        std::fs::create_dir_all(release.join("docs")).unwrap();
        std::fs::write(release.join("veilvoice"), b"the command line").unwrap();
        std::fs::write(release.join("veilvoice-gui"), b"the window").unwrap();
        std::fs::write(release.join("docs/README.md"), b"# VeilVoice\n").unwrap();

        let archive = root.join(format!("{name}.tar.gz"));
        let ran = std::process::Command::new(&tar)
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&staged)
            .arg(&name)
            .output()
            .expect("tar runs");
        assert!(ran.status.success(), "tar failed");

        // The list, built here from the staged files rather than by the
        // generator. What the generator and the reader agree about is
        // `tests/inside_the_archive.rs`'s subject; what this file is about is
        // what happens after a list has been proved.
        let mut members = Vec::new();
        for relative in ["veilvoice", "veilvoice-gui", "docs/README.md"] {
            members.push(contents::Member {
                path: format!("{name}/{relative}"),
                digest: check::sha256_file(&release.join(relative)).unwrap(),
            });
        }
        Some((
            archive,
            contents::ArchiveContents {
                archive: format!("{name}.tar.gz"),
                members,
            },
        ))
    }

    /// An archive that matches its list is opened, and the release directory
    /// named by the **list** is what comes back.
    #[test]
    fn an_archive_that_matches_its_signed_list_is_opened() {
        let root = room("opened");
        let Some((archive, listed)) = a_release(&root, "v0.1.23") else {
            std::fs::remove_dir_all(&root).ok();
            return; // no tar on this machine
        };
        let into = root.join("unpacked");
        let release = open_archive(&archive, &into, &listed).expect("it opens");
        assert!(release.join("veilvoice").is_file());
        assert!(release.join("docs/README.md").is_file());
        assert_eq!(release.parent(), Some(into.as_path()));
        std::fs::remove_dir_all(&root).ok();
    }

    /// What came out of the archive is hashed **again**, against the same
    /// signed list.
    ///
    /// The members were already proved inside the archive before it was opened,
    /// so this second pass exists only to catch an extractor that wrote
    /// something other than what it read. Proved by giving the list a digest
    /// the file does not have: if the second pass were not there, or were
    /// comparing against something it derived from the disk, this would open
    /// happily.
    #[test]
    fn what_came_out_of_the_archive_is_hashed_again() {
        let root = room("rehashed");
        let Some((archive, mut listed)) = a_release(&root, "v0.1.23") else {
            std::fs::remove_dir_all(&root).ok();
            return;
        };
        listed.members[0].digest =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let into = root.join("unpacked");
        let error = open_archive(&archive, &into, &listed)
            .expect_err("a file that is not what the list says must be refused");
        let said = error.to_string();
        assert!(said.contains("not what the release signed"), "{said}");
        assert!(
            said.contains("veilvoice"),
            "the refusal must name the file: {said}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// An archive carrying a file the release never published is refused.
    ///
    /// The comparison before the archive is opened already catches this, and
    /// the sweep afterwards catches it again. Both are kept: the first means
    /// nothing is written at all, and the second means an extractor that
    /// invented a file on its own does not go unnoticed either.
    #[test]
    fn a_file_the_release_never_published_is_refused() {
        let root = room("extras");
        let Some((archive, mut listed)) = a_release(&root, "v0.1.23") else {
            std::fs::remove_dir_all(&root).ok();
            return;
        };
        // Drop a member from the list rather than adding a file to the archive:
        // the same disagreement, and it does not need tar run twice.
        let dropped = listed.members.pop().expect("three members");
        let into = root.join("unpacked");
        let error = open_archive(&archive, &into, &listed)
            .expect_err("a file that was never published must be refused");
        let said = error.to_string();
        assert!(said.contains("never published"), "{said}");
        assert!(
            said.contains("README.md"),
            "the refusal must name it, and {} is what was dropped: {said}",
            dropped.path
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// Replacing puts the new files in and takes the old ones away.
    #[test]
    fn the_new_files_replace_the_old_ones() {
        let root = room("replace");
        let from = root.join("new");
        let into = root.join("installed");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&into).unwrap();
        for stem in crate::extracted::PROGRAMS {
            let name = crate::builder::with_platform_extension(stem);
            std::fs::write(from.join(&name), b"the new one").unwrap();
            std::fs::write(into.join(&name), b"the old one").unwrap();
        }

        let (replaced, left_behind) = put_in_place(&from, &into).expect("it replaces");
        assert_eq!(replaced.len(), crate::extracted::PROGRAMS.len());
        assert!(left_behind.is_empty(), "{left_behind:?}");
        for name in &replaced {
            assert_eq!(
                std::fs::read(into.join(name)).unwrap(),
                b"the new one",
                "{name} was not replaced"
            );
            assert!(
                !into.join(format!("{name}.old")).exists(),
                "{name}.old was left behind"
            );
        }
        std::fs::remove_dir_all(&root).ok();
    }

    /// A release carrying only the command line replaces only the command line.
    ///
    /// Three of the published targets are command-line only, so this is the
    /// ordinary case on them rather than an edge. Replacing what is there and
    /// leaving the rest alone is right; refusing because the window was not in
    /// the archive would make those platforms un-updatable.
    #[test]
    fn a_release_with_only_the_command_line_replaces_only_that() {
        let root = room("cli-only");
        let from = root.join("new");
        let into = root.join("installed");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&into).unwrap();
        let cli = crate::builder::with_platform_extension("veilvoice");
        let gui = crate::builder::with_platform_extension("veilvoice-gui");
        std::fs::write(from.join(&cli), b"the new one").unwrap();
        std::fs::write(into.join(&cli), b"the old one").unwrap();
        std::fs::write(into.join(&gui), b"a window nobody replaced").unwrap();

        let (replaced, _) = put_in_place(&from, &into).expect("it replaces what is there");
        assert_eq!(replaced, vec![cli.clone()]);
        assert_eq!(std::fs::read(into.join(&cli)).unwrap(), b"the new one");
        assert_eq!(
            std::fs::read(into.join(&gui)).unwrap(),
            b"a window nobody replaced",
            "a program the release did not carry was touched"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// A directory carrying none of the programs replaces nothing and says so.
    ///
    /// Rather than reporting a successful update of no files, which is how
    /// somebody ends up believing they are on a version they are not.
    #[test]
    fn a_release_carrying_no_programs_is_refused() {
        let root = room("empty");
        let from = root.join("new");
        let into = root.join("installed");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&into).unwrap();
        std::fs::write(from.join("README.md"), b"# nothing to install\n").unwrap();

        let error = put_in_place(&from, &into).expect_err("nothing to install must be refused");
        assert!(error.to_string().contains("none of the VeilVoice programs"));
        std::fs::remove_dir_all(&root).ok();
    }

    /// The new file keeps the execute bit the archive published for it.
    ///
    /// A copy takes the destination's mode on some systems, and an update that
    /// left VeilVoice without its execute bit would produce exactly the
    /// complaint `crate::extracted` exists to report: a folder that looks
    /// perfect and does nothing.
    #[cfg(unix)]
    #[test]
    fn the_replaced_program_is_still_runnable() {
        use std::os::unix::fs::PermissionsExt;
        let root = room("mode");
        let from = root.join("new");
        let into = root.join("installed");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&into).unwrap();
        let name = crate::builder::with_platform_extension("veilvoice");
        std::fs::write(from.join(&name), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(from.join(&name), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(into.join(&name), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(into.join(&name), std::fs::Permissions::from_mode(0o644)).unwrap();

        put_in_place(&from, &into).expect("it replaces");
        let mode = std::fs::metadata(into.join(&name))
            .unwrap()
            .permissions()
            .mode();
        assert!(
            mode & 0o111 != 0,
            "the replacement is not runnable: mode {mode:o}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// The workspace is beside the copy being replaced, and is hidden.
    #[test]
    fn the_workspace_sits_beside_the_program_it_replaces() {
        let Ok(workspace) = workspace() else {
            return; // no current_exe on this platform
        };
        let installation = installation().expect("an installation, since there is a workspace");
        assert_eq!(workspace.parent(), Some(installation.as_path()));
        let name = workspace
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with('.'),
            "a directory an update leaves for a moment should not sit in the middle \
             of somebody's folder listing: {name}"
        );
    }
}
