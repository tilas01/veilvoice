// SPDX-License-Identifier: GPL-3.0-or-later
//! Build VeilVoice here, and compare what came out against what was published.
//!
//! # The question this answers, and the one it does not
//!
//! `veilvoice-verify file` answers *is this download the one that was
//! published*. This answers the harder one: **is the published build the one
//! this source produces**. A signature says who made a file. Only a build says
//! what the file is made of.
//!
//! It cannot answer it for anybody else. A build here proves something about
//! this platform and this machine, and that is exactly how a reproducible-build
//! claim is normally checked -- three machines give you three platforms
//! verified. It is a real answer rather than a pretended one.
//!
//! # "Builds for every operating system" means "builds for the one it is on"
//!
//! A build needs that platform's headers and linker. `veilvoice-cli` cannot be
//! compiled for Linux from Windows because `alsa-sys` needs ALSA's headers, and
//! a macOS build needs Apple's SDK, which Apple's licence does not allow to be
//! redistributed or run elsewhere. Every other crate cross-checks cleanly with
//! `--target`, and that is a *type check*, not a binary anybody should install.
//!
//! So this builds VeilVoice for the machine it is on, and compares that against
//! the published build **for that platform**.
//!
//! # A difference is a finding, not an accusation
//!
//! Reproducibility is a property of the release, not of the checker. If a build
//! here and the published build differ, that is something to look into and
//! publish -- and this prints both hashes and the names of the differing files
//! rather than a verdict, because "not reproducible" has several causes and
//! most of them are boring: a different compiler version, a path baked into a
//! panic message, a timestamp. It exits [`Status::NotReproducible`], which is
//! deliberately not the status that means tampering.
//!
//! # In plain words
//!
//! Anyone can sign a file. A signature tells you who put their name to
//! something; it does not tell you that the thing they signed was built from
//! the source code they published. This builds VeilVoice on your own computer,
//! from the source in front of you, and checks whether what comes out is
//! byte-for-byte the same as what was released.
//!
//! If it is, you know the released program is the source code -- not because
//! anybody said so, but because you produced the same thing yourself.
//!
//! If it is not, that is worth knowing and worth reporting, and it is usually
//! something dull rather than something sinister. So this shows you both
//! answers and which files differed, and leaves the conclusion to you.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::deps::{self, Presence, Route};
use crate::report::{self, Loudness, Status};

/// The profile a release is built with.
///
/// Written out rather than taken from a flag: the whole point is to run **the
/// same build the release does**, and a build with different settings answers
/// a different question while looking like it answered this one.
pub const RELEASE_ARGS: &[&str] = &["build", "--release", "--workspace", "--locked"];

/// Where a release build leaves its binaries, relative to the target directory.
pub const RELEASE_DIR: &str = "release";

/// The binaries a release publishes, without any platform extension.
pub const SHIPPED: &[&str] = &["veilvoice", "veilvoice-gui", "veilvoice-verify"];

/// What a build produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// Each binary that was found, and its SHA-256.
    pub files: Vec<(String, String)>,
    /// Binaries that were expected and are not there.
    ///
    /// Not an error on its own: `veilvoice-gui` is behind a feature on some
    /// platforms, and a release that does not ship it is not a broken build.
    pub absent: Vec<String>,
}

/// Whether the source tree this is pointed at is really one.
///
/// Checked before a compiler is started rather than after: a build in the wrong
/// directory takes minutes to fail and fails with a message about Cargo rather
/// than about the mistake.
pub fn looks_like_the_source(root: &Path) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "{} has no Cargo.toml, so it is not a source tree",
            root.display()
        ));
    }
    let text = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
    if !text.contains("veilvoice") {
        return Err(format!(
            "{} is a Cargo workspace, but not VeilVoice's",
            root.display()
        ));
    }
    if !root.join("rust-toolchain.toml").is_file() {
        return Err(format!(
            "{} has no rust-toolchain.toml. That file pins the compiler, and a \
             build with an unpinned compiler cannot be compared against a \
             published one",
            root.display()
        ));
    }
    Ok(())
}

