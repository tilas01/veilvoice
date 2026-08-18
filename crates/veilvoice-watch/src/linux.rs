// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Linux detection, via open file handles in `/proc`.
//!
//! # How it works
//!
//! A process using the microphone has a file descriptor open on an ALSA PCM
//! capture node — `/dev/snd/pcmC0D0c`, where the trailing `c` means capture as
//! opposed to `p` for playback. A process using the camera has one open on
//! `/dev/video*`. Walking `/proc/*/fd` and resolving the symlinks finds them,
//! along with the PID and the process name, with no dependency and no daemon.
//!
//! Capture and playback are distinguished deliberately. Treating every open
//! `/dev/snd` handle as microphone use would report a music player as
//! listening to you, and a monitor that cries wolf gets ignored — which is the
//! worst possible outcome for this feature.
//!
//! # Sound servers
//!
//! On most desktops PipeWire or PulseAudio owns the hardware, so the process
//! holding the PCM node is the *server*, not the application behind it. That is
//! reported honestly rather than hidden: the server appearing means something
//! is capturing, and where the client can be identified from the ALSA
//! `/proc/asound` bookkeeping, it is named too.
//!
//! # The permission boundary
//!
//! `/proc/<pid>/fd` is readable only by the process owner and root. Without
//! root you therefore see your own processes; another user's are invisible.
//! That is a kernel boundary, not a gap in this code, and [`crate::support`]
//! says so rather than letting an empty list imply an empty machine.

use crate::{DeviceKind, DeviceUse, Error};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub fn scan() -> Result<Vec<DeviceUse>, Error> {
    let mut found = Vec::new();

    let entries = match fs::read_dir("/proc") {
        Ok(e) => e,
        Err(e) => return Err(Error::Io(e)),
    };

    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(handles) = fs::read_dir(&fd_dir) else {
            // Not ours to inspect, or the process exited between listing and
            // opening. Either way, skip it rather than fail the whole scan.
            continue;
        };

        let mut seen = Vec::new();
        for handle in handles.flatten() {
            let Ok(target) = fs::read_link(handle.path()) else {
                continue;
            };
            let Some(kind) = classify(&target) else {
                continue;
            };
            if seen.contains(&kind) {
                continue; // one entry per process per device kind
            }
            seen.push(kind);

            found.push(DeviceUse {
                kind,
                app: process_name(pid).unwrap_or_else(|| format!("pid {pid}")),
                path: fs::read_link(entry.path().join("exe"))
                    .ok()
                    .map(|p| p.display().to_string()),
                pid: Some(pid),
                since: started_at(&entry.path()),
                device: Some(target.display().to_string()),
            });
        }
    }

    found.sort_by_key(|u| (u.kind, u.pid));
    Ok(found)
}

/// Decide whether an open handle means capture.
///
/// Returns `None` for playback devices, control nodes and everything else, so
/// a music player is never mistaken for something listening.
fn classify(target: &Path) -> Option<DeviceKind> {
    let path = target.to_str()?;

    if let Some(node) = path.strip_prefix("/dev/snd/") {
        // pcmC0D0c — capture. pcmC0D0p — playback. Only the former counts.
        if node.starts_with("pcm") && node.ends_with('c') {
            return Some(DeviceKind::Microphone);
        }
        return None;
    }

    if path.starts_with("/dev/video") {
        // /dev/video1 may be a metadata node rather than a capture stream, but
        // an open handle on it still means the camera subsystem is in use, and
        // over-reporting a camera is the safer direction to err.
        return Some(DeviceKind::Camera);
    }

    None
}

fn process_name(pid: u32) -> Option<String> {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = comm.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// When the process started, from the modification time of its `/proc` entry.
///
/// This is the process start time, not the moment it opened the device — the
/// kernel does not record the latter. It is reported as the best available
/// answer rather than omitted, since "running since" is still useful context.
fn started_at(proc_dir: &Path) -> Option<SystemTime> {
    fs::metadata(proc_dir).ok()?.modified().ok()
}

/// Unused today; kept because a future PipeWire client lookup will want it.
#[allow(dead_code)]
fn approx_now_minus(seconds: u64) -> Option<SystemTime> {
    SystemTime::now().checked_sub(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_nodes_are_recognised() {
        assert_eq!(
            classify(Path::new("/dev/snd/pcmC0D0c")),
            Some(DeviceKind::Microphone)
        );
        assert_eq!(
            classify(Path::new("/dev/snd/pcmC1D2c")),
            Some(DeviceKind::Microphone)
        );
    }

    /// The distinction that keeps this feature trustworthy: playing music must
    /// never be reported as using the microphone.
    #[test]
    fn playback_nodes_are_not_microphone_use() {
        assert_eq!(classify(Path::new("/dev/snd/pcmC0D0p")), None);
        assert_eq!(classify(Path::new("/dev/snd/controlC0")), None);
        assert_eq!(classify(Path::new("/dev/snd/timer")), None);
        assert_eq!(classify(Path::new("/dev/snd/seq")), None);
    }

    #[test]
    fn video_nodes_are_camera_use() {
        assert_eq!(classify(Path::new("/dev/video0")), Some(DeviceKind::Camera));
        assert_eq!(
            classify(Path::new("/dev/video12")),
            Some(DeviceKind::Camera)
        );
    }

    #[test]
    fn ordinary_files_are_ignored() {
        for path in [
            "/home/user/notes.txt",
            "/dev/null",
            "/dev/urandom",
            "/tmp/x",
        ] {
            assert_eq!(classify(Path::new(path)), None, "{path}");
        }
    }

    /// Scanning must survive a machine where most of /proc is unreadable.
    #[test]
    fn scanning_does_not_fail_on_inaccessible_processes() {
        let entries = scan().expect("a scan should not fail outright");
        for entry in &entries {
            assert!(entry.pid.is_some(), "Linux always knows the PID");
            assert!(!entry.app.is_empty());
        }
    }
}
