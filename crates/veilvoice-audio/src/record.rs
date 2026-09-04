// SPDX-License-Identifier: GPL-3.0-or-later
//! Recording the veiled voice without it ever reaching unprotected memory.
//!
//! # What this is for
//!
//! [`live`](crate::live) sends the veiled voice to a device and keeps nothing.
//! This keeps it, and the whole difficulty is *where*. A recording that is
//! accumulated in a `Vec`, encoded with a library that returns a `Vec`, and
//! then sealed, has existed in unlocked, unzeroized memory three times over by
//! the time it is encrypted, and the operating system may have written any of
//! those copies to the page file. Sealing it afterwards does not take that
//! back.
//!
//! So the recording lives in a [`Tape`] from the first sample to the last, the
//! WAV is assembled inside a [`Secret`], and the only thing that leaves this
//! module is that sealed-ready `Secret`. There is no route here that produces a
//! plain `Vec` of the audio, because a route that existed would eventually be
//! taken.
//!
//! # Never a plaintext file, either
//!
//! Nothing here writes to disk at all. The caller seals the [`Secret`] and
//! writes the result. A recorder that wrote a WAV and encrypted it afterwards
//! would leave a plaintext file that
//! [`veilvoice_crypto::shred`](../../veilvoice_crypto/shred/index.html) explains
//! cannot be
//! reliably taken back on flash storage, which is the whole reason at-rest
//! encryption is the default rather than an option.
//!
//! # The two halves, and why they are split
//!
//! [`Sink`] is handed to the audio callback and [`Recorder`] is kept by the
//! caller. They are joined by a lock-free ring buffer, for the reason the
//! [`live`](crate::live) module documentation gives: a callback that allocates
//! or waits produces a dropout, and locking a page or growing a tape does both.
//! So the callback only ever pushes into a buffer that is already allocated,
//! and the slow, careful work of moving those samples into locked memory
//! happens on the caller's thread in [`Recorder::drain`].
//!
//! A caller that stops draining does not stall the audio. The ring fills, and
//! samples are counted as dropped rather than waited for, because a glitch in a
//! recording is better than a glitch in the live output somebody is speaking
//! into. [`Recorder::dropped`] reports it rather than letting the recording be
//! quietly short.
//!
//! # In plain words
//!
//! Keeps the veiled voice as it is produced, in memory the operating system has
//! been asked not to write to disk, and hands it over ready to be encrypted.
//!
//! It never writes an unencrypted recording anywhere, not even briefly, because
//! a file that is written and deleted can still be recovered from the disk
//! afterwards.

use crate::Error;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use veilvoice_crypto::{Secret, Tape};

/// Bytes in a canonical 16-bit PCM WAV header.
const HEADER: usize = 44;

/// The most PCM data a RIFF/WAVE file can describe, in bytes.
///
/// Not a limit this module chose. A WAV header states its sizes in unsigned
/// 32-bit fields, so the format itself cannot describe more, and the `RIFF`
/// size field has to hold the data plus the 36 bytes of header around it.
/// At 48 kHz, 16-bit, mono, this is a little over twelve hours.
///
/// It is checked rather than wrapped. A cast would produce a header claiming a
/// fraction of the real length, and that file opens, plays, and is silently
/// short: the worst shape a defect can take on a recording somebody made once
/// and cannot make again.
const WAV_MAX_DATA: usize = u32::MAX as usize - 36;

/// How much audio the ring holds before samples are dropped, in seconds.
///
/// Generous on purpose. The ring exists to absorb the gap between an audio
/// callback that runs every few milliseconds and a caller that drains when it
/// gets round to it, and a caller doing a screen redraw between drains is
/// normal. Sized in seconds rather than samples so the slack does not shrink
/// when the device runs at a higher rate.
pub const SLACK_SECONDS: f32 = 8.0;

/// The writing half, handed to the audio callback.
///
/// Every method is safe to call from a realtime audio callback: no allocation,
/// no locking, no syscall, no waiting.
pub struct Sink {
    producer: HeapProd<f32>,
    dropped: Arc<AtomicU64>,
}

