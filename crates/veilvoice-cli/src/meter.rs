// SPDX-License-Identifier: GPL-3.0-or-later
//! Level meters for `veilvoice live`, on a scale that means something.
//!
//! # Why the old one was wrong
//!
//! The first meter was linear: a peak of 0.5 filled half the bar. That is
//! arithmetically fine and useless as a meter, because loudness is not linear.
//! Ordinary speech recorded at a sensible level peaks around **-12 dBFS**,
//! which is 0.25 linear — three of twelve blocks. Somebody speaking normally
//! saw a meter that looked like near-silence, and the only way to fill the bar
//! was to be clipping.
//!
//! Every real meter is logarithmic for that reason, and this one is too:
//! -60 dBFS at the left, 0 dBFS at the right. Speech now sits in the middle of
//! the bar where a person can see it move.
//!
//! # What it measures, and what it does not
//!
//! **Sample peak, since the last read.** `veilvoice-audio` keeps the largest
//! absolute sample seen since the meter was last looked at and resets it on
//! read, so nothing between two reads is missed.
//!
//! It is **not** a loudness meter. RMS, LUFS and everything else that
//! correlates with how loud a thing *sounds* need a window and a weighting
//! curve, and they answer a different question: this one is for "am I being
//! recorded, and am I clipping", which is a peak question.
//!
//! It also cannot see an **inter-sample peak** — a waveform that passes above
//! full scale between two samples and clips in a converter or an encoder
//! without any single sample exceeding 1.0. Catching those needs oversampling.
//! The meter says `CLIP` when a sample actually reaches full scale, and says
//! nothing about the ones it cannot see, which is the honest half of a true
//! peak meter rather than a claim to be one.
//!
//! # Peak hold
//!
//! A bar that only shows the current moment cannot show a transient: the loud
//! syllable is gone before a human eye finishes moving. The highest level of
//! the last [`HOLD`] is kept and drawn as a single marker, and it decays rather
//! than sticking, so the bar stays honest about what is happening *now* while
//! still showing what just happened.

use crate::theme::{colour, paint};
use std::time::{Duration, Instant};

/// How long a peak marker is held before it falls back.
pub const HOLD: Duration = Duration::from_millis(1500);

/// The quietest level the bar shows. Below this is drawn as silence.
///
/// Sixty decibels is the range a person can usefully read off twelve
/// characters. A meter that went to -90 would spend a third of itself on room
/// tone.
pub const FLOOR_DB: f32 = -60.0;

/// At or above this, the level is called clipping.
///
/// -0.1 dBFS rather than exactly 0. A sample that reaches 32767 in a 16-bit
/// file is already at full scale and the next one up does not exist, so
/// waiting for a mathematically perfect 1.0 means never saying `CLIP` about a
/// signal that is plainly clipped.
pub const CLIP_DB: f32 = -0.1;

/// Level in decibels relative to full scale.
///
/// Silence is [`FLOOR_DB`] rather than negative infinity: the caller wants a
/// number to place on a bar, and a meter is not the place to introduce an
/// infinity into arithmetic that has to keep running.
///
/// # The two ways a reading can be nonsense, answered differently
///
/// **NaN** is not a level at all — nothing can be inferred from it, and it is
/// read as silence.
///
/// **Positive infinity** is read as **full scale**, not as silence. Both are
/// wrong readings, and a meter should be wrong in the direction that gets
/// looked at: a broken meter pinned at the top is noticed in a second, and one
/// pinned at the bottom looks exactly like a microphone that is not plugged in.
/// The engine cannot produce either, and it is not the engine this guards
/// against — it is whatever produces the samples.
pub fn dbfs(peak: f32) -> f32 {
    if peak.is_nan() || peak <= 0.0 {
        return FLOOR_DB;
    }
    (20.0 * peak.clamp(0.0, 1.0).log10()).max(FLOOR_DB)
}

