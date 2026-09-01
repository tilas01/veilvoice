// SPDX-License-Identifier: GPL-3.0-or-later
//! **Marker 97.** The release job's contents list, read back by the parser
//! that will read it for real.
//!
//! # Why this test exists
//!
//! `CONTENTS.sha256` is the newest link in the chain a verifier follows:
//!
//! ```text
//! SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk
//! ```
//!
//! Every other link has tests on both sides of it. This one had a writer that
//! ran once a release, in a job nobody can run on a laptop, and a reader with
//! unit tests over hand-written samples. Two halves that were never introduced
//! to each other, and the failure mode is the worst shape a verifier has: a
//! manifest the reader parses happily and whose paths do not line up with what
//! is actually on disk, so every file reads as `MISSING` and a genuine release
//! is refused. Or worse, paths that line up by accident on one platform.
//!
//! So this builds a release the way the release job does, runs the real
//! generator over it, and checks the real reader against the real extracted
//! files. Nothing here is a stand-in.
//!
//! # Why it is allowed to skip
//!
//! It needs Python, and the `test` job does not install one. Every runner this
//! project uses has one anyway, so the test runs on all three in practice; on a
//! machine without one it returns rather than failing, because "Python is not
//! installed here" is a fact about the machine and not a defect in the release
//! job. The generator is also run by the release workflow itself, which is
//! where its absence would actually matter and where it cannot be absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use veilvoice_check::contents;

/// The repository root, from this test's own location.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the repository root")
}

/// A Python to run, if this machine has one.
fn python() -> Option<&'static str> {
    ["python3", "python"]
        .into_iter()
        .find(|name| Command::new(name).arg("--version").output().is_ok())
}

/// Somewhere to build a release, removed by the caller.
fn room(what: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("veilvoice-{what}-{stamp:x}"));
    std::fs::create_dir_all(&path).expect("a directory to work in");
    path
}

/// Build one release directory, the shape the release job stages.
///
/// A program, a second program, a README and a document in a subdirectory:
/// enough that a generator which forgot to recurse, or which wrote paths
/// relative to the wrong place, produces something this notices.
fn stage(root: &Path, name: &str) -> PathBuf {
    let release = root.join(name);
    std::fs::create_dir_all(release.join("docs")).unwrap();
    std::fs::write(release.join("veilvoice"), b"the command line").unwrap();
    std::fs::write(release.join("veilvoice-verify"), b"the verifier").unwrap();
    std::fs::write(release.join("README.md"), b"# VeilVoice\n").unwrap();
    std::fs::write(release.join("docs/INSTALL.md"), b"install this\n").unwrap();
    release
}

/// Whether a program is on this machine at all.
fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--help")
        .output()
        .map(|out| out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty())
        .unwrap_or(false)
}

/// The whole seam: stage, archive, generate, parse, extract, check.
///
/// The one test in this file, deliberately. Splitting it would mean staging a
/// release three times to assert three things about the same run, and the
/// property being tested is that the whole sequence agrees with itself.
#[test]
fn what_the_release_job_writes_is_what_the_verifier_reads() {
    let Some(python) = python() else {
        return;
    };
    if !have("tar") {
        return;
    }

    let repo = repository();
    let work = room("release-manifest");
    let dist = work.join("dist");
    let staging = work.join("staging");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::create_dir_all(&staging).unwrap();

    let name = "veilvoice-v0.1.15-linux-x86_64";
    stage(&dist, name);

    // Archived exactly as the release job does it: from the parent, naming the
    // release directory, so the archive carries that directory at its root.
    let tarball = staging.join(format!("{name}.tar.gz"));
    let made = Command::new("tar")
        .arg("-C")
        .arg(&dist)
        .arg("-czf")
        .arg(&tarball)
        .arg(name)
        .status()
        .expect("tar runs");
    assert!(made.success(), "the archive could not be built");

    // The generator the release job runs. Not a copy of it.
    let generator = repo.join("tools/release/contents.py");
    assert!(generator.is_file(), "{} is missing", generator.display());
    let list = staging.join(contents::CONTENTS);
    let ran = Command::new(python)
        .arg(&generator)
        .arg(&staging)
        .arg("-o")
        .arg(&list)
        .output()
        .expect("the generator runs");
    assert!(
        ran.status.success(),
        "the generator failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );

    // The reader the verifier uses. Not a stand-in for it either.
    let text = std::fs::read_to_string(&list).expect("the generator wrote something");
    let all = contents::parse(&text).unwrap_or_else(|why| {
        panic!("the verifier cannot read what the release job wrote: {why}\n{text}")
    });
    let section = contents::for_archive(&all, &format!("{name}.tar.gz")).unwrap_or_else(|| {
        panic!("the manifest does not mention the archive it was made from:\n{text}")
    });
    assert_eq!(
        section.members.len(),
        4,
        "every file, including the one in a subdirectory:\n{text}"
    );

    // Extracted the way somebody extracts a download, beside the archive, and
    // then checked file by file. This is the assertion the whole file is for:
    // the paths the generator wrote line up with the paths the reader looks
    // for, on this platform, without anybody having agreed on them by hand.
    let out = Command::new("tar")
        .arg("-C")
        .arg(&staging)
        .arg("-xzf")
        .arg(&tarball)
        .status()
        .expect("tar runs");
    assert!(out.success(), "the archive could not be extracted");

    let outcomes = contents::check(&staging, section);
    for outcome in &outcomes {
        assert!(
            outcome.is_good(),
            "{}: {:?}\n{text}",
            outcome.path,
            outcome.verdict
        );
    }

    let sweep = contents::extras(&staging, section);
    assert!(
        sweep.is_clean(),
        "an untouched extraction is not the release: {sweep:?}"
    );

    // And a changed file is caught, so the pass above is not the check being
    // asleep. Written after the clean run rather than instead of it: a test
    // that only proves failure cannot tell a working verifier from one that
    // refuses everything.
    std::fs::write(staging.join(name).join("veilvoice"), b"something else").unwrap();
    let after = contents::check(&staging, section);
    let changed: Vec<&contents::Outcome> = after.iter().filter(|o| !o.is_good()).collect();
    assert_eq!(changed.len(), 1, "exactly the file that was changed");
    assert!(changed[0].path.ends_with("veilvoice"), "{:?}", changed[0]);

    std::fs::remove_dir_all(&work).ok();
}
