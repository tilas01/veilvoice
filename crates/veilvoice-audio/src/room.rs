// SPDX-License-Identifier: GPL-3.0-or-later
//! **Marker 147.** Several microphones at once: a guest each, veiled each,
//! mixed once.
//!
//! # What this is for
//!
//! An interview with everybody in the room on their own microphone. Each guest
//! gets their own engine with their own seed and their own destination voice,
//! exactly as a group *render* gives them, and the veiled results are mixed
//! into the one output the call or the recorder is listening to.
//!
//! [`live`](crate::live) opens one input. That is the right shape for one
//! person on a call and the wrong shape for a room, because one microphone
//! carrying four people is one signal: whatever it is turned into, everybody in
//! it is turned into the same thing, and a listener can no longer follow who is
//! speaking.
//!
//! # One engine per guest is one FFT chain per guest, on one deadline
//!
//! The output callback runs every guest's engine before it returns. They share
//! a deadline of a few milliseconds, so the cost is the sum, and this is the
//! honest account the marker asked for rather than a paragraph promising it is
//! fine: [`RoomStats::load`] is that sum measured against that deadline, drawn
//! where somebody can see it. At 1.0 the engines have used the whole of the
//! time the block had, and what follows is dropouts.
//!
//! It is a measurement rather than a limit, because the number of guests a
//! machine can carry is a fact about the machine. [`MAX_GUESTS`] is a bound on
//! the arithmetic, not a claim about performance.
//!
//! # The mix is summed and clipped, and never limited
//!
//! Two people talking at once is two signals added together, which can go past
//! full scale. A render fixes that afterwards by scaling the whole file by one
//! factor, which a live path cannot do because it cannot see the rest of the
//! conversation.
//!
//! So the sum is clipped, the peak *before* clipping is reported, and the
//! number of blocks that clipped is counted. There is deliberately **no
//! limiter**: a limiter is a dynamics processor, it changes the voice, and this
//! program's entire claim is about what changes a voice and what does not.
//! Adding one to avoid a warning would mean a second thing altering the sound
//! that nobody asked for. The warning is the honest end of that.
//!
//! # Every microphone has its own clock
//!
//! Two USB microphones are two oscillators, and neither is the output's. Each
//! guest gets their own ring, of the same length [`live`](crate::live) uses,
//! which absorbs the jitter and reports what it could not: a guest whose device
//! runs slightly fast fills their ring and drops samples, counted against that
//! guest, and one running slow starves and is padded with silence.
//!
//! What is **not** absorbed is a rate mismatch, which is F-168 and is refused
//! before anything starts: see `live::agree_on_a_rate_for`.
//!
//! # In plain words
//!
//! Everybody in the room speaks into their own microphone. Each voice is
//! disguised separately, so they still sound like different people, and the
//! result is mixed together into one signal for the call or the recording.
//!
//! Disguising four voices at once is four times the work, on the same short
//! deadline, so the screen shows how much of that deadline is being used. If it
//! reaches the top, the computer cannot keep up and the sound will break.

use crate::live::{Keeping, Kept};
use crate::Error;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use veilvoice_core::{DeidConfig, Deidentifier};

/// The most microphones one room will open at once.
///
/// A bound on the arithmetic rather than a claim about any machine: what a
/// machine can actually carry is [`RoomStats::load`], measured while it runs.
/// Eight is past the number of people who can hold one conversation, and every
/// guest costs a device, a ring, an engine and a place in the mix.
pub const MAX_GUESTS: usize = 8;

/// One guest: the microphone they speak into and the voice they become.
pub struct Guest<'a> {
    /// The device their voice arrives on. One each: two guests sharing a
    /// microphone is one signal, and this crate cannot separate it.
    pub device: &'a cpal::Device,
    /// Their engine's settings, carrying the destination voice their slot has.
    ///
    /// **This crate does not choose voices.** Two guests given the same
    /// configuration get the same voice, which defeats the point of separating
    /// them, and the crate that knows about voice tables is the one that must
    /// refuse that. `sample_rate` is overwritten with the rate the devices
    /// agreed on.
    pub config: DeidConfig,
    /// What to keep of this guest, if anything. Marker 131's warning about the
    /// unveiled side applies once per guest.
    pub keeping: Keeping,
}

