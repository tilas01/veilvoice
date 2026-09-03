// SPDX-License-Identifier: GPL-3.0-or-later
//! Locking the window again after a period of no use.
//!
//! **Marker 92.** On at half an hour, and the delay is the user's to choose:
//! anything from five minutes to forty eight hours from a list, a number typed
//! in if none of those fit, and the ends of that range movable by anybody who
//! wants a shorter or longer one.
//!
//! # Why it is on by default, having been off
//!
//! It was off, on the argument that a lock engaging part way through a
//! recording is a lock that gets removed, and that VeilVoice cannot know
//! whether a given user is the one who walks away or the one who leaves a job
//! running -- so it should ask rather than assume.
//!
//! The asking is the part that was wrong. A default nobody is shown is not a
//! question, it is an answer, and the answer it gave was "no protection" to
//! everybody who never opened the settings tab. The people most helped by an
//! autolock are the least likely to go looking for one.
//!
//! So it is on, and the first-run setup shows it rather than leaving it to be
//! discovered: half an hour, with the choice and the off switch right there.
//! The original concern is answered by the delay rather than by the default --
//! thirty minutes of an untouched window is not somebody part way through
//! anything -- and by the fact that this has never counted a running job as
//! use, which is deliberate and explained below.
//!
//! # What counts as use
//!
//! Any keystroke, click, scroll or pointer movement over the window, which is
//! what egui reports as input. Deliberately **not** the passage of a job:
//! somebody who starts a long render and leaves the room has left the room,
//! and the recording they are producing is the thing worth locking away.
//!
//! The clock is egui's own frame time rather than the system clock, so moving
//! the machine's clock does not bring the lock forward or push it back. It also
//! means the countdown only advances while the window is being drawn, which is
//! the honest limit of this: a window nobody is drawing is a window nobody is
//! looking at, and it locks the moment it is looked at again.
//!
//! # In plain words
//!
//! Locks the window again if you have not touched it for half an hour.
//!
//! On to begin with, and setup shows you the switch. You pick how long, from
//! five minutes up to two days, or type your own, and you can turn it off.
//! Starting a long job does not count as using it: if you walk away while
//! something is rendering, that is exactly when you would want it locked.

use std::time::Duration;

/// The delay a fresh installation uses: half an hour.
///
/// Long enough that it does not interrupt somebody working, short enough to
/// matter if they walk out. It is the setup screen's suggestion as well as the
/// code's default, so the number a user is shown is the number they get.
pub const DEFAULT_SECS: u64 = 30 * 60;

/// The shortest delay the list offers, in seconds.
pub const FLOOR_SECS: u64 = 5 * 60;
/// The longest, in seconds. Forty eight hours.
pub const CEILING_SECS: u64 = 48 * 60 * 60;

/// The delays offered without typing anything, in seconds.
///
/// Chosen to be the ones people actually mean: a coffee, a lunch, an afternoon,
/// overnight, a weekend away. Anything else is typed.
pub const CHOICES: &[u64] = &[
    5 * 60,
    15 * 60,
    30 * 60,
    60 * 60,
    2 * 60 * 60,
    4 * 60 * 60,
    8 * 60 * 60,
    12 * 60 * 60,
    24 * 60 * 60,
    48 * 60 * 60,
];

/// How the autolock is configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Autolock {
    /// Whether it engages at all.
    pub enabled: bool,
    /// The delay, in seconds.
    pub after_secs: u64,
    /// The bottom of the range the user is offered.
    pub floor_secs: u64,
    /// The top of it.
    pub ceiling_secs: u64,
}

impl Default for Autolock {
    fn default() -> Self {
        Self {
            // On, at half an hour. See the module note for what changed.
            enabled: true,
            after_secs: DEFAULT_SECS,
            floor_secs: FLOOR_SECS,
            ceiling_secs: CEILING_SECS,
        }
    }
}

