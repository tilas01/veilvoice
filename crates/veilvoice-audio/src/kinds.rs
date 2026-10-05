// SPDX-License-Identifier: GPL-3.0-or-later
//! What the live path reports, kept apart from the live path itself.
//!
//! **Roadmap item 162.** These are plain data: which side something happened
//! on, what the meters read, what a session was asked to keep, what a device is
//! called. None of them touches `cpal`, so they are compiled whether or not the
//! `live` feature is on. The modules that do the capturing re-export them, so
//! `veilvoice_audio::live::LiveStats` is the same type in both builds, and a
//! front end built without live capture (the window on the BSDs) still has the
//! words to describe a session it cannot start.
//!
//! # In plain words
//!
//! The numbers and names the live microphone path reports, in a file of their
//! own so that a build with no microphone support still knows what they are.

use veilvoice_core::ProcessStats;

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
/// **Roadmap item 132.** The platform reports these on a callback of its own, and
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
    /// Whether that side's stream is over: the device it was using stopped
    /// existing, or the platform invalidated the stream.
    ///
    /// This is the swapped-device case and the unplugged-device case, and it
    /// is the platform saying so rather than this crate polling for it: a
    /// device list read once a second is a guess between reads, and on Windows
    /// enumerating devices from another thread is what F-163 and F-165 were.
    ///
    /// **F-245.** Only `DeviceNotAvailable` used to count. `cpal` 0.18 reports
    /// some of the same endings as `StreamInvalidated`, which it documents as
    /// a stream that "must be rebuilt": WASAPI when the endpoint's resources
    /// are invalidated under it, and the PulseAudio host when the server goes.
    /// No further sample arrives either way, and a take the Studio kept
    /// recording after one was recording nothing.
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
    /// **Roadmap item 132.** A count rather than the reports themselves, so this
    /// stays `Copy` and cheap to read every frame. A caller that sees it move
    /// asks [`crate::LiveSession::interference`] what happened.
    pub interfered: u64,
}

/// Which sides of the engine a session keeps.
///
/// Both are named at construction rather than passed as a pair of positional
/// flags, which is the property **roadmap item 131** asked for and which two separate
/// arguments used to give: no caller reaches a recording of somebody's real
/// voice without writing the word `plain` next to it.
///
/// The default keeps neither, which is what [`crate::LiveSession::start`] is.
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

/// The most microphones one room will open at once.
///
/// A bound on the arithmetic rather than a claim about any machine: what a
/// machine can actually carry is [`RoomStats::load`], measured while it runs.
/// Eight is past the number of people who can hold one conversation, and every
/// guest costs a device, a ring, an engine and a place in the mix.
pub const MAX_GUESTS: usize = 8;

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
    /// **Roadmap item 132**, the same counter the single-microphone path carries.
    pub interfered: u64,
    /// What every engine together costs against the deadline they share.
    ///
    /// The sum of each guest's realtime factor. Below 1.0 the machine keeps up;
    /// at 1.0 the engines have used the whole block, and past it the audio
    /// breaks. This is the honest account roadmap item 147 asked for.
    pub load: f32,
}

/// Which direction a device carries audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// A capture device (microphone, loopback).
    Input,
    /// A playback device (speakers, virtual cable).
    Output,
}

/// A device the user can choose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Human-readable device name, as the OS reports it.
    pub name: String,
    /// Whether this is the host's default device for its direction.
    pub is_default: bool,
    /// Whether the name matches a known virtual audio cable.
    pub is_virtual_cable: bool,
}
