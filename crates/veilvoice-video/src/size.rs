// SPDX-License-Identifier: GPL-3.0-or-later
//! The size and frame rate a video is rendered at.
//!
//! # Why this is a module rather than two numbers
//!
//! The renderer used to write `1280x720` at thirty frames a second and offer no
//! way to say otherwise. Both numbers were reasonable defaults and neither was a
//! choice anybody could make, which is the whole of the problem: somebody
//! rendering a conversation to put on a screen wants it to look like the screen
//! they are putting it on.
//!
//! Getting that wrong is expensive in a way a wrong colour is not. Frames are
//! rendered one file at a time before `ffmpeg` is asked for anything, so a
//! choice made at the start decides how long the render takes, how much disk it
//! wants while it runs, and whether it finishes at all. [`Plan::estimate`]
//! exists so a front end can say that before starting rather than after.
//!
//! # The rules a size has to obey, and where they come from
//!
//! **Both dimensions have to be even.** H.264 with `yuv420p`, which is what
//! every player and every platform accepts, stores colour at half resolution in
//! both directions, so an odd dimension has half a chroma sample in it. `ffmpeg`
//! refuses outright: "width not divisible by 2". A person typing 1921 has made a
//! typo rather than a request, so [`Size::new`] says so and
//! [`Size::nearest_valid`] offers the size they meant.
//!
//! **There is a floor and a ceiling**, and both are about somebody's typo
//! rather than about taste. Under [`MIN_EDGE`] the subtitles are unreadable and
//! the waveform is a line. Over [`MAX_EDGE`], which is 8K, an extra digit turns
//! a ten-minute render into one that fills the disk: at 4K a frame is about
//! eight megabytes before compression, and at 60 frames a second that is half a
//! gigabyte for every second of recording.
//!
//! # The default is the screen it will be watched on, when the screen will say
//!
//! [`Choice::Monitor`] is the default and it is *resolved late*: the size is
//! decided when the render starts, from the display the program is actually
//! running on, rather than stored as a number that becomes wrong when somebody
//! plugs in a different monitor.
//!
//! Where nothing can say what the display is running at, and a command line on a
//! machine with no display server is the ordinary case, it falls back to
//! [`Preset::Hd1080`] **and says so**. A guess about somebody's monitor
//! presented as a detection would be worse than a stated default.
//!
//! # In plain words
//!
//! How big the video is and how many pictures a second it has.
//!
//! By default it matches the screen you are using, because that is usually the
//! screen you are going to watch it on. You can pick 720p, 1080p, 1440p or 4K
//! instead, or type your own size, and anything up to 60 frames a second.
//!
//! Bigger and faster is not better here. The picture is a waveform, some
//! circles and words on a flat background, none of which move quickly, so 4K at
//! 60 costs a great deal of time and disk for something that looks the same as
//! 1080p at 30 to almost everybody.

use crate::Error;

/// The shortest edge a render may have, in pixels.
///
/// Below this the subtitles cannot be read and the waveform is one pixel tall,
/// so the file would be a video of nothing. 256 is the smallest that survives a
/// phone screen.
pub const MIN_EDGE: u32 = 256;

/// The longest edge a render may have, in pixels.
///
/// 8K. Not because anybody needs it, but because a limit has to be somewhere
/// and this is the largest thing that exists to be watched on.
pub const MAX_EDGE: u32 = 7680;

/// The most frames a second a render may have.
///
/// **Sixty, and it is a cap rather than a target.** Nothing in this picture
/// moves quickly: a waveform scrolls, a circle brightens, words appear. Sixty
/// doubles the frames, the render time and the file against thirty and looks
/// the same to almost everybody. It is offered because somebody cutting this
/// into 60fps footage needs it to match, which is a real reason, and it is not
/// the default because it is not an improvement.
pub const MAX_FPS: u32 = 60;

/// The fewest frames a second a render may have.
///
/// Under this the waveform stutters rather than scrolls.
pub const MIN_FPS: u32 = 5;

/// A frame size in pixels, known to be one a render can actually use.
///
/// Constructed only through [`Size::new`], so a value of this type has already
/// been checked: both edges even, both inside [`MIN_EDGE`] and [`MAX_EDGE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size {
    width: u32,
    height: u32,
}

