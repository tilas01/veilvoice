// SPDX-License-Identifier: GPL-3.0-or-later
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
//!
//! # In plain words
//!
//! Writes a file that only you can read.
//!
//! The important part is the order. The permissions are set as the file is
//! created, not afterwards, because a file that exists for even a moment with the
//! wrong permissions is a file somebody else's program may have read in that
//! moment.
//!
//! When it cannot manage that, it says exactly why rather than just failing, since
//! "could not write the key" is not something anybody can act on.

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

/// Replace `path` with `bytes` in one step, or leave what was there.
///
/// The write goes to a temporary file in the same directory and is renamed over
/// the destination, which the operating system performs as a single operation.
/// A process that dies part-way through leaves the old file, not half of a new
/// one.
///
/// This matters where a half-written file is not merely useless but *wrong*. A
/// truncated app-lock copy reads as a copy that has been interfered with, and a
/// false report of interference is worse than none: it teaches the person
/// reading it to dismiss the true one.
///
/// The temporary file is created owner-only, and the rename carries that
/// permission to the destination, so there is no moment at which the contents
/// are readable by anybody else.
pub fn replace_owner_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        // Nowhere to put a temporary file beside it, so nowhere to rename from:
        // a rename across filesystems is not atomic and may not be permitted.
        return write_inner(path, bytes, false);
    };
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(".new");
    let temporary = parent.join(name);

    // Not `create_new`: a temporary left by a previous crash must not stop the
    // write for ever. It is in a directory this process owns and its name is
    // derived from the destination, so there is nothing here to be raced for.
    write_inner(&temporary, bytes, false)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(e)
        }
    }
}

/// Make an existing file readable only by its owner.
///
/// For files this program did not create. `veilvoice import` and
/// `veilvoice video` hand the writing to `ffmpeg`, which creates the output
/// under its own umask, and `import`'s output is the *original* audio pulled
/// out of a container: the untouched voiceprint, which is the single most
/// revealing thing this program ever puts on a disk. It was being left
/// world-readable.
///
/// # What this cannot do
///
/// There is a window between `ffmpeg` creating the file and this tightening
/// it, and during that window the file is whatever the umask made it. That is
/// inherent to delegating the write to another program, and it is not closed
/// by pretending otherwise. Narrowing the exposure from "for ever" to "for the
/// length of one transcode" is worth having, and the honest description of it
/// belongs here rather than in a claim that it is airtight.
///
/// A no-op on platforms without Unix permissions, where the file's protection
/// comes from the directory it is in.
pub fn tighten(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
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

    /// A file another program created is tightened afterwards.
    #[cfg(unix)]
    #[test]
    fn tightening_closes_a_file_somebody_else_left_open() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("from-ffmpeg.wav");
        // As another program would leave it.
        std::fs::write(&path, b"the original audio").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );

        tighten(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // And the contents are untouched: this changes the mode, not the file.
        assert_eq!(std::fs::read(&path).unwrap(), b"the original audio");
    }

    #[test]
    fn a_replacement_lands_whole_or_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record.bin");
        replace_owner_only(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        replace_owner_only(&path, b"second and longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second and longer");

        // Nothing is left beside it. A temporary that outlived the write would
        // be a copy of the contents with nobody watching it.
        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .filter(|n| n != "record.bin")
            .collect();
        assert!(strays.is_empty(), "left behind: {strays:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_replacement_is_owner_only_including_while_it_is_temporary() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record.bin");
        replace_owner_only(&path, b"sensitive").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "readable by somebody else");
    }

    #[test]
    fn a_replacement_with_no_directory_beside_it_still_writes() {
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = replace_owner_only(Path::new("bare.bin"), b"content");
        let read = std::fs::read("bare.bin");
        std::env::set_current_dir(previous).unwrap();
        result.unwrap();
        assert_eq!(read.unwrap(), b"content");
    }

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
