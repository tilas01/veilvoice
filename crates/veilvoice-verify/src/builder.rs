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

/// Everything that would otherwise differ between two builds of one source.
///
/// F-70. The build ran `cargo build --release` and nothing else, which meant a
/// comparison against a published build was **guaranteed** to differ. Two
/// builds of this tree in two directories on this machine produced three
/// different binaries out of three -- measured, not supposed.
///
/// The cause is the dull one this module's own documentation already named: the
/// absolute path of the source tree is baked into panic messages and debug
/// info, so a build in `C:\src\veilvoice` and a build in `/home/a/veilvoice`
/// cannot be the same bytes. `docs/REPRODUCIBLE_BUILDS.md` has said so all
/// along, and the release workflow sets the flags that fix it. The checker did
/// not.
///
/// A reproducibility checker that always answers "not reproducible" is worse
/// than no checker. It teaches the one reader who took the trouble to build
/// from source that the release does not match -- and the next time it says so
/// for a real reason, they will have learned to ignore it.
///
/// So this reproduces the release environment rather than approximating it.
/// Every value here has a counterpart in `.github/workflows/release.yml`, and
/// [`describe`] prints them, because a comparison whose settings are invisible
/// cannot be checked by the person reading the result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    /// `RUSTFLAGS`, with the path remapping and the per-linker flags.
    ///
    /// Three remaps: the source tree, `CARGO_HOME`, and the **target
    /// directory**. The third is the one this checker needs and the release
    /// workflow does not, because the release builds into `target/` inside the
    /// source tree it is already remapping.
    pub rustflags: String,
    /// `SOURCE_DATE_EPOCH`, from the commit being built.
    ///
    /// `None` outside a git checkout, where there is no commit to take a date
    /// from. Said rather than substituted: a made-up timestamp would make the
    /// build differ from the published one for a new reason.
    pub source_date_epoch: Option<String>,
    /// The target triple, passed explicitly because the release passes it.
    pub triple: String,
    /// macOS only: stops `ar` writing timestamps into static archives.
    pub zero_ar_date: bool,
}

impl Environment {
    /// The settings, for printing before a build.
    pub fn describe(&self) -> Vec<String> {
        let mut out = vec![
            format!("target            {}", self.triple),
            format!("RUSTFLAGS         {}", self.rustflags),
        ];
        out.push(match &self.source_date_epoch {
            Some(epoch) => format!("SOURCE_DATE_EPOCH {epoch}"),
            None => "SOURCE_DATE_EPOCH not set -- this is not a git checkout".to_string(),
        });
        if self.zero_ar_date {
            out.push("ZERO_AR_DATE      1".to_string());
        }
        out
    }
}

/// Flags that make this platform's linker deterministic.
///
/// Each one is here because that linker writes something into the output that
/// is not a function of the input, and each is the same flag the release uses.
pub fn repro_link() -> &'static str {
    if cfg!(all(windows, target_env = "msvc")) {
        // MSVC stamps a timestamp and a PDB signature into the PE header;
        // /Brepro replaces both with a hash of the input.
        "-C link-arg=/Brepro"
    } else if cfg!(target_os = "macos") {
        // ld64 writes an LC_UUID that is not a pure function of the input, so
        // two identical builds differ by sixteen bytes.
        "-C link-arg=-Wl,-no_uuid"
    } else {
        ""
    }
}

/// Where cargo keeps downloaded crates, whose paths are also baked in.
fn cargo_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(home));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".cargo"))
}

/// The date of the commit being built, as seconds since the epoch.
fn commit_date(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%ct"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Refused rather than passed through: `SOURCE_DATE_EPOCH` is read by tools
    // that will do something unhelpful with a value that is not a number.
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(text)
}

/// A path as the compiler will see it, for the remapping to match.
///
/// Not [`std::fs::canonicalize`], which on Windows returns an extended-length
/// path beginning `\\?\`. Cargo does not hand rustc that form, so a remap built
/// from it matches nothing and silently does nothing at all -- the exact
/// failure the release workflow's own comment warns about on macOS, arriving
/// through the other platform's door.
fn as_the_compiler_sees_it(root: &Path) -> Result<PathBuf, String> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?
            .join(root)
    };

    // Resolve `.` and `..` without touching the filesystem, so `build .` and
    // `build` remap to the same prefix.
    let mut parts = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str()),
        }
    }
    Ok(parts)
}

