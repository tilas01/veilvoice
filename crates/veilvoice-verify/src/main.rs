// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]

//! The portable verifier: check a VeilVoice release without GnuPG installed.
//!
//! # What this is for
//!
//! Verifying a download by hand needs GnuPG and a SHA-256 tool. That is four
//! commands and two dependencies, and on Windows it is usually neither. This
//! is one binary that does the same checks with nothing else installed: the
//! signing key and its fingerprint are compiled into it.
//!
//! # The one thing it cannot embed
//!
//! It cannot carry the expected hash of the file it is checking. A file cannot
//! contain its own digest -- writing the digest in changes the file, which
//! changes the digest. So the hash has to come from outside, and there are
//! exactly two places it can come from. They prove **different things**, and
//! this tool is careful never to blur them:
//!
//! **From the published `SHA256SUMS`** -- whose signature this tool checks
//! against the embedded key. A match proves the download is *intact*: it is
//! byte-for-byte the file that was published, not a corrupted or substituted
//! one. It says nothing about whether that file corresponds to the source,
//! because whoever published it produced both the file and the list.
//!
//! **Typed in by hand, from a hash somebody else produced** by building the
//! same tagged source themselves. A match proves something strictly stronger:
//! that the published binary is what that source compiles to, on a machine
//! that is not the publisher's. That is *reproducibility*, and it is the only
//! check that does not ultimately rest on trusting whoever signed the release.
//!
//! Most people want the first. The second is what makes the first worth
//! anything, and it needs somebody other than the author to have done a build.
//! `docs/REPRODUCIBLE_BUILDS.md` says how.
//!
//! # What it does not do
//!
//! It does not download anything -- this project has no network code and this
//! binary is not the exception. Fetch the files however you like; this reads
//! them from disk. It does not install anything, and it writes nothing.
//!
//! # In plain words
//!
//! This is the small program you can check a download with before trusting
//! anything else here.
//!
//! It is deliberately tiny and it is on its own: no window, no other pieces, and
//! it does not need any other software installed -- not even the usual signature
//! program. That matters because it is the first thing you run, and the point of
//! it is to be small enough to be worth reading.
//!
//! Double-click it and it looks for a downloaded release nearby and checks it.
//! Give it arguments and it does exactly what you asked.

/// A line of ordinary progress: a step being taken, a check that passed.
///
/// Every `println!` in this program goes through one of these three macros, so
/// the verbosity level is applied in one place rather than remembered at each
/// call. A quiet mode with one loud line left in it is not a quiet mode, and
/// that is exactly what "remember to check the level here" produces.
macro_rules! out {
    ($($arg:tt)*) => {
        if crate::report::level() >= crate::report::Loudness::Normal {
            println!($($arg)*);
        }
    };
}

/// **The answer.** Printed at every level except `--quiet`, where the exit
/// status carries it instead.
///
/// This is what `--brief` is for: the fingerprint, the hash, the verdict on a
/// file. If a reader at `--brief` would be left without the thing they ran the
/// command to find out, the line belongs here rather than in `out!`.
macro_rules! verdict {
    ($($arg:tt)*) => {
        if crate::report::level() >= crate::report::Loudness::Minimal {
            println!($($arg)*);
        }
    };
}

