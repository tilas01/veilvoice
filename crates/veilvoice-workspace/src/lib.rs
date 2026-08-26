// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Named profiles and saved projects.
//!
//! Two things, which sound alike and are not:
//!
//! * A [`Profile`] is a **way of working** — a named set of settings you can
//!   switch to. "Anonymise one person, as thoroughly as this can." "A group,
//!   everybody the same voice." Three are built in and you can save your own.
//! * A [`Workspace`] is **one piece of work** — which recording, which plan,
//!   who is in it and what they are called, and the profile it was done under.
//!   Saved beside the recording, so opening it a week later puts everything
//!   back where it was.
//!
//! # Why a project file is worth having here in particular
//!
//! Setting up a group recording is a dozen small decisions: who is in it, what
//! each is called, what colour each is drawn in, whether they share a voice.
//! Getting halfway through and having to stop is ordinary. Losing all of it
//! because the window was closed is not, and it is worse than usual here,
//! because the *recording* may be the only copy of something that cannot be
//! made again.
//!
//! # Plain text, and the same shape as a plan
//!
//! `VEILWORK1`, then one `key  value` per line. The same format
//! `veilvoice-conversation` uses for a plan, for the same reasons: it is
//! readable, diffable, greppable, and has no syntax in which something
//! surprising can hide. A project file is a description of your work; it should
//! not require this program to read it.
//!
//! # What a project file does **not** contain
//!
//! **No audio and no passwords.** It records the *path* to a recording, not the
//! recording, and nothing about how anything is encrypted. A workspace is a
//! thing you might send somebody so they can set their machine up the same way;
//! if it carried a passphrase, sending it would be handing over the recordings
//! too, and people would find that out afterwards.
//!
//! Speaker names *are* in it, because the whole point is to put them back — and
//! a name is a name. The file says so.
//!
//! # In plain words
//!
//! This is the "save my project" and "load my setup" part.
//!
//! Profiles are named ways of working you can flip between: one for anonymising
//! a single person as thoroughly as possible, one for a group where everybody
//! keeps their own disguised voice, one for a group where everybody sounds
//! identical and only the names tell you who is speaking.
//!
//! A project file remembers the rest: which recording you were working on, who
//! is in it, what you called them and what colour each one is. Open it next
//! week and everything is where you left it.
//!
//! It does not contain the recording itself and it does not contain any
//! password. It does contain the names you typed, because putting those back is
//! the point of it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veilvoice_conversation::mode::VoiceMode;
use veilvoice_core::DeidConfig;

/// The first line of a project file.
const MAGIC: &str = "VEILWORK1";

/// Something that could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The file could not be opened or written.
    Io(String),
    /// The text is not a project file, or is one this build cannot read.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(what) | Self::Malformed(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for Error {}

/// A named way of working.
///
/// The settings, and — as importantly — a plain sentence about what choosing it
/// actually means. A profile called "highest security" that does not say what
/// it does and does not do is a name doing the work of an explanation.
// No `Eq`: two of these fields are `f32`, and floats have no total equality.
// `PartialEq` is what comparing two profiles actually needs and is honest about
// what it is doing.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Stable identifier, written into a project file.
    pub id: &'static str,
    /// What it is called in a picker.
    pub name: &'static str,
    /// What choosing it means, in one paragraph, including the limits.
    pub note: &'static str,
    /// How far the transform pushes, 0..1.
    pub intensity: f32,
    /// Whether the accent neutraliser runs.
    pub neutralise_accent: bool,
    /// Seconds between modulation seed rolls.
    pub reseed_secs: f32,
    /// Whether results are sealed at rest.
    pub encrypt_at_rest: bool,
    /// Whether metadata is stripped from what is written.
    pub clean_metadata: bool,
    /// Whether this profile is a group setup at all.
    pub group: bool,
    /// If it is, whether everybody shares a voice.
    pub voices: VoiceMode,
}

impl Profile {
    /// The engine settings this profile asks for, over a starting point.
    ///
    /// The sample rate is left alone: it belongs to the recording, not to a
    /// way of working, and a profile that overwrote it would resample somebody's
    /// audio because they picked a preset.
    pub fn applied_to(&self, mut config: DeidConfig) -> DeidConfig {
        config.intensity = self.intensity;
        config.accent.enabled = self.neutralise_accent;
        config.reseed_secs = self.reseed_secs;
        config
    }
}

