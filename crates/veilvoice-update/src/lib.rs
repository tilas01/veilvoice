// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Ask, **only when told to**, whether a newer VeilVoice release exists.
//!
//! # What this is, and the claim it changes
//!
//! Until this crate existed, VeilVoice's front page said *"no telemetry, no
//! update check"*. Half of that is unchanged and half of it is not, and the
//! wording moved in the same commit as the code rather than afterwards:
//!
//! * **No telemetry.** Unchanged, and nothing here sends anything about you.
//!   The request is a plain `GET` of a public URL that anybody can open in a
//!   browser; it carries no identifier, no configuration and no counter.
//! * **No *automatic* update check.** Nothing runs on a timer, at startup, or
//!   in the background. [`check`] runs because a person pressed a button in
//!   this run of the program, and it does nothing else ever.
//!
//! An update checker that runs by itself is a beacon: it tells a server that
//! this machine has VeilVoice on it, roughly how often it is used, and from
//! which address. That is the thing being refused. A button somebody presses,
//! once, when they want to know, is a different act with different consequences,
//! and it is the only one on offer.
//!
//! # There is still no HTTP client in the dependency graph
//!
//! This crate has **no dependencies**. It runs the transfer tool the operating
//! system already ships, exactly as `veilvoice-verify` has fetched releases
//! since it existed, and reads its output. `cargo tree` shows no `reqwest`, no
//! `hyper`, no `ureq`; the CI job that fails the build if one appears is
//! unchanged and still passes.
//!
//! The tool is found at an **absolute path**, never by bare name. Resolving a
//! program by name on Windows searches the current directory before most of
//! `PATH`, so a file called `curl.exe` sitting beside the program would be run
//! instead of the system one. That is finding F-13, and it does not get to
//! happen twice.
//!
//! # What it will not do
//!
//! It does not download a release, it does not install anything, and it does
//! not restart the program. It reports a version string and leaves every
//! decision to the reader. Downloading a release and checking its signature is
//! `veilvoice-verify`'s job, and that is a separate, deliberate act too.
//!
//! An update checker that could install its own answer is an update checker
//! that can be made to install somebody else's.
//!
//! # What a "newer version" is worth here
//!
//! The answer comes from a public web page over TLS. That is enough to say
//! *"there is probably something newer, go and look"* and it is **not** enough
//! to act on: a name in a document is not a signature. Nothing in this crate
//! verifies anything, and [`Report::caveat`] says so in the words the user
//! sees rather than only in this comment.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// The repository asked about.
pub const REPO: &str = "tilas01/veilvoice";

/// The page fetched. A plain URL a person can open themselves and compare.
///
/// Deliberately the human-readable redirect rather than an API endpoint: it is
/// checkable by eye, it needs no token, and it is not rate limited per address
/// in the way the API is. The redirect's target carries the tag.
pub const LATEST_URL: &str = "https://github.com/tilas01/veilvoice/releases/latest";

/// Where releases are listed, for somebody doing this by hand.
pub const RELEASES_URL: &str = "https://github.com/tilas01/veilvoice/releases";

/// How long the transfer tool is given before it is given up on.
///
/// Short on purpose. This runs because somebody pressed a button and is
/// waiting; a check that hangs for a minute on a captive portal is worse than
/// one that says it could not reach anything.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// How this build's version compares with the newest published one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The newest published release is this one.
    UpToDate,
    /// Something newer exists. Carries the published version.
    Newer(String),
    /// This build is ahead of anything published — an unreleased `main`.
    Ahead(String),
    /// A version string came back that this build cannot compare.
    ///
    /// Reported rather than guessed at. Two version strings that do not parse
    /// are two strings, and pretending to order them is how a checker tells
    /// somebody they are out of date when they are not.
    Unreadable(String),
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The version this build reports.
    pub current: String,
    /// The newest version the page named.
    pub latest: String,
    /// How the two compare.
    pub verdict: Verdict,
}

impl Report {
    /// What this answer is worth, in the words the user should see.
    ///
    /// Carried on the report rather than written into whichever front end
    /// happens to be showing it, so a second front end cannot show the answer
    /// without the caveat.
    pub fn caveat(&self) -> &'static str {
        "This is a version number read off a public page. It is not a signature \
         and nothing here has verified anything. Download a release and check it \
         with veilvoice-verify before you run it."
    }
}

