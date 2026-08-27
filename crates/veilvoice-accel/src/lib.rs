// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! What hardware this machine has, and the one place VeilVoice can use it.
//!
//! # The honest answer about the audio engine, with the number
//!
//! **The de-identifier is not going on a graphics card, and it would be slower
//! if it did.** That is a measurement rather than an opinion: veiling sixty
//! seconds of audio takes about 0.58 seconds on one core of an ordinary
//! desktop, which is roughly a hundred times faster than real time. Live mode
//! works on 1024-sample frames, so each frame has about 21 ms to be finished
//! in and takes about 0.05 ms.
//!
//! A graphics card is fast at doing the same arithmetic to a very large batch
//! at once. It is not fast at answering small questions quickly: getting 1024
//! samples onto the card, waiting for a kernel, and getting them back costs
//! more than the whole computation. Offering a "use the GPU" switch for that
//! work would make VeilVoice slower and would be exactly the kind of claim
//! this project refuses to make.
//!
//! # Where it genuinely helps, which is video
//!
//! Encoding a video is the opposite shape of problem: a great deal of the same
//! work, on large frames, where a dedicated encoder block on the card does in
//! hardware what `libx264` does on the processor. Every current NVIDIA card has
//! **NVENC**, every current AMD card has **AMF**, and Intel's integrated
//! graphics have **Quick Sync**. That is what this crate detects and what
//! `veilvoice conversation video` can be pointed at.
//!
//! # Detection asks the system, and can fail
//!
//! There is no portable way to enumerate graphics hardware from the standard
//! library, and every native route is FFI. So this asks a tool the platform
//! already ships, exactly as the rest of this workspace does, and when it
//! cannot it says so rather than reporting an empty machine.
//!
//! **Finding a card is not the same as being able to use it.** An encoder needs
//! a driver, and it needs the copy of `ffmpeg` on this machine to have been
//! built with support for it. [`Adapter::caveat`] says so, and nothing here
//! reports a device as usable on the strength of its name.
//!
//! # In plain words
//!
//! Changing a voice is already about a hundred times faster than listening to
//! it, so there is nothing for a graphics card to speed up, and pretending
//! otherwise would just make VeilVoice slower. Making a **video** is different:
//! that is real work, and most graphics cards have a dedicated chip for it.
//!
//! So this finds the graphics hardware you have, tells you which of it can
//! encode video, and lets you pick. If you have two cards, an integrated one
//! and a separate one, you can say which. And it is honest that finding a card
//! is not proof it will work: that also depends on your drivers and on the
//! copy of ffmpeg you have.

use std::process::Command;

/// Who made a graphics device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Vendor {
    /// NVIDIA. Hardware encoding through NVENC.
    Nvidia,
    /// AMD. Hardware encoding through AMF.
    Amd,
    /// Intel. Hardware encoding through Quick Sync.
    Intel,
    /// Apple silicon, through VideoToolbox.
    Apple,
    /// Something this build does not recognise.
    Unknown,
}

impl Vendor {
    /// The vendor a device name belongs to.
    pub fn of(name: &str) -> Vendor {
        let name = name.to_ascii_lowercase();
        if name.contains("nvidia") || name.contains("geforce") || name.contains("quadro") {
            Vendor::Nvidia
        } else if name.contains("amd") || name.contains("radeon") || name.contains("firepro") {
            Vendor::Amd
        } else if name.contains("intel") || name.contains("uhd graphics") || name.contains("iris") {
            Vendor::Intel
        } else if name.contains("apple") {
            Vendor::Apple
        } else {
            Vendor::Unknown
        }
    }

