// SPDX-License-Identifier: GPL-3.0-or-later
//! The policy itself: what can be required, and what requiring it does.
//!
//! # Every requirement tightens, and there is nowhere to write one that does not
//!
//! [`Requirement`] has five variants and all five move VeilVoice in the same
//! direction. There is no `AllowPlaintext`, no `MaximumIntensity`, no
//! `SkipMetadataCleaning`. That is not an oversight to be filled in later; it
//! is the property the whole crate rests on, and
//! [`Posture::is_at_least_as_strict_as`] exists so a test can hold it.
//!
//! Anybody adding a variant should read [`crate`]'s documentation first. A
//! loosening variant does not merely add a feature — it removes the reason the
//! plain file can be read without a passphrase.
//!
//! # Format
//!
//! Text, one requirement per line, for the same reason the tamper manifest is
//! text: the point of the file is to be readable by the person it constrains.
//!
//! ```text
//! VEILPOLICY1
//! note  Set by the IT department. Ask before changing.
//! require  encrypt-recordings
//! require  clean-metadata
//! require  minimum-intensity  80
//! ```
//!
//! The floor is a whole number of hundredths, not a decimal. A policy has to
//! compare equal to its own sealed copy, and a value that reads back as
//! 0.7999999 would make [`crate::verify`] report `Differs` for ever.
//!
//! An unknown `require` keyword is an **error**, not a line to skip. A policy
//! written by a newer build says something this one cannot honour, and quietly
//! honouring the rest would leave the machine less restricted than the person
//! who wrote it believes. Refusing says so.
//!
//! # In plain words
//!
//! A way to say "these settings must always be on", so that they cannot be turned
//! off later by accident.
//!
//! Everything here only ever tightens. There is deliberately no way to write a
//! rule that makes VeilVoice do less, because a settings file that could weaken
//! the program would be the first thing worth attacking.
//!
//! If a rule and a control disagree, the rule wins and the window shows you the
//! value that will actually be used, rather than one that quietly changes when you
//! press the button.

use crate::Error;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Magic first line. The digit is a format version.
const MAGIC: &str = "VEILPOLICY1";

/// The plain policy, read at every launch and needing no passphrase.
pub const PLAIN_FILE: &str = "policy.txt";

/// The same policy sealed under a passphrase, for proving the plain one is
/// what was written.
pub const SEALED_FILE: &str = "policy.sealed";

/// One thing a policy can insist on.
///
/// **Every variant tightens.** See the module documentation before adding one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Requirement {
    /// Recordings must be encrypted at rest. The user may not turn it off.
    EncryptRecordings,
    /// Metadata must be stripped from what VeilVoice writes.
    CleanMetadata,
    /// Accent neutralisation must stay on.
    NeutraliseAccent,
    /// The app lock must be set before the program can be used.
    AppLock,
    /// The de-identification intensity has a floor.
    ///
    /// Stored as hundredths so the type stays `Ord` and the file round-trips
    /// exactly: a policy that reads back as 0.7999999 and then compares
    /// unequal to the one that was written is a policy nobody can verify.
    MinimumIntensity(u8),
}

impl Requirement {
    /// The keyword this is written as.
    pub fn keyword(&self) -> &'static str {
        match self {
            Requirement::EncryptRecordings => "encrypt-recordings",
            Requirement::CleanMetadata => "clean-metadata",
            Requirement::NeutraliseAccent => "neutralise-accent",
            Requirement::AppLock => "app-lock",
            Requirement::MinimumIntensity(_) => "minimum-intensity",
        }
    }

    /// What this means, in the words a front end should show beside the
    /// control it has taken away.
    ///
    /// A disabled control with no explanation is the thing people complain
    /// about; a disabled control with a reason is a decision somebody made.
    pub fn describe(&self) -> String {
        match self {
            Requirement::EncryptRecordings => {
                "Recordings are encrypted at rest, and that cannot be turned off here.".to_string()
            }
            Requirement::CleanMetadata => {
                "Metadata is stripped from what VeilVoice writes, and that cannot be \
                 turned off here."
                    .to_string()
            }
            Requirement::NeutraliseAccent => {
                "Accent neutralisation stays on. It removes the melody of an accent and \
                 not which sounds you make, so a strong accent may still be audible."
                    .to_string()
            }
            Requirement::AppLock => {
                "The app lock must be set before VeilVoice can be used. The lock is a \
                 verifier and not disk encryption."
                    .to_string()
            }
            Requirement::MinimumIntensity(hundredths) => format!(
                "De-identification intensity may not go below {:.2}.",
                *hundredths as f32 / 100.0
            ),
        }
    }
}

