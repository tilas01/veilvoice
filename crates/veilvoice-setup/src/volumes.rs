// SPDX-License-Identifier: GPL-3.0-or-later
//! Encrypted volumes this machine already has: Cryptomator and VeraCrypt.
//!
//! # What this does, and the two things it deliberately does not
//!
//! It **reads**. It finds whether either program is installed, and which of
//! their volumes are mounted right now, so VeilVoice can offer to write veiled
//! recordings into one instead of into a Downloads folder somebody meant to
//! clear out.
//!
//! It does **not drive either program**. No launching, no mounting, no
//! unlocking, and it never sees a volume passphrase. Mounting somebody's
//! encrypted volume is their act, taken in the tool they chose, and a voice
//! de-identifier is not the program to be doing it for them. This is marker
//! 39's rule about privilege in a second place: use what is already there, ask
//! for nothing.
//!
//! It does **not decide whether a volume is hidden**. VeraCrypt's hidden
//! volumes exist so that somebody under compulsion can hand over one passphrase
//! and reveal an outer volume, and the two are indistinguishable from outside
//! *by design*. No amount of looking will tell them apart, so [`Hidden`] has an
//! `Unknown` state and it is the caller's job to ask rather than to guess. See
//! [`Hidden`] for why guessing wrong destroys data.
//!
//! # By the time VeilVoice sees one, it is a directory
//!
//! That is what makes this honest and small. A mounted Cryptomator vault and a
//! mounted VeraCrypt volume are both ordinary directories to anything that
//! writes a file. The encryption is entirely the other tool's, VeilVoice adds
//! none of its own here, and calling any of this "VeilVoice encryption" would
//! be the overclaim this project refuses.
//!
//! # What it is worth, which is less than it sounds
//!
//! A vault protects the file inside it. It does not protect the temporary file
//! an operating system wrote while the file was being produced, the swap or
//! hibernation image the kernel wrote, the thumbnail a file manager made, or
//! the recently-opened list a desktop keeps. Full-volume encryption is what
//! covers those. [`DISK_ADVICE`] is that sentence, single-sourced so the
//! command line and the window cannot drift into two different promises.
//!
//! # In plain words
//!
//! Notices whether you already have Cryptomator or VeraCrypt, and which of
//! their encrypted folders are open right now, so VeilVoice can offer to save
//! into one.
//!
//! It never opens or closes them for you and never asks for their password.
//! It also cannot tell whether a VeraCrypt volume is the hidden one, because
//! nothing can, which is why VeilVoice asks you instead of assuming.

use crate::companions::Presence;
use std::path::{Path, PathBuf};

/// What VeilVoice tells the user about the disk under the volume.
///
/// Single-sourced, and asserted by a test, so the command line and the window
/// cannot end up making two different promises about the same thing.
pub const DISK_ADVICE: &str =
    "An encrypted volume protects the files inside it. It does not protect \
     the temporary files, swap or hibernation image, thumbnails or \
     recently-opened lists your system writes about them. Encrypt the whole \
     disk as well: BitLocker on Windows, FileVault on macOS, LUKS or LUKS2 on \
     Linux, softraid on OpenBSD, GELI on FreeBSD.";

/// One of the two tools this module knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Cryptomator: per-file encryption in a vault directory, FOSS.
    Cryptomator,
    /// VeraCrypt: container and whole-partition encryption, FOSS.
    VeraCrypt,
}

impl Tool {
    /// Every tool, in the order a user interface should offer them.
    pub const ALL: &'static [Tool] = &[Tool::Cryptomator, Tool::VeraCrypt];

    /// The name to print.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Cryptomator => "Cryptomator",
            Tool::VeraCrypt => "VeraCrypt",
        }
    }

    /// A stable identifier, for a settings file.
    pub fn key(self) -> &'static str {
        match self {
            Tool::Cryptomator => "cryptomator",
            Tool::VeraCrypt => "veracrypt",
        }
    }

    /// The tool with this key, if it is one.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.key() == key)
    }

    /// Where to read about it, for a user who has neither installed.
    pub fn home_page(self) -> &'static str {
        match self {
            Tool::Cryptomator => "https://cryptomator.org/",
            Tool::VeraCrypt => "https://veracrypt.io/",
        }
    }
}

