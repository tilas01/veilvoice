// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding files that were sealed under no passphrase at all, and sealing them
//! again under a real one.
//!
//! # The fault this repairs
//!
//! Sealing with the app lock became the default in VeilVoice 0.1.15. The
//! window captured the app-lock passphrase as the lock opened, but wiped the
//! field before the sealing step collected it, so what reached
//! [`container::seal_with_password`] was an empty slice. Every release from
//! 0.1.15 to 0.1.22 could therefore write a recording, and the integrity
//! record beside it, sealed under nothing.
//!
//! Such a file looks exactly like any other sealed file. It carries the magic,
//! a real salt, real Argon2id costs and a valid tag, and it opens for anybody
//! who passes an empty passphrase, which is to say for anybody at all. Nothing
//! in it is corrupt, so nothing reports it; the only way to know is to try the
//! empty passphrase and see whether the tag verifies. That is what this module
//! does.
//!
//! # It looks before it touches
//!
//! [`look`] answers for one file and [`sweep`] for a folder, and neither
//! changes anything. [`reseal`] is the half that writes, and it takes one path
//! at a time, so what gets rewritten is a file somebody has seen named.
//!
//! Judging a file costs one Argon2id derivation, but only for files that are
//! actually VeilVoice password containers: anything else is refused by the
//! header parse, which happens before any memory is asked for. A folder of
//! ordinary files is swept in the time it takes to read them.
//!
//! The derivation is capped at
//! [`UNATTENDED_MAX_M_COST`](kdf::KdfParams::UNATTENDED_MAX_M_COST) rather
//! than the ordinary ceiling, because a sweep is handed whatever is in the
//! folder rather than a file somebody chose. A container declaring an absurd
//! memory cost is reported as [`Standing::Unreadable`] instead of being
//! allowed to decide how much memory this process asks the operating system
//! for.
//!
//! # It refuses anything it did not find open
//!
//! [`reseal`] rewrites a file **only** when that file opens with an empty
//! passphrase, and reports [`Error::NotSealedUnderNothing`] otherwise. The
//! refusal is the safety property, not a convenience: a repair that rewrote a
//! file already sealed under a real passphrase would be destroying the
//! recording it was asked to protect, and it would be doing so to the files
//! that were never affected. It also makes the tool safe to run twice, which
//! matters because the first run may stop part-way through a folder.
//!
//! The new container is opened again with the new passphrase and compared
//! against what came out of the old one, before anything is written. Only then
//! does [`crate::privatefile::replace_owner_only`] put it in place, as a write
//! beside the file followed by a rename, so an interrupted repair leaves the
//! original rather than half of a replacement.
//!
//! # What it cannot do
//!
//! It only knows about folders it is given. A recording exported somewhere
//! else, copied to another machine, or already sent to somebody is outside
//! anything this can reach, and a file that was sealed under nothing and has
//! since been read by somebody else stays read: resealing closes the exposure
//! from now on, it does not undo it. [`WARNING`] is the wording that says so,
//! kept here so that the window and the command line say the same thing.
//!
//! # In plain words
//!
//! Some versions of VeilVoice locked your recordings with a blank password by
//! mistake, which means they are not really locked at all. This finds those
//! files and locks them properly with your real password.
//!
//! It checks each file by trying the blank password on it. If the file does not
//! open that way it is already locked properly and this leaves it completely
//! alone. Nothing is sent anywhere, and your recordings do not change: the same
//! audio comes out, under a lock that now needs your password.

use crate::{container, kdf, privatefile, Error};
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

/// The one line that points at the repair, for somewhere a paragraph does not
/// fit.
///
/// `veilvoice lock status` prints it, because somebody who has an app lock is
/// exactly who the fault could have reached, and checking the lock is where
/// they look. It says what to run rather than attempting the check itself: the
/// check costs one Argon2id derivation per sealed file, which is not what a
/// status command should spend somebody's wait on.
pub const NOTICE: &str = "Recordings sealed with the app lock by VeilVoice 0.1.15 to 0.1.22 may have been sealed under an empty passphrase. Run `veilvoice reseal` to check this folder and repair what it finds.";