impl Autolock {
    /// Bring every field into a state that can be offered and obeyed.
    ///
    /// A settings file is editable, so every number that reaches here has been
    /// through somebody's text editor as far as this code knows. Rather than
    /// refuse, which would leave a user with an autolock they cannot fix from
    /// the interface, each value is brought back into range and the result is
    /// always something the interface can show.
    pub fn sane(mut self) -> Self {
        if self.floor_secs == 0 {
            self.floor_secs = FLOOR_SECS;
        }
        if self.ceiling_secs == 0 {
            self.ceiling_secs = CEILING_SECS;
        }
        // A range with its ends the wrong way round is a range with one end.
        if self.floor_secs > self.ceiling_secs {
            std::mem::swap(&mut self.floor_secs, &mut self.ceiling_secs);
        }
        self.after_secs = self.after_secs.clamp(self.floor_secs, self.ceiling_secs);
        self
    }

    /// The delay as a `Duration`.
    pub fn after(self) -> Duration {
        Duration::from_secs(self.after_secs)
    }

    /// Whether `idle` has reached the delay.
    ///
    /// False when the autolock is off, whatever `idle` says, so a caller cannot
    /// lock a window the user asked to leave unlocked by getting the condition
    /// the wrong way round.
    pub fn expired(self, idle: Duration) -> bool {
        self.enabled && idle >= self.after()
    }

    /// The delay in the words a person uses for it.
    pub fn describe(self) -> String {
        describe_secs(self.after_secs)
    }
}

/// A number of seconds as a phrase.
///
/// Whole units only, because every value this offers is a whole number of
/// minutes or hours and "1 hour 0 minutes" reads like a machine talking.
pub fn describe_secs(secs: u64) -> String {
    let plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("1 {unit}")
        } else {
            format!("{n} {unit}s")
        }
    };
    match secs {
        0 => "immediately".to_string(),
        s if s % 86_400 == 0 => plural(s / 86_400, "day"),
        s if s % 3_600 == 0 => plural(s / 3_600, "hour"),
        s if s % 60 == 0 => plural(s / 60, "minute"),
        s => plural(s, "second"),
    }
}