/// The settings a policy can reach, as a front end holds them.
///
/// Deliberately small. This is not a copy of the preferences file — it is the
/// subset a policy is allowed to constrain, which is the subset where being
/// *more* strict is never a loss.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Posture {
    /// Recordings are encrypted at rest.
    pub encrypt_recordings: bool,
    /// Metadata is stripped from what is written.
    pub clean_metadata: bool,
    /// Accent neutralisation is on.
    pub neutralise_accent: bool,
    /// The app lock is set.
    pub app_lock: bool,
    /// De-identification intensity, 0.0 to 1.0.
    pub intensity: f32,
}

impl Default for Posture {
    /// VeilVoice's own defaults, which are the strict ones.
    fn default() -> Self {
        Self {
            encrypt_recordings: true,
            clean_metadata: true,
            neutralise_accent: true,
            app_lock: false,
            intensity: 1.0,
        }
    }
}

impl Posture {
    /// The most permissive arrangement the controls can reach.
    ///
    /// Not a default anybody gets. It exists so a test can start from the
    /// loosest possible state and prove that applying a policy never loosens
    /// it further.
    pub fn most_permissive() -> Self {
        Self {
            encrypt_recordings: false,
            clean_metadata: false,
            neutralise_accent: false,
            app_lock: false,
            intensity: 0.0,
        }
    }

    /// Whether `self` is at least as strict as `other` in every dimension.
    ///
    /// The property the whole crate rests on: `policy.constrain(p)` must always
    /// be at least as strict as `p`, for every policy and every `p`.
    pub fn is_at_least_as_strict_as(&self, other: &Posture) -> bool {
        (self.encrypt_recordings || !other.encrypt_recordings)
            && (self.clean_metadata || !other.clean_metadata)
            && (self.neutralise_accent || !other.neutralise_accent)
            && (self.app_lock || !other.app_lock)
            && self.intensity >= other.intensity
    }
}

/// A set of requirements, and an optional note from whoever wrote them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    /// A `BTreeSet` so the serialised form is byte-identical for the same set.
    /// A policy that serialises differently on each save cannot be compared
    /// against its own sealed copy, which is the only thing the seal is for.
    requirements: BTreeSet<Requirement>,
    note: Option<String>,
}

impl Policy {
    /// A policy that requires nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a requirement.
    pub fn require(&mut self, requirement: Requirement) -> &mut Self {
        // A second floor replaces the first rather than sitting beside it: two
        // `MinimumIntensity` entries in one set would serialise as two lines
        // and the higher would silently win, which is a policy file that does
        // not say what it does.
        if let Requirement::MinimumIntensity(_) = requirement {
            self.requirements
                .retain(|held| !matches!(held, Requirement::MinimumIntensity(_)));
        }
        self.requirements.insert(requirement);
        self
    }

    /// Set the note shown beside every control the policy has fixed.
    ///
    /// Line breaks are refused rather than escaped: the format is one record
    /// per line, and a note containing a newline could forge a `require` line.
    pub fn with_note(mut self, note: &str) -> Result<Self, Error> {
        if note.contains('\n') || note.contains('\r') {
            return Err(Error::Malformed(
                "a note may not contain a line break: it would be able to forge a \
                 requirement line"
                    .into(),
            ));
        }
        self.note = if note.trim().is_empty() {
            None
        } else {
            Some(note.trim().to_string())
        };
        Ok(self)
    }

