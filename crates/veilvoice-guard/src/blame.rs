// SPDX-License-Identifier: GPL-3.0-or-later
//! Best-effort attribution: which program changed a file.
//!
//! # Read this before relying on it
//!
//! **Most of the time this cannot answer, and says so.** Neither Windows nor
//! Linux records who wrote to a file unless auditing has been switched on for
//! that path in advance, and on a normal desktop it has not been. An
//! unprivileged program cannot switch it on either.
//!
//! So the honest design is a function that returns [`Blame::Unknown`] with a
//! reason, and tells the user what would have to be configured for a real
//! answer. A plausible guess would be worse than nothing here: naming the wrong
//! program to somebody worried about surveillance is not a small error.
//!
//! # Where an answer can come from
//!
//! - **Linux** -- the kernel audit subsystem. With a watch in place
//!   (`auditctl -w <path> -p wa -k veilvoice`), `ausearch` reports the syscall,
//!   the pid and the executable name for every write. Reading those records
//!   usually needs root as well.
//! - **Windows** -- object access auditing. With a SACL on the file and the
//!   "Audit File System" policy enabled, Security event 4663 records the
//!   process that opened it for write. `wevtutil` can query that log, though
//!   reading the Security log needs elevation.
//!
//! Both are checked for, neither is configured by this crate, and the absence
//! of either is reported rather than hidden. Setting them up is a deliberate
//! act by an administrator, and pretending otherwise would misrepresent what a
//! clean report means.

use std::path::Path;

/// What, if anything, is known about who changed a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blame {
    /// A named process, from an audit record.
    Process {
        /// Executable name or path, as the audit record gives it.
        name: String,
        /// Process id, where the record carries one.
        pid: Option<u32>,
        /// Which facility produced this -- so the user can go and read it.
        source: &'static str,
    },
    /// Nothing is known, and this is why.
    Unknown {
        /// Why no answer is available, in plain words.
        why: String,
        /// What an administrator would have to do to get one.
        remedy: &'static str,
    },
}

impl Blame {
    /// A single line for a terminal, an alert or a log.
    pub fn describe(&self) -> String {
        match self {
            Blame::Process { name, pid, source } => match pid {
                Some(pid) => format!("{name} (pid {pid}, from {source})"),
                None => format!("{name} (from {source})"),
            },
            Blame::Unknown { why, .. } => format!("unknown - {why}"),
        }
    }

    /// Whether an actual name was found.
    pub fn is_known(&self) -> bool {
        matches!(self, Blame::Process { .. })
    }
}

/// Build the "nobody configured auditing" answer for this platform.
fn unconfigured(why: impl Into<String>) -> Blame {
    Blame::Unknown {
        why: why.into(),
        remedy: if cfg!(target_os = "linux") {
            "as root: auditctl -w <path> -p wa -k veilvoice, then read it with ausearch -k veilvoice"
        } else if cfg!(windows) {
            "enable 'Audit File System' in Local Security Policy and put a SACL on the file, \
             then read Security event 4663"
        } else {
            "this platform exposes no file-write auditing VeilVoice can read"
        },
    }
}