/// Whether a destination is, or might be, a VeraCrypt hidden volume.
///
/// # Why this cannot be detected, and what happens if it is guessed
///
/// A VeraCrypt container can hold a second, hidden volume inside the free
/// space of the first. Two passphrases open the same file and produce two
/// different filesystems, and that is the point: somebody compelled to open it
/// can reveal the outer one truthfully and the existence of the inner one is
/// not provable from outside. Nothing VeilVoice can read distinguishes them,
/// and nothing ever will, because a construction that could be distinguished
/// would not be doing its job.
///
/// The danger is specific rather than theoretical. **Writing into the outer
/// volume of a container that has a hidden one can destroy the hidden data**,
/// because the outer filesystem does not know the inner one is there and will
/// happily allocate over it. VeraCrypt offers a protection mode for exactly
/// this, and it requires the hidden volume's passphrase, which VeilVoice does
/// not have and will not ask for.
///
/// So the only safe behaviour is to ask the person who knows, once, before the
/// first write, and to refuse to write until there is an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidden {
    /// Nobody has said yet. Nothing may be written to a destination in this
    /// state.
    Unanswered,
    /// The user says this container has no hidden volume inside it.
    NoHiddenVolume,
    /// The user says this *is* the hidden volume, opened with its own
    /// passphrase. Writing here is writing inside the hidden one, which is
    /// safe.
    IsTheHiddenVolume,
    /// The user says the container has a hidden volume and this is the outer
    /// one. Writing here can destroy it.
    OuterVolumeOfAHiddenPair,
}

impl Hidden {
    /// Whether VeilVoice may write here.
    ///
    /// False for `Unanswered`, because an unanswered question is not a "no",
    /// and false for `OuterVolumeOfAHiddenPair`, because that is the case the
    /// question exists to catch.
    pub fn safe_to_write(self) -> bool {
        matches!(self, Hidden::NoHiddenVolume | Hidden::IsTheHiddenVolume)
    }

    /// Why writing is refused, in the words a user reads.
    pub fn refusal(self) -> Option<&'static str> {
        match self {
            Hidden::Unanswered => {
                Some("say whether this container has a hidden volume before VeilVoice writes to it")
            }
            Hidden::OuterVolumeOfAHiddenPair => Some(
                "this is the outer volume of a container with a hidden one inside it, and \
                 writing here can destroy the hidden data",
            ),
            _ => None,
        }
    }
}

/// A mounted volume VeilVoice could write into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// Where it is mounted, which is a plain directory by the time we see it.
    pub path: PathBuf,
    /// Which tool it belongs to, as far as the mount table says.
    pub tool: Tool,
    /// Whether it is a hidden volume, which only the user can answer.
    pub hidden: Hidden,
}

impl Volume {
    /// A volume found by probing, with its hidden state not yet asked.
    pub fn found(path: PathBuf, tool: Tool) -> Self {
        Self {
            path,
            tool,
            // Never anything else here. A default of "no hidden volume" would
            // be a guess wearing the clothes of an answer.
            hidden: Hidden::Unanswered,
        }
    }

    /// Whether VeilVoice may write here right now.
    ///
    /// Cryptomator has no hidden-volume concept, so the question does not
    /// apply and is not asked; VeraCrypt must be answered.
    pub fn ready(&self) -> bool {
        match self.tool {
            Tool::Cryptomator => true,
            Tool::VeraCrypt => self.hidden.safe_to_write(),
        }
    }

    /// Why it is not ready, if it is not.
    pub fn blocked(&self) -> Option<&'static str> {
        if self.tool == Tool::Cryptomator {
            return None;
        }
        self.hidden.refusal()
    }
}

/// Whether `tool` looks installed on this machine.
///
/// A file-system and `PATH` probe, like every other probe in this crate: no
/// subprocess, nothing that needs a spinner, and an [`Presence::Unknown`] when
/// the probe itself could not answer rather than a false "not there".
pub fn installed(tool: Tool) -> Presence {
    for candidate in candidates(tool) {
        let path = Path::new(&candidate);
        match path.try_exists() {
            Ok(true) => return Presence::Present(candidate),
            Ok(false) => {}
            Err(e) => return Presence::Unknown(format!("{candidate}: {e}")),
        }
    }
    if let Some(found) = on_path(tool) {
        return Presence::Present(found);
    }
    Presence::NotDetected
}

