// SPDX-License-Identifier: GPL-3.0-or-later
//! Live microphone scrambling.
//!
//! # Structure
//!
//! Capture and playback run as two independent callbacks driven by the audio
//! hardware, joined by a lock-free SPSC ring buffer. The de-identification runs
//! inside the *output* callback, which is the shortest path: adding a worker
//! thread would mean a second buffer and a second scheduling delay for no
//! benefit, and [`veilvoice_core::Deidentifier::process`] is explicitly
//! allocation-free and safe to call from an audio callback.
//!
//! # Rules the callbacks follow
//!
//! An audio callback that blocks produces a dropout, so neither callback ever
//! allocates, locks, or waits. Statistics are published through a mutex the
//! callback only ever *tries* to take: if the UI thread happens to hold it, the
//! update is skipped rather than the audio stalling.
//!
//! # Latency
//!
//! Total latency is the input buffer, plus the ring backlog, plus the engine's
//! one-frame group delay (~21 ms at the defaults), plus the output buffer. The
//! ring is intentionally short, enough to absorb jitter between two clocks
//! that are not synchronised, not enough to accumulate a delay the user would
//! notice while speaking.
//!
//! # In plain words
//!
//! This is live mode: your microphone in one end, a voice that is not yours out
//! the other, fast enough to hold a conversation.
//!
//! Sound arrives from the microphone in small pieces, and each one has to be dealt
//! with before the next arrives. There is no room to be late. So the veiling
//! happens on the same short path the sound is already travelling, with nothing
//! queued up behind it and nothing that could pause to allocate memory or wait for
//! another part of the program.
//!
//! If the computer ever cannot keep up, that is counted and shown rather than
//! hidden. A gap in the sound you can see explained is far better than one you
//! cannot.

use crate::Error;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use veilvoice_core::{DeidConfig, Deidentifier, ProcessStats};

/// How much jitter the ring absorbs before it starts dropping samples.
const RING_MILLIS: f32 = 120.0;

/// Which side of the engine something happened to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The microphone.
    Input,
    /// Where the veiled voice goes.
    Output,
}

impl Side {
    /// The word for this side, as a person reading a warning would meet it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// Something that happened to the audio path while it was running.
///
/// **Marker 132.** The platform reports these on a callback of its own, and
/// until now the only thing done with one was `eprintln!`: on Windows the
/// desktop application is built with no console at all, so a microphone
/// unplugged in the middle of a call was silent, and a recording carried on
/// being made of nothing.
///
/// Not `Copy`, and deliberately kept out of [`LiveStats`]: the meters are read
/// sixty times a second and this holds a `String`. The count is in the stats,
/// so a caller learns that something happened at meter speed and asks what it
/// was only when the answer changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interference {
    /// Which stream it happened on.
    pub side: Side,
    /// Whether the device that side was using stopped existing.
    ///
    /// This is the swapped-device case and the unplugged-device case, and it
    /// is the platform saying so rather than this crate polling for it: a
    /// device list read once a second is a guess between reads, and on Windows
    /// enumerating devices from another thread is what F-163 and F-165 were.
    pub device_gone: bool,
    /// What the platform said, verbatim.
    pub said: String,
    /// How many have happened on either side, this one included.
    pub count: u64,
}

/// A snapshot of what the live path is doing, safe to read from the UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiveStats {
    /// Engine performance counters.
    pub process: ProcessStats,
    /// Peak input level since the last read, in `[0, 1]`.
    pub input_peak: f32,
    /// Peak output level since the last read, in `[0, 1]`.
    pub output_peak: f32,
    /// Samples dropped because the ring overflowed (capture outrunning
    /// playback). A non-zero value means audible glitching.
    pub dropped: u64,
    /// Times the output callback found the ring empty and emitted silence.
    pub starved: u64,
    /// How many times the platform has reported trouble on either stream.
    ///
    /// **Marker 132.** A count rather than the reports themselves, so this
    /// stays `Copy` and cheap to read every frame. A caller that sees it move
    /// asks [`LiveSession::interference`] what happened.
    pub interfered: u64,
}

