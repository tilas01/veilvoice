// SPDX-License-Identifier: GPL-3.0-or-later
//! The live modules, in a build that has no live capture.
//!
//! **Roadmap item 162.** `cpal` has no backend for FreeBSD, OpenBSD or NetBSD,
//! so the `live` feature is off there. The command line has always coped by
//! leaving its live commands out. The window cannot do that as neatly: the
//! Studio and the Browser are written against sessions, recorders and players,
//! and leaving each use out would mean a `cfg` on every line of the tab that
//! uses them, which is where a build that only one platform exercises goes to
//! rot.
//!
//! So the same names exist here, with the same signatures, and **none of them
//! can do anything**. Every constructor refuses with [`REASON`], which a front
//! end shows where the button was pressed. Every type that stands for a
//! running thing (a session, a room, a recorder, a player, an opened device)
//! holds a [`std::convert::Infallible`], so no value of it can ever exist and
//! its methods are proofs that they are never called rather than code that
//! pretends to measure something.
//!
//! The data the live path reports is not here: it is in [`crate::kinds`] and
//! is the same type in both builds.
//!
//! # What this is not
//!
//! It is not a fake microphone. Nothing here produces a sample, a level or a
//! device name, because a meter that moved on a platform that cannot hear
//! anything would be a lie about the one thing this program is for.
//!
//! # In plain words
//!
//! On a system where VeilVoice cannot use a microphone, the parts that would
//! use one are still there, and every one of them says that this system cannot
//! capture sound when it is asked to start.

use std::convert::Infallible;

use crate::Error;

/// Why nothing live can start in this build, in the words a person reads.
pub const REASON: &str = "This build of VeilVoice has no live audio: cpal, the library \
     that talks to sound devices, has no backend for this operating system. Veiling a \
     file, the vault, encryption and cleaning metadata all work; recording, playing \
     back and the Monitor's microphone do not.";

/// The one answer every constructor here gives.
fn refused<T>() -> Result<T, Error> {
    Err(Error::NoLiveAudio(REASON))
}

/// Devices, of which this build can see none.
pub mod devices {
    pub use crate::kinds::{DeviceInfo, Direction};

    use super::{refused, Infallible};
    use crate::Error;

    /// An opened device. None can exist in this build.
    pub struct Device(Infallible);

    /// Refuses: there are no devices to list.
    pub fn list(_direction: Direction) -> Result<Vec<DeviceInfo>, Error> {
        refused()
    }

    /// Always `None`: there are no devices to look through.
    pub fn find_virtual_cable() -> Option<DeviceInfo> {
        None
    }

    /// Never called: no [`Device`] exists.
    pub fn name_of(device: &Device) -> String {
        match device.0 {}
    }

    /// Refuses: there is nothing to open.
    pub fn open(_direction: Direction, _name: Option<&str>) -> Result<Device, Error> {
        refused()
    }
}

/// Recorders, which only a live session can create.
pub mod record {
    use super::Infallible;
    use crate::Error;
    use veilvoice_crypto::Secret;

    /// Seconds of slack a recorder holds, the same figure as the live build.
    pub const SLACK_SECONDS: f32 = 8.0;

    /// The realtime side of a recorder. None can exist in this build.
    pub struct Sink(Infallible);

    impl Sink {
        /// Never called.
        pub fn write(&mut self, _samples: &[f32]) {
            match self.0 {}
        }
    }

    /// A recording in progress. None can exist in this build.
    pub struct Recorder(Infallible);

    impl Recorder {
        /// Never called.
        pub fn drain(&mut self) -> usize {
            match self.0 {}
        }
        /// Never called.
        pub fn samples(&self) -> usize {
            match self.0 {}
        }
        /// Never called.
        pub fn seconds(&self) -> f32 {
            match self.0 {}
        }
        /// Never called.
        pub fn dropped(&self) -> u64 {
            match self.0 {}
        }
        /// Never called.
        pub fn fully_locked(&self) -> bool {
            match self.0 {}
        }
        /// Never called.
        pub fn sample_rate(&self) -> u32 {
            match self.0 {}
        }
        /// Never called.
        pub fn wav(&mut self) -> Result<Secret, Error> {
            match self.0 {}
        }
        /// Never called.
        pub fn discard(&mut self) {
            match self.0 {}
        }
    }
}

/// Playback, which needs an output device this build cannot open.
pub mod playback {
    use super::{refused, Infallible};
    use crate::Error;

