// SPDX-License-Identifier: GPL-3.0-or-later
//! Encryption at rest for the recordings VeilVoice writes, and the passphrase
//! prompts that feed it.
//!
//! # Why this is the default
//!
//! De-identification and confidentiality are different problems, and VeilVoice
//! only solves the first: the words survive on purpose, so a veiled recording
//! sitting on disk is still a recording of everything that was said. Writing it
//! in the clear by default would quietly leave the second problem unsolved for
//! everyone who did not think to ask.
//!
//! So the result is sealed into a [`container`] — Argon2id or the X25519 +
//! ML-KEM-768 hybrid — unless the user asks for plaintext, and asking for
//! plaintext prints [`PLAINTEXT_WARNING`] and, on a terminal, waits for an
//! answer.
//!
//! # Never through a plaintext file
//!
//! The WAV is encoded in memory and sealed there. It is never written to disk
//! and then encrypted, because a plaintext file that is created and deleted is
//! precisely what [`veilvoice_crypto::shred`] explains cannot be reliably taken
//! back on flash storage.

use crate::theme::{colour, err, paint, warn};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use veilvoice_crypto::{container, hybrid, kdf, Secret};
use zeroize::Zeroize;

/// What the user is told before a recording is written in the clear.
///
/// Kept here as data rather than inline `println!`s so the test suite can
/// assert it still says the uncomfortable part.
pub const PLAINTEXT_WARNING: &[&str] = &[
    "The de-identified recording will be written to disk unencrypted.",
    "",
    "VeilVoice destroys the voiceprint, not the words. Anyone who can read",
    "this file — another user, a backup, a sync client, anyone who later",
    "gets the disk — can still hear everything that was said.",
    "",
    "Deleting it afterwards is not a fix: on an SSD, SD card or USB stick",
    "the original blocks can survive every overwrite. That is why at-rest",
    "encryption is the default rather than an option you have to find.",
    "",
    "The file will be created readable only by your account. That is a file",
    "permission and nothing more — it does not survive a copy, a backup, or",
    "anyone who has the disk.",
];

/// How a recording is to be sealed.
pub enum Recipient<'a> {
    /// Argon2id over a passphrase typed at the prompt.
    Password,
    /// The X25519 + ML-KEM-768 hybrid, to a recipient's public key file.
    PublicKey(&'a Path),
}

/// Seal `plaintext` and write it to `<path>.veil`, returning where it landed.
pub fn seal_to_disk(
    path: &Path,
    plaintext: &[u8],
    recipient: Recipient<'_>,
) -> Result<PathBuf, String> {
    let sealed = match recipient {
        Recipient::PublicKey(key_path) => {
            let encoded =
                std::fs::read(key_path).map_err(|e| format!("{}: {e}", key_path.display()))?;
            let pk = hybrid::PublicKey::from_bytes(&encoded).map_err(|e| e.to_string())?;
            container::seal_to_public_key(&pk, plaintext).map_err(|e| e.to_string())?
        }
        Recipient::Password => {
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  Choose a passphrase for this recording. It is separate from",
                )
            );
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  the app lock, and there is no way to recover it."
                )
            );
            let password = read_new_password()?;
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  Deriving key (Argon2id, this is meant to be slow)..."
                )
            );
            container::seal_with_password(password.expose(), plaintext, kdf::KdfParams::default())
                .map_err(|e| e.to_string())?
        }
    };

    let out = container::veil_path(path);
    std::fs::write(&out, &sealed).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(out)
}

/// Print the plaintext warning and, on an interactive terminal, require an
/// explicit answer before continuing.
///
/// Non-interactive callers — scripts, pipelines, CI — still see the warning on
/// stderr but are not blocked on a prompt nobody is there to answer. They asked
/// for plaintext on the command line, which is as explicit as it gets.
pub fn confirm_plaintext(assume_yes: bool) -> Result<(), String> {
    println!();
    println!("{}", err("WRITING THIS RECORDING UNENCRYPTED"));
    for line in PLAINTEXT_WARNING {
        println!("{}", paint(colour::MUTED, &format!("  {line}")));
    }
    println!();

    if assume_yes || !std::io::stdin().is_terminal() {
        eprintln!(
            "{}",
            warn("continuing without at-rest encryption, as asked")
        );
        return Ok(());
    }

    print!("  Type UNENCRYPTED to continue: ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    if answer.trim() != "UNENCRYPTED" {
        return Err("cancelled — nothing was written".into());
    }
    Ok(())
}

