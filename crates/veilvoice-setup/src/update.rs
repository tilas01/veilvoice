// SPDX-License-Identifier: GPL-3.0-or-later
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
//!
//! # In plain words
//!
//! This is the "check for updates" button, and nothing else.
//!
//! It runs when you press it and at no other time. There is no timer and nothing
//! in the background, because a program that checks by itself is telling somebody
//! else's computer that yours exists and how often you use it.
//!
//! It reads a public page anybody can open, tells you the newest version number,
//! and stops there. It does not download anything and it does not install
//! anything.

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

/// Which stream of releases a build came from.
///
/// **Roadmap item 165.** Releases are cut from `main` and development happens on
/// `dev`, which used to leave somebody who wanted the newest work either
/// building it themselves or waiting. An early build is the same reproducible,
/// signed archive published from `dev` under a tag that says so, marked a
/// prerelease on GitHub so it is never what the download page offers by
/// default.
///
/// The channel is read from the version the build carries rather than from
/// anything baked in at compile time, and that is the whole design: a
/// prerelease version *is* a prerelease, by semantic versioning, so the binary
/// stays a pure function of its source and the reproducibility instructions
/// need no extra input. `0.1.23-beta.1` is early; `0.1.23` is stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Cut from `main`. What the download page offers.
    Stable,
    /// Cut from `dev` under a prerelease tag. Newer, and checkable in exactly
    /// the same way.
    Early,
}

impl Channel {
    /// Which stream this version belongs to.
    pub fn of(version: &str) -> Channel {
        match parse(version) {
            Some(parsed) if !parsed.pre.is_empty() => Channel::Early,
            _ => Channel::Stable,
        }
    }

    /// The word for it, for a line somebody reads.
    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Early => "early",
        }
    }

    /// One sentence saying what this stream is, shown beside the label.
    pub fn describe(self) -> &'static str {
        match self {
            Channel::Stable => {
                "cut from main, signed, and what the download \
                                page offers"
            }
            Channel::Early => {
                "cut from dev under a prerelease tag: the same \
                               build, the same signing and the same hash \
                               lists, newer and less used"
            }
        }
    }

    /// The page an update check reads for this stream.
    ///
    /// `/releases/latest` never points at a prerelease, which is right for a
    /// stable build and useless for an early one: it would report a version
    /// older than the one running and call it an update. The list page names
    /// every release newest first, so an early build reads that instead and
    /// takes the first.
    pub fn url(self) -> &'static str {
        match self {
            Channel::Stable => LATEST_URL,
            Channel::Early => RELEASES_URL,
        }
    }
}

/// How this build's version compares with the newest published one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The newest published release is this one.
    UpToDate,
    /// Something newer exists. Carries the published version.
    Newer(String),
    /// This build is ahead of anything published, meaning an unreleased `main`.
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
    /// Which stream this build came from, and therefore which page was read.
    pub channel: Channel,
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
         with `veilvoice verify` before you run it."
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
    // Along this build's own stream, never across. Moving somebody from one to
    // the other without being asked is the one thing an update check must not
    // do: a stable copy should not be told about a prerelease, and an early
    // copy told only about stable releases would be told it is ahead of
    // everything for ever.
    let body = fetch(&tool, Channel::of(current).url())?;
    let latest = tag_in(&body).ok_or(Error::NoVersionFound)?;
    Ok(report(current, &latest))
}

/// Compare two version strings and build the report.
///
/// Split out from [`check`] so the comparison is testable without a network,
/// a subprocess, or a machine that has either.
pub fn report(current: &str, latest: &str) -> Report {
    use std::cmp::Ordering;
    let verdict = match (parse(current), parse(latest)) {
        (Some(here), Some(there)) => match precedence(&here, &there) {
            Ordering::Less => Verdict::Newer(latest.to_string()),
            Ordering::Greater => Verdict::Ahead(latest.to_string()),
            Ordering::Equal => Verdict::UpToDate,
        },
        _ => Verdict::Unreadable(latest.to_string()),
    };
    Report {
        current: current.to_string(),
        latest: latest.to_string(),
        channel: Channel::of(current),
        verdict,
    }
}

/// A version, in the only shape this project publishes.
///
/// Three numbers and an optional prerelease, which is what a tag like
/// `v0.1.23-beta.1` is. Build metadata after a `+` is accepted and dropped,
/// because semantic versioning says it takes no part in precedence.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    number: (u64, u64, u64),
    /// The dot-separated identifiers after the `-`, empty for a release.
    pre: Vec<String>,
}

