// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Learn what normally runs on this machine, then notice what does not.
//!
//! # What this is, said before anything else
//!
//! **This is not application control.** It does not stop a program starting,
//! it cannot stop one starting, and nothing in this crate tries. It is a
//! *baseline*: you tell it to watch for a while, it records what it sees, and
//! afterwards it can tell you when something runs that was not in that
//! picture.
//!
//! The name is the roadmap's and it is kept because renaming it would leave two
//! names for one thing, but [`SCOPE`] is the wording every front end must show,
//! and it says outright that nothing here prevents anything. Real enforcement
//! means a kernel driver or a signed policy blob and an application identity to
//! sign it with, and this project is published under a pseudonym on purpose.
//! Shipping something called "app control" that quietly only *watches* would be
//! the exact failure this project's second rule exists to prevent.
//!
//! # Learning, and why it has an end
//!
//! [`Baseline::learning`] records what runs. It is not left on: a baseline that
//! is always learning has learned nothing, because whatever an attacker starts
//! becomes part of the picture the moment it starts. So learning is a phase
//! with an end, and [`Baseline::freeze`] closes it.
//!
//! # Grants expire, and that is the whole design
//!
//! Allowing something for ever is how an allowlist becomes a list of everything
//! anybody ever ran. [`Grant`]s carry an expiry, [`Baseline::allowed`] checks
//! it against the clock it is given, and an expired grant is simply not a
//! grant. Nothing sweeps them: an expired entry is kept, because *"this was
//! allowed until Tuesday"* is worth more to somebody reading the log than a row
//! that vanished.
//!
//! Permanent grants exist and are spelled [`Grant::forever`], so that choosing
//! one is a thing somebody typed rather than a default they never saw.
//!
//! # The log is append-only, and it is the point
//!
//! Every decision is recorded: what was seen, whether it was known, which grant
//! covered it and when that grant ends. A control whose decisions cannot be
//! reviewed afterwards is a control nobody can check, and this one is *only*
//! ever going to be reviewable, since it does not enforce.
//!
//! # In plain words
//!
//! For a while, this watches which programs you normally run and writes them
//! down. After that, it can tell you when something starts that was not on that
//! list.
//!
//! It does **not** block anything. It cannot. It is a way of noticing, not a
//! lock on the door, and anything that told you otherwise would be lying to
//! you about how safe you are.
//!
//! You can allow a program you recognise, and when you do you say for how long
//! -- an hour, a day, or permanently if you really mean it. Temporary is the
//! normal case, because a list that only ever grows stops meaning anything.
//! Everything it decides is written to a log you can read.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

/// The file format's first line. The digit is a version.
const MAGIC: &str = "VEILAPPCTL1";

/// How long a grant lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grant {
    /// Until this moment, and not one second after.
    Until(SystemTime),
    /// For good.
    ///
    /// Spelled out rather than represented by a very large date, so that a
    /// reader of the file and a reader of the code both see the difference
    /// between "allowed until 2027" and "allowed for ever". A far-future
    /// timestamp is a permanent grant wearing a disguise.
    Forever,
}

impl Grant {
    /// A grant lasting `how_long` from `now`.
    ///
    /// Returns `None` when the duration would run off the end of the clock,
    /// rather than saturating: a grant that silently became permanent because
    /// somebody typed too many digits is precisely what [`Grant::Forever`]
    /// exists to make explicit.
    pub fn for_duration(now: SystemTime, how_long: Duration) -> Option<Grant> {
        now.checked_add(how_long).map(Grant::Until)
    }

    /// A permanent grant. Named, so choosing one is deliberate.
    pub fn forever() -> Grant {
        Grant::Forever
    }

    /// Whether this grant still covers `now`.
    pub fn covers(&self, now: SystemTime) -> bool {
        match self {
            Grant::Forever => true,
            Grant::Until(end) => now < *end,
        }
    }

    /// How this reads in a report.
    pub fn describe(&self, now: SystemTime) -> String {
        match self {
            Grant::Forever => "for ever".to_string(),
            Grant::Until(end) => match end.duration_since(now) {
                Ok(left) => format!("for another {}", plain_duration(left)),
                Err(_) => "expired".to_string(),
            },
        }
    }
}