/// The environment the release is built in, for this tree on this machine.
pub fn environment(root: &Path, target_dir: Option<&Path>) -> Result<Environment, String> {
    let source = as_the_compiler_sees_it(root)?;
    let mut rustflags = format!("--remap-path-prefix={}=/veilvoice", source.display());
    if let Some(cargo) = cargo_home() {
        rustflags.push_str(&format!(" --remap-path-prefix={}=/cargo", cargo.display()));
    }

    // The target directory too, and this one was learned the hard way.
    //
    // With only the two remaps above, two builds of this tree in two target
    // directories gave two identical binaries and one that differed:
    // `veilvoice-gui`. The reason is `OUT_DIR`, which lives under the target
    // directory and reaches a binary through a build script. The release
    // workflow never notices because it puts `target/` *inside* the source
    // tree it is already remapping, so `OUT_DIR` is covered for free -- and a
    // checker that compares two builds in two separate target directories does
    // not get that for free.
    //
    // Without this, the tool reports a difference caused entirely by where it
    // chose to put its own build output. That is worse than a false negative:
    // it is a false negative the tool manufactured itself.
    let target = match target_dir {
        Some(dir) => as_the_compiler_sees_it(dir)?,
        None => target_directory(root)?,
    };
    rustflags.push_str(&format!(
        " --remap-path-prefix={}=/target",
        target.display()
    ));
    let link = repro_link();
    if !link.is_empty() {
        rustflags.push(' ');
        rustflags.push_str(link);
    }

    let triple =
        host_triple().ok_or_else(|| "rustc would not say what platform this is".to_string())?;

    Ok(Environment {
        rustflags,
        source_date_epoch: commit_date(root),
        triple,
        zero_ar_date: cfg!(target_os = "macos"),
    })
}