impl Size {
    /// A size, if it is one a video can be rendered at.
    ///
    /// The error says which rule was broken and what the nearest allowed size
    /// is, because "invalid size" sends somebody to guess and this does not.
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        if width < MIN_EDGE || height < MIN_EDGE {
            return Err(Error::Size(format!(
                "{width}x{height} is smaller than {MIN_EDGE} on one side, where \
                 the subtitles cannot be read"
            )));
        }
        if width > MAX_EDGE || height > MAX_EDGE {
            return Err(Error::Size(format!(
                "{width}x{height} is larger than {MAX_EDGE} on one side, which \
                 is 8K and is the largest this renders"
            )));
        }
        if width % 2 == 1 || height % 2 == 1 {
            let near = Self {
                width: width & !1,
                height: height & !1,
            };
            return Err(Error::Size(format!(
                "{width}x{height} has an odd side, which H.264 cannot store in \
                 yuv420p; {} is the nearest size that works",
                near.label()
            )));
        }
        Ok(Self { width, height })
    }

    /// The nearest size that obeys every rule, for offering after a refusal.
    ///
    /// Rounds each edge down to even and clamps it into range. Always returns a
    /// value: the clamping cannot fail, because both bounds are themselves
    /// even and inside the range.
    pub fn nearest_valid(width: u32, height: u32) -> Self {
        let fix = |edge: u32| edge.clamp(MIN_EDGE, MAX_EDGE) & !1;
        Self {
            width: fix(width),
            height: fix(height),
        }
    }

    /// Width in pixels. Always even.
    pub fn width(self) -> u32 {
        self.width
    }

    /// Height in pixels. Always even.
    pub fn height(self) -> u32 {
        self.height
    }

    /// How many pixels one frame holds.
    ///
    /// `u64` deliberately: 7680 by 4320 is 33 million, which fits a `u32`, but
    /// multiplying it by a frame count does not, and this is the number that
    /// gets multiplied.
    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// What `ffmpeg` wants after `-s`, and what a person reads in a menu.
    ///
    /// `1920x1080`, with the familiar name after it where there is one. Nobody
    /// says "1920 by 1080" out loud and everybody says "1080p".
    pub fn label(self) -> String {
        match Preset::matching(self) {
            Some(preset) => format!("{}x{} ({})", self.width, self.height, preset.name()),
            None => format!("{}x{}", self.width, self.height),
        }
    }

    /// Just the digits, for a command line argument.
    pub fn geometry(self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

/// The sizes offered by name.
///
/// Sixteen by nine throughout, because that is what every player, every phone
/// and every platform expects, and because a conversation rendered to a shape
/// nobody uses gets black bars added by somebody else's software.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Preset {
    /// 1280x720.
    Hd720,
    /// 1920x1080. The fallback when no display can be asked.
    Hd1080,
    /// 2560x1440.
    Qhd1440,
    /// 3840x2160, which is what "4K" means for video.
    Uhd2160,
}

impl Preset {
    /// Every preset, in the order a menu should list them.
    pub const ALL: [Preset; 4] = [
        Preset::Hd720,
        Preset::Hd1080,
        Preset::Qhd1440,
        Preset::Uhd2160,
    ];

    /// The size this preset means.
    pub fn size(self) -> Size {
        let (width, height) = match self {
            Preset::Hd720 => (1280, 720),
            Preset::Hd1080 => (1920, 1080),
            Preset::Qhd1440 => (2560, 1440),
            Preset::Uhd2160 => (3840, 2160),
        };
        // Every one of these is even and in range by construction, so the
        // checking constructor cannot refuse them. Written as an expect with
        // the reason rather than a second unchecked constructor, so there is
        // exactly one way to make a `Size`.
        Size::new(width, height).expect("a preset is a valid size by construction")
    }

    /// What a person calls it.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Hd720 => "720p",
            Preset::Hd1080 => "1080p",
            Preset::Qhd1440 => "1440p",
            Preset::Uhd2160 => "4K",
        }
    }

    /// What it answers to on a command line. Lower case and stable.
    pub fn key(self) -> &'static str {
        match self {
            Preset::Hd720 => "720p",
            Preset::Hd1080 => "1080p",
            Preset::Qhd1440 => "1440p",
            Preset::Uhd2160 => "4k",
        }
    }

    /// The preset a size is, if it is one of them.
    pub fn matching(size: Size) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.size() == size)
    }
}