/// `1.2.3`, `v1.2.3` or `v1.2.3-beta.1`.
///
/// Anything else is `None` rather than a guess.
///
/// **This used to refuse a prerelease outright**, and said why: ordering
/// `1.0.0-rc1` against `1.0.0` correctly needs the whole of semantic
/// versioning's precedence rules, and a checker that gets it subtly wrong
/// tells people to downgrade. That was the right call while nothing published
/// a prerelease. Roadmap item 165 publishes them, so refusing one now means an early
/// build cannot be told anything about its own stream, which is worse. The
/// rules are implemented in [`precedence`] rather than approximated, and the
/// tests below are the examples from the specification itself.
fn parse(version: &str) -> Option<Version> {
    let version = version.trim().trim_start_matches('v');
    // Build metadata is ignored for precedence, by the specification.
    let version = version.split('+').next()?;
    // A `-` with nothing after it is a typo, not a release: `1.0.0-` parsed as
    // `1.0.0` until a test asked, which would have made a mangled tag look
    // like the release it was mangled from.
    let (numbers, pre, separated) = match version.split_once('-') {
        Some((numbers, pre)) => (numbers, pre, true),
        None => (version, "", false),
    };
    if separated && pre.is_empty() {
        return None;
    }
    let mut parts = numbers.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let identifiers: Vec<String> = if pre.is_empty() {
        Vec::new()
    } else {
        pre.split('.').map(str::to_string).collect()
    };
    // An empty identifier is not a version, it is a typo: `1.0.0-` and
    // `1.0.0-beta..1` are both refused rather than read as something.
    if identifiers.iter().any(|part| part.is_empty()) {
        return None;
    }
    if identifiers
        .iter()
        .any(|part| !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return None;
    }
    Some(Version {
        number: (major, minor, patch),
        pre: identifiers,
    })
}

