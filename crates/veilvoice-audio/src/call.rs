// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 188.** A call, recorded one track per side.
//!
//! # What this is for
//!
//! A phone does not give its calls to a computer over a cable. Android has
//! refused call audio to ordinary apps since Android 9 and iOS has never
//! offered it. What does work is the call running *through* the computer:
//! Phone Link on Windows, an iPhone call taken on a Mac, a Linux machine acting
//! as a Bluetooth hands-free headset, or any calling program on the desktop.
//! Then the other side comes out of the computer's speakers and you go into its
//! microphone, and those are two things this crate can open separately.
//!
//! So a call here is a list of [`Party`]s, and each one is a device: you on a
//! [`Capture::Microphone`], the other side on [`Capture::Playback`], which is
//! the computer's own output recorded as it plays. A headset splitter cable
//! from a phone into an audio interface is the same shape with the other side
//! on a microphone input instead, which is why both are one type rather than
//! two features.
//!
//! # Why one track per side, and not a room
//!
//! [`room`](crate::room) mixes everybody into one output, which is what an
//! interview going out to a call wants. A call being recorded wants the
//! opposite. Mixing the other side into an output on this machine would play
//! them back into the speakers they are being recorded from, and every
//! choice the person makes is per person: you veiled and them as they are,
//! them recorded and you not at all. Those are only exact when each side is
//! its own signal from the first sample, so nothing here mixes.
//!
//! # Each side runs on its own clock, and that is allowed
//!
//! A room refuses devices that disagree on a rate, because it mixes them and
//! nothing in this crate resamples (F-168). A call does not mix, so each side
//! records at the rate its own device runs at and its engine is configured to
//! match. A microphone at 44.1 kHz and speakers at 48 kHz is an ordinary
//! laptop, and refusing it would refuse most calls for a reason that does not
//! apply.
//!
//! The one exception is listening. A side that is [`Party::heard`] is played
//! to the listening device as it arrives, and that is one output fed from
//! those sides, so they and it have to agree on a rate exactly as a room does.
//!
//! # The engine runs in the input callback
//!
//! There is no output to drive it, so each side's engine runs where that
//! side's samples arrive. The rule is the one [`live`](crate::live) states:
//! nothing in a callback allocates, locks or prints. Every buffer is sized
//! before the stream starts and a block longer than them is processed in
//! pieces rather than grown into.
//!
//! # Hearing the call, at the level it arrives
//!
//! When the call is on the computer it is already playing through the speakers,
//! and recording it changes nothing about that. When it arrives on a cable it
//! is not playing anywhere, so a side can be heard: copied, untouched and at
//! the level it arrived, to the listening device. Nothing scales it. What the
//! person hears then depends on their own volume control, which is theirs.
//!
//! # Nothing here tells anybody a call is being recorded
//!
//! Recording a side reads samples that are already on this machine. It sends
//! nothing to the call and changes nothing the other side can hear. Telling
//! them is a separate choice, made by the person, and it is not this module's.
//!
//! # In plain words
//!
//! On a call, you and the person you are talking to are recorded separately:
//! you from the microphone, them from what the computer plays. Each of you can
//! be disguised or left as you are, and kept or not kept at all. Nobody on the
//! call is told by this; it only listens to what is already here.

pub use crate::kinds::{Capture, GuestStats, RoomStats, MAX_GUESTS};
use crate::live::{Keeping, Kept};
use crate::Error;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use veilvoice_core::{DeidConfig, Deidentifier};

/// One side of a call: where their voice arrives, and what to do with it.
pub struct Party<'a> {
    /// The device their voice arrives on.
    pub device: &'a cpal::Device,
    /// Whether `device` is a microphone or the computer's own output recorded
    /// as it plays.
    pub capture: Capture,
    /// Their engine's settings. Only used when the veiled side is kept;
    /// `sample_rate` is overwritten with the rate their device runs at.
    pub config: DeidConfig,
    /// What to keep of them. Nothing kept is a real choice and means nothing
    /// of theirs is written anywhere: their level is still drawn, so somebody
    /// can see the call is arriving.
    pub keeping: Keeping,
    /// Whether they are played, untouched, to the listening device.
    pub heard: bool,
}

