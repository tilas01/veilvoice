// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-watch
//!
//! Find out which applications are using your microphone and camera, right now.
//!
//! ## Why this belongs in a voice-privacy tool
//!
//! VeilVoice protects the audio you choose to send. This answers a different
//! and more basic question: *is something listening that you did not choose?*
//! A de-identified voice on a call is worth very little if a second program is
//! recording the raw microphone at the same time.
//!
//! Operating systems have grown indicators for this, the orange dot and the
//! taskbar icon, but they are small, easily missed, and tell you only that
//! *something* is active, rarely what. This reports the process, its PID and
//! how long it has held the device.
//!
//! ## What it can actually see, per platform
//!
//! Detection is honest about its limits, because a monitor that quietly sees
//! nothing is worse than no monitor at all, because it produces false confidence.
//! [`support`] reports what the current platform can do before you rely on it.
//!
//! | Platform | Microphone | Camera | How |
//! |---|---|---|---|
//! | Windows | ✅ | ✅ | The same `CapabilityAccessManager` records the OS privacy indicator uses |
//! | Linux | ✅ | ✅ | `/proc/*/fd` handles open on `/dev/snd/pcm*` and `/dev/video*` |
//! | macOS | ❌ | ❌ | No public API exposes it; anything claiming otherwise on macOS is guessing |
//!
//! On Linux you see every process you have permission to inspect. Without root
//! that means your own; other users' processes are invisible, and that is a
//! kernel permission boundary rather than something this crate can work around.
//!
//! # In plain words
//!
//! This tells you when something is using your microphone or camera.
//!
//! Not what it is doing with them -- just that a program has them open, and which
//! program. That is worth knowing before you start talking, and it is the kind of
//! thing an operating system knows and does not always show you.
//!
//! It cannot see everything. Some ways of getting at a microphone do not go past
//! the place this reads.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;
use std::time::SystemTime;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The kind of device being used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeviceKind {
    /// A microphone or other audio capture device.
    Microphone,
    /// A camera or other video capture device.
    Camera,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Microphone => "microphone",
            Self::Camera => "camera",
        })
    }
}

/// One application holding one device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceUse {
    /// Which kind of device.
    pub kind: DeviceKind,
    /// A readable name for the application.
    pub app: String,
    /// Full path to the executable, where it could be determined.
    pub path: Option<String>,
    /// Process ID, where the platform exposes one.
    ///
    /// Windows reports usage per *application* rather than per process, so this
    /// is often `None` there even though the app is definitely active.
    pub pid: Option<u32>,
    /// When the application started using the device, if known.
    pub since: Option<SystemTime>,
    /// The specific device node or endpoint, where known.
    pub device: Option<String>,
}

impl DeviceUse {
    /// A stable key for comparing two scans, so an app is not reported as
    /// having stopped and restarted when nothing changed.
    pub fn key(&self) -> (DeviceKind, String, Option<u32>) {
        (self.kind, self.app.to_lowercase(), self.pid)
    }

    /// How long this application has held the device.
    pub fn held_for(&self) -> Option<std::time::Duration> {
        self.since
            .and_then(|t| SystemTime::now().duration_since(t).ok())
    }
}

/// What detection is possible here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Support {
    /// Whether microphone use can be detected.
    pub microphone: bool,
    /// Whether camera use can be detected.
    pub camera: bool,
    /// How it works, or why it does not, in one sentence for the user.
    pub explanation: &'static str,
}

/// Report what this platform can detect.
///
/// Check this before showing a monitor. Presenting an empty list as "nothing is
/// listening" on a platform that cannot tell is a false assurance, and this is
/// exactly the kind of tool where that matters.
pub fn support() -> Support {
    #[cfg(target_os = "windows")]
    {
        Support {
            microphone: true,
            camera: true,
            explanation: "Reads the same CapabilityAccessManager records that drive \
                          the Windows privacy indicator.",
        }
    }
    #[cfg(target_os = "linux")]
    {
        Support {
            microphone: true,
            camera: true,
            explanation: "Inspects open file handles under /proc. Without root this \
                          sees your own processes only.",
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Support {
            microphone: false,
            camera: false,
            explanation: "This platform exposes no public interface for which \
                          application is using the microphone or camera, so nothing \
                          is reported rather than something guessed.",
        }
    }
}

/// Everything that can go wrong here.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The platform provides no way to answer the question.
    Unsupported,
    /// The system refused access to the information.
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str(support().explanation),
            Self::Io(e) => write!(f, "could not read system state: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Take one snapshot of what is currently using the microphone and camera.
///
/// Returns an empty list when nothing is active, which is only meaningful if
/// [`support`] says this platform can tell.
pub fn scan() -> Result<Vec<DeviceUse>, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::scan()
    }
    #[cfg(target_os = "linux")]
    {
        linux::scan()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(Error::Unsupported)
    }
}

/// A change between two scans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// An application began using a device.
    Started(DeviceUse),
    /// An application stopped using a device.
    Stopped(DeviceUse),
}

impl Change {
    /// A one-line alert suitable for a notification or an overlay.
    pub fn alert(&self) -> String {
        match self {
            Self::Started(u) => format!("{} is now using your {}", u.describe(), u.kind),
            Self::Stopped(u) => format!("{} stopped using your {}", u.describe(), u.kind),
        }
    }
}

impl DeviceUse {
    /// `name (pid 1234)`, or just the name when there is no PID.
    pub fn describe(&self) -> String {
        match self.pid {
            Some(pid) => format!("{} (PID {pid})", self.app),
            None => self.app.clone(),
        }
    }
}