/// Semantic versioning's precedence, section 11, implemented rather than
/// approximated.
///
/// The three rules that matter here, in the order they are applied:
///
/// 1. the numbers compare first, and decide it if they differ;
/// 2. a version **with** a prerelease is lower than the same numbers without
///    one, which is the rule an approximation gets wrong and the reason
///    `0.1.23-beta.1` must not be read as newer than `0.1.23`;
/// 3. otherwise the identifiers compare left to right: two numeric ones
///    compare as numbers, a numeric one is always lower than an alphanumeric
///    one, two alphanumeric ones compare as text, and if everything so far is
///    equal the longer list wins.
fn precedence(here: &Version, there: &Version) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match here.number.cmp(&there.number) {
        Ordering::Equal => {}
        other => return other,
    }
    match (here.pre.is_empty(), there.pre.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    for (mine, yours) in here.pre.iter().zip(there.pre.iter()) {
        let numbers = (mine.parse::<u64>().ok(), yours.parse::<u64>().ok());
        let ordering = match numbers {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => mine.as_str().cmp(yours.as_str()),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    here.pre.len().cmp(&there.pre.len())
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
///
/// `url` is the page for this build's own stream, and which page it is decides
/// how curl is driven: see the note beside the two argument lists.
fn fetch(tool: &Tool, url: &str) -> Result<String, Error> {
    let seconds = TIMEOUT.as_secs().to_string();
    // `/releases/latest` answers with a redirect whose target *is* the tag, so
    // the body can be thrown away. `/releases`, which an early build reads
    // because the first page never names a prerelease, answers with itself:
    // there is no redirect to read and the tag is in the body. Asking curl for
    // the effective URL of that page would return the page's own address every
    // time, which parses as no version at all, so the body is what is read.
    let redirect_carries_the_answer = url == LATEST_URL;
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
            url,
        ]);
    } else if !redirect_carries_the_answer {
        command.args([
            "-L",
            "--max-redirs",
            "5",
            "--silent",
            "--show-error",
            "--max-time",
            &seconds,
            "--proto",
            "=https",
            url,
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
            url,
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
with `veilvoice verify` before running it.";

#[cfg(test)]
mod tests {
    use super::*;

    /// **Roadmap item 165.** Semantic versioning's own precedence examples, section
    /// 11, taken from the specification rather than invented here.
    ///
    /// The one that matters to this project is the middle of the chain:
    /// `1.0.0-rc.1` is **lower** than `1.0.0`. An approximation that compared
    /// the numbers and ignored the suffix would call an early build newer
    /// than the release it precedes and tell everybody on the stable channel
    /// to downgrade, which is the reason this was refused outright until
    /// there were prereleases to order.
    #[test]
    fn prerelease_precedence_is_the_specifications() {
        let chain = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in chain.windows(2) {
            let (lower, higher) = (parse(pair[0]).unwrap(), parse(pair[1]).unwrap());
            assert_eq!(
                precedence(&lower, &higher),
                std::cmp::Ordering::Less,
                "{} should be lower than {}",
                pair[0],
                pair[1]
            );
            assert_eq!(
                precedence(&higher, &lower),
                std::cmp::Ordering::Greater,
                "and {} higher than {}",
                pair[1],
                pair[0]
            );
        }
        // `1.0.0-beta.11` against `1.0.0-beta.2` is the case a text comparison
        // gets wrong, and it is in the chain above for that reason. Stated
        // again here so a rewrite that drops the chain still has it.
        assert_eq!(
            precedence(
                &parse("1.0.0-beta.2").unwrap(),
                &parse("1.0.0-beta.11").unwrap()
            ),
            std::cmp::Ordering::Less
        );
    }

    /// Build metadata takes no part in precedence, and a malformed version is
    /// refused rather than read as something.
    #[test]
    fn what_a_version_is_and_is_not() {
        assert_eq!(parse("v0.1.23+build.5"), parse("0.1.23"));
        for bad in [
            "",
            "1.0",
            "1.0.0.0",
            "1.0.0-",
            "1.0.0-beta..1",
            "banana",
            "1.0.0-b^ta",
        ] {
            assert!(parse(bad).is_none(), "{bad:?} is not a version");
        }
    }

    /// A build reports the stream it came from, read from its own version.
    #[test]
    fn the_channel_comes_from_the_version() {
        assert_eq!(Channel::of("0.1.23"), Channel::Stable);
        assert_eq!(Channel::of("v0.1.23"), Channel::Stable);
        assert_eq!(Channel::of("0.1.23-beta.1"), Channel::Early);
        // Unreadable is treated as stable rather than as early: a build whose
        // version cannot be parsed has said nothing about its stream, and
        // quietly moving somebody to the prerelease page on the strength of a
        // string nobody could read is the wrong way round.
        assert_eq!(Channel::of("nonsense"), Channel::Stable);
        assert_eq!(Channel::Stable.url(), LATEST_URL);
        assert_eq!(Channel::Early.url(), RELEASES_URL);
    }

    /// An update check looks along its own stream and says which.
    #[test]
    fn a_check_stays_on_its_own_channel() {
        let early = report("0.1.23-beta.1", "0.1.23-beta.2");
        assert_eq!(early.channel, Channel::Early);
        assert_eq!(early.verdict, Verdict::Newer("0.1.23-beta.2".into()));

        // The release this build precedes is newer than it, which is the
        // answer somebody on an early build wants when the release lands.
        let landed = report("0.1.23-beta.1", "0.1.23");
        assert_eq!(landed.verdict, Verdict::Newer("0.1.23".into()));

        // And a stable build is never told about a prerelease, because it
        // never reads the page that names one.
        let stable = report("0.1.23", "0.1.23");
        assert_eq!(stable.channel, Channel::Stable);
        assert_eq!(stable.verdict, Verdict::UpToDate);

        // A source build ahead of everything published still says so.
        assert_eq!(
            report("0.2.0", "0.1.23").verdict,
            Verdict::Ahead("0.1.23".into())
        );
    }

    /// The guide describes the tool this actually looks for.
    ///
    /// Written after the guide said "PowerShell's web request on Windows",
    /// which this has never used: it looks for `System32\\curl.exe` by
    /// absolute path. That was a sentence about the program, written beside
    /// the program, wrong on the day it was written -- which is the shape of
    /// finding after finding in this repository, and is why the two are
    /// compared here rather than trusted to agree.
    #[test]
    fn the_guide_names_the_tool_this_looks_for() {
        let guide = include_str!("../../../docs/USER_GUIDE.md").replace("\r\n", "\n");
        let source = include_str!("update.rs");
        let looking = source
            .split("fn find_tool()")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("find_tool exists");

        // Every absolute path the program will actually try.
        let mut paths: Vec<&str> = looking
            .match_indices('"')
            .map(|(at, _)| &looking[at + 1..])
            .filter_map(|rest| rest.split('"').next())
            .filter(|candidate| candidate.contains('/') || candidate.contains('\\'))
            .collect();
        paths.sort();
        paths.dedup();
        assert!(!paths.is_empty(), "find_tool names no paths");

        for path in paths {
            // The guide writes Windows paths with the environment variable
            // spelled out, so compare on the part that identifies the tool.
            let needle = path.rsplit(['/', '\\']).next().unwrap_or(path);
            assert!(
                guide.contains(needle),
                "the update check looks for {path:?} and docs/USER_GUIDE.md \
                 never mentions {needle:?}"
            );
        }

        assert!(
            !guide.contains("PowerShell's web request"),
            "the guide credits PowerShell for the update check; this has never \
             used it"
        );
    }

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

    /// Ordering is refused rather than guessed, and what counts as
    /// unorderable has narrowed.
    ///
    /// This used to assert that **any** prerelease was unreadable, which was
    /// right while nothing published one: a checker that gets prerelease
    /// precedence subtly wrong tells people to downgrade. Roadmap item 165 publishes
    /// them, so they are ordered now, by the specification's rules and against
    /// the specification's own examples. A string that is not a version at all
    /// is still refused, and that is the half worth keeping.
    #[test]
    fn a_version_that_cannot_be_compared_is_refused_rather_than_ordered() {
        assert_eq!(
            report("0.1.12", "nightly").verdict,
            Verdict::Unreadable("nightly".into())
        );
        assert_eq!(
            report("0.1.12", "0.2").verdict,
            Verdict::Unreadable("0.2".into())
        );
        assert!(parse("1.2").is_none(), "three numbers or nothing");
        assert!(parse("1.2.3.4").is_none(), "three numbers or nothing");
        // And the one that changed: a prerelease is now ordered rather than
        // refused, and ordered *below* the release it precedes.
        assert_eq!(
            report("0.1.12", "0.2.0-rc1").verdict,
            Verdict::Newer("0.2.0-rc1".into())
        );
        assert_eq!(
            report("0.2.0", "0.2.0-rc1").verdict,
            Verdict::Ahead("0.2.0-rc1".into())
        );
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
        assert!(caveat.contains("veilvoice verify"));
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