/// Anonymise one person, with everything this engine has turned on.
pub const INDIVIDUAL: Profile = Profile {
    id: "individual",
    name: "One person",
    note: "One speaker, veiled as thoroughly as this engine does it: the accent \
           neutraliser on, the transform at full strength, the seed rolling every \
           two seconds, the result sealed at rest and its metadata stripped. This \
           is the default and it is what most recordings want. What it does not do \
           is hide *what was said* -- the words are kept on purpose.",
    intensity: 1.0,
    neutralise_accent: true,
    reseed_secs: 2.0,
    encrypt_at_rest: true,
    clean_metadata: true,
    group: false,
    voices: VoiceMode::Distinct,
};

/// A group, each person with their own disguised voice.
pub const GROUP_VOICES: Profile = Profile {
    id: "group-voices",
    name: "A group, a voice each",
    note: "Several people in one recording, each given a different destination \
           voice so a listener can follow it by ear. Capped at the number of \
           voices far enough apart to actually be told apart, which is measured \
           rather than chosen. Every voiceprint is destroyed just as thoroughly \
           as one speaker's would be.",
    intensity: 1.0,
    neutralise_accent: true,
    reseed_secs: 2.0,
    encrypt_at_rest: true,
    clean_metadata: true,
    group: true,
    voices: VoiceMode::Distinct,
};

/// A group where nobody can be picked out by sound at all.
pub const GROUP_ONE_VOICE: Profile = Profile {
    id: "group-one-voice",
    name: "A group, one voice for everybody",
    note: "Several people, all rendered as the same voice, told apart only by \
           their names in the subtitles and by which circle lights up in the \
           picture. This is the most private of the three: the output carries no \
           trace of *which* speaker somebody was, so two recordings of the same \
           group cannot be lined up by voice. The price is that nobody can \
           follow it by ear, and it is a real price -- an audio-only listener \
           cannot tell who is speaking at all. It has no speaker limit, because \
           one voice cannot collide with itself.",
    intensity: 1.0,
    neutralise_accent: true,
    reseed_secs: 2.0,
    encrypt_at_rest: true,
    clean_metadata: true,
    group: true,
    voices: VoiceMode::Uniform,
};

/// The profiles that ship, in the order a picker shows them.
pub const BUILT_IN: &[Profile] = &[INDIVIDUAL, GROUP_VOICES, GROUP_ONE_VOICE];

/// The profile with this identifier.
///
/// `None` rather than a fallback: a project file naming a profile this build
/// does not have should say so, not open in a different one and let somebody
/// render under settings they did not choose.
pub fn profile(id: &str) -> Option<&'static Profile> {
    BUILT_IN.iter().find(|p| p.id == id)
}

/// The profile a fresh install starts in.
pub fn default_profile() -> &'static Profile {
    &BUILT_IN[0]
}

/// One person in a saved project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Member {
    /// What they are called.
    pub name: String,
    /// A colour chosen by hand as `#rrggbb`, or `None` for their slot's.
    pub colour: Option<String>,
}

/// One piece of work, saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
    /// A name for the project, for a window title or a list.
    pub title: Option<String>,
    /// The recording being worked on.
    pub input: Option<PathBuf>,
    /// The plan saying who speaks when.
    pub plan: Option<PathBuf>,
    /// Which profile this was set up under.
    pub profile: String,
    /// The people, in slot order.
    pub members: Vec<Member>,
    /// The palette a rendered page is drawn in.
    pub theme: String,
    /// What a render writes: any of `audio`, `subtitles`, `page`.
    pub outputs: Vec<String>,
}

impl Workspace {
    /// A new, empty project under the default profile.
    pub fn new() -> Self {
        Self {
            profile: default_profile().id.to_string(),
            theme: "tokyo-night".to_string(),
            outputs: vec![
                "audio".to_string(),
                "subtitles".to_string(),
                "page".to_string(),
            ],
            ..Self::default()
        }
    }

