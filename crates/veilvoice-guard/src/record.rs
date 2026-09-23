// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the integrity record is kept.
//!
//! Three lines of arithmetic over a path, and they are here rather than in the
//! programs that use them because there are now three of those. The command
//! line's `veilvoice guard` worked it out, the desktop application's
//! integrity tab worked it out again, and roadmap item 179's updater needed it
//! a third time: it re-takes the record after it has replaced the
//! installation, because a new installation's files are new and the old record
//! would report every one of them as changed.
//!
//! A third copy would have been the point at which the two existing ones
//! started to drift, and a record written to one path and checked at another
//! is a check that passes for the wrong reason. So the callers delegate and
//! this is the definition.
//!
//! # Why it sits beside the app lock
//!
//! Because the sealed form of the record is sealed under the app lock's own
//! passphrase, and that passphrase exists for exactly as long as an unlock
//! does. Keeping the two files in one directory means one place to look, one
//! place to back up and one place to delete, and it means [`record_path`] can
//! be derived from a location the rest of the project already knows how to
//! find rather than invented here.
//!
//! # In plain words
//!
//! The list of what VeilVoice's own files should look like is kept next to your
//! app lock, and this is the one piece of code that decides where that is.

use std::path::{Path, PathBuf};

/// The file name the record is written under, plain.
pub const RECORD: &str = "integrity.manifest";

/// Where the record is kept, or `None` on a platform that offers nowhere.
///
/// `None` is a real answer rather than a failure to try: it means this system
/// gave no `APPDATA`, no `XDG_CONFIG_HOME` and no `HOME`, and a caller must
/// say so rather than falling back to the working directory. A record written
/// beside whatever the user happened to be standing in is a record nothing
/// will find again.
pub fn record_path() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name(RECORD))
}

/// The sealed record, which sits beside the plain one under the container
/// suffix.
///
/// Taken as an argument rather than derived from [`record_path`] so that a
/// caller who was handed an explicit location, which the command line's
/// `--path` does, gets the sealed name beside **that** file and not beside the
/// default one.
pub fn sealed_path(plain: &Path) -> PathBuf {
    veilvoice_crypto::container::veil_path(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sealed record must be a different file from the plain one, and must
    /// sit in the same directory.
    ///
    /// Both halves matter. A sealed record written over the plain one would
    /// destroy the thing it was derived from; a sealed record written into
    /// another directory would be missed by a caller that looks for the two
    /// side by side, and that caller prefers the sealed one precisely so that
    /// dropping a plain record beside it cannot downgrade the check.
    #[test]
    fn the_sealed_record_is_a_separate_file_in_the_same_directory() {
        let plain = Path::new("/somewhere/config/integrity.manifest");
        let sealed = sealed_path(plain);
        assert_ne!(sealed, plain);
        assert_eq!(sealed.parent(), plain.parent());
    }

    /// The record goes beside the app lock, under the name every front end
    /// reads.
    ///
    /// Skipped rather than failed where this platform names no configuration
    /// directory, because that is a fact about the machine the test is running
    /// on and not about the code.
    #[test]
    fn the_record_sits_beside_the_app_lock() {
        let Some(record) = record_path() else {
            return;
        };
        let lock = veilvoice_crypto::lock::default_path()
            .expect("a lock path, since there is a record path");
        assert_eq!(record.parent(), lock.parent());
        assert_eq!(record.file_name().unwrap(), RECORD);
    }
}