/// Where a level sits along the bar, from 0.0 at the floor to 1.0 at full
/// scale.
pub fn position(peak: f32) -> f32 {
    ((dbfs(peak) - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// The eighth-block characters, so a bar of `n` characters has `8n` steps.
///
/// Twelve characters at one step each is a meter that moves in jumps of five
/// decibels, which reads as broken rather than as coarse. The same twelve
/// characters at eighths move in jumps of well under one.
const EIGHTHS: [&str; 9] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

/// One meter: the bar, the peak marker, and the number.
///
/// `width` is in characters, and does not include the number after it.
pub fn render(peak: f32, hold: f32, width: usize) -> String {
    let filled = position(peak) * width as f32;
    let whole = filled.floor() as usize;
    let part = ((filled - filled.floor()) * 8.0).round() as usize;

    let mut bar = String::new();
    for index in 0..width {
        // The held peak is drawn as a marker in the empty part of the bar, and
        // is simply invisible inside the filled part -- where it would be
        // saying the same thing as the fill.
        let marker = hold > 0.0 && (position(hold) * width as f32).floor() as usize == index;
        if index < whole {
            bar.push_str(EIGHTHS[8]);
        } else if index == whole && part > 0 {
            bar.push_str(EIGHTHS[part.min(8)]);
        } else if marker {
            bar.push('╵');
        } else {
            bar.push('·');
        }
    }

    let db = dbfs(peak);
    let shade = if db >= CLIP_DB {
        colour::RED
    } else if db >= -6.0 {
        colour::YELLOW
    } else if db >= -40.0 {
        colour::GREEN
    } else {
        // Below -40 the signal is room tone, not speech. Drawn muted so a quiet
        // room does not read as a working microphone.
        colour::MUTED
    };

    let number = if db <= FLOOR_DB {
        "  -inf".to_string()
    } else {
        format!("{db:6.1}")
    };
    format!(
        "{} {}",
        paint(shade, &bar),
        paint(
            if db >= CLIP_DB {
                colour::RED
            } else {
                colour::MUTED
            },
            &number
        )
    )
}

/// One channel's meter, keeping the peak between reads.
pub struct Channel {
    hold: f32,
    since: Instant,
    clipped: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            hold: 0.0,
            since: Instant::now(),
            clipped: false,
        }
    }
}

impl Channel {
    /// Take a new reading and give back the meter to print.
    pub fn update(&mut self, peak: f32, width: usize) -> String {
        // NaN only. Infinity is left alone so it reaches `dbfs`, which pins it
        // to full scale -- reading it as silence here would have contradicted
        // the reasoning three functions up, quietly, in the one place that
        // decides what the user actually sees.
        let peak = if peak.is_nan() { 0.0 } else { peak.max(0.0) };
        if peak >= self.hold || self.since.elapsed() >= HOLD {
            self.hold = peak;
            self.since = Instant::now();
        }
        if dbfs(peak) >= CLIP_DB {
            self.clipped = true;
        }
        render(peak, self.hold, width)
    }

    /// Whether this channel has clipped at any point in the session.
    ///
    /// Sticky on purpose. Clipping is destructive and it is over in a
    /// millisecond; a warning that disappears before the person looks up is a
    /// warning that was never given.
    pub fn has_clipped(&self) -> bool {
        self.clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_is_zero_and_silence_is_the_floor() {
        assert!((dbfs(1.0) - 0.0).abs() < 0.001);
        assert_eq!(dbfs(0.0), FLOOR_DB);
        assert_eq!(dbfs(-1.0), FLOOR_DB, "a negative peak is not a level");
        assert_eq!(dbfs(f32::NAN), FLOOR_DB, "nothing can be read from NaN");
        assert_eq!(dbfs(f32::NEG_INFINITY), FLOOR_DB);
        // Wrong in the direction that gets looked at. See the note on `dbfs`.
        assert_eq!(dbfs(f32::INFINITY), 0.0, "a broken meter should pin high");
    }

    /// Halving the amplitude is six decibels, and this is the check that the
    /// scale is a decibel scale rather than something that merely curves.
    #[test]
    fn halving_the_amplitude_is_six_decibels() {
        let full = dbfs(1.0);
        let half = dbfs(0.5);
        assert!((full - half - 6.0206).abs() < 0.01, "{full} to {half}");
        assert!((dbfs(0.5) - dbfs(0.25) - 6.0206).abs() < 0.01);
    }

    /// The defect this file exists to fix, stated as a test.
    ///
    /// Speech at a sensible recording level peaks near -12 dBFS. On the linear
    /// meter that filled a quarter of the bar and read as near-silence. On this
    /// one it sits in the middle, where a person can see it move.
    #[test]
    fn ordinary_speech_lands_in_the_middle_of_the_bar() {
        let speech = 0.251; // -12 dBFS
        let where_it_sits = position(speech);
        assert!(
            (0.7..0.85).contains(&where_it_sits),
            "-12 dBFS sits at {where_it_sits:.3} of the bar"
        );
        // What it used to do, for the record.
        assert!(speech < 0.3, "the linear meter filled under a third");
    }

    #[test]
    fn the_bar_is_the_width_it_was_asked_for() {
        for peak in [0.0f32, 0.001, 0.25, 0.5, 0.9, 1.0, 2.0, -1.0, f32::NAN] {
            for width in [1usize, 8, 12, 40] {
                let text = render(peak, 0.0, width);
                let bar: String = text.chars().filter(|c| !c.is_ascii()).collect();
                assert_eq!(
                    bar.chars().count(),
                    width,
                    "peak {peak} at width {width} drew {bar:?}"
                );
            }
        }
    }

    #[test]
    fn full_scale_fills_it_and_silence_empties_it() {
        let full = render(1.0, 0.0, 12);
        assert_eq!(full.matches('█').count(), 12);
        let quiet = render(0.0, 0.0, 12);
        assert_eq!(quiet.matches('·').count(), 12);
        assert!(quiet.contains("-inf"));
    }

    /// A held peak is drawn as a marker, and only where the bar is empty --
    /// inside the fill it would be saying what the fill already says.
    #[test]
    fn a_held_peak_shows_as_a_marker_beyond_the_current_level() {
        let text = render(0.01, 0.9, 12);
        assert!(text.contains('╵'), "the held peak should be marked: {text}");
        let covered = render(0.9, 0.9, 12);
        assert!(
            !covered.contains('╵'),
            "no marker inside the fill: {covered}"
        );
    }

    /// The hold falls back rather than sticking, or the meter slowly becomes a
    /// picture of the loudest thing that ever happened.
    #[test]
    fn the_hold_rises_at_once_and_falls_back_after_a_while() {
        let mut channel = Channel::default();
        channel.update(0.9, 12);
        assert!((channel.hold - 0.9).abs() < 1e-6);
        // A quieter reading does not move it...
        channel.update(0.1, 12);
        assert!((channel.hold - 0.9).abs() < 1e-6);
        // ...until the hold has expired.
        channel.since = Instant::now() - HOLD - Duration::from_millis(1);
        channel.update(0.1, 12);
        assert!((channel.hold - 0.1).abs() < 1e-6);
    }

    /// Clipping is sticky: it is over in a millisecond and it is destructive.
    #[test]
    fn clipping_is_remembered_for_the_session() {
        let mut channel = Channel::default();
        assert!(!channel.has_clipped());
        channel.update(0.5, 12);
        assert!(!channel.has_clipped(), "-6 dBFS is not clipping");
        channel.update(0.95, 12);
        assert!(!channel.has_clipped(), "-0.45 dBFS is not clipping either");
        channel.update(1.0, 12);
        assert!(channel.has_clipped());
        channel.update(0.0, 12);
        assert!(
            channel.has_clipped(),
            "a warning that vanishes was never given"
        );
    }

    /// -0.1 dBFS counts, because a sample at full scale in a 16-bit file has
    /// no larger neighbour to reach.
    ///
    /// The numbers here were checked rather than assumed, and the first version
    /// of this test had them wrong: a *linear* 0.99 is -0.087 dBFS, which is
    /// already inside a tenth of a decibel of full scale. Decibels near the top
    /// of the scale are much finer than they look in linear terms, which is
    /// most of the reason a linear meter is a bad meter.
    #[test]
    fn the_clip_threshold_is_just_below_full_scale() {
        assert!(dbfs(0.9) < CLIP_DB, "0.9 is -0.9 dBFS, not clipping");
        assert!(dbfs(0.98) < CLIP_DB, "0.98 is -0.18 dBFS, not clipping");
        assert!(dbfs(0.99) >= CLIP_DB, "0.99 is -0.087 dBFS, which is");
        assert!(dbfs(1.0) >= CLIP_DB);
    }

    #[test]
    fn nothing_here_panics_on_anything() {
        for peak in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -5.0, 1e30, 0.0] {
            for hold in [f32::NAN, 0.0, 1.0, -1.0] {
                let _ = render(peak, hold, 12);
            }
            let mut channel = Channel::default();
            let _ = channel.update(peak, 12);
        }
        let _ = render(0.5, 0.5, 0);
    }
}