/// What the user asked for, before anything has looked at the display.
///
/// Held rather than resolved, so that "match my screen" stays true when the
/// screen changes. See the module documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Choice {
    /// Whatever the display this is running on is set to.
    #[default]
    Monitor,
    /// One of the sizes offered by name.
    Named(Preset),
    /// A size somebody typed.
    Exact(Size),
}

/// A size, and why it is that size.
///
/// The reason travels with the number because a front end has to be able to say
/// "1080p, because this machine could not tell us what your display is running
/// at" rather than showing 1080p as though it had been detected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The size to render at.
    pub size: Size,
    /// What to tell the person, or nothing when the answer needs no explaining.
    pub note: Option<String>,
}

impl Choice {
    /// What this answers to on a command line, in the order help should list it.
    ///
    /// One list, used by the command line's help, the command line's parser and
    /// the window's menu, so a name that works in one works in all of them.
    pub const KEYS: [&'static str; 5] = ["monitor", "720p", "1080p", "1440p", "4k"];

    /// Read a choice somebody typed.
    ///
    /// Accepts a name from [`Choice::KEYS`] or an explicit `WIDTHxHEIGHT`. The
    /// `x` may be an `X` or a `*`, because both are what people type, and the
    /// error names every accepted form rather than only refusing.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        let lower = text.to_ascii_lowercase();
        if lower == "monitor" || lower == "screen" || lower == "auto" {
            return Ok(Choice::Monitor);
        }
        if let Some(preset) = Preset::ALL.into_iter().find(|p| p.key() == lower) {
            return Ok(Choice::Named(preset));
        }
        // `2160p` and `1080` are what somebody means even though the menu does
        // not spell them that way, so they are read rather than refused.
        if let Some(preset) = Preset::ALL.into_iter().find(|p| {
            lower == format!("{}p", p.size().height()) || lower == p.size().height().to_string()
        }) {
            return Ok(Choice::Named(preset));
        }
        let parts: Vec<&str> = lower.split(['x', '*']).collect();
        if parts.len() == 2 {
            if let (Ok(width), Ok(height)) = (
                parts[0].trim().parse::<u32>(),
                parts[1].trim().parse::<u32>(),
            ) {
                return Ok(Choice::Exact(Size::new(width, height)?));
            }
        }
        Err(Error::Size(format!(
            "`{text}` is not a size. Use one of {}, or a size such as 1920x1080.",
            Choice::KEYS.join(", ")
        )))
    }

    /// What this reads as in a menu or a report.
    pub fn describe(self) -> String {
        match self {
            Choice::Monitor => "match this display".to_string(),
            Choice::Named(preset) => preset.size().label(),
            Choice::Exact(size) => size.label(),
        }
    }

    /// Turn a choice into a size, given what the display said.
    ///
    /// `monitor` is the display's size where something could ask for it, and
    /// `None` where nothing could: a headless command line, a platform with no
    /// interface for it, a remote session. Both cases are ordinary and neither
    /// is an error.
    ///
    /// A monitor size that breaks the rules is corrected rather than refused. A
    /// display running at an odd height is a fact about somebody's hardware and
    /// not a mistake they made, so the nearest usable size is taken and the
    /// note says what happened.
    pub fn resolve(self, monitor: Option<(u32, u32)>) -> Resolved {
        match self {
            Choice::Named(preset) => Resolved {
                size: preset.size(),
                note: None,
            },
            Choice::Exact(size) => Resolved { size, note: None },
            Choice::Monitor => match monitor {
                Some((width, height)) => match Size::new(width, height) {
                    Ok(size) => Resolved { size, note: None },
                    Err(_) => {
                        let size = Size::nearest_valid(width, height);
                        Resolved {
                            note: Some(format!(
                                "this display is {width}x{height}, which a video \
                                 cannot be exactly; rendering at {} instead",
                                size.label()
                            )),
                            size,
                        }
                    }
                },
                None => Resolved {
                    size: Preset::Hd1080.size(),
                    note: Some(
                        "nothing here can say what this display is running at, \
                         so this is 1080p rather than a guess"
                            .to_string(),
                    ),
                },
            },
        }
    }
}

