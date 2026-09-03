// SPDX-License-Identifier: GPL-3.0-or-later
//! The two things VeilVoice insists on unless you say otherwise.
//!
//! # What this is
//!
//! By default VeilVoice requires **an app lock** and **encryption of every
//! recording at rest**. Both matter for the same reason: de-identification
//! removes the voiceprint but keeps the words, so a veiled recording is still a
//! recording of everything that was said, and an unlocked application sitting
//! open is still a window into what you have processed.
//!
//! So both are on, by default, without anybody choosing them. This module is
//! how you *stop* insisting on one or both -- a deliberate, recorded choice
//! rather than a setting that quietly drifts.
//!
//! # It is not the sealed policy, and the difference is the point
//!
//! [`crate::Policy`] is the sealed, administrator-set policy that can only ever
//! make VeilVoice **stricter** and cannot be weakened without the passphrase.
//! This is the opposite tool for the opposite person: it is *your own* baseline,
//! plainly stored, that you may relax. The two compose safely -- the effective
//! requirement is this baseline OR whatever the sealed policy adds -- so an
//! administrator can still force on something you turned off, and never the
//! other way round.
//!
//! # Why it keeps a history
//!
//! Turning off encryption or the app lock is exactly the kind of change someone
//! should be able to see was made, when, and away from what. So every change is
//! appended to a log with its timestamp and whether the value it left was the
//! default. Nothing here is secret -- it is your own record of your own
//! decisions -- so it is a plain file you can read.
//!
//! # In plain words
//!
//! VeilVoice asks for a password for itself and encrypts your recordings,
//! unless you deliberately turn one or both off. It remembers when you did, and
//! what it was before, so the choice is never a mystery later.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The magic on the first line, so a stray file is not mistaken for this one.
const MAGIC: &str = "VEILMANDATE1";

/// Which requirement a change concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The application lock.
    AppLock,
    /// Encryption of recordings at rest.
    Encryption,
}

impl Field {
    /// The word used in the file and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Self::AppLock => "app-lock",
            Self::Encryption => "encryption",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "app-lock" => Some(Self::AppLock),
            "encryption" => Some(Self::Encryption),
            _ => None,
        }
    }
}

/// One recorded change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Unix seconds when it was made.
    pub at: i64,
    /// Which requirement.
    pub field: Field,
    /// What it was.
    pub from: bool,
    /// What it became.
    pub to: bool,
}

/// The current requirements, and the log of how they got there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mandate {
    require_app_lock: bool,
    require_encryption: bool,
    history: Vec<Change>,
}

impl Default for Mandate {
    /// Both required. This is what a machine that has never been told otherwise
    /// insists on.
    fn default() -> Self {
        Self {
            require_app_lock: true,
            require_encryption: true,
            history: Vec::new(),
        }
    }
}

fn now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs().min(i64::MAX as u64) as i64,
        Err(e) => -(e.duration().as_secs().min(i64::MAX as u64) as i64),
    }
}

impl Mandate {
    /// Whether an app lock is required.
    pub fn requires_app_lock(&self) -> bool {
        self.require_app_lock
    }

    /// Whether encryption of recordings at rest is required.
    pub fn requires_encryption(&self) -> bool {
        self.require_encryption
    }

    /// The value of one field.
    pub fn requires(&self, field: Field) -> bool {
        match field {
            Field::AppLock => self.require_app_lock,
            Field::Encryption => self.require_encryption,
        }
    }

    /// Whether this is still the default: both required, nothing turned off.
    pub fn is_default(&self) -> bool {
        self.require_app_lock && self.require_encryption
    }

    /// The change log, oldest first.
    pub fn history(&self) -> &[Change] {
        &self.history
    }

    /// Set one requirement, recording the change if it is actually a change.
    ///
    /// Returns whether anything changed. Setting a field to the value it
    /// already has is a no-op and is not logged, so the history stays a record
    /// of real decisions rather than repeated commands.
    pub fn set(&mut self, field: Field, value: bool) -> bool {
        self.set_at(field, value, now())
    }

    fn set_at(&mut self, field: Field, value: bool, at: i64) -> bool {
        let slot = match field {
            Field::AppLock => &mut self.require_app_lock,
            Field::Encryption => &mut self.require_encryption,
        };
        if *slot == value {
            return false;
        }
        let from = *slot;
        *slot = value;
        self.history.push(Change {
            at,
            field,
            from,
            to: value,
        });
        true
    }

    /// Return to the default (both required), recording the changes.
    pub fn reset(&mut self) -> bool {
        self.reset_at(now())
    }

    fn reset_at(&mut self, at: i64) -> bool {
        let a = self.set_at(Field::AppLock, true, at);
        let b = self.set_at(Field::Encryption, true, at);
        a || b
    }