/// A duration a person can read.
fn plain_duration(left: Duration) -> String {
    let seconds = left.as_secs();
    if seconds >= 86_400 {
        let days = seconds / 86_400;
        format!("{days} day{}", if days == 1 { "" } else { "s" })
    } else if seconds >= 3_600 {
        let hours = seconds / 3_600;
        format!("{hours} hour{}", if hours == 1 { "" } else { "s" })
    } else if seconds >= 60 {
        let minutes = seconds / 60;
        format!("{minutes} minute{}", if minutes == 1 { "" } else { "s" })
    } else {
        format!("{seconds} second{}", if seconds == 1 { "" } else { "s" })
    }
}

/// What a baseline says about one program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Seen while learning. Ordinary for this machine.
    Known,
    /// Not in the baseline, but covered by a grant that has not expired.
    Granted,
    /// Not in the baseline and not granted.
    ///
    /// **Not "blocked".** Nothing was stopped. The program is running; this is
    /// a statement about the baseline, not about the machine.
    Unknown,
    /// Still learning, so there is nothing to compare against yet.
    ///
    /// Deliberately its own answer rather than [`Verdict::Known`]: during
    /// learning everything would be "known", which is true and useless, and a
    /// front end must be able to say "still learning" instead of implying the
    /// machine has been checked.
    Learning,
}

impl Verdict {
    /// The wording a front end should use.
    pub fn phrasing(self) -> &'static str {
        match self {
            Self::Known => "was running while the baseline was learned",
            Self::Granted => "is not in the baseline, and you allowed it",
            Self::Unknown => {
                "is not in the baseline and has not been allowed -- it is still \
                 running, because nothing here stops anything"
            }
            Self::Learning => "was recorded; the baseline is still learning",
        }
    }
}

/// One line of the decision log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// When it was decided, as seconds since the Unix epoch.
    pub at: u64,
    /// The process name.
    pub program: String,
    /// What was decided.
    pub verdict: Verdict,
    /// How the covering grant reads, when there was one.
    pub grant: Option<String>,
}

/// What normally runs here, what has been allowed, and what has been decided.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Baseline {
    /// Programs seen while learning.
    known: BTreeMap<String, ()>,
    /// Programs allowed since, and until when.
    grants: BTreeMap<String, Grant>,
    /// Every decision, oldest first.
    log: Vec<Entry>,
    /// Whether the learning phase is still open.
    learning: bool,
}

impl Baseline {
    /// A baseline that has learned nothing and is not learning.
    pub fn new() -> Self {
        Self::default()
    }

    /// A baseline in its learning phase.
    pub fn learning() -> Self {
        Self {
            learning: true,
            ..Self::default()
        }
    }

    /// Whether the learning phase is open.
    pub fn is_learning(&self) -> bool {
        self.learning
    }

    /// Close the learning phase.
    ///
    /// Refuses to close on an empty baseline. A baseline that learned nothing
    /// calls everything unknown, which is the same as calling nothing unknown:
    /// the reader gets a page of noise, learns to ignore it, and is worse off
    /// than before they started.
    pub fn freeze(&mut self) -> Result<usize, Error> {
        if !self.learning {
            return Err(Error::NotLearning);
        }
        if self.known.is_empty() {
            return Err(Error::LearnedNothing);
        }
        self.learning = false;
        Ok(self.known.len())
    }

    /// How many programs the baseline holds.
    pub fn len(&self) -> usize {
        self.known.len()
    }

    /// Whether the baseline holds nothing.
    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    /// Every program in the baseline, in order.
    pub fn programs(&self) -> impl Iterator<Item = &str> {
        self.known.keys().map(String::as_str)
    }

    /// Allow a program until a moment, or for good.
    ///
    /// A grant for something already in the baseline is refused: it would be a
    /// row in the allowlist that never does anything, and later reads as
    /// though somebody had to permit an ordinary program.
    pub fn allow(&mut self, program: &str, grant: Grant) -> Result<(), Error> {
        let name = normalise(program);
        if name.is_empty() {
            return Err(Error::NoProgram);
        }
        if self.known.contains_key(&name) {
            return Err(Error::AlreadyKnown(name));
        }
        self.grants.insert(name, grant);
        Ok(())
    }

    /// Withdraw a grant.
    pub fn revoke(&mut self, program: &str) {
        self.grants.remove(&normalise(program));
    }

    /// The grant covering a program, expired or not.
    pub fn grant(&self, program: &str) -> Option<Grant> {
        self.grants.get(&normalise(program)).copied()
    }