/// The compiler version the source tree pins itself to.
///
/// Read rather than assumed. A reproducible build is reproducible *against a
/// stated compiler*, and reporting a comparison without saying which compiler
/// produced it leaves the most common cause of a difference unmentioned.
pub fn pinned_toolchain(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("channel") {
            let value = rest.trim_start_matches([' ', '=']).trim();
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// The platform triple this build is for, as `rustc` names it.
pub fn host_triple() -> Option<String> {
    let output = Command::new("rustc").arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("host: ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Where this workspace's build output actually goes.
///
/// F-69. This used to be `root/target`, which is right on a default machine and
/// wrong on a great many others. `CARGO_TARGET_DIR` in the environment,
/// `build.target-dir` in a `.cargo/config.toml`, and a shared target directory
/// across several checkouts all move it, and none of them are exotic -- the
/// machine this was written on has it set.
///
/// The symptom was the worst shape available: the build **succeeded**, took
/// several minutes, and then the hashing step reported that `.\target\release`
/// was not there. Minutes of correct work, discarded, with a message pointing
/// at the wrong place entirely.
///
/// So it asks cargo, which is the only thing that knows. `--no-deps` because
/// the dependency graph is not wanted and resolving it is slow.
pub fn target_directory(root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cargo would not start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo could not read the workspace:\n{}",
            stderr.trim_end()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    json_string_field(&text, "target_directory")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "cargo metadata did not name a target directory, which it always does".to_string()
        })
}

/// One top-level string out of cargo's JSON, without a JSON parser.
///
/// A dependency is not worth taking for one field, but the field is a Windows
/// path and Windows paths are full of backslashes, so the escapes have to be
/// undone properly rather than by taking the text between two quotes. Getting
/// that wrong gives a path with `\\` in it that looks almost right and does not
/// open.
///
/// Only the escapes cargo can actually emit here are handled; anything else is
/// left as it was written rather than guessed at.
fn json_string_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\":");
    let start = json.find(&key)? + key.len();
    let rest = json[start..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }

    let mut out = String::new();
    let mut escaped = false;
    for c in chars {
        if escaped {
            out.push(match c {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                // `\\`, `\"`, `\/` and anything else stand for themselves.
                other => other,
            });
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    // A string that never closed. Refused rather than returned truncated.
    None
}

/// Run the release build.
///
/// The compiler's own output goes to the terminal at `--verbose` and is
/// captured otherwise, so a failure can still be shown in full: a build that
/// stops with its reason discarded is a build nobody can act on.
pub fn build(root: &Path, target_dir: Option<&Path>) -> Result<PathBuf, String> {
    let mut command = Command::new("cargo");
    command.args(RELEASE_ARGS).current_dir(root);
    if let Some(dir) = target_dir {
        command.env("CARGO_TARGET_DIR", dir);
    }

    // Asked before the build rather than after, so a workspace that cannot be
    // read costs a second instead of the length of a compile.
    let where_it_lands = match target_dir {
        Some(dir) => dir.join(RELEASE_DIR),
        None => target_directory(root)?.join(RELEASE_DIR),
    };

    let line = format!(
        "cargo {} (in {}, output to {})",
        RELEASE_ARGS.join(" "),
        root.display(),
        where_it_lands.display()
    );

    if report::level() >= Loudness::Everything {
        println!("        {line}");
        let status = command
            .status()
            .map_err(|error| format!("cargo would not start: {error}"))?;
        if !status.success() {
            return Err("the build stopped; its output is above".to_string());
        }
    } else {
        let output = command
            .output()
            .map_err(|error| format!("cargo would not start: {error}"))?;
        if !output.status.success() {
            // Shown whatever the level, because a build failure with no reason
            // attached is not something anybody can do anything about. The
            // level decides how much is said about success, never about this.
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("the build stopped:\n{}", stderr.trim_end()));
        }
    }
    Ok(where_it_lands)
}

/// Hash every binary a release ships, from a directory a build left behind.
pub fn hash_what_was_built(dir: &Path) -> Result<Built, String> {
    if !dir.is_dir() {
        return Err(format!("{} is not there", dir.display()));
    }
    let mut files = Vec::new();
    let mut absent = Vec::new();
    for name in SHIPPED {
        let file = dir.join(with_platform_extension(name));
        if !file.is_file() {
            absent.push(with_platform_extension(name));
            continue;
        }
        let digest = veilvoice_check::sha256_file(&file)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        files.push((with_platform_extension(name), digest));
    }
    if files.is_empty() {
        return Err(format!(
            "nothing a release ships was found in {}. Looked for: {}",
            dir.display(),
            SHIPPED.join(", ")
        ));
    }
    Ok(Built { files, absent })
}

/// A binary's name on this platform.
pub fn with_platform_extension(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// How a built file compared against the published list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compared {
    /// The same bytes.
    Same {
        /// The file.
        name: String,
        /// The hash both agree on.
        digest: String,
    },
    /// Different bytes. **Both** hashes are carried, because a verdict without
    /// them cannot be checked by the person reading it.
    Different {
        /// The file.
        name: String,
        /// What was built here.
        built: String,
        /// What was published.
        published: String,
    },
    /// Built here and not in the published list at all.
    ///
    /// Not a difference: a release may ship a subset, and calling this a
    /// mismatch would report a naming difference as a reproducibility failure.
    NotPublished {
        /// The file.
        name: String,
    },
}

/// Compare a build against a hash list.
///
/// **The caller must have verified the signature over `sums` first.** This
/// function takes text, not a path, precisely so it cannot be handed an
/// unverified file by accident: there is nowhere in it that reads from disk.
pub fn compare(built: &Built, sums: &str) -> Vec<Compared> {
    let mut out = Vec::new();
    for (name, digest) in &built.files {
        match veilvoice_check::digest_from_sums(sums, name) {
            Some(published) if veilvoice_check::digests_match(&published, digest) => {
                out.push(Compared::Same {
                    name: name.clone(),
                    digest: digest.clone(),
                })
            }
            Some(published) => out.push(Compared::Different {
                name: name.clone(),
                built: digest.clone(),
                published,
            }),
            None => out.push(Compared::NotPublished { name: name.clone() }),
        }
    }
    out
}

/// Whether every file that could be compared matched.
///
/// A build with nothing to compare against is **not** reproducible-and-fine.
/// It is unanswered, and saying so is the difference between a check and a
/// formality.
pub fn all_matched(comparison: &[Compared]) -> bool {
    let compared = comparison
        .iter()
        .filter(|one| !matches!(one, Compared::NotPublished { .. }))
        .count();
    compared > 0
        && comparison
            .iter()
            .all(|one| !matches!(one, Compared::Different { .. }))
}

/// Report what a dependency check found. Returns whether a build can go ahead.
pub fn report_dependencies() -> (bool, Vec<&'static deps::Need>) {
    for need in deps::for_this_platform() {
        let presence = need.detect();
        let line = format!("{:<28} {}", need.name, presence.describe());
        // A probe that could not run reads as satisfied here rather than as
        // missing: it is not evidence of absence, and offering to install over
        // the top of something already there is worse than saying so and
        // letting the build be the judge.
        if presence.is_satisfied() || matches!(presence, Presence::Unknown(_)) {
            crate::good(&line);
        } else if report::level() >= Loudness::Minimal {
            println!("  --    {line}");
        }
    }
    let (required, optional) = deps::missing();
    for need in required.iter().chain(optional.iter()) {
        if report::level() >= Loudness::Normal {
            println!();
            println!("  {} is missing.", need.name);
            println!("    what it is: {}", need.what);
            println!("    why:        {}", need.why);
            match need.route() {
                Route::Run { vendor, .. } => {
                    println!("    packaged by {vendor}");
                    if let Some(line) = need.route().command_line() {
                        println!("    would run:  {line}");
                    }
                }
                Route::Yourself(words) => println!("    {words}"),
                Route::NotOnThisPlatform => {}
                Route::Unknown(why) => println!("    {why}"),
            }
        }
    }
    let mut all: Vec<&'static deps::Need> = required.clone();
    all.extend(optional);
    (required.is_empty(), all)
}

/// Run one install command, having been told yes.
///
/// Separated from everything that decides *whether* to, so the decision and the
/// act are never in the same place. Nothing in [`deps`] can reach this.
pub fn install(need: &deps::Need) -> Result<(), String> {
    let route = need.route();
    let Route::Run { program, args, .. } = &route else {
        return Err(format!(
            "{} has no command this program can run; it has to be done by hand",
            need.name
        ));
    };
    let line = route.command_line().unwrap_or_default();
    if report::level() >= Loudness::Minimal {
        println!("  running: {line}");
    }
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("{program} would not start: {error}"))?;
    if !status.success() {
        return Err(format!("{line} exited with {status}"));
    }
    Ok(())
}