impl Sink {
    /// Take a block of veiled samples.
    ///
    /// Samples that do not fit are counted and discarded rather than waited
    /// for. Blocking here would stall the output callback and glitch the audio
    /// the speaker is producing, to protect a recording of it, which is the
    /// wrong way round.
    pub fn write(&mut self, samples: &[f32]) {
        let taken = self.producer.push_slice(samples);
        if taken < samples.len() {
            self.dropped
                .fetch_add((samples.len() - taken) as u64, Ordering::Relaxed);
        }
    }
}

/// The reading half: moves samples out of the ring and into locked memory.
pub struct Recorder {
    consumer: HeapCons<f32>,
    tape: Tape,
    dropped: Arc<AtomicU64>,
    sample_rate: u32,
    samples: usize,
    /// Drain scratch, allocated once. Not protected memory, and does not need
    /// to be: it holds at most one drain's worth and is overwritten every time,
    /// but it is wiped in [`Recorder::wav`] so the last block does not sit in
    /// it after the recording is handed over.
    scratch: Vec<f32>,
}

/// Start a recorder and the sink that feeds it.
///
/// `sample_rate` is the rate the device actually agreed to, not the one that
/// was asked for: it is written into the WAV header, and a header that
/// disagrees with the samples plays back at the wrong speed and the wrong
/// pitch, which on a de-identified recording would be a second voice change
/// nobody chose.
pub fn start(sample_rate: u32) -> (Recorder, Sink) {
    let capacity = ((sample_rate as f32 * SLACK_SECONDS) as usize).max(4096);
    let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
    let dropped = Arc::new(AtomicU64::new(0));
    let recorder = Recorder {
        consumer,
        tape: Tape::new(),
        dropped: Arc::clone(&dropped),
        sample_rate,
        samples: 0,
        scratch: vec![0.0; 8192],
    };
    (recorder, Sink { producer, dropped })
}

impl Recorder {
    /// Move everything waiting in the ring into the tape.
    ///
    /// Returns how many samples moved. Call this regularly: the ring holds
    /// [`SLACK_SECONDS`] and drops what does not fit.
    ///
    /// Samples are converted to 16-bit here rather than at the end, so the tape
    /// holds exactly the bytes the WAV will carry and the final step is a copy
    /// rather than a second pass over the whole recording. Values are clamped
    /// rather than allowed to wrap, for the reason
    /// [`io::wav_bytes`](crate::io::wav_bytes) gives: a sample past full scale
    /// that wrapped would flip sign and produce a loud click.
    pub fn drain(&mut self) -> usize {
        let mut moved = 0;
        loop {
            let got = self.consumer.pop_slice(&mut self.scratch);
            if got == 0 {
                break;
            }
            // A small stack buffer keeps the conversion allocation-free.
            let mut bytes = [0u8; 2 * 512];
            for block in self.scratch[..got].chunks(512) {
                for (pair, &s) in bytes.chunks_mut(2).zip(block) {
                    let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
                    pair.copy_from_slice(&v.to_le_bytes());
                }
                self.tape.push(&bytes[..block.len() * 2]);
            }
            self.samples += got;
            moved += got;
        }
        moved
    }

    /// Samples recorded so far, as of the last [`Recorder::drain`].
    pub fn samples(&self) -> usize {
        self.samples
    }

    /// Length so far in seconds, as of the last [`Recorder::drain`].
    pub fn seconds(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples as f32 / self.sample_rate as f32
    }

    /// Samples lost because the caller did not drain in time.
    ///
    /// Non-zero means the recording is short by this many samples and has a gap
    /// rather than a glitch. Worth reporting: a recording that is quietly
    /// missing a second of speech is worse than one that says so.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Whether every page holding the recording is locked out of swap.
    ///
    /// False is not a failure, as [`veilvoice_crypto::tape`] explains: the
    /// operating system's lock budget is small and unprivileged processes
    /// cannot raise it. It is surfaced so the caller can say what was actually
    /// obtained rather than imply a guarantee.
    pub fn fully_locked(&self) -> bool {
        self.tape.fully_locked()
    }

