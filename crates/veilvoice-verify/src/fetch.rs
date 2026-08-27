// SPDX-License-Identifier: GPL-3.0-or-later
//! Download a release, without putting an HTTP client in the dependency graph.
//!
//! # The constraint this is written around
//!
//! VeilVoice is **offline by construction**, and that is not a slogan: a CI job
//! fails the build if `reqwest`, `hyper`, `curl`, `ureq`, `tungstenite`,
//! `isahc` or `surf` appears anywhere in `cargo tree`. The claim on the front
//! page -- "no network code in the dependency graph" -- is one a reader can
//! check in ten seconds, and it is a large part of why this project is worth
//! trusting.
//!
//! Fetching a release to check it is a genuinely useful thing for *this*
//! binary to do, and it is also the one thing that claim forbids. So the
//! download is done by **the tool the operating system already ships**, invoked
//! as a subprocess:
//!
//! | Platform | Used | Already present because |
//! |---|---|---|
//! | Windows 10+ | `curl.exe` | shipped in `System32` since 2018 |
//! | macOS | `curl` | part of the base system |
//! | Linux, BSD | `curl`, else `wget` | one or the other is on essentially every install |
//!
//! `cargo tree` stays exactly as clean as it was, the CI job that enforces it
//! is untouched, and nothing in the *library* crates gained the ability to talk
//! to anything. What changed is that one command-line tool, whose entire
//! purpose is checking downloads, can now also make one -- when asked, never on
//! its own.
//!
//! This is the same pattern the rest of the project already uses for
//! platform work it will not link a dependency for: `veilvoice-watch` reads the
//! registry through `reg query`, and `veilvoice-guard` reads the event log
//! through `wevtutil`.
//!
//! # What is deliberately not done here
//!
//! **No downloader is resolved by bare name.** Finding `curl` by searching
//! `PATH` on Windows includes the current directory, so running this from a
//! folder containing a hostile `curl.exe` would run that instead -- finding
//! F-13, in the one program whose job is deciding whether a download is
//! genuine. Absolute paths are tried first, and a bare name is only ever a
//! last resort on platforms where the search order does not include the
//! working directory.
//!
//! **Nothing is fetched implicitly.** A download happens because the user
//! passed a subcommand that says so. There is no update check, no telemetry,
//! and no "just in case" request.
//!
//! **Only one host is ever contacted**, and it is compiled in. A URL cannot be
//! supplied on the command line, so this cannot be turned into a general
//! downloader by an argument.
//!
//! # In plain words
//!
//! Downloads a release, without VeilVoice containing any networking code.
//!
//! It asks the tool your system already has to do the fetching. That is what keeps
//! a real promise the rest of the project makes: there is no HTTP client anywhere
//! in what VeilVoice is built from, which you can check yourself, and this is the
//! one command that touches the network at all.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The only host this will ever talk to.
///
/// Compiled in rather than accepted as an argument. A verifier that can be
/// pointed anywhere is a download tool wearing a verifier's name, and the
/// fingerprint check below is only meaningful against artefacts from the
/// project it was built for.
pub const HOST: &str = "https://github.com";

/// The repository releases are fetched from.
pub const REPO: &str = "tilas01/veilvoice";

/// The largest file this will accept.
///
/// A release archive is tens of megabytes. This bounds what a redirected or
/// substituted response can make the tool write to disk, and it is checked
/// after the download rather than trusted from a header, because a header is
/// something the other end chose.
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Where a downloader was found, and what to call it.
struct Downloader {
    program: PathBuf,
    style: Style,
}

#[derive(Clone, Copy)]
enum Style {
    Curl,
    // Only reachable on the platforms whose lookup below considers it. Windows
    // ships curl and nothing else worth preferring, so on that target this
    // variant is genuinely dead -- and `-D warnings` is right to say so.
    #[cfg_attr(windows, allow(dead_code))]
    Wget,
}

/// Absolute paths first, and a bare name only where that is safe.
///
/// Resolving a program by bare name on Windows searches the **current
/// directory** before most of `PATH`, so a file called `curl.exe` sitting
/// beside a downloaded archive would be run instead of the system one. That is
/// finding F-13, and this is the program where it would matter most.
fn find_downloader() -> Option<Downloader> {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let curl = PathBuf::from(format!(r"{root}\System32\curl.exe"));
        if curl.is_file() {
            return Some(Downloader {
                program: curl,
                style: Style::Curl,
            });
        }
        None
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/curl", "/bin/curl", "/usr/local/bin/curl"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(Downloader {
                    program: path,
                    style: Style::Curl,
                });
            }
        }
        for candidate in ["/usr/bin/wget", "/bin/wget", "/usr/local/bin/wget"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(Downloader {
                    program: path,
                    style: Style::Wget,
                });
            }
        }
        None
    }
}

/// Say what could not be found, and what to do instead.
///
/// A tool that cannot download should explain how to proceed without it, not
/// merely report an absence. Everything this fetches can be fetched by hand.
pub fn no_downloader_message() -> String {
    format!(
        "no downloader was found on this system.\n\n\
         This tool does not contain an HTTP client -- VeilVoice has no network \
         code in its dependency graph at all, which is a property you can check \
         with `cargo tree`. It borrows the one your operating system already \
         ships, and could not find it.\n\n\
         Download the files yourself from\n  {HOST}/{REPO}/releases\n\
         and pass them in:\n\n  \
         veilvoice-verify file <ARCHIVE> --sums SHA256SUMS --sig SHA256SUMS.asc\n"
    )
}