/// Where each tool installs itself, per platform.
fn candidates(tool: Tool) -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    match (tool, cfg!(windows), cfg!(target_os = "macos")) {
        (Tool::Cryptomator, true, _) => vec![
            r"C:\Program Files\Cryptomator\Cryptomator.exe".into(),
            r"C:\Program Files (x86)\Cryptomator\Cryptomator.exe".into(),
        ],
        (Tool::Cryptomator, _, true) => vec!["/Applications/Cryptomator.app".into()],
        (Tool::Cryptomator, _, _) => vec![
            "/usr/bin/cryptomator".into(),
            "/usr/local/bin/cryptomator".into(),
            "/opt/Cryptomator/cryptomator".into(),
            format!("{home}/.local/share/flatpak/app/org.cryptomator.Cryptomator"),
            "/var/lib/flatpak/app/org.cryptomator.Cryptomator".into(),
        ],
        (Tool::VeraCrypt, true, _) => vec![
            r"C:\Program Files\VeraCrypt\VeraCrypt.exe".into(),
            r"C:\Program Files (x86)\VeraCrypt\VeraCrypt.exe".into(),
        ],
        (Tool::VeraCrypt, _, true) => vec!["/Applications/VeraCrypt.app".into()],
        (Tool::VeraCrypt, _, _) => vec![
            "/usr/bin/veracrypt".into(),
            "/usr/local/bin/veracrypt".into(),
        ],
    }
}

/// The tool's command on `PATH`, if it is there under its usual name.
fn on_path(tool: Tool) -> Option<String> {
    let name = match tool {
        Tool::Cryptomator => "cryptomator",
        Tool::VeraCrypt => "veracrypt",
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.try_exists().unwrap_or(false) {
            return Some(candidate.display().to_string());
        }
    }
    None
}

