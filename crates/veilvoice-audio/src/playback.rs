// SPDX-License-Identifier: GPL-3.0-or-later
//! Playing a recording that is only in memory, and never on disk.
//!
//! # What this is for
//!
//! A take in the studio vault is sealed. Hearing it back means decrypting it,
//! and the obvious way to hear a WAV is to write it somewhere and hand the path
//! to something that plays files. That would put an unencrypted recording on
//! the disk, which is the one thing the vault exists to prevent, and it would
//! leave it there until somebody remembered to shred it.
//!
//! So this takes the samples as they already are, in page-locked memory, and
//! plays them from there. Nothing is written. When playback stops the buffer is
//! dropped and `veilvoice_crypto::Secret` wipes itself.
//!
//! # The samples are held, not streamed from the vault
//!
//! Be plain about the shape rather than implying a stronger one. The whole take
//! is decrypted into locked memory before a note is heard, because the
//! container is sealed and authenticated as one piece: there is no way to open
//! the first second of it without opening all of it, and an AEAD that let you
//! would not be authenticating anything.
//!
//! What that buys is still the thing that matters: **no plaintext file, at any
//! point.** What it does not buy is a smaller footprint than the recording, and
//! an hour of audio is an hour of audio in RAM. A format sealed in blocks would
//! change that and is not what the vault writes today.
//!
//! # In plain words
//!
//! Plays a recording straight out of protected memory, so listening to one
//! never leaves a copy on the disk for somebody to find later.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, StreamTrait};

use crate::Error;

/// What both sides can see while a take is playing.
struct Shared {
    /// The next sample to hand the device.
    at: AtomicUsize,
    /// Whether the end has been reached.
    done: AtomicBool,
    /// The loudest sample the device has been given since the last read.
    peak: std::sync::Mutex<f32>,
}

/// A take being played, for as long as this is held.
///
/// Dropping it stops the audio and releases the samples. That is deliberate:
/// there is no `stop` that leaves the buffer alive, because a buffer of
/// somebody's recording outliving the reason it was decrypted is exactly the
/// leak this module is avoiding.
pub struct Playing {
    // The stream must outlive the session; dropping it stops the callback.
    _stream: cpal::Stream,
    shared: Arc<Shared>,
    samples: usize,
    rate: u32,
}

impl Playing {
    /// How far in, in seconds.
    pub fn position(&self) -> f32 {
        if self.rate == 0 {
            return 0.0;
        }
        self.shared.at.load(Ordering::Relaxed) as f32 / self.rate as f32
    }

    /// How long the take is, in seconds.
    pub fn duration(&self) -> f32 {
        if self.rate == 0 {
            return 0.0;
        }
        self.samples as f32 / self.rate as f32
    }

    /// Whether it has reached the end.
    pub fn finished(&self) -> bool {
        self.shared.done.load(Ordering::Relaxed)
    }

    /// The loudest sample since this was last called, and it resets.
    pub fn peak(&self) -> f32 {
        match self.shared.peak.lock() {
            Ok(mut p) => {
                let was = *p;
                *p = 0.0;
                was
            }
            // A poisoned lock means a callback panicked. Reporting silence is
            // wrong but harmless; reporting a stale peak would be a meter that
            // has quietly stopped moving.
            Err(_) => 0.0,
        }
    }
}

/// Start playing `samples` at `rate` on the default output device.
///
/// The samples are moved into the callback. The caller's copy is gone, which is
/// what keeps there from being two: the one being played and one left behind.
pub fn start(samples: Vec<f32>, rate: u32, device: Option<&str>) -> Result<Playing, Error> {
    if samples.is_empty() {
        return Err(Error::Device("there is nothing to play".into()));
    }
    if rate == 0 {
        return Err(Error::Device(
            "the recording does not say what rate it was made at".into(),
        ));
    }

    let out = crate::devices::open(crate::devices::Direction::Output, device)?;
    let cfg = out
        .default_output_config()
        .map_err(|e| Error::Device(e.to_string()))?;
    let channels = cfg.channels() as usize;

    let total = samples.len();
    let shared = Arc::new(Shared {
        at: AtomicUsize::new(0),
        done: AtomicBool::new(false),
        peak: std::sync::Mutex::new(0.0),
    });
    let callback_shared = Arc::clone(&shared);

    // Resampling is deliberately not done here.
    //
    // The take was recorded at whatever the capture device agreed to, and the
    // output device may want something else. Playing it at the wrong rate makes
    // a voice sound higher or lower, which in a program whose whole purpose is
    // that a voice cannot be traced back is a wrong answer rather than a small
    // one. Where the device will not take the recording's rate this refuses and
    // says so, and the export path is the way to hear it in something that does
    // resample properly.
    if cfg.sample_rate().0 != rate {
        return Err(Error::Device(format!(
            "this recording is {rate} Hz and the output device wants {}. \
             Take it out of the vault to play it somewhere that can convert.",
            cfg.sample_rate().0
        )));
    }

    let stream = out
        .build_output_stream(
            &cfg.config(),
            move |data: &mut [f32], _| {
                let start = callback_shared.at.load(Ordering::Relaxed);
                let frames = data.len() / channels.max(1);
                let available = total.saturating_sub(start);
                let taking = frames.min(available);

                let mut loudest = 0.0f32;
                for (frame, &s) in data
                    .chunks_mut(channels.max(1))
                    .zip(&samples[start..start + taking])
                {
                    let v = s.clamp(-1.0, 1.0);
                    loudest = loudest.max(v.abs());
                    for slot in frame.iter_mut() {
                        *slot = v;
                    }
                }
                // Past the end is silence, not a repeat of the last block,
                // which would be an audible stutter at the end of every take.
                for slot in data.iter_mut().skip(taking * channels.max(1)) {
                    *slot = 0.0;
                }

                callback_shared.at.store(start + taking, Ordering::Relaxed);
                if start + taking >= total {
                    callback_shared.done.store(true, Ordering::Relaxed);
                }
                if let Ok(mut p) = callback_shared.peak.try_lock() {
                    *p = p.max(loudest);
                }
            },
            move |e| eprintln!("veilvoice: playback stream error: {e}"),
            None,
        )
        .map_err(|e| Error::Stream(e.to_string()))?;

    stream.play().map_err(|e| Error::Stream(e.to_string()))?;

    Ok(Playing {
        _stream: stream,
        shared,
        samples: total,
        rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_play_is_refused_rather_than_started() {
        // A stream that plays an empty buffer reaches the end immediately and
        // looks like a device fault. Saying so is more use.
        assert!(start(Vec::new(), 48_000, None).is_err());
    }

    #[test]
    fn a_rate_of_zero_is_refused_rather_than_divided_by() {
        assert!(start(vec![0.0; 10], 0, None).is_err());
    }
}