    /// The sample rate written into the WAV header.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Drain what is left and hand over the recording as a WAV, in a
    /// [`Secret`], ready to be sealed.
    ///
    /// Returns a `Secret` rather than a `Vec` deliberately. This is the moment
    /// the recording is complete and therefore at its most worth protecting,
    /// and it is exactly the moment a convenient `Vec` would put all of it into
    /// memory that can be paged to disk and is never wiped.
    ///
    /// The header is written by hand rather than through the WAV library for
    /// the same reason: that library builds its output in a `Vec` it grows and
    /// returns, which is the copy this module exists to avoid.
    pub fn wav(&mut self) -> Result<Secret, Error> {
        self.drain();
        let data = self.tape.len();
        // Refused before the allocation, not after: a recording this long is
        // already several gigabytes, and finding out afterwards would mean
        // reserving all of it to then throw it away.
        if data > WAV_MAX_DATA {
            return Err(Error::TooLong(data));
        }
        let mut out = Secret::zeroed(HEADER + data);
        write_header(out.expose_mut(), self.sample_rate, data);
        self.tape
            .copy_into(&mut out.expose_mut()[HEADER..])
            .map_err(Error::Crypto)?;
        // The last drained block is still in the scratch buffer, which is not
        // protected memory. Nothing else wipes it.
        self.wipe_scratch();
        Ok(out)
    }

    /// Wipe the recording held so far and start again from nothing.
    pub fn discard(&mut self) {
        self.tape.wipe();
        self.samples = 0;
        self.wipe_scratch();
    }

    /// Clear the drain scratch.
    ///
    /// The one place this is written. `wav`, `discard` and the destructor all
    /// call it rather than each clearing the buffer themselves, so there is no
    /// second copy of the operation to fall out of step with the others, and a
    /// test of this function is a test of what the destructor actually runs.
    fn wipe_scratch(&mut self) {
        self.scratch.iter_mut().for_each(|s| *s = 0.0);
    }
}

impl Drop for Recorder {
    /// Wipe the drain scratch.
    ///
    /// The tape wipes itself: every chunk of it is a [`Secret`]. The scratch
    /// buffer is not, and it holds up to one drain's worth of veiled audio in
    /// ordinary heap memory. Without this, abandoning a recording (Ctrl-C, an
    /// error on the way to sealing, or simply dropping the recorder) frees
    /// those samples without clearing them, and the allocator is then free to
    /// hand that memory, contents intact, to anything else in the process.
    ///
    /// # What this does not reach
    ///
    /// The ring buffer between [`Sink`] and [`Recorder`] also holds veiled
    /// samples, up to [`SLACK_SECONDS`] of them, and `ringbuf` exposes no way
    /// to clear its backing storage. Those bytes are freed unwiped and this
    /// module cannot prevent it. It is written down rather than left for
    /// somebody to discover: the exposure is veiled audio, which is the same
    /// audio the file holds and is not key material, but it is not nothing and
    /// claiming the recorder wipes everything would be false.
    fn drop(&mut self) {
        self.wipe_scratch();
    }
}