    /// Something playing. Nothing can be, in this build.
    pub struct Playing(Infallible);

    impl Playing {
        /// Never called.
        pub fn position(&self) -> f32 {
            match self.0 {}
        }
        /// Never called.
        pub fn duration(&self) -> f32 {
            match self.0 {}
        }
        /// Never called.
        pub fn finished(&self) -> bool {
            match self.0 {}
        }
        /// Never called.
        pub fn peak(&self) -> f32 {
            match self.0 {}
        }
    }

    /// Refuses: there is nothing to play through.
    pub fn start(_samples: Vec<f32>, _rate: u32, _device: Option<&str>) -> Result<Playing, Error> {
        refused()
    }
}

/// The single-microphone session, which cannot start in this build.
pub mod live {
    pub use crate::kinds::{Interference, Keeping, LiveStats, Side};

    use super::devices::Device;
    use super::{refused, Infallible};
    use crate::Error;
    use veilvoice_core::DeidConfig;

    /// The recorders a session was asked for. Always empty in this build.
    #[derive(Default)]
    pub struct Kept {
        /// The veiled voice. Never present.
        pub veiled: Option<crate::record::Recorder>,
        /// The microphone. Never present.
        pub plain: Option<crate::record::Recorder>,
    }

    /// A running session. None can exist in this build.
    pub struct LiveSession(Infallible);

    impl LiveSession {
        /// Refuses: nothing can be captured.
        pub fn start(
            _input: &Device,
            _output: &Device,
            _config: DeidConfig,
        ) -> Result<Self, Error> {
            refused()
        }

        /// Refuses: nothing can be captured.
        pub fn start_recording(
            _input: &Device,
            _output: &Device,
            _config: DeidConfig,
            _keeping: Keeping,
        ) -> Result<(Self, Kept), Error> {
            refused()
        }

        /// Never called.
        pub fn stats(&self) -> LiveStats {
            match self.0 {}
        }

        /// Never called.
        pub fn interference(&self) -> Option<Interference> {
            match self.0 {}
        }
    }
}

/// Several microphones at once, which cannot start in this build.
pub mod room {
    pub use crate::kinds::{GuestStats, RoomStats, MAX_GUESTS};

    use super::devices::Device;
    use super::{refused, Infallible};
    use crate::live::{Keeping, Kept};
    use crate::Error;
    use veilvoice_core::DeidConfig;

    /// One guest, described the same way as in the live build.
    pub struct Guest<'a> {
        /// Their microphone.
        pub device: &'a Device,
        /// Their engine's settings.
        pub config: DeidConfig,
        /// What to keep of them.
        pub keeping: Keeping,
    }

    /// The recorders a room was asked for. Always empty in this build.
    #[derive(Default)]
    pub struct KeptRoom {
        /// One per guest. Never filled.
        pub guests: Vec<Kept>,
        /// The mix. Never present.
        pub mixed: Option<crate::record::Recorder>,
    }

    /// A running room. None can exist in this build.
    pub struct RoomSession(Infallible);

    impl RoomSession {
        /// Refuses: nothing can be captured.
        pub fn start(
            _guests: &[Guest<'_>],
            _output: &Device,
            _mixed: bool,
        ) -> Result<(Self, KeptRoom), Error> {
            refused()
        }

        /// Never called.
        pub fn guests(&self) -> usize {
            match self.0 {}
        }

        /// Never called.
        pub fn stats(&self) -> RoomStats {
            match self.0 {}
        }

        /// Never called.
        pub fn interference(&self) -> Option<crate::live::Interference> {
            match self.0 {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every way into the live path refuses, and says why in the same words.
    #[test]
    fn everything_that_would_start_says_this_build_cannot() {
        let said = |error: Error| {
            let text = error.to_string();
            assert!(text.contains("no backend"), "{text}");
            assert!(
                text.contains("Veiling a file"),
                "the refusal says what still works: {text}"
            );
        };
        said(devices::list(devices::Direction::Input).unwrap_err());
        said(
            devices::open(devices::Direction::Output, None)
                .err()
                .unwrap(),
        );
        said(playback::start(vec![0.0; 4], 48_000, None).err().unwrap());
        assert!(devices::find_virtual_cable().is_none());
        const { assert!(!crate::CAN_CAPTURE) };
        assert_eq!(
            crate::WHY_NO_CAPTURE,
            Some(REASON),
            "the notice shown up front and the refusal say different things"
        );
    }
}
