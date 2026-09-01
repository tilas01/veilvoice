// SPDX-License-Identifier: GPL-3.0-or-later
//! Enumerating audio devices, and guessing which of them are virtual cables.
//!
//! # What this is for
//!
//! Live scrambling is only useful if the veiled voice can be routed *into*
//! something else -- a call, a stream, a recorder. The way that is done on every
//! desktop platform is a **virtual audio cable**: a driver that presents a
//! playback device on one side and a microphone on the other, so anything that
//! can select a microphone can receive VeilVoice's output.
//!
//! So the list this module produces is not merely a list. Picking the wrong
//! output device is the single most common way for live mode to appear broken
//! while working perfectly, and the whole reason [`DeviceInfo::is_virtual_cable`]
//! exists is to put the right entry in front of the user.
//!
//! # The detection is name matching, and that is a limitation, not an oversight
//!
//! There is **no portable way to ask an audio device whether it is virtual**.
//! CPAL does not expose it because the underlying APIs largely do not either.
//! So [`VIRTUAL_CABLE_HINTS`] matches on name fragments, which means:
//!
//! * a cable this list has never heard of is reported as an ordinary device;
//! * a real device whose name happens to contain "loopback" or "virtual" is
//!   flagged when it should not be.
//!
//! Both are wrong in the harmless direction: the flag reorders and annotates a
//! list, it never restricts what the user may choose. A heuristic that hides
//! options would be a different and worse thing than one that highlights them,
//! and this is deliberately the second.
//!
//! The alternative -- showing an unsorted list of identically named endpoints
//! and letting the user find the right one -- was tried and is worse.
//!
//! # Enumeration can fail, and does
//!
//! Device lists come from the OS and are not stable: a device can disappear
//! between being listed and being opened, a host may have no devices at all,
//! and on Linux a machine with no sound server is entirely normal. Every
//! function here returns a [`crate::Error`] rather than panicking or quietly
//! returning an empty list, because an empty list and a failed query mean very
//! different things to somebody trying to work out why they cannot be heard.
//!
//! # In plain words
//!
//! This asks your computer which microphones and speakers it has, and works out
//! which of them are **virtual cables**.
//!
//! A virtual cable is a small piece of software that pretends to be a speaker on
//! one side and a microphone on the other. It is how a veiled voice gets into a
//! call: VeilVoice plays into the cable, and the calling program picks the cable
//! as its microphone and never knows the difference.
//!
//! Working out which device is a cable is done by recognising the names the common
//! ones use, so it is a good guess rather than a certainty. Nothing depends on the
//! guess being right: it decides which device is *suggested*, never which ones you
//! are allowed to choose.

use crate::Error;
use cpal::traits::{DeviceTrait, HostTrait};

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

/// Name fragments used by the common virtual audio cables.
///
/// Routing the veiled voice into one of these is what lets any other
/// application, whether a call, a stream or a recorder, receive it as if it were a
/// microphone. Matching on the name is crude, but there is no portable way to
/// ask an audio device whether it is virtual, and the alternative is making the
/// user hunt through a list of identically-named endpoints.
const VIRTUAL_CABLE_HINTS: &[&str] = &[
    "cable input",  // VB-CABLE (Windows), the one the installer offers
    "cable output", //
    "vb-audio",     // VB-Audio's other products
    "voicemeeter",  //
    "blackhole",    // macOS
    "soundflower",  // macOS, older
    "loopback",     // Rogue Amoeba, and some ALSA setups
    "pulse",        // PulseAudio null sink, commonly named this way
    "virtual",      // generic catch-all, last resort
];

fn looks_virtual(name: &str) -> bool {
    let lower = name.to_lowercase();
    VIRTUAL_CABLE_HINTS.iter().any(|h| lower.contains(h))
}