/// Write a canonical 44-byte mono 16-bit PCM WAV header into `out`.
///
/// `out` must be at least [`HEADER`] bytes; callers here always size it from
/// the same constant.
fn write_header(out: &mut [u8], sample_rate: u32, data_len: usize) {
    // `wav` refuses anything past `WAV_MAX_DATA` before calling here, so both
    // of these fit. They are still written as saturating rather than wrapping
    // conversions: if that guard is ever moved or lost, an over-long file
    // becomes an obviously wrong header rather than a plausible short one.
    let data_len = u32::try_from(data_len).unwrap_or(u32::MAX);
    let riff = data_len.saturating_add(36);
    let byte_rate = sample_rate.saturating_mul(2); // one channel, two bytes
    out[0..4].copy_from_slice(b"RIFF");
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    out[8..12].copy_from_slice(b"WAVE");
    out[12..16].copy_from_slice(b"fmt ");
    out[16..20].copy_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM, uncompressed
    out[22..24].copy_from_slice(&1u16.to_le_bytes()); // mono
    out[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    out[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    out[32..34].copy_from_slice(&2u16.to_le_bytes()); // block align
    out[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
    out[36..40].copy_from_slice(b"data");
    out[40..44].copy_from_slice(&data_len.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode a WAV back to samples with the same library the rest of the crate
    /// reads files with, so the header is checked by something other than the
    /// code that wrote it.
    fn decode(wav: &[u8]) -> (u32, Vec<i16>) {
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).expect("a readable WAV");
        let rate = reader.spec().sample_rate;
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples = reader.into_samples::<i16>().map(|s| s.unwrap()).collect();
        (rate, samples)
    }

    #[test]
    fn the_header_this_writes_is_one_a_wav_reader_accepts() {
        // Written by hand to keep the recording out of a library's Vec, so it
        // is checked against a real decoder rather than against itself.
        let (mut rec, mut sink) = start(48_000);
        sink.write(&[0.0, 0.5, -0.5, 1.0]);
        let wav = rec.wav().unwrap();

        let (rate, samples) = decode(wav.expose());
        assert_eq!(rate, 48_000);
        assert_eq!(samples.len(), 4);
    }

    #[test]
    fn what_was_spoken_is_what_comes_back() {
        let (mut rec, mut sink) = start(16_000);
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 / 40.0).sin() * 0.8).collect();
        sink.write(&input);
        rec.drain();
        let wav = rec.wav().unwrap();

        let (_, got) = decode(wav.expose());
        assert_eq!(got.len(), input.len());
        for (i, (&want, &have)) in input.iter().zip(&got).enumerate() {
            let expected = (want.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            assert_eq!(have, expected, "sample {i} came back changed");
        }
    }

    #[test]
    fn a_sample_past_full_scale_is_clamped_rather_than_wrapped() {
        // Wrapping would flip the sign and produce a loud click, which on a
        // recording somebody is going to publish is the worst kind of defect:
        // inaudible while monitoring and obvious in the file.
        let (mut rec, mut sink) = start(8_000);
        sink.write(&[2.0, -2.0, 1.0, -1.0]);
        let wav = rec.wav().unwrap();

        let (_, got) = decode(wav.expose());
        assert_eq!(got, vec![i16::MAX, -i16::MAX, i16::MAX, -i16::MAX]);
    }

    #[test]
    fn draining_in_pieces_gives_the_same_recording_as_draining_at_the_end() {
        // The caller drains whenever it gets round to it, so the result must
        // not depend on how often that was.
        let input: Vec<f32> = (0..5000).map(|i| ((i % 97) as f32 / 97.0) - 0.5).collect();

        let (mut often, mut sink_a) = start(44_100);
        for block in input.chunks(64) {
            sink_a.write(block);
            often.drain();
        }
        let a = often.wav().unwrap();

        let (mut once, mut sink_b) = start(44_100);
        for block in input.chunks(64) {
            sink_b.write(block);
        }
        let b = once.wav().unwrap();

        assert_eq!(a.expose(), b.expose());
    }

    #[test]
    fn a_recording_nobody_drained_reports_what_it_lost_rather_than_going_short_in_silence() {
        // The ring is finite. Overrunning it has to be counted, because a
        // recording quietly missing a second of speech is worse than one that
        // says so.
        let (mut rec, mut sink) = start(8_000); // 8 s of slack at 8 kHz
        let flood = vec![0.25f32; 8_000 * 20];
        sink.write(&flood);

        assert!(rec.dropped() > 0, "the overrun was not counted");
        rec.drain();
        assert!(
            rec.samples() < flood.len(),
            "more samples arrived than the ring can hold"
        );
        assert_eq!(
            rec.samples() as u64 + rec.dropped(),
            flood.len() as u64,
            "every sample must be either recorded or counted as dropped"
        );
    }

    #[test]
    fn an_empty_recording_is_a_valid_wav_of_no_length() {
        let (mut rec, _sink) = start(48_000);
        let wav = rec.wav().unwrap();
        assert_eq!(wav.len(), HEADER, "a header and no samples");
        let (rate, samples) = decode(wav.expose());
        assert_eq!(rate, 48_000);
        assert!(samples.is_empty());
    }

    #[test]
    fn the_length_reported_matches_the_samples_recorded() {
        let (mut rec, mut sink) = start(1000);
        sink.write(&vec![0.1f32; 2500]);
        rec.drain();
        assert_eq!(rec.samples(), 2500);
        assert!((rec.seconds() - 2.5).abs() < 1e-6, "{}", rec.seconds());
    }

    #[test]
    fn discarding_leaves_a_recorder_that_records_again_from_nothing() {
        let (mut rec, mut sink) = start(8_000);
        sink.write(&vec![0.5f32; 100]);
        rec.drain();
        assert_eq!(rec.samples(), 100);

        rec.discard();
        assert_eq!(rec.samples(), 0);

        sink.write(&[0.25, 0.25]);
        let wav = rec.wav().unwrap();
        let (_, got) = decode(wav.expose());
        assert_eq!(got.len(), 2, "the discarded audio came back");
    }

    #[test]
    fn the_recording_is_handed_over_in_protected_memory() {
        // The type is the guarantee: `wav` returns a Secret, which is wiped on
        // drop and locked where the OS allows. This asserts the shape rather
        // than the locking, which is best-effort and budget-dependent.
        let (mut rec, mut sink) = start(8_000);
        sink.write(&vec![0.5f32; 64]);
        let wav: Secret = rec.wav().unwrap();
        assert_eq!(wav.len(), HEADER + 128);
    }

    #[test]
    fn a_recording_too_long_for_the_format_is_refused_rather_than_truncated() {
        // A WAV states its sizes in 32-bit fields. Casting a longer length into
        // one wraps, and the file that comes out opens, plays, and is silently
        // a fraction of its real length. On a recording somebody made once,
        // that is the worst shape a defect can take, so the limit is checked.
        //
        // The limit itself is asserted rather than the behaviour of a
        // multi-gigabyte allocation: at 48 kHz 16-bit mono this is a little
        // over twelve hours, and a test that actually recorded that would not
        // be a test anybody runs.
        assert_eq!(WAV_MAX_DATA, u32::MAX as usize - 36);

        // Twelve hours fits; thirteen does not. Both are computed the way the
        // recorder computes a length, so the boundary is checked in the units
        // a caller thinks in.
        let per_second = 48_000 * 2;
        assert!(12 * 3600 * per_second < WAV_MAX_DATA);
        assert!(13 * 3600 * per_second > WAV_MAX_DATA);
    }

    #[test]
    fn the_header_stays_wrong_rather_than_plausible_if_the_guard_is_ever_lost() {
        // `write_header` is only reached after `wav` has refused an over-long
        // recording. This asserts the second line of defence: were that guard
        // moved or removed, the length written saturates rather than wrapping,
        // so the file is obviously broken instead of quietly short.
        let mut out = [0u8; HEADER];
        write_header(&mut out, 48_000, usize::MAX);
        let data = u32::from_le_bytes(out[40..44].try_into().unwrap());
        let riff = u32::from_le_bytes(out[4..8].try_into().unwrap());
        assert_eq!(data, u32::MAX, "a wrapped length would look plausible");
        assert_eq!(riff, u32::MAX);
    }

    #[test]
    fn abandoning_a_recording_does_not_leave_the_scratch_buffer_full_of_audio() {
        // The tape wipes itself, chunk by chunk, because every chunk is a
        // Secret. The scratch buffer is ordinary heap memory, and before this
        // it was freed with the last drained block still in it: Ctrl-C, or any
        // error on the way to sealing, handed that memory back to the
        // allocator with the audio intact.
        //
        // Reading freed memory is not something a safe test can do, so this
        // checks the state `drop` leaves rather than the heap afterwards: the
        // same buffer, wiped by the same code path the destructor runs.
        let (mut rec, mut sink) = start(8_000);
        sink.write(&vec![0.75f32; 2048]);
        rec.drain();
        assert!(
            rec.scratch.iter().any(|&s| s != 0.0),
            "the drain left nothing in the scratch, so this proves nothing"
        );

        // The very function the destructor calls, not a copy of it.
        rec.wipe_scratch();
        assert!(
            rec.scratch.iter().all(|&s| s == 0.0),
            "audio survived in the scratch buffer"
        );
    }

    #[test]
    fn a_zero_sample_rate_does_not_divide_by_it() {
        let (rec, _sink) = start(0);
        assert_eq!(rec.seconds(), 0.0);
    }
}