/// Read a delay somebody typed.
///
/// Accepts a bare number of minutes, or a number with a unit: `90`, `90m`,
/// `90 min`, `2h`, `2 hours`, `1d`. Returns `None` rather than guessing when it
/// cannot tell, so the interface can say it did not understand instead of
/// silently applying a number the user did not mean.
pub fn parse(typed: &str) -> Option<u64> {
    let typed = typed.trim().to_ascii_lowercase();
    if typed.is_empty() {
        return None;
    }
    let split = typed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(typed.len());
    let (number, unit) = typed.split_at(split);
    let value: f64 = number.parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let unit = unit.trim();
    let multiplier = match unit {
        // A bare number is minutes, because that is the unit somebody reaches
        // for when they say "lock after 20".
        "" | "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3_600.0,
        "d" | "day" | "days" => 86_400.0,
        _ => return None,
    };
    let secs = value * multiplier;
    // Guard the cast: a typed `99999999999999d` must not wrap into something
    // small and lock the window immediately.
    if !secs.is_finite() || secs < 1.0 || secs > u64::MAX as f64 {
        return None;
    }
    Some(secs as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_on_at_half_an_hour_out_of_the_box() {
        let auto = Autolock::default();
        assert!(
            auto.enabled,
            "the default is protection, not the absence of it"
        );
        assert_eq!(auto.after_secs, DEFAULT_SECS);
        assert_eq!(auto.after_secs, 30 * 60);
        assert!(
            !auto.expired(Duration::from_secs(29 * 60)),
            "half an hour means half an hour"
        );
        assert!(auto.expired(Duration::from_secs(31 * 60)));
    }

    #[test]
    fn switching_it_off_switches_it_off() {
        // The default changed; the ability to refuse it must not have.
        let auto = Autolock {
            enabled: false,
            ..Autolock::default()
        };
        assert!(
            !auto.expired(Duration::from_secs(u64::MAX / 2)),
            "a window must not lock itself when somebody has said not to"
        );
    }

    #[test]
    fn the_default_delay_is_one_of_the_offered_choices() {
        // Otherwise the settings tab opens showing a custom value nobody
        // typed, which reads as a setting that has been meddled with.
        assert!(CHOICES.contains(&DEFAULT_SECS));
    }

    #[test]
    fn it_locks_once_the_delay_has_passed_and_not_before() {
        let auto = Autolock {
            enabled: true,
            after_secs: 900,
            ..Default::default()
        };
        assert!(!auto.expired(Duration::from_secs(899)));
        assert!(auto.expired(Duration::from_secs(900)));
        assert!(auto.expired(Duration::from_secs(901)));
    }

    #[test]
    fn every_offered_choice_is_inside_the_range_it_is_offered_from() {
        for choice in CHOICES {
            assert!(*choice >= FLOOR_SECS, "{choice} is below the floor");
            assert!(*choice <= CEILING_SECS, "{choice} is above the ceiling");
        }
        assert_eq!(CHOICES.first(), Some(&FLOOR_SECS));
        assert_eq!(CHOICES.last(), Some(&CEILING_SECS));
    }

    /// The settings file is editable, so every number here has been through a
    /// text editor as far as this code knows.
    #[test]
    fn a_hand_edited_settings_file_is_brought_back_into_range() {
        let mad = Autolock {
            enabled: true,
            after_secs: 1,
            floor_secs: 0,
            ceiling_secs: 0,
        }
        .sane();
        assert_eq!(mad.floor_secs, FLOOR_SECS);
        assert_eq!(mad.ceiling_secs, CEILING_SECS);
        assert_eq!(mad.after_secs, FLOOR_SECS, "clamped up to the floor");

        let backwards = Autolock {
            enabled: true,
            after_secs: 3_600,
            floor_secs: 48 * 3_600,
            ceiling_secs: 300,
        }
        .sane();
        assert!(
            backwards.floor_secs < backwards.ceiling_secs,
            "ends swapped"
        );
        assert!(backwards.after_secs >= backwards.floor_secs);
    }

    /// The user may move the ends of the range, so a delay outside the default
    /// one is not an error.
    #[test]
    fn a_range_the_user_widened_is_kept() {
        let wide = Autolock {
            enabled: true,
            after_secs: 60,
            floor_secs: 30,
            ceiling_secs: 7 * 86_400,
        }
        .sane();
        assert_eq!(wide.after_secs, 60, "a minute is inside the widened range");
        assert_eq!(wide.floor_secs, 30);
        assert_eq!(wide.ceiling_secs, 7 * 86_400);
    }

    #[test]
    fn typed_delays_are_read_the_way_they_are_written() {
        assert_eq!(parse("20"), Some(20 * 60), "a bare number is minutes");
        assert_eq!(parse("20m"), Some(20 * 60));
        assert_eq!(parse("20 minutes"), Some(20 * 60));
        assert_eq!(parse("2h"), Some(2 * 3_600));
        assert_eq!(parse("2 hours"), Some(2 * 3_600));
        assert_eq!(parse("1d"), Some(86_400));
        assert_eq!(parse("90 sec"), Some(90));
        assert_eq!(parse("  3 HRS "), Some(3 * 3_600));
    }

    /// Refusing beats guessing: a number nobody meant, silently applied, is a
    /// window that locks at a time its owner cannot explain.
    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        for bad in ["", "   ", "soon", "-5", "0", "5 fortnights", "m", "1e999d"] {
            assert_eq!(parse(bad), None, "{bad:?} should not parse");
        }
    }

    /// A huge typed number must not wrap into a small one and lock the window
    /// immediately, which is the opposite of what was asked for.
    #[test]
    fn an_absurd_number_does_not_wrap_into_a_tiny_delay() {
        for absurd in ["99999999999999999999d", "1e30h"] {
            match parse(absurd) {
                None => {}
                Some(secs) => assert!(
                    secs > CEILING_SECS,
                    "{absurd:?} parsed as {secs}s, which is shorter than the ceiling"
                ),
            }
        }
    }

    #[test]
    fn a_delay_is_described_the_way_a_person_would_say_it() {
        assert_eq!(describe_secs(300), "5 minutes");
        assert_eq!(describe_secs(3_600), "1 hour");
        assert_eq!(describe_secs(2 * 3_600), "2 hours");
        assert_eq!(describe_secs(86_400), "1 day");
        assert_eq!(describe_secs(48 * 3_600), "2 days");
        assert_eq!(describe_secs(90), "90 seconds");
    }

    #[test]
    fn every_choice_has_a_readable_name() {
        for choice in CHOICES {
            let text = describe_secs(*choice);
            assert!(!text.is_empty());
            assert!(!text.contains("second"), "{text} should be minutes or more");
        }
    }
}
