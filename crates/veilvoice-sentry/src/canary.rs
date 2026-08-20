// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoy files that should never change, and a record of what they were.
//!
//! # The idea, in one paragraph
//!
//! Put a file somewhere nothing legitimately writes, remember exactly what it
//! contained, and look at it later. If it differs, something walked that
//! directory and wrote to everything it found. That is a much cleaner signal
//! than counting file changes, because there is no innocent explanation for a
//! file nobody uses having been rewritten.
//!
//! # And the hole in it, which is large
//!
//! A canary fires only if whatever is running **reaches it**. Something that
//! encrypts `.docx` under one folder will never look at a canary planted
//! anywhere else, and this crate has no way to tell "nothing happened" from
//! "it happened somewhere I was not". A quiet canary is not an all-clear, and
//! no wording in a front end may present it as one.
//!
//! Plant several, in the directories that actually matter, and accept that the
//! answer is still one-sided: a trip is evidence, silence is not.
//!
//! # The name is a deliberate trade, and the default takes the honest side
//!
//! [`DEFAULT_NAME`] says what the file is. Anything that reads filenames before
//! deciding what to encrypt can therefore skip it, and the signal is lost. A
//! decoy called `quarterly-report.docx` would survive that.
//!
//! The default is the recognisable name anyway, for two reasons. Indiscriminate
//! encryption of everything under a directory is the overwhelmingly common
//! case, and it is not fooled either way. And a file the user does not
//! recognise is a file the user eventually deletes — which reads here as a
//! trip, produces an alarm that was nobody's fault, and teaches them to ignore
//! the next one. A warning system whose alarms are usually wrong is worse than
//! no warning system.
//!
//! [`Nest::plant`] takes a name, so anybody who wants the quieter trade can
//! have it. It is a choice, made in the open, not a default that decides for
//! them.
//!
//! # A deletion and an encryption look the same
//!
//! Much ransomware writes a new encrypted file beside the original and deletes
//! the original, so the canary comes back as [`State::Removed`] — which is
//! also what a user tidying a folder produces. [`State`] reports which of the
//! two happened to the file, never which of the two happened in the world.
//!
//! # Format
//!
//! One record per line, text, for the same reason the tamper manifest is text:
//! the point of the file is to be checkable without this crate.
//!
//! ```text
//! VEILSENTRY-NEST1
//! <sha256 hex>  <size>  <planted unix seconds>  <path>
//! ```

use crate::{entropy, Error};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Magic first line. The digit is a format version.
const MAGIC: &str = "VEILSENTRY-NEST1";

/// The default filename for a canary.
///
/// Recognisable on purpose. See the module documentation for the trade this
/// makes and why the default takes this side of it.
pub const DEFAULT_NAME: &str = "veilvoice-canary.txt";

/// Below this, a canary that was prose has stopped being prose.
///
/// Encrypted output sits within a hair of 8.0 bits per byte; English prose is
/// under 5. The threshold is well clear of both, so it says "this is no longer
/// text" rather than "this is encrypted" — which this crate cannot know and
/// does not claim. A `.zip` written over the canary would read the same way.
pub const PROSE_CEILING: f32 = 6.0;

/// One planted decoy, and what it was when it was planted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canary {
    /// Where it is, normalised with forward slashes.
    pub path: String,
    /// Its size in bytes when planted.
    pub size: u64,
    /// Lowercase hex SHA-256 of its contents when planted.
    pub digest: String,
    /// When it was planted, in seconds since the Unix epoch.
    pub planted: u64,
}

/// What a canary looks like now.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Byte for byte what was planted.
    Intact,
    /// Still there, and different.
    Modified {
        /// The digest recorded at planting.
        was: String,
        /// The digest now.
        now: String,
        /// Size now, which a front end can compare against the recorded one.
        size_now: u64,
        /// Shannon entropy of the contents now, in bits per byte.
        ///
        /// Meaningful **only** because this crate wrote the original and knows
        /// it was prose. Read [`crate::entropy`] before showing this to
        /// anybody.
        entropy_now: f32,
    },
    /// Recorded and no longer there. A user deleting it looks like this too.
    Removed,
    /// There and unreadable, which is not the same as absent: a permissions
    /// change is itself worth reporting.
    Unreadable(String),
}

impl State {
    /// Whether this is anything other than [`State::Intact`].
    pub fn is_trip(&self) -> bool {
        !matches!(self, State::Intact)
    }

