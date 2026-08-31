// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Run the GnuPG that is already on this machine.
//!
//! **Marker 97.** VeilVoice checks a release signature itself, with a key
//! compiled into the binary, so that somebody with no GnuPG installed is not
//! stuck. That is a real convenience and it has an obvious circularity: the
//! program telling you the download is genuine came out of that download.
//!
//! Until now the answer to that was to print the commands and leave the running
//! of them to the reader. That is correct, almost nobody does it, and a check
//! almost nobody runs is a check that is not protecting anybody.
//!
//! So this runs them. When GnuPG is on the machine, the signature is checked by
//! **two independent implementations** -- this project's built-in one and
//! somebody else's -- and both answers are reported. Two implementations
//! agreeing is worth more than either alone, and two disagreeing is the loudest
//! thing a verifier can find.
//!
//! # What this does not fix
//!
//! Running `gpg` from inside the binary under suspicion does not make the
//! answer independent of that binary. What it makes independent is the
//! *implementation*: the packet parsing, the hashing and the signature
//! arithmetic are somebody else's code. The independent *invocation* is still
//! the reader typing the commands themselves, and every front end that uses
//! this goes on printing them for exactly that reason.
//!
//! # Why the status lines and not the words
//!
//! `gpg --verify` prints "Good signature from ..." in the reader's own
//! language. Parsing that would be a verifier whose answer depends on a locale,
//! and the failure mode is the bad one: an unrecognised string reads as "no
//! good signature found" in one direction, or matches a translated word in the
//! other.
//!
//! `--status-fd` is GnuPG's machine-readable channel and it is not translated.
//! Measured against GnuPG 2.4.4, which is where these shapes come from rather
//! than from the manual:
//!
//! ```text
//! good:      [GNUPG:] GOODSIG <keyid> <uid>
//!            [GNUPG:] VALIDSIG <fingerprint> ...            exit 0
//! tampered:  [GNUPG:] BADSIG <keyid> <uid>                  exit 1
//! no key:    [GNUPG:] ERRSIG ... / NO_PUBKEY <keyid>        exit 2
//! import:    [GNUPG:] IMPORT_OK <flags> <fingerprint>       exit 0
//! ```
//!
//! # A good signature from *some* key proves nothing
//!
//! This is the mistake the whole thing exists to avoid making on somebody's
//! behalf. `gpg --verify` reports success for a valid signature by **any** key
//! in the keyring, and anybody who can hand you an archive can hand you a
//! signature by a key they made this morning. So [`Gnupg::verify`] is given the
//! fingerprint that is expected and compares `VALIDSIG` against it. A good
//! signature by a different key is its own outcome, [`Outcome::AnotherKey`],
//! and it is a failure rather than a pass with a note attached.
//!
//! # In plain words
//!
//! If you have GnuPG, this uses it, so the answer does not come only from a
//! program you have just downloaded.
//!
//! It adds the VeilVoice signing key to your keyring, checks the signature with
//! your GnuPG, and tells you exactly what your GnuPG said. It also checks that
//! the signature is by the right key, which is the part that matters and the
//! part that is easiest to skip.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Something that stopped GnuPG being asked at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// GnuPG is not on `PATH`.
    NotInstalled,
    /// GnuPG could not be started, or died without answering.
    CouldNotRun(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotInstalled => write!(f, "GnuPG is not on your PATH"),
            Error::CouldNotRun(why) => write!(f, "GnuPG could not be run: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// Where GnuPG is, if it is on `PATH`.
///
/// A lookup and nothing else: it never runs the program to find out.
pub fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["gpg", "gpg2", "gpg.exe", "gpg2.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// What GnuPG made of a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A valid signature by the key that was expected.
    Good,
    /// A valid signature by some other key. **Not a pass.** See the module note.
    AnotherKey {
        /// The fingerprint that actually signed it.
        fingerprint: String,
    },
    /// The signature does not match the data.
    Bad,
    /// The signing key is not in the keyring, so nothing could be decided.
    NoKey,
    /// GnuPG ran and said nothing this understands.
    Unclear,
}

impl Outcome {
    /// Whether this is the one outcome that means the file is as published.
    pub fn is_good(&self) -> bool {
        *self == Outcome::Good
    }

    /// One line saying what happened, in words a reader can act on.
    pub fn plainly(&self) -> String {
        match self {
            Outcome::Good => "your GnuPG agrees: signed by the VeilVoice key".to_string(),
            Outcome::AnotherKey { fingerprint } => format!(
                "your GnuPG found a good signature by {fingerprint}, which is NOT the \
                 VeilVoice key. Treat this download as unverified."
            ),
            Outcome::Bad => "your GnuPG says the signature does not match the file".to_string(),
            Outcome::NoKey => "your GnuPG has no key that could check this signature".to_string(),
            Outcome::Unclear => "your GnuPG ran and gave no answer this could read".to_string(),
        }
    }
}

/// One run of GnuPG, and what it said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The command, as a reader could type it themselves.
    pub command: String,
    /// What it decided.
    pub outcome: Outcome,
    /// The `[GNUPG:]` status lines, with the prefix removed.
    ///
    /// Kept so a front end can show what GnuPG actually said rather than only
    /// this crate's reading of it. A verifier that hides its evidence is asking
    /// to be taken on trust, which is the thing being avoided.
    pub status: Vec<String>,
    /// What GnuPG printed for a person to read, trimmed.
    ///
    /// Never parsed -- see the module note on why the prose is not the channel
    /// to decide anything from -- and worth showing when the status lines said
    /// nothing this understands. "GnuPG gave no answer this could read" helps
    /// nobody; "no valid OpenPGP data found" tells them the file is not a
    /// signature at all.
    pub said: String,
}

/// Whether the key was already in the keyring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imported {
    /// It was added by this run.
    Added,
    /// It was already there and nothing changed.
    AlreadyThere,
}

