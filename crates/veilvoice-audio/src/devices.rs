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

pub use crate::kinds::{DeviceInfo, Direction};
use crate::Error;
use cpal::traits::{DeviceTrait, HostTrait};

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

/// Whether a device's name suggests it is a virtual cable rather than
/// real hardware.
///
/// A guess from a name, and treated as one everywhere it is used: it decides
/// what to suggest, never what to refuse.
fn looks_virtual(name: &str) -> bool {
    let lower = name.to_lowercase();
    VIRTUAL_CABLE_HINTS.iter().any(|h| lower.contains(h))
}

/// A device's name as the platform reports it, or `None` when it will not say.
///
/// `cpal` 0.18 replaced `Device::name` with a whole `DeviceDescription`, of
/// which the name is the only field anything here wants. Unwrapping it once
/// keeps that detail in one place rather than at all five call sites.
fn name_of_opt(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

/// A device as `list` shows it and `open` finds it.
///
/// **F-244.** `cpal` 0.18 named devices by their description, and on ALSA that
/// is the first line of the card's description, which every PCM on one card
/// shares. A laptop's one sound card was listed as the same name ten times or
/// more, for `sysdefault`, `front`, `hw`, `plughw`, `dsnoop` and the rest, and
/// `open` took whichever carried that name first, so choosing any entry but
/// the first opened a different device from the one chosen. And `cpal` 0.15,
/// which v0.1.22 used, named an ALSA device by its PCM, `hw:CARD=PCH,DEV=0`,
/// so a name typed from that release's `veilvoice devices` opened nothing.
struct Described {
    /// The name the platform gives it.
    name: String,
    /// The platform's own identifier for it: the PCM on ALSA.
    id: Option<String>,
}

/// What `cpal` says about a device, or `None` when it will not say.
fn describe(device: &cpal::Device) -> Option<Described> {
    Some(Described {
        name: name_of_opt(device)?,
        id: device.id().ok().map(|id| id.id().to_string()),
    })
}

/// The name each device is listed under.
///
/// Its own, unless another device in the same list has that name too, and
/// then with the platform's identifier after it in brackets. Only when it is
/// needed: an identifier on Windows is a GUID, which nobody wants to read when
/// the name alone already says which device it is.
fn listed_names(described: &[Option<Described>]) -> Vec<Option<String>> {
    let mut uses = std::collections::HashMap::<&str, usize>::new();
    for device in described.iter().flatten() {
        *uses.entry(device.name.as_str()).or_default() += 1;
    }
    described
        .iter()
        .map(|device| {
            device.as_ref().map(|device| match &device.id {
                Some(id) if uses[device.name.as_str()] > 1 => format!("{} ({id})", device.name),
                _ => device.name.clone(),
            })
        })
        .collect()
}

/// Which device a name asked for means, as a position in `described`.
///
/// The name [`list`] shows, first. Then the platform identifier on its own,
/// which is what an ALSA device was called before F-244, so a command written
/// against v0.1.22 still opens the device it named. Then the first device
/// whose own name it is, which is what this matched before F-244.
fn position_of(described: &[Option<Described>], wanted: &str) -> Option<usize> {
    let listed = listed_names(described);
    listed
        .iter()
        .position(|name| name.as_deref() == Some(wanted))
        .or_else(|| {
            described
                .iter()
                .position(|d| d.as_ref().and_then(|d| d.id.as_deref()) == Some(wanted))
        })
        .or_else(|| {
            described
                .iter()
                .position(|d| d.as_ref().map(|d| d.name.as_str()) == Some(wanted))
        })
}

/// Every device in one direction, in the order the platform gives them.
fn every(host: &cpal::Host, direction: Direction) -> Result<Vec<cpal::Device>, Error> {
    Ok(match direction {
        Direction::Input => host
            .input_devices()
            .map_err(|e| Error::Device(e.to_string()))?
            .collect(),
        Direction::Output => host
            .output_devices()
            .map_err(|e| Error::Device(e.to_string()))?
            .collect(),
    })
}

/// List the devices available in one direction.
///
/// Each name is one [`open`] finds that same device by, which F-244 is about.
pub fn list(direction: Direction) -> Result<Vec<DeviceInfo>, Error> {
    let host = cpal::default_host();
    let default = match direction {
        Direction::Input => host.default_input_device(),
        Direction::Output => host.default_output_device(),
    }
    .and_then(|d| describe(&d));

    let described: Vec<Option<Described>> = every(&host, direction)?.iter().map(describe).collect();
    let listed = listed_names(&described);

    Ok(described
        .iter()
        .zip(listed)
        .filter_map(|(device, name)| Some((device.as_ref()?, name?)))
        .map(|(device, name)| DeviceInfo {
            // By identifier where both have one, because two devices can
            // share a name and only one of them is the default.
            is_default: default
                .as_ref()
                .is_some_and(|default| match (&default.id, &device.id) {
                    (Some(a), Some(b)) => a == b,
                    _ => default.name == device.name,
                }),
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
    name_of_opt(device).unwrap_or_else(|| "<unnamed device>".into())
}

/// Look up a device by the name [`list`] gave it, or the host default when
/// `name` is `None`. See [`position_of`] for the names it also accepts.
pub fn open(direction: Direction, name: Option<&str>) -> Result<cpal::Device, Error> {
    let host = cpal::default_host();
    match name {
        None => match direction {
            Direction::Input => host.default_input_device(),
            Direction::Output => host.default_output_device(),
        }
        .ok_or_else(|| Error::Device("no default device".into())),
        Some(wanted) => {
            let devices = every(&host, direction)?;
            let described: Vec<Option<Described>> = devices.iter().map(describe).collect();
            position_of(&described, wanted)
                .and_then(|at| devices.into_iter().nth(at))
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

    /// One ALSA sound card, as `cpal` 0.18 describes its PCMs: one name, many
    /// identifiers.
    fn one_alsa_card() -> Vec<Option<Described>> {
        let card = "HDA Intel PCH, ALC3246 Analog";
        let mut described = vec![Some(Described {
            name: "Default ALSA Output (currently PipeWire Media Server)".into(),
            id: Some("default".into()),
        })];
        for pcm in [
            "sysdefault:CARD=PCH",
            "front:CARD=PCH,DEV=0",
            "hw:CARD=PCH,DEV=0",
        ] {
            described.push(Some(Described {
                name: card.into(),
                id: Some(pcm.into()),
            }));
        }
        described.push(None);
        described
    }

    /// F-244. Every name listed is different, and each opens the device it
    /// was listed for rather than the first with the same card name.
    #[test]
    fn every_listed_name_opens_the_device_it_was_listed_for() {
        let described = one_alsa_card();
        let listed = listed_names(&described);
        let names: Vec<&str> = listed.iter().flatten().map(String::as_str).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            names.len(),
            "two devices listed alike: {names:?}"
        );
        assert_eq!(
            names[0], "Default ALSA Output (currently PipeWire Media Server)",
            "a name nothing else shares is shown as it is"
        );
        assert_eq!(
            names[3],
            "HDA Intel PCH, ALC3246 Analog (hw:CARD=PCH,DEV=0)"
        );
        for (at, name) in listed.iter().enumerate() {
            if let Some(name) = name {
                assert_eq!(position_of(&described, name), Some(at), "{name}");
            }
        }
    }

    /// F-244. The names earlier builds used still find a device: the PCM that
    /// v0.1.22 listed, and the bare card name the development builds did.
    #[test]
    fn a_name_from_an_earlier_release_still_opens_something() {
        let described = one_alsa_card();
        assert_eq!(position_of(&described, "hw:CARD=PCH,DEV=0"), Some(3));
        assert_eq!(
            position_of(&described, "HDA Intel PCH, ALC3246 Analog"),
            Some(1)
        );
        assert_eq!(position_of(&described, "no such device"), None);
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