/// Run the release build.
///
/// The compiler's own output goes to the terminal at `--verbose` and is
/// captured otherwise, so a failure can still be shown in full: a build that
/// stops with its reason discarded is a build nobody can act on.
pub fn build(
    root: &Path,
    target_dir: Option<&Path>,
    environment: &Environment,
) -> Result<PathBuf, String> {
    let mut command = Command::new("cargo");
    command
        .args(RELEASE_ARGS)
        .args(["--target", &environment.triple])
        .current_dir(root);
    if let Some(dir) = target_dir {
        command.env("CARGO_TARGET_DIR", dir);
    }

    // The release environment, not this shell's. `RUSTFLAGS` is set rather than
    // appended to on purpose: a value inherited from the terminal is a value
    // the published build did not have, and it would change the answer.
    command.env("RUSTFLAGS", &environment.rustflags);
    match &environment.source_date_epoch {
        Some(epoch) => {
            command.env("SOURCE_DATE_EPOCH", epoch);
        }
        None => {
            command.env_remove("SOURCE_DATE_EPOCH");
        }
    }
    if environment.zero_ar_date {
        command.env("ZERO_AR_DATE", "1");
    }

    // Asked before the build rather than after, so a workspace that cannot be
    // read costs a second instead of the length of a compile.
    //
    // Under the triple, because the release builds with an explicit `--target`
    // and that moves the output down one level.
    let base = match target_dir {
        Some(dir) => dir.to_path_buf(),
        None => target_directory(root)?,
    };
    let where_it_lands = base.join(&environment.triple).join(RELEASE_DIR);

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
        let root = repo_root();
        assert!(looks_like_the_source(&root).is_ok(), "{}", root.display());
        let pinned = pinned_toolchain(&root).expect("a pinned channel");
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
        let root = repo_root();
        let asked = target_directory(&root).expect("cargo knows where it builds");

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

    /// F-70. The build has to be run in the environment the release is built
    /// in, or the comparison is decided before it starts.
    ///
    /// Measured before it was written: two builds of this tree in two
    /// directories on this machine produced three differing binaries out of
    /// three. The cause is the dull one -- the absolute source path is baked
    /// into panic messages and debug info -- and it is exactly what the
    /// release workflow's `--remap-path-prefix` exists to remove.
    #[test]
    fn the_release_environment_is_reproduced_rather_than_approximated() {
        let root = repo_root();
        let environment = environment(&root, None).expect("this is a git checkout");

        // The source tree, so two checkouts in different places agree.
        assert!(
            environment.rustflags.contains("--remap-path-prefix="),
            "{}",
            environment.rustflags
        );
        assert!(
            environment.rustflags.contains("=/veilvoice"),
            "the source has to remap to the same name the release uses: {}",
            environment.rustflags
        );
        // And the crate cache, whose paths are baked in just as firmly.
        assert!(
            environment.rustflags.contains("=/cargo"),
            "CARGO_HOME is in the binary too: {}",
            environment.rustflags
        );
        // And the target directory, without which two builds in two target
        // directories differ for a reason the checker created itself.
        assert!(
            environment.rustflags.contains("=/target"),
            "OUT_DIR lives under the target directory: {}",
            environment.rustflags
        );

        // The per-linker flag, which is not optional on the two platforms that
        // need it: MSVC stamps a timestamp, ld64 writes a UUID.
        if cfg!(all(windows, target_env = "msvc")) {
            assert!(environment.rustflags.contains("/Brepro"));
        }
        if cfg!(target_os = "macos") {
            assert!(environment.rustflags.contains("-no_uuid"));
            assert!(environment.zero_ar_date);
        }

        // A commit date, because this tree is a checkout.
        let epoch = environment
            .source_date_epoch
            .as_deref()
            .expect("a git checkout has a commit");
        assert!(epoch.chars().all(|c| c.is_ascii_digit()), "{epoch}");

        assert!(!environment.triple.is_empty());
    }

    /// Every setting is printed. A comparison whose settings are invisible
    /// cannot be checked by the person reading the result, and "not
    /// reproducible" with no environment attached is unactionable.
    #[test]
    fn the_settings_are_shown_rather_than_applied_silently() {
        let environment = environment(&repo_root(), None).expect("a checkout");
        let shown = environment.describe().join("\n");
        assert!(shown.contains(&environment.triple), "{shown}");
        assert!(shown.contains("RUSTFLAGS"), "{shown}");
        assert!(shown.contains("SOURCE_DATE_EPOCH"), "{shown}");
        assert!(
            shown.contains(environment.source_date_epoch.as_deref().unwrap()),
            "{shown}"
        );
    }

    /// Outside a git checkout there is no commit to date the build from. Said,
    /// not substituted: an invented timestamp would make the build differ from
    /// the published one for a brand new reason.
    #[test]
    fn a_tree_with_no_commit_says_so_instead_of_inventing_a_date() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(commit_date(dir.path()), None);

        let missing = Environment {
            rustflags: String::new(),
            source_date_epoch: None,
            triple: "x86_64-unknown-linux-gnu".into(),
            zero_ar_date: false,
        };
        let shown = missing.describe().join("\n");
        assert!(shown.contains("not set"), "{shown}");
        assert!(shown.contains("not a git checkout"), "{shown}");
    }

    /// The remap has to match the path the compiler is given, or it matches
    /// nothing and does nothing -- silently, with every check still passing.
    ///
    /// On Windows `canonicalize` returns an extended-length path beginning
    /// `\\?\`, which cargo never hands to rustc. The release workflow's own
    /// comment records the same failure on macOS through `/tmp` against
    /// `/private/tmp`.
    #[test]
    fn the_remapped_path_is_the_one_the_compiler_is_given() {
        let root = repo_root();
        let seen = as_the_compiler_sees_it(&root).unwrap();
        assert!(
            !seen.to_string_lossy().starts_with(r"\\?\"),
            "an extended-length path remaps nothing: {}",
            seen.display()
        );
        assert!(seen.is_absolute(), "{}", seen.display());

        // `build .` and `build` have to remap to the same prefix, or two runs
        // of the same command from the same directory disagree.
        let dotted = as_the_compiler_sees_it(&root.join(".")).unwrap();
        assert_eq!(seen, dotted);
        let up_and_back = as_the_compiler_sees_it(&root.join("crates").join("..")).unwrap();
        assert_eq!(seen, up_and_back);
    }

    /// The flags match the release workflow, which is the only thing that makes
    /// the comparison meaningful. Checked against the workflow file itself, so
    /// changing one and not the other fails the build.
    #[test]
    fn the_flags_are_the_same_ones_the_release_workflow_uses() {
        let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
            .expect("the release workflow");

        for flag in ["--remap-path-prefix", "=/veilvoice", "=/cargo"] {
            assert!(
                workflow.contains(flag),
                "{flag} is used here and not by the release"
            );
        }
        assert!(workflow.contains("SOURCE_DATE_EPOCH"));
        assert!(workflow.contains("/Brepro"), "the MSVC flag");
        assert!(workflow.contains("-no_uuid"), "the ld64 flag");
        assert!(workflow.contains("ZERO_AR_DATE"));

        // And this platform's linker flag is one of the two the workflow sets.
        let link = repro_link();
        if !link.is_empty() {
            let bare = link.rsplit("link-arg=").next().unwrap();
            assert!(workflow.contains(bare), "{link} is not in the workflow");
        }
    }

    /// The build is run with an explicit `--target`, because the release is,
    /// and that moves the output down a level.
    #[test]
    fn the_output_directory_accounts_for_the_explicit_target() {
        let source = include_str!("builder.rs");
        let start = source.find("pub fn build(").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        assert!(
            body.contains(r#".args(["--target", &environment.triple])"#),
            "the release passes --target; a build without it is a different build"
        );
        assert!(
            body.contains("join(&environment.triple)"),
            "--target puts the binaries under the triple"
        );
        // RUSTFLAGS is set, never appended to: a value inherited from the
        // terminal is one the published build did not have.
        assert!(body.contains(r#".env("RUSTFLAGS", &environment.rustflags)"#));
    }

    /// This repository's own root, for the tests that need a real checkout.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
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