/// Try to name the program that last wrote to `path`.
///
/// Returns [`Blame::Unknown`] far more often than not. That is the correct
/// answer on an unaudited system, and is reported as such rather than guessed
/// at.
pub fn who_touched(path: &Path) -> Blame {
    #[cfg(target_os = "linux")]
    {
        linux::who_touched(path)
    }
    #[cfg(windows)]
    {
        windows::who_touched(path)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = path;
        unconfigured("this platform records no per-file write auditing VeilVoice can read")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{unconfigured, Blame};
    use std::path::Path;
    use std::process::Command;

    /// Ask `ausearch` what touched the path, if the audit tools are present at
    /// all and this process is allowed to read the records.
    pub fn who_touched(path: &Path) -> Blame {
        let Ok(output) = Command::new("ausearch")
            .args(["-f", &path.to_string_lossy(), "-i", "-m", "PATH"])
            .output()
        else {
            return unconfigured("the audit tools (ausearch) are not installed");
        };
        if !output.status.success() {
            return unconfigured(
                "no audit records for this path - either nothing is watching it or reading \
                 the log needs root",
            );
        }

        let text = String::from_utf8_lossy(&output.stdout);
        // `ausearch -i` interprets the record, so the fields read as
        // `exe="/usr/bin/thing"` and `pid=1234`. The most recent record last.
        let mut name = None;
        let mut pid = None;
        for line in text.lines() {
            if let Some(value) = field(line, "exe=") {
                name = Some(value);
            }
            if let Some(value) = field(line, "pid=") {
                pid = value.parse().ok();
            }
        }
        match name {
            Some(name) => Blame::Process {
                name,
                pid,
                source: "the Linux audit subsystem",
            },
            None => unconfigured("the audit records name no executable for this path"),
        }
    }

    /// Pull `key="value"` or `key=value` out of an interpreted audit line.
    fn field(line: &str, key: &str) -> Option<String> {
        let start = line.find(key)? + key.len();
        let rest = &line[start..];
        let value = if let Some(stripped) = rest.strip_prefix('"') {
            stripped.split('"').next()?
        } else {
            rest.split_whitespace().next()?
        };
        (!value.is_empty()).then(|| value.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn fields_are_pulled_out_of_an_interpreted_record() {
            let line = r#"type=PATH msg=audit(1): pid=4321 exe="/usr/bin/vim" key="veilvoice""#;
            assert_eq!(field(line, "pid="), Some("4321".to_string()));
            assert_eq!(field(line, "exe="), Some("/usr/bin/vim".to_string()));
            assert_eq!(field(line, "nothing="), None);
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{unconfigured, Blame};
    use std::path::Path;
    use std::process::Command;

    /// Query the Security log for object-access events naming this path.
    ///
    /// Event 4663 is "an attempt was made to access an object", and carries the
    /// process name. It is only written if a SACL is on the object *and* the
    /// audit policy is on, which is off by default; and reading the Security
    /// log needs elevation. All three are ordinary reasons to get nothing.
    pub fn who_touched(path: &Path) -> Blame {
        let query = format!(
            "*[System[(EventID=4663)]] and *[EventData[Data[@Name='ObjectName']='{}']]",
            path.to_string_lossy().replace('\'', "''")
        );
        let Ok(output) = Command::new("wevtutil")
            .args(["qe", "Security", "/f:text", "/c:1", "/rd:true"])
            .arg(format!("/q:{query}"))
            .output()
        else {
            return unconfigured("wevtutil is not available");
        };
        if !output.status.success() {
            return unconfigured(
                "the Security log could not be read - object-access auditing is off, or this \
                 needs to run elevated",
            );
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let name = text.lines().find_map(|line| {
            let line = line.trim();
            line.strip_prefix("Process Name:")
                .map(|rest| rest.trim().to_string())
                .filter(|value| !value.is_empty())
        });
        let pid = text.lines().find_map(|line| {
            let line = line.trim();
            line.strip_prefix("Process ID:")
                .and_then(|rest| u32::from_str_radix(rest.trim().trim_start_matches("0x"), 16).ok())
        });

        match name {
            Some(name) => Blame::Process {
                name,
                pid,
                source: "the Windows Security event log (4663)",
            },
            None => unconfigured("no object-access record names a process for this path"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On an ordinary machine there is no auditing, so this must return a
    /// reason rather than a guess -- and must never panic doing it.
    #[test]
    fn an_unaudited_file_yields_an_honest_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("untouched.bin");
        std::fs::write(&path, b"x").unwrap();

        match who_touched(&path) {
            Blame::Unknown { why, remedy } => {
                assert!(!why.is_empty());
                assert!(!remedy.is_empty(), "a remedy must always be offered");
            }
            // If a machine really does have auditing configured, that is a
            // legitimate answer too.
            Blame::Process { name, .. } => assert!(!name.is_empty()),
        }
    }

    #[test]
    fn a_missing_file_does_not_panic() {
        let _ = who_touched(Path::new("no-such-file-anywhere-at-all.bin"));
    }

    #[test]
    fn descriptions_read_sensibly() {
        let known = Blame::Process {
            name: "vim".into(),
            pid: Some(42),
            source: "test",
        };
        assert_eq!(known.describe(), "vim (pid 42, from test)");
        assert!(known.is_known());

        let unknown = Blame::Unknown {
            why: "nothing is watching".into(),
            remedy: "turn auditing on",
        };
        assert!(unknown.describe().starts_with("unknown - "));
        assert!(!unknown.is_known());
    }
}