/// Frames per second, known to be inside the range a render allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameRate(u32);

impl FrameRate {
    /// A frame rate, if it is one a render allows.
    pub fn new(fps: u32) -> Result<Self, Error> {
        if !(MIN_FPS..=MAX_FPS).contains(&fps) {
            return Err(Error::Size(format!(
                "{fps} frames a second is outside {MIN_FPS} to {MAX_FPS}"
            )));
        }
        Ok(Self(fps))
    }

    /// The number.
    pub fn get(self) -> u32 {
        self.0
    }

    /// The rates offered by name, in the order a menu should list them.
    ///
    /// Twenty-four because it is what film runs at and what somebody cutting
    /// this into film footage needs; thirty as the default; fifty and sixty for
    /// matching the two broadcast rates.
    pub const OFFERED: [u32; 4] = [24, 30, 50, 60];
}

impl FrameRate {
    /// Read a frame rate somebody typed.
    ///
    /// A trailing `fps` is accepted because it is what people write.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim().to_ascii_lowercase();
        let digits = text.strip_suffix("fps").unwrap_or(&text).trim();
        match digits.parse::<u32>() {
            Ok(fps) => Self::new(fps),
            Err(_) => Err(Error::Size(format!(
                "`{text}` is not a frame rate. Use a whole number from \
                 {MIN_FPS} to {MAX_FPS}, such as 30."
            ))),
        }
    }
}

impl Default for FrameRate {
    fn default() -> Self {
        // Thirty. Enough for a waveform and a circle that brightens, and half
        // the frames of sixty for a picture that does not move fast enough to
        // tell them apart.
        Self(30)
    }
}

/// A size and a frame rate together, with what they will cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The frame size.
    pub size: Size,
    /// Frames per second.
    pub fps: FrameRate,
}

/// What a render is going to want before it is started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimate {
    /// How many frames will be written.
    pub frames: u64,
    /// Roughly how many bytes those frames occupy on disk while rendering.
    ///
    /// **Before the encode, not after.** The video file is far smaller; this is
    /// the temporary directory the frames pass through, which is what fills a
    /// disk. Deliberately rough and deliberately generous: a flat-coloured PNG
    /// compresses to a small fraction of its pixels, and an estimate that is
    /// too low is the one that hurts.
    pub scratch_bytes: u64,
}

/// A byte count, in the units a person reads.
///
/// Here rather than in a front end because both the window and the command line
/// print the same estimate, and two roundings of one number is two numbers.
/// Binary units, because a disk with "1 GB free" has 1 GiB free and the
/// estimate is about whether the render fits.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

impl Plan {
    /// A plan.
    pub fn new(size: Size, fps: FrameRate) -> Self {
        Self { size, fps }
    }

    /// What rendering `seconds` of recording will want.
    ///
    /// Saturating throughout. An hour of 8K at sixty frames a second is a
    /// number nobody should reach, and reaching it should produce a large
    /// figure a front end can refuse, not an overflow.
    pub fn estimate(&self, seconds: f64) -> Estimate {
        let seconds = if seconds.is_finite() && seconds > 0.0 {
            seconds
        } else {
            0.0
        };
        let frames = (seconds * f64::from(self.fps.get())).ceil();
        let frames = if frames >= 0.0 && frames <= u64::MAX as f64 {
            frames as u64
        } else {
            u64::MAX
        };
        // A quarter of a byte per pixel. Flat colour, large areas of one shade
        // and a little text is what PNG is best at, and measured output for
        // these frames sits well under this.
        let per_frame = (self.size.pixels() / 4).max(1);
        Estimate {
            frames,
            scratch_bytes: frames.saturating_mul(per_frame),
        }
    }
}