/// The recorders a call was asked for, one [`Kept`] per party in order.
#[derive(Default)]
pub struct KeptCall {
    /// One per party, in the order they were given.
    pub parties: Vec<Kept>,
}

/// A running call. Dropping it stops every stream.
pub struct CallSession {
    // The streams must outlive the session; dropping them stops the callbacks.
    _inputs: Vec<cpal::Stream>,
    _listen: Option<cpal::Stream>,
    shared: Arc<Shared>,
}

/// Counted in the audio callbacks, read by the caller. The same arrangement a
/// room uses, and the same statistics type, so a window that draws a room's
/// bars draws a call's.
#[derive(Default)]
struct Shared {
    stats: Mutex<RoomStats>,
    starved: AtomicU64,
    troubles: crate::live::Troubles,
}

/// Why a call cannot be recorded as it was described, or `None` when it can.
///
/// Pure, so it is decided and tested without a sound card. Each party is its
/// capture and the name of its device, `None` being the default for that
/// direction; `listen` is the listening device's name when there is one and
/// any party is heard.
///
/// Two things are refused. Two parties on the same device are one signal, which
/// nothing here can separate. And listening on the very output a party is being
/// recorded from plays them back into their own recording, twice over and
/// slightly late, which is an echo built on purpose.
pub fn refusal(
    parties: &[(Capture, Option<&str>)],
    listen: Option<Option<&str>>,
) -> Option<String> {
    if parties.is_empty() {
        return Some("a call with nobody on it has nothing to record".into());
    }
    if parties.len() > MAX_GUESTS {
        return Some(format!(
            "{} sides were given and a call records at most {MAX_GUESTS}",
            parties.len()
        ));
    }
    for (first, (capture, device)) in parties.iter().enumerate() {
        for (other_capture, other_device) in parties.iter().skip(first + 1) {
            if capture == other_capture && device == other_device {
                return Some(format!(
                    "two sides of this call are both on {}. One device is one signal, and \
                     nothing here can separate it again: give each side its own.",
                    where_it_is(*capture, *device)
                ));
            }
        }
    }
    if let Some(listening) = listen {
        let echo = parties
            .iter()
            .any(|(capture, device)| *capture == Capture::Playback && *device == listening);
        if echo {
            return Some(format!(
                "the call would be heard on {}, which is the output one side is being \
                 recorded from. They would be recorded twice, the second time late. A call \
                 on this computer is already playing there: turn listening off, or listen \
                 on headphones.",
                where_it_is(Capture::Playback, listening)
            ));
        }
    }
    None
}

/// A device as a sentence names it.
fn where_it_is(capture: Capture, device: Option<&str>) -> String {
    match (capture, device) {
        (_, Some(name)) => format!("{name:?}"),
        (Capture::Microphone, None) => "this machine's default microphone".into(),
        (Capture::Playback, None) => "this machine's default output".into(),
    }
}

/// The configuration a party's stream opens with.
///
/// A microphone is asked for its input configuration. The computer's output is
/// asked for its *output* configuration and then opened as an input, which is
/// how cpal records what an output plays: WASAPI on Windows and Core Audio on
/// macOS 14.6 and later do it for any output device. On a host that cannot, the
/// open fails, and the words for that are [`no_playback_capture`].
fn default_config(
    device: &cpal::Device,
    capture: Capture,
) -> Result<cpal::SupportedStreamConfig, Error> {
    match capture {
        Capture::Microphone => device.default_input_config(),
        Capture::Playback => device.default_output_config(),
    }
    .map_err(|e| Error::Device(e.to_string()))
}

/// What a party's device reports it will take, as plain numbers.
fn ranges(device: &cpal::Device, capture: Capture) -> Result<Vec<(u32, u32)>, Error> {
    match capture {
        Capture::Microphone => crate::live::input_ranges(device),
        Capture::Playback => Ok(device
            .supported_output_configs()
            .map_err(|e| Error::Device(e.to_string()))?
            .map(|range| (range.min_sample_rate(), range.max_sample_rate()))
            .collect()),
    }
}