    /// Whether this program is allowed to be running, as of `now`.
    pub fn allowed(&self, program: &str, now: SystemTime) -> bool {
        matches!(
            self.verdict(program, now),
            Verdict::Known | Verdict::Granted | Verdict::Learning
        )
    }

    /// What this baseline says about a program, without recording anything.
    pub fn verdict(&self, program: &str, now: SystemTime) -> Verdict {
        let name = normalise(program);
        if self.learning {
            return Verdict::Learning;
        }
        if self.known.contains_key(&name) {
            return Verdict::Known;
        }
        match self.grants.get(&name) {
            // An expired grant is not a grant. It is left in place on purpose:
            // "this was allowed until Tuesday" is worth more to a reader than
            // a row that quietly disappeared.
            Some(grant) if grant.covers(now) => Verdict::Granted,
            _ => Verdict::Unknown,
        }
    }

    /// Record what is running now, and return what was decided.
    ///
    /// While learning, everything seen joins the baseline. Afterwards, nothing
    /// does -- which is the whole point of the phase having an end.
    pub fn observe(&mut self, running: &[String], now: SystemTime) -> Vec<Entry> {
        let at = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut decided = Vec::new();
        for program in running {
            let name = normalise(program);
            if name.is_empty() {
                continue;
            }
            let verdict = self.verdict(&name, now);
            if self.learning {
                self.known.insert(name.clone(), ());
            }
            let entry = Entry {
                at,
                program: name.clone(),
                verdict,
                grant: self.grants.get(&name).map(|g| g.describe(now)),
            };
            // Only the interesting decisions are logged. Writing a line for
            // every ordinary program every time it is seen produces a log
            // nobody reads, and a log nobody reads is not a control.
            if matches!(verdict, Verdict::Unknown | Verdict::Granted) {
                self.log.push(entry.clone());
            }
            decided.push(entry);
        }
        decided
    }

    /// The decision log, oldest first.
    pub fn log(&self) -> &[Entry] {
        &self.log
    }

    /// Everything running that is neither known nor granted.
    pub fn unknown(&self, running: &[String], now: SystemTime) -> Vec<String> {
        let mut out: Vec<String> = running
            .iter()
            .map(|p| normalise(p))
            .filter(|name| !name.is_empty() && self.verdict(name, now) == Verdict::Unknown)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Write the baseline as text.
    ///
    /// Plain text on purpose, like the project format: it can be read, edited
    /// and diffed, and a control whose state is opaque is a control nobody can
    /// audit. The log is included, because a decision record kept separately
    /// from the decisions is one that can be lost separately.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        out.push_str(
            "# What normally runs on this machine. This does NOT block anything;\n\
             # see the scope note in the application. Editing this file changes\n\
             # what is treated as ordinary, so treat it as a security setting.\n",
        );
        out.push_str(&format!("learning  {}\n", self.learning));
        for program in self.known.keys() {
            out.push_str(&format!("known  {program}\n"));
        }
        for (program, grant) in &self.grants {
            match grant {
                Grant::Forever => out.push_str(&format!("grant  {program}  forever\n")),
                Grant::Until(end) => {
                    let seconds = end
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    out.push_str(&format!("grant  {program}  {seconds}\n"));
                }
            }
        }
        for entry in &self.log {
            out.push_str(&format!(
                "log  {}  {}  {}\n",
                entry.at,
                verdict_key(entry.verdict),
                entry.program
            ));
        }
        out
    }

    /// Read a baseline back.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next().map(str::trim) {
            Some(first) if first == MAGIC => {}
            _ => return Err(Error::NotABaseline),
        }
        let mut baseline = Baseline::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (keyword, rest) = take_token(line);
            let rest = rest.trim();
            match keyword {
                "learning" => baseline.learning = rest == "true",
                "known" => {
                    baseline.known.insert(normalise(rest), ());
                }
                "grant" => {
                    let (program, when) = take_token(rest);
                    let when = when.trim();
                    let grant = if when == "forever" {
                        Grant::Forever
                    } else {
                        let seconds: u64 = when
                            .parse()
                            .map_err(|_| Error::Malformed(line.to_string()))?;
                        Grant::Until(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
                    };
                    baseline.grants.insert(normalise(program), grant);
                }
                "log" => {
                    let (at, rest) = take_token(rest);
                    let (verdict, program) = take_token(rest.trim());
                    baseline.log.push(Entry {
                        at: at.parse().map_err(|_| Error::Malformed(line.to_string()))?,
                        verdict: verdict_from_key(verdict)
                            .ok_or_else(|| Error::Malformed(line.to_string()))?,
                        program: normalise(program.trim()),
                        grant: None,
                    });
                }
                other => return Err(Error::UnknownKeyword(other.to_string())),
            }
        }
        Ok(baseline)
    }
}