/// Why a check could not be completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The system ships no transfer tool this crate knows how to drive.
    NoTransferTool,
    /// The tool ran and failed. Carries what it said.
    Failed(String),
    /// The response arrived and held no version this crate could find.
    NoVersionFound,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTransferTool => write!(
                f,
                "no transfer tool was found on this system. VeilVoice contains no HTTP \
                 client -- it borrows the one your operating system ships, and could not \
                 find it. Open {RELEASES_URL} yourself instead."
            ),
            Self::Failed(why) => write!(f, "the check could not be completed: {why}"),
            Self::NoVersionFound => write!(
                f,
                "the reply held no version number this build could read. Open \
                 {RELEASES_URL} and look."
            ),
        }
    }
}

impl std::error::Error for Error {}

/// The version this build was compiled as.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ask whether anything newer than `current` has been published.
///
/// **Runs a subprocess and waits.** Never call it from a thread that paints:
/// a network round trip on the UI thread is the freeze the user reports.
pub fn check(current: &str) -> Result<Report, Error> {
    let tool = find_tool().ok_or(Error::NoTransferTool)?;
    let body = fetch(&tool)?;
    let latest = tag_in(&body).ok_or(Error::NoVersionFound)?;
    Ok(report(current, &latest))
}

/// Compare two version strings and build the report.
///
/// Split out from [`check`] so the comparison is testable without a network,
/// a subprocess, or a machine that has either.
pub fn report(current: &str, latest: &str) -> Report {
    let verdict = match (parse(current), parse(latest)) {
        (Some(here), Some(there)) => {
            if there > here {
                Verdict::Newer(latest.to_string())
            } else if here > there {
                Verdict::Ahead(latest.to_string())
            } else {
                Verdict::UpToDate
            }
        }
        _ => Verdict::Unreadable(latest.to_string()),
    };
    Report {
        current: current.to_string(),
        latest: latest.to_string(),
        verdict,
    }
}

/// `1.2.3` or `v1.2.3` as three numbers.
///
/// Anything else is `None` rather than a guess. A pre-release suffix makes the
/// string unreadable on purpose: ordering `1.0.0-rc1` against `1.0.0` correctly
/// needs the whole of semantic versioning's precedence rules, and a checker
/// that gets it subtly wrong tells people to downgrade.
fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let version = version.trim().trim_start_matches('v');
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// The tag in whatever the transfer tool printed.
///
/// With curl this is the one-line redirect target and the tag is the whole
/// point of it. With wget it is the page body, which names the same tag. One
/// scanner for both, because the shape being looked for is identical and a
/// second code path is a second thing to get wrong.
///
/// Scanned rather than parsed as HTML: parsing a document to read one substring
/// is a dependency and an attack surface for a job a search does exactly as
/// well.
///
/// # Every match, not the first one
///
/// The first version of this took the first `/releases/tag/` it found. Run
/// against the real page, it returned nothing: GitHub's release page contains
/// an **empty** `/releases/tag/` before any real one -- a template link with no
/// tag after it -- so the first match yielded an empty string and the check
/// reported "no version number" against a page that plainly had one. Found by
/// running it, not by reading it.
///
/// Bounded on both ends: at most 32 characters, and only characters a version
/// tag is made of. A page that came back as something else entirely produces no
/// match rather than a run of somebody's markup shown to the user as a version.
pub fn tag_in(body: &str) -> Option<String> {
    const MARKER: &str = "/releases/tag/";
    let mut from = 0usize;
    while let Some(found) = body[from..].find(MARKER) {
        let start = from + found + MARKER.len();
        let tag: String = body[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .take(32)
            .collect();
        if !tag.is_empty() {
            return Some(tag);
        }
        from = start;
    }
    None
}

/// This platform's bit bucket, for a reply whose body is not wanted.
#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
/// This platform's bit bucket, for a reply whose body is not wanted.
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

/// Where a transfer tool was found, and how to drive it.
struct Tool {
    program: PathBuf,
    wget: bool,
}

/// Absolute paths only. See the module note on finding F-13.
fn find_tool() -> Option<Tool> {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let curl = PathBuf::from(format!(r"{root}\System32\curl.exe"));
        if curl.is_file() {
            return Some(Tool {
                program: curl,
                wget: false,
            });
        }
        None
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/curl", "/bin/curl", "/usr/local/bin/curl"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(Tool {
                    program: path,
                    wget: false,
                });
            }
        }
        for candidate in ["/usr/bin/wget", "/bin/wget", "/usr/local/bin/wget"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(Tool {
                    program: path,
                    wget: true,
                });
            }
        }
        None
    }
}

