// SPDX-License-Identifier: GPL-3.0-or-later
//! Turning a plan and a recording into veiled audio, one engine per speaker.
//!
//! # One engine each, and why that is not merely convenient
//!
//! Every speaker gets their own [`veilvoice_core::Deidentifier`], built with
//! their slot's destination voice and **its own seed**. Three consequences,
//! and all three are the point:
//!
//! * Each speaker arrives at their own canonical register and vocal tract, so
//!   the output is followable.
//! * Each speaker's modulation stream is independent, so the ratchet in one
//!   voice tells an adversary nothing about another.
//! * Each speaker's engine keeps its state **across their own turns**. The
//!   accent neutraliser needs a few seconds to measure a speaker before its
//!   corrections reach full strength; carrying that across the turns of one
//!   person means it converges once, rather than warming up again every time
//!   they take a breath.
//!
//! # A speaker needs a few seconds before they arrive at their voice
//!
//! The accent neutraliser ramps its corrections in over
//! [`veilvoice_core::WARMUP_S`] of **voiced** audio, from nothing. Until it has
//! finished, a speaker is only partly moved toward their destination register --
//! so a slot that should sound like 187 Hz sounds like something between the
//! original speaker and 187 Hz.
//!
//! This was found by measuring the fundamental of a rendered file rather than
//! by reading the code: ten slots given one second each came out at three
//! distinct pitches, and the same ten given five seconds each came out at four,
//! exactly where the table says. Nothing was wrong with the table the second
//! time; the first measurement was of the ramp.
//!
//! Keeping one engine per speaker across all of their turns is what makes this
//! bearable -- the ramp happens **once per speaker for the whole recording**
//! rather than once per turn. A speaker whose total time is shorter than the
//! ramp never finishes it, so [`Rendered::notes`] says so by name.
//!
//! What is **not** affected: the phase discard and the CSPRNG modulation are
//! unconditional and full strength from the first frame. Those are the two
//! reasons the transform is one-way. The ramp weakens the *normalisation onto a
//! canonical register* early on, which costs distinguishability between
//! speakers, and leaves some of the original pitch contour in the first couple
//! of seconds.
//!
//! # Audio nobody claimed is silenced, and the amount is reported
//!
//! A gap between turns is a span the plan does not assign to anybody. It is
//! **silenced**, never passed through.
//!
//! That is the one decision in this file that is not a trade-off. Passing
//! unassigned audio through unveiled would put somebody's real voice into a
//! file whose entire purpose is that it contains no real voice — because of a
//! gap in a text file, silently, in the middle of an otherwise veiled
//! recording. Silence loses content and can be seen; a raw voice cannot be
//! unheard.
//!
//! [`Rendered::unassigned_secs`] says how much went, so a plan with a hole in
//! it is a thing you find out about rather than a thing you notice later.
//!
//! # Latency is removed, so a turn lands where the plan said
//!
//! The engine has a fixed algorithmic latency — the STFT cannot emit a sample
//! until it has a frame around it. Each span is therefore processed with a tail
//! of silence and the first `latency_samples` of output are dropped, which puts
//! the veiled audio back exactly where the original was. Without this every
//! turn would drift later than the subtitle describing it, by about 16 ms at
//! the default frame size, and a subtitle 16 ms late is a subtitle that looks
//! wrong.
//!
//! # Boundaries are faded, because a splice is a click
//!
//! Cutting audio at an arbitrary sample and starting different audio there
//! produces a step, and a step is broadband noise. Each rendered span is faded
//! in and out over a few milliseconds. Short enough not to swallow a syllable,
//! long enough to remove the click.
//!
//! # Overlaps are mixed, and the mixing is admitted
//!
//! Two people talking at once is two engines writing into the same samples, so
//! their outputs are summed. A sum can exceed full scale; when it does, the
//! whole output is scaled down by one factor and [`Rendered::gain_applied`]
//! records it. One factor for the whole file rather than a limiter that acts
//! only where it clipped, because a limiter changes the relative loudness of
//! the speakers and this crate has just spent considerable effort making them
//! distinguishable.