/// List the devices available in one direction.
pub fn list(direction: Direction) -> Result<Vec<DeviceInfo>, Error> {
    let host = cpal::default_host();
    let default_name = match direction {
        Direction::Input => host.default_input_device().and_then(|d| d.name().ok()),
        Direction::Output => host.default_output_device().and_then(|d| d.name().ok()),
    };

    let devices: Vec<cpal::Device> = match direction {
        Direction::Input => host
            .input_devices()
            .map_err(|e| Error::Device(e.to_string()))?
            .collect(),
        Direction::Output => host
            .output_devices()
            .map_err(|e| Error::Device(e.to_string()))?
            .collect(),
    };

    Ok(devices
        .into_iter()
        .filter_map(|d| d.name().ok())
        .map(|name| DeviceInfo {
            is_default: Some(&name) == default_name.as_ref(),
            is_virtual_cable: looks_virtual(&name),
            name,
        })
        .collect())
}

/// Find the first output device that looks like a virtual audio cable.
///
/// Returns `None` rather than an error when none is installed: that is a normal
/// state, and the caller should offer to install one rather than fail.
pub fn find_virtual_cable() -> Option<DeviceInfo> {
    list(Direction::Output)
        .ok()?
        .into_iter()
        .find(|d| d.is_virtual_cable)
}

/// The name of an opened device, or a placeholder when the OS will not say.
///
/// Saves every caller from depending on `cpal` just to print a device name.
pub fn name_of(device: &cpal::Device) -> String {
    device.name().unwrap_or_else(|_| "<unnamed device>".into())
}

/// Look up a device by exact name, or the host default when `name` is `None`.
pub fn open(direction: Direction, name: Option<&str>) -> Result<cpal::Device, Error> {
    let host = cpal::default_host();
    match name {
        None => match direction {
            Direction::Input => host.default_input_device(),
            Direction::Output => host.default_output_device(),
        }
        .ok_or_else(|| Error::Device("no default device".into())),
        Some(wanted) => {
            let mut devices: Box<dyn Iterator<Item = cpal::Device>> = match direction {
                Direction::Input => Box::new(
                    host.input_devices()
                        .map_err(|e| Error::Device(e.to_string()))?,
                ),
                Direction::Output => Box::new(
                    host.output_devices()
                        .map_err(|e| Error::Device(e.to_string()))?,
                ),
            };
            devices
                .find(|d| d.name().map(|n| n == wanted).unwrap_or(false))
                .ok_or_else(|| Error::Device(format!("no device named {wanted:?}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_cable_names_are_recognised() {
        for name in [
            "CABLE Input (VB-Audio Virtual Cable)",
            "VoiceMeeter Aux Input",
            "BlackHole 2ch",
            "Loopback Audio",
        ] {
            assert!(looks_virtual(name), "{name} should be recognised");
        }
    }

    #[test]
    fn ordinary_devices_are_not_mistaken_for_cables() {
        for name in [
            "Speakers (Realtek High Definition Audio)",
            "Headset Earphone",
            "HDMI Output",
        ] {
            assert!(!looks_virtual(name), "{name} should not be flagged");
        }
    }

    #[test]
    fn matching_ignores_case() {
        assert!(looks_virtual("cable input"));
        assert!(looks_virtual("CABLE INPUT"));
        assert!(looks_virtual("Cable Input"));
    }

    /// Enumeration must not panic or hang on a machine with no sound hardware,
    /// which is exactly what CI runners look like.
    #[test]
    fn enumeration_is_safe_without_audio_hardware() {
        for direction in [Direction::Input, Direction::Output] {
            match list(direction) {
                Ok(devices) => {
                    assert!(devices.iter().filter(|d| d.is_default).count() <= 1);
                }
                Err(Error::Device(_)) => {} // headless runner: acceptable
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
    }

    #[test]
    fn missing_virtual_cable_is_not_an_error() {
        let _ = find_virtual_cable();
    }

    #[test]
    fn opening_an_unknown_device_reports_its_name() {
        match open(Direction::Output, Some("definitely-not-a-real-device")) {
            Err(Error::Device(msg)) => assert!(msg.contains("definitely-not-a-real-device")),
            Err(e) => panic!("unexpected error: {e}"),
            Ok(_) => panic!("a nonexistent device should not open"),
        }
    }
}