    /// Whether the contents stopped looking like the text that was planted.
    ///
    /// False for every state except [`State::Modified`], and false there for a
    /// rewrite that is still text. **Not** a claim that the file was
    /// encrypted; see [`PROSE_CEILING`].
    pub fn stopped_being_text(&self) -> bool {
        match self {
            State::Modified { entropy_now, .. } => *entropy_now >= PROSE_CEILING,
            _ => false,
        }
    }

    /// A single line for a terminal or a log.
    pub fn describe(&self) -> String {
        match self {
            State::Intact => "intact".to_string(),
            State::Modified {
                size_now,
                entropy_now,
                ..
            } => {
                let note = if *entropy_now >= PROSE_CEILING {
                    " and is no longer text (which is what encrypting it would look \
                     like, and also what compressing it would look like)"
                } else {
                    ""
                };
                format!("rewritten: {size_now} bytes at {entropy_now:.2} bits/byte{note}")
            }
            State::Removed => {
                "gone -- deleted, or replaced by something written under another name".to_string()
            }
            State::Unreadable(why) => format!("unreadable: {why}"),
        }
    }
}

/// One canary and what became of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sighting {
    /// The canary, as recorded.
    pub canary: Canary,
    /// What it looks like now.
    pub state: State,
}

/// The set of planted canaries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nest {
    /// Keyed by normalised path, in a `BTreeMap` so the serialised form is
    /// byte-identical for the same set. Two nests are then comparable, and a
    /// file that is committed or synchronised does not churn.
    canaries: BTreeMap<String, Canary>,
}