/// Which sides of the engine a session keeps.
///
/// Both are named at construction rather than passed as a pair of positional
/// flags, which is the property **marker 131** asked for and which two separate
/// arguments used to give: no caller reaches a recording of somebody's real
/// voice without writing the word `plain` next to it.
///
/// The default keeps neither, which is what [`LiveSession::start`] is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keeping {
    /// Keep the veiled voice, taken from inside the output callback.
    pub veiled: bool,
    /// Keep the microphone, taken from inside the input callback after the
    /// downmix to mono. This is the real voice, and it is the one thing in
    /// this crate that records it.
    pub plain: bool,
}

impl Keeping {
    /// Whether anything at all is being kept.
    pub fn is_anything(self) -> bool {
        self.veiled || self.plain
    }
}

/// The recorders a session was asked for, one per side of [`Keeping`].
///
/// Built by the session rather than by the caller, because the rate they are
/// built at has to be the rate the device agreed to and only the session knows
/// that. See [`LiveSession::start_recording`] for what went wrong when callers
/// built their own.
#[derive(Default)]
pub struct Kept {
    /// The veiled voice, present exactly when [`Keeping::veiled`] was set.
    pub veiled: Option<crate::record::Recorder>,
    /// The microphone, present exactly when [`Keeping::plain`] was set.
    pub plain: Option<crate::record::Recorder>,
}

/// A running live-scramble session. Dropping it stops the audio.
pub struct LiveSession {
    // Streams must outlive the session; dropping them stops the callbacks.
    _input: cpal::Stream,
    _output: cpal::Stream,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    stats: Mutex<LiveStats>,
    dropped: AtomicU64,
    starved: AtomicU64,
    /// How many times either stream has reported trouble.
    troubles: AtomicU64,
    /// The most recent one, for a caller that asks.
    trouble: Mutex<Option<Interference>>,
}

impl Shared {
    /// Record what the platform said about a stream.
    ///
    /// Called from cpal's error callback, which is **not** the realtime data
    /// callback: it runs when something has gone wrong rather than every
    /// block, so allocating a string and taking a lock here costs nothing that
    /// is being timed. The guard that forbids both reads the data callbacks
    /// and is right not to object to this one.
    fn report(&self, side: Side, error: &cpal::StreamError) {
        let count = self.troubles.fetch_add(1, Ordering::Relaxed) + 1;
        // Still printed. The command line has a console and somebody watching
        // it, and this is the only place `veilvoice live` can say anything
        // once it is running.
        eprintln!("veilvoice: {} stream error: {error}", side.word());
        if let Ok(mut held) = self.trouble.lock() {
            *held = Some(Interference {
                side,
                device_gone: matches!(error, cpal::StreamError::DeviceNotAvailable),
                said: error.to_string(),
                count,
            });
        }
    }
}

impl LiveSession {
    /// Start scrambling from `input` into `output`.
    ///
    /// `config.sample_rate` is overwritten with the rate the hardware actually
    /// agrees to, so the engine is never configured for a rate the device is
    /// not running at.
    pub fn start(
        input: &cpal::Device,
        output: &cpal::Device,
        config: DeidConfig,
    ) -> Result<Self, Error> {
        Self::start_recording(input, output, config, Keeping::default()).map(|(session, _)| session)
    }