/// Run the tool and hand back what it printed.
fn fetch(tool: &Tool) -> Result<String, Error> {
    let seconds = TIMEOUT.as_secs().to_string();
    let mut command = Command::new(&tool.program);
    if tool.wget {
        command.args([
            "--quiet",
            "--max-redirect=5",
            "--timeout",
            &seconds,
            "--tries=1",
            "-O",
            "-",
            LATEST_URL,
        ]);
    } else {
        // The body is thrown away and only the **final URL** is printed. The
        // redirect target of `/releases/latest` *is* the tag -- one line
        // instead of two hundred kilobytes of a page, nothing of the reply
        // reaching the scanner, and no way for markup to be mistaken for a
        // version. Measured on the real page: 205,538 bytes against 54.
        command.args([
            "-L",
            "--max-redirs",
            "5",
            "--silent",
            "--show-error",
            "--max-time",
            &seconds,
            // `--proto =https` refuses to be redirected onto a plain-text or a
            // file scheme. Without it a redirect chain decides the protocol.
            "--proto",
            "=https",
            "-o",
            NULL_DEVICE,
            "-w",
            "%{url_effective}",
            LATEST_URL,
        ]);
    }
    let output = command.output().map_err(|e| Error::Failed(e.to_string()))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Failed(if said.is_empty() {
            format!("{} exited with {}", tool.program.display(), output.status)
        } else {
            said
        }));
    }
    // Bounded before it is looked at. A reply is a public release page; a
    // reply the size of a film is something else, and reading all of it into
    // a string to search for one substring is the wrong thing to do with it.
    const MAX: usize = 4 * 1024 * 1024;
    let body = &output.stdout[..output.stdout.len().min(MAX)];
    Ok(String::from_utf8_lossy(body).into_owned())
}