/// Normalise a path for storage: forward slashes, as the tamper manifest does,
/// so a nest written on Windows still reads on Linux.
fn normalise(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn digest_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The text a canary is filled with.
///
/// Derived from the filename alone, so it is deterministic: the same name
/// always produces the same bytes and therefore the same digest, which is what
/// makes this testable without a clock or a random source.
///
/// It is prose, and that is load-bearing. The entropy of what is there later is
/// only informative because the entropy of what was put there is known to be
/// low. Filling a canary with random bytes would have destroyed the one
/// measurement it supports.
pub fn contents(name: &str) -> String {
    let mut text = String::new();
    text.push_str(
        "This file was placed here by VeilVoice, and its only job is to stay exactly as \
         it is.\n\n\
         Nothing reads it and nothing needs it. VeilVoice keeps a record of these bytes, \
         and if they ever change it will say so -- because a file that nothing uses does \
         not get rewritten by accident. If you move or delete this file, VeilVoice will \
         report that as a change, so remove it from the watch list first rather than \
         simply deleting it.\n\n\
         What this cannot do is worth knowing. It only notices something that reaches \
         this folder. It cannot stop anything. And it cannot tell you which program was \
         responsible.\n\n",
    );
    // Named in the body so two canaries in different folders differ, and a
    // record cannot be silently satisfied by a copy of another one.
    text.push_str("Canary: ");
    text.push_str(name);
    text.push_str("\n\n");
    // Enough prose for an entropy measurement to mean anything. A hundred bytes
    // of text has a noisy byte distribution; a couple of kilobytes does not.
    for line in FILLER.iter().cycle().take(FILLER.len() * 6) {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Plain sentences, in the project's own register, repeated to make a body.
const FILLER: &[&str] = &[
    "The words are kept on purpose; the voiceprint is destroyed.",
    "Every claim in this project is written to be checkable by the person reading it.",
    "Detection is not prevention, and the difference is not a detail.",
    "A tool that overstates what it does leaves its user worse off than one that says nothing.",
    "Encryption at rest is the default because the alternative is a file somebody later regrets.",
];

impl Nest {
    /// An empty nest.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many canaries are planted.
    pub fn len(&self) -> usize {
        self.canaries.len()
    }

    /// Whether nothing is planted.
    pub fn is_empty(&self) -> bool {
        self.canaries.is_empty()
    }

    /// The planted canaries, in a stable order.
    pub fn canaries(&self) -> impl Iterator<Item = &Canary> {
        self.canaries.values()
    }

    /// Write a canary into `dir` and record it.
    ///
    /// `name` defaults to [`DEFAULT_NAME`]. Returns the path written.
    ///
    /// **Refuses to overwrite an existing file.** A canary is a file this crate
    /// creates; writing over one that was already there would destroy somebody's
    /// data in the name of protecting it, and the failure would be silent
    /// because the whole point is that nothing reads these afterwards.
    pub fn plant(&mut self, dir: &Path, name: Option<&str>) -> Result<PathBuf, Error> {
        let name = name.unwrap_or(DEFAULT_NAME);
        if name.is_empty()
            || name.contains(['/', '\\'])
            || name.contains('\n')
            || name.contains('\r')
        {
            return Err(Error::Malformed(format!(
                "{name:?} is not a filename: a canary's name may not contain a path \
                 separator or a line break"
            )));
        }
        std::fs::create_dir_all(dir)?;
        let path = dir.join(name);
        if path.exists() {
            return Err(Error::Malformed(format!(
                "{} already exists. A canary is a file VeilVoice creates; it will not \
                 write over one that is already there.",
                path.display()
            )));
        }
        let key = normalise(&path);
        if key.contains('\n') || key.contains('\r') {
            return Err(Error::Malformed(format!(
                "path contains a line break and cannot be recorded: {key:?}"
            )));
        }

        let body = contents(name);
        std::fs::write(&path, body.as_bytes())?;
        self.canaries.insert(
            key.clone(),
            Canary {
                path: key,
                size: body.len() as u64,
                digest: digest_of(body.as_bytes()),
                planted: now_seconds(),
            },
        );
        Ok(path)
    }

    /// Stop watching a canary, and delete it.
    ///
    /// The order matters: the record goes first. If the delete fails, the file
    /// is left behind but no longer watched — an untidy folder. Doing it the
    /// other way round and failing leaves a record of a file that is gone,
    /// which is a permanent false alarm.
    pub fn pull_up(&mut self, path: &Path) -> Result<(), Error> {
        let key = normalise(path);
        if self.canaries.remove(&key).is_none() {
            return Err(Error::Malformed(format!(
                "{key} is not a canary in this nest"
            )));
        }
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Io(error)),
        }
    }

    /// Look at every canary. Changes nothing.
    pub fn check(&self) -> Vec<Sighting> {
        self.canaries
            .values()
            .map(|canary| Sighting {
                canary: canary.clone(),
                state: state_of(canary),
            })
            .collect()
    }

    /// Only the canaries that are not intact.
    pub fn trips(&self) -> Vec<Sighting> {
        self.check()
            .into_iter()
            .filter(|sighting| sighting.state.is_trip())
            .collect()
    }

    /// Serialise to the text format described at the top of this module.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        for canary in self.canaries.values() {
            out.push_str(&format!(
                "{}  {}  {}  {}\n",
                canary.digest, canary.size, canary.planted, canary.path
            ));
        }
        out
    }

    /// Parse the text format.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the nest file is empty".into())),
        }

        let mut canaries = BTreeMap::new();
        for (index, line) in lines.enumerate() {
            let number = index + 2;
            if line.trim().is_empty() {
                continue;
            }
            // Split on the double space, and only three times: a path may
            // contain single spaces and routinely does on Windows.
            let mut parts = line.splitn(4, "  ");
            let digest = parts.next().unwrap_or_default().trim();
            let size = parts.next().unwrap_or_default().trim();
            let planted = parts.next().unwrap_or_default().trim();
            let path = parts.next().unwrap_or_default().trim();

            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::Malformed(format!(
                    "line {number}: {digest:?} is not a SHA-256 digest"
                )));
            }
            let size: u64 = size
                .parse()
                .map_err(|_| Error::Malformed(format!("line {number}: bad size {size:?}")))?;
            let planted: u64 = planted
                .parse()
                .map_err(|_| Error::Malformed(format!("line {number}: bad time {planted:?}")))?;
            if path.is_empty() {
                return Err(Error::Malformed(format!("line {number}: no path")));
            }
            // Normalised on the way in exactly as on the way out. The tamper
            // manifest learned this the expensive way: a hand-written file with
            // backslashes keyed its entries differently from the ones the check
            // looked up, so every recorded file was reported as missing.
            let path = path.replace('\\', "/");
            canaries.insert(
                path.clone(),
                Canary {
                    path,
                    size,
                    digest: digest.to_ascii_lowercase(),
                    planted,
                },
            );
        }
        Ok(Self { canaries })
    }

    /// Write the nest to `path`.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read a nest written by [`Nest::save`].
    pub fn load(path: &Path) -> Result<Self, Error> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
}