impl Default for Plan {
    fn default() -> Self {
        Self::new(Preset::Hd1080.size(), FrameRate::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_a_size_a_video_can_be() {
        for preset in Preset::ALL {
            let size = preset.size();
            assert_eq!(size.width() % 2, 0, "{} width", preset.name());
            assert_eq!(size.height() % 2, 0, "{} height", preset.name());
            assert!(size.width() >= MIN_EDGE && size.width() <= MAX_EDGE);
            assert!(size.height() >= MIN_EDGE && size.height() <= MAX_EDGE);
            // Sixteen by nine, to one part in a thousand.
            let ratio = f64::from(size.width()) / f64::from(size.height());
            assert!((ratio - 16.0 / 9.0).abs() < 0.001, "{}", preset.name());
        }
    }

    #[test]
    fn a_preset_knows_its_own_size_and_back_again() {
        for preset in Preset::ALL {
            assert_eq!(Preset::matching(preset.size()), Some(preset));
        }
        // And a size that is not one of them says so rather than guessing.
        assert_eq!(Preset::matching(Size::new(1000, 1000).unwrap()), None);
    }

    #[test]
    fn an_odd_side_is_refused_and_the_nearest_even_one_is_offered() {
        let refused = Size::new(1921, 1080).unwrap_err().to_string();
        assert!(refused.contains("odd side"), "{refused}");
        assert!(refused.contains("1920x1080"), "{refused}");
        assert!(refused.contains("yuv420p"), "{refused}");
        assert_eq!(
            Size::nearest_valid(1921, 1081),
            Size::new(1920, 1080).unwrap()
        );
    }

    #[test]
    fn sizes_outside_the_range_say_which_end_they_fell_off() {
        let small = Size::new(100, 100).unwrap_err().to_string();
        assert!(small.contains("smaller"), "{small}");
        let large = Size::new(10_000, 10_000).unwrap_err().to_string();
        assert!(large.contains("larger"), "{large}");
        // The bounds themselves are allowed, which is what "inside" means.
        assert!(Size::new(MIN_EDGE, MIN_EDGE).is_ok());
        assert!(Size::new(MAX_EDGE, MAX_EDGE).is_ok());
    }

    #[test]
    fn nearest_valid_always_produces_something_new_accepts() {
        // Including the shapes that broke every rule at once.
        for (w, h) in [(0, 0), (1, 1), (99_999, 3), (7681, 4321), (1921, 1081)] {
            let fixed = Size::nearest_valid(w, h);
            assert!(
                Size::new(fixed.width(), fixed.height()).is_ok(),
                "{w}x{h} became {fixed:?}, which new() refuses"
            );
        }
    }

    #[test]
    fn the_default_choice_follows_the_display() {
        let resolved = Choice::default().resolve(Some((2560, 1440)));
        assert_eq!(resolved.size, Preset::Qhd1440.size());
        assert!(resolved.note.is_none(), "a plain answer needs no note");
    }

    #[test]
    fn a_display_nothing_can_ask_about_falls_back_and_says_so() {
        let resolved = Choice::Monitor.resolve(None);
        assert_eq!(resolved.size, Preset::Hd1080.size());
        let note = resolved.note.expect("a fallback has to be explained");
        assert!(note.contains("1080p"), "{note}");
        assert!(note.contains("guess"), "{note}");
    }

    #[test]
    fn an_awkward_display_is_corrected_rather_than_refused() {
        // A real shape: some laptop panels are 3000x2000, and 1366x768 has an
        // odd sibling in 1365x767 on scaled displays.
        let resolved = Choice::Monitor.resolve(Some((1365, 767)));
        assert_eq!(resolved.size, Size::new(1364, 766).unwrap());
        let note = resolved.note.expect("a correction has to be explained");
        assert!(note.contains("1365x767"), "{note}");
        assert!(note.contains("1364x766"), "{note}");
    }

    #[test]
    fn a_display_below_the_floor_is_lifted_to_it() {
        let resolved = Choice::Monitor.resolve(Some((320, 200)));
        assert_eq!(resolved.size.height(), MIN_EDGE);
        assert!(resolved.note.is_some());
    }

    #[test]
    fn a_named_or_exact_choice_ignores_the_display_entirely() {
        let named = Choice::Named(Preset::Hd720).resolve(Some((3840, 2160)));
        assert_eq!(named.size, Preset::Hd720.size());
        assert!(named.note.is_none());

        let exact = Size::new(1000, 500).unwrap();
        assert_eq!(Choice::Exact(exact).resolve(None).size, exact);
    }

    #[test]
    fn the_frame_rate_stops_at_sixty() {
        assert_eq!(FrameRate::new(60).unwrap().get(), 60);
        let over = FrameRate::new(61).unwrap_err().to_string();
        assert!(over.contains("60"), "{over}");
        assert!(FrameRate::new(MIN_FPS - 1).is_err());
        assert_eq!(FrameRate::default().get(), 30);
        for fps in FrameRate::OFFERED {
            assert!(FrameRate::new(fps).is_ok(), "{fps} is offered and refused");
        }
    }

    #[test]
    fn an_estimate_grows_with_size_and_rate_and_never_overflows() {
        let small = Plan::new(Preset::Hd720.size(), FrameRate::new(24).unwrap());
        let large = Plan::new(Preset::Uhd2160.size(), FrameRate::new(60).unwrap());
        let (a, b) = (small.estimate(60.0), large.estimate(60.0));
        assert!(b.frames > a.frames);
        assert!(b.scratch_bytes > a.scratch_bytes);
        assert_eq!(a.frames, 60 * 24);

        // The shapes that would panic or wrap if this were arithmetic on `u32`.
        for seconds in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e30] {
            let _ = large.estimate(seconds);
        }
        assert_eq!(large.estimate(f64::NAN).frames, 0);
        assert_eq!(large.estimate(-1.0).frames, 0);
    }

    #[test]
    fn every_offered_name_parses_back_to_what_it_names() {
        assert_eq!(Choice::parse("monitor").unwrap(), Choice::Monitor);
        for preset in Preset::ALL {
            assert_eq!(
                Choice::parse(preset.key()).unwrap(),
                Choice::Named(preset),
                "{}",
                preset.key()
            );
        }
        // Every key the help prints has to be one the parser takes, or the
        // help is a list of things that do not work.
        for key in Choice::KEYS {
            assert!(
                Choice::parse(key).is_ok(),
                "help offers `{key}` and it fails"
            );
        }
    }

    #[test]
    fn the_forms_people_actually_type_are_read_rather_than_refused() {
        for text in ["1080P", " 1080p ", "2160p", "2160", "auto", "SCREEN"] {
            assert!(Choice::parse(text).is_ok(), "{text}");
        }
        assert_eq!(
            Choice::parse("2160p").unwrap(),
            Choice::Named(Preset::Uhd2160)
        );
        for text in ["1920x1080", "1920X1080", "1920*1080", " 1920 x 1080 "] {
            assert_eq!(
                Choice::parse(text).unwrap(),
                Choice::Exact(Preset::Hd1080.size()),
                "{text}"
            );
        }
        for text in ["30fps", "30 FPS", " 30 "] {
            assert_eq!(FrameRate::parse(text).unwrap().get(), 30, "{text}");
        }
    }

    #[test]
    fn a_size_that_is_not_one_says_what_would_be() {
        let refused = Choice::parse("enormous").unwrap_err().to_string();
        for key in Choice::KEYS {
            assert!(refused.contains(key), "{refused} omits {key}");
        }
        assert!(refused.contains("1920x1080"), "{refused}");

        // An explicit size that breaks a rule keeps the rule's own message,
        // which is more useful than "not a size".
        let odd = Choice::parse("1921x1080").unwrap_err().to_string();
        assert!(odd.contains("odd side"), "{odd}");

        let bad_rate = FrameRate::parse("lots").unwrap_err().to_string();
        assert!(bad_rate.contains("60"), "{bad_rate}");
    }

    #[test]
    fn a_choice_describes_itself_without_needing_a_display() {
        assert_eq!(Choice::Monitor.describe(), "match this display");
        assert_eq!(Choice::Named(Preset::Uhd2160).describe(), "3840x2160 (4K)");
    }

    #[test]
    fn bytes_are_printed_in_units_somebody_reads() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(human_bytes(200 * 1024 * 1024), "200 MiB");
        // The largest thing this can be handed, which must not panic or wrap.
        assert!(human_bytes(u64::MAX).ends_with("TiB"));
    }

    #[test]
    fn a_label_names_the_preset_where_there_is_one() {
        assert_eq!(Preset::Hd1080.size().label(), "1920x1080 (1080p)");
        assert_eq!(Size::new(1000, 500).unwrap().label(), "1000x500");
        assert_eq!(Preset::Uhd2160.size().geometry(), "3840x2160");
    }
}