    /// The note, if there is one.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Whether anything at all is required.
    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    /// How many requirements there are.
    pub fn len(&self) -> usize {
        self.requirements.len()
    }

    /// The requirements, in a stable order.
    pub fn requirements(&self) -> impl Iterator<Item = &Requirement> {
        self.requirements.iter()
    }

    /// Whether a particular requirement is in force.
    pub fn requires(&self, requirement: &Requirement) -> bool {
        self.requirements.contains(requirement)
    }

    /// The intensity floor, or 0.0 when none is set.
    pub fn minimum_intensity(&self) -> f32 {
        self.requirements
            .iter()
            .filter_map(|requirement| match requirement {
                Requirement::MinimumIntensity(hundredths) => Some(*hundredths as f32 / 100.0),
                _ => None,
            })
            .fold(0.0f32, f32::max)
    }

    /// Apply the policy to a posture.
    ///
    /// Only ever tightens. The test suite holds that as a property across every
    /// subset of requirements and a range of postures, rather than trusting the
    /// five lines below to keep saying what they say today.
    pub fn constrain(&self, mut posture: Posture) -> Posture {
        if self.requires(&Requirement::EncryptRecordings) {
            posture.encrypt_recordings = true;
        }
        if self.requires(&Requirement::CleanMetadata) {
            posture.clean_metadata = true;
        }
        if self.requires(&Requirement::NeutraliseAccent) {
            posture.neutralise_accent = true;
        }
        if self.requires(&Requirement::AppLock) {
            posture.app_lock = true;
        }
        let floor = self.minimum_intensity();
        if posture.intensity < floor {
            posture.intensity = floor;
        }
        posture
    }