/// Watches for changes between scans.
///
/// Holds the previous snapshot and reports what appeared or disappeared, so a
/// caller can raise an alert on transitions rather than repeating a list.
#[derive(Debug, Default)]
pub struct Monitor {
    previous: Vec<DeviceUse>,
}

impl Monitor {
    /// A monitor that has not yet seen anything.
    pub fn new() -> Self {
        Self::default()
    }

    /// The most recent snapshot.
    pub fn current(&self) -> &[DeviceUse] {
        &self.previous
    }

    /// Scan, and report what changed since the previous call.
    ///
    /// The first call reports everything already active as `Started`. That is
    /// deliberate: something that was already recording when the monitor opened
    /// is precisely what the user needs to be told about.
    pub fn poll(&mut self) -> Result<Vec<Change>, Error> {
        self.diff(scan()?)
    }

    /// The comparison, split out so it can be tested without a real system.
    fn diff(&mut self, next: Vec<DeviceUse>) -> Result<Vec<Change>, Error> {
        let mut changes = Vec::new();

        for entry in &next {
            if !self.previous.iter().any(|old| old.key() == entry.key()) {
                changes.push(Change::Started(entry.clone()));
            }
        }
        for entry in &self.previous {
            if !next.iter().any(|new| new.key() == entry.key()) {
                changes.push(Change::Stopped(entry.clone()));
            }
        }

        self.previous = next;
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_of(kind: DeviceKind, app: &str, pid: Option<u32>) -> DeviceUse {
        DeviceUse {
            kind,
            app: app.to_string(),
            path: None,
            pid,
            since: None,
            device: None,
        }
    }

    #[test]
    fn a_first_poll_reports_everything_already_active() {
        // Something already recording when the monitor opens is the single most
        // important thing to surface, not something to treat as the baseline.
        let mut monitor = Monitor::new();
        let changes = monitor
            .diff(vec![use_of(DeviceKind::Microphone, "zoom", Some(42))])
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0], Change::Started(_)));
    }

    #[test]
    fn an_unchanged_list_reports_nothing() {
        let mut monitor = Monitor::new();
        let snapshot = vec![use_of(DeviceKind::Microphone, "zoom", Some(42))];
        monitor.diff(snapshot.clone()).unwrap();
        assert!(
            monitor.diff(snapshot).unwrap().is_empty(),
            "no change, no alert"
        );
    }

    #[test]
    fn starting_and_stopping_are_both_reported() {
        let mut monitor = Monitor::new();
        monitor
            .diff(vec![use_of(DeviceKind::Microphone, "zoom", Some(42))])
            .unwrap();

        let changes = monitor
            .diff(vec![use_of(DeviceKind::Camera, "obs", Some(7))])
            .unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes
            .iter()
            .any(|c| matches!(c, Change::Started(u) if u.app == "obs")));
        assert!(changes
            .iter()
            .any(|c| matches!(c, Change::Stopped(u) if u.app == "zoom")));
    }

    /// The same application on the microphone and on the camera is two
    /// separate facts, and losing one of them would hide a camera going live.
    #[test]
    fn the_same_app_on_two_devices_is_tracked_separately() {
        let mut monitor = Monitor::new();
        monitor
            .diff(vec![use_of(DeviceKind::Microphone, "zoom", Some(42))])
            .unwrap();
        let changes = monitor
            .diff(vec![
                use_of(DeviceKind::Microphone, "zoom", Some(42)),
                use_of(DeviceKind::Camera, "zoom", Some(42)),
            ])
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Started(u) if u.kind == DeviceKind::Camera));
    }

    #[test]
    fn the_same_app_under_two_pids_is_tracked_separately() {
        let mut monitor = Monitor::new();
        monitor
            .diff(vec![use_of(DeviceKind::Microphone, "chrome", Some(1))])
            .unwrap();
        let changes = monitor
            .diff(vec![
                use_of(DeviceKind::Microphone, "chrome", Some(1)),
                use_of(DeviceKind::Microphone, "chrome", Some(2)),
            ])
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Started(u) if u.pid == Some(2)));
    }

    #[test]
    fn alerts_name_the_app_the_device_and_the_pid() {
        let started = Change::Started(use_of(DeviceKind::Microphone, "zoom", Some(42)));
        let text = started.alert();
        assert!(text.contains("zoom"));
        assert!(text.contains("microphone"));
        assert!(text.contains("42"));

        let stopped = Change::Stopped(use_of(DeviceKind::Camera, "obs", None));
        assert!(stopped.alert().contains("stopped"));
        assert!(stopped.alert().contains("camera"));
    }

    #[test]
    fn support_is_reported_honestly_for_this_platform() {
        let s = support();
        assert!(!s.explanation.is_empty());
        if cfg!(any(target_os = "windows", target_os = "linux")) {
            assert!(s.microphone && s.camera);
        } else {
            assert!(
                !s.microphone && !s.camera,
                "a platform that cannot detect must not claim it can"
            );
        }
    }

    /// A real scan must never panic, whatever the machine looks like.
    #[test]
    fn scanning_this_machine_does_not_panic() {
        match scan() {
            Ok(list) => {
                for entry in list {
                    assert!(!entry.app.is_empty(), "an entry with no name is useless");
                }
            }
            Err(Error::Unsupported) => assert!(!support().microphone),
            Err(Error::Io(_)) => {}
        }
    }
}