use crate::{Conversation, Error};
use veilvoice_core::{DeidConfig, Deidentifier};

/// How to render.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// The engine settings every speaker starts from.
    ///
    /// Each speaker gets a copy with their slot's destination voice applied to
    /// the accent configuration; nothing else is changed, so the user's
    /// intensity and strengths apply to all of them equally.
    pub config: DeidConfig,
    /// The fade at each end of a rendered span, in milliseconds.
    pub fade_ms: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            config: DeidConfig::default(),
            // Long enough to remove a splice click, short enough that it cannot
            // swallow a consonant. A plosive is around 5-15 ms.
            fade_ms: 4.0,
        }
    }
}

/// What came back.
#[derive(Clone, Debug)]
pub struct Rendered {
    /// The veiled audio, the same length as the input.
    pub samples: Vec<f32>,
    /// How long each speaker was given, in seconds, by slot.
    pub per_speaker_secs: Vec<f64>,
    /// How much of the recording no turn claimed, in seconds.
    ///
    /// **Silenced, not passed through.** A non-zero figure here means the plan
    /// has a hole in it, and a front end should say so rather than leaving it
    /// in a struct nobody reads.
    pub unassigned_secs: f64,
    /// The factor the whole output was scaled by to keep it inside full scale.
    ///
    /// `1.0` means nothing was scaled. Below that, overlapping speech summed
    /// past full scale and everything was brought down by one factor.
    pub gain_applied: f32,
    /// Anything worth telling the user about what was rendered.
    pub notes: Vec<String>,
}

impl Rendered {
    /// Whether some of the recording was silenced because no turn claimed it.
    pub fn has_unassigned(&self) -> bool {
        self.unassigned_secs > 0.0
    }
}