    /// Serialise to the text format described at the top of this module.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        if let Some(note) = &self.note {
            out.push_str(&format!("note  {note}\n"));
        }
        for requirement in &self.requirements {
            match requirement {
                Requirement::MinimumIntensity(hundredths) => {
                    out.push_str(&format!(
                        "require  {}  {}\n",
                        requirement.keyword(),
                        hundredths
                    ));
                }
                other => out.push_str(&format!("require  {}\n", other.keyword())),
            }
        }
        out
    }

    /// Parse the text format.
    ///
    /// An unrecognised requirement is refused. See the module note: honouring
    /// the rest of a policy this build does not fully understand leaves the
    /// machine less restricted than whoever wrote it believes.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the policy file is empty".into())),
        }

        let mut policy = Policy::new();
        for (index, line) in lines.enumerate() {
            let number = index + 2;
            if line.trim().is_empty() {
                continue;
            }
            let Some((keyword, rest)) = line.split_once("  ") else {
                return Err(Error::Malformed(format!(
                    "line {number}: no keyword, found {line:?}"
                )));
            };
            match keyword {
                "note" => policy.note = Some(rest.trim().to_string()),
                "require" => {
                    let (name, argument) = match rest.split_once("  ") {
                        Some((name, argument)) => (name.trim(), Some(argument.trim())),
                        None => (rest.trim(), None),
                    };
                    policy.require(requirement_from(name, argument, number)?);
                }
                other => {
                    return Err(Error::Malformed(format!(
                        "line {number}: unknown keyword {other:?}"
                    )))
                }
            }
        }
        Ok(policy)
    }

    /// Seal the policy under a passphrase.
    ///
    /// The sealed copy is what proves the plain one is the policy that was
    /// written. It is not what makes the policy apply — see [`crate`].
    pub fn seal(&self, password: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(veilvoice_crypto::container::seal_with_password(
            password,
            self.to_text().as_bytes(),
            veilvoice_crypto::kdf::KdfParams::default(),
        )?)
    }

    /// Open a policy sealed by [`Policy::seal`].
    ///
    /// F-92, the third place the same question comes up. The generous
    /// four-gigabyte ceiling is for a container somebody was sent and chose to
    /// open. This file is not that: it sits at a fixed path beside the policy,
    /// and the person running `veilvoice policy verify` chose the command, not
    /// the file. [`Policy::seal`] writes it at this crate's default cost, so a
    /// ceiling of one gigabyte leaves four times the headroom anything
    /// legitimate needs and refuses a planted file instead of allocating for
    /// it.
    ///
    /// Changed at the same time as the sealed manifest, deliberately. Fixing
    /// the two places a campaign happened to point at and leaving the third
    /// would be the exclusion list that names the files somebody thought of.
    pub fn open_sealed(container: &[u8], password: &[u8]) -> Result<Self, Error> {
        let bytes = veilvoice_crypto::container::open_with_password_within(
            password,
            container,
            veilvoice_crypto::kdf::KdfParams::UNATTENDED_MAX_M_COST,
        )?;
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::Malformed("the sealed policy is not text".into()))?;
        Self::parse(&text)
    }

    /// Write the plain policy into `dir`, and the sealed copy beside it.
    ///
    /// Both, always. A plain file with no sealed copy beside it is a policy
    /// nobody can check, and writing one silently would make [`verify`]'s
    /// [`Verification::NotSealed`] indistinguishable from a sealed copy that
    /// somebody deleted.
    pub fn save(&self, dir: &Path, password: &[u8]) -> Result<(), Error> {
        std::fs::create_dir_all(dir)?;
        let sealed = self.seal(password)?;
        // The sealed copy first. If the second write fails, what is left is a
        // sealed copy with no plain one -- nothing is applied, and `verify`
        // says the plain file is missing. The other order leaves a policy in
        // force that nobody can check, which is the state this crate exists to
        // avoid.
        std::fs::write(dir.join(SEALED_FILE), &sealed)?;
        std::fs::write(dir.join(PLAIN_FILE), self.to_text())?;
        Ok(())
    }

    /// Read the plain policy from `dir`. Never asks for a passphrase.
    ///
    /// `Ok(None)` when there is no policy at all, which is the ordinary state
    /// and not an error.
    pub fn load(dir: &Path) -> Result<Option<Self>, Error> {
        match std::fs::read_to_string(dir.join(PLAIN_FILE)) {
            Ok(text) => Ok(Some(Self::parse(&text)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io(error)),
        }
    }
}

fn requirement_from(name: &str, argument: Option<&str>, line: usize) -> Result<Requirement, Error> {
    match name {
        "encrypt-recordings" => Ok(Requirement::EncryptRecordings),
        "clean-metadata" => Ok(Requirement::CleanMetadata),
        "neutralise-accent" => Ok(Requirement::NeutraliseAccent),
        "app-lock" => Ok(Requirement::AppLock),
        "minimum-intensity" => {
            let argument = argument.ok_or_else(|| {
                Error::Malformed(format!("line {line}: minimum-intensity needs a value"))
            })?;
            let hundredths: u8 = argument.parse().map_err(|_| {
                Error::Malformed(format!(
                    "line {line}: minimum-intensity is a whole number of hundredths from \
                     0 to 100, not {argument:?}"
                ))
            })?;
            if hundredths > 100 {
                return Err(Error::Malformed(format!(
                    "line {line}: minimum-intensity is at most 100, not {hundredths}"
                )));
            }
            Ok(Requirement::MinimumIntensity(hundredths))
        }
        other => Err(Error::Malformed(format!(
            "line {line}: this build does not understand the requirement {other:?}. \
             Refusing the whole policy rather than honouring part of it."
        ))),
    }
}