/// Ask, and take only an unambiguous yes.
///
/// Anything that is not `y` or `yes` is a no, including an empty line and a
/// closed input. A prompt whose default is yes is not a prompt.
pub fn agreed(question: &str) -> bool {
    use std::io::Write;
    // At `--quiet` there is nobody to ask: nothing was printed, so nothing
    // explained what the question is about. Silence is a no.
    if report::level() < Loudness::Minimal {
        return false;
    }
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// The status a comparison should exit with.
pub fn status_for(comparison: &[Compared]) -> Status {
    if comparison
        .iter()
        .any(|one| matches!(one, Compared::Different { .. }))
    {
        Status::NotReproducible
    } else if all_matched(comparison) {
        Status::Success
    } else {
        // Nothing could be compared. Not a pass.
        Status::Incomplete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(files: &[(&str, &str)]) -> Built {
        Built {
            files: files
                .iter()
                .map(|(n, d)| (n.to_string(), d.to_string()))
                .collect(),
            absent: Vec::new(),
        }
    }

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn a_build_that_matches_is_reported_as_matching() {
        let sums = format!("{A}  veilvoice\n");
        let comparison = compare(&built(&[("veilvoice", A)]), &sums);
        assert_eq!(
            comparison,
            vec![Compared::Same {
                name: "veilvoice".into(),
                digest: A.into()
            }]
        );
        assert!(all_matched(&comparison));
        assert_eq!(status_for(&comparison), Status::Success);
    }

    /// Both hashes are carried, because a reader cannot check a verdict.
    #[test]
    fn a_difference_carries_both_hashes_and_is_not_called_tampering() {
        let sums = format!("{B}  veilvoice\n");
        let comparison = compare(&built(&[("veilvoice", A)]), &sums);
        assert_eq!(
            comparison,
            vec![Compared::Different {
                name: "veilvoice".into(),
                built: A.into(),
                published: B.into()
            }]
        );
        assert!(!all_matched(&comparison));

        let status = status_for(&comparison);
        assert_eq!(status, Status::NotReproducible);
        assert_ne!(status, Status::Refused, "a difference is a finding");
        assert!(!status.meaning().contains("do not run"));
    }

    /// A build with nothing to compare against has not passed. This is the
    /// failure mode the whole exercise is vulnerable to: a hash list naming
    /// nothing that was built would otherwise report success by vacuum.
    #[test]
    fn a_comparison_against_nothing_is_not_a_pass() {
        let comparison = compare(&built(&[("veilvoice", A)]), "");
        assert_eq!(
            comparison,
            vec![Compared::NotPublished {
                name: "veilvoice".into()
            }]
        );
        assert!(!all_matched(&comparison), "nothing was compared");
        assert_eq!(status_for(&comparison), Status::Incomplete);

        // And an entirely empty build likewise.
        assert!(!all_matched(&[]));
        assert_eq!(status_for(&[]), Status::Incomplete);
    }

    /// One file matching does not excuse another differing.
    #[test]
    fn one_good_file_does_not_carry_a_bad_one() {
        let sums = format!("{A}  veilvoice\n{B}  veilvoice-gui\n");
        let comparison = compare(&built(&[("veilvoice", A), ("veilvoice-gui", A)]), &sums);
        assert!(!all_matched(&comparison));
        assert_eq!(status_for(&comparison), Status::NotReproducible);
    }

    /// A file built here and absent from the list is not counted as a match --
    /// but it does not fail the run on its own either, because a release may
    /// legitimately ship a subset.
    #[test]
    fn a_file_the_release_does_not_ship_is_neither_a_match_nor_a_failure() {
        let sums = format!("{A}  veilvoice\n");
        let comparison = compare(&built(&[("veilvoice", A), ("veilvoice-gui", B)]), &sums);
        assert!(all_matched(&comparison), "the one comparable file matched");
        assert_eq!(status_for(&comparison), Status::Success);
        assert!(comparison.contains(&Compared::NotPublished {
            name: "veilvoice-gui".into()
        }));
    }

    /// `compare` takes text, never a path. That is what stops an unverified
    /// hash list being read by this half of the program by accident -- the
    /// signature check happens before, in the caller, and there is nothing
    /// here that could skip it.
    #[test]
    fn nothing_in_the_comparison_reads_a_file() {
        let source = include_str!("builder.rs");
        let start = source.find("pub fn compare(").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        for forbidden in ["read_to_string", "File::open", "std::fs::"] {
            assert!(!body.contains(forbidden), "{forbidden} in compare()");
        }
    }

    #[test]
    fn the_release_build_is_the_one_the_release_uses() {
        assert!(RELEASE_ARGS.contains(&"--release"));
        assert!(RELEASE_ARGS.contains(&"--workspace"));
        // `--locked` is not a nicety here. A build that is allowed to update
        // Cargo.lock is a build of different source than the one published.
        assert!(
            RELEASE_ARGS.contains(&"--locked"),
            "an unlocked build compares different source"
        );
    }

    #[test]
    fn the_pinned_compiler_is_read_out_of_the_tree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        assert!(looks_like_the_source(root).is_ok(), "{}", root.display());
        let pinned = pinned_toolchain(root).expect("a pinned channel");
        assert!(
            pinned.chars().next().is_some_and(|c| c.is_ascii_digit()),
            "{pinned}"
        );
    }

    /// A directory that is not the source tree is refused before a compiler is
    /// started, with the reason.
    #[test]
    fn a_wrong_directory_is_refused_before_anything_is_compiled() {
        let dir = tempfile::tempdir().unwrap();
        let why = looks_like_the_source(dir.path()).unwrap_err();
        assert!(why.contains("no Cargo.toml"), "{why}");

        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let why = looks_like_the_source(dir.path()).unwrap_err();
        assert!(why.contains("not VeilVoice's"), "{why}");

        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/veilvoice-core\"]\n",
        )
        .unwrap();
        let why = looks_like_the_source(dir.path()).unwrap_err();
        assert!(why.contains("pins the compiler"), "{why}");
    }

    /// Nothing is built until a directory has been checked, and nothing is
    /// installed until somebody has said yes. Both decisions live outside the
    /// functions that act, and this is the test that keeps them there.
    #[test]
    fn deciding_and_acting_are_in_different_places() {
        let source = include_str!("builder.rs");
        let start = source.find("pub fn install(").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        assert!(
            !body.contains("agreed("),
            "install() must not ask; something else asks and then calls it"
        );

        let start = source.find("pub fn agreed(").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        assert!(
            !body.contains("Command::new"),
            "agreed() must not run anything"
        );
    }

    /// Silence is a no. At `--quiet` nothing explained the question, so there
    /// is nobody who could have agreed to it.
    #[test]
    fn a_question_nobody_was_shown_is_answered_no() {
        report::set_level(Loudness::Nothing);
        assert!(!agreed("install ALSA headers?"));
        report::set_level(Loudness::Normal);
    }

    /// F-69. Where the build output goes is asked, never assumed.
    ///
    /// The check that matters is the one against the environment: this test
    /// suite runs with `CARGO_TARGET_DIR` set, so a function that returned
    /// `root/target` would disagree with reality right here.
    #[test]
    fn the_target_directory_comes_from_cargo_rather_than_from_a_guess() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let asked = target_directory(root).expect("cargo knows where it builds");

        // `OUT_DIR` is inside the real target directory, whatever it is, so it
        // is an answer that does not depend on this machine's configuration.
        let truth = Path::new(env!("OUT_DIR"));
        let asked_text = asked.to_string_lossy().replace('\\', "/");
        let truth_text = truth.to_string_lossy().replace('\\', "/");
        assert!(
            truth_text.starts_with(&asked_text),
            "cargo builds into {truth_text}, and this said {asked_text}"
        );
    }

    /// A directory that is not a workspace fails with cargo's own words, before
    /// anything is compiled.
    #[test]
    fn a_directory_cargo_cannot_read_says_so_rather_than_guessing() {
        let dir = tempfile::tempdir().unwrap();
        let why = target_directory(dir.path()).unwrap_err();
        assert!(!why.is_empty());
        assert!(why.contains("workspace") || why.contains("cargo"), "{why}");
    }

    /// The escapes in a Windows path have to be undone, or the answer is a
    /// path with `\\` in it that looks almost right and does not open.
    #[test]
    fn a_json_string_is_unescaped_rather_than_taken_between_quotes() {
        let json = r#"{"a":1,"target_directory":"C:\\Users\\a b\\target","b":2}"#;
        assert_eq!(
            json_string_field(json, "target_directory").as_deref(),
            Some(r"C:\Users\a b\target")
        );

        // A quote inside the value must not end it early.
        let json = r#"{"target_directory":"od\"d/target"}"#;
        assert_eq!(
            json_string_field(json, "target_directory").as_deref(),
            Some("od\"d/target")
        );

        // Absent, and malformed, are both `None` rather than something wrong.
        assert_eq!(json_string_field(r#"{"a":1}"#, "target_directory"), None);
        assert_eq!(
            json_string_field(r#"{"target_directory":"unclosed"#, "target_directory"),
            None
        );
        assert_eq!(
            json_string_field(r#"{"target_directory":42}"#, "target_directory"),
            None
        );
    }

    #[test]
    fn a_binary_gets_this_platforms_extension() {
        let name = with_platform_extension("veilvoice");
        if cfg!(windows) {
            assert_eq!(name, "veilvoice.exe");
        } else {
            assert_eq!(name, "veilvoice");
        }
    }

    #[test]
    fn hashing_an_empty_directory_says_so_rather_than_reporting_success() {
        let dir = tempfile::tempdir().unwrap();
        let why = hash_what_was_built(dir.path()).unwrap_err();
        assert!(why.contains("nothing a release ships"), "{why}");
        for name in SHIPPED {
            assert!(why.contains(name), "{why}");
        }
    }
}