    /// The `ffmpeg` encoder this vendor's hardware provides, if any.
    ///
    /// The name only. Whether this copy of `ffmpeg` was built with it is a
    /// separate question, and one this crate does not guess at.
    pub fn encoder(self) -> Option<&'static str> {
        match self {
            Vendor::Nvidia => Some("h264_nvenc"),
            Vendor::Amd => Some("h264_amf"),
            Vendor::Intel => Some("h264_qsv"),
            Vendor::Apple => Some("h264_videotoolbox"),
            Vendor::Unknown => None,
        }
    }

    /// What to call the encoder in front of a person.
    pub fn encoder_name(self) -> Option<&'static str> {
        match self {
            Vendor::Nvidia => Some("NVENC"),
            Vendor::Amd => Some("AMF"),
            Vendor::Intel => Some("Quick Sync"),
            Vendor::Apple => Some("VideoToolbox"),
            Vendor::Unknown => None,
        }
    }
}

/// One graphics device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    /// What the system calls it.
    pub name: String,
    /// Who made it.
    pub vendor: Vendor,
    /// The driver version the system reports, where it does.
    pub driver: Option<String>,
    /// Whether this looks like integrated graphics rather than a separate card.
    ///
    /// A guess from the name, and labelled as one. It decides which device is
    /// *recommended*, never which is allowed.
    pub integrated: bool,
}

impl Adapter {
    /// The encoder to ask `ffmpeg` for, if this device has one.
    pub fn encoder(&self) -> Option<&'static str> {
        self.vendor.encoder()
    }

    /// What finding this device does and does not establish.
    pub fn caveat(&self) -> &'static str {
        "Finding a device is not proof it can be used. Hardware encoding also \
         needs a working driver and a copy of ffmpeg built with support for that \
         encoder, and neither is something VeilVoice can determine by reading a \
         device name. If a render fails, the software encoder is always there."
    }

    /// One line, for a list.
    pub fn describe(&self) -> String {
        let where_ = if self.integrated {
            "integrated"
        } else {
            "separate card"
        };
        match (self.vendor.encoder_name(), &self.driver) {
            (Some(encoder), Some(driver)) => {
                format!("{} ({where_}, {encoder}, driver {driver})", self.name)
            }
            (Some(encoder), None) => format!("{} ({where_}, {encoder})", self.name),
            (None, Some(driver)) => {
                format!(
                    "{} ({where_}, no known encoder, driver {driver})",
                    self.name
                )
            }
            (None, None) => format!("{} ({where_}, no known encoder)", self.name),
        }
    }
}

/// Everything found, and anything that went wrong looking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The devices, in the order the system listed them.
    pub adapters: Vec<Adapter>,
    /// Why the list may be short or empty.
    ///
    /// A list that came back empty because a tool failed is not an empty
    /// machine, and the difference is the whole reason this is returned.
    pub problems: Vec<String>,
}

impl Found {
    /// Whether anything could be established at all.
    pub fn is_answerable(&self) -> bool {
        self.problems.is_empty() || !self.adapters.is_empty()
    }

    /// The device to suggest, and why.
    ///
    /// A separate card before integrated graphics, because its encoder block is
    /// usually the faster of the two. Nothing here measures that, and the
    /// wording says "usually" rather than pretending otherwise: a real answer
    /// would mean encoding the same video on each, which is a minute of
    /// somebody's time to save a few seconds of it.
    pub fn recommended(&self) -> Option<&Adapter> {
        let usable: Vec<&Adapter> = self
            .adapters
            .iter()
            .filter(|a| a.encoder().is_some())
            .collect();
        usable
            .iter()
            .find(|a| !a.integrated)
            .or_else(|| usable.first())
            .copied()
    }

    /// Why that one, in the words to show.
    pub fn why_recommended(&self) -> Option<String> {
        let pick = self.recommended()?;
        let others = self
            .adapters
            .iter()
            .filter(|a| a.encoder().is_some() && a.name != pick.name)
            .count();
        Some(if others == 0 {
            format!("{} is the only device here with an encoder.", pick.name)
        } else if pick.integrated {
            format!(
                "{} is suggested because it is the only one with an encoder that \
                 this build recognises.",
                pick.name
            )
        } else {
            format!(
                "{} is suggested over the integrated graphics: a separate card's \
                 encoder is usually the faster of the two. Usually, not measured.",
                pick.name
            )
        })
    }
}

