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
//! So the result is sealed into a [`container`], with Argon2id or the X25519
//! plus ML-KEM-768 hybrid, unless the user asks for plaintext, and asking for
//! plaintext prints [`PLAINTEXT_WARNING`] and, on a terminal, waits for an
//! answer.
//!
//! # Never through a plaintext file
//!
//! The WAV is encoded in memory and sealed there. It is never written to disk
//! and then encrypted, because a plaintext file that is created and deleted is
//! precisely what [`veilvoice_crypto::shred`] explains cannot be reliably taken
//! back on flash storage.
//!
//! # In plain words
//!
//! Asks for a passphrase and encrypts the recording VeilVoice has just written.
//!
//! It is on by default, and the reason is worth stating: the words survive
//! de-identification on purpose, so an unencrypted result is still a recording of
//! everything that was said. Veiling the voice and leaving the file open protects
//! the speaker and not the conversation.
//!
//! Writing one unencrypted is allowed, and asks first.

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
    "this file can still hear everything that was said: another user, a",
    "backup, a sync client, anyone who later gets the disk.",
    "",
    "Deleting it afterwards is not a fix: on an SSD, SD card or USB stick",
    "the original blocks can survive every overwrite. That is why at-rest",
    "encryption is the default rather than an option you have to find.",
    "",
    "The file will be created readable only by your account. That is a file",
    "permission and nothing more. It does not survive a copy, a backup, or",
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
            let encoded = crate::read_named(key_path)?;
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
/// Non-interactive callers, meaning scripts, pipelines and CI, still see it on
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
        return Err("cancelled, and nothing was written".into());
    }
    Ok(())
}

/// Move a typed password into page-locked, zeroizing storage, wiping the
/// `String` it arrived in.
///
/// `rpassword` hands back an ordinary `String`, which is an ordinary heap
/// allocation that can be paged out and is not wiped when it is dropped. That
/// is a window this crate cannot remove, because something has to receive the
/// keystrokes. It can be made as short as possible, which is what this
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

/// What to say when there is no terminal to ask on.
///
/// **F-109.** Every one of these prompts used to surface the operating
/// system's own error, so `veilvoice anonymise recording.wav` run from a
/// script, a cron job, a CI step or anything with its input redirected failed
/// with:
///
/// ```text
/// ✗ No such device or address (os error 6)
/// ```
///
/// That is `ENXIO` from opening the console, and it says nothing: not what was
/// being asked for, not why it failed, and not one of the three ways to
/// proceed. The message is also different on Windows, so nobody could search
/// for it and find the same answer twice.
///
/// `confirm_plaintext` in this same file already got this right, checking for
/// a terminal before asking anything. The prompts did not, which is the same
/// defect in the same file with a different door, and is the shape this
/// project has recorded most often.
fn no_terminal(asked_for: &str) -> String {
    // Built line by line rather than as one continued literal, so the
    // indentation in this file is not the indentation on the reader's screen.
    //
    // Every flag named here is attributed to the command that has it, and that
    // is not tidiness. The first version of this message offered
    // `--encrypt-to` and `--encrypt false --yes` to everybody, because it was
    // written while fixing `anonymise`. `veilvoice encrypt` spells the same
    // idea `--to` and has no `--encrypt` at all, and `lock`, `guard` and
    // `policy` have none of them: a dozen callers share these prompts, and a
    // message that names flags the command in front of the reader does not
    // have is worse than one that names none. Caught by running `veilvoice
    // encrypt` with no terminal and reading what it suggested.
    let lines = [
        "",
        "  This is what happens in a script, a scheduled job, or anything with",
        "  its input redirected.",
        "",
        "  Run it in a terminal, if somebody is there to type.",
        "",
        "  Or, for the two commands that write a recording, seal it to a public",
        "  key instead, which types nothing and is what works in a script:",
        "",
        "    veilvoice anonymise <FILE> --encrypt-to <PUBKEY>",
        "    veilvoice encrypt   <FILE> --to <PUBKEY>",
        "",
        "  Make the key once, in a terminal, with veilvoice keygen.",
        "",
        "  veilvoice anonymise can also write a recording with no encryption at",
        "  all, using --encrypt false --yes. That leaves every word that was",
        "  said readable by anyone who gets the file.",
        "",
        "  Nothing was written.",
    ];
    format!(
        "there is no terminal here to ask for {asked_for}.\n{}",
        lines.join("\n")
    )
}

