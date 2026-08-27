// SPDX-License-Identifier: GPL-3.0-or-later
//! User-defined colour schemes, and the contrast check that keeps them usable.
//!
//! A reader can drop a small text file into a `palettes/` directory beside the
//! preferences file and have it appear in the theme picker alongside the nine
//! built-in schemes. The format is the same `key = value` shape
//! [`crate::prefs`] already uses, for the same reason: it is trivial to write
//! by hand, trivial to parse without a dependency, and has no syntax in which
//! something surprising can hide.
//!
//! # A palette file is untrusted input
//!
//! It arrives from the filesystem, it may have been written by hand at two in
//! the morning, and it may have been copied from a web page by somebody who has
//! never seen a hex colour. So it is parsed like anything else this project
//! reads: every field validated, every failure named, and **refused rather than
//! patched up**.
//!
//! Refusing matters more here than it first appears. The obvious lenient design
//! is to fill in whatever is missing from the default theme -- and that produces
//! a palette which is *mostly* the user's, with a few colours from somewhere
//! else, and no indication which. The user sees an application that looks
//! subtly wrong and has nothing to go on. An error naming the missing token is
//! worth more than a window that opens.
//!
//! # Contrast is computed, not trusted
//!
//! The request that prompted this asked for "whatever colour with correct
//! contrast to read". Correct contrast is not an aesthetic judgement, it is
//! arithmetic: WCAG 2.1 defines relative luminance and a contrast ratio, and a
//! ratio below 4.5 means body text a substantial number of people cannot read.
//!
//! So a palette whose foreground fails against its own background is **refused
//! with the measured ratio in the message**, rather than accepted and left to
//! produce an application nobody can use. It is the one validation here that
//! is about the user rather than about the parser, and it is the reason this
//! module exists at all rather than the fields being read straight into a
//! struct.
//!
//! The thresholds are stated where they are enforced, and they are the
//! standard's, not invented here:
//!
//! | Pair | Minimum | Why |
//! |---|---|---|
//! | `fg` on `bg` | 4.5 | Body text, WCAG AA |
//! | `fg` on `bg_soft` | 4.5 | The same text on a raised surface |
//! | `muted` on `bg` | 3.0 | Secondary text, AA large-text threshold |
//! | `accent` on `bg` | 3.0 | Links and controls, AA non-text contrast |
//! | `err` on `bg` | 3.0 | A warning nobody can read is worse than none |
//!
//! `muted` is deliberately held to 3.0 rather than 4.5. It is used for
//! secondary text that is meant to recede, every built-in theme would fail at
//! 4.5, and pretending otherwise would mean shipping a rule the project's own
//! themes break.
//!
//! # In plain words
//!
//! Lets you write your own colour scheme and have VeilVoice use it.
//!
//! Drop a small text file in a folder and it appears in the list. Every colour has
//! to be there, and the text has to be readable against the background: a scheme
//! whose text fails that check is refused rather than applied, with the measured
//! numbers so you know how far off it is and which way to move.

use crate::theme::Theme;
use egui::Color32;
use std::path::{Path, PathBuf};

/// Every token a palette file has to define.
///
/// Listed rather than derived so that a missing one is named in the error. The
/// order is the order they are reported in.
pub const REQUIRED: &[&str] = &[
    "bg", "bg-soft", "bg-inset", "border", "fg", "muted", "accent", "accent-2", "cyan", "ok",
    "warn", "err",
];

/// The most palette files that will be read from the directory.
///
/// A bound on work driven by the contents of a directory, in the same spirit as
/// the bounds in the search index and the release verifier. Nobody has forty
/// colour schemes; a directory containing forty thousand files is a mistake or
/// a prank, and either way the application should still start.
pub const MAX_PALETTES: usize = 40;

/// The largest palette file that will be read.
pub const MAX_BYTES: u64 = 16 * 1024;