/// What to tell somebody running VeilVoice 0.1.23 for the first time.
///
/// One copy, named by both front ends, because a warning the window and the
/// command line word differently is a warning somebody reads twice and
/// believes once.
pub const WARNING: &str = "\
Recordings sealed with the app lock by VeilVoice 0.1.15 to 0.1.22 may have been \
sealed under an empty passphrase. A file in that state opens for anybody who \
has it, whatever your app-lock passphrase is.\n\
\n\
Run `veilvoice reseal` to find those files and seal them again under your \
app-lock passphrase. It reads your recordings and writes them back in place; \
nothing is sent anywhere and nothing that was sealed properly is touched.\n\
\n\
Resealing protects a file from now on. It cannot undo a copy somebody already \
has.";

/// Where one file stands with respect to the empty passphrase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    /// A password container that opens with an empty passphrase. This is the
    /// fault, and this is the only thing [`reseal`] will rewrite.
    OpensWithNothing,
    /// A password container that does not open with an empty passphrase, so it
    /// is sealed under a real one and there is nothing here to repair.
    Sealed,
    /// Sealed to a public key rather than to a passphrase, by
    /// [`container::seal_to_public_key`]. The fault could not reach these:
    /// there is no passphrase in that path to be empty.
    PublicKey,
    /// Not a VeilVoice container. Settings, a log, a stray download, anything.
    NotAContainer,
    /// A container that could not be judged, with the reason. The file is left
    /// alone and reported, rather than counted as safe, because "this one
    /// could not be checked" is the half of the answer somebody has to act on
    /// themselves.
    Unreadable(String),
}

impl Standing {
    /// Whether [`reseal`] would rewrite a file in this state.
    pub fn needs_repair(&self) -> bool {
        matches!(self, Self::OpensWithNothing)
    }
}

/// One file a sweep looked at, and what it found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Where the file is.
    pub path: PathBuf,
    /// What trying the empty passphrase on it said.
    pub standing: Standing,
}

/// Judge the bytes of a file.
///
/// Separated from [`look`] so that a caller holding a container it has not
/// written to disk, such as the window checking what it is about to save, can
/// ask the same question of it.
pub fn standing(bytes: &[u8]) -> Standing {
    match container::open_with_password_within(b"", bytes, kdf::KdfParams::UNATTENDED_MAX_M_COST) {
        Ok(mut plaintext) => {
            // Opened only to learn that it opens. The audio itself is nobody's
            // business here, least of all a report's.
            plaintext.zeroize();
            Standing::OpensWithNothing
        }
        Err(Error::Decrypt) => Standing::Sealed,
        Err(Error::WrongMode) => Standing::PublicKey,
        Err(
            Error::BadMagic
            | Error::BadHeader
            | Error::Truncated
            | Error::UnsupportedVersion(_)
            | Error::UnsupportedMode(_),
        ) => Standing::NotAContainer,
        Err(other) => Standing::Unreadable(other.to_string()),
    }
}

/// Judge one file on disk.
///
/// A symbolic link is reported rather than followed, for the reason
/// [`Error::ResealSymlink`] gives: the repair replaces a file in place, and
/// the thing a link points at is not the thing a sweep of this folder
/// described.
pub fn look(path: &Path) -> Standing {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Standing::Unreadable("this path is a symbolic link".to_owned())
        }
        Ok(_) => {}
        Err(e) => return Standing::Unreadable(e.to_string()),
    }
    match std::fs::read(path) {
        Ok(bytes) => standing(&bytes),
        Err(e) => Standing::Unreadable(e.to_string()),
    }
}