/// The key, in the keyring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The fingerprint GnuPG reported for what it took in.
    pub fingerprint: String,
    /// Whether this run put it there.
    pub what: Imported,
}

impl Import {
    /// What was done to the reader's keyring, and how to undo it.
    ///
    /// GnuPG has no field for a note about somebody else's key. Measured: there
    /// is no `--comment` that survives an import, and the only way to attach
    /// text to a key you do not own is a local certification, which needs a
    /// secret key of your own that a reader may not have. So the note is said
    /// here, where the person who will wonder about it is looking, and the
    /// removal is one line rather than a paragraph of `--edit-key`.
    pub fn note(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.what {
            Imported::Added => out.push(format!(
                "VeilVoice added its signing key {} to your GnuPG keyring.",
                self.fingerprint
            )),
            Imported::AlreadyThere => out.push(format!(
                "Your GnuPG keyring already had the VeilVoice signing key {}.",
                self.fingerprint
            )),
        }
        out.push("It is a public key: it lets you check signatures and can sign".to_string());
        out.push("nothing and decrypt nothing. It carries no e-mail address.".to_string());
        out.push(format!(
            "To remove it:  gpg --delete-keys {}",
            self.fingerprint
        ));
        out
    }
}

/// A GnuPG to run, and the keyring to run it against.
///
/// A value rather than a pair of free functions, because the keyring is part of
/// the question. The default is the one the reader already uses, which is the
/// point of the feature: after this, their own `gpg --verify` works. A caller
/// that wants isolation -- this crate's own tests, and anything that should not
/// touch somebody's keyring -- names a directory instead and gets a GnuPG that
/// can neither see nor change anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gnupg {
    program: PathBuf,
    home: Option<PathBuf>,
}

impl Gnupg {
    /// The GnuPG on `PATH`, if there is one.
    pub fn found() -> Result<Self, Error> {
        on_path().map(Self::at).ok_or(Error::NotInstalled)
    }

    /// A particular GnuPG, using the keyring its owner already has.
    pub fn at(program: PathBuf) -> Self {
        Self {
            program,
            home: None,
        }
    }

    /// The same GnuPG, against a keyring of its own.
    ///
    /// Nothing outside that directory is read or written, so this neither adds
    /// a key to somebody's own keyring nor sees one that is in it.
    pub fn in_home(mut self, home: PathBuf) -> Self {
        self.home = Some(home);
        self
    }