/// What one guest's half of a running room is doing.
#[derive(Clone, Copy, Debug, Default)]
pub struct GuestStats {
    /// Peak of what arrived from their microphone, since the last read.
    pub input_peak: f32,
    /// Peak of what their engine produced, since the last read.
    pub output_peak: f32,
    /// Samples dropped because their ring overflowed. Their device is running
    /// faster than the output, or the output callback is late.
    pub dropped: u64,
}

/// What a running room is doing, safe to read from the interface.
///
/// Not `Copy`: it holds one entry per guest. Read once a frame, as
/// [`crate::LiveStats`] is.
#[derive(Clone, Debug, Default)]
pub struct RoomStats {
    /// One per guest, in the order they were given.
    pub guests: Vec<GuestStats>,
    /// Peak of the mix **before** it was clipped, since the last read.
    ///
    /// Above 1.0 means the guests together went past full scale and the excess
    /// was cut off. It is reported unclipped on purpose: a meter that showed
    /// the clipped value would sit at exactly 1.0 and look correct.
    pub mix_peak: f32,
    /// Output blocks in which the mix went past full scale.
    pub clipped: u64,
    /// Times the output callback found a guest's ring empty and padded with
    /// silence.
    pub starved: u64,
    /// How many times the platform has reported trouble on any of the streams.
    /// **Marker 132**, the same counter the single-microphone path carries.
    pub interfered: u64,
    /// What every engine together costs against the deadline they share.
    ///
    /// The sum of each guest's realtime factor. Below 1.0 the machine keeps up;
    /// at 1.0 the engines have used the whole block, and past it the audio
    /// breaks. This is the honest account marker 147 asked for.
    pub load: f32,
}

/// The recorders a room was asked for.
#[derive(Default)]
pub struct KeptRoom {
    /// One per guest, in the order they were given.
    pub guests: Vec<Kept>,
    /// The mix everybody hears, when it was asked for.
    pub mixed: Option<crate::record::Recorder>,
}

/// A running room. Dropping it stops every stream.
pub struct RoomSession {
    // The streams must outlive the session; dropping them stops the callbacks.
    _inputs: Vec<cpal::Stream>,
    _output: cpal::Stream,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    stats: Mutex<RoomStats>,
    starved: AtomicU64,
    troubles: AtomicU64,
    trouble: Mutex<Option<crate::live::Interference>>,
}

impl Shared {
    /// Record what the platform said about one of the streams.
    ///
    /// The same shape as the single-microphone path's, and not a realtime
    /// callback: cpal calls this when something has gone wrong rather than
    /// every block.
    fn report(&self, side: crate::live::Side, error: &cpal::StreamError) {
        let count = self.troubles.fetch_add(1, Ordering::Relaxed) + 1;
        eprintln!("veilvoice: {} stream error: {error}", side.word());
        if let Ok(mut held) = self.trouble.lock() {
            *held = Some(crate::live::Interference {
                side,
                device_gone: matches!(error, cpal::StreamError::DeviceNotAvailable),
                said: error.to_string(),
                count,
            });
        }
    }
}

/// Everything one guest's engine needs, owned by the output callback.
///
/// Built before the stream starts. The callback only indexes into it: nothing
/// here is allocated, resized or locked once the audio is running.
struct Voice {
    from: ringbuf::HeapCons<f32>,
    engine: Deidentifier,
    arriving: Vec<f32>,
    veiled: Vec<f32>,
    sink: Option<crate::record::Sink>,
}

