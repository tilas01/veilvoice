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

mod discover;
mod fetch;

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
veilvoice-verify -- check a VeilVoice release without GnuPG installed

USAGE
  veilvoice-verify
  veilvoice-verify auto [DIRECTORY]
      Find a downloaded release near you and check it, with nothing else to
      type. Looks in the directory given, then the current one, then beside
      this program, then your Downloads and Desktop. Entirely offline.

      This is also what double-clicking the program does.

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
    println!("  ok    {message}");
}

/// A failure that is not a refusal: something did not happen, rather than
/// something was checked and found wrong.
///
/// Kept apart from [`deny`] on purpose. "The download failed" and "the
/// signature is bad" are different facts and a reader must not have to work out
/// which one they were told -- the second means somebody may have tampered
/// with a release, and the first usually means a network hiccup.
fn fail(reason: &str) -> ExitCode {
    eprintln!();
    eprintln!("FAILED: could not complete the check.");
    for line in reason.lines() {
        eprintln!("  {line}");
    }
    eprintln!();
    eprintln!("This is not a verification failure -- nothing was checked and");
    eprintln!("found wrong. Nothing has been proven either. Try again, or");
    eprintln!("download the files yourself and pass them in.");
    ExitCode::FAILURE
}

/// Every refusal goes through here, so every refusal names the check.
fn deny(reason: &str, detail: &[&str]) -> ExitCode {
    eprintln!();
    eprintln!("REFUSED: {reason}");
    for line in detail {
        eprintln!("  {line}");
    }
    eprintln!();
    eprintln!("Nothing about this download has been proven. Do not run it.");
    ExitCode::FAILURE
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
            println!();
            println!("  fingerprint  {}", fingerprint_of(&key));
            for uid in key.details.users.iter() {
                println!("  user id      {}", String::from_utf8_lossy(uid.id.id()));
            }
            println!();
            println!("  Compare that fingerprint against the one published in README.md,");
            println!("  on https://tilas01.github.io/veilvoice/ and in the release notes.");
            println!("  If they disagree, stop.");
            println!();
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
        Err(e) => return deny("the hash list could not be read", &[&format!("{e}")]),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(e) => return deny("the signature could not be read", &[&e]),
    };

    match verify_detached(&key, &signature, &sums) {
        Ok(()) => {
            good("signature over the hash list is good");
            println!();
            println!("  That hash list is genuinely the one signed by {FINGERPRINT}.");
            println!("  It does not yet say anything about any particular file --");
            println!("  use `veilvoice-verify file` for that.");
            println!();
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
        Err(e) => return deny("the hash list could not be read", &[&format!("{e}")]),
    };
    let signature = match read_text(sig_path) {
        Ok(text) => text,
        Err(e) => return deny("the signature could not be read", &[&e]),
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
        Err(e) => return deny("the file could not be hashed", &[&e]),
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
    println!();
    println!("  INTACT. This file is byte-for-byte what was published, signed by");
    println!("  {FINGERPRINT}.");
    println!();
    println!("  That is not the same as knowing it was built from the published");
    println!("  source -- the same person produced the binary and the list. For");
    println!("  that, compare against a hash somebody else produced from their own");
    println!("  build:  veilvoice-verify --explain");
    println!();
    ExitCode::SUCCESS
}

fn command_file_against_hash(file: &Path, expected: &str) -> ExitCode {
    let cleaned = expected.trim();
    if cleaned.len() != 64 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return deny(
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
        Err(e) => return deny("the file could not be hashed", &[&e]),
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
    println!();
    println!("  This file matches the hash you gave.");
    println!();
    println!("  What that proves depends entirely on where the hash came from, and");
    println!("  only you know that:");
    println!();
    println!("    - from the published SHA256SUMS: the download is INTACT.");
    println!("    - from somebody else's independent build of the same tag: the");
    println!("      release is REPRODUCIBLE, which is the stronger claim.");
    println!();
    println!("  This tool cannot tell which, so it does not guess.");
    println!("  veilvoice-verify --explain");
    println!();
    ExitCode::SUCCESS
}

fn command_hash(file: &Path) -> ExitCode {
    match sha256_file(file) {
        Ok(digest) => {
            println!(
                "{digest}  {}",
                file.file_name().unwrap_or_default().to_string_lossy()
            );
            ExitCode::SUCCESS
        }
        Err(e) => deny("the file could not be hashed", &[&e]),
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
        return deny(
            "that does not look like a release tag",
            &["veilvoice-verify release v0.1.11"],
        );
    }
    if let Some(name) = asset {
        if !fetch::valid_asset(name) {
            return deny(
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

    println!();
    println!("  fetching into {}", directory.display());

    let mut fetched = Vec::new();
    for name in [fetch::SUMS, fetch::SIGNATURE] {
        let url = fetch::asset_url(tag, name);
        print!("  {name} ... ");
        match fetch::download(&url, &directory.join(name)) {
            Ok(path) => {
                println!("ok");
                fetched.push(path);
            }
            Err(error) => {
                println!("failed");
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
    print!("  {name} ... ");
    let archive = match fetch::download(&url, &directory.join(name)) {
        Ok(path) => {
            println!("ok");
            path
        }
        Err(error) => {
            println!("failed");
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
        return deny(
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
        return deny("a release was found but cannot be checked", &borrowed);
    };

    println!("Checking what is in {}", found.directory.display());
    println!();
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
        println!();
    }
    if failures > 0 {
        println!(
            "{failures} of {} did not check out. Nothing above should be run.",
            found.archives.len()
        );
    }
    worst
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
    println!();
    println!("Press Enter to close.");
    let mut discard = String::new();
    let _ = std::io::stdin().read_line(&mut discard);
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // No arguments is very probably a double-click. Do the useful thing --
    // look for a release nearby and check it -- and then wait, so the window
    // does not vanish before it has been read.
    if args.is_empty() {
        let outcome = command_auto(None);
        wait_before_the_window_closes();
        return outcome;
    }

    if args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args[0] == "--explain" {
        print!("{EXPLAIN}");
        return ExitCode::SUCCESS;
    }
    if args[0] == "--version" {
        println!("veilvoice-verify {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match args[0].as_str() {
        "auto" => command_auto(args.get(1).map(Path::new)),

        "key" => command_key(),

        "release" => match args.get(1) {
            Some(tag) => command_release(tag, args.get(2).map(String::as_str)),
            None => deny(
                "`release` needs a tag",
                &["veilvoice-verify release v0.1.11"],
            ),
        },

        "hash" => match args.get(1) {
            Some(path) => command_hash(Path::new(path)),
            None => deny("`hash` needs a file", &["veilvoice-verify hash <FILE>"]),
        },

        "sums" => {
            if args.len() < 3 {
                return deny(
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
                    return deny(
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
                    return deny(&why, &["veilvoice-verify --help"]);
                }
                index += 2;
            }

            match (sha256, sums, sig) {
                (Some(_), Some(_), _) | (Some(_), _, Some(_)) => deny(
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
                (None, Some(_), None) => deny(
                    "--sums without --sig",
                    &[
                        "An unsigned hash list proves nothing: whoever could replace the",
                        "download could replace the list beside it. Pass the signature",
                        "as well, or use --sha256 with a hash you obtained some other way.",
                    ],
                ),
                (None, None, Some(_)) => deny("--sig without --sums", &["Pass both."]),
                (None, None, None) => deny(
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

        other => deny(
            &format!("unknown command: {other}"),
            &["veilvoice-verify --help"],
        ),
    }
}

#[cfg(test)]
mod tests;