    /// Start scrambling, keeping the sides of it [`Keeping`] asks for.
    ///
    /// Each side is taken from the callback where its samples exist and from
    /// nowhere else. The veiled voice comes from inside the output callback,
    /// which is the only place it exists before it reaches the device; the
    /// microphone comes from inside the input callback, after the downmix to
    /// mono and before anything else sees it. Taking either anywhere else would
    /// mean a second copy of the audio living somewhere unprotected, which is
    /// the thing [`record`](crate::record) is for avoiding.
    ///
    /// **Marker 131.** [`Keeping::plain`] is the one thing in this crate that
    /// records the real voice, and it is named at the call site for that
    /// reason: a caller cannot reach it without writing the word. It is false
    /// in every path that has not been asked for it, and the interface that
    /// offers it says what it is before it is started.
    ///
    /// # The recorders are built here, and F-166 is why
    ///
    /// This used to take a pair of already-built sinks, which meant the caller
    /// chose the rate the recording would be written at. Both callers passed
    /// the rate they had *asked* for, `config.sample_rate`, and this function
    /// then overwrites that with the rate the hardware agreed to. On any device
    /// not running at 48 kHz the two disagreed, and a WAV header that disagrees
    /// with its samples plays back at the wrong speed and the wrong pitch:
    /// on a de-identified recording, a second voice change nobody chose.
    ///
    /// So the caller no longer has a rate to get wrong. It says which sides to
    /// keep, and gets back the recorders for them, built from the rate this
    /// function is about to run the engine at.
    ///
    /// [`Sink::write`](crate::record::Sink::write) is realtime-safe, so each
    /// costs its callback a memcpy into an already-allocated ring and nothing
    /// else. A sink that cannot keep up drops samples and counts them rather
    /// than stalling the audio somebody is speaking into.
    pub fn start_recording(
        input: &cpal::Device,
        output: &cpal::Device,
        mut config: DeidConfig,
        keeping: Keeping,
    ) -> Result<(Self, Kept), Error> {
        let in_cfg = input
            .default_input_config()
            .map_err(|e| Error::Device(e.to_string()))?;
        let out_cfg = output
            .default_output_config()
            .map_err(|e| Error::Device(e.to_string()))?;

        let sample_rate = out_cfg.sample_rate().0;
        config.sample_rate = sample_rate as f32;
        let in_channels = in_cfg.channels() as usize;
        let out_channels = out_cfg.channels() as usize;

        // The recorders, at the rate the device agreed to rather than the rate
        // that was asked for. This line is the whole of F-166: it is the first
        // point at which the true rate exists, and building them anywhere
        // earlier is building them from a guess.
        let (veiled_recorder, mut veiled) = if keeping.veiled {
            let (recorder, sink) = crate::record::start(sample_rate);
            (Some(recorder), Some(sink))
        } else {
            (None, None)
        };
        let (plain_recorder, mut plain) = if keeping.plain {
            let (recorder, sink) = crate::record::start(sample_rate);
            (Some(recorder), Some(sink))
        } else {
            (None, None)
        };

        let mut deid = Deidentifier::new(config).map_err(Error::Engine)?;

        let capacity = ((sample_rate as f32 * RING_MILLIS / 1000.0) as usize).max(2048);
        let (mut producer, mut consumer) = HeapRb::<f32>::new(capacity).split();

        let shared = Arc::new(Shared::default());
        let cap_shared = Arc::clone(&shared);
        let play_shared = Arc::clone(&shared);

        // Sized once, here, so the input callback never allocates. Only used
        // when the microphone is being kept: the mono samples have to exist as
        // a slice before `Sink::write` can take them, and building that slice
        // per callback would be an allocation in a realtime path.
        let mut mono_scratch = vec![0.0f32; capacity];

        let input_stream = input
            .build_input_stream(
                &in_cfg.config(),
                move |data: &[f32], _| {
                    let mut peak = 0.0f32;
                    let mut dropped = 0u64;
                    let mut kept = 0usize;
                    // Downmix to mono: the engine is single channel, and a
                    // stereo image is itself a recording-setup fingerprint.
                    for frame in data.chunks(in_channels) {
                        let mono = frame.iter().sum::<f32>() / in_channels as f32;
                        peak = peak.max(mono.abs());
                        // The real voice, at the one moment it exists in this
                        // process, and only where it was asked for.
                        if plain.is_some() && kept < mono_scratch.len() {
                            mono_scratch[kept] = mono;
                            kept += 1;
                        }
                        if producer.try_push(mono).is_err() {
                            dropped += 1;
                        }
                    }
                    if let Some(sink) = plain.as_mut() {
                        sink.write(&mono_scratch[..kept]);
                    }
                    if dropped > 0 {
                        cap_shared.dropped.fetch_add(dropped, Ordering::Relaxed);
                    }
                    // Never block the callback for a statistics update.
                    if let Ok(mut s) = cap_shared.stats.try_lock() {
                        s.input_peak = s.input_peak.max(peak);
                    }
                },
                {
                    let shared = Arc::clone(&shared);
                    move |e| shared.report(Side::Input, &e)
                },
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        // Scratch buffers, sized once here so the callback never allocates.
        let max_frames = capacity;
        let mut scratch_in = vec![0.0f32; max_frames];
        let mut scratch_out = vec![0.0f32; max_frames];

        let output_stream = output
            .build_output_stream(
                &out_cfg.config(),
                move |data: &mut [f32], _| {
                    let frames = data.len() / out_channels.max(1);
                    let frames = frames.min(max_frames);

                    let got = consumer.pop_slice(&mut scratch_in[..frames]);
                    if got < frames {
                        // Underrun: pad with silence rather than repeating old
                        // audio, which would be an audible stutter.
                        scratch_in[got..frames].fill(0.0);
                        play_shared.starved.fetch_add(1, Ordering::Relaxed);
                    }

                    deid.process(&scratch_in[..frames], &mut scratch_out[..frames]);

                    // The veiled voice, at the one moment it exists. Copied
                    // into the recorder's ring here rather than read back from
                    // the device, which would be a second unprotected copy.
                    if let Some(sink) = veiled.as_mut() {
                        sink.write(&scratch_out[..frames]);
                    }

                    let mut peak = 0.0f32;
                    for (frame, &s) in data.chunks_mut(out_channels).zip(&scratch_out[..frames]) {
                        let v = s.clamp(-1.0, 1.0);
                        peak = peak.max(v.abs());
                        // The same mono signal to every output channel.
                        for slot in frame.iter_mut() {
                            *slot = v;
                        }
                    }
                    // Any tail beyond `frames` (only when the device asks for
                    // more than the ring can hold) stays silent.
                    for slot in data.iter_mut().skip(frames * out_channels) {
                        *slot = 0.0;
                    }

                    if let Ok(mut st) = play_shared.stats.try_lock() {
                        st.process = deid.stats();
                        st.output_peak = st.output_peak.max(peak);
                        st.dropped = play_shared.dropped.load(Ordering::Relaxed);
                        st.starved = play_shared.starved.load(Ordering::Relaxed);
                    }
                },
                {
                    let shared = Arc::clone(&shared);
                    move |e| shared.report(Side::Output, &e)
                },
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        input_stream
            .play()
            .map_err(|e| Error::Stream(e.to_string()))?;
        output_stream
            .play()
            .map_err(|e| Error::Stream(e.to_string()))?;

        Ok((
            Self {
                _input: input_stream,
                _output: output_stream,
                shared,
            },
            Kept {
                veiled: veiled_recorder,
                plain: plain_recorder,
            },
        ))
    }

    /// Read the current statistics, resetting the peak meters.
    ///
    /// Peaks reset on read so a meter shows the level since the last frame
    /// rather than the loudest moment since the session began.
    pub fn stats(&self) -> LiveStats {
        let Ok(mut s) = self.shared.stats.lock() else {
            return LiveStats::default();
        };
        let mut snapshot = *s;
        s.input_peak = 0.0;
        s.output_peak = 0.0;
        // Read here rather than written by the output callback, which is where
        // `dropped` and `starved` come from. A device that has gone stops
        // calling that callback, and the one number that has to survive a
        // stream which is no longer running is the one saying it stopped.
        snapshot.interfered = self.shared.troubles.load(Ordering::Relaxed);
        snapshot
    }

    /// What the platform last reported about either stream.
    ///
    /// `None` until something goes wrong. Asked when [`LiveStats::interfered`]
    /// moves, rather than every frame: this clones a `String`.
    pub fn interference(&self) -> Option<Interference> {
        self.shared.trouble.lock().ok()?.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_start_empty() {
        let s = LiveStats::default();
        assert_eq!(s.dropped, 0);
        assert_eq!(s.starved, 0);
        assert_eq!(s.input_peak, 0.0);
    }

    /// The ring must be long enough to absorb a typical device buffer, but not
    /// so long that it becomes an audible delay on its own.
    #[test]
    fn ring_length_is_a_sane_compromise() {
        for rate in [16_000u32, 44_100, 48_000, 96_000] {
            let capacity = ((rate as f32 * RING_MILLIS / 1000.0) as usize).max(2048);
            let millis = capacity as f32 / rate as f32 * 1000.0;
            assert!(
                millis >= 40.0,
                "{rate} Hz: {millis:.0} ms is too little jitter room"
            );
            assert!(
                millis <= 200.0,
                "{rate} Hz: {millis:.0} ms of latency is too much"
            );
        }
    }

    /// **Marker 132.** What the recorder is given is what the engine produced.
    ///
    /// The row this comes from asked for the samples reaching the recorder to
    /// be *checked* against the engine's output. They cannot differ, and a
    /// check would be a buffer compared with itself: the veiled sink is
    /// written from inside the output callback, from the same slice
    /// `Deidentifier::process` has just written into, and there is nothing
    /// between the two to interfere with.
    ///
    /// That is a property worth keeping rather than one worth measuring, so it
    /// is read out of the source. A change that took the recorder's samples
    /// from anywhere else, the device being the obvious candidate, would be a
    /// second unprotected copy of the audio and would fail here.
    #[test]
    fn the_recorder_is_fed_from_the_engine_and_from_nowhere_else() {
        // The code, not this module. The needles below appear in this test's
        // own assertion messages, and a guard that counts the strings it is
        // looking for is counting itself: `dialog.rs`'s guard hit exactly this
        // and it is recorded in the audit.
        let whole = include_str!("live.rs").replace("\r\n", "\n");
        let source = whole
            .split("\n#[cfg(test)]")
            .next()
            .expect("the code above the tests");

        // The engine writes into `scratch_out` and the veiled sink is written
        // from it, adjacent, inside the output callback.
        assert!(
            source.contains("deid.process(&scratch_in[..frames], &mut scratch_out[..frames]);"),
            "the engine no longer writes into `scratch_out`, so the assertion \
             below is about a buffer that has moved"
        );
        assert!(
            source.contains("sink.write(&scratch_out[..frames]);"),
            "the veiled recorder is no longer fed from the engine's own output \
             buffer. Whatever it is fed from now is a second copy of the audio, \
             and marker 132 is the claim that there is not one"
        );

        // The microphone side, the same way: from the downmix inside the input
        // callback, not from anything the platform hands back afterwards.
        assert!(
            source.contains("sink.write(&mono_scratch[..kept]);"),
            "the plain recorder is no longer fed from the downmix in the input \
             callback. That is the one moment the real voice exists in this \
             process, and taking it anywhere else means it exists twice"
        );
        assert_eq!(
            source.matches("sink.write(").count(),
            2,
            "there are exactly two sinks and two places they are written. A \
             third write is a third copy of somebody's voice"
        );
    }

    /// A stream error becomes something a caller can show.
    ///
    /// **Marker 132.** Before this, both error callbacks were `eprintln!` and
    /// nothing else: on Windows the desktop application has no console, so a
    /// device unplugged mid-call was silent and the recording carried on.
    #[test]
    fn trouble_is_recorded_rather_than_only_printed() {
        let shared = Shared::default();
        assert!(shared.trouble.lock().expect("a fresh lock").is_none());

        shared.report(Side::Input, &cpal::StreamError::DeviceNotAvailable);
        let first = shared
            .trouble
            .lock()
            .expect("a fresh lock")
            .clone()
            .expect("the report was kept");
        assert_eq!(first.side, Side::Input);
        assert!(first.device_gone, "a device that has gone says so");
        assert_eq!(first.count, 1);

        shared.report(
            Side::Output,
            &cpal::StreamError::BackendSpecific {
                err: cpal::BackendSpecificError {
                    description: "the mixer said no".to_string(),
                },
            },
        );
        let second = shared
            .trouble
            .lock()
            .expect("a fresh lock")
            .clone()
            .expect("the report was kept");
        assert_eq!(second.side, Side::Output);
        assert!(
            !second.device_gone,
            "only a missing device is a missing device"
        );
        assert!(
            second.said.contains("the mixer said no"),
            "what the platform said is passed through rather than summarised, \
             and it said {:?}",
            second.said
        );
        assert_eq!(second.count, 2, "the count is of both streams together");
    }
}