    /// Serialise to the text format.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        out.push_str(
            "# A VeilVoice project. Plain text on purpose: read it, edit it, or\n\
             # delete it. It holds no audio and no passwords -- only where things\n\
             # are and how they were set up. It does hold the speaker names you\n\
             # typed, because putting those back is the point of it.\n",
        );
        if let Some(title) = &self.title {
            out.push_str(&format!("title  {}\n", one_line(title)));
        }
        if let Some(input) = &self.input {
            out.push_str(&format!("input  {}\n", path_text(input)));
        }
        if let Some(plan) = &self.plan {
            out.push_str(&format!("plan  {}\n", path_text(plan)));
        }
        out.push_str(&format!("profile  {}\n", one_line(&self.profile)));
        out.push_str(&format!("theme  {}\n", one_line(&self.theme)));
        for output in &self.outputs {
            out.push_str(&format!("output  {}\n", one_line(output)));
        }
        for (index, member) in self.members.iter().enumerate() {
            match &member.colour {
                Some(colour) => out.push_str(&format!(
                    "member  {index}  {}  {}\n",
                    one_line(colour),
                    one_line(&member.name)
                )),
                None => out.push_str(&format!("member  {index}  -  {}\n", one_line(&member.name))),
            }
        }
        out
    }

    /// Parse the text format.
    ///
    /// An unknown keyword is **refused**, not skipped. A project file written by
    /// a newer build may describe a setup this one cannot reproduce, and quietly
    /// honouring the half it understands would put somebody's recording through
    /// settings they did not choose — which is the same reasoning
    /// `veilvoice-conversation` gives for refusing an unknown line in a plan.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next().map(str::trim) {
            Some(first) if first == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "this does not start with {MAGIC}, so it is not a VeilVoice project \
                     file (it starts {other:?})"
                )))
            }
            None => return Err(Error::Malformed("the file is empty".into())),
        }

        let mut work = Self::new();
        work.outputs.clear();
        let mut members: BTreeMap<usize, Member> = BTreeMap::new();

        for (number, line) in lines.enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let keyword = parts.next().unwrap_or("");
            let rest = line[keyword.len()..].trim();
            let at = number + 2;
            match keyword {
                "title" => work.title = Some(rest.to_string()),
                "input" => work.input = Some(PathBuf::from(rest)),
                "plan" => work.plan = Some(PathBuf::from(rest)),
                "profile" => work.profile = rest.to_string(),
                "theme" => work.theme = rest.to_string(),
                "output" => work.outputs.push(rest.to_string()),
                "member" => {
                    // Taken one token at a time rather than `splitn`, because
                    // the format separates fields with *two* spaces and
                    // `splitn(3, char::is_whitespace)` splits on the first of
                    // them -- handing back an empty colour and a name of
                    // "-  Alex". A round-trip test caught it; reading the line
                    // would not have.
                    let (index_text, rest) = take_token(rest);
                    let (colour, name) = take_token(rest);
                    let index: usize = index_text
                        .parse()
                        .map_err(|_| Error::Malformed(format!("line {at}: no slot number")))?;
                    let colour = colour.to_string();
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        return Err(Error::Malformed(format!(
                            "line {at}: a member needs a name"
                        )));
                    }
                    if members.contains_key(&index) {
                        return Err(Error::Malformed(format!(
                            "line {at}: slot {index} is listed twice"
                        )));
                    }
                    members.insert(
                        index,
                        Member {
                            name,
                            colour: (colour != "-").then_some(colour),
                        },
                    );
                }
                other => {
                    return Err(Error::Malformed(format!(
                        "line {at}: {other:?} is not something this build understands. A \
                         project file written by a newer VeilVoice may describe a setup \
                         this one cannot reproduce, and honouring half of it would render \
                         under settings you did not choose."
                    )))
                }
            }
        }

        // Slots have to be 0, 1, 2, ... with nothing missing: the index *is* the
        // voice slot, so a gap would move everybody after it onto a different
        // voice from the one they were saved with.
        for (expected, (index, member)) in members.into_iter().enumerate() {
            if index != expected {
                return Err(Error::Malformed(format!(
                    "slot {index} appears where slot {expected} should be. Slots are the \
                     voice each person is given, so a gap would move everybody after it \
                     onto a different voice."
                )));
            }
            work.members.push(member);
        }

        Ok(work)
    }

    /// Read a project file.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Io(format!("cannot read {}: {e}", path.display())))?;
        Self::parse(&text)
    }

    /// Write a project file, creating the directory if needed.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Error::Io(format!("cannot create {}: {e}", parent.display())))?;
            }
        }
        std::fs::write(path, self.to_text())
            .map_err(|e| Error::Io(format!("cannot write {}: {e}", path.display())))
    }

    /// The profile this project names, if this build has it.
    pub fn profile(&self) -> Option<&'static Profile> {
        profile(&self.profile)
    }
}

/// The first whitespace-delimited token, and everything after it.
///
/// The remainder keeps its own internal spacing, because the last field on a
/// line is a name and a name may contain spaces.
fn take_token(line: &str) -> (&str, &str) {
    let line = line.trim_start();
    match line.find(char::is_whitespace) {
        Some(at) => (&line[..at], line[at..].trim_start()),
        None => (line, ""),
    }
}