    /// Where this GnuPG is.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The arguments every call here shares.
    ///
    /// `--batch` and `--no-tty` so nothing ever waits for a person who is not
    /// there. `--status-fd 1` for the machine-readable channel, on standard
    /// output, where GnuPG's prose does not go.
    fn base(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(home) = &self.home {
            command.arg("--homedir").arg(home);
        }
        command
            .args(["--batch", "--no-tty", "--status-fd", "1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Put a key into the keyring this GnuPG is using.
    ///
    /// The armour goes in on standard input, so nothing is written to disk and
    /// no temporary file is left behind if this is interrupted.
    ///
    /// `expected` is checked against what GnuPG says it imported. The two can
    /// only differ if the armour handed in is not the key it claims to be,
    /// which is worth failing on rather than reporting a successful import of
    /// something else.
    pub fn import(&self, armour: &str, expected: &str) -> Result<Import, Error> {
        use std::io::Write as _;

        let mut command = self.base();
        command.arg("--import").stdin(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|e| Error::CouldNotRun(format!("{e}")))?;
        // Written before the output is read: GnuPG takes the whole key before
        // it says anything, so reading first would be waiting for a program
        // that is waiting for us.
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(armour.as_bytes())
                .map_err(|e| Error::CouldNotRun(format!("could not hand GnuPG the key: {e}")))?;
        }
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .map_err(|e| Error::CouldNotRun(format!("{e}")))?;
        let status = status_lines(&output.stdout);

        let Some(ok) = field(&status, "IMPORT_OK") else {
            return Err(Error::CouldNotRun(format!(
                "GnuPG did not say it imported anything: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        };
        let mut parts = ok.split_whitespace();
        let flags = parts.next().unwrap_or("");
        let fingerprint = parts.next().unwrap_or("").to_string();
        if !same_fingerprint(&fingerprint, expected) {
            return Err(Error::CouldNotRun(format!(
                "GnuPG imported {fingerprint}, which is not {expected}"
            )));
        }
        // Flag 0 means nothing about the key changed, which is GnuPG's way of
        // saying it already had it. Anything else means something was added.
        let what = if flags == "0" {
            Imported::AlreadyThere
        } else {
            Imported::Added
        };
        Ok(Import { fingerprint, what })
    }

    /// Check a detached signature with this machine's GnuPG.
    ///
    /// `expected` is the fingerprint that must have signed it. A valid
    /// signature by any other key is [`Outcome::AnotherKey`] and is a failure;
    /// see the module note for why that is the check nobody should be asked to
    /// remember to make.
    pub fn verify(&self, signature: &Path, data: &Path, expected: &str) -> Result<Run, Error> {
        let output = self
            .base()
            .arg("--verify")
            .arg(signature)
            .arg(data)
            .output()
            .map_err(|e| Error::CouldNotRun(format!("{e}")))?;
        let status = status_lines(&output.stdout);
        let command = format!("gpg --verify {} {}", signature.display(), data.display());

        // Order matters. `VALIDSIG` is the only line carrying a full
        // fingerprint, so it is read first and compared. `GOODSIG` alone
        // carries a key id, which is short enough to be chosen by an attacker
        // and is never enough on its own.
        let outcome = if let Some(valid) = field(&status, "VALIDSIG") {
            let signer = valid.split_whitespace().next().unwrap_or("");
            if same_fingerprint(signer, expected) {
                Outcome::Good
            } else {
                Outcome::AnotherKey {
                    fingerprint: signer.to_string(),
                }
            }
        } else if field(&status, "BADSIG").is_some() {
            Outcome::Bad
        } else if field(&status, "NO_PUBKEY").is_some() || field(&status, "ERRSIG").is_some() {
            Outcome::NoKey
        } else {
            Outcome::Unclear
        };

        Ok(Run {
            command,
            outcome,
            status,
            said: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// The `[GNUPG:]` lines out of what GnuPG printed, prefix removed.
fn status_lines(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] "))
        .map(str::to_string)
        .collect()
}

/// The rest of a status line whose first word is `keyword`.
///
/// The space is load bearing: without it `NO_PUBKEY` would match a line
/// beginning `NO_PUBKEYS`, and a keyword GnuPG adds later could quietly change
/// what this decides.
fn field<'a>(status: &'a [String], keyword: &str) -> Option<&'a str> {
    status
        .iter()
        .find_map(|line| line.strip_prefix(keyword)?.strip_prefix(' '))
}

/// Whether two fingerprints are the same, ignoring case and spacing.
///
/// GnuPG prints them unspaced and uppercase; a fingerprint copied off a website
/// often arrives in groups of four. Comparing the two as typed would refuse a
/// correct key over its formatting. Two empty strings are not a match, so a run
/// that produced no fingerprint at all cannot compare equal to an expectation
/// of nothing.
fn same_fingerprint(a: &str, b: &str) -> bool {
    let strip = |text: &str| {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_uppercase)
            .collect::<String>()
    };
    let (a, b) = (strip(a), strip(b));
    !a.is_empty() && a == b
}

/// The commands a reader can type to get this answer without VeilVoice.
///
/// Printed as well as run, always. See the module note: running GnuPG makes the
/// implementation independent and does not make the invocation independent, and
/// only the reader can supply that.
pub fn commands(sums: &Path, signature: &Path, key: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(key) = key {
        out.push(format!("gpg --import {}", key.display()));
    }
    out.push(format!(
        "gpg --verify {} {}",
        signature.display(),
        sums.display()
    ));
    out.push("sha256sum -c SHA256SUMS --ignore-missing".to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The project's own key, so the import test imports the real thing.
    const KEY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../website/assets/veilvoice-signing-key.asc"
    ));
    const FINGERPRINT: &str = "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A";

    #[test]
    fn a_fingerprint_is_compared_by_its_digits_and_not_its_spacing() {
        assert!(same_fingerprint(
            "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A",
            "8101 FB3B B28D 02FB 239E  0CDF 9CC1 C7E7 A9B5 833A"
        ));
        assert!(same_fingerprint(
            "8101fb3bb28d02fb239e0cdf9cc1c7e7a9b5833a",
            "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A"
        ));
        assert!(!same_fingerprint(
            "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A",
            "0000FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A"
        ));
    }

    /// Two empty strings are not a match. Otherwise a run that produced no
    /// fingerprint would compare equal to an expectation of nothing.
    #[test]
    fn nothing_does_not_match_nothing() {
        assert!(!same_fingerprint("", ""));
        assert!(!same_fingerprint("   ", ""));
    }

    /// The four shapes, taken from GnuPG 2.4.4 rather than from the manual.
    #[test]
    fn the_status_channel_is_read_and_the_prose_is_not() {
        let good = status_lines(
            b"[GNUPG:] NEWSIG\n\
              [GNUPG:] GOODSIG F63C0EF4DF07E273 testkey\n\
              [GNUPG:] VALIDSIG 520A54F1437E8CE8E18416AEF63C0EF4DF07E273 2026-08-31 1788205030 0 4 0 1 10 00 520A54F1437E8CE8E18416AEF63C0EF4DF07E273\n",
        );
        assert_eq!(
            field(&good, "VALIDSIG").and_then(|v| v.split_whitespace().next()),
            Some("520A54F1437E8CE8E18416AEF63C0EF4DF07E273")
        );
        assert!(field(&good, "BADSIG").is_none());

        let bad = status_lines(b"[GNUPG:] BADSIG F63C0EF4DF07E273 testkey\n");
        assert!(field(&bad, "BADSIG").is_some());
        assert!(field(&bad, "VALIDSIG").is_none());

        let missing = status_lines(b"[GNUPG:] NO_PUBKEY F63C0EF4DF07E273\n");
        assert!(field(&missing, "NO_PUBKEY").is_some());

        let imported =
            status_lines(b"[GNUPG:] IMPORT_OK 1 520A54F1437E8CE8E18416AEF63C0EF4DF07E273\n");
        assert_eq!(
            field(&imported, "IMPORT_OK"),
            Some("1 520A54F1437E8CE8E18416AEF63C0EF4DF07E273")
        );
    }

    /// Prose is never read, so a translated GnuPG cannot change the answer.
    #[test]
    fn a_translated_good_signature_line_is_not_a_status_line() {
        let translated = status_lines(
            "gpg: Korrekte Signatur von \"tilas01\"\n[GNUPG:] BADSIG AA testkey\n".as_bytes(),
        );
        assert_eq!(translated, vec!["BADSIG AA testkey".to_string()]);
    }

    /// A keyword matches only as a whole word.
    #[test]
    fn a_longer_keyword_is_not_the_one_being_looked_for() {
        let status = vec!["NO_PUBKEYX AA".to_string(), "VALIDSIGGED BB".to_string()];
        assert!(field(&status, "NO_PUBKEY").is_none());
        assert!(field(&status, "VALIDSIG").is_none());
    }

    /// Only one outcome is a pass, and a good signature by the wrong key is not
    /// it. This is the check the release notes ask a reader to make by hand.
    #[test]
    fn only_the_expected_key_is_a_pass() {
        assert!(Outcome::Good.is_good());
        assert!(!Outcome::AnotherKey {
            fingerprint: "0".repeat(40)
        }
        .is_good());
        assert!(!Outcome::Bad.is_good());
        assert!(!Outcome::NoKey.is_good());
        assert!(!Outcome::Unclear.is_good());
        assert!(Outcome::AnotherKey {
            fingerprint: "0".repeat(40)
        }
        .plainly()
        .contains("NOT the"));
    }

    /// The commands are the independent article and must name both files.
    #[test]
    fn the_printed_commands_are_runnable_as_printed() {
        let lines = commands(
            Path::new("SHA256SUMS"),
            Path::new("SHA256SUMS.asc"),
            Some(Path::new("veilvoice-signing-key.asc")),
        );
        assert_eq!(lines[0], "gpg --import veilvoice-signing-key.asc");
        assert_eq!(lines[1], "gpg --verify SHA256SUMS.asc SHA256SUMS");
        assert!(lines[2].starts_with("sha256sum -c"));
    }

    /// Nothing here ever asks a person for anything: these run where no
    /// terminal exists, and a GnuPG that stopped to prompt would hang a
    /// verifier somebody double-clicked.
    #[test]
    fn every_call_is_unattended() {
        let source = include_str!("lib.rs");
        let body = source.split("#[cfg(test)]").next().unwrap();
        for flag in ["--batch", "--no-tty", "--status-fd"] {
            assert!(body.contains(flag), "{flag} is not passed");
        }
        assert!(
            !body.contains("--yes"),
            "nothing here overwrites anything, so nothing here needs --yes"
        );
    }

    /// The note says what was done and how to undo it, because a key that
    /// appears in somebody's keyring without explanation is a thing they will
    /// later find and not understand.
    #[test]
    fn the_note_says_what_changed_and_how_to_put_it_back() {
        let added = Import {
            fingerprint: FINGERPRINT.to_string(),
            what: Imported::Added,
        }
        .note()
        .join("\n");
        assert!(added.contains("added its signing key"));
        assert!(added.contains(&format!("gpg --delete-keys {FINGERPRINT}")));
        assert!(added.contains("public key"));

        let already = Import {
            fingerprint: FINGERPRINT.to_string(),
            what: Imported::AlreadyThere,
        }
        .note()
        .join("\n");
        assert!(already.contains("already had"));
    }

    /// Somewhere for a keyring that is nobody's real one.
    fn scratch_home() -> Option<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        // Short, and not under a long temporary path: GnuPG puts an agent
        // socket in here and a long directory name overflows the sockaddr,
        // which fails as "error running gpg-agent" and looks like a bug in
        // this code. Measured while writing this test.
        let path = std::env::temp_dir().join(format!("vvg{stamp:x}"));
        std::fs::create_dir_all(&path).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).ok()?;
        }
        Some(path)
    }

    /// The real key, into a real GnuPG, twice.
    ///
    /// Skipped where GnuPG is not installed, because that is a fact about the
    /// machine rather than a failure of this code, and the whole crate already
    /// reports [`Error::NotInstalled`] for it.
    #[test]
    fn the_real_key_imports_into_a_real_gnupg_and_says_so_the_second_time() {
        let Ok(gpg) = Gnupg::found() else {
            return;
        };
        let Some(home) = scratch_home() else {
            return;
        };
        let gpg = gpg.in_home(home.clone());

        let first = gpg.import(KEY, FINGERPRINT);
        // A GnuPG that cannot start its agent cannot import either, and that
        // is the machine rather than this code. Anything it does report has to
        // be the right key.
        if let Ok(first) = first {
            assert_eq!(first.what, Imported::Added, "the first import adds it");
            assert!(same_fingerprint(&first.fingerprint, FINGERPRINT));

            let again = gpg.import(KEY, FINGERPRINT).expect("a second import");
            assert_eq!(
                again.what,
                Imported::AlreadyThere,
                "the second import changes nothing"
            );

            // And it is refused when the caller expects a different key.
            let wrong = gpg.import(KEY, &"0".repeat(40));
            assert!(wrong.is_err(), "{wrong:?}");
        }
        std::fs::remove_dir_all(&home).ok();
    }

    /// A signature nobody can check is `NoKey`, not a pass.
    ///
    /// Run against an empty keyring, so the answer cannot come from a key that
    /// happens to be on the machine running the tests.
    #[test]
    fn a_signature_with_no_key_to_check_it_is_never_a_pass() {
        let Ok(gpg) = Gnupg::found() else {
            return;
        };
        let Some(home) = scratch_home() else {
            return;
        };
        let data = home.join("SHA256SUMS");
        let signature = home.join("SHA256SUMS.asc");
        std::fs::write(&data, b"a hash  a file\n").unwrap();
        std::fs::write(
            &signature,
            b"-----BEGIN PGP SIGNATURE-----\n\nbm90IGEgc2lnbmF0dXJl\n=aaaa\n\
              -----END PGP SIGNATURE-----\n",
        )
        .unwrap();

        let run = gpg
            .in_home(home.clone())
            .verify(&signature, &data, FINGERPRINT)
            .expect("gpg runs");
        assert!(!run.outcome.is_good(), "{:?}", run.outcome);
        assert!(run.command.contains("--verify"));
        std::fs::remove_dir_all(&home).ok();
    }
}