/// The first whitespace-separated token, and the rest.
fn take_token(line: &str) -> (&str, &str) {
    match line.find(char::is_whitespace) {
        Some(at) => (&line[..at], &line[at..]),
        None => (line, ""),
    }
}

/// A process name as this crate compares them.
fn normalise(program: &str) -> String {
    program.trim().to_ascii_lowercase()
}

/// The word written to the file for a verdict.
fn verdict_key(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Known => "known",
        Verdict::Granted => "granted",
        Verdict::Unknown => "unknown",
        Verdict::Learning => "learning",
    }
}

/// The verdict a word means.
fn verdict_from_key(key: &str) -> Option<Verdict> {
    Some(match key {
        "known" => Verdict::Known,
        "granted" => Verdict::Granted,
        "unknown" => Verdict::Unknown,
        "learning" => Verdict::Learning,
        _ => return None,
    })
}

/// Why something was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The file does not begin with this format's first line.
    NotABaseline,
    /// A line could not be read.
    Malformed(String),
    /// A keyword this build does not know.
    ///
    /// Refused rather than skipped: a line this build ignores is a security
    /// setting somebody wrote and this program did not apply.
    UnknownKeyword(String),
    /// A grant was asked for with no program named.
    NoProgram,
    /// The program is already in the baseline.
    AlreadyKnown(String),
    /// The learning phase was already closed.
    NotLearning,
    /// Nothing was seen while learning.
    LearnedNothing,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotABaseline => write!(f, "that file does not start with {MAGIC}"),
            Self::Malformed(line) => write!(f, "this line could not be read: {line}"),
            Self::UnknownKeyword(word) => write!(
                f,
                "\"{word}\" is not something this version understands. Refused rather \
                 than skipped: a line this build ignores is a setting you wrote and it \
                 did not apply"
            ),
            Self::NoProgram => write!(f, "no program was named"),
            Self::AlreadyKnown(program) => write!(
                f,
                "{program} is already in the baseline, so allowing it would add a rule \
                 that never does anything"
            ),
            Self::NotLearning => write!(f, "the baseline is not learning"),
            Self::LearnedNothing => write!(
                f,
                "nothing was seen while learning. A baseline that learned nothing calls \
                 everything unknown, which is the same as calling nothing unknown"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// What a reader must be told, in the words to tell them.
pub const SCOPE: &str = "\
This does not block anything and cannot. It learns which programs normally run \
on this machine and afterwards tells you when something runs that was not in \
that picture -- a way of noticing, not a lock on the door. A program it calls \
unknown is still running. Real enforcement needs a kernel driver or a signed \
system policy, and neither is something this project ships. It also only sees \
programs running as you, so an empty report is not proof that nothing is there.";

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// **The most important test here.** Nothing this crate says may suggest it
    /// stopped anything, because it did not and cannot.
    #[test]
    fn nothing_this_crate_says_claims_to_have_blocked_anything() {
        let mut sentences = vec![SCOPE.to_string()];
        for verdict in [
            Verdict::Known,
            Verdict::Granted,
            Verdict::Unknown,
            Verdict::Learning,
        ] {
            sentences.push(verdict.phrasing().to_string());
        }
        for sentence in &sentences {
            let lower = sentence.to_lowercase();
            for claim in ["was blocked", "prevented it", "stopped it", "denied access"] {
                assert!(!lower.contains(claim), "\"{claim}\" in:\n{sentence}");
            }
        }
        // And the unknown verdict has to say the program is still running,
        // because that is the thing a reader will otherwise assume.
        assert!(Verdict::Unknown.phrasing().contains("still"));
        assert!(Verdict::Unknown.phrasing().contains("nothing here stops"));

        let scope = SCOPE.to_lowercase();
        assert!(
            scope.contains("does not block anything and cannot"),
            "{scope}"
        );
        assert!(scope.contains("still running"), "{scope}");
        assert!(scope.contains("not proof that nothing is there"), "{scope}");
    }

    /// A grant that has run out is not a grant, and the check is against the
    /// clock rather than against a sweep that may not have run.
    #[test]
    fn a_grant_stops_covering_the_moment_it_expires() {
        let mut base = Baseline::learning();
        base.observe(&names(&["explorer.exe"]), at(0));
        base.freeze().unwrap();

        base.allow(
            "thing.exe",
            Grant::for_duration(at(100), Duration::from_secs(60)).unwrap(),
        )
        .unwrap();

        assert_eq!(base.verdict("thing.exe", at(120)), Verdict::Granted);
        assert_eq!(base.verdict("thing.exe", at(159)), Verdict::Granted);
        // Exactly at the boundary it is over: `now < end`, not `<=`.
        assert_eq!(base.verdict("thing.exe", at(160)), Verdict::Unknown);
        assert_eq!(base.verdict("thing.exe", at(9_999)), Verdict::Unknown);

        // And the expired grant is still on record, because "allowed until"
        // is worth more to a reader than a row that vanished.
        assert!(base.grant("thing.exe").is_some());
        assert_eq!(
            base.grant("thing.exe").unwrap().describe(at(9_999)),
            "expired"
        );
    }

    /// Permanent is spelled out, never a very large date.
    #[test]
    fn forever_is_its_own_thing_and_not_a_distant_timestamp() {
        assert_eq!(Grant::forever(), Grant::Forever);
        assert!(Grant::Forever.covers(at(0)));
        assert!(Grant::Forever.covers(at(u32::MAX as u64)));
        assert_eq!(Grant::Forever.describe(at(0)), "for ever");

        // A duration that runs off the clock is refused rather than saturating
        // into an accidental permanent grant.
        assert_eq!(Grant::for_duration(at(0), Duration::MAX), None);
    }

    /// Learning has an end, and a baseline that learned nothing may not be
    /// frozen -- it would call everything unknown, which is noise.
    #[test]
    fn learning_must_end_and_must_have_learned_something() {
        let mut empty = Baseline::learning();
        assert_eq!(empty.freeze(), Err(Error::LearnedNothing));
        assert!(empty.is_learning(), "a refused freeze changes nothing");

        let mut base = Baseline::learning();
        base.observe(&names(&["a.exe", "b.exe"]), at(0));
        assert_eq!(base.freeze(), Ok(2));
        assert!(!base.is_learning());
        assert_eq!(base.freeze(), Err(Error::NotLearning));
    }

    /// After learning ends, nothing joins the baseline by running. That is the
    /// entire reason the phase has an end.
    #[test]
    fn a_frozen_baseline_does_not_learn_from_what_runs_next() {
        let mut base = Baseline::learning();
        base.observe(&names(&["known.exe"]), at(0));
        base.freeze().unwrap();
        assert_eq!(base.len(), 1);

        base.observe(&names(&["stranger.exe"]), at(10));
        assert_eq!(
            base.len(),
            1,
            "a stranger must not become ordinary by running"
        );
        assert_eq!(base.verdict("stranger.exe", at(10)), Verdict::Unknown);
    }

    /// While learning, the verdict says so rather than saying everything is
    /// fine -- which is true and useless and reads as a clean bill of health.
    #[test]
    fn while_learning_the_answer_is_learning_and_not_known() {
        let mut base = Baseline::learning();
        let decided = base.observe(&names(&["anything.exe"]), at(0));
        assert_eq!(decided[0].verdict, Verdict::Learning);
        assert_ne!(decided[0].verdict, Verdict::Known);
        assert!(Verdict::Learning.phrasing().contains("still learning"));
    }

    /// Only the decisions worth reading are logged. A line for every ordinary
    /// program every time it is seen is a log nobody reads.
    #[test]
    fn the_log_records_what_is_worth_reading_and_not_every_sighting() {
        let mut base = Baseline::learning();
        base.observe(&names(&["ordinary.exe"]), at(0));
        base.freeze().unwrap();

        for _ in 0..5 {
            base.observe(&names(&["ordinary.exe"]), at(10));
        }
        assert!(base.log().is_empty(), "{:?}", base.log());

        base.observe(&names(&["stranger.exe"]), at(20));
        assert_eq!(base.log().len(), 1);
        assert_eq!(base.log()[0].verdict, Verdict::Unknown);
        assert_eq!(base.log()[0].program, "stranger.exe");
        assert_eq!(base.log()[0].at, 20);
    }

    /// Allowing something already ordinary is refused: the rule would never do
    /// anything, and later reads as though it had been needed.
    #[test]
    fn a_grant_for_something_already_in_the_baseline_is_refused() {
        let mut base = Baseline::learning();
        base.observe(&names(&["ordinary.exe"]), at(0));
        base.freeze().unwrap();

        let refused = base.allow("ordinary.exe", Grant::Forever);
        assert_eq!(refused, Err(Error::AlreadyKnown("ordinary.exe".into())));
        assert!(base.grant("ordinary.exe").is_none());

        assert_eq!(base.allow("   ", Grant::Forever), Err(Error::NoProgram));
    }

    /// Names compare the same however they are written.
    #[test]
    fn a_name_is_matched_however_it_is_typed() {
        let mut base = Baseline::learning();
        base.observe(&names(&["Explorer.EXE"]), at(0));
        base.freeze().unwrap();
        for spelling in ["explorer.exe", "EXPLORER.EXE", "  Explorer.exe  "] {
            assert_eq!(base.verdict(spelling, at(0)), Verdict::Known, "{spelling}");
        }
    }

    /// Every shape a baseline can be in survives being written and read back.
    #[test]
    fn every_shape_of_baseline_round_trips() {
        let mut shapes = vec![Baseline::new(), Baseline::learning()];

        let mut full = Baseline::learning();
        full.observe(&names(&["a.exe", "b.exe", "c.exe"]), at(1_000));
        full.freeze().unwrap();
        full.allow("temp.exe", Grant::Until(at(5_000))).unwrap();
        full.allow("always.exe", Grant::Forever).unwrap();
        full.observe(&names(&["stranger.exe", "temp.exe"]), at(2_000));
        shapes.push(full);

        let mut granted_only = Baseline::new();
        granted_only.allow("x.exe", Grant::Forever).unwrap();
        shapes.push(granted_only);

        for shape in shapes {
            let text = shape.to_text();
            let back = Baseline::parse(&text).unwrap_or_else(|e| panic!("{e}\n---\n{text}"));
            let again = Baseline::parse(&back.to_text()).expect("stable");
            assert_eq!(back, again, "reading twice must agree:\n{text}");
            assert_eq!(back.learning, shape.learning);
            assert_eq!(back.known, shape.known);
            assert_eq!(back.grants, shape.grants);
            assert_eq!(back.log.len(), shape.log.len());
        }
    }

    /// A keyword this build does not know is **refused**, never skipped: a line
    /// silently ignored is a security setting somebody wrote and this program
    /// did not apply.
    #[test]
    fn an_unreadable_line_is_refused_rather_than_skipped() {
        let text = format!("{MAGIC}\nknown  a.exe\nblockall  yes\n");
        let refused = Baseline::parse(&text).unwrap_err();
        assert_eq!(refused, Error::UnknownKeyword("blockall".into()));
        assert!(refused.to_string().contains("did not apply"));

        assert_eq!(Baseline::parse("nonsense\n"), Err(Error::NotABaseline));
        assert!(Baseline::parse(&format!("{MAGIC}\ngrant  x.exe  soon\n")).is_err());
    }

    /// The file holds no secrets, and says what it is.
    #[test]
    fn the_file_explains_itself_and_carries_no_credentials() {
        let mut base = Baseline::learning();
        base.observe(&names(&["a.exe"]), at(0));
        base.freeze().unwrap();
        let text = base.to_text();
        assert!(text.starts_with(MAGIC));
        assert!(text.contains("does NOT block anything"));
        let data: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect();
        for line in data {
            let lower = line.to_lowercase();
            for secret in ["password", "passphrase", "key ", "token"] {
                assert!(!lower.contains(secret), "{line}");
            }
        }
    }

    #[test]
    fn what_is_unknown_is_listed_once_and_in_order() {
        let mut base = Baseline::learning();
        base.observe(&names(&["ok.exe"]), at(0));
        base.freeze().unwrap();
        base.allow("allowed.exe", Grant::Forever).unwrap();

        let running = names(&["ok.exe", "zeta.exe", "allowed.exe", "alpha.exe", "zeta.exe"]);
        assert_eq!(base.unknown(&running, at(1)), vec!["alpha.exe", "zeta.exe"]);
    }

    #[test]
    fn durations_read_as_english() {
        assert_eq!(plain_duration(Duration::from_secs(1)), "1 second");
        assert_eq!(plain_duration(Duration::from_secs(90)), "1 minute");
        assert_eq!(plain_duration(Duration::from_secs(7_200)), "2 hours");
        assert_eq!(plain_duration(Duration::from_secs(172_800)), "2 days");
    }
}