/// The configuration a party's device offers at `rate`.
fn config_at(
    device: &cpal::Device,
    capture: Capture,
    rate: u32,
) -> Result<Option<cpal::SupportedStreamConfig>, Error> {
    Ok(match capture {
        Capture::Microphone => crate::live::at_rate(
            device
                .supported_input_configs()
                .map_err(|e| Error::Device(e.to_string()))?,
            rate,
        ),
        Capture::Playback => crate::live::at_rate(
            device
                .supported_output_configs()
                .map_err(|e| Error::Device(e.to_string()))?,
            rate,
        ),
    })
}

/// Why recording what this computer plays did not open, in words.
///
/// The platform's own error is kept beside it, because it is the part a person
/// can search for. Linux is named because it is the one place the answer is a
/// setting rather than a version: PipeWire and PulseAudio offer every output as
/// a "Monitor of" source, which is an ordinary input and is picked as one.
pub fn no_playback_capture(said: &str) -> String {
    format!(
        "this computer would not let its own output be recorded ({said}). Windows does \
         this for any output, and macOS from 14.6. On Linux, record the other side from \
         the input called \"Monitor of\" your speakers instead, which PipeWire and \
         PulseAudio both offer."
    )
}

impl CallSession {
    /// Open every party's device and keep what each was asked to keep.
    ///
    /// `listen` is where the heard parties play, and is ignored when nobody is
    /// heard. The checks [`refusal`] makes are the caller's to make first,
    /// because only the caller knows the devices' names as the person chose
    /// them; this refuses an empty call and more than [`MAX_GUESTS`] again
    /// rather than trusting that.
    pub fn start(
        parties: &[Party<'_>],
        listen: Option<&cpal::Device>,
    ) -> Result<(Self, KeptCall), Error> {
        if parties.is_empty() || parties.len() > MAX_GUESTS {
            return Err(Error::Device(format!(
                "a call records between one and {MAX_GUESTS} sides, and {} were given",
                parties.len()
            )));
        }

        let mut configs = Vec::with_capacity(parties.len());
        for party in parties {
            configs.push(default_config(party.device, party.capture)?);
        }

        // Listening is one output fed from every heard side, so those sides and
        // that output agree on one rate or the call is refused, exactly as a
        // room is. The sides nobody hears keep their own.
        let listen = match listen {
            Some(device) if parties.iter().any(|party| party.heard) => {
                let out_default = device
                    .default_output_config()
                    .map_err(|e| Error::Device(e.to_string()))?;
                let out_ranges: Vec<(u32, u32)> = device
                    .supported_output_configs()
                    .map_err(|e| Error::Device(e.to_string()))?
                    .map(|range| (range.min_sample_rate(), range.max_sample_rate()))
                    .collect();
                let mut heard = Vec::new();
                for (party, config) in parties.iter().zip(&configs) {
                    if party.heard {
                        heard.push((config.sample_rate(), ranges(party.device, party.capture)?));
                    }
                }
                let Some(rate) =
                    crate::live::rate_they_agree_on(out_default.sample_rate(), &out_ranges, &heard)
                else {
                    return Err(Error::Device(format!(
                        "the side being listened to and the device it would be heard on \
                         have no sample rate they will both take, and VeilVoice does not \
                         resample. Set them to the same rate in your system's sound \
                         settings, or record without listening ({} Hz to listen on).",
                        out_default.sample_rate()
                    )));
                };
                for (party, config) in parties.iter().zip(configs.iter_mut()) {
                    if party.heard && config.sample_rate() != rate {
                        *config =
                            config_at(party.device, party.capture, rate)?.ok_or_else(|| {
                                Error::Device(format!(
                                    "a side would not open at {rate} Hz after all"
                                ))
                            })?;
                    }
                }
                let out = if out_default.sample_rate() == rate {
                    out_default
                } else {
                    crate::live::at_rate(
                        device
                            .supported_output_configs()
                            .map_err(|e| Error::Device(e.to_string()))?,
                        rate,
                    )
                    .ok_or_else(|| {
                        Error::Device(format!("the listening device would not open at {rate} Hz"))
                    })?
                };
                Some((device, out))
            }
            _ => None,
        };

        let shared = Arc::new(Shared::default());
        if let Ok(mut stats) = shared.stats.lock() {
            // Sized once, here. The callbacks index into it and never grow it.
            stats.guests = vec![GuestStats::default(); parties.len()];
        }

        let mut kept = KeptCall::default();
        let mut streams = Vec::with_capacity(parties.len());
        let mut to_ears = Vec::new();

        for (index, (party, config)) in parties.iter().zip(&configs).enumerate() {
            let rate = config.sample_rate();
            let in_channels = (config.channels() as usize).max(1);
            // The largest block handled in one piece. A callback handing over
            // more is processed in pieces of this size rather than grown into.
            let block = ((rate as f32 * crate::live::RING_MILLIS / 1000.0) as usize).max(2048);

            let mut engine = if party.keeping.veiled {
                let mut settings = party.config;
                settings.sample_rate = rate as f32;
                Some(Deidentifier::new(settings).map_err(Error::Engine)?)
            } else {
                None
            };

            // The recorders, at the rate this side's device runs at. F-166:
            // the caller has no rate to get wrong.
            let (veiled_recorder, mut veiled_sink) = if party.keeping.veiled {
                let (recorder, sink) = crate::record::start(rate);
                (Some(recorder), Some(sink))
            } else {
                (None, None)
            };
            let (plain_recorder, mut plain_sink) = if party.keeping.plain {
                let (recorder, sink) = crate::record::start(rate);
                (Some(recorder), Some(sink))
            } else {
                (None, None)
            };
            kept.parties.push(Kept {
                veiled: veiled_recorder,
                plain: plain_recorder,
            });

            // What the listener hears of this side, when anybody does.
            let mut to_ear = if party.heard && listen.is_some() {
                let (producer, consumer) = HeapRb::<f32>::new(block).split();
                to_ears.push(consumer);
                Some(producer)
            } else {
                None
            };

            // Sized once, so the input callback never allocates.
            let mut mono = vec![0.0f32; block];
            let mut veiled = vec![0.0f32; block];
            let capture_shared = Arc::clone(&shared);

            let stream = party
                .device
                .build_input_stream(
                    config.config(),
                    move |data: &[f32], _| {
                        let mut peak = 0.0f32;
                        let mut out_peak = 0.0f32;
                        let mut dropped = 0u64;
                        for piece in data.chunks(block * in_channels) {
                            let mut frames = 0usize;
                            for frame in piece.chunks(in_channels) {
                                let sample = frame.iter().sum::<f32>() / in_channels as f32;
                                peak = peak.max(sample.abs());
                                mono[frames] = sample;
                                frames += 1;
                            }
                            // The real voice, at the one moment it exists in
                            // this process, and only where it was asked for.
                            if let Some(sink) = plain_sink.as_mut() {
                                sink.write(&mono[..frames]);
                            }
                            if let Some(engine) = engine.as_mut() {
                                engine.process(&mono[..frames], &mut veiled[..frames]);
                                for &sample in &veiled[..frames] {
                                    out_peak = out_peak.max(sample.abs());
                                }
                                // The veiled voice, at the one moment it exists,
                                // copied into this side's recorder and nowhere
                                // else.
                                if let Some(sink) = veiled_sink.as_mut() {
                                    sink.write(&veiled[..frames]);
                                }
                            }
                            if let Some(ear) = to_ear.as_mut() {
                                let taken = ear.push_slice(&mono[..frames]);
                                dropped += (frames - taken) as u64;
                            }
                        }
                        // Never block the callback for a statistics update.
                        if let Ok(mut stats) = capture_shared.stats.try_lock() {
                            if let Some(side) = stats.guests.get_mut(index) {
                                side.input_peak = side.input_peak.max(peak);
                                side.output_peak = side.output_peak.max(out_peak);
                                side.dropped += dropped;
                            }
                        }
                    },
                    {
                        let shared = Arc::clone(&shared);
                        move |e| shared.troubles.report(crate::live::Side::Input, &e)
                    },
                    None,
                )
                .map_err(|e| match party.capture {
                    Capture::Playback => Error::Device(no_playback_capture(&e.to_string())),
                    Capture::Microphone => Error::Stream(e.to_string()),
                })?;
            streams.push(stream);
        }

        let listen_stream = match listen {
            Some((device, out_cfg)) => {
                let out_channels = (out_cfg.channels() as usize).max(1);
                let rate = out_cfg.sample_rate();
                let max_frames =
                    ((rate as f32 * crate::live::RING_MILLIS / 1000.0) as usize).max(2048);
                // Sized once, here, for the same reason every other buffer is.
                let mut mix = vec![0.0f32; max_frames];
                let mut arriving = vec![0.0f32; max_frames];
                let play_shared = Arc::clone(&shared);
                let stream = device
                    .build_output_stream(
                        out_cfg.config(),
                        move |data: &mut [f32], _| {
                            let frames = (data.len() / out_channels).min(max_frames);
                            mix[..frames].fill(0.0);
                            let mut starved = 0u64;
                            for ear in to_ears.iter_mut() {
                                let got = ear.pop_slice(&mut arriving[..frames]);
                                if got < frames {
                                    // Silence rather than a repeat of old audio.
                                    arriving[got..frames].fill(0.0);
                                    starved += 1;
                                }
                                for (into, &sample) in
                                    mix[..frames].iter_mut().zip(&arriving[..frames])
                                {
                                    *into += sample;
                                }
                            }
                            let mut peak = 0.0f32;
                            for &sample in &mix[..frames] {
                                peak = peak.max(sample.abs());
                            }
                            // Unity gain. What is heard is what arrived, and the
                            // only thing between the two is the person's own
                            // volume control.
                            for (frame, &sample) in
                                data.chunks_mut(out_channels).zip(&mix[..frames])
                            {
                                let value = sample.clamp(-1.0, 1.0);
                                for slot in frame.iter_mut() {
                                    *slot = value;
                                }
                            }
                            for slot in data.iter_mut().skip(frames * out_channels) {
                                *slot = 0.0;
                            }
                            if starved > 0 {
                                play_shared.starved.fetch_add(starved, Ordering::Relaxed);
                            }
                            if let Ok(mut stats) = play_shared.stats.try_lock() {
                                stats.mix_peak = stats.mix_peak.max(peak);
                                if peak > 1.0 {
                                    stats.clipped += 1;
                                }
                                stats.starved = play_shared.starved.load(Ordering::Relaxed);
                            }
                        },
                        {
                            let shared = Arc::clone(&shared);
                            move |e| shared.troubles.report(crate::live::Side::Output, &e)
                        },
                        None,
                    )
                    .map_err(|e| Error::Stream(e.to_string()))?;
                Some(stream)
            }
            None => None,
        };

        for stream in &streams {
            stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        }
        if let Some(stream) = &listen_stream {
            stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        }

        Ok((
            Self {
                _inputs: streams,
                _listen: listen_stream,
                shared,
            },
            kept,
        ))
    }

    /// Read the counters, resetting the peak meters.
    ///
    /// The same shape a room reports: one entry per side, and the mix being
    /// what the listener hears. `load` is not measured for a call, because
    /// each side runs on its own deadline rather than sharing one.
    pub fn stats(&self) -> RoomStats {
        let Ok(mut stats) = self.shared.stats.lock() else {
            return RoomStats::default();
        };
        stats.interfered = self.shared.troubles.count();
        let snapshot = stats.clone();
        for side in stats.guests.iter_mut() {
            side.input_peak = 0.0;
            side.output_peak = 0.0;
        }
        stats.mix_peak = 0.0;
        snapshot
    }

    /// What the platform last reported about any of the streams.
    pub fn interference(&self) -> Option<crate::live::Interference> {
        self.shared.troubles.latest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The code above the tests, so a guard does not read its own needles.
    fn code() -> String {
        include_str!("call.rs")
            .replace("\r\n", "\n")
            .split("\n#[cfg(test)]")
            .next()
            .expect("the code above the tests")
            .to_string()
    }

    #[test]
    fn a_call_of_you_and_the_computer_is_allowed() {
        assert_eq!(
            refusal(
                &[(Capture::Microphone, None), (Capture::Playback, None)],
                None
            ),
            None,
            "the ordinary call, you on the default microphone and them on the default \
             output, was refused"
        );
        // The splitter cable: both sides are inputs, on different devices.
        assert_eq!(
            refusal(
                &[
                    (Capture::Microphone, None),
                    (Capture::Microphone, Some("USB Audio Line In"))
                ],
                Some(None)
            ),
            None
        );
    }

    #[test]
    fn one_device_cannot_be_two_sides() {
        let said = refusal(
            &[
                (Capture::Microphone, Some("Headset")),
                (Capture::Microphone, Some("Headset")),
            ],
            None,
        )
        .expect("two sides on one microphone were allowed");
        assert!(said.contains("\"Headset\""), "{said}");

        // A microphone and the output of the same name are two signals, and
        // a headset is exactly that: allowed.
        assert_eq!(
            refusal(
                &[
                    (Capture::Microphone, Some("Headset")),
                    (Capture::Playback, Some("Headset")),
                ],
                None
            ),
            None
        );
    }

    /// Listening on the output a side is recorded from is an echo built on
    /// purpose, and the default output is a device like any other.
    #[test]
    fn listening_where_a_side_is_recorded_from_is_refused() {
        let said = refusal(
            &[(Capture::Microphone, None), (Capture::Playback, None)],
            Some(None),
        )
        .expect("listening on the default output while recording it was allowed");
        assert!(said.contains("default output"), "{said}");

        assert_eq!(
            refusal(
                &[(Capture::Microphone, None), (Capture::Playback, None)],
                Some(Some("Headphones"))
            ),
            None,
            "listening on headphones while recording the speakers is the right way to \
             do it, and was refused"
        );
    }

    #[test]
    fn an_empty_call_and_an_oversized_one_are_refused() {
        assert!(refusal(&[], None).is_some());
        let many = vec![(Capture::Microphone, None); MAX_GUESTS + 1];
        assert!(refusal(&many, None).is_some());
    }

    #[test]
    fn the_linux_route_is_named_when_playback_cannot_be_recorded() {
        let said = no_playback_capture("not supported");
        assert!(said.contains("Monitor of"), "{said}");
        assert!(
            said.contains("not supported"),
            "the platform's own words were lost"
        );
    }

    /// Nothing in the callbacks allocates: every buffer is sized before the
    /// streams start, and a long block is cut up rather than grown into.
    #[test]
    fn every_buffer_is_sized_before_the_streams_start() {
        let code = code();
        for needle in [
            "let mut mono = vec![0.0f32; block];",
            "let mut veiled = vec![0.0f32; block];",
            "let mut mix = vec![0.0f32; max_frames];",
            "let mut arriving = vec![0.0f32; max_frames];",
            "for piece in data.chunks(block * in_channels) {",
        ] {
            assert!(code.contains(needle), "{needle} is gone");
        }
    }

    /// The recorders are fed from the two moments a voice exists, and from
    /// nowhere else: the downmix for the real voice, the engine's own output
    /// for the veiled one. A third write is a third copy of somebody's voice.
    #[test]
    fn each_recorder_is_fed_from_one_place() {
        let code = code();
        assert!(code.contains("sink.write(&mono[..frames]);"));
        assert!(code.contains("engine.process(&mono[..frames], &mut veiled[..frames]);"));
        assert!(code.contains("sink.write(&veiled[..frames]);"));
        assert_eq!(code.matches("sink.write(").count(), 2);
    }

    /// What is heard is what arrived. Nothing scales it on the way to the
    /// listener, so the level is the one the call played at.
    #[test]
    fn listening_is_at_unity_gain() {
        let code = code();
        let written: String = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(written.contains("let value = sample.clamp(-1.0, 1.0);"));
        for scaled in ["gain *", "* gain", "volume"] {
            assert!(
                !written.contains(scaled),
                "something scales what the listener hears ({scaled})"
            );
        }
    }
}