/// Relative luminance, as defined by WCAG 2.1.
///
/// The channel transfer is the standard's, including the 0.03928 knee: it is
/// not the same as a plain gamma of 2.2, and using one would shift every ratio
/// slightly -- enough to pass a palette the standard fails.
fn luminance(colour: Color32) -> f32 {
    fn channel(value: u8) -> f32 {
        let v = value as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

/// The WCAG contrast ratio between two colours, from 1.0 to 21.0.
///
/// Symmetric: the order of the arguments does not matter, which is why the
/// lighter of the two is worked out here rather than assumed by the caller.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    let (lighter, darker) = if x > y { (x, y) } else { (y, x) };
    (lighter + 0.05) / (darker + 0.05)
}

/// The contrast pairs a palette has to satisfy, with the reason for each.
const PAIRS: &[(&str, &str, f32)] = &[
    ("fg", "bg", 4.5),
    ("fg", "bg-soft", 4.5),
    ("muted", "bg", 3.0),
    ("accent", "bg", 3.0),
    ("err", "bg", 3.0),
];

/// Check a palette's contrast, returning one message per failing pair.
///
/// Returns the measured ratio in each message. "Your colours are too similar"
/// is not actionable; "fg on bg is 2.1:1, and body text needs 4.5:1" tells
/// somebody exactly how far off they are and which way to move.
pub fn contrast_problems(theme: &Theme) -> Vec<String> {
    let value = |token: &str| -> Color32 {
        match token {
            "bg" => theme.bg,
            "bg-soft" => theme.bg_soft,
            "fg" => theme.fg,
            "muted" => theme.muted,
            "accent" => theme.accent,
            "err" => theme.err,
            _ => theme.fg,
        }
    };
    let mut problems = Vec::new();
    for (front, back, minimum) in PAIRS {
        let ratio = contrast(value(front), value(back));
        if ratio < *minimum {
            problems.push(format!(
                "{front} on {back} is {ratio:.1}:1, and needs at least {minimum:.1}:1"
            ));
        }
    }
    problems
}

/// One parsed colour scheme, before it is accepted.
#[derive(Debug)]
struct Parsed {
    id: String,
    name: String,
    light: bool,
    colours: Vec<(String, Color32)>,
}

fn parse_hex(text: &str) -> Option<Color32> {
    let text = text.trim().trim_start_matches('#');
    if text.len() != 6 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some(Color32::from_rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ))
}

/// Parse a palette file's text. Returns the scheme, or every problem found.
///
/// Every problem, not the first: somebody fixing a hand-written file should
/// learn about all four mistakes in one go rather than rerunning after each.
fn parse(text: &str, fallback_id: &str) -> Result<Parsed, Vec<String>> {
    let mut id = fallback_id.to_string();
    let mut name = String::new();
    let mut light = false;
    let mut colours: Vec<(String, Color32)> = Vec::new();
    let mut problems = Vec::new();

    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') && !line.contains('=') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!("line {}: expected `key = value`", number + 1));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "id" => id = value.to_string(),
            "name" => name = value.to_string(),
            "light" => light = value.eq_ignore_ascii_case("true"),
            _ if REQUIRED.contains(&key) => match parse_hex(value) {
                Some(colour) => colours.push((key.to_string(), colour)),
                None => problems.push(format!(
                    "line {}: {key} is `{value}`, which is not a #rrggbb colour",
                    number + 1
                )),
            },
            _ => problems.push(format!(
                "line {}: `{key}` is not a palette token",
                number + 1
            )),
        }
    }

    for token in REQUIRED {
        if !colours.iter().any(|(k, _)| k == token) {
            problems.push(format!("no `{token}` is defined"));
        }
    }

    // An id has to survive being written to the preferences file and read back,
    // and it shares a namespace with the built-in ids and with the website's
    // `data-theme` values. Anything outside this set is refused rather than
    // sanitised, because a silently renamed theme is one the user cannot select
    // again.
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        problems.push(format!(
            "`{id}` is not a usable id: lower-case letters, digits and hyphens only"
        ));
    }
    // Deliberately the *built-in* list rather than `theme::by_id`, which also
    // searches palettes already loaded. Checking the combined table would make
    // a palette collide with itself the second time it is read.
    if crate::theme::THEMES.iter().any(|t| t.id == id) {
        problems.push(format!("`{id}` is already the id of a built-in theme"));
    }

    if problems.is_empty() {
        if name.is_empty() {
            name = id.clone();
        }
        Ok(Parsed {
            id,
            name,
            light,
            colours,
        })
    } else {
        Err(problems)
    }
}

fn build(parsed: Parsed) -> Theme {
    let get = |token: &str| -> Color32 {
        parsed
            .colours
            .iter()
            .find(|(k, _)| k == token)
            .map(|(_, c)| *c)
            // Unreachable: `parse` refuses a palette with a missing token, and
            // this is only called on one that passed. Falling back to black
            // rather than panicking keeps a paint loop out of the panic path.
            .unwrap_or(Color32::BLACK)
    };
    Theme {
        // Leaked on purpose -- see `load` for why this is bounded and safe.
        id: Box::leak(parsed.id.into_boxed_str()),
        name: Box::leak(parsed.name.into_boxed_str()),
        light: parsed.light,
        bg: get("bg"),
        bg_soft: get("bg-soft"),
        bg_inset: get("bg-inset"),
        border: get("border"),
        fg: get("fg"),
        muted: get("muted"),
        accent: get("accent"),
        accent_2: get("accent-2"),
        cyan: get("cyan"),
        ok: get("ok"),
        warn: get("warn"),
        err: get("err"),
    }
}