/// What is known about the seal on a policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verification {
    /// Nobody has offered a passphrase, so the seal has not been looked at.
    /// This is what [`Policy::load`] leaves behind, and it is not a complaint.
    Unchecked,
    /// The sealed copy opened and matches the plain one.
    Matches,
    /// The sealed copy opened and says something different from the plain one.
    /// Somebody edited the plain file.
    Differs {
        /// The policy that was actually sealed.
        sealed: Box<Policy>,
    },
    /// There is a plain policy and no sealed copy beside it.
    NotSealed,
    /// There is a sealed copy and no plain policy, so nothing is being applied.
    NotApplied,
}

impl Verification {
    /// One line for a front end. Says what is known, never more.
    pub fn describe(&self) -> String {
        match self {
            Verification::Unchecked => {
                "in force, seal not checked -- a policy can only make VeilVoice stricter, \
                 so it is applied before anybody checks"
                    .to_string()
            }
            Verification::Matches => "in force, and it is the policy that was sealed".to_string(),
            Verification::Differs { .. } => {
                "in force, and it is NOT the policy that was sealed: the plain file has \
                 been edited"
                    .to_string()
            }
            Verification::NotSealed => {
                "in force, and there is no sealed copy to check it against".to_string()
            }
            Verification::NotApplied => {
                "a sealed policy exists and the plain file is missing, so nothing is \
                 being applied"
                    .to_string()
            }
        }
    }

    /// Whether this is a state somebody should look at.
    pub fn wants_attention(&self) -> bool {
        matches!(
            self,
            Verification::Differs { .. } | Verification::NotSealed | Verification::NotApplied
        )
    }
}