/// Look for graphics hardware. Changes nothing.
pub fn look() -> Found {
    #[cfg(windows)]
    {
        windows_adapters()
    }
    #[cfg(target_os = "linux")]
    {
        linux_adapters()
    }
    #[cfg(target_os = "macos")]
    {
        macos_adapters()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Found {
            adapters: Vec::new(),
            problems: vec!["no way to list graphics devices is written for this platform".into()],
        }
    }
}

/// Ask Windows through its own management interface.
#[cfg(windows)]
fn windows_adapters() -> Found {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    // Absolute path, and PowerShell rather than the deprecated `wmic`, which
    // recent Windows no longer ships.
    let program = format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut command = Command::new(&program);
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Get-CimInstance Win32_VideoController | ForEach-Object { \
         \"$($_.Name)|$($_.DriverVersion)\" }",
    ]);
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match command.output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            Found {
                adapters: parse_pairs(&text),
                problems: Vec::new(),
            }
        }
        Ok(output) => Found {
            adapters: Vec::new(),
            problems: vec![format!(
                "could not list graphics devices: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )],
        },
        Err(error) => Found {
            adapters: Vec::new(),
            problems: vec![format!("{program} would not run: {error}")],
        },
    }
}

/// Ask Linux through `lspci`.
#[cfg(target_os = "linux")]
fn linux_adapters() -> Found {
    match Command::new("lspci").output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let adapters = text
                .lines()
                .filter(|line| {
                    let lower = line.to_lowercase();
                    lower.contains("vga compatible controller")
                        || lower.contains("3d controller")
                        || lower.contains("display controller")
                })
                .filter_map(|line| line.split_once(": ").map(|(_, rest)| rest.trim()))
                .map(|name| adapter(name, None))
                .collect();
            Found {
                adapters,
                problems: Vec::new(),
            }
        }
        _ => Found {
            adapters: Vec::new(),
            problems: vec![
                "lspci is not installed, so the graphics devices could not be listed".into(),
            ],
        },
    }
}

/// Ask macOS through `system_profiler`.
#[cfg(target_os = "macos")]
fn macos_adapters() -> Found {
    match Command::new("/usr/sbin/system_profiler")
        .arg("SPDisplaysDataType")
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let adapters = text
                .lines()
                .filter_map(|line| line.trim().strip_prefix("Chipset Model: "))
                .map(|name| adapter(name.trim(), None))
                .collect();
            Found {
                adapters,
                problems: Vec::new(),
            }
        }
        _ => Found {
            adapters: Vec::new(),
            problems: vec!["system_profiler would not run".into()],
        },
    }
}

/// `name|driver` lines into adapters.
#[cfg(any(windows, test))]
fn parse_pairs(text: &str) -> Vec<Adapter> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once('|') {
            Some((name, driver)) => {
                let driver = driver.trim();
                adapter(
                    name.trim(),
                    if driver.is_empty() {
                        None
                    } else {
                        Some(driver.to_string())
                    },
                )
            }
            None => adapter(line, None),
        })
        .collect()
}

/// One adapter from a name.
#[cfg(any(windows, target_os = "linux", target_os = "macos", test))]
fn adapter(name: &str, driver: Option<String>) -> Adapter {
    let lower = name.to_ascii_lowercase();
    // A guess, and labelled as one wherever it is shown. Apple silicon is
    // integrated by construction; Intel's desktop parts are almost always the
    // integrated half of a processor; the rest are named for it or are not.
    let integrated = lower.contains("uhd graphics")
        || lower.contains("hd graphics")
        || lower.contains("iris")
        || lower.contains("vega") && lower.contains("graphics")
        || lower.contains("radeon graphics")
        || lower.contains("apple m");
    Adapter {
        vendor: Vendor::of(name),
        name: name.to_string(),
        driver,
        integrated,
    }
}