/// Judge every file under `dir`, deepest folders included.
///
/// Files that are not VeilVoice containers at all are left out of the answer,
/// because a report listing every settings file and every log in the folder is
/// a report nobody finishes reading. Everything else is in it, including the
/// files that are already sealed properly, so that a caller can say how many
/// were checked rather than only how many were wrong.
///
/// Symbolic links are not followed, neither to files nor to folders. A folder
/// it cannot read is reported as one [`Standing::Unreadable`] entry for the
/// folder itself rather than silently contributing nothing, because a sweep
/// that quietly skipped a vault would report a clean machine that is not one.
///
/// The answer is sorted by path, so two runs over an unchanged folder read the
/// same way.
pub fn sweep(dir: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    gather(dir, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The recursive half of [`sweep`].
fn gather(dir: &Path, out: &mut Vec<Found>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            out.push(Found {
                path: dir.to_path_buf(),
                standing: Standing::Unreadable(e.to_string()),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `symlink_metadata`, so a link to a folder is not descended into and
        // a link to a file is reported by `look` rather than opened.
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_dir() {
            gather(&path, out);
            continue;
        }
        let standing = look(&path);
        if standing == Standing::NotAContainer {
            continue;
        }
        out.push(Found { path, standing });
    }
}

/// Seal `path` again under `password`, if and only if it opens under none.
///
/// `params` are the costs the new container is written with; callers pass
/// [`kdf::KdfParams::default`] and tests pass something cheaper.
///
/// # Errors
///
/// - [`Error::ResealEmptyPassphrase`] if `password` is empty, which would
///   leave the file exactly as it was found while reporting success.
/// - [`Error::ResealSymlink`] if `path` is a symbolic link.
/// - [`Error::NotSealedUnderNothing`] if the file does not open with an empty
///   passphrase. This is the ordinary answer for most files and is not a
///   failure of the sweep; it is the refusal that makes the tool safe to point
///   at a folder and safe to run twice.
/// - [`Error::ResealStore`] if the file could not be read, or the replacement
///   could not be written. The original is untouched in both cases.
/// - [`Error::Encrypt`] if the new container did not open again with the new
///   passphrase, or did not give back what went into it. Nothing is written,
///   because a replacement that cannot be opened is the one outcome worse than
///   the fault being repaired.
pub fn reseal(path: &Path, password: &[u8], params: kdf::KdfParams) -> Result<(), Error> {
    if password.is_empty() {
        return Err(Error::ResealEmptyPassphrase);
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| Error::ResealStore)?;
    if meta.file_type().is_symlink() {
        return Err(Error::ResealSymlink);
    }
    let sealed_under_nothing = std::fs::read(path).map_err(|_| Error::ResealStore)?;

    let mut plaintext = match container::open_with_password_within(
        b"",
        &sealed_under_nothing,
        kdf::KdfParams::UNATTENDED_MAX_M_COST,
    ) {
        Ok(plaintext) => plaintext,
        Err(Error::Decrypt | Error::WrongMode) => return Err(Error::NotSealedUnderNothing),
        // A truncated or absurdly expensive file is a different complaint and
        // is passed on as itself, so a report does not read as "already fine".
        Err(other) => return Err(other),
    };

    let sealed = match container::seal_with_password(password, &plaintext, params) {
        Ok(sealed) => sealed,
        Err(e) => {
            plaintext.zeroize();
            return Err(e);
        }
    };

    // Read back before replacing, not afterwards. The original is the only
    // copy of this recording, so the new file has to be known good while the
    // old one is still there.
    let opened = container::open_with_password(password, &sealed);
    let same = matches!(&opened, Ok(back) if back.as_slice() == plaintext.as_slice());
    if let Ok(mut back) = opened {
        back.zeroize();
    }
    plaintext.zeroize();
    if !same {
        return Err(Error::Encrypt);
    }

    privatefile::replace_owner_only(path, &sealed).map_err(|_| Error::ResealStore)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cheap() -> kdf::KdfParams {
        kdf::KdfParams::weak_for_tests()
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write");
        path
    }

    /// A folder to work in, removed when the test ends.
    struct Folder(PathBuf);

    impl Folder {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "veilvoice-reseal-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create");
            Self(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The fault, stated as a test: a container sealed under an empty
    /// passphrase opens for anybody, and this is what that looks like from the
    /// outside.
    #[test]
    fn a_file_sealed_under_nothing_is_recognised() {
        let sealed = container::seal_with_password(b"", b"audio", cheap()).expect("seal");
        assert_eq!(standing(&sealed), Standing::OpensWithNothing);
        assert!(standing(&sealed).needs_repair());
    }

    #[test]
    fn a_properly_sealed_file_is_left_alone() {
        let sealed = container::seal_with_password(b"real", b"audio", cheap()).expect("seal");
        assert_eq!(standing(&sealed), Standing::Sealed);
        assert!(!standing(&sealed).needs_repair());
    }

    #[test]
    fn a_public_key_container_is_not_mistaken_for_the_fault() {
        let (_secret, public) = crate::hybrid::SecretKey::generate().expect("key");
        let sealed = container::seal_to_public_key(&public, b"audio").expect("seal");
        assert_eq!(standing(&sealed), Standing::PublicKey);
    }

    #[test]
    fn anything_else_is_not_a_container() {
        assert_eq!(standing(b"settings go here"), Standing::NotAContainer);
        assert_eq!(standing(b""), Standing::NotAContainer);
    }

    /// A container declaring more memory than an unattended caller allows is
    /// reported rather than obeyed. Judged by the error's own words, so that
    /// the reason reaches whoever reads the report.
    #[test]
    fn an_absurdly_expensive_container_is_reported_not_opened() {
        let mut sealed = container::seal_with_password(b"", b"audio", cheap()).expect("seal");
        let (mut header, _) = container::Header::parse(&sealed).expect("parse");
        header.kdf.m_cost = kdf::KdfParams::UNATTENDED_MAX_M_COST + 1;
        let fresh = header.to_bytes();
        sealed[..fresh.len()].copy_from_slice(&fresh);
        assert!(matches!(standing(&sealed), Standing::Unreadable(_)));
    }

    /// The repair, end to end: the file opens with nothing before and does not
    /// afterwards, the audio survives, and it needs the new passphrase.
    #[test]
    fn resealing_closes_the_file_and_keeps_the_audio() {
        let folder = Folder::new("repair");
        let sealed = container::seal_with_password(b"", b"the audio", cheap()).expect("seal");
        let path = write(&folder.0, "take.veil", &sealed);

        assert_eq!(look(&path), Standing::OpensWithNothing);
        reseal(&path, b"a real passphrase", cheap()).expect("reseal");
        assert_eq!(look(&path), Standing::Sealed);

        let now = std::fs::read(&path).expect("read");
        assert_eq!(
            container::open_with_password(b"a real passphrase", &now).expect("open"),
            b"the audio"
        );
    }

    /// Running it twice is safe, which matters because the first run over a
    /// folder may stop part-way.
    #[test]
    fn a_second_run_refuses_rather_than_rewriting() {
        let folder = Folder::new("twice");
        let sealed = container::seal_with_password(b"", b"the audio", cheap()).expect("seal");
        let path = write(&folder.0, "take.veil", &sealed);

        reseal(&path, b"a real passphrase", cheap()).expect("reseal");
        let after_first = std::fs::read(&path).expect("read");
        assert_eq!(
            reseal(&path, b"a real passphrase", cheap()),
            Err(Error::NotSealedUnderNothing)
        );
        assert_eq!(std::fs::read(&path).expect("read"), after_first);
    }

    /// A file sealed under a real passphrase is never rewritten, whatever
    /// passphrase the repair is given. This is the test that stands between
    /// the tool and the recordings it was not meant to touch.
    #[test]
    fn a_properly_sealed_file_is_never_rewritten() {
        let folder = Folder::new("innocent");
        let sealed = container::seal_with_password(b"theirs", b"the audio", cheap()).expect("seal");
        let path = write(&folder.0, "take.veil", &sealed);

        assert_eq!(
            reseal(&path, b"mine", cheap()),
            Err(Error::NotSealedUnderNothing)
        );
        assert_eq!(std::fs::read(&path).expect("read"), sealed);
        assert_eq!(
            container::open_with_password(b"theirs", &sealed).expect("open"),
            b"the audio"
        );
    }

    #[test]
    fn an_empty_passphrase_is_refused() {
        let folder = Folder::new("empty");
        let sealed = container::seal_with_password(b"", b"the audio", cheap()).expect("seal");
        let path = write(&folder.0, "take.veil", &sealed);

        assert_eq!(
            reseal(&path, b"", cheap()),
            Err(Error::ResealEmptyPassphrase)
        );
        assert_eq!(std::fs::read(&path).expect("read"), sealed);
    }

    #[test]
    fn a_missing_file_is_a_store_error() {
        let folder = Folder::new("missing");
        assert_eq!(
            reseal(&folder.0.join("nothing.veil"), b"real", cheap()),
            Err(Error::ResealStore)
        );
    }

    /// A sweep reports what needs repairing and what was checked, and leaves
    /// out the files that are not containers.
    #[test]
    fn a_sweep_reports_the_containers_and_skips_the_rest() {
        let folder = Folder::new("sweep");
        let open = container::seal_with_password(b"", b"one", cheap()).expect("seal");
        let shut = container::seal_with_password(b"real", b"two", cheap()).expect("seal");
        write(&folder.0, "open.veil", &open);
        write(&folder.0, "settings.toml", b"not a container");
        let deeper = folder.0.join("vault");
        std::fs::create_dir_all(&deeper).expect("create");
        write(&deeper, "shut.veil", &shut);

        let found = sweep(&folder.0);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found
            .iter()
            .any(|f| f.path.ends_with("open.veil") && f.standing.needs_repair()));
        assert!(found
            .iter()
            .any(|f| f.path.ends_with("shut.veil") && f.standing == Standing::Sealed));
    }

    /// Symbolic links are reported, not followed. Without this, a sweep of one
    /// folder could rewrite a file in another, and the file it rewrote would
    /// not be the one the report named.
    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_refused_rather_than_followed() {
        let folder = Folder::new("link");
        let sealed = container::seal_with_password(b"", b"the audio", cheap()).expect("seal");
        let real = write(&folder.0, "elsewhere.veil", &sealed);
        let link = folder.0.join("link.veil");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");

        assert!(matches!(look(&link), Standing::Unreadable(_)));
        assert_eq!(reseal(&link, b"real", cheap()), Err(Error::ResealSymlink));
        // The link's target is untouched, and still reported by its own name.
        assert_eq!(std::fs::read(&real).expect("read"), sealed);
        let found = sweep(&folder.0);
        assert!(found
            .iter()
            .any(|f| f.path == real && f.standing.needs_repair()));
        assert!(found
            .iter()
            .any(|f| f.path == link && matches!(f.standing, Standing::Unreadable(_))));
    }

    /// The warning says which releases are affected and what resealing cannot
    /// do, because a warning that only says "update" leaves somebody believing
    /// a file somebody else already has is now safe.
    /// The short line and the long one say the same thing, so somebody who
    /// reads one and then the other is not told two different stories.
    #[test]
    fn the_notice_and_the_warning_agree() {
        for text in [NOTICE, WARNING] {
            assert!(text.contains("0.1.15"));
            assert!(text.contains("0.1.22"));
            assert!(text.contains("veilvoice reseal"));
            assert!(text.contains("empty passphrase"));
        }
    }

    #[test]
    fn the_warning_states_the_versions_and_the_limit() {
        assert!(WARNING.contains("0.1.15"));
        assert!(WARNING.contains("0.1.22"));
        assert!(WARNING.contains("veilvoice reseal"));
        assert!(WARNING.contains("cannot undo a copy"));
        assert!(!WARNING.contains('\u{2014}'), "no em dashes");
    }
}