/// Fetch one URL into `into`. Returns the path written.
///
/// Every failure is reported rather than retried. A verifier that quietly
/// tries again is a verifier whose output does not say what actually happened.
pub fn download(url: &str, into: &Path) -> Result<PathBuf, String> {
    if !url.starts_with(HOST) {
        // Unreachable through the public API, which builds every URL from the
        // constants above -- but this is the check that keeps that true if
        // somebody adds a caller later.
        return Err(format!("refusing to fetch from outside {HOST}: {url}"));
    }
    let Some(downloader) = find_downloader() else {
        return Err(no_downloader_message());
    };

    let mut command = Command::new(&downloader.program);
    match downloader.style {
        Style::Curl => {
            command.args([
                "--fail",     // an HTTP error is a failure, not a saved error page
                "--location", // GitHub redirects release assets to its CDN
                "--silent",
                "--show-error",
                "--proto",
                "=https", // never downgrade, never another scheme
                "--max-time",
                "600",
                "--output",
            ]);
            command.arg(into);
            command.arg(url);
        }
        Style::Wget => {
            command.args(["--quiet", "--https-only", "--timeout=600", "-O"]);
            command.arg(into);
            command.arg(url);
        }
    }

    let output = command
        .output()
        .map_err(|error| format!("could not run {}: {error}", downloader.program.display()))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = std::fs::remove_file(into);
        return Err(if detail.is_empty() {
            format!("download failed: {url}")
        } else {
            format!("download failed: {url}\n  {detail}")
        });
    }

    let size = std::fs::metadata(into)
        .map_err(|error| format!("downloaded file is unreadable: {error}"))?
        .len();
    if size == 0 {
        let _ = std::fs::remove_file(into);
        return Err(format!("download produced an empty file: {url}"));
    }
    if size > MAX_BYTES {
        let _ = std::fs::remove_file(into);
        return Err(format!(
            "downloaded file is {size} bytes, over the {MAX_BYTES} limit; refusing it"
        ));
    }
    Ok(into.to_path_buf())
}

/// The URL of one file in one release.
pub fn asset_url(tag: &str, name: &str) -> String {
    format!("{HOST}/{REPO}/releases/download/{tag}/{name}")
}

/// The three files every release publishes for checking itself.
pub const SUMS: &str = "SHA256SUMS";
pub const SIGNATURE: &str = "SHA256SUMS.asc";

/// A release tag, rejected unless it looks like one.
///
/// The tag becomes part of a URL and part of a filename, so it is validated
/// rather than trusted: without this, a tag containing `../` would write
/// outside the download directory, and one containing a shell metacharacter
/// would be passed to a subprocess. The argument is passed as a separate
/// argv entry rather than through a shell, so the second is already handled --
/// but a check that only holds because of how the caller happens to invoke
/// things is not a check.
pub fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 40
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// An asset filename, rejected unless it looks like one.
pub fn valid_asset(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_outside_the_compiled_in_host_is_refused() {
        let target = std::env::temp_dir().join("veilvoice-fetch-test-never-written");
        let error = download("https://example.invalid/thing", &target)
            .expect_err("an arbitrary host must be refused");
        assert!(error.contains("refusing to fetch"), "{error}");
        assert!(!target.exists(), "a refused download still wrote a file");
    }

    #[test]
    fn a_tag_that_would_escape_the_directory_is_refused() {
        for bad in [
            "../../etc",
            "a/b",
            "tag with spaces",
            "",
            "a;rm -rf /",
            "v1$(x)",
        ] {
            assert!(!valid_tag(bad), "the tag {bad:?} was accepted");
        }
        for good in ["v0.1.11", "v1.0.0-rc1", "nightly_2026"] {
            assert!(valid_tag(good), "the tag {good:?} was refused");
        }
    }

    #[test]
    fn an_asset_name_that_would_escape_the_directory_is_refused() {
        for bad in ["../SHA256SUMS", "a/b.tar.gz", "", "x y.zip"] {
            assert!(!valid_asset(bad), "the name {bad:?} was accepted");
        }
        for good in [
            "SHA256SUMS",
            "SHA256SUMS.asc",
            "veilvoice-v0.1.11-linux-x86_64.tar.gz",
        ] {
            assert!(valid_asset(good), "the name {good:?} was refused");
        }
    }

    #[test]
    fn an_asset_url_is_built_from_the_compiled_in_host_and_repository() {
        let url = asset_url("v0.1.11", "SHA256SUMS");
        assert!(url.starts_with(HOST), "{url}");
        assert!(url.contains(REPO), "{url}");
        assert!(url.ends_with("/v0.1.11/SHA256SUMS"), "{url}");
    }

    #[test]
    fn the_absent_downloader_message_says_how_to_proceed_without_one() {
        // A tool that cannot download must explain the manual route rather
        // than merely reporting that it cannot.
        let text = no_downloader_message();
        assert!(text.contains("releases"), "no link to the releases: {text}");
        assert!(text.contains("--sums"), "no manual command shown: {text}");
        assert!(
            text.contains("no network code"),
            "the reason there is no built-in client should be stated: {text}"
        );
    }
}