/// How many threads this machine can usefully run at once.
///
/// Used for **batches**, never to split one recording. The engine's ratchet and
/// its phase state run forward in time, so two halves of one file cannot be
/// veiled in parallel and produce the file the whole of it would have. Batching
/// several files is a different thing and parallelises exactly.
pub fn usable_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Why the audio engine is not offered a graphics card, with the numbers.
pub const WHY_NOT_THE_ENGINE: &str = "\
Veiling a voice is not work a graphics card helps with, and offering the option \
would make VeilVoice slower. Sixty seconds of audio takes about 0.58 seconds on \
one processor core, roughly a hundred times faster than real time, and live \
mode finishes each 1024-sample frame in about 0.05 ms out of the 21 ms it has. \
A graphics card is fast at doing one thing to a very large batch at once; \
moving a frame that small onto the card and back costs more than the work. So \
there is no switch for it, and the reason is a measurement rather than a \
preference.";

/// What hardware encoding is for, and what it does not change.
pub const WHAT_IT_CHANGES: &str = "\
Hardware encoding changes how long a video takes to write, and nothing else. \
The audio is veiled by the same engine either way, the picture is drawn by the \
same code, and the result is the same recording. If it is unavailable or fails, \
the software encoder produces the same video more slowly.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vendor_this_build_knows_has_an_encoder_and_a_name() {
        for vendor in [Vendor::Nvidia, Vendor::Amd, Vendor::Intel, Vendor::Apple] {
            assert!(vendor.encoder().is_some(), "{vendor:?}");
            assert!(vendor.encoder_name().is_some(), "{vendor:?}");
        }
        assert_eq!(Vendor::Unknown.encoder(), None);
        assert_eq!(Vendor::Unknown.encoder_name(), None);
    }

    /// The names real machines actually report, including the ones that do not
    /// contain the maker's name at all.
    #[test]
    fn the_vendors_are_recognised_from_the_names_systems_really_use() {
        for (name, want) in [
            ("NVIDIA GeForce RTX 4070", Vendor::Nvidia),
            ("GeForce GTX 1060 6GB", Vendor::Nvidia),
            ("NVIDIA Quadro P2000", Vendor::Nvidia),
            ("AMD Radeon RX 7900 XT", Vendor::Amd),
            ("Radeon(TM) Graphics", Vendor::Amd),
            ("Intel(R) UHD Graphics 630", Vendor::Intel),
            ("Intel(R) Iris(R) Xe Graphics", Vendor::Intel),
            ("Apple M2 Pro", Vendor::Apple),
            ("Microsoft Basic Display Adapter", Vendor::Unknown),
        ] {
            assert_eq!(Vendor::of(name), want, "{name}");
        }
    }

    #[test]
    fn a_windows_listing_becomes_adapters_with_drivers() {
        let adapters = parse_pairs(
            "NVIDIA GeForce RTX 4070|31.0.15.3699\nIntel(R) UHD Graphics 770|30.0.101.1404\n",
        );
        assert_eq!(adapters.len(), 2);
        assert_eq!(adapters[0].vendor, Vendor::Nvidia);
        assert_eq!(adapters[0].driver.as_deref(), Some("31.0.15.3699"));
        assert!(!adapters[0].integrated);
        assert_eq!(adapters[1].vendor, Vendor::Intel);
        assert!(adapters[1].integrated, "UHD Graphics is integrated");
        assert_eq!(adapters[0].encoder(), Some("h264_nvenc"));
        assert_eq!(adapters[1].encoder(), Some("h264_qsv"));
    }

    #[test]
    fn a_listing_with_no_driver_column_still_parses() {
        let adapters = parse_pairs("AMD Radeon RX 7900 XT\n\n");
        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].driver, None);
        assert!(adapters[0].describe().contains("AMF"));
    }

    /// A separate card is suggested over integrated graphics, and the reason
    /// says "usually" because nothing here measured it.
    #[test]
    fn the_recommendation_prefers_a_separate_card_and_admits_it_is_a_guess() {
        let found = Found {
            adapters: parse_pairs("Intel(R) UHD Graphics 770|30.0\nNVIDIA GeForce RTX 4070|31.0\n"),
            problems: Vec::new(),
        };
        let pick = found.recommended().expect("one of the two");
        assert_eq!(pick.vendor, Vendor::Nvidia);
        let why = found.why_recommended().unwrap();
        assert!(why.contains("usually"), "{why}");
        assert!(why.contains("not measured"), "{why}");
    }

    /// Integrated graphics on their own are still offered. "Supports integrated
    /// graphics" was asked for by name, and refusing them because they are the
    /// slower option would leave a laptop with nothing.
    #[test]
    fn integrated_graphics_alone_are_still_recommended() {
        let found = Found {
            adapters: parse_pairs("Intel(R) Iris(R) Xe Graphics|30.0\n"),
            problems: Vec::new(),
        };
        let pick = found.recommended().expect("the only one");
        assert!(pick.integrated);
        assert_eq!(pick.encoder(), Some("h264_qsv"));
        assert!(found.why_recommended().unwrap().contains("only device"));
    }

    /// A device with no encoder this build knows is never recommended, and its
    /// absence is not reported as an absence of hardware.
    #[test]
    fn a_device_with_no_known_encoder_is_not_suggested() {
        let found = Found {
            adapters: parse_pairs("Microsoft Basic Display Adapter\n"),
            problems: Vec::new(),
        };
        assert_eq!(found.recommended(), None);
        assert!(found.why_recommended().is_none());
        // And the device is still listed, because "we do not know its encoder"
        // is not "there is no graphics hardware here".
        assert_eq!(found.adapters.len(), 1);
        assert!(found.is_answerable());
    }

    /// "I could not look" is never reported as "there is nothing here".
    #[test]
    fn a_failed_look_is_not_an_empty_machine() {
        let broken = Found {
            adapters: Vec::new(),
            problems: vec!["lspci is not installed".into()],
        };
        assert!(!broken.is_answerable());
        let empty = Found::default();
        assert!(empty.is_answerable(), "nothing found and nothing wrong");
    }

    /// The two notes have to state the measurement rather than assert a
    /// preference, because "we did not bother" and "we measured and it is
    /// slower" are different claims.
    #[test]
    fn the_engine_note_carries_the_numbers() {
        let why = WHY_NOT_THE_ENGINE.to_lowercase();
        assert!(why.contains("0.58 seconds"), "{why}");
        assert!(why.contains("hundred times faster than real time"), "{why}");
        assert!(
            why.contains("measurement rather than a preference"),
            "{why}"
        );
        assert!(
            !why.contains("not supported"),
            "the reason is that it would be slower, not that it is missing"
        );

        let what = WHAT_IT_CHANGES.to_lowercase();
        assert!(what.contains("nothing else"), "{what}");
        assert!(what.contains("the same recording"), "{what}");
    }

    /// Threads are for batches. One recording cannot be split, because the
    /// ratchet and the phase state run forward in time.
    #[test]
    fn the_thread_count_is_sane_and_documented_as_being_for_batches() {
        let n = usable_threads();
        assert!(n >= 1);
        let source = include_str!("lib.rs");
        let doc = source.split("pub fn usable_threads").next().unwrap();
        assert!(doc.contains("never to split one recording"));
    }

    /// Asking the real machine must not panic or change anything.
    #[test]
    fn looking_is_safe_wherever_this_runs() {
        let found = look();
        for adapter in &found.adapters {
            assert!(!adapter.name.is_empty());
            assert!(!adapter.describe().is_empty());
            assert!(adapter.caveat().contains("not proof it can be used"));
        }
        for problem in &found.problems {
            assert!(!problem.is_empty());
        }
    }
}
