// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 164.** Reading a release archive from the inside, checked
//! against the list the release job actually writes.
//!
//! # Why this test exists
//!
//! `check::archive` hashes every member of a `.tar.gz` or a `.zip` and compares
//! the result against `CONTENTS.sha256`. Both ends of that comparison are
//! produced by different code on different machines: the list by
//! `tools/release/contents.py`, running on the publishing runner under Python's
//! `tarfile` and `zipfile`; the hashes by two readers written here, running on
//! whatever the reader downloaded to.
//!
//! The failure that matters is not a crash. It is the two ends disagreeing
//! quietly about a member's *name* -- a leading `./`, a backslash, a path
//! rebuilt from `ustar`'s `prefix` field -- so that every file reads as
//! `MISSING` and a perfectly sound release is refused. Or the same disagreement
//! in the other direction, where a name lines up by accident and a file nobody
//! published is passed over.
//!
//! So nothing here is a stand-in. The archives are built by `tar` and by
//! Python's `zipfile`, which is what built every archive this project has ever
//! published; the list is written by the real generator; and the readers are
//! the ones a download is checked with.
//!
//! # The names are the interesting part
//!
//! Each release staged here carries a path long enough to force `tar` out of
//! the plain header and into a `L` record or a `pax` header, a file in a
//! subdirectory, and a dotfile. Those are the three shapes that have actually
//! been got wrong, here and in the generator (see F-102).
//!
//! # Why it is allowed to skip
//!
//! It needs Python and `tar`, for the reason `release_manifest.rs` gives at
//! length: their absence is a fact about the machine rather than a defect in
//! the release job, and the release workflow itself cannot be without them.

use std::path::{Path, PathBuf};
use std::process::Command;

use veilvoice_verify::check::{archive, contents};

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

/// Whether a program is on this machine at all.
fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--help")
        .output()
        .map(|out| out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty())
        .unwrap_or(false)
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

/// A path long enough that `tar` cannot fit it in a header's 100-byte name.
///
/// Not invented for the test. The generated per-file reference pages a release
/// ships are named after the source file they document, and
/// `docs/files/crates-veilvoice-core-src-chain-rs.md` under a release directory
/// is already most of the way here, so this is the shape that will meet a
/// reader rather than a contrived one.
const LONG: &str =
    "docs/files/crates-veilvoice-core-src-chain-rs-and-then-some-more-of-it-to-be-sure.md";

/// Build one release directory, the shape the release job stages.
fn stage(root: &Path, name: &str) -> PathBuf {
    let release = root.join(name);
    std::fs::create_dir_all(release.join("docs/files")).unwrap();
    std::fs::write(release.join("veilvoice"), b"the command line").unwrap();
    std::fs::write(release.join("veilvoice-gui"), b"the window").unwrap();
    std::fs::write(release.join("README.md"), b"# VeilVoice\n").unwrap();
    // F-102's dotfile, which a generator that normalised with `lstrip` renamed.
    std::fs::write(release.join(".hidden"), b"a dotfile\n").unwrap();
    std::fs::write(release.join(LONG), b"a generated reference page\n").unwrap();
    release
}