/// Render `input` according to `plan`.
///
/// `seeds` supplies one seed per speaker for a deterministic render; `None`
/// draws each from the OS CSPRNG, which is what a front end should do. The
/// deterministic form exists so this can be tested at all — a de-identifier
/// whose output cannot be reproduced cannot be checked.
pub fn render(
    plan: &Conversation,
    input: &[f32],
    settings: &Settings,
    seeds: Option<&[[u8; 32]]>,
) -> Result<Rendered, Error> {
    if plan.is_empty() {
        return Err(Error::Malformed(
            "the plan has no speakers, so there is nothing to render into".into(),
        ));
    }
    let sample_rate = settings.config.sample_rate;
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err(Error::Malformed(format!(
            "{sample_rate} is not a sample rate"
        )));
    }
    if let Some(seeds) = seeds {
        if seeds.len() != plan.len() {
            return Err(Error::Malformed(format!(
                "{} seeds were given for {} speakers",
                seeds.len(),
                plan.len()
            )));
        }
    }

    // One engine per speaker, each with their own destination voice and their
    // own stream. Built once and kept, so a speaker's accent neutraliser
    // converges over the whole recording rather than per turn.
    let mut engines = Vec::with_capacity(plan.len());
    for slot in 0..plan.len() {
        let mut config = settings.config;
        config.accent = plan.voice(slot).applied_to(config.accent);
        let engine = match seeds {
            Some(seeds) => Deidentifier::from_seed(config, seeds[slot]),
            None => Deidentifier::new(config),
        }
        .map_err(Error::Malformed)?;
        engines.push(engine);
    }

    let mut output = vec![0.0f32; input.len()];
    let mut claimed = vec![false; input.len()];
    let mut per_speaker_secs = vec![0.0f64; plan.len()];
    let mut notes = Vec::new();

    let fade = ((settings.fade_ms / 1000.0) * sample_rate).round().max(1.0) as usize;

    for turn in plan.turns() {
        let start = seconds_to_index(turn.start, sample_rate, input.len());
        let end = seconds_to_index(turn.end, sample_rate, input.len());
        if end <= start {
            // A turn entirely past the end of the audio. Reported rather than
            // ignored: a plan describing thirty seconds of a ten-second file
            // is a plan somebody should look at.
            notes.push(format!(
                "the turn at {:.3}s to {:.3}s lies outside a recording {:.3}s long, and \
                 rendered nothing",
                turn.start,
                turn.end,
                input.len() as f64 / sample_rate as f64
            ));
            continue;
        }

        let span = &input[start..end];
        let veiled = process_span(&mut engines[turn.speaker], span);
        let faded = fade_ends(veiled, fade);
        for (offset, sample) in faded.iter().enumerate() {
            // Summed, not overwritten: two people talking at once is two
            // engines writing here, and picking a winner would delete one of
            // them from the recording.
            output[start + offset] += sample;
            claimed[start + offset] = true;
        }
        per_speaker_secs[turn.speaker] += (end - start) as f64 / sample_rate as f64;
    }

    // A speaker who never gets enough audio to finish the accent ramp never
    // arrives at their destination register, so they are both less veiled and
    // less distinguishable than the interface implies. Said by name rather
    // than left in a struct.
    for (slot, seconds) in per_speaker_secs.iter().enumerate() {
        if *seconds > 0.0 && (*seconds as f32) < veilvoice_core::WARMUP_S * 2.0 {
            notes.push(format!(
                "{} speaks for only {seconds:.2}s in total. The accent neutraliser ramps \
                 in over {:.2}s of voiced audio, so they may not reach their destination \
                 register at all -- they will be less distinct from the other speakers, \
                 and some of their own pitch contour survives. The phase discard and the \
                 modulation are unaffected and full strength throughout.",
                plan.speakers()[slot].name,
                veilvoice_core::WARMUP_S,
            ));
        }
    }

    let unassigned = claimed.iter().filter(|c| !**c).count();
    let unassigned_secs = unassigned as f64 / sample_rate as f64;

    // Everything nobody claimed is already zero, because `output` started that
    // way and only a turn ever writes into it. Said explicitly because the
    // alternative -- copying the input across and letting turns overwrite it --
    // is the obvious implementation and would leak a real voice through every
    // gap in the plan.
    if unassigned > 0 {
        notes.push(format!(
            "{unassigned_secs:.3}s of the recording is not claimed by any turn and was \
             silenced. It was not passed through: audio nobody assigned to a speaker has \
             not been veiled, and a gap in a plan must not put a real voice into the \
             result."
        ));
    }

    let peak = output.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
    let mut gain_applied = 1.0f32;
    if peak > 1.0 && peak.is_finite() {
        gain_applied = 1.0 / peak;
        for sample in &mut output {
            *sample *= gain_applied;
        }
        notes.push(format!(
            "overlapping speech summed to {peak:.2} of full scale, so the whole render \
             was scaled by {gain_applied:.3}. One factor for the file rather than a \
             limiter, which would have changed how loud each speaker is relative to the \
             others."
        ));
    }

    Ok(Rendered {
        samples: output,
        per_speaker_secs,
        unassigned_secs,
        gain_applied,
        notes,
    })
}

/// A time in seconds as a sample index, clamped into the recording.
fn seconds_to_index(seconds: f64, sample_rate: f32, len: usize) -> usize {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    let index = (seconds * sample_rate as f64).round();
    if index >= len as f64 {
        len
    } else {
        index as usize
    }
}

/// Run one span through one engine and give back audio aligned with the input.
///
/// The engine cannot emit a sample until it has a frame around it, so the span
/// is followed by a tail of silence and the leading `latency_samples` of output
/// are dropped. What comes back lines up with what went in.
fn process_span(engine: &mut Deidentifier, span: &[f32]) -> Vec<f32> {
    let latency = engine.latency_samples();
    let mut padded = Vec::with_capacity(span.len() + latency);
    padded.extend_from_slice(span);
    padded.resize(span.len() + latency, 0.0);

    let mut processed = vec![0.0f32; padded.len()];
    engine.process(&padded, &mut processed);

    let mut aligned = processed.split_off(latency.min(processed.len()));
    aligned.resize(span.len(), 0.0);
    aligned
}