impl RoomSession {
    /// Open every guest's microphone, veil each, and mix into `output`.
    ///
    /// `mixed` asks for a recorder fed with what the output actually receives,
    /// which is the one recording that has everybody in it.
    ///
    /// Refuses an empty room, more than [`MAX_GUESTS`], and any set of devices
    /// with no sample rate they all accept.
    pub fn start(
        guests: &[Guest<'_>],
        output: &cpal::Device,
        mixed: bool,
    ) -> Result<(Self, KeptRoom), Error> {
        if guests.is_empty() {
            return Err(Error::Device(
                "a room with nobody in it has nothing to veil".into(),
            ));
        }
        if guests.len() > MAX_GUESTS {
            return Err(Error::Device(format!(
                "{} microphones were given and this opens at most {MAX_GUESTS}. Every \
                 guest costs a device, a ring and an engine on the same deadline.",
                guests.len()
            )));
        }

        // **F-168, once per guest.** One rate for the whole room, or a refusal
        // that names what disagreed.
        let devices: Vec<&cpal::Device> = guests.iter().map(|guest| guest.device).collect();
        let (in_cfgs, out_cfg) = crate::live::agree_on_a_rate_for(&devices, output)?;

        let sample_rate = out_cfg.sample_rate().0;
        let out_channels = out_cfg.channels() as usize;
        let capacity =
            ((sample_rate as f32 * crate::live::RING_MILLIS / 1000.0) as usize).max(2048);

        let shared = Arc::new(Shared::default());
        if let Ok(mut stats) = shared.stats.lock() {
            // Sized once, here. The callbacks index into it and never grow it.
            stats.guests = vec![GuestStats::default(); guests.len()];
        }

        let mut voices = Vec::with_capacity(guests.len());
        let mut kept = KeptRoom::default();
        let mut input_streams = Vec::with_capacity(guests.len());

        for (index, (guest, in_cfg)) in guests.iter().zip(&in_cfgs).enumerate() {
            let mut config = guest.config;
            config.sample_rate = sample_rate as f32;
            let engine = Deidentifier::new(config).map_err(Error::Engine)?;

            // The recorders, at the rate the devices agreed on. F-166: the
            // caller has no rate to get wrong here either.
            let (veiled_recorder, veiled_sink) = if guest.keeping.veiled {
                let (recorder, sink) = crate::record::start(sample_rate);
                (Some(recorder), Some(sink))
            } else {
                (None, None)
            };
            let (plain_recorder, mut plain_sink) = if guest.keeping.plain {
                let (recorder, sink) = crate::record::start(sample_rate);
                (Some(recorder), Some(sink))
            } else {
                (None, None)
            };
            kept.guests.push(Kept {
                veiled: veiled_recorder,
                plain: plain_recorder,
            });

            let (mut producer, consumer) = HeapRb::<f32>::new(capacity).split();
            let in_channels = in_cfg.channels() as usize;
            // Sized once, so the input callback never allocates. Only used when
            // this guest's microphone is being kept.
            let mut mono_scratch = vec![0.0f32; capacity];
            let capture_shared = Arc::clone(&shared);

            let stream = guest
                .device
                .build_input_stream(
                    &in_cfg.config(),
                    move |data: &[f32], _| {
                        let mut peak = 0.0f32;
                        let mut dropped = 0u64;
                        let mut kept_here = 0usize;
                        for frame in data.chunks(in_channels) {
                            let mono = frame.iter().sum::<f32>() / in_channels as f32;
                            peak = peak.max(mono.abs());
                            // The real voice, at the one moment it exists in
                            // this process, and only where it was asked for.
                            if plain_sink.is_some() && kept_here < mono_scratch.len() {
                                mono_scratch[kept_here] = mono;
                                kept_here += 1;
                            }
                            if producer.try_push(mono).is_err() {
                                dropped += 1;
                            }
                        }
                        if let Some(sink) = plain_sink.as_mut() {
                            sink.write(&mono_scratch[..kept_here]);
                        }
                        // Never block the callback for a statistics update.
                        if let Ok(mut stats) = capture_shared.stats.try_lock() {
                            if let Some(guest) = stats.guests.get_mut(index) {
                                guest.input_peak = guest.input_peak.max(peak);
                                guest.dropped += dropped;
                            }
                        }
                    },
                    {
                        let shared = Arc::clone(&shared);
                        move |e| shared.report(crate::live::Side::Input, &e)
                    },
                    None,
                )
                .map_err(|e| Error::Stream(e.to_string()))?;
            input_streams.push(stream);

            voices.push(Voice {
                from: consumer,
                engine,
                arriving: vec![0.0f32; capacity],
                veiled: vec![0.0f32; capacity],
                sink: veiled_sink,
            });
        }

        let (mixed_recorder, mut mixed_sink) = if mixed {
            let (recorder, sink) = crate::record::start(sample_rate);
            (Some(recorder), Some(sink))
        } else {
            (None, None)
        };
        kept.mixed = mixed_recorder;

        // Sized once, here, for the same reason every other buffer is.
        let max_frames = capacity;
        let mut mix = vec![0.0f32; max_frames];
        let play_shared = Arc::clone(&shared);

        let output_stream = output
            .build_output_stream(
                &out_cfg.config(),
                move |data: &mut [f32], _| {
                    let frames = (data.len() / out_channels.max(1)).min(max_frames);
                    mix[..frames].fill(0.0);

                    let mut starved = 0u64;
                    let mut load = 0.0f32;
                    for voice in voices.iter_mut() {
                        let got = voice.from.pop_slice(&mut voice.arriving[..frames]);
                        if got < frames {
                            // Underrun: silence rather than a repeat of old
                            // audio, which would be an audible stutter.
                            voice.arriving[got..frames].fill(0.0);
                            starved += 1;
                        }
                        voice
                            .engine
                            .process(&voice.arriving[..frames], &mut voice.veiled[..frames]);
                        // The veiled voice, at the one moment it exists, copied
                        // into this guest's recorder and nowhere else.
                        if let Some(sink) = voice.sink.as_mut() {
                            sink.write(&voice.veiled[..frames]);
                        }
                        load += voice.engine.stats().last_realtime_factor() as f32;
                        for (into, &sample) in mix[..frames].iter_mut().zip(&voice.veiled[..frames])
                        {
                            *into += sample;
                        }
                    }

                    // The peak before clipping, which is the one worth showing:
                    // a meter of the clipped signal sits at 1.0 and looks fine.
                    let mut peak = 0.0f32;
                    let mut clipped = false;
                    for &sample in &mix[..frames] {
                        peak = peak.max(sample.abs());
                    }
                    if peak > 1.0 {
                        clipped = true;
                    }

                    for (frame, &sample) in data.chunks_mut(out_channels).zip(&mix[..frames]) {
                        let value = sample.clamp(-1.0, 1.0);
                        // The same mono mix to every output channel.
                        for slot in frame.iter_mut() {
                            *slot = value;
                        }
                    }
                    // Any tail beyond `frames` stays silent.
                    for slot in data.iter_mut().skip(frames * out_channels) {
                        *slot = 0.0;
                    }
                    if let Some(sink) = mixed_sink.as_mut() {
                        // What the output actually received, clipping included,
                        // so the recording is what the room heard.
                        for sample in mix[..frames].iter_mut() {
                            *sample = sample.clamp(-1.0, 1.0);
                        }
                        sink.write(&mix[..frames]);
                    }

                    if starved > 0 {
                        play_shared.starved.fetch_add(starved, Ordering::Relaxed);
                    }
                    if let Ok(mut stats) = play_shared.stats.try_lock() {
                        for (guest, voice) in stats.guests.iter_mut().zip(voices.iter()) {
                            let mut theirs = 0.0f32;
                            for &sample in &voice.veiled[..frames] {
                                theirs = theirs.max(sample.abs());
                            }
                            guest.output_peak = guest.output_peak.max(theirs);
                        }
                        stats.mix_peak = stats.mix_peak.max(peak);
                        if clipped {
                            stats.clipped += 1;
                        }
                        stats.load = load;
                        stats.starved = play_shared.starved.load(Ordering::Relaxed);
                    }
                },
                {
                    let shared = Arc::clone(&shared);
                    move |e| shared.report(crate::live::Side::Output, &e)
                },
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        for stream in &input_streams {
            stream.play().map_err(|e| Error::Stream(e.to_string()))?;
        }
        output_stream
            .play()
            .map_err(|e| Error::Stream(e.to_string()))?;

        Ok((
            Self {
                _inputs: input_streams,
                _output: output_stream,
                shared,
            },
            kept,
        ))
    }

    /// How many guests this room opened.
    pub fn guests(&self) -> usize {
        self.shared
            .stats
            .lock()
            .map(|stats| stats.guests.len())
            .unwrap_or(0)
    }

    /// Read the counters, resetting the peak meters.
    ///
    /// Peaks reset on read so a meter shows the level since the last frame
    /// rather than the loudest moment since the room opened.
    pub fn stats(&self) -> RoomStats {
        let Ok(mut stats) = self.shared.stats.lock() else {
            return RoomStats::default();
        };
        // Read here rather than written by a callback, for the reason the
        // single-microphone path reads it here: a device that has gone stops
        // calling its callback, and the number that has to survive a stream
        // which is no longer running is the one saying it stopped.
        stats.interfered = self.shared.troubles.load(Ordering::Relaxed);
        let snapshot = stats.clone();
        for guest in stats.guests.iter_mut() {
            guest.input_peak = 0.0;
            guest.output_peak = 0.0;
        }
        stats.mix_peak = 0.0;
        snapshot
    }

    /// What the platform last reported about any of the streams.
    ///
    /// **Marker 132**, the same report the single-microphone path gives. It
    /// says which side, not which guest: cpal's error callback is per stream
    /// and this keeps the most recent one, which is the one worth showing.
    pub fn interference(&self) -> Option<crate::live::Interference> {
        self.shared.trouble.lock().ok()?.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_starts_empty_and_says_nothing() {
        let stats = RoomStats::default();
        assert!(stats.guests.is_empty());
        assert_eq!(stats.clipped, 0);
        assert_eq!(stats.starved, 0);
        assert_eq!(stats.load, 0.0);
        assert_eq!(stats.mix_peak, 0.0);
    }

    /// The bound is on the arithmetic, and the honest number is measured.
    ///
    /// Eight is past the number of people who can hold one conversation. What a
    /// machine can carry is [`RoomStats::load`], and no constant here is a
    /// claim about that.
    #[test]
    fn the_guest_limit_is_a_bound_rather_than_a_promise() {
        // A `const` block, because both sides are constants and clippy is
        // right that a runtime assertion over two of those is a test that never
        // runs: this now fails to compile rather than failing to pass.
        const _: () = assert!(MAX_GUESTS >= 2);
        const _: () = assert!(MAX_GUESTS <= veilvoice_core::voices::MAX_VOICES);

        // Said in words as well, because the compile error says only which line
        // it was. A room may not hold more guests than there are voices far
        // enough apart to tell apart: two guests given one voice is the thing
        // this module exists to avoid.
        assert_eq!(
            MAX_GUESTS, 8,
            "the bound moved, and the reason above should \
             move with it: eight is past the number of people who can hold one \
             conversation"
        );
    }

    /// **Marker 147.** The load is a sum, because the deadline is shared.
    ///
    /// One engine at 0.2 of realtime is comfortable. Four of them is 0.8, on
    /// the same block, and that is the number worth showing rather than the
    /// 0.2 each of them would report about itself.
    #[test]
    fn the_load_is_what_every_engine_costs_together() {
        let source = include_str!("room.rs").replace("\r\n", "\n");
        let code = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("the code above the tests");
        assert!(
            code.contains("load += voice.engine.stats().last_realtime_factor() as f32;"),
            "the load is no longer the sum of what each engine costs, and a \
             per-guest figure would say a machine is comfortable while the room \
             breaks up"
        );
    }

    /// No limiter, ever.
    ///
    /// A limiter is a dynamics processor: it changes the voice. This program's
    /// whole claim is about what changes a voice, so a limiter added to avoid a
    /// clipping warning would be a second thing altering the sound that nobody
    /// asked for. The mix clips, says so, and counts it.
    #[test]
    fn the_mix_clips_and_says_so_rather_than_limiting() {
        let source = include_str!("room.rs").replace("\r\n", "\n");
        let code = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("the code above the tests");
        assert!(
            code.contains("stats.clipped += 1;"),
            "the mix no longer counts the blocks it clipped"
        );
        assert!(
            code.contains("stats.mix_peak = stats.mix_peak.max(peak);"),
            "the reported peak is no longer the one taken before clipping, so a \
             meter of it would sit at 1.0 and look correct"
        );
        // The code, not the prose. This module's own documentation argues at
        // length that there is no limiter here, and a guard that searched the
        // whole file for the word would be reading that argument and failing on
        // it. `dialog.rs`'s guard hit the same thing and it is in the audit.
        let written: String = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for dynamics in ["compress", "limiter", "makeup_gain"] {
            assert!(
                !written.contains(dynamics),
                "something in here looks like a dynamics processor ({dynamics}), \
                 which changes the voice, which is the one thing this program \
                 may not do without saying so"
            );
        }
    }

    /// Nothing in the callbacks allocates. Checked here as well as by the
    /// workspace guard, because this file is where the temptation is: a room
    /// has a variable number of guests, and a `Vec` per block would be the
    /// obvious way to write it.
    #[test]
    fn every_buffer_is_sized_before_the_streams_start() {
        let source = include_str!("room.rs").replace("\r\n", "\n");
        let code = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("the code above the tests");
        let callbacks = code.match_indices("| {").count();
        assert!(
            callbacks >= 2,
            "the input and output callbacks have to be findable, and {callbacks} \
             were"
        );
        assert!(
            code.contains("let mut mix = vec![0.0f32; max_frames];"),
            "the mix buffer is no longer sized once before the streams start"
        );
        assert!(
            code.contains("arriving: vec![0.0f32; capacity],"),
            "a guest's arriving buffer is no longer sized once"
        );
    }
}