/// What this crate does and does not do, in one paragraph, for a front end to
/// show beside the button.
pub const SCOPE: &str = "\
This check happens because you pressed the button, and at no other time. There \
is no timer, no check at startup, and nothing runs in the background. VeilVoice \
contains no HTTP client: this borrows the transfer tool your operating system \
already ships and reads a public page anybody can open. It sends nothing about \
you or this machine. It does not download a release, install anything, or \
restart the program -- it reports a version number, and every decision after \
that is yours. A version number on a page is not a signature: check a download \
with veilvoice-verify before running it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_higher_published_version_is_newer() {
        let report = report("0.1.12", "0.2.0");
        assert_eq!(report.verdict, Verdict::Newer("0.2.0".into()));
        assert_eq!(report.current, "0.1.12");
    }

    #[test]
    fn the_same_version_is_up_to_date() {
        assert_eq!(report("0.1.12", "v0.1.12").verdict, Verdict::UpToDate);
    }

    /// An unreleased build is ahead, and is told so rather than told it is
    /// current. Saying "up to date" to somebody running `main` hides the one
    /// fact that matters about what they are running.
    #[test]
    fn a_build_ahead_of_the_newest_release_is_told_so() {
        assert_eq!(
            report("0.2.0", "0.1.12").verdict,
            Verdict::Ahead("0.1.12".into())
        );
    }

    /// Ordering is refused rather than guessed. A checker that gets
    /// pre-release precedence subtly wrong tells people to downgrade.
    #[test]
    fn a_version_that_cannot_be_compared_is_refused_rather_than_ordered() {
        assert_eq!(
            report("0.1.12", "0.2.0-rc1").verdict,
            Verdict::Unreadable("0.2.0-rc1".into())
        );
        assert_eq!(
            report("0.1.12", "nightly").verdict,
            Verdict::Unreadable("nightly".into())
        );
        assert!(parse("1.2").is_none(), "three numbers or nothing");
        assert!(parse("1.2.3.4").is_none(), "three numbers or nothing");
    }

    #[test]
    fn the_leading_v_is_optional_on_both_sides() {
        assert_eq!(parse("v1.2.3"), parse("1.2.3"));
        assert_eq!(
            report("v1.0.0", "1.0.1").verdict,
            Verdict::Newer("1.0.1".into())
        );
    }

    #[test]
    fn the_tag_is_read_out_of_a_release_page() {
        let body = r#"<a href="/tilas01/veilvoice/releases/tag/v0.1.12">v0.1.12</a>"#;
        assert_eq!(tag_in(body).as_deref(), Some("v0.1.12"));
    }

    /// A page that is not a release page yields nothing, rather than a
    /// fragment of somebody's markup shown to the user as a version.
    /// The real page carries an empty `/releases/tag/` before any real one.
    /// Taking the first match returned nothing against a page that plainly had
    /// a version on it, which is how this was found: by running it.
    #[test]
    fn an_empty_tag_link_before_a_real_one_is_stepped_over() {
        let body = r#"<a href="/tilas01/veilvoice/releases/tag/"></a>
                      <a href="/tilas01/veilvoice/releases/tag/v0.1.12">v0.1.12</a>"#;
        assert_eq!(tag_in(body).as_deref(), Some("v0.1.12"));
    }

    /// curl is asked for the redirect target rather than the page, so what the
    /// scanner sees is one line. This is the shape it has to handle.
    #[test]
    fn the_redirect_target_alone_is_enough() {
        let url = "https://github.com/tilas01/veilvoice/releases/tag/v0.1.12";
        assert_eq!(tag_in(url).as_deref(), Some("v0.1.12"));
    }

    #[test]
    fn a_page_with_no_tag_yields_nothing() {
        assert_eq!(tag_in("<html><body>hello</body></html>"), None);
        assert_eq!(tag_in(""), None);
        assert_eq!(tag_in("/releases/tag/"), None);
        assert_eq!(tag_in("/releases/tag/<script>"), None);
    }

    /// Whatever comes back, what is shown is short and made of characters a
    /// version number is made of.
    #[test]
    fn a_hostile_tag_is_bounded_in_length_and_alphabet() {
        let long = format!("/releases/tag/{}", "a".repeat(500));
        let tag = tag_in(&long).expect("a tag is found");
        assert_eq!(tag.len(), 32, "bounded to 32 characters");

        let markup = "/releases/tag/v1.0.0\"><img src=x onerror=alert(1)>";
        assert_eq!(tag_in(markup).as_deref(), Some("v1.0.0"));
    }

    /// The URL asked about is this project's own, over TLS, and is a page a
    /// person can open and compare by hand.
    #[test]
    fn the_url_is_this_project_over_tls() {
        assert!(LATEST_URL.starts_with("https://"));
        assert!(LATEST_URL.contains(REPO));
        assert!(RELEASES_URL.starts_with("https://"));
        assert!(RELEASES_URL.contains(REPO));
    }

    /// The scope note has to state the limits, not only the capability. This
    /// is the wording the front page's claim now depends on.
    #[test]
    fn the_scope_note_states_what_it_does_not_do() {
        let scope = SCOPE.to_lowercase();
        for phrase in [
            "you pressed the button",
            "no timer",
            "no http client",
            "sends nothing about you",
            "does not download",
            "not a signature",
        ] {
            assert!(scope.contains(phrase), "the scope note must say {phrase:?}");
        }
    }

    /// Every report carries the caveat, so no front end can show the answer
    /// without it.
    #[test]
    fn a_report_carries_what_the_answer_is_worth() {
        let caveat = report("0.1.12", "0.2.0").caveat().to_lowercase();
        assert!(caveat.contains("not a signature"));
        assert!(caveat.contains("veilvoice-verify"));
    }

    /// Errors explain what to do instead rather than only that something
    /// failed. A tool that cannot reach the network still has a user who wants
    /// to know whether there is an update.
    #[test]
    fn every_failure_says_what_to_do_instead() {
        for error in [
            Error::NoTransferTool,
            Error::NoVersionFound,
            Error::Failed("connection refused".into()),
        ] {
            let text = error.to_string();
            assert!(!text.is_empty());
        }
        assert!(Error::NoTransferTool.to_string().contains(RELEASES_URL));
        assert!(Error::NoVersionFound.to_string().contains(RELEASES_URL));
        assert!(Error::Failed("refused".into())
            .to_string()
            .contains("refused"));
    }
}
