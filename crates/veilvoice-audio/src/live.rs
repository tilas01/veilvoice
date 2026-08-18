// SPDX-License-Identifier: CC-BY-NC-SA-4.0
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
//! ring is intentionally short — enough to absorb jitter between two clocks
//! that are not synchronised, not enough to accumulate a delay the user would
//! notice while speaking.

use crate::Error;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use veilvoice_core::{DeidConfig, Deidentifier, ProcessStats};

/// How much jitter the ring absorbs before it starts dropping samples.
const RING_MILLIS: f32 = 120.0;

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
    /// playback) — a non-zero value means audible glitching.
    pub dropped: u64,
    /// Times the output callback found the ring empty and emitted silence.
    pub starved: u64,
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
        mut config: DeidConfig,
    ) -> Result<Self, Error> {
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

        let mut deid = Deidentifier::new(config).map_err(Error::Engine)?;

        let capacity = ((sample_rate as f32 * RING_MILLIS / 1000.0) as usize).max(2048);
        let (mut producer, mut consumer) = HeapRb::<f32>::new(capacity).split();

        let shared = Arc::new(Shared::default());
        let cap_shared = Arc::clone(&shared);
        let play_shared = Arc::clone(&shared);

        let input_stream = input
            .build_input_stream(
                &in_cfg.config(),
                move |data: &[f32], _| {
                    let mut peak = 0.0f32;
                    let mut dropped = 0u64;
                    // Downmix to mono: the engine is single channel, and a
                    // stereo image is itself a recording-setup fingerprint.
                    for frame in data.chunks(in_channels) {
                        let mono = frame.iter().sum::<f32>() / in_channels as f32;
                        peak = peak.max(mono.abs());
                        if producer.try_push(mono).is_err() {
                            dropped += 1;
                        }
                    }
                    if dropped > 0 {
                        cap_shared.dropped.fetch_add(dropped, Ordering::Relaxed);
                    }
                    // Never block the callback for a statistics update.
                    if let Ok(mut s) = cap_shared.stats.try_lock() {
                        s.input_peak = s.input_peak.max(peak);
                    }
                },
                move |e| eprintln!("veilvoice: input stream error: {e}"),
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
                move |e| eprintln!("veilvoice: output stream error: {e}"),
                None,
            )
            .map_err(|e| Error::Stream(e.to_string()))?;

        input_stream
            .play()
            .map_err(|e| Error::Stream(e.to_string()))?;
        output_stream
            .play()
            .map_err(|e| Error::Stream(e.to_string()))?;

        Ok(Self {
            _input: input_stream,
            _output: output_stream,
            shared,
        })
    }

    /// Read the current statistics, resetting the peak meters.
    ///
    /// Peaks reset on read so a meter shows the level since the last frame
    /// rather than the loudest moment since the session began.
    pub fn stats(&self) -> LiveStats {
        let Ok(mut s) = self.shared.stats.lock() else {
            return LiveStats::default();
        };
        let snapshot = *s;
        s.input_peak = 0.0;
        s.output_peak = 0.0;
        snapshot
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
}