/// A value with no line break in it.
///
/// A newline inside a name would let one line become two, and the second could
/// claim to be any keyword it liked — the same forging a plan's speaker names
/// are already guarded against.
fn one_line(value: &str) -> String {
    value.replace(['\n', '\r'], " ").trim().to_string()
}

/// A path as one line, with Windows separators normalised.
fn path_text(path: &Path) -> String {
    one_line(&path.display().to_string().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Workspace {
        let mut work = Workspace::new();
        work.title = Some("Two people, one afternoon".into());
        work.input = Some(PathBuf::from("recordings/talk.wav"));
        work.plan = Some(PathBuf::from("recordings/plan.txt"));
        work.profile = GROUP_ONE_VOICE.id.to_string();
        work.theme = "gruvbox".into();
        work.members = vec![
            Member {
                name: "Alex".into(),
                colour: None,
            },
            Member {
                name: "Sam".into(),
                colour: Some("#73daca".into()),
            },
        ];
        work
    }

    #[test]
    fn a_project_round_trips_through_its_text_format() {
        let work = sample();
        let read_back = Workspace::parse(&work.to_text()).expect("should parse");
        assert_eq!(read_back, work);
    }

    #[test]
    fn a_new_project_writes_all_three_outputs() {
        let work = Workspace::new();
        assert_eq!(work.outputs, vec!["audio", "subtitles", "page"]);
        assert_eq!(work.profile, INDIVIDUAL.id);
        let read_back = Workspace::parse(&work.to_text()).unwrap();
        assert_eq!(read_back.outputs, work.outputs);
    }

    #[test]
    fn it_saves_and_loads_through_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("project.veilwork");
        let work = sample();
        work.save(&path).unwrap();
        assert_eq!(Workspace::load(&path).unwrap(), work);
    }

    /// **No passwords and no audio.** A project file is a thing somebody might
    /// send; if it carried a passphrase, sending it would hand over the
    /// recordings too, and people would learn that afterwards.
    #[test]
    fn a_project_file_carries_no_secret_and_no_audio() {
        let text = sample().to_text();

        // The **data**, not the header comment. The first version of this
        // scanned the whole file and tripped on the file's own denial -- the
        // header says "no audio and no passwords", which contains "password".
        // `docs/AUDIT.md` records the identical trap in a scope note, where a
        // search for "prevents" matched "nothing here prevents it". A checker
        // has to read the way the thing it checks is read.
        let data: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();

        for forbidden in ["password", "passphrase", "secret", "riff", "wave"] {
            assert!(
                !data.contains(forbidden),
                "a project file must not contain {forbidden:?}:\n{data}"
            );
        }
        // And the header does have to say what is not in here, in the words
        // somebody opening the file would read.
        assert!(text.to_lowercase().contains("no audio and no passwords"));
    }

    /// An unknown keyword is refused rather than skipped, and the refusal says
    /// why that is the safe answer.
    #[test]
    fn an_unknown_keyword_is_refused_rather_than_ignored() {
        let text = format!("{MAGIC}\nprofile  individual\nsomething_new  42\n");
        let error = Workspace::parse(&text).expect_err("unknown keywords are refused");
        let words = error.to_string();
        assert!(words.contains("something_new"), "{words}");
        assert!(words.contains("settings you did not choose"), "{words}");
    }

    #[test]
    fn a_file_that_is_not_a_project_is_refused_by_its_first_line() {
        let error =
            Workspace::parse("VEILCONV1\nspeaker  0  Alex\n").expect_err("a plan, not a project");
        assert!(error.to_string().contains(MAGIC));
        assert!(Workspace::parse("").is_err());
    }

    /// A gap in the slots would move everybody after it onto a different voice
    /// from the one they were saved with -- silently, and only audible as
    /// "somebody sounds wrong".
    #[test]
    fn a_gap_in_the_slots_is_refused() {
        let text = format!("{MAGIC}\nmember  0  -  Alex\nmember  2  -  Sam\n");
        let error = Workspace::parse(&text).expect_err("slot 1 is missing");
        assert!(error.to_string().contains("different voice"), "{error}");
    }

    #[test]
    fn a_repeated_slot_is_refused() {
        let text = format!("{MAGIC}\nmember  0  -  Alex\nmember  0  -  Sam\n");
        assert!(Workspace::parse(&text).is_err());
    }

    #[test]
    fn a_member_with_no_name_is_refused() {
        let text = format!("{MAGIC}\nmember  0  -  \n");
        assert!(Workspace::parse(&text).is_err());
    }

    /// A name with a line break in it could forge a record. The writer strips
    /// it rather than producing a file that reads back as something else.
    #[test]
    fn a_name_cannot_forge_a_second_line() {
        let mut work = Workspace::new();
        work.members = vec![Member {
            name: "Alex\nprofile  group-one-voice".into(),
            colour: None,
        }];
        let text = work.to_text();
        let read_back = Workspace::parse(&text).expect("should still parse");
        assert_eq!(
            read_back.profile, INDIVIDUAL.id,
            "the forged line must not win"
        );
        assert!(!read_back.members[0].name.contains('\n'));
    }

    /// A profile this build does not have is reported, not silently swapped for
    /// one it does. Rendering under settings somebody did not choose is the
    /// failure to avoid.
    #[test]
    fn an_unknown_profile_is_reported_rather_than_replaced() {
        let mut work = Workspace::new();
        work.profile = "from-a-newer-build".into();
        assert!(work.profile().is_none());
        let read_back = Workspace::parse(&work.to_text()).unwrap();
        assert_eq!(read_back.profile, "from-a-newer-build");
        assert!(read_back.profile().is_none());
    }

    #[test]
    fn every_built_in_profile_is_findable_and_uniquely_named() {
        let mut ids: Vec<&str> = BUILT_IN.iter().map(|p| p.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "two profiles share an identifier");
        for one in BUILT_IN {
            assert_eq!(profile(one.id), Some(one));
            assert!(!one.name.is_empty() && !one.note.is_empty());
        }
        assert_eq!(default_profile(), &INDIVIDUAL);
    }

    /// Every profile's note has to say what it does **not** do as well as what
    /// it does. A name like "highest security" doing the work of an explanation
    /// is the thing this project refuses everywhere else.
    #[test]
    fn every_profile_note_states_a_limit() {
        for one in BUILT_IN {
            let note = one.note.to_lowercase();
            assert!(
                note.contains("does not")
                    || note.contains("cannot")
                    || note.contains("the price")
                    || note.contains("rather than"),
                "{}: the note states no limit:\n{}",
                one.id,
                one.note
            );
        }
        // And the most private one has to name its cost outright.
        assert!(GROUP_ONE_VOICE.note.contains("The price is"));
    }

    /// A profile shapes the engine but must not touch the sample rate, which
    /// belongs to the recording. Overwriting it would resample somebody's audio
    /// because they picked a preset.
    #[test]
    fn a_profile_leaves_the_sample_rate_alone() {
        let config = DeidConfig {
            sample_rate: 44_100.0,
            ..DeidConfig::default()
        };
        for one in BUILT_IN {
            let applied = one.applied_to(config);
            assert_eq!(applied.sample_rate, 44_100.0, "{}", one.id);
            assert_eq!(applied.intensity, one.intensity);
            assert_eq!(applied.accent.enabled, one.neutralise_accent);
        }
    }

    /// The two group profiles differ in exactly one thing, which is the point
    /// of there being two of them.
    ///
    /// Looked up by identifier rather than named directly: an assertion about
    /// two `const`s is one the compiler can fold away, and clippy is right that
    /// a test which cannot fail is not a test. Going through `profile` is the
    /// path a front end takes anyway.
    #[test]
    fn the_two_group_profiles_differ_only_in_the_voices() {
        let voices = profile("group-voices").expect("a built-in profile");
        let one_voice = profile("group-one-voice").expect("a built-in profile");
        let single = profile("individual").expect("a built-in profile");

        assert!(voices.group && one_voice.group);
        assert!(!single.group);
        assert_eq!(voices.voices, VoiceMode::Distinct);
        assert_eq!(one_voice.voices, VoiceMode::Uniform);
        assert_eq!(voices.intensity, one_voice.intensity);
        assert_eq!(voices.neutralise_accent, one_voice.neutralise_accent);
        assert_eq!(voices.reseed_secs, one_voice.reseed_secs);
    }

    /// Every profile seals what it writes and strips metadata. A preset that
    /// quietly turned either off would be a preset that loses somebody data
    /// they thought was protected.
    #[test]
    fn no_profile_turns_off_a_protection() {
        for one in BUILT_IN.iter() {
            assert!(one.encrypt_at_rest, "{}", one.id);
            assert!(one.clean_metadata, "{}", one.id);
            assert!(one.neutralise_accent, "{}", one.id);
            assert!(one.intensity >= 1.0, "{}", one.id);
        }
    }
}
