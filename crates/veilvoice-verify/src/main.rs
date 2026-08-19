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

mod fetch;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
use pgp::types::KeyDetails as _;
use sha2::{Digest, Sha256};

/// The signing key, compiled in.
///
/// Read from the copy the website serves, so there is exactly one key file in
/// this repository and no chance of a second one drifting from it. A test
/// asserts that this key's fingerprint is [`FINGERPRINT`]; if somebody swaps
/// the file, the build fails rather than the verifier trusting a new key.
const PUBLIC_KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../website/assets/veilvoice-signing-key.asc"
));

/// The fingerprint, written out rather than derived.
///
/// Deriving it from `PUBLIC_KEY` would make this constant agree with the key
/// automatically, which sounds like an improvement and is the opposite of one:
/// the whole point is that a reader can compare this string against the one
/// published in `README.md`, on the website and in the release notes. A value
/// computed from the very file it is meant to authenticate checks nothing.
const FINGERPRINT: &str = "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A";

const USAGE: &str = "\
veilvoice-verify -- check a VeilVoice release without GnuPG installed

USAGE
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

/// Parse the embedded key and confirm its fingerprint is the expected one.
///
/// Done at run time rather than trusted, because "the binary contains a key"
/// and "the binary contains *the* key" are different statements, and only the
/// second is worth anything.
fn embedded_key() -> Result<SignedPublicKey, String> {
    let (key, _) = SignedPublicKey::from_string(PUBLIC_KEY)
        .map_err(|e| format!("the embedded public key does not parse: {e}"))?;

    let actual = fingerprint_of(&key);
    if actual != FINGERPRINT {
        return Err(format!(
            "the embedded key's fingerprint is {actual}, not {FINGERPRINT}"
        ));
    }
    Ok(key)
}

fn fingerprint_of(key: &SignedPublicKey) -> String {
    let mut out = String::new();
    for byte in key.fingerprint().as_bytes() {
        let _ = write!(out, "{byte:02X}");
    }
    out
}

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// SHA-256 of a file, read in chunks.
///
/// Streamed rather than read whole: a release archive is tens of megabytes and
/// there is no reason for this to need that much memory at once. The web
/// verifier had the same problem in the other direction -- finding F-36.
fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;

    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let mut out = String::new();
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    Ok(out)
}

/// Compare two hex digests without caring about case or stray whitespace.
///
/// Not constant time, and deliberately so: both values are public, and there
/// is no secret here for a timing difference to leak. Saying that plainly is
/// better than a `subtle` dependency that implies there was a threat.
fn digests_match(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Find a file's line in a `sha256sum`-format list.
///
/// The format is `<hex>  <name>`, and `sha256sum` writes a `*` before the name
/// for a binary-mode hash. Only the file's base name is compared: the list is
/// written with plain names, and the file being checked is usually somewhere
/// else entirely.
fn digest_from_sums(sums: &str, wanted: &str) -> Option<String> {
    for line in sums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (digest, name) = match line.split_once(char::is_whitespace) {
            Some(parts) => parts,
            None => continue,
        };
        let name = name.trim().trim_start_matches('*');
        if name == wanted {
            return Some(digest.trim().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Signature checking
// ---------------------------------------------------------------------------

/// Verify a detached signature over `data` using the embedded key.
fn verify_detached(key: &SignedPublicKey, signature: &str, data: &[u8]) -> Result<(), String> {
    let (signature, _) = DetachedSignature::from_string(signature)
        .map_err(|e| format!("the signature file does not parse as OpenPGP: {e}"))?;

    // Try the primary key and every subkey. Release signatures are normally
    // made by a signing subkey rather than the primary -- checking only the
    // primary would reject every genuine signature this project has ever made.
    if signature.verify(key, data).is_ok() {
        return Ok(());
    }
    for subkey in &key.public_subkeys {
        if signature.verify(subkey, data).is_ok() {
            return Ok(());
        }
    }
    Err("the signature was not made by this key".to_string())
}

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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
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