/// The same as [`out`], without the newline, for a progress line that is
/// finished by a later `out!`.
macro_rules! outp {
    ($($arg:tt)*) => {
        if crate::report::level() >= crate::report::Loudness::Normal {
            print!($($arg)*);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    };
}

/// Working detail: a command line, a path, a hash being compared. Only at
/// `--verbose`.
#[allow(unused_macros)]
macro_rules! note {
    ($($arg:tt)*) => {
        if crate::report::level() >= crate::report::Loudness::Everything {
            println!("        {}", format!($($arg)*));
        }
    };
}

mod builder;
mod deps;
mod discover;
mod extracted;
mod fetch;
mod report;

use report::{Loudness, Status};

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pgp::composed::SignedPublicKey;

// ---------------------------------------------------------------------------
// The key, the hashing and the signature check live in `veilvoice-check`.
//
// They were here, in a binary crate, which by construction has no consumers.
// The desktop application was asked for a verify tab, and the choice was to
// link a GUI toolkit into this 1.5 MB single file -- whose smallness is a
// feature, because it is what somebody downloads before they trust anything
// else here -- or to move the arithmetic somewhere both front ends can call it.
//
// This binary is unchanged in what it does and what it prints. `veilvoice-gui`
// is now a second caller rather than a second implementation, and the one place
// a silent accept could come from is the one place there is only one of.
// ---------------------------------------------------------------------------

use veilvoice_check::{digest_from_sums, digests_match, fingerprint_of, FINGERPRINT};

/// The embedded key, with its fingerprint checked against [`FINGERPRINT`].
fn embedded_key() -> Result<SignedPublicKey, String> {
    veilvoice_check::key().map_err(|error| error.to_string())
}

// The two fallible ones are wrapped rather than imported. The library reports a
// typed `Error` so a graphical front end can tell "could not check" from "the
// answer is no"; this program has always reported a sentence and prints it in
// one place, so it flattens them here rather than rewriting every call site to
// say the same thing in a longer way.

/// SHA-256 of a file, as this program's `Result<_, String>`.
fn sha256_file(path: &Path) -> Result<String, String> {
    veilvoice_check::sha256_file(path).map_err(|error| error.to_string())
}

/// Verify a detached signature, as this program's `Result<_, String>`.
fn verify_detached(key: &SignedPublicKey, signature: &str, data: &[u8]) -> Result<(), String> {
    veilvoice_check::verify_detached(key, signature, data).map_err(|error| error.to_string())
}

const USAGE: &str = "\
veilvoice-verify -- check a VeilVoice download, with or without GnuPG

IF YOU ONLY READ ONE LINE
  Put this program in the folder you downloaded to and run it. That is all.
  It finds the release, checks the signature, checks the archive, and checks
  every file you extracted out of it, one by one.

USAGE
  veilvoice-verify
  veilvoice-verify auto [DIRECTORY]
      Find a downloaded release near you and check all of it, with nothing
      else to type. Looks in the directory given, then the current one, then
      beside this program, then your Downloads and Desktop.

      In order, and each step only if the one before it passed:
        1. the signature over SHA256SUMS
        2. every archive, against SHA256SUMS
        3. CONTENTS.sha256, against SHA256SUMS
        4. every file you extracted, against CONTENTS.sha256
        5. all of it again through your own GnuPG, if you have one

      Step 4 is what tells you the program you are about to run is the one
      that was published, rather than only that the zip was. Releases before
      v0.1.15 carry no CONTENTS.sha256 and are checked as far as step 2,
      which it says at the time.

      Entirely offline. This is also what double-clicking the program does.

  veilvoice-verify gnupg [DIRECTORY]
      The same check through the GnuPG on this machine rather than the key
      built into this program. It adds the VeilVoice public key to your
      keyring, tells you it did and how to remove it, runs gpg, and prints
      what gpg said. It also prints the commands so you can run them
      yourself, which is the part no program can do for you: the one telling
      you a download is genuine came out of that download.

  veilvoice-verify key
      Print the signing key this binary carries, and its fingerprint.
      Compare it against README.md and https://tilas01.github.io/veilvoice/

  veilvoice-verify sums <SHA256SUMS> <SHA256SUMS.asc>
      Check the signature over a hash list.

  veilvoice-verify file <FILE> --sums <SHA256SUMS> --sig <SHA256SUMS.asc>
      Check the signature over the hash list, then check FILE against it.
      Proves the download is INTACT.

  veilvoice-verify file <FILE> --sha256 <HEX>
      Check FILE against a hash you supply. If that hash came from somebody
      else's independent build of the same tagged source, this proves the
      release is REPRODUCIBLE -- a stronger claim. See --explain.

  veilvoice-verify hash <FILE>
      Print the SHA-256 of a file. Nothing is verified.

  veilvoice-verify release <TAG> [ASSET]
      Fetch a release and check it, in one step. With no ASSET, downloads and
      verifies the hash list itself; with one, downloads that file too and
      checks it against the signed list.

        veilvoice-verify release v0.1.11
        veilvoice-verify release v0.1.11 veilvoice-v0.1.11-linux-x86_64.tar.gz

  veilvoice-verify --explain
      What 'intact' and 'reproducible' each mean, and why they differ.

BUILDING IT YOURSELF
  A signature says who made a file. Only a build says what it is made of.

  veilvoice-verify deps
      What a build needs on this machine, and which of it is already here.
      Add --install to be offered the missing pieces one at a time, with the
      exact command shown before each question. --yes answers them all in
      advance, which is the same explicit yes given in writing.

  veilvoice-verify build [DIR]
      Build the workspace from source and print the hash of everything a
      release ships. DIR defaults to the current directory.

  veilvoice-verify reproduce [DIR] --sums SHA256SUMS --sig SHA256SUMS.asc
      Build here, then compare against the published hashes for this
      platform. The signature is verified before any hash from the list is
      read.

      It builds for the machine it is on, and compares against the published
      build for that platform. That is not a limitation being apologised for:
      a build needs that platform's headers and linker, and three machines
      give you three platforms verified, which is how a reproducible-build
      claim is normally checked.

      A difference is a FINDING, not an accusation. Both hashes are printed
      and the exit status is 5, which is deliberately not the status that
      means tampering.

  veilvoice-verify install --from DIR --cli --gui
      Copy the binaries in DIR to where a shell will find them. This command
      copies; it does not verify. Check DIR first with `file` or `reproduce`,
      so the yes you give is to one thing rather than two.

THE NETWORK
  Every command above except `release` is entirely offline.

  `release` downloads, and it does so through the tool your operating system
  already ships -- curl on Windows and macOS, curl or wget elsewhere. This
  binary contains no HTTP client: VeilVoice has no networking crate anywhere in
  its dependency graph, which you can check yourself with `cargo tree`.

  Only one host is ever contacted and it is compiled in. There is no way to
  point this at another, no update check, and nothing is fetched unless you
  asked for it on this command line.
";

const EXPLAIN: &str = "\
INTACT and REPRODUCIBLE are different claims
============================================

This tool carries the signing key and its fingerprint. It cannot carry the
expected hash of the file you are checking -- a file cannot contain its own
digest. So the hash comes from outside, and where it came from decides what a
match actually proves.

  1. INTACT -- the hash came from the published SHA256SUMS
  --------------------------------------------------------
  This tool checks the detached signature over that list against the key it
  carries, and then checks your file against the list.

  A match proves your download is byte-for-byte the file that was published:
  not truncated, not corrupted in transit, not swapped by whoever served it
  to you.

  It does NOT prove the file corresponds to the published source. The same
  person produced the binary and the hash list and signed the list. If you
  do not trust them, this check does not help you -- it only proves you got
  what they meant to give you.

  2. REPRODUCIBLE -- the hash came from somebody else's own build
  ---------------------------------------------------------------
  Somebody who is not the publisher checks out the same tag, builds it as
  docs/REPRODUCIBLE_BUILDS.md describes, and produces a hash. You type that
  hash in here.

  A match proves the published binary is what that source compiles to. It
  closes the gap the first check leaves open: that a signed binary could
  contain something the source does not.

  This is the check worth having, and it is the one this project cannot
  perform for you -- it needs somebody other than the author to have done a
  build. Two independent hashes that agree are worth more than any number of
  signatures from one person.

VeilVoice's releases are built twice, in separate directories, and compared
before they ship. That is the publisher checking their own work, which is
worth something and is not the same as somebody else checking it.
";

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn good(message: &str) {
    out!("  ok    {message}");
}

/// A failure that is not a refusal: something did not happen, rather than
/// something was checked and found wrong.
///
/// Kept apart from [`deny`] on purpose. "The download failed" and "the
/// signature is bad" are different facts and a reader must not have to work out
/// which one they were told -- the second means somebody may have tampered
/// with a release, and the first usually means a network hiccup.
fn fail(reason: &str) -> ExitCode {
    if report::level() >= Loudness::Minimal {
        eprintln!();
        eprintln!("FAILED: could not complete the check.");
        for line in reason.lines() {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("This is not a verification failure -- nothing was checked and");
        eprintln!("found wrong. Nothing has been proven either. Try again, or");
        eprintln!("download the files yourself and pass them in.");
    }
    Status::Incomplete.into()
}

/// Every refusal goes through here, so every refusal names the check.
fn deny(reason: &str, detail: &[&str]) -> ExitCode {
    if report::level() >= Loudness::Minimal {
        eprintln!();
        eprintln!("REFUSED: {reason}");
        for line in detail {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Nothing about this download has been proven. Do not run it.");
    }
    Status::Refused.into()
}

/// Nothing could be checked, with the same shape of detail as a refusal.
///
/// The distinction that matters is the one in the last line and in the exit
/// status: a release that is not there has not been found wanting.
fn incomplete_deny(reason: &str, detail: &[&str]) -> ExitCode {
    if report::level() >= Loudness::Minimal {
        eprintln!();
        eprintln!("FAILED: {reason}");
        for line in detail {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Nothing was checked, so nothing has been proven either way.");
    }
    Status::Incomplete.into()
}

/// A file this program was told to read could not be read.
///
/// Nothing was checked and nothing was found wrong, so it carries
/// [`Status::Incomplete`] and says so in the words -- the old code told a
/// reader with a mistyped path that their download might be compromised.
fn cannot(reason: &str, detail: &[&str]) -> ExitCode {
    if report::level() >= Loudness::Minimal {
        eprintln!();
        eprintln!("FAILED: {reason}");
        for line in detail {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Nothing was checked, so nothing has been proven either way.");
    }
    Status::Incomplete.into()
}

/// The command line could not be understood, so nothing was attempted.
///
/// Kept apart from [`deny`] because they are different facts and the old code
/// printed the same words for both. A mistyped path is not a reason to tell
/// somebody their download may have been tampered with, and "do not run it"
/// is nonsense advice when nothing was examined. It also carries
/// [`Status::Usage`], so a script can tell its own mistake from a finding.
fn usage(reason: &str, detail: &[&str]) -> ExitCode {
    if report::level() >= Loudness::Minimal {
        eprintln!();
        eprintln!("USAGE: {reason}");
        for line in detail {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Nothing was checked. Fix the command and run it again.");
    }
    Status::Usage.into()
}

// ---------------------------------------------------------------------------
// The key
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn command_key() -> ExitCode {
    match embedded_key() {
        Err(why) => deny("the key compiled into this binary is not usable", &[&why]),
        Ok(key) => {
            out!();
            verdict!("  fingerprint  {}", fingerprint_of(&key));
            for uid in key.details.users.iter() {
                verdict!("  user id      {}", String::from_utf8_lossy(uid.id.id()));
            }
            out!();
            out!("  Compare that fingerprint against the one published in README.md,");
            out!("  on https://tilas01.github.io/veilvoice/ and in the release notes.");
            out!("  If they disagree, stop.");
            out!();
            ExitCode::SUCCESS
        }
    }
}

fn command_sums(sums_path: &Path, sig_path: &Path) -> ExitCode {
    let key = match embedded_key() {
        Ok(key) => key,
        Err(why) => return deny("the key compiled into this binary is not usable", &[&why]),
    };
    good(&format!("embedded key fingerprint {FINGERPRINT}"));

    let sums = match std::fs::read(sums_path) {
        Ok(bytes) => bytes,
        Err(e) => return cannot("the hash list could not be read", &[&format!("{e}")]),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(e) => return cannot("the signature could not be read", &[&e]),
    };

    match verify_detached(&key, &signature, &sums) {
        Ok(()) => {
            good("signature over the hash list is good");
            out!();
            verdict!("  That hash list is genuinely the one signed by {FINGERPRINT}.");
            out!("  It does not yet say anything about any particular file --");
            out!("  use `veilvoice-verify file` for that.");
            out!();
            ExitCode::SUCCESS
        }
        Err(why) => deny(
            "the signature over the hash list is not valid",
            &[
                &why,
                "",
                "The list is not the one this key signed. Every hash in it is",
                "therefore worthless, and a file that matches it proves nothing.",
            ],
        ),
    }
}

fn command_file_against_sums(file: &Path, sums_path: &Path, sig_path: &Path) -> ExitCode {
    let key = match embedded_key() {
        Ok(key) => key,
        Err(why) => return deny("the key compiled into this binary is not usable", &[&why]),
    };
    good(&format!("embedded key fingerprint {FINGERPRINT}"));

    let sums_bytes = match std::fs::read(sums_path) {
        Ok(bytes) => bytes,
        Err(e) => return cannot("the hash list could not be read", &[&format!("{e}")]),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(e) => return cannot("the signature could not be read", &[&e]),
    };

    // The signature first, always. Checking the file against the list first
    // would prove only that it matches a list that might itself be forged.
    if let Err(why) = verify_detached(&key, &signature, &sums_bytes) {
        return deny(
            "the signature over the hash list is not valid",
            &[
                &why,
                "",
                "Nothing was compared against it: an unverified hash list is not",
                "a thing worth comparing against.",
            ],
        );
    }
    good("signature over the hash list is good");

    let sums = String::from_utf8_lossy(&sums_bytes).into_owned();
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let expected = match digest_from_sums(&sums, &name) {
        Some(digest) => digest,
        None => {
            return deny(
                "that file is not listed in the hash list",
                &[
                    &format!("looked for: {name}"),
                    "",
                    "The signature was good, so the list is genuine -- it simply does",
                    "not mention this file. That means this file was not part of this",
                    "release.",
                ],
            )
        }
    };

    let actual = match sha256_file(file) {
        Ok(digest) => digest,
        Err(e) => return cannot("the file could not be hashed", &[&e]),
    };

    if !digests_match(&expected, &actual) {
        return deny(
            "the file does not match the signed hash list",
            &[
                &format!("expected  {expected}"),
                &format!("found     {actual}"),
                "",
                "The download is not what was published. It is corrupt, truncated,",
                "or not the file it claims to be.",
            ],
        );
    }

    good(&format!("sha256 matches ({actual})"));
    out!();
    verdict!("  INTACT. This file is byte-for-byte what was published, signed by");
    verdict!("  {FINGERPRINT}.");
    out!();
    out!("  That is not the same as knowing it was built from the published");
    out!("  source -- the same person produced the binary and the list. For");
    out!("  that, compare against a hash somebody else produced from their own");
    out!("  build:  veilvoice-verify --explain");
    out!();
    ExitCode::SUCCESS
}

fn command_file_against_hash(file: &Path, expected: &str) -> ExitCode {
    let cleaned = expected.trim();
    if cleaned.len() != 64 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return usage(
            "that does not look like a SHA-256 hash",
            &[
                &format!("got: {cleaned}"),
                "",
                "A SHA-256 digest is exactly 64 hexadecimal characters.",
                "Refusing rather than comparing, because a comparison against a",
                "malformed value would fail and look like a corrupt download.",
            ],
        );
    }

    let actual = match sha256_file(file) {
        Ok(digest) => digest,
        Err(e) => return cannot("the file could not be hashed", &[&e]),
    };

    if !digests_match(cleaned, &actual) {
        return deny(
            "the file does not match the hash you supplied",
            &[
                &format!("expected  {cleaned}"),
                &format!("found     {actual}"),
                "",
                "If that hash came from somebody else's build of the same tag, this",
                "means the two builds did not produce the same bytes. That is worth",
                "reporting -- it is either a broken reproducible build or something",
                "much worse.",
            ],
        );
    }

    good(&format!("sha256 matches ({actual})"));
    out!();
    verdict!("  This file matches the hash you gave.");
    out!();
    out!("  What that proves depends entirely on where the hash came from, and");
    out!("  only you know that:");
    out!();
    out!("    - from the published SHA256SUMS: the download is INTACT.");
    out!("    - from somebody else's independent build of the same tag: the");
    out!("      release is REPRODUCIBLE, which is the stronger claim.");
    out!();
    out!("  This tool cannot tell which, so it does not guess.");
    out!("  veilvoice-verify --explain");
    out!();
    ExitCode::SUCCESS
}

fn command_hash(file: &Path) -> ExitCode {
    match sha256_file(file) {
        Ok(digest) => {
            verdict!(
                "{digest}  {}",
                file.file_name().unwrap_or_default().to_string_lossy()
            );
            ExitCode::SUCCESS
        }
        Err(e) => cannot("the file could not be hashed", &[&e]),
    }
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

fn take_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

/// Fetch a release and check it, in one step.
///
/// The order is the same one every other path in this tool takes and the same
/// one the install scripts take: **the signature over the hash list first**,
/// then the file against that list. Checking the hash first would prove only
/// that a download matches a list which might itself have been replaced.
///
/// Downloads go to a directory the caller can inspect afterwards. Nothing is
/// deleted on success: somebody who has just verified a release usually wants
/// the release.
fn command_release(tag: &str, asset: Option<&str>) -> ExitCode {
    if !fetch::valid_tag(tag) {
        return usage(
            "that does not look like a release tag",
            &["veilvoice-verify release v0.1.11"],
        );
    }
    if let Some(name) = asset {
        if !fetch::valid_asset(name) {
            return usage(
                "that does not look like a release file name",
                &["veilvoice-verify release v0.1.11 veilvoice-v0.1.11-linux-x86_64.tar.gz"],
            );
        }
    }

    // A directory named for the tag, in the working directory rather than a
    // temporary one: these are files the user asked for and will want to keep,
    // and writing them somewhere the system may clear is a surprise.
    let directory = PathBuf::from(format!("veilvoice-{tag}"));
    if let Err(error) = std::fs::create_dir_all(&directory) {
        return fail(&format!(
            "could not create {}: {error}",
            directory.display()
        ));
    }

    out!();
    out!("  fetching into {}", directory.display());

    let mut fetched = Vec::new();
    for name in [fetch::SUMS, fetch::SIGNATURE] {
        let url = fetch::asset_url(tag, name);
        outp!("  {name} ... ");
        match fetch::download(&url, &directory.join(name)) {
            Ok(path) => {
                out!("ok");
                fetched.push(path);
            }
            Err(error) => {
                out!("failed");
                return fail(&error);
            }
        }
    }

    let sums = &fetched[0];
    let signature = &fetched[1];

    let Some(name) = asset else {
        // No asset named: check the list's signature and stop there, which is
        // a complete and useful answer on its own.
        return command_sums(sums, signature);
    };

    let url = fetch::asset_url(tag, name);
    outp!("  {name} ... ");
    let archive = match fetch::download(&url, &directory.join(name)) {
        Ok(path) => {
            out!("ok");
            path
        }
        Err(error) => {
            out!("failed");
            return fail(&error);
        }
    };

    command_file_against_sums(&archive, sums, signature)
}

/// Find a release near the user and check it, with nothing else to type.
///
/// The command somebody who has just downloaded an archive actually wants.
/// Everything it does is offline: it looks in a few obvious places, and if it
/// finds an archive with its hash list and signature beside it, it runs exactly
/// the same check `file --sums --sig` runs.
///
/// A directory holding an archive but no hash list is **reported**, never
/// completed from a hash list found somewhere else -- that would be checking one
/// release against another release's list, and it would say "verified".
fn command_auto(explicit: Option<&Path>) -> ExitCode {
    let (complete, all) = discover::search(explicit);

    if all.is_empty() {
        return incomplete_deny(
            "no VeilVoice release was found to check",
            &[
                "Looked in: the directory given, the current directory, the folder this",
                "program is in, and your Downloads and Desktop.",
                "",
                "Put the archive, SHA256SUMS and SHA256SUMS.asc in one folder and run this",
                "again from there -- or name the folder:",
                "",
                "  veilvoice-verify auto <DIRECTORY>",
            ],
        );
    }

    let Some(found) = complete else {
        // Something turned up and it cannot be checked. Say exactly what is
        // missing, in each place, rather than a single unhelpful refusal.
        let mut detail: Vec<String> = vec![
            "Found something, but not a set that can be checked offline.".to_string(),
            "A check needs the archive, SHA256SUMS and SHA256SUMS.asc together.".to_string(),
            String::new(),
        ];
        for place in &all {
            detail.push(format!("  {}", place.directory.display()));
            for archive in &place.archives {
                detail.push(format!(
                    "    {}",
                    archive.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            detail.push(format!("    -- {}", place.missing().join(", ")));
        }
        detail.push(String::new());
        detail.push("Both missing files are on the release page beside the archive.".to_string());
        let borrowed: Vec<&str> = detail.iter().map(String::as_str).collect();
        return incomplete_deny("a release was found but cannot be checked", &borrowed);
    };

    out!("Checking what is in {}", found.directory.display());
    out!();
    let sums = found.sums.clone().unwrap_or_default();
    let signature = found.signature.clone().unwrap_or_default();

    let mut worst = ExitCode::SUCCESS;
    let mut failures = 0usize;
    for archive in &found.archives {
        let outcome = command_file_against_sums(archive, &sums, &signature);
        // `ExitCode` cannot be compared, so failures are counted instead. Any
        // one archive failing has to fail the whole run: a set where three
        // files are good and one is not is not a good download.
        if format!("{outcome:?}") != format!("{:?}", ExitCode::SUCCESS) {
            failures += 1;
            worst = outcome;
        }
        out!();
    }
    if failures > 0 {
        verdict!(
            "{failures} of {} did not check out. Nothing above should be run.",
            found.archives.len()
        );
        return worst;
    }

    // Marker 97. Both of these now check rather than describe, so both can
    // fail the run. A verifier that prints "CHANGED veilvoice" and then exits
    // zero has told somebody nothing they will act on.
    let wrong = report_extracted(&found) + report_gnupg(&found);
    if wrong > 0 {
        verdict!("  {wrong} thing(s) above did not check out.");
        return Status::Refused.into();
    }
    worst
}

/// Marker 97. Every file in the extracted folder, against the signed list.
///
/// # What changed, and why the old caveat is gone
///
/// This used to report that the programs were present and runnable, and then
/// say in as many words that it could not tell whether the folder came out of
/// the archive it had just checked. That was true: `SHA256SUMS` covers the
/// archives, nothing on disk records what a directory was extracted from, and
/// no signed list covered the loose files.
///
/// A release now publishes `CONTENTS.sha256`, which lists every file inside
/// every archive with its SHA-256 and is itself covered by `SHA256SUMS` and so
/// by the signature. The chain is complete:
///
/// ```text
/// SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk
/// ```
///
/// So the question is now answered rather than deferred. Where a release does
/// not carry that file -- everything published before v0.1.15 -- the old report
/// and the old caveat are what is printed, because they were honest and still
/// are.
///
/// Returns how many things were wrong, so the caller can fail the run.
fn report_extracted(found: &discover::Found) -> usize {
    let published = match manifest(found) {
        Manifest::None => None,
        Manifest::Unusable(why) => {
            out!("Beside it, the extracted folder");
            out!("  the release published a contents list and it could not be used:");
            out!("  {why}");
            out!();
            out!("  Nothing about the extracted folder was checked. Do not treat what");
            out!("  is in it as verified.");
            out!();
            return 1;
        }
        Manifest::Ready(all) => Some(all),
    };

    let mut problems = 0usize;
    let mut looked = false;
    for archive in &found.archives {
        let Some(directory) = extracted::directory_for(archive) else {
            continue;
        };
        if !directory.is_dir() {
            continue;
        }
        looked = true;
        out!("Beside it, {}", directory.display());

        let name = archive
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let section = published
            .as_ref()
            .and_then(|all| veilvoice_check::contents::for_archive(all, &name));

        match section {
            Some(section) => problems += report_against_manifest(&found.directory, section),
            None => {
                if published.is_some() {
                    out!("  the contents list does not mention {name}, so nothing in this");
                    out!("  folder could be checked against it");
                    problems += 1;
                }
                report_presence_only(&directory);
            }
        }
    }

    if looked && published.is_none() {
        out!();
        out!("  This release published no list of what is inside its archives, so");
        out!("  the folder can only be reported on, not checked. The archive above");
        out!("  is the one that was signed; nothing on disk records that this folder");
        out!("  came out of it. To be certain, extract the checked archive again.");
        out!();
    } else if looked {
        out!();
    }
    problems
}

/// What the release said is inside its archives, if anything usable.
enum Manifest {
    /// No contents list was published beside the archive.
    None,
    /// One was, and it cannot be trusted or read.
    Unusable(String),
    /// One was, and it is genuine.
    Ready(Vec<veilvoice_check::contents::ArchiveContents>),
}

/// Read `CONTENTS.sha256`, having first proved it is the published one.
///
/// The order is this program's usual one and it matters more here than
/// anywhere else: this file decides which paths get read and what they are
/// compared against, so checking it against the signed hash list **before**
/// parsing it is the difference between a verifier and a program that does what
/// a downloaded text file tells it to.
fn manifest(found: &discover::Found) -> Manifest {
    let Some(path) = &found.contents else {
        return Manifest::None;
    };
    let (Some(sums_path), Some(sig_path)) = (&found.sums, &found.signature) else {
        return Manifest::Unusable("there is no signed hash list to check it against".to_string());
    };
    let sums = match read_text(sums_path) {
        Ok(text) => text,
        Err(why) => return Manifest::Unusable(why),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(why) => return Manifest::Unusable(why),
    };
    match veilvoice_check::check_file(path, &sums, &signature) {
        Err(why) => Manifest::Unusable(format!("{why}")),
        Ok(checked) if !checked.matched => Manifest::Unusable(
            "it is not the list this release signed: the hashes do not agree".to_string(),
        ),
        Ok(_) => match read_text(path) {
            Err(why) => Manifest::Unusable(why),
            Ok(text) => match veilvoice_check::contents::parse(&text) {
                Ok(all) => Manifest::Ready(all),
                Err(why) => Manifest::Unusable(format!("{why}")),
            },
        },
    }
}

/// Check one extracted folder against the section of the list that covers it.
fn report_against_manifest(
    root: &Path,
    section: &veilvoice_check::contents::ArchiveContents,
) -> usize {
    use veilvoice_check::contents::Verdict;

    let outcomes = veilvoice_check::contents::check(root, section);
    let sweep = veilvoice_check::contents::extras(root, section);
    let as_published = outcomes.iter().filter(|o| o.is_good()).count();

    for outcome in &outcomes {
        match &outcome.verdict {
            // The ones that pass are counted rather than listed. A release
            // carries about seventy files and printing every one of them buries
            // the three lines somebody actually needs to read.
            Verdict::Matches => {}
            Verdict::Differs { found } => {
                verdict!("  CHANGED  {}", outcome.path);
                note!("expected {}", digest_for(section, &outcome.path));
                note!("found    {found}");
            }
            Verdict::Missing => verdict!("  MISSING  {}", outcome.path),
            Verdict::Unreadable(why) => verdict!("  UNREADABLE  {}: {why}", outcome.path),
            // F-99. Not hashed, and not a pass whatever it points at.
            Verdict::NotAFile(what) => {
                verdict!("  {what} WHERE A FILE SHOULD BE  {}", outcome.path)
            }
        }
    }
    for extra in &sweep.extras {
        verdict!(
            "  NOT PART OF THE RELEASE  {}",
            extra.strip_prefix(root).unwrap_or(extra).display()
        );
    }
    // F-98. A directory that could not be opened is a directory whose contents
    // are unknown, and unknown is not empty. Saying "there is nothing else in
    // the folder" after failing to look in part of it is the false assurance
    // this whole program exists to avoid giving.
    for shut in &sweep.unreadable {
        verdict!(
            "  COULD NOT LOOK INSIDE  {}",
            shut.strip_prefix(root).unwrap_or(shut).display()
        );
    }

    let wrong = outcomes.len() - as_published + sweep.extras.len() + sweep.unreadable.len();
    if wrong == 0 {
        good(&format!(
            "all {} files match the signed list, and there is nothing else in the folder",
            outcomes.len()
        ));
        report_runnable(root, section);
    } else {
        out!();
        verdict!(
            "  {as_published} of {} files are as published.",
            outcomes.len()
        );
        out!();
        out!("  A file that has changed, gone missing, or arrived from somewhere");
        out!("  else is not what was signed. Extract the checked archive again and");
        out!("  use what comes out of it.");
        if !sweep.unreadable.is_empty() {
            out!();
            out!("  A folder listed above could not be opened, so what is in it is");
            out!("  unknown rather than absent. Check its permissions and run this");
            out!("  again.");
        }
    }
    wrong
}

/// The published hash for one path, for a `--verbose` line.
fn digest_for(section: &veilvoice_check::contents::ArchiveContents, path: &str) -> String {
    section
        .members
        .iter()
        .find(|m| m.path == path)
        .map(|m| m.digest.clone())
        .unwrap_or_default()
}

/// Whether the operating system will run the programs that are there.
///
/// The other half of what an extracted folder can be wrong about, and it
/// survives the manifest: a hash says a file is byte for byte correct, and an
/// unpacking tool that dropped the execute bit leaves that correct file
/// unrunnable.
fn report_runnable(root: &Path, section: &veilvoice_check::contents::ArchiveContents) {
    let mut stuck = Vec::new();
    for directory in section.roots() {
        let here = extracted::look_in(&root.join(&directory));
        for program in here.not_runnable() {
            stuck.push(program.path.clone());
        }
    }
    if stuck.is_empty() {
        return;
    }
    out!();
    for path in &stuck {
        verdict!(
            "  present and correct, but your system will NOT run it: {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    out!();
    out!("  Some tools drop the execute bit when they unpack an archive.");
    out!("  On Linux or macOS: chmod +x <the files above>");
}

/// The old report, for a release that published no contents list.
fn report_presence_only(directory: &Path) {
    let here = extracted::look_in(directory);
    if here.is_empty() {
        out!("  nothing that looks like a VeilVoice program is in it");
        return;
    }
    for program in &here.programs {
        let name = program
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        if program.runnable {
            out!("  {name}: present, and your system will run it");
        } else {
            out!("  {name}: present, but your system will NOT run it");
        }
    }
    if !here.not_runnable().is_empty() {
        out!();
        out!("  Some tools drop the execute bit when they unpack an archive.");
        out!("  On Linux or macOS: chmod +x {}/*", directory.display());
    }
}

/// Marker 97. The same check, run through the GnuPG the reader already has.
///
/// Two implementations, both reported. This program checked the signature with
/// a key compiled into itself, and it came out of the same download; GnuPG's
/// answer is arrived at by somebody else's code. Where the two disagree, that
/// is the loudest thing this tool can find and it fails the run.
///
/// **GnuPG failing to run is not a disagreement.** A missing keyring directory,
/// a read-only home, an agent that will not start: none of those is a statement
/// about the file that was downloaded, and counting them as refusals would tell
/// somebody not to run a release that is entirely sound. Only an answer counts,
/// and only a bad answer counts against.
///
/// The commands are still printed, every time. Running GnuPG from inside the
/// binary under suspicion makes the *implementation* independent and does not
/// make the *invocation* independent, and only the reader can supply that.
///
/// Returns how many things were wrong.
fn report_gnupg(found: &discover::Found) -> usize {
    let sums = found.sums.clone().unwrap_or_default();
    let signature = found.signature.clone().unwrap_or_default();
    let key = found.directory.join("veilvoice-signing-key.asc");
    let key = key.is_file().then_some(key);

    out!("Checking it again with your own GnuPG");
    let mut problems = 0usize;
    match veilvoice_gnupg::Gnupg::found() {
        Err(why) => {
            out!("  {why}. These are the commands if you install it.");
        }
        Ok(gpg) => {
            out!("  found at {}", gpg.program().display());
            // The key first, because GnuPG cannot check a signature by a key it
            // has never seen, and asking somebody to import it by hand is the
            // step at which almost everybody stops.
            match gpg.import(veilvoice_check::PUBLIC_KEY, FINGERPRINT) {
                // Not counted against the release. GnuPG being unusable on this
                // machine -- no keyring directory, a read-only home, an agent
                // that will not start -- says nothing whatever about the file
                // that was downloaded, and reporting it as a refusal would tell
                // somebody not to run a download that is perfectly good.
                Err(why) => {
                    out!("  the signing key could not be added to your keyring: {why}");
                    out!("  so GnuPG could not be asked. Nothing about the download changed.");
                }
                Ok(import) => {
                    for line in import.note() {
                        out!("  {line}");
                    }
                    match gpg.verify(&signature, &sums, FINGERPRINT) {
                        // Again: GnuPG failing to run is the machine, not the
                        // release.
                        Err(why) => {
                            out!("  GnuPG could not check the signature: {why}");
                            out!("  Nothing about the download changed.");
                        }
                        Ok(run) => {
                            out!();
                            for line in &run.status {
                                note!("{line}");
                            }
                            if run.outcome.is_good() {
                                good(&run.outcome.plainly());
                            } else {
                                verdict!("  {}", run.outcome.plainly());
                                // What GnuPG printed, shown rather than
                                // summarised away. A verifier that reports a
                                // refusal without the evidence for it is asking
                                // to be taken on trust.
                                for line in run.said.lines() {
                                    out!("    {line}");
                                }
                                problems += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    out!();
    out!("  And the same thing, typed by you, which is the part this program");
    out!("  cannot do for itself:");
    out!();
    for line in extracted::gnupg_commands(&sums, &signature, key.as_deref()) {
        out!("    {line}");
    }
    out!();
    out!("  Worth doing. This program checked the signature with a key built into");
    out!("  itself, and it came out of the same download you are checking. The");
    out!("  fingerprint on the website is the independent answer.");
    out!();
    problems
}

/// Marker 91. Print the commands that check this release with somebody else's
/// GnuPG, and nothing else.
///
/// A separate subcommand rather than only a footnote under `auto`, because the
/// person who wants this is the person who does not want to be told the answer
/// by this binary. Making them run the full check first to reach the commands
/// would be the wrong way round.
fn command_gnupg(explicit: Option<&Path>) -> ExitCode {
    let (complete, all) = discover::search(explicit);
    let found = complete.or_else(|| all.into_iter().next());

    let Some(found) = found else {
        return incomplete_deny(
            "no VeilVoice release was found",
            &[
                "Looked in: the directory given, the current directory, the folder this",
                "program is in, and your Downloads and Desktop.",
                "",
                "  veilvoice-verify gnupg <DIRECTORY>",
            ],
        );
    };

    out!("In {}", found.directory.display());
    out!();
    let wrong = report_gnupg(&found);
    out!("  The fingerprint to compare against is on the release page and in");
    out!("  README.md. `veilvoice-verify key` prints the one this binary carries.");
    if wrong > 0 {
        return Status::Refused.into();
    }
    ExitCode::SUCCESS
}

/// Keep the window open when there was nobody watching a terminal.
///
/// A console program started by double-clicking gets a console of its own, and
/// that console closes the instant the process exits -- so the usage text this
/// used to print flashed past and vanished. Reported as "veilvoice-verify
/// crashes on launch", and reasonably so: from the outside a window that
/// appears and disappears is indistinguishable from one that fell over.
///
/// Detecting a double-click properly means asking Windows how many processes
/// share this console, which is FFI, and every crate here carries
/// `#![forbid(unsafe_code)]`. **No arguments** is the safe stand-in: somebody
/// running this from a terminal almost always types a subcommand, and somebody
/// who types the bare name gets one extra keypress.
fn wait_before_the_window_closes() {
    out!();
    out!("Press Enter to close.");
    let mut discard = String::new();
    let _ = std::io::stdin().read_line(&mut discard);
}

// ---------------------------------------------------------------------------
// Building it yourself
// ---------------------------------------------------------------------------

/// What this machine needs before it can build VeilVoice.
///
/// `install` here means *offer*: every missing thing is named, described, and
/// the exact command line is shown before the question. Nothing runs without a
/// yes typed by a person, or `--yes` typed on this command line -- which is the
/// same explicit yes, given in advance and in writing.
fn command_deps(offer_to_install: bool, always_yes: bool) -> ExitCode {
    out!("What a build of VeilVoice needs on this machine");
    out!();

    let (satisfied, absent) = builder::report_dependencies();

    if absent.is_empty() {
        verdict!("Everything a build needs is here.");
        return Status::Success.into();
    }

    if !offer_to_install {
        out!();
        out!("Nothing has been installed. `veilvoice-verify deps --install` offers to.");
        return if satisfied {
            // Only optional things are missing, so a build will still work --
            // with less in it. That is not a failure.
            verdict!("A build will work. Live mode will not be built.");
            Status::Success.into()
        } else {
            Status::DependenciesMissing.into()
        };
    }

    let mut failures = Vec::new();
    for need in &absent {
        let route = need.route();
        let Some(line) = route.command_line() else {
            // Nothing to run: the route is something a person does themselves,
            // and it was printed above with the reason.
            continue;
        };
        out!();
        let agreed =
            always_yes || builder::agreed(&format!("Run: {line}\n  Install {}?", need.name));
        if !agreed {
            out!("  skipped {}", need.name);
            continue;
        }
        if let Err(why) = builder::install(need) {
            failures.push(format!("{}: {why}", need.name));
        }
    }

    for line in &failures {
        out!("  FAILED  {line}");
    }

    // Asked again rather than assumed. An installer exiting zero is not the
    // same as the header being where the compiler will look for it.
    let (still_required, _) = deps::missing();
    if still_required.is_empty() {
        verdict!("Everything a build needs is here.");
        Status::Success.into()
    } else {
        verdict!(
            "{} still missing: {}",
            still_required.len(),
            still_required
                .iter()
                .map(|need| need.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        Status::DependenciesMissing.into()
    }
}

/// Build the workspace from source and hash what came out.
///
/// No comparison: this is the half somebody runs to get a build, and it says
/// what it produced. `reproduce` is the half that checks it against a release.
fn command_build(root: &Path, target_dir: Option<&Path>) -> ExitCode {
    let (built, code) = match do_build(root, target_dir) {
        Ok(built) => built,
        Err(code) => return code,
    };
    let _ = code;

    out!();
    for (name, digest) in &built.files {
        verdict!("{digest}  {name}");
    }
    for name in &built.absent {
        out!("  --    {name} was not built (it may be behind a feature here)");
    }
    Status::Success.into()
}

/// Everything both build commands do before they differ.
fn do_build(
    root: &Path,
    target_dir: Option<&Path>,
) -> Result<(builder::Built, ExitCode), ExitCode> {
    if let Err(why) = builder::looks_like_the_source(root) {
        return Err(usage("that is not VeilVoice's source tree", &[&why]));
    }

    let (satisfied, _) = builder::report_dependencies();
    if !satisfied {
        out!();
        out!("`veilvoice-verify deps --install` offers to install them.");
        return Err(Status::DependenciesMissing.into());
    }

    // Said before the build rather than after, because these are the first
    // things to check when two builds disagree and afterwards nobody scrolls
    // back. The environment especially: a comparison whose settings are
    // invisible cannot be checked by the person reading the result.
    if let Some(pinned) = builder::pinned_toolchain(root) {
        good(&format!("the source pins Rust {pinned}"));
    }
    let environment = match builder::environment(root, target_dir) {
        Ok(environment) => environment,
        Err(why) => {
            return Err(cannot(
                "the build environment could not be worked out",
                &[&why],
            ))
        }
    };
    good(&format!("building for {}", environment.triple));
    out!();
    out!("  The settings a release is built with, reproduced here:");
    for line in environment.describe() {
        out!("    {line}");
    }
    if environment.source_date_epoch.is_none() {
        out!();
        out!("  Without a commit date this build may differ from the published one");
        out!("  for that reason alone. Build from a git checkout to rule it out.");
    }

    out!();
    out!("Building. This takes a few minutes the first time.");
    let dir = match builder::build(root, target_dir, &environment) {
        Ok(dir) => dir,
        Err(why) => {
            if report::level() >= Loudness::Minimal {
                eprintln!();
                eprintln!("{why}");
            }
            return Err(Status::BuildFailed.into());
        }
    };
    good("the build finished");
    note!("binaries in {}", dir.display());

    match builder::hash_what_was_built(&dir) {
        Ok(built) => Ok((built, Status::Success.into())),
        Err(why) => Err(cannot("the build left nothing to hash", &[&why])),
    }
}

/// Build here, and compare against the published hashes for this platform.
///
/// **The signature is verified before any hash from the list is read**, by the
/// same code path `sums` uses. A hash list that has not been verified is a
/// list of numbers somebody sent you.
fn command_reproduce(
    root: &Path,
    sums_path: &Path,
    sig_path: &Path,
    target_dir: Option<&Path>,
) -> ExitCode {
    let key = match embedded_key() {
        Ok(key) => key,
        Err(why) => return deny("the key compiled into this binary is not usable", &[&why]),
    };
    let sums_bytes = match std::fs::read(sums_path) {
        Ok(bytes) => bytes,
        Err(e) => return cannot("the hash list could not be read", &[&format!("{e}")]),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(e) => return cannot("the signature could not be read", &[&e]),
    };
    if let Err(why) = verify_detached(&key, &signature, &sums_bytes) {
        return deny(
            "the signature over the hash list is not valid",
            &[
                &why,
                "",
                "Nothing was built and nothing was compared. A hash list that",
                "does not verify is a list of numbers somebody sent you.",
            ],
        );
    }
    good(&format!("the hash list is signed by {FINGERPRINT}"));
    let sums = String::from_utf8_lossy(&sums_bytes).into_owned();

    let (built, _) = match do_build(root, target_dir) {
        Ok(pair) => pair,
        Err(code) => return code,
    };

    let comparison = builder::compare(&built, &sums);
    out!();
    let mut differed = Vec::new();
    for one in &comparison {
        match one {
            builder::Compared::Same { name, digest } => {
                good(&format!("{name} matches the published build"));
                note!("{digest}");
            }
            builder::Compared::Different {
                name,
                built,
                published,
            } => {
                differed.push(name.clone());
                if report::level() >= Loudness::Minimal {
                    println!("  DIFFERS  {name}");
                    println!("    built here  {built}");
                    println!("    published   {published}");
                }
            }
            builder::Compared::NotPublished { name } => {
                out!("  --    {name} is not in the published list, so nothing to compare");
            }
        }
    }

    let status = builder::status_for(&comparison);
    out!();
    match status {
        Status::Success => {
            verdict!("REPRODUCIBLE. What was published is what this source builds.");
            out!();
            out!("  You did not take anybody's word for that. You built it.");
        }
        Status::NotReproducible => {
            verdict!(
                "NOT REPRODUCIBLE here: {} file(s) differ -- {}",
                differed.len(),
                differed.join(", ")
            );
            if report::level() >= Loudness::Minimal {
                println!();
                println!("  This is a finding, not an accusation. Most causes are dull: a");
                println!("  different compiler version, a path baked into a panic message,");
                println!("  a timestamp. Both hashes are above so somebody else can check.");
                println!();
                println!("  Worth reporting either way, with the compiler version and the");
                println!("  platform above.");
            }
        }
        _ => {
            verdict!("Nothing could be compared: none of what was built is in that list.");
            if report::level() >= Loudness::Minimal {
                println!();
                println!("  That is not a pass. The list may be for another platform --");
                println!(
                    "  check that it is the SHA256SUMS for {}.",
                    builder::host_triple().unwrap_or_else(|| "this platform".into())
                );
            }
        }
    }
    status.into()
}

/// Put binaries where a shell will find them.
///
/// Only from a directory this program was pointed at, and it says which files
/// it copied and to where. It does **not** verify anything itself: `file`,
/// `auto` and `reproduce` are how a directory earns being installed from, and
/// folding a check into a copy would mean two things happening under one yes.
fn command_install(from: &Path, cli: bool, gui: bool) -> ExitCode {
    let Some(destination) = veilvoice_setup::install::bin_dir() else {
        return cannot(
            "this system offers no per-user program directory",
            &["Copy the binaries wherever you keep programs."],
        );
    };
    if !from.is_dir() {
        return usage(
            "that is not a directory of binaries",
            &[&format!("{} is not there", from.display())],
        );
    }

    let mut wanted: Vec<&str> = Vec::new();
    if cli {
        wanted.push("veilvoice");
    }
    if gui {
        wanted.push("veilvoice-gui");
    }
    if wanted.is_empty() {
        return usage(
            "nothing was selected to install",
            &["veilvoice-verify install --from <DIR> --cli --gui"],
        );
    }

    if let Err(e) = std::fs::create_dir_all(&destination) {
        return cannot(
            "the install directory could not be made",
            &[&format!("{}: {e}", destination.display())],
        );
    }

    let mut copied = Vec::new();
    for name in wanted {
        let file = builder::with_platform_extension(name);
        let source = from.join(&file);
        if !source.is_file() {
            return cannot(
                "one of the binaries asked for is not there",
                &[&format!("{}", source.display())],
            );
        }
        let target = destination.join(&file);
        if let Err(e) = std::fs::copy(&source, &target) {
            return cannot(
                "a binary could not be copied",
                &[&format!(
                    "{} -> {}: {e}",
                    source.display(),
                    target.display()
                )],
            );
        }
        good(&format!("installed {file}"));
        note!("{} -> {}", source.display(), target.display());
        copied.push(file);
    }

    verdict!(
        "{} installed to {}",
        copied.join(", "),
        destination.display()
    );
    let status = veilvoice_setup::install::status();
    if !status.on_path {
        out!();
        out!(
            "  {} is not on this terminal's PATH.",
            destination.display()
        );
        out!("  Add it, or run the binaries by their full path.");
    }
    Status::Success.into()
}

/// Print something the reader asked for by name, at any level.
///
/// The one exception to "every line goes through the level", and it is narrow
/// on purpose: `--help`, `--explain`, `--version` and `--exit-status` are not
/// reports about a check. Somebody who types `--help` wants the help, whatever
/// else they passed, and a `--quiet --help` that prints nothing is a bug
/// dressed as consistency.
///
/// One function rather than four bare `print!` calls, so the exception has a
/// single place, a stated reason, and one line for the source-level test to
/// know about.
fn asked_for(text: &str) {
    print!("{text}");
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // Before a single line is printed, and before the arguments are read for
    // anything else: `--quiet` has to mean quiet from the first word, and the
    // verbosity flags are not positional.
    report::set_level(Loudness::take_from(&mut args));

    // No arguments is very probably a double-click. Do the useful thing --
    // look for a release nearby and check it -- and then wait, so the window
    // does not vanish before it has been read.
    if args.is_empty() {
        let outcome = command_auto(None);
        wait_before_the_window_closes();
        return outcome;
    }

    if args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        // The tables are printed from the code that defines them rather than
        // written out again here. The quiet level is only usable because the
        // statuses are documented, and a second copy of that documentation is
        // a copy that goes stale.
        asked_for(&format!(
            "{USAGE}\n{}\n{}",
            Loudness::table(),
            Status::table()
        ));
        return ExitCode::SUCCESS;
    }
    if args[0] == "--explain" {
        asked_for(EXPLAIN);
        return ExitCode::SUCCESS;
    }
    if args[0] == "--version" {
        asked_for(&format!("veilvoice-verify {}\n", env!("CARGO_PKG_VERSION")));
        return ExitCode::SUCCESS;
    }
    if args[0] == "--exit-status" {
        asked_for(&Status::table());
        return ExitCode::SUCCESS;
    }

    match args[0].as_str() {
        "auto" => command_auto(args.get(1).map(Path::new)),

        "key" => command_key(),

        "gnupg" => command_gnupg(args.get(1).map(Path::new)),

        "deps" => {
            let install = args.iter().any(|a| a == "--install");
            let yes = args.iter().any(|a| a == "--yes");
            command_deps(install || yes, yes)
        }

        "build" => {
            let root = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            command_build(&root, None)
        }

        "reproduce" => {
            let root = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let mut sums = None;
            let mut sig = None;
            let mut index = 1;
            while index < args.len() {
                let result = match args[index].as_str() {
                    "--sums" => take_value(&args, index, "--sums").map(|v| sums = Some(v)),
                    "--sig" => take_value(&args, index, "--sig").map(|v| sig = Some(v)),
                    other if other.starts_with("--") => Err(format!("unknown option: {other}")),
                    _ => Ok(()),
                };
                if let Err(why) = result {
                    return usage(&why, &["veilvoice-verify --help"]);
                }
                index += if args[index].starts_with("--") { 2 } else { 1 };
            }
            match (sums, sig) {
                (Some(sums), Some(sig)) => {
                    command_reproduce(&root, Path::new(&sums), Path::new(&sig), None)
                }
                _ => usage(
                    "`reproduce` needs the published hash list and its signature",
                    &[
                        "veilvoice-verify reproduce . --sums SHA256SUMS --sig SHA256SUMS.asc",
                        "",
                        "Both are on the release page. Without the signature there is",
                        "nothing to check the list against, and an unverified list is a",
                        "list of numbers somebody sent you.",
                    ],
                ),
            }
        }

        "install" => {
            let mut from = None;
            let mut index = 1;
            while index < args.len() {
                let result = match args[index].as_str() {
                    "--from" => take_value(&args, index, "--from").map(|v| from = Some(v)),
                    "--cli" | "--gui" => Ok(()),
                    other => Err(format!("unknown option: {other}")),
                };
                if let Err(why) = result {
                    return usage(&why, &["veilvoice-verify --help"]);
                }
                index += if args[index] == "--from" { 2 } else { 1 };
            }
            let cli = args.iter().any(|a| a == "--cli");
            let gui = args.iter().any(|a| a == "--gui");
            match from {
                Some(dir) => command_install(Path::new(&dir), cli, gui),
                None => usage(
                    "`install` needs a directory to install from",
                    &[
                        "veilvoice-verify install --from target/release --cli --gui",
                        "",
                        "Check that directory first, with `file` or `reproduce`.",
                        "This command copies; it does not verify.",
                    ],
                ),
            }
        }

        "release" => match args.get(1) {
            Some(tag) => command_release(tag, args.get(2).map(String::as_str)),
            None => usage(
                "`release` needs a tag",
                &["veilvoice-verify release v0.1.11"],
            ),
        },

        "hash" => match args.get(1) {
            Some(path) => command_hash(Path::new(path)),
            None => usage("`hash` needs a file", &["veilvoice-verify hash <FILE>"]),
        },

        "sums" => {
            if args.len() < 3 {
                return usage(
                    "`sums` needs a hash list and a signature",
                    &["veilvoice-verify sums <SHA256SUMS> <SHA256SUMS.asc>"],
                );
            }
            command_sums(Path::new(&args[1]), Path::new(&args[2]))
        }

        "file" => {
            let file = match args.get(1).map(PathBuf::from) {
                Some(path) => path,
                None => {
                    return usage(
                        "`file` needs a file to check",
                        &["veilvoice-verify file <FILE> --sums <SHA256SUMS> --sig <SIG>"],
                    )
                }
            };

            let mut sums = None;
            let mut sig = None;
            let mut sha256 = None;
            let mut index = 2;
            while index < args.len() {
                let result = match args[index].as_str() {
                    "--sums" => take_value(&args, index, "--sums").map(|v| sums = Some(v)),
                    "--sig" => take_value(&args, index, "--sig").map(|v| sig = Some(v)),
                    "--sha256" => take_value(&args, index, "--sha256").map(|v| sha256 = Some(v)),
                    other => Err(format!("unknown option: {other}")),
                };
                if let Err(why) = result {
                    return usage(&why, &["veilvoice-verify --help"]);
                }
                index += 2;
            }

            match (sha256, sums, sig) {
                (Some(_), Some(_), _) | (Some(_), _, Some(_)) => usage(
                    "--sha256 and --sums are two different checks",
                    &[
                        "They prove different things, so this tool will not run both",
                        "at once and report one answer. Pick one:",
                        "",
                        "  --sums/--sig  proves the download is INTACT",
                        "  --sha256      can prove it is REPRODUCIBLE",
                        "",
                        "veilvoice-verify --explain",
                    ],
                ),
                (Some(hash), None, None) => command_file_against_hash(&file, &hash),
                (None, Some(sums), Some(sig)) => {
                    command_file_against_sums(&file, Path::new(&sums), Path::new(&sig))
                }
                (None, Some(_), None) => usage(
                    "--sums without --sig",
                    &[
                        "An unsigned hash list proves nothing: whoever could replace the",
                        "download could replace the list beside it. Pass the signature",
                        "as well, or use --sha256 with a hash you obtained some other way.",
                    ],
                ),
                (None, None, Some(_)) => usage("--sig without --sums", &["Pass both."]),
                (None, None, None) => usage(
                    "nothing to check the file against",
                    &[
                        "This binary carries the signing key, but it cannot carry the",
                        "expected hash of your file -- a file cannot contain its own",
                        "digest. Give it one:",
                        "",
                        "  --sums SHA256SUMS --sig SHA256SUMS.asc",
                        "  --sha256 <64 hex characters>",
                        "",
                        "veilvoice-verify --explain",
                    ],
                ),
            }
        }

        other => usage(
            &format!("unknown command: {other}"),
            &["veilvoice-verify --help"],
        ),
    }
}

#[cfg(test)]
mod tests;