    /// Parse the file format.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        match lines.next().map(str::trim) {
            Some(MAGIC) => {}
            other => {
                return Err(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                ))
            }
        }
        let mut mandate = Mandate {
            require_app_lock: true,
            require_encryption: true,
            history: Vec::new(),
        };
        for (index, raw) in lines.enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let number = index + 2;
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["require", key, value] => {
                    let field = Field::from_key(key)
                        .ok_or_else(|| format!("line {number}: unknown requirement {key:?}"))?;
                    let on = parse_bool(value)
                        .ok_or_else(|| format!("line {number}: not a yes or no: {value:?}"))?;
                    match field {
                        Field::AppLock => mandate.require_app_lock = on,
                        Field::Encryption => mandate.require_encryption = on,
                    }
                }
                ["change", at, key, from, to] => {
                    let field = Field::from_key(key)
                        .ok_or_else(|| format!("line {number}: unknown requirement {key:?}"))?;
                    mandate.history.push(Change {
                        at: at.parse().map_err(|_| format!("line {number}: bad time"))?,
                        field,
                        from: parse_bool(from).ok_or_else(|| format!("line {number}: bad from"))?,
                        to: parse_bool(to).ok_or_else(|| format!("line {number}: bad to"))?,
                    });
                }
                _ => return Err(format!("line {number}: not understood: {line:?}")),
            }
        }
        Ok(mandate)
    }

    /// Render the file format.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        out.push_str(&format!(
            "require app-lock {}\n",
            yesno(self.require_app_lock)
        ));
        out.push_str(&format!(
            "require encryption {}\n",
            yesno(self.require_encryption)
        ));
        for change in &self.history {
            out.push_str(&format!(
                "change {} {} {} {}\n",
                change.at,
                change.field.key(),
                yesno(change.from),
                yesno(change.to)
            ));
        }
        out
    }

    /// Load from `path`, or the default if it is not there.
    ///
    /// A file that will not parse is an error rather than a silent default:
    /// silently defaulting would turn a corrupt file into "both required",
    /// which is the safe direction but hides that something is wrong. The
    /// caller decides what to do with the error; the desktop application shows
    /// it and keeps the strict default.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("could not read the mandate file: {e}")),
        }
    }

    /// Write to `path`, owner-only.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        veilvoice_crypto::privatefile::write_owner_only(path, self.to_text().as_bytes())
            .map_err(|e| e.to_string())
    }
}

/// Where the mandate file lives: beside the app lock, under its own name.
pub fn default_path() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("mandate.conf"))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "yes" | "true" | "on" | "1" => Some(true),
        "no" | "false" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn yesno(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_requires_both() {
        let m = Mandate::default();
        assert!(m.requires_app_lock());
        assert!(m.requires_encryption());
        assert!(m.is_default());
        assert!(m.history().is_empty());
    }

    #[test]
    fn turning_one_off_records_it() {
        let mut m = Mandate::default();
        assert!(m.set_at(Field::Encryption, false, 1000));
        assert!(!m.requires_encryption());
        assert!(m.requires_app_lock());
        assert!(!m.is_default());
        assert_eq!(m.history().len(), 1);
        assert_eq!(
            m.history()[0],
            Change {
                at: 1000,
                field: Field::Encryption,
                from: true,
                to: false
            }
        );
    }

    #[test]
    fn setting_a_field_to_what_it_already_is_changes_nothing() {
        let mut m = Mandate::default();
        assert!(!m.set_at(Field::AppLock, true, 1));
        assert!(m.history().is_empty());
    }

    #[test]
    fn reset_puts_both_back_and_logs_only_what_moved() {
        let mut m = Mandate::default();
        m.set_at(Field::AppLock, false, 10);
        m.set_at(Field::Encryption, false, 20);
        let reset = m.reset_at(30);
        assert!(reset);
        assert!(m.is_default());
        // Two off-changes, then two on-changes at reset.
        assert_eq!(m.history().len(), 4);
        assert!(m.history()[2..].iter().all(|c| c.to));
    }

    #[test]
    fn reset_from_default_does_nothing() {
        let mut m = Mandate::default();
        assert!(!m.reset_at(1));
        assert!(m.history().is_empty());
    }

    #[test]
    fn it_round_trips_through_its_text_format() {
        let mut m = Mandate::default();
        m.set_at(Field::AppLock, false, 111);
        m.set_at(Field::Encryption, false, 222);
        m.set_at(Field::Encryption, true, 333);
        let text = m.to_text();
        let back = Mandate::parse(&text).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn a_file_without_the_magic_is_refused() {
        assert!(Mandate::parse("require app-lock no\n").is_err());
    }

    #[test]
    fn a_missing_file_is_the_default_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let m = Mandate::load(&dir.path().join("nope.conf")).unwrap();
        assert!(m.is_default());
    }

    #[test]
    fn it_survives_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mandate.conf");
        let mut m = Mandate::default();
        m.set_at(Field::Encryption, false, 999);
        m.save(&path).unwrap();
        let back = Mandate::load(&path).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn a_corrupt_file_is_an_error_rather_than_a_silent_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mandate.conf");
        std::fs::write(&path, "garbage that is not a mandate").unwrap();
        assert!(Mandate::load(&path).is_err());
    }

    #[test]
    fn unknown_requirements_are_refused_rather_than_ignored() {
        assert!(Mandate::parse("VEILMANDATE1\nrequire telepathy no\n").is_err());
    }
}