/// Check the plain policy in `dir` against its sealed copy.
///
/// This is the only function here that needs a passphrase, and nothing calls it
/// at launch.
pub fn verify(dir: &Path, password: &[u8]) -> Result<Verification, Error> {
    let plain = Policy::load(dir)?;
    let sealed_path: PathBuf = dir.join(SEALED_FILE);
    let sealed_bytes = match std::fs::read(&sealed_path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(Error::Io(error)),
    };

    match (plain, sealed_bytes) {
        (None, None) => Ok(Verification::NotSealed),
        (Some(_), None) => Ok(Verification::NotSealed),
        (None, Some(_)) => Ok(Verification::NotApplied),
        (Some(plain), Some(bytes)) => {
            let sealed = Policy::open_sealed(&bytes, password)?;
            if sealed == plain {
                Ok(Verification::Matches)
            } else {
                Ok(Verification::Differs {
                    sealed: Box::new(sealed),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cheap KDF for the tests. The default is 256 MiB and three passes,
    /// deliberately, and running it in twenty tests would make the suite take
    /// minutes for no extra coverage of anything in this crate.
    fn sealed_with(policy: &Policy, password: &[u8]) -> Vec<u8> {
        veilvoice_crypto::container::seal_with_password(
            password,
            policy.to_text().as_bytes(),
            veilvoice_crypto::kdf::KdfParams {
                m_cost: 8,
                t_cost: 1,
                p_cost: 1,
            },
        )
        .expect("sealing should work")
    }

    fn every_requirement() -> Vec<Requirement> {
        vec![
            Requirement::EncryptRecordings,
            Requirement::CleanMetadata,
            Requirement::NeutraliseAccent,
            Requirement::AppLock,
            Requirement::MinimumIntensity(80),
        ]
    }

    /// **The property the crate rests on.** Every subset of every requirement,
    /// against a range of postures: applying a policy must never loosen one.
    ///
    /// Written as an exhaustive check rather than an example, because the
    /// interesting failure is a variant somebody adds later that goes the
    /// other way, and no example test would catch it.
    #[test]
    fn a_policy_can_only_ever_tighten() {
        let all = every_requirement();
        let postures = [
            Posture::most_permissive(),
            Posture::default(),
            Posture {
                encrypt_recordings: false,
                clean_metadata: true,
                neutralise_accent: false,
                app_lock: true,
                intensity: 0.5,
            },
            Posture {
                intensity: 1.0,
                ..Posture::most_permissive()
            },
        ];
        for mask in 0..(1u32 << all.len()) {
            let mut policy = Policy::new();
            for (index, requirement) in all.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    policy.require(*requirement);
                }
            }
            for posture in postures {
                let after = policy.constrain(posture);
                assert!(
                    after.is_at_least_as_strict_as(&posture),
                    "mask {mask:b} loosened {posture:?} into {after:?}"
                );
            }
        }
    }

    /// Applying a policy twice must give the same answer as applying it once,
    /// or a front end that constrains on every frame drifts.
    #[test]
    fn constraining_is_idempotent() {
        let mut policy = Policy::new();
        for requirement in every_requirement() {
            policy.require(requirement);
        }
        let once = policy.constrain(Posture::most_permissive());
        assert_eq!(policy.constrain(once), once);
    }

    #[test]
    fn an_empty_policy_changes_nothing() {
        let policy = Policy::new();
        assert!(policy.is_empty());
        assert_eq!(policy.len(), 0);
        let posture = Posture::most_permissive();
        assert_eq!(policy.constrain(posture), posture);
        assert_eq!(policy.minimum_intensity(), 0.0);
    }

    #[test]
    fn a_floor_raises_a_low_intensity_and_leaves_a_high_one() {
        let mut policy = Policy::new();
        policy.require(Requirement::MinimumIntensity(80));
        assert_eq!(policy.minimum_intensity(), 0.8);
        let raised = policy.constrain(Posture {
            intensity: 0.1,
            ..Posture::most_permissive()
        });
        assert!((raised.intensity - 0.8).abs() < 1e-6);
        let untouched = policy.constrain(Posture {
            intensity: 1.0,
            ..Posture::most_permissive()
        });
        assert_eq!(untouched.intensity, 1.0);
    }

    /// Two floors in one policy would serialise as two lines and the higher
    /// would silently win, which is a file that does not say what it does.
    #[test]
    fn a_second_floor_replaces_the_first() {
        let mut policy = Policy::new();
        policy.require(Requirement::MinimumIntensity(50));
        policy.require(Requirement::MinimumIntensity(90));
        assert_eq!(policy.len(), 1);
        assert_eq!(policy.minimum_intensity(), 0.9);
        assert_eq!(policy.to_text().matches("minimum-intensity").count(), 1);

        // And lowering it is possible for whoever writes the policy -- the
        // one-way property is about the *user's* controls, not about the
        // administrator's ability to write a weaker policy on purpose.
        policy.require(Requirement::MinimumIntensity(10));
        assert_eq!(policy.minimum_intensity(), 0.1);
    }

    #[test]
    fn a_policy_survives_a_round_trip_through_text() {
        let mut policy = Policy::new();
        policy.require(Requirement::EncryptRecordings);
        policy.require(Requirement::MinimumIntensity(75));
        let policy = policy
            .clone()
            .with_note("Set by whoever set it. Ask before changing.")
            .unwrap();
        let text = policy.to_text();
        let read_back = Policy::parse(&text).expect("its own output must parse");
        assert_eq!(policy, read_back);
        assert_eq!(read_back.to_text(), text, "and byte for byte");
        assert_eq!(read_back.note(), policy.note());
    }

    /// The floor must come back exactly, or a policy can never equal its own
    /// sealed copy and `verify` says `Differs` for ever.
    #[test]
    fn every_floor_round_trips_exactly() {
        for hundredths in 0..=100u8 {
            let mut policy = Policy::new();
            policy.require(Requirement::MinimumIntensity(hundredths));
            let read_back = Policy::parse(&policy.to_text()).unwrap();
            assert_eq!(policy, read_back, "{hundredths} did not survive");
        }
    }

    #[test]
    fn a_note_may_not_contain_a_line_break() {
        let error = Policy::new()
            .with_note("harmless\nrequire  app-lock")
            .expect_err("a note that can forge a line must be refused");
        assert!(error.to_string().contains("line break"));
        // An empty note is simply no note.
        assert_eq!(Policy::new().with_note("   ").unwrap().note(), None);
    }

    /// A policy this build only half understands would leave the machine less
    /// restricted than whoever wrote it believes.
    #[test]
    fn an_unknown_requirement_refuses_the_whole_policy() {
        let text =
            format!("{MAGIC}\nrequire  encrypt-recordings\nrequire  something-from-the-future\n");
        let error = Policy::parse(&text).expect_err("must refuse");
        assert!(error.to_string().contains("does not understand"), "{error}");
        assert!(
            error.to_string().contains("rather than honouring part"),
            "the reason must be in the message: {error}"
        );
    }

    #[test]
    fn a_malformed_policy_is_refused_rather_than_half_read() {
        assert!(Policy::parse("").is_err(), "empty");
        assert!(Policy::parse("NOT-THE-MAGIC\n").is_err(), "wrong magic");
        for bad in [
            "require  minimum-intensity",
            "require  minimum-intensity  ten",
            "require  minimum-intensity  101",
            "require  minimum-intensity  -1",
            "whatever  1",
            "nokeyword",
        ] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(Policy::parse(&text).is_err(), "should refuse: {bad:?}");
        }
    }

    #[test]
    fn blank_lines_are_tolerated() {
        let mut policy = Policy::new();
        policy.require(Requirement::AppLock);
        let padded = policy.to_text().replace('\n', "\n\n");
        assert_eq!(Policy::parse(&padded).unwrap(), policy);
    }

    #[test]
    fn a_sealed_policy_opens_with_the_right_passphrase_and_not_the_wrong_one() {
        let mut policy = Policy::new();
        policy.require(Requirement::EncryptRecordings);
        let container = sealed_with(&policy, b"correct horse");
        assert_eq!(
            Policy::open_sealed(&container, b"correct horse").unwrap(),
            policy
        );
        assert!(Policy::open_sealed(&container, b"wrong horse").is_err());
    }

    #[test]
    fn verification_reports_a_match() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = Policy::new();
        policy.require(Requirement::CleanMetadata);
        std::fs::write(dir.path().join(SEALED_FILE), sealed_with(&policy, b"pw")).unwrap();
        std::fs::write(dir.path().join(PLAIN_FILE), policy.to_text()).unwrap();
        assert_eq!(verify(dir.path(), b"pw").unwrap(), Verification::Matches);
        assert!(!Verification::Matches.wants_attention());
    }

    /// The case the seal exists for: somebody edited the plain file.
    #[test]
    fn verification_reports_an_edited_plain_file_and_says_what_was_sealed() {
        let dir = tempfile::tempdir().unwrap();
        let mut sealed_policy = Policy::new();
        sealed_policy.require(Requirement::EncryptRecordings);
        sealed_policy.require(Requirement::MinimumIntensity(90));
        std::fs::write(
            dir.path().join(SEALED_FILE),
            sealed_with(&sealed_policy, b"pw"),
        )
        .unwrap();

        let mut edited = Policy::new();
        edited.require(Requirement::EncryptRecordings);
        std::fs::write(dir.path().join(PLAIN_FILE), edited.to_text()).unwrap();

        match verify(dir.path(), b"pw").unwrap() {
            Verification::Differs { sealed } => {
                assert_eq!(*sealed, sealed_policy);
                assert_eq!(sealed.minimum_intensity(), 0.9);
            }
            other => panic!("expected Differs, got {other:?}"),
        }
        assert!(Verification::Differs {
            sealed: Box::new(sealed_policy)
        }
        .wants_attention());
    }

    #[test]
    fn a_plain_policy_with_no_seal_is_reported_as_unsealed() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = Policy::new();
        policy.require(Requirement::AppLock);
        std::fs::write(dir.path().join(PLAIN_FILE), policy.to_text()).unwrap();
        let checked = verify(dir.path(), b"pw").unwrap();
        assert_eq!(checked, Verification::NotSealed);
        assert!(checked.wants_attention());
    }

    /// A sealed copy with the plain file deleted means nothing is applied, and
    /// that is a different state from "no policy here".
    #[test]
    fn a_seal_with_no_plain_file_says_nothing_is_being_applied() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Policy::new();
        std::fs::write(dir.path().join(SEALED_FILE), sealed_with(&policy, b"pw")).unwrap();
        let checked = verify(dir.path(), b"pw").unwrap();
        assert_eq!(checked, Verification::NotApplied);
        assert!(checked.wants_attention());
        assert!(checked.describe().contains("nothing is being applied"));
    }

    #[test]
    fn no_policy_at_all_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Policy::load(dir.path()).unwrap(), None);
    }

    /// `save` writes both files, always.
    #[test]
    fn saving_writes_both_the_plain_and_the_sealed_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = Policy::new();
        policy.require(Requirement::EncryptRecordings);
        // The real `save` uses the default 256 MiB KDF, which is the right
        // cost for a file somebody seals once. It is exercised here exactly
        // once rather than in every test.
        policy.save(dir.path(), b"pw").unwrap();
        assert!(dir.path().join(PLAIN_FILE).exists());
        assert!(dir.path().join(SEALED_FILE).exists());
        assert_eq!(Policy::load(dir.path()).unwrap().unwrap(), policy);
        assert_eq!(verify(dir.path(), b"pw").unwrap(), Verification::Matches);
    }

    /// Loading never asks for a passphrase, so what it can say about the seal
    /// is nothing -- and it says exactly that.
    #[test]
    fn the_unchecked_state_explains_why_it_is_still_applied() {
        let text = Verification::Unchecked.describe();
        assert!(text.contains("seal not checked"), "{text}");
        assert!(text.contains("only make VeilVoice stricter"), "{text}");
        assert!(
            !Verification::Unchecked.wants_attention(),
            "an unchecked seal is the ordinary state, not a complaint"
        );
    }

    /// Every requirement explains itself beside the control it disables, and
    /// none of them overstates what VeilVoice does.
    #[test]
    fn every_requirement_explains_itself_without_overclaiming() {
        for requirement in every_requirement() {
            let text = requirement.describe();
            assert!(!text.trim().is_empty(), "{requirement:?}");
            assert!(!requirement.keyword().is_empty());
            for boast in ["tamper-proof", "unbreakable", "cannot be bypassed"] {
                assert!(!text.to_lowercase().contains(boast), "{text}");
            }
        }
        // The two requirements over things with documented limits must repeat
        // those limits rather than implying the requirement removes them.
        assert!(Requirement::NeutraliseAccent
            .describe()
            .contains("may still be audible"));
        assert!(Requirement::AppLock
            .describe()
            .contains("not disk encryption"));
    }

    #[test]
    fn keywords_are_unique_and_match_what_parses() {
        for requirement in every_requirement() {
            let argument = match requirement {
                Requirement::MinimumIntensity(hundredths) => Some(hundredths.to_string()),
                _ => None,
            };
            let parsed = requirement_from(requirement.keyword(), argument.as_deref(), 1).unwrap();
            assert_eq!(parsed, requirement);
        }
    }

    #[test]
    fn the_strictness_comparison_is_honest_in_both_directions() {
        let loose = Posture::most_permissive();
        let strict = Posture {
            encrypt_recordings: true,
            clean_metadata: true,
            neutralise_accent: true,
            app_lock: true,
            intensity: 1.0,
        };
        assert!(strict.is_at_least_as_strict_as(&loose));
        assert!(!loose.is_at_least_as_strict_as(&strict));
        assert!(loose.is_at_least_as_strict_as(&loose));
        assert!(strict.is_at_least_as_strict_as(&strict));
    }
}