/// Whether a passphrase can be asked for at all.
///
/// Checked before prompting rather than after failing, so the answer is the
/// same on every platform. `rpassword` reports a missing console differently
/// on Windows and Unix, and a message a reader can search for should not
/// depend on which.
fn can_prompt() -> bool {
    std::io::stdin().is_terminal()
}

/// Prompt once, without echoing, and keep the answer in a [`Secret`].
pub fn prompt_secret(prompt: &str) -> Result<Secret, String> {
    if !can_prompt() {
        return Err(no_terminal("a passphrase"));
    }
    let typed = rpassword::prompt_password(prompt).map_err(|e| e.to_string())?;
    Ok(into_secret(typed))
}

/// Read a password twice, without echoing it, and check the two agree.
pub fn read_new_password() -> Result<Secret, String> {
    if !can_prompt() {
        return Err(no_terminal("a passphrase"));
    }
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

#[cfg(test)]
mod no_terminal_tests {
    use super::*;

    /// **F-109.** No terminal is explained, not reported as an errno.
    ///
    /// `veilvoice anonymise recording.wav` from a script, a scheduled job or
    /// anything with its input redirected used to fail with
    /// `No such device or address (os error 6)`, which is `ENXIO` from opening
    /// the console. It names nothing that was being asked for, nothing about
    /// why, and none of the ways on. It is also a different string on Windows,
    /// so nobody could search for it and get the same answer twice.
    #[test]
    fn the_no_terminal_message_says_what_was_wanted_and_how_to_proceed() {
        let message = no_terminal("a passphrase");

        assert!(
            message.starts_with("there is no terminal here to ask for a passphrase."),
            "the first line has to say what could not be asked for: {message}"
        );

        // Every flag is named beside the command that has it.
        //
        // The first version of this message offered `--encrypt-to` and
        // `--encrypt false --yes` to every caller, because it was written
        // while fixing `anonymise`. `veilvoice encrypt` spells that `--to`
        // and has no `--encrypt`; `lock`, `guard` and `policy` have none of
        // them. A dozen callers share these prompts, so an unattributed flag
        // is a flag the reader's command probably does not have.
        for (flag, command) in [
            ("--encrypt-to", "veilvoice anonymise"),
            ("--encrypt false --yes", "veilvoice anonymise"),
            ("--to <PUBKEY>", "veilvoice encrypt"),
        ] {
            let at = message
                .find(flag)
                .unwrap_or_else(|| panic!("the message no longer mentions {flag}: {message}"));
            let before = &message[..at];
            let line_start = before.rfind('\n').map(|n| n + 1).unwrap_or(0);
            let context = &message[line_start..];
            let context = &context[..context.find('\n').unwrap_or(context.len())];
            assert!(
                context.contains(command)
                    || before.rfind(command).is_some_and(|c| {
                        // Named earlier in the same paragraph is fine; named
                        // nowhere is not.
                        message[c..at].matches("\n\n").count() == 0
                    }),
                "{flag} is offered without saying it belongs to {command}, and \
                 most callers of this message are not that command: {message}"
            );
        }

        assert!(
            message.contains("veilvoice keygen"),
            "the message has to say where the public key comes from: {message}"
        );

        assert!(
            message.contains("Nothing was written."),
            "a refusal has to say that nothing was written, or the reader \
             cannot tell whether a half-finished file is lying around"
        );

        // The operating system's own words are what this replaced.
        assert!(
            !message.contains("os error"),
            "the errno is back in the message: {message}"
        );
    }

    /// The same guidance whichever prompt could not run.
    ///
    /// Two functions prompt, and both refuse through here. A reader who hits
    /// one and then the other should not get two different accounts of the
    /// same situation.
    #[test]
    fn both_prompts_refuse_with_the_same_explanation() {
        let source = include_str!("atrest.rs").replace("\r\n", "\n");
        for function in ["pub fn prompt_secret(", "pub fn read_new_password("] {
            let body = source
                .split(function)
                .nth(1)
                .unwrap_or_else(|| panic!("{function} has to be findable"));
            let body = body.split("\npub fn ").next().unwrap();
            assert!(
                body.contains("can_prompt()") && body.contains("no_terminal("),
                "{function} does not check for a terminal before prompting, so \
                 it will surface the operating system's error instead"
            );
        }
    }
}