fn state_of(canary: &Canary) -> State {
    match std::fs::read(&canary.path) {
        Ok(bytes) => {
            let now = digest_of(&bytes);
            if now == canary.digest {
                State::Intact
            } else {
                State::Modified {
                    was: canary.digest.clone(),
                    now,
                    size_now: bytes.len() as u64,
                    entropy_now: entropy(&bytes),
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::Removed,
        Err(error) => State::Unreadable(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nest_in(dir: &Path) -> (Nest, PathBuf) {
        let mut nest = Nest::new();
        let path = nest.plant(dir, None).expect("planting should work");
        (nest, path)
    }

    #[test]
    fn a_planted_canary_is_intact() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, path) = nest_in(dir.path());
        assert!(path.exists());
        assert_eq!(nest.len(), 1);
        assert!(!nest.is_empty());
        let sightings = nest.check();
        assert_eq!(sightings.len(), 1);
        assert_eq!(sightings[0].state, State::Intact);
        assert!(nest.trips().is_empty());
    }

    /// Checking must not be what changes the thing being checked.
    #[test]
    fn checking_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, path) = nest_in(dir.path());
        let before = std::fs::read(&path).unwrap();
        for _ in 0..3 {
            assert!(nest.trips().is_empty());
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_rewritten_canary_is_a_trip() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, path) = nest_in(dir.path());
        std::fs::write(&path, b"something else entirely, but still words").unwrap();
        let trips = nest.trips();
        assert_eq!(trips.len(), 1);
        match &trips[0].state {
            State::Modified { was, now, .. } => assert_ne!(was, now),
            other => panic!("expected Modified, got {other:?}"),
        }
        assert!(
            !trips[0].state.stopped_being_text(),
            "text rewritten as other text must not be reported as no longer text"
        );
    }

    /// The one measurement entropy supports: a canary that was prose and is
    /// now incompressible.
    #[test]
    fn a_canary_full_of_incompressible_bytes_reads_as_no_longer_text() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, path) = nest_in(dir.path());
        // Every byte value, evenly: 8.0 bits per byte, which is what encrypted
        // output looks like -- and equally what a .zip looks like, which is
        // exactly why `describe` says both.
        let dense: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();
        std::fs::write(&path, &dense).unwrap();
        let trips = nest.trips();
        assert!(trips[0].state.stopped_being_text());
        let described = trips[0].state.describe();
        assert!(described.contains("no longer text"), "{described}");
        assert!(
            described.contains("compressing"),
            "the honest alternative explanation must be in the sentence: {described}"
        );
    }

    #[test]
    fn a_deleted_canary_is_reported_as_gone_without_saying_why() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, path) = nest_in(dir.path());
        std::fs::remove_file(&path).unwrap();
        let trips = nest.trips();
        assert_eq!(trips[0].state, State::Removed);
        let described = trips[0].state.describe();
        assert!(described.contains("deleted"), "{described}");
        // It must not assert that something malicious happened.
        for word in ["ransomware", "attack", "malware"] {
            assert!(!described.to_lowercase().contains(word), "{described}");
        }
    }

    /// A file already there is somebody's data. Refusing is the only safe
    /// answer, because nothing reads a canary afterwards and the loss would
    /// never be noticed.
    #[test]
    fn planting_never_writes_over_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DEFAULT_NAME);
        std::fs::write(&path, b"the user's own notes").unwrap();
        let mut nest = Nest::new();
        let error = nest
            .plant(dir.path(), None)
            .expect_err("must refuse rather than overwrite");
        assert!(error.to_string().contains("already exists"));
        assert_eq!(std::fs::read(&path).unwrap(), b"the user's own notes");
        assert!(nest.is_empty(), "a refused plant must record nothing");
    }

    #[test]
    fn a_name_that_is_a_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut nest = Nest::new();
        for bad in [
            "../escape.txt",
            "sub/dir.txt",
            "back\\slash.txt",
            "",
            "a\nb",
        ] {
            assert!(
                nest.plant(dir.path(), Some(bad)).is_err(),
                "{bad:?} should be refused"
            );
        }
        assert!(nest.is_empty());
    }

    #[test]
    fn pulling_up_removes_both_the_record_and_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let (mut nest, path) = nest_in(dir.path());
        nest.pull_up(&path).expect("pulling up should work");
        assert!(nest.is_empty());
        assert!(!path.exists());
        // And doing it twice is an error about the record, not a panic.
        assert!(nest.pull_up(&path).is_err());
    }

    /// A canary somebody already deleted must still be removable from the
    /// record, or the alarm can never be cleared.
    #[test]
    fn pulling_up_a_canary_that_is_already_gone_still_clears_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let (mut nest, path) = nest_in(dir.path());
        std::fs::remove_file(&path).unwrap();
        nest.pull_up(&path).expect("the record must still clear");
        assert!(nest.is_empty());
    }

    #[test]
    fn several_canaries_are_each_watched() {
        let dir = tempfile::tempdir().unwrap();
        let one = dir.path().join("one");
        let two = dir.path().join("two");
        let mut nest = Nest::new();
        let a = nest.plant(&one, None).unwrap();
        nest.plant(&two, None).unwrap();
        assert_eq!(nest.len(), 2);
        std::fs::write(&a, b"changed").unwrap();
        assert_eq!(nest.trips().len(), 1, "only the changed one should trip");
    }

    /// Two canaries in different folders must not contain identical bytes, or
    /// a record could be satisfied by copying one over the other.
    #[test]
    fn a_canary_names_itself_so_copies_do_not_satisfy_each_other() {
        assert_ne!(contents("one.txt"), contents("two.txt"));
        assert!(contents("one.txt").contains("one.txt"));
    }

    /// The same name always produces the same bytes: the digest is testable
    /// without a clock or a random source.
    #[test]
    fn contents_are_deterministic() {
        assert_eq!(contents(DEFAULT_NAME), contents(DEFAULT_NAME));
    }

    /// Prose, and enough of it for the entropy figure to mean anything.
    #[test]
    fn the_planted_body_is_low_entropy_text_of_a_usable_size() {
        let body = contents(DEFAULT_NAME);
        assert!(
            body.len() > 1024,
            "too short for entropy to mean anything: {}",
            body.len()
        );
        let measured = entropy(body.as_bytes());
        assert!(
            measured < PROSE_CEILING,
            "a freshly planted canary must not already read as non-text: {measured}"
        );
    }

    /// The body must say what it cannot do, in the file itself, because that
    /// is the copy somebody reads when they find it and wonder what it is.
    #[test]
    fn the_planted_body_states_its_own_limits() {
        let body = contents(DEFAULT_NAME).to_lowercase();
        assert!(body.contains("cannot stop anything"));
        assert!(body.contains("reaches this folder"));
        assert!(body.contains("which program was responsible"));
    }

    #[test]
    fn a_nest_survives_a_round_trip_through_text() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, _) = nest_in(dir.path());
        let text = nest.to_text();
        let read_back = Nest::parse(&text).expect("its own output must parse");
        assert_eq!(nest, read_back);
        assert_eq!(
            read_back.to_text(),
            text,
            "and must round-trip byte for byte"
        );
    }

    #[test]
    fn a_nest_survives_a_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, _) = nest_in(dir.path());
        let file = dir.path().join("deeper").join("nest.txt");
        nest.save(&file).unwrap();
        assert_eq!(Nest::load(&file).unwrap(), nest);
    }

    #[test]
    fn a_windows_path_written_by_hand_reads_back_normalised() {
        let text = format!(
            "{MAGIC}\n{}  10  1700000000  C:\\Users\\somebody\\My Documents\\{DEFAULT_NAME}\n",
            "a".repeat(64)
        );
        let nest = Nest::parse(&text).expect("a hand-written record must parse");
        let canary = nest.canaries().next().unwrap();
        assert!(canary.path.contains('/'));
        assert!(!canary.path.contains('\\'));
        assert!(
            canary.path.contains("My Documents"),
            "a single space in a path must survive: {}",
            canary.path
        );
    }

    #[test]
    fn a_malformed_nest_is_refused_rather_than_half_read() {
        assert!(Nest::parse("").is_err(), "empty");
        assert!(Nest::parse("NOT-THE-MAGIC\n").is_err(), "wrong magic");
        for bad in [
            "zz  10  1  a.txt",
            &format!("{}  notanumber  1  a.txt", "a".repeat(64)),
            &format!("{}  10  notatime  a.txt", "a".repeat(64)),
            &format!("{}  10  1  ", "a".repeat(64)),
        ] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(Nest::parse(&text).is_err(), "should refuse: {bad}");
        }
    }

    #[test]
    fn blank_lines_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let (nest, _) = nest_in(dir.path());
        let padded = nest.to_text().replace('\n', "\n\n");
        assert_eq!(Nest::parse(&padded).unwrap(), nest);
    }

    /// An unreadable canary is not an absent one, and the difference is worth
    /// reporting: a permissions change is itself a thing that happened.
    #[test]
    fn a_state_that_could_not_be_read_says_so() {
        let state = State::Unreadable("permission denied".to_string());
        assert!(state.is_trip());
        assert!(!state.stopped_being_text());
        assert!(state.describe().contains("permission denied"));
    }

    #[test]
    fn intact_is_the_only_state_that_is_not_a_trip() {
        assert!(!State::Intact.is_trip());
        assert_eq!(State::Intact.describe(), "intact");
        assert!(State::Removed.is_trip());
    }
}