/// Every mounted volume either tool is currently offering.
///
/// Reads the platform's mount table and nothing else. An empty list means
/// "nothing recognisable is mounted", which is not the same as "neither tool is
/// installed": see [`installed`].
pub fn mounted() -> Vec<Volume> {
    #[cfg(target_os = "linux")]
    {
        match std::fs::read_to_string("/proc/mounts") {
            Ok(table) => from_proc_mounts(&table),
            Err(_) => Vec::new(),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        from_mount_directories()
    }
}

/// Parse a Linux mount table into the volumes we recognise.
///
/// Separated from the read so it can be tested against a fixed table rather
/// than against whatever this machine happens to have mounted.
pub fn from_proc_mounts(table: &str) -> Vec<Volume> {
    let mut out = Vec::new();
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let source = fields.next().unwrap_or("");
        let Some(target) = fields.next() else {
            continue;
        };
        // `/proc/mounts` escapes spaces and a few other characters as octal.
        let target = unescape_mount(target);
        if let Some(tool) = recognise(source, &target) {
            out.push(Volume::found(PathBuf::from(target), tool));
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

/// Which tool a mount belongs to, judged by where it is mounted and what
/// mounted it.
///
/// Deliberately conservative. A directory that merely has "vault" in its name
/// is not evidence of anything, and offering to write veiled recordings into a
/// directory VeilVoice guessed at is worse than offering nothing.
fn recognise(source: &str, target: &str) -> Option<Tool> {
    // VeraCrypt mounts at /media/veracrypt<N> by default on Linux.
    if target.starts_with("/media/veracrypt") {
        return Some(Tool::VeraCrypt);
    }
    // Cryptomator's FUSE mount names itself in the source field.
    let source = source.to_ascii_lowercase();
    if source.contains("cryptomator") || target.contains("/Cryptomator/mnt/") {
        return Some(Tool::Cryptomator);
    }
    if source.starts_with("veracrypt") {
        return Some(Tool::VeraCrypt);
    }
    None
}

/// Undo the octal escaping `/proc/mounts` applies to a path.
fn unescape_mount(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let digits = &raw[i + 1..i + 4];
            if let Ok(value) = u8::from_str_radix(digits, 8) {
                out.push(value as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Volumes found by looking in the directories each platform mounts into.
///
/// The fallback for platforms with no `/proc/mounts`. Weaker than reading a
/// mount table, and it says so by being a separate function rather than
/// pretending to be the same probe.
#[cfg(not(target_os = "linux"))]
fn from_mount_directories() -> Vec<Volume> {
    let mut out = Vec::new();
    for dir in ["/Volumes"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let tool = if name.contains("veracrypt") {
                Tool::VeraCrypt
            } else if name.contains("cryptomator") {
                Tool::Cryptomator
            } else {
                continue;
            };
            out.push(Volume::found(entry.path(), tool));
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
proc /proc proc rw,nosuid 0 0
/dev/sda1 / ext4 rw,relatime 0 0
/dev/mapper/veracrypt1 /media/veracrypt1 ext4 rw,relatime 0 0
cryptomator@ /home/someone/Vault fuse rw,nosuid,nodev 0 0
tmpfs /run/user/1000 tmpfs rw,nosuid 0 0
/dev/sdb1 /media/holiday\\040photos vfat rw 0 0
";

    #[test]
    fn a_mount_table_yields_the_volumes_and_nothing_else() {
        let found = from_proc_mounts(SAMPLE);
        assert_eq!(found.len(), 2, "found: {found:?}");
        assert!(found
            .iter()
            .any(|v| v.tool == Tool::VeraCrypt && v.path == Path::new("/media/veracrypt1")));
        assert!(found
            .iter()
            .any(|v| v.tool == Tool::Cryptomator && v.path == Path::new("/home/someone/Vault")));
    }

    /// The ordinary parts of a machine must not be offered as encrypted
    /// storage. Offering `/` as a vault would be worse than offering nothing.
    #[test]
    fn ordinary_mounts_are_not_mistaken_for_encrypted_ones() {
        for volume in from_proc_mounts(SAMPLE) {
            assert_ne!(volume.path, Path::new("/"));
            assert_ne!(volume.path, Path::new("/proc"));
            assert_ne!(volume.path, Path::new("/run/user/1000"));
        }
    }

    #[test]
    fn an_escaped_mount_point_is_read_back_whole() {
        assert_eq!(
            unescape_mount("/media/holiday\\040photos"),
            "/media/holiday photos"
        );
        assert_eq!(unescape_mount("/media/plain"), "/media/plain");
    }

    /// The whole point of `Hidden`. A found volume starts unanswered, and an
    /// unanswered volume is not writable.
    #[test]
    fn a_freshly_found_veracrypt_volume_may_not_be_written_to() {
        let volume = Volume::found(PathBuf::from("/media/veracrypt1"), Tool::VeraCrypt);
        assert_eq!(volume.hidden, Hidden::Unanswered);
        assert!(!volume.ready(), "an unanswered question is not a yes");
        assert!(volume.blocked().is_some());
    }

    #[test]
    fn the_outer_volume_of_a_hidden_pair_is_refused() {
        let mut volume = Volume::found(PathBuf::from("/media/veracrypt1"), Tool::VeraCrypt);
        volume.hidden = Hidden::OuterVolumeOfAHiddenPair;
        assert!(!volume.ready());
        assert!(volume
            .blocked()
            .expect("a refusal has to say why")
            .contains("destroy"));

        volume.hidden = Hidden::NoHiddenVolume;
        assert!(volume.ready());
        volume.hidden = Hidden::IsTheHiddenVolume;
        assert!(
            volume.ready(),
            "writing inside the hidden volume is the safe case"
        );
    }

    /// Cryptomator has no hidden-volume concept, so asking would be a question
    /// with no meaning and a user trained to click through questions.
    #[test]
    fn a_cryptomator_vault_is_not_asked_the_veracrypt_question() {
        let volume = Volume::found(PathBuf::from("/home/someone/Vault"), Tool::Cryptomator);
        assert!(volume.ready());
        assert!(volume.blocked().is_none());
    }

    #[test]
    fn every_tool_round_trips_through_its_key() {
        for tool in Tool::ALL {
            assert_eq!(Tool::from_key(tool.key()), Some(*tool));
            assert!(!tool.name().is_empty());
            assert!(tool.home_page().starts_with("https://"));
        }
        assert_eq!(Tool::from_key("bitlocker"), None);
    }

    /// The advice about the disk underneath must keep naming the platforms it
    /// claims to cover, and must not quietly become a boast about vaults.
    #[test]
    fn the_disk_advice_names_every_platform_and_claims_nothing_extra() {
        let advice = DISK_ADVICE.to_lowercase();
        for platform in ["bitlocker", "filevault", "luks", "openbsd", "freebsd"] {
            assert!(
                advice.contains(platform),
                "the advice does not mention {platform}"
            );
        }
        assert!(
            advice.contains("does not protect"),
            "the limit has to be stated"
        );
        for boast in ["unbreakable", "guarantee", "completely secure"] {
            assert!(!advice.contains(boast), "overclaim: {boast}");
        }
    }

    /// Detection must never be reported as a certainty it is not. The probe
    /// answers about the places it looked.
    #[test]
    fn a_probe_that_finds_nothing_says_so_about_the_probe() {
        for tool in Tool::ALL {
            let presence = installed(*tool);
            assert!(!presence.describe().is_empty());
            if let Presence::Present(evidence) = &presence {
                assert!(
                    !evidence.is_empty(),
                    "evidence has to be a path a user can check"
                );
            }
        }
    }

    /// Nothing in this module may start a process. Detection reads.
    #[test]
    fn nothing_here_launches_either_program() {
        let source = include_str!("volumes.rs").replace("\r\n", "\n");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or("");
        let body: String = shipped
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["Command::new", "process::Command", "spawn("] {
            assert!(
                !body.contains(forbidden),
                "this module calls {forbidden:?}: detection reads, it does not drive"
            );
        }
    }
}