/// Move a typed password into page-locked, zeroizing storage, wiping the
/// `String` it arrived in.
///
/// `rpassword` hands back an ordinary `String`, which is an ordinary heap
/// allocation that can be paged out and is not wiped when it is dropped. That
/// is a window this crate cannot remove — something has to receive the
/// keystrokes — but it can be made as short as possible, which is what this
/// does: copy into a [`Secret`], wipe the copy, wipe the original, and hand
/// back the only remaining version.
///
/// No `unsafe`, so the intermediate `Vec` is a real second copy for a moment.
/// It is wiped by `Secret::new` before this returns. Writing through
/// `String::as_bytes_mut` would avoid it and is not worth an `unsafe` block in
/// a crate that has none.
fn into_secret(mut typed: String) -> Secret {
    let mut bytes = typed.as_bytes().to_vec();
    let secret = Secret::new(&mut bytes);
    typed.zeroize();
    secret
}

/// Prompt once, without echoing, and keep the answer in a [`Secret`].
pub fn prompt_secret(prompt: &str) -> Result<Secret, String> {
    let typed = rpassword::prompt_password(prompt).map_err(|e| e.to_string())?;
    Ok(into_secret(typed))
}

/// Read a password twice, without echoing it, and check the two agree.
pub fn read_new_password() -> Result<Secret, String> {
    let first = rpassword::prompt_password("Passphrase: ").map_err(|e| e.to_string())?;
    if first.is_empty() {
        return Err("passphrase must not be empty".into());
    }
    let again = rpassword::prompt_password("Repeat: ").map_err(|e| e.to_string())?;
    // Compared before either is moved into a `Secret`, then both are wiped
    // whichever way the comparison went.
    let matched = first == again;
    let first = into_secret(first);
    let _ = into_secret(again);
    if !matched {
        return Err("passphrases do not match".into());
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The warning has one job. If it is ever softened into reassurance, this
    /// is what stops it shipping.
    #[test]
    fn the_warning_states_the_actual_consequence() {
        let text = PLAINTEXT_WARNING.join(" ").to_lowercase();
        assert!(text.contains("unencrypted"));
        assert!(
            text.contains("everything that was said"),
            "the words surviving must be spelled out"
        );
        assert!(
            text.contains("deleting it afterwards is not a fix"),
            "the flash-retention trap must be stated"
        );
        for reassurance in ["safe", "secure", "protected"] {
            assert!(
                !text.contains(reassurance),
                "reassuring word: {reassurance}"
            );
        }
        // The owner-only permission the plaintext now gets must be described as
        // the small thing it is. If it ever reads as a substitute for the
        // encryption being declined, this is what stops it shipping.
        assert!(
            text.contains("a file permission and nothing more"),
            "the permission must be belittled, not offered as consolation"
        );
        assert!(
            text.contains("anyone who has the disk"),
            "the limit of a file permission must be stated"
        );
    }

    #[test]
    fn sealed_output_goes_beside_the_recording_with_a_veil_suffix() {
        assert_eq!(
            container::veil_path(Path::new("clip.veiled.wav")),
            PathBuf::from("clip.veiled.wav.veil")
        );
    }

    /// A sealed recording must not contain its own audio in the clear. This is
    /// the property the whole default exists for.
    #[test]
    fn a_sealed_recording_does_not_contain_its_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.veiled.wav");
        let audio = veilvoice_audio::io::wav_bytes(&veilvoice_audio::io::Audio {
            samples: vec![0.25; 4_000],
            sample_rate: 48_000,
        })
        .unwrap();

        // Public-key mode needs no prompt, so it is the one that can be tested
        // end to end here; both modes share `finish` inside the container.
        let (sk, pk) = hybrid::SecretKey::generate().unwrap();
        let key_path = dir.path().join("veilvoice.pub");
        std::fs::write(&key_path, pk.to_bytes()).unwrap();

        let out = seal_to_disk(&path, &audio, Recipient::PublicKey(&key_path)).unwrap();
        assert_eq!(out, container::veil_path(&path));
        assert!(!path.exists(), "the plaintext must never reach the disk");

        let sealed = std::fs::read(&out).unwrap();
        assert_eq!(&sealed[..8], container::MAGIC);
        assert!(
            !sealed.windows(4).any(|w| w == b"RIFF"),
            "the WAV header is visible in the container"
        );
        assert_eq!(
            container::open_with_secret_key(&sk, &sealed).unwrap(),
            audio
        );
    }

    #[test]
    fn a_missing_public_key_is_reported_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nowhere.pub");
        let result = seal_to_disk(
            &dir.path().join("clip.wav"),
            b"audio",
            Recipient::PublicKey(&missing),
        );
        assert!(result.is_err());
    }
}