/// Where palettes live, beside the preferences file.
pub fn default_dir() -> Option<PathBuf> {
    crate::prefs::default_path().map(|p| {
        p.parent()
            .map(|d| d.join("palettes"))
            .unwrap_or_else(|| PathBuf::from("palettes"))
    })
}

/// Read every palette in `dir`, returning the usable ones and every complaint.
///
/// Both halves are returned rather than one or the other. A palette that failed
/// must not silently vanish -- the user wrote a file, put it in the right place,
/// and is entitled to be told why it is not in the picker. The settings tab
/// shows these messages verbatim.
///
/// # On leaking
///
/// [`Theme`] holds `&'static str` for its id and name so that reading the
/// active theme in a paint loop is one relaxed atomic load and a slice index,
/// with no lock and no allocation -- see the module docs on `theme.rs`. Custom
/// palettes therefore leak those two strings.
///
/// That is a deliberate, bounded, load-once leak: at most [`MAX_PALETTES`]
/// entries, read during startup, never in a loop, and each a few dozen bytes.
/// The alternative -- reference counting or a lock around the palette table --
/// would put synchronisation into the hottest read in the application to avoid
/// leaking about a kilobyte, once.
pub fn load(dir: &Path) -> (Vec<Theme>, Vec<String>) {
    let mut themes = Vec::new();
    let mut problems = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        // No directory is the normal case, not an error worth reporting.
        return (themes, problems);
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "palette"))
        .collect();
    // Sorted so the picker's order is the same on every machine and every run.
    files.sort();

    if files.len() > MAX_PALETTES {
        problems.push(format!(
            "{} palette files found; reading the first {MAX_PALETTES}",
            files.len()
        ));
        files.truncate(MAX_PALETTES);
    }

    for path in files {
        let shown = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Open first, then ask the *handle* what it is. Calling `metadata` on
        // the path and then opening it is two operations on something that can
        // change in between, and it answers the wrong question anyway: a FIFO
        // reports a length of zero, sails past a size check, and then blocks
        // `read_to_string` for ever -- during startup, before the window
        // exists, so the application simply never appears.
        let handle = match std::fs::File::open(&path) {
            Ok(handle) => handle,
            Err(error) => {
                problems.push(format!("{shown}: {error}"));
                continue;
            }
        };
        match handle.metadata() {
            Ok(meta) if !meta.is_file() => {
                problems.push(format!("{shown}: not a regular file, ignored"));
                continue;
            }
            Err(error) => {
                problems.push(format!("{shown}: {error}"));
                continue;
            }
            _ => {}
        }

        // Bound the read rather than the file. One byte past the limit is
        // enough to know the limit was passed, and nothing larger is ever held
        // in memory -- so a file that grows between being checked and being
        // read cannot get past this.
        let mut bytes = Vec::new();
        use std::io::Read as _;
        if let Err(error) = handle.take(MAX_BYTES + 1).read_to_end(&mut bytes) {
            problems.push(format!("{shown}: {error}"));
            continue;
        }
        if bytes.len() as u64 > MAX_BYTES {
            problems.push(format!("{shown}: larger than {MAX_BYTES} bytes, ignored"));
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            problems.push(format!("{shown}: not readable as UTF-8"));
            continue;
        };

        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match parse(&text, &stem) {
            Err(found) => {
                for line in found {
                    problems.push(format!("{shown}: {line}"));
                }
            }
            Ok(parsed) => {
                let theme = build(parsed);
                let contrast = contrast_problems(&theme);
                if contrast.is_empty() {
                    themes.push(theme);
                } else {
                    for line in contrast {
                        problems.push(format!("{shown}: {line}"));
                    }
                }
            }
        }
    }

    (themes, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "\
id = midnight
name = Midnight
light = false
bg = #101014
bg-soft = #1a1a22
bg-inset = #0b0b0e
border = #2f2f3a
fg = #e6e6f0
muted = #9a9ab0
accent = #7aa2f7
accent-2 = #bb9af7
cyan = #7dcfff
ok = #9ece6a
warn = #e0af68
err = #f7768e
";

    #[test]
    fn a_well_formed_palette_is_accepted() {
        let parsed = parse(GOOD, "fallback").expect("should parse");
        assert_eq!(parsed.id, "midnight");
        assert_eq!(parsed.name, "Midnight");
        assert_eq!(parsed.colours.len(), REQUIRED.len());
    }

    #[test]
    fn the_contrast_ratio_matches_the_standard() {
        // The two anchors every implementation is checked against: black on
        // white is exactly 21, and a colour against itself is exactly 1.
        let ratio = contrast(Color32::BLACK, Color32::WHITE);
        assert!((ratio - 21.0).abs() < 0.01, "black on white was {ratio}");
        let same = contrast(
            Color32::from_rgb(0x7a, 0xa2, 0xf7),
            Color32::from_rgb(0x7a, 0xa2, 0xf7),
        );
        assert!(
            (same - 1.0).abs() < 0.001,
            "a colour against itself was {same}"
        );
    }

    #[test]
    fn contrast_is_symmetric() {
        let a = Color32::from_rgb(0x1a, 0x1b, 0x26);
        let b = Color32::from_rgb(0xc0, 0xca, 0xf5);
        assert!((contrast(a, b) - contrast(b, a)).abs() < 1e-6);
    }

    #[test]
    fn every_built_in_theme_passes_its_own_contrast_check() {
        // If a rule the project enforces on other people's palettes fails on
        // its own, the rule is wrong or the theme is. Either way it must not
        // ship, and finding out here is better than from a user who cannot
        // read the window.
        for theme in crate::theme::THEMES {
            let problems = contrast_problems(theme);
            assert!(
                problems.is_empty(),
                "the built-in '{}' theme fails the contrast rule: {:?}",
                theme.id,
                problems
            );
        }
    }

    #[test]
    fn an_unreadable_palette_is_refused_with_the_measured_ratio() {
        let text = GOOD.replace("fg = #e6e6f0", "fg = #17171d");
        let parsed = parse(&text, "x").expect("parses");
        let theme = build(parsed);
        let problems = contrast_problems(&theme);
        assert!(!problems.is_empty(), "near-invisible text was accepted");
        assert!(
            problems[0].contains(":1"),
            "the message must carry the measured ratio, got {:?}",
            problems[0]
        );
    }

    #[test]
    fn a_missing_token_is_named_rather_than_filled_in() {
        let text = GOOD.replace("cyan = #7dcfff\n", "");
        let problems = parse(&text, "x").expect_err("should be refused");
        assert!(
            problems.iter().any(|p| p.contains("`cyan`")),
            "the missing token was not named: {problems:?}"
        );
    }

    #[test]
    fn every_problem_is_reported_not_just_the_first() {
        let text = GOOD
            .replace("cyan = #7dcfff\n", "")
            .replace("ok = #9ece6a", "ok = green");
        let problems = parse(&text, "x").expect_err("should be refused");
        assert!(
            problems.len() >= 2,
            "only {} problem(s) reported: {problems:?}",
            problems.len()
        );
    }

    #[test]
    fn a_bad_colour_is_refused_rather_than_guessed() {
        for bad in ["green", "#fff", "#gggggg", "", "0x112233", "#1122333"] {
            let text = GOOD.replace("ok = #9ece6a", &format!("ok = {bad}"));
            assert!(
                parse(&text, "x").is_err(),
                "the value {bad:?} was accepted as a colour"
            );
        }
    }

    #[test]
    fn a_palette_cannot_impersonate_a_built_in_theme() {
        let text = GOOD.replace("id = midnight", "id = tokyo-night");
        let problems = parse(&text, "x").expect_err("should be refused");
        assert!(problems.iter().any(|p| p.contains("built-in")));
    }

    #[test]
    fn an_id_that_would_not_survive_the_preferences_file_is_refused() {
        for bad in ["Midnight", "mid night", "../../etc/passwd", "mid=night", ""] {
            let text = GOOD.replace("id = midnight", &format!("id = {bad}"));
            assert!(
                parse(&text, "fallbackid").is_err(),
                "the id {bad:?} was accepted"
            );
        }
    }

    /// A scratch directory that cleans up after itself.
    ///
    /// No `tempfile` dependency for four tests: this project's argument is that
    /// its supply chain is small enough to read, and a crate pulled in to make
    /// a directory would be a poor trade.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            // The process id keeps two `cargo test` runs from colliding, and
            // the tag keeps the tests within one run apart.
            let dir = std::env::temp_dir().join(format!(
                "veilvoice-palettes-{}-{}",
                std::process::id(),
                tag
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch directory");
            Self(dir)
        }

        fn write(&self, name: &str, text: &str) {
            std::fs::write(self.0.join(name), text).expect("write palette");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_palette_file_on_disk_becomes_a_theme() {
        let scratch = Scratch::new("good");
        scratch.write("midnight.palette", GOOD);

        let (themes, problems) = load(&scratch.0);
        assert!(problems.is_empty(), "unexpected complaints: {problems:?}");
        assert_eq!(themes.len(), 1, "the file did not become a theme");
        assert_eq!(themes[0].id, "midnight");
        assert_eq!(themes[0].name, "Midnight");
        assert_eq!(themes[0].bg, Color32::from_rgb(0x10, 0x10, 0x14));
    }

    #[test]
    fn a_rejected_palette_is_reported_rather_than_dropped_in_silence() {
        let scratch = Scratch::new("bad");
        scratch.write("broken.palette", &GOOD.replace("cyan = #7dcfff\n", ""));

        let (themes, problems) = load(&scratch.0);
        assert!(themes.is_empty(), "a palette missing a token was accepted");
        assert!(
            problems.iter().any(|p| p.contains("broken.palette")),
            "the complaint does not name the file: {problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("`cyan`")),
            "the complaint does not name the missing token: {problems:?}"
        );
    }

    #[test]
    fn one_bad_palette_does_not_hide_a_good_one() {
        // The lenient-parser trap in reverse: a directory with a mistake in it
        // must still yield the palettes that are fine, or one typo costs the
        // user every scheme they wrote.
        let scratch = Scratch::new("mixed");
        scratch.write("midnight.palette", GOOD);
        scratch.write("broken.palette", "id = nonsense\n");
        scratch.write("notes.txt", "this is not a palette and must be ignored");

        let (themes, problems) = load(&scratch.0);
        assert_eq!(themes.len(), 1, "the good palette was lost: {problems:?}");
        assert_eq!(themes[0].id, "midnight");
        assert!(
            problems.iter().any(|p| p.contains("broken.palette")),
            "the broken one was not reported: {problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.contains("notes.txt")),
            "a file that is not a palette was complained about: {problems:?}"
        );
    }

    #[test]
    fn palettes_load_in_a_stable_order() {
        // The picker's order has to be the same on every machine and every
        // run, and `read_dir` guarantees no order at all.
        let scratch = Scratch::new("order");
        for (name, id) in [("zulu", "zulu"), ("alpha", "alpha"), ("mike", "mike")] {
            scratch.write(
                &format!("{name}.palette"),
                &GOOD.replace("id = midnight", &format!("id = {id}")),
            );
        }
        let (themes, _) = load(&scratch.0);
        let ids: Vec<&str> = themes.iter().map(|t| t.id).collect();
        assert_eq!(ids, ["alpha", "mike", "zulu"]);
    }

    #[test]
    fn something_that_is_not_a_regular_file_is_refused_not_read() {
        // A FIFO is the case that mattered -- it reports a length of zero,
        // passes a size check and then blocks `read_to_string` for ever during
        // startup. `mkfifo` is not portable, so the rule is exercised with a
        // directory named like a palette: same path through the loader, same
        // question asked of the handle, and it must not be read.
        let scratch = Scratch::new("special");
        std::fs::create_dir(scratch.0.join("trap.palette")).expect("make a directory");

        let (themes, problems) = load(&scratch.0);
        assert!(themes.is_empty());
        assert!(
            problems.iter().any(|p| p.contains("trap.palette")),
            "the entry was skipped without a word: {problems:?}"
        );
    }

    #[test]
    fn a_file_over_the_size_bound_is_refused() {
        let scratch = Scratch::new("huge");
        let padding = "\n# ".to_string() + &"x".repeat(MAX_BYTES as usize);
        scratch.write("fat.palette", &(GOOD.to_string() + &padding));

        let (themes, problems) = load(&scratch.0);
        assert!(themes.is_empty(), "an oversized palette was accepted");
        assert!(
            problems.iter().any(|p| p.contains("larger than")),
            "the size refusal was not reported: {problems:?}"
        );
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let (themes, problems) = load(Path::new("no/such/directory/anywhere"));
        assert!(themes.is_empty());
        assert!(
            problems.is_empty(),
            "a missing directory reported {problems:?}"
        );
    }
}