/// Fade the first and last `fade` samples, so a splice is not a click.
fn fade_ends(mut samples: Vec<f32>, fade: usize) -> Vec<f32> {
    let len = samples.len();
    // A span shorter than two fades gets one fade over its whole length rather
    // than two overlapping ones, which would multiply the middle by a number
    // smaller than either.
    let fade = fade.min(len / 2).max(1).min(len);
    for index in 0..fade {
        let gain = index as f32 / fade as f32;
        samples[index] *= gain;
        samples[len - 1 - index] *= gain;
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Speaker, Turn};

    const RATE: f32 = 48_000.0;

    fn settings() -> Settings {
        Settings {
            config: DeidConfig {
                sample_rate: RATE,
                ..DeidConfig::default()
            },
            ..Settings::default()
        }
    }

    /// A tone, so a span that was processed is obviously not silence.
    fn tone(seconds: f64) -> Vec<f32> {
        let len = (seconds * RATE as f64) as usize;
        (0..len)
            .map(|i| {
                let t = i as f32 / RATE;
                0.4 * (2.0 * std::f32::consts::PI * 150.0 * t).sin()
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn two_people(gap: bool) -> Conversation {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Alex")).unwrap();
        plan.add_speaker(Speaker::named("Sam")).unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 1.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        plan.add_turn(Turn {
            start: if gap { 2.0 } else { 1.0 },
            end: 3.0,
            speaker: 1,
            text: None,
        })
        .unwrap();
        plan
    }

    fn seeds(count: usize) -> Vec<[u8; 32]> {
        (0..count).map(|i| [i as u8 + 1; 32]).collect()
    }

    #[test]
    fn the_output_is_the_same_length_as_the_input() {
        let input = tone(3.0);
        let rendered = render(&two_people(false), &input, &settings(), Some(&seeds(2))).unwrap();
        assert_eq!(rendered.samples.len(), input.len());
    }

    /// The one decision in this file that is not a trade-off.
    #[test]
    fn audio_no_turn_claimed_is_silenced_and_never_passed_through() {
        let input = tone(3.0);
        let rendered = render(&two_people(true), &input, &settings(), Some(&seeds(2))).unwrap();

        // The gap is 1.0s to 2.0s, and the input there is a loud tone.
        let gap_start = RATE as usize;
        let gap_end = 2 * RATE as usize;
        let gap = &rendered.samples[gap_start..gap_end];
        assert_eq!(
            rms(gap),
            0.0,
            "a gap in the plan let unveiled audio through"
        );
        assert!(
            rms(&input[gap_start..gap_end]) > 0.1,
            "the input was loud there"
        );

        assert!(rendered.has_unassigned());
        assert!(
            (rendered.unassigned_secs - 1.0).abs() < 0.01,
            "{}",
            rendered.unassigned_secs
        );
        let note = rendered.notes.join(" ");
        assert!(note.contains("was silenced"), "{note}");
        assert!(note.contains("not passed through"), "{note}");
    }

    /// A plan with no gaps leaves nothing unassigned.
    #[test]
    fn a_plan_that_covers_the_recording_reports_no_gap() {
        let rendered =
            render(&two_people(false), &tone(3.0), &settings(), Some(&seeds(2))).unwrap();
        assert!(!rendered.has_unassigned());
        assert_eq!(rendered.unassigned_secs, 0.0);
    }

    /// Each speaker must actually be processed, and their spans must carry
    /// audio rather than silence.
    #[test]
    fn every_speakers_span_carries_veiled_audio() {
        let rendered =
            render(&two_people(false), &tone(3.0), &settings(), Some(&seeds(2))).unwrap();
        let first = &rendered.samples[(0.2 * RATE as f64) as usize..(0.8 * RATE as f64) as usize];
        let second = &rendered.samples[(1.2 * RATE as f64) as usize..(2.8 * RATE as f64) as usize];
        assert!(rms(first) > 0.001, "the first speaker rendered silence");
        assert!(rms(second) > 0.001, "the second speaker rendered silence");
    }

    /// Two speakers must not come out as the same audio. If they did, the whole
    /// crate would be pointless.
    #[test]
    fn two_speakers_do_not_render_alike() {
        let input = tone(2.0);
        let mut alone = Conversation::new();
        alone.add_speaker(Speaker::named("Alex")).unwrap();
        alone
            .add_turn(Turn {
                start: 0.0,
                end: 2.0,
                speaker: 0,
                text: None,
            })
            .unwrap();
        let as_first = render(&alone, &input, &settings(), Some(&[[1u8; 32]])).unwrap();

        let mut second = Conversation::new();
        second.add_speaker(Speaker::named("Alex")).unwrap();
        second.add_speaker(Speaker::named("Sam")).unwrap();
        second
            .add_turn(Turn {
                start: 0.0,
                end: 2.0,
                speaker: 1,
                text: None,
            })
            .unwrap();
        // The same seed for the speaker doing the talking, so the only
        // difference is the destination voice.
        let as_second =
            render(&second, &input, &settings(), Some(&[[9u8; 32], [1u8; 32]])).unwrap();

        let difference: f32 = as_first
            .samples
            .iter()
            .zip(&as_second.samples)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / as_first.samples.len() as f32;
        assert!(
            difference > 1e-4,
            "slot 0 and slot 1 produced near-identical audio ({difference})"
        );
    }

    /// The same seeds must give the same audio, or nothing here is testable.
    #[test]
    fn the_same_seeds_render_the_same_audio() {
        let input = tone(2.0);
        let a = render(&two_people(false), &input, &settings(), Some(&seeds(2))).unwrap();
        let b = render(&two_people(false), &input, &settings(), Some(&seeds(2))).unwrap();
        assert_eq!(a.samples, b.samples);
    }

    /// Different seeds must not, or the modulation is not doing anything.
    #[test]
    fn different_seeds_render_different_audio() {
        let input = tone(2.0);
        let a = render(&two_people(false), &input, &settings(), Some(&seeds(2))).unwrap();
        let b = render(
            &two_people(false),
            &input,
            &settings(),
            Some(&[[77u8; 32], [88u8; 32]]),
        )
        .unwrap();
        assert_ne!(a.samples, b.samples);
    }

    #[test]
    fn each_speaker_is_credited_with_their_own_time() {
        let rendered = render(&two_people(true), &tone(3.0), &settings(), Some(&seeds(2))).unwrap();
        assert_eq!(rendered.per_speaker_secs.len(), 2);
        assert!((rendered.per_speaker_secs[0] - 1.0).abs() < 0.01);
        assert!((rendered.per_speaker_secs[1] - 1.0).abs() < 0.01);
    }

    /// Overlapping speech is summed and then kept inside full scale by one
    /// factor for the whole file.
    #[test]
    fn an_overlap_is_mixed_and_kept_inside_full_scale() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Alex")).unwrap();
        plan.add_speaker(Speaker::named("Sam")).unwrap();
        for speaker in 0..2 {
            plan.add_turn(Turn {
                start: 0.0,
                end: 2.0,
                speaker,
                text: None,
            })
            .unwrap();
        }
        // Loud input, so two engines summing over the same span has a real
        // chance of leaving full scale.
        let input: Vec<f32> = tone(2.0).iter().map(|s| s * 2.0).collect();
        let rendered = render(&plan, &input, &settings(), Some(&seeds(2))).unwrap();
        let peak = rendered.samples.iter().fold(0.0f32, |p, s| p.max(s.abs()));
        assert!(peak <= 1.0 + 1e-5, "the render clipped at {peak}");
        assert!(rendered.gain_applied > 0.0 && rendered.gain_applied <= 1.0);
        assert!(!rendered.has_unassigned());
    }

    /// A splice must not click. A click is a step, and a step is broadband, so
    /// this checks that no single sample-to-sample jump is large.
    #[test]
    fn a_turn_boundary_does_not_step() {
        let rendered =
            render(&two_people(false), &tone(3.0), &settings(), Some(&seeds(2))).unwrap();
        let boundary = RATE as usize;
        let window = &rendered.samples[boundary - 32..boundary + 32];
        let biggest_step = window
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            biggest_step < 0.2,
            "a step of {biggest_step} at the boundary is a click"
        );
    }

    /// Everything the engine produces must be a real number, whatever the plan.
    #[test]
    fn every_rendered_sample_is_finite() {
        let rendered = render(&two_people(true), &tone(3.0), &settings(), Some(&seeds(2))).unwrap();
        assert!(rendered.samples.iter().all(|s| s.is_finite()));
    }

    /// A turn past the end of the audio is reported, not silently dropped.
    #[test]
    fn a_turn_beyond_the_recording_is_reported() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Alex")).unwrap();
        plan.add_turn(Turn {
            start: 30.0,
            end: 40.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        let rendered = render(&plan, &tone(1.0), &settings(), Some(&[[1u8; 32]])).unwrap();
        let note = rendered.notes.join(" ");
        assert!(note.contains("lies outside a recording"), "{note}");
        assert!(rendered.has_unassigned(), "and the whole file is unclaimed");
    }

    /// A speaker given less audio than the accent ramp needs is named, because
    /// they arrive somewhere short of their destination register and nothing
    /// in the audio shows it.
    #[test]
    fn a_speaker_with_too_little_audio_is_named() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Fleeting")).unwrap();
        plan.add_speaker(Speaker::named("Talkative")).unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 0.3,
            speaker: 0,
            text: None,
        })
        .unwrap();
        plan.add_turn(Turn {
            start: 0.3,
            end: 6.0,
            speaker: 1,
            text: None,
        })
        .unwrap();
        let rendered = render(&plan, &tone(6.0), &settings(), Some(&seeds(2))).unwrap();
        let notes = rendered.notes.join(" ");
        assert!(notes.contains("Fleeting speaks for only"), "{notes}");
        assert!(
            notes.contains("phase discard and the modulation are unaffected"),
            "the note must say what is *not* weakened: {notes}"
        );
        assert!(
            !notes.contains("Talkative speaks for only"),
            "a speaker with plenty of audio must not be warned about: {notes}"
        );
    }

    #[test]
    fn an_empty_plan_is_refused() {
        let error = render(&Conversation::new(), &tone(1.0), &settings(), None)
            .expect_err("nothing to render into");
        assert!(error.to_string().contains("no speakers"), "{error}");
    }

    #[test]
    fn the_wrong_number_of_seeds_is_refused() {
        let error = render(
            &two_people(false),
            &tone(1.0),
            &settings(),
            Some(&[[1u8; 32]]),
        )
        .expect_err("two speakers need two seeds");
        assert!(error.to_string().contains("2 speakers"), "{error}");
    }

    #[test]
    fn an_impossible_sample_rate_is_refused() {
        let mut broken = settings();
        broken.config.sample_rate = f32::NAN;
        assert!(render(&two_people(false), &tone(1.0), &broken, None).is_err());
    }

    /// An empty recording is not an error: a plan can legitimately describe a
    /// file that turned out to be empty, and the answer is an empty render.
    #[test]
    fn an_empty_recording_renders_to_nothing() {
        let rendered = render(&two_people(false), &[], &settings(), Some(&seeds(2))).unwrap();
        assert!(rendered.samples.is_empty());
        assert_eq!(rendered.unassigned_secs, 0.0);
    }

    /// A span shorter than two fades must not be multiplied down twice in the
    /// middle.
    #[test]
    fn a_very_short_span_is_faded_once_rather_than_twice() {
        let faded = fade_ends(vec![1.0; 8], 64);
        assert!(faded.iter().all(|s| *s <= 1.0 && *s >= 0.0));
        let middle = faded[faded.len() / 2];
        assert!(middle > 0.2, "the middle was faded to {middle}");
    }

    #[test]
    fn a_time_outside_the_recording_clamps_rather_than_panicking() {
        assert_eq!(seconds_to_index(-1.0, RATE, 100), 0);
        assert_eq!(seconds_to_index(f64::NAN, RATE, 100), 0);
        assert_eq!(seconds_to_index(1e9, RATE, 100), 100);
        assert_eq!(seconds_to_index(0.0, RATE, 100), 0);
    }
}
