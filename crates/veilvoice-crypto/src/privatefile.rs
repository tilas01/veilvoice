// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Writing a file that only its owner can read.
//!
//! Returns [`std::io::Result`] rather than this crate's [`Error`](crate::Error),
//! which is `Copy` and therefore cannot carry the underlying reason. A caller
//! reporting "could not write the key" is far more useful when it can say why.
//!
//! # Why this is not `std::fs::write` plus a `chmod`
//!
//! `std::fs::write` creates the file with the process umask, which on almost
//! every Unix system means `0644` -- world readable. Tightening it afterwards
//! with `set_permissions` leaves a window, however short, in which any other
//! local user can open the file and read all of it. For a file that exists
//! *because* its contents are sensitive, that window has no reason to exist:
//! `OpenOptions::mode` applies the permission at the moment of creation, before
//! any byte is written.
//!
//! The audit found this pattern in three places -- the app-lock verifier, the
//! encrypted private key written by `veilvoice keygen`, and the plaintext a
//! recording is decrypted into. The verifier one was the worst, because it is
//! rewritten after *every* failed unlock attempt, so the window reopened on
//! each try. This module is the single answer to all of them.
//!
//! # What this does not do
//!
//! It is a Unix permission, not a security boundary against root, against
//! someone holding the disk, or against a backup client running as you. It
//! narrows one specific, avoidable exposure: another unprivileged user on the
//! same machine.
//!
//! On Windows there is no `mode`. A file created under the user profile
//! inherits an ACL that already excludes other unprivileged users, and there is
//! no portable tightening to apply beyond that -- so on Windows this is an
//! ordinary write, and says so rather than implying a protection it did not
//! obtain.

use std::io::Write;
use std::path::Path;

/// Create `path` containing `bytes`, readable only by the current user.
///
/// An existing file is truncated and rewritten. Use
/// [`write_owner_only_new`] when the file must not already exist.
pub fn write_owner_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_inner(path, bytes, false)
}

/// As [`write_owner_only`], but fail if anything is already at `path`.
///
/// This is the way to create a file without a check-then-write race. Testing
/// `path.exists()` first and then writing loses twice: another process can win
/// between the two steps, and a symbolic link planted at the path would be
/// followed, so the write would land on whatever it points at. `create_new`
/// asks the kernel for one atomic answer to both.
pub fn write_owner_only_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_inner(path, bytes, true)
}

fn write_inner(path: &Path, bytes: &[u8], exclusive: bool) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;

    // `mode` only applies when the file is created, so a file left behind by an
    // older build, or by a looser umask, still needs tightening. This is the
    // fix-up for that case, not the defence.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }

    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contents_are_what_was_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.bin");
        write_owner_only(&path, b"sensitive").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"sensitive");

        // Rewriting truncates rather than appending.
        write_owner_only(&path, b"short").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"short");
    }

    /// The point of the module: owner-only from the moment the file exists,
    /// not after a second syscall.
    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only_immediately() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.bin");
        write_owner_only(&path, b"x").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "created with mode {mode:o}");
    }

    /// A file left world-readable by an older build is tightened on the next
    /// write, since `mode` alone would not touch it.
    #[cfg(unix)]
    #[test]
    fn an_existing_loose_file_is_tightened() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.bin");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_owner_only(&path, b"new").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "left at mode {mode:o}");
    }

    #[test]
    fn the_exclusive_form_refuses_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("once.bin");
        write_owner_only_new(&path, b"first").unwrap();
        assert!(write_owner_only_new(&path, b"second").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
    }

    /// A symbolic link at the target path must not be followed: creating
    /// exclusively is how that is refused rather than obeyed.
    #[cfg(unix)]
    #[test]
    fn the_exclusive_form_does_not_follow_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, b"important").unwrap();
        let link = dir.path().join("key.bin");
        std::os::unix::fs::symlink(&victim, &link).unwrap();

        assert!(write_owner_only_new(&link, b"overwritten").is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"important");
    }
}