/// Run the real generator over a staging directory.
fn generate(python: &str, staging: &Path) -> Vec<contents::ArchiveContents> {
    let generator = repository().join("tools/release/contents.py");
    assert!(generator.is_file(), "{} is missing", generator.display());
    let list = staging.join(contents::CONTENTS);
    let ran = Command::new(python)
        .arg(&generator)
        .arg(staging)
        .arg("-o")
        .arg(&list)
        .output()
        .expect("the generator runs");
    assert!(
        ran.status.success(),
        "the generator failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let text = std::fs::read_to_string(&list).expect("the generator wrote something");
    contents::parse(&text)
        .unwrap_or_else(|why| panic!("the verifier cannot read what the release job wrote: {why}"))
}

/// Assert that an archive holds exactly what the generated list says it does.
///
/// The assertion the whole file is for, in both directions: every published
/// file is found with its published hash, and the archive holds nothing that
/// was not published.
fn agrees(archive_path: &Path, all: &[contents::ArchiveContents]) {
    let name = archive_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let section = contents::for_archive(all, &name)
        .unwrap_or_else(|| panic!("the manifest does not mention {name}"));
    let inside = archive::members(archive_path)
        .unwrap_or_else(|why| panic!("{name} could not be read from the inside: {why}"));

    let comparison = archive::compare(&inside, section);
    assert!(
        comparison.is_clean(),
        "{name}: the reader and the generator disagree.\n  outcomes: {:?}\n  extras: {:?}",
        comparison
            .outcomes
            .iter()
            .filter(|o| !o.is_good())
            .collect::<Vec<_>>(),
        comparison.extras,
    );
    assert_eq!(
        comparison.as_published(),
        5,
        "every file, the dotfile and the long path among them: {:?}",
        inside.iter().map(|m| &m.path).collect::<Vec<_>>()
    );
    assert!(
        inside.iter().any(|m| m.path.ends_with(LONG)),
        "the long path survived: {:?}",
        inside.iter().map(|m| &m.path).collect::<Vec<_>>()
    );
    assert!(
        inside.iter().any(|m| m.path.ends_with("/.hidden")),
        "F-102: the dotfile kept its name: {:?}",
        inside.iter().map(|m| &m.path).collect::<Vec<_>>()
    );
}

/// A gzipped tar, in each of the three ways `tar` spells a long name.
///
/// `gnu` writes an `L` record, `pax` an extended header, and `ustar` splits the
/// path across `prefix` and `name`. This project's releases are built on five
/// machines with whichever `tar` each of them ships, so a reader that handles
/// only one of the three would work on some platforms and refuse the release on
/// others -- the worst shape a verifier defect can take, because it looks like
/// a compromised download.
#[test]
fn a_tarball_holds_what_the_release_job_published() {
    let Some(python) = python() else { return };
    if !have("tar") {
        return;
    }
    for format in ["gnu", "pax", "ustar"] {
        let work = room(&format!("inside-tar-{format}"));
        let dist = work.join("dist");
        let staging = work.join("staging");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::create_dir_all(&staging).unwrap();

        let name = "veilvoice-v0.1.23-linux-x86_64";
        stage(&dist, name);
        let tarball = staging.join(format!("{name}.tar.gz"));
        let made = Command::new("tar")
            .arg(format!("--format={format}"))
            .arg("-C")
            .arg(&dist)
            .arg("-czf")
            .arg(&tarball)
            .arg(name)
            .status()
            .expect("tar runs");
        // `ustar` cannot hold every path, and refusing one it cannot hold is
        // correct behaviour rather than a failure of this test. The other two
        // formats exist precisely because of that limit.
        if !made.success() {
            assert_eq!(format, "ustar", "{format} should have archived this");
            std::fs::remove_dir_all(&work).ok();
            continue;
        }

        let all = generate(python, &staging);
        agrees(&tarball, &all);
        std::fs::remove_dir_all(&work).ok();
    }
}

/// A zip, deflated and stored, read from its central directory.
///
/// Both compression methods, because a release zip holds both: `Compress-Archive`
/// stores a member it cannot usefully compress, and a reader that handled only
/// deflate would report a changed file for one that is perfectly sound.
#[test]
fn a_zip_holds_what_the_release_job_published() {
    let Some(python) = python() else { return };
    for (method, label) in [("ZIP_DEFLATED", "deflated"), ("ZIP_STORED", "stored")] {
        let work = room(&format!("inside-zip-{label}"));
        let dist = work.join("dist");
        let staging = work.join("staging");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::create_dir_all(&staging).unwrap();

        let name = "veilvoice-v0.1.23-windows-x86_64";
        stage(&dist, name);
        let zip = staging.join(format!("{name}.zip"));
        // Built with the same library that built every release zip this
        // project has published, so the bytes under test are the bytes a
        // reader will meet.
        let script = format!(
            "import pathlib, zipfile\n\
             root = pathlib.Path({dist:?})\n\
             with zipfile.ZipFile({zip:?}, 'w', zipfile.{method}) as z:\n\
             \x20   for path in sorted(root.rglob('*')):\n\
             \x20       if path.is_file():\n\
             \x20           z.write(path, path.relative_to(root).as_posix())\n",
            dist = dist.to_string_lossy(),
            zip = zip.to_string_lossy(),
        );
        let ran = Command::new(python)
            .arg("-c")
            .arg(&script)
            .output()
            .expect("python runs");
        assert!(
            ran.status.success(),
            "the zip could not be built: {}",
            String::from_utf8_lossy(&ran.stderr)
        );

        let all = generate(python, &staging);
        agrees(&zip, &all);
        std::fs::remove_dir_all(&work).ok();
    }
}

/// A file changed inside the archive is named, and an added one is too.
///
/// Written after the clean runs above rather than instead of them: a test that
/// only proves refusal cannot tell a working verifier from one that refuses
/// everything.
#[test]
fn a_changed_or_added_file_inside_the_archive_is_caught() {
    let Some(python) = python() else { return };
    if !have("tar") {
        return;
    }
    let work = room("inside-tampered");
    let dist = work.join("dist");
    let staging = work.join("staging");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::create_dir_all(&staging).unwrap();

    let name = "veilvoice-v0.1.23-linux-x86_64";
    let release = stage(&dist, name);
    let tarball = staging.join(format!("{name}.tar.gz"));
    let archive_it = |to: &Path| {
        let made = Command::new("tar")
            .arg("-C")
            .arg(&dist)
            .arg("-czf")
            .arg(to)
            .arg(name)
            .status()
            .expect("tar runs");
        assert!(made.success());
    };
    archive_it(&tarball);

    // The list is written from the honest archive, and then the archive is
    // rebuilt with a changed program and an extra one in it. That is the
    // substitution this whole command exists to notice: the hash list and its
    // signature are untouched and genuine, and the archive is not.
    let all = generate(python, &staging);
    std::fs::write(release.join("veilvoice"), b"something else entirely").unwrap();
    std::fs::write(release.join("helpfully-added"), b"not published\n").unwrap();
    archive_it(&tarball);

    let section = contents::for_archive(&all, &format!("{name}.tar.gz")).unwrap();
    let inside = archive::members(&tarball).expect("the archive still reads");
    let comparison = archive::compare(&inside, section);

    assert!(!comparison.is_clean(), "a changed archive must not pass");
    let changed: Vec<&str> = comparison
        .outcomes
        .iter()
        .filter(|o| matches!(o.verdict, archive::Verdict::Differs { .. }))
        .map(|o| o.path.as_str())
        .collect();
    assert_eq!(
        changed,
        vec![format!("{name}/veilvoice")],
        "the changed program is named, and only it"
    );
    assert_eq!(
        comparison.extras,
        vec![format!("{name}/helpfully-added")],
        "the file nobody published is named"
    );
    assert_eq!(comparison.wrong(), 2, "one changed and one added");

    std::fs::remove_dir_all(&work).ok();
}

/// A published file missing from the archive is reported as missing.
#[test]
fn a_file_taken_out_of_the_archive_is_reported() {
    let Some(python) = python() else { return };
    if !have("tar") {
        return;
    }
    let work = room("inside-missing");
    let dist = work.join("dist");
    let staging = work.join("staging");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::create_dir_all(&staging).unwrap();

    let name = "veilvoice-v0.1.23-linux-x86_64";
    let release = stage(&dist, name);
    let tarball = staging.join(format!("{name}.tar.gz"));
    let archive_it = || {
        assert!(Command::new("tar")
            .arg("-C")
            .arg(&dist)
            .arg("-czf")
            .arg(&tarball)
            .arg(name)
            .status()
            .expect("tar runs")
            .success());
    };
    archive_it();
    let all = generate(python, &staging);

    std::fs::remove_file(release.join("veilvoice-gui")).unwrap();
    archive_it();

    let section = contents::for_archive(&all, &format!("{name}.tar.gz")).unwrap();
    let inside = archive::members(&tarball).expect("the archive still reads");
    let comparison = archive::compare(&inside, section);
    let missing: Vec<&str> = comparison
        .outcomes
        .iter()
        .filter(|o| o.verdict == archive::Verdict::Missing)
        .map(|o| o.path.as_str())
        .collect();
    assert_eq!(missing, vec![format!("{name}/veilvoice-gui")]);
    assert!(comparison.extras.is_empty(), "nothing was added");

    std::fs::remove_dir_all(&work).ok();
}

/// An archive this reader cannot open says so, rather than reporting nothing.
///
/// `.tar.xz` is published and is not readable here. The distinction that
/// matters is between "there is nothing wrong with this" and "I did not look",
/// and a verifier must never spell the second as the first.
#[test]
fn an_archive_this_cannot_read_says_so() {
    assert_eq!(
        archive::kind_of("veilvoice-v0.1.23-linux-x86_64.tar.gz"),
        archive::Kind::TarGz
    );
    assert_eq!(
        archive::kind_of("veilvoice-v0.1.23-windows-x86_64.zip"),
        archive::Kind::Zip
    );
    assert!(matches!(
        archive::kind_of("veilvoice-v0.1.23-linux-x86_64.tar.xz"),
        archive::Kind::Unreadable(_)
    ));
    assert_eq!(archive::kind_of("notes.txt"), archive::Kind::Unknown);

    let work = room("inside-unreadable");
    let path = work.join("veilvoice-v0.1.23-linux-x86_64.tar.xz");
    std::fs::write(&path, b"not really an xz").unwrap();
    let why = archive::members(&path).expect_err("an unreadable format is an error");
    assert!(
        why.to_string().contains(".tar.gz"),
        "it says which file to fetch instead: {why}"
    );
    std::fs::remove_dir_all(&work).ok();
}

/// A file that is not an archive at all is refused, not passed over.
#[test]
fn rubbish_in_place_of_an_archive_is_refused() {
    let work = room("inside-rubbish");
    for name in [
        "veilvoice-v0.1.23-linux-x86_64.tar.gz",
        "veilvoice-v0.1.23-windows-x86_64.zip",
    ] {
        let path = work.join(name);
        std::fs::write(&path, b"this is not an archive, it is a sentence").unwrap();
        assert!(
            archive::members(&path).is_err(),
            "{name} is not an archive and must not read as an empty one"
        );
    }
    std::fs::remove_dir_all(&work).ok();
}
