// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional third-party software, detected rather than assumed.
//!
//! Four programs make VeilVoice easier to live with and none of them is part
//! of VeilVoice. A virtual audio cable is what lets live mode feed a veiled
//! microphone into a video call; an audio editor is how most people trim a
//! recording before veiling it. This module says which are already on the
//! machine, who makes each one, under what licence, and, for the ones that
//! are not, exactly what command would install it.
//!
//! # Three rules, and none of them relaxes
//!
//! **Nothing is installed without an explicit yes.** There are no checkboxes
//! here to leave ticked. [`Companion::offer`] produces a command; running it
//! is a separate deliberate act by the caller, on one named program at a time.
//!
//! **VeilVoice never runs somebody else's installer.** Where the software is
//! proprietary or ships as a driver, and VB-CABLE is both, the offer is to open
//! the vendor's page, not to fetch and execute an unverified binary. This
//! project's front page is about verifying what you run; downloading a signed
//! release, checking its signature, and then silently running an unchecked
//! third-party `.exe` from the same program would be a strange thing to do
//! with that reputation.
//!
//! **Privilege is reported, never requested.** A system package manager needs
//! root, and a graphical program cannot honestly collect a `sudo` password.
//! [`Offer::Command`] carries a `needs_privilege` flag, [`run`] refuses any
//! command that sets it, and a front end shows such a command rather than
//! pretending to run it.
//!
//! # Detection is a probe, and says when it could not tell
//!
//! [`Presence`] has three states, not two. "I looked in the places this
//! software installs itself and found nothing" and "I could not read the place
//! I wanted to look" are different answers, and reporting the second as the
//! first is how a tool ends up offering to install something that is already
//! there. Every probe here is a file-system or `PATH` lookup: no subprocess,
//! no registry sweep, and nothing that takes long enough to need a spinner.
//!
//! The probes look where each program installs itself by default. Somebody who
//! has put Audacity somewhere unusual will be told it was not detected, which
//! is exactly what the words say: [`Presence::NotDetected`] is not a claim
//! that the software is absent from the machine.
//!
//! # In plain words
//!
//! Optional extra software that makes VeilVoice easier to live with, none of which
//! is part of VeilVoice.
//!
//! Each one is looked for rather than assumed, and each is described first: what
//! it is, who makes it, what licence it has and why VeilVoice mentions it at all.
//! Nothing is installed without an explicit yes, and the exact command is shown
//! before the question.

use std::path::{Path, PathBuf};

/// What a probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// Found, with the evidence that found it: a path, so the user can check.
    Present(String),
    /// The usual locations were readable and it was not in them.
    ///
    /// Deliberately not called `Absent`: this is a statement about where the
    /// probe looked, not about the whole machine.
    NotDetected,
    /// The probe could not answer, and says why.
    Unknown(String),
}

impl Presence {
    /// True only for [`Presence::Present`].
    ///
    /// Named so the reading is unambiguous at a call site: an `Unknown` is not
    /// a `false` about the software, it is a `false` about the probe.
    pub fn is_present(&self) -> bool {
        matches!(self, Presence::Present(_))
    }

    /// A short line for a user interface, in the project's usual register.
    pub fn describe(&self) -> String {
        match self {
            Presence::Present(evidence) => format!("found at {evidence}"),
            Presence::NotDetected => "not found where it usually installs".to_string(),
            Presence::Unknown(reason) => format!("could not tell: {reason}"),
        }
    }
}

/// What VeilVoice can offer to do about a companion that is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// A command that installs it, and whether it needs privilege this program
    /// does not have.
    Command {
        /// The program and its arguments, exactly as they would be run.
        argv: Vec<String>,
        /// Which package manager this came from, for the user to recognise.
        via: &'static str,
        /// True when the command needs root or administrator rights.
        ///
        /// A front end must not run one of these. Show it, and let the user
        /// run it in a terminal where they can see what they are approving.
        needs_privilege: bool,
    },
    /// Open the vendor's own page. Their software, their installer, their
    /// licence to accept.
    Page(&'static str),
    /// Part of the operating system on this platform, so there is nothing for
    /// VeilVoice to install. Carries what to do instead.
    PartOfTheSystem(&'static str),
    /// Not applicable to the platform this is running on.
    NotOnThisPlatform,
    /// Nothing here knows how to install it on this system, and says so rather
    /// than guessing at a package manager that may not exist.
    NoKnownRoute(String),
}

impl Offer {
    /// True when a front end may run this itself.
    ///
    /// False for everything that needs privilege, opens a browser, or has no
    /// route, each of which is a decision for the person at the keyboard.
    pub fn is_runnable(&self) -> bool {
        matches!(
            self,
            Offer::Command {
                needs_privilege: false,
                ..
            }
        )
    }

    /// The command as a single line, for showing or copying. `None` when this
    /// offer is not a command.
    pub fn command_line(&self) -> Option<String> {
        match self {
            Offer::Command { argv, .. } => Some(argv.join(" ")),
            _ => None,
        }
    }
}

/// One piece of optional third-party software.
///
/// The prose fields are not decoration. Somebody being asked to install
/// software is entitled to know who wrote it and under what licence before
/// they answer, and burying that in a manual is the same as not saying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Companion {
    /// Stable identifier, for a front end to key on.
    pub key: &'static str,
    /// What it is called.
    pub name: &'static str,
    /// Who makes it.
    pub vendor: &'static str,
    /// Its licence, in the same words its authors use.
    pub licence: &'static str,
    /// What the software is.
    pub what: &'static str,
    /// Why VeilVoice mentions it at all.
    pub why: &'static str,
    /// The vendor's own page.
    pub page: &'static str,
}

impl Companion {
    /// Look for it, without changing anything.
    pub fn detect(&self) -> Presence {
        match self.key {
            "vb-cable" => detect_vb_cable(),
            "blackhole" => detect_blackhole(),
            "pipewire" => detect_pipewire(),
            "audacity" => detect_audacity(),
            "gnupg" => detect_gnupg(),
            other => Presence::Unknown(format!("no probe is written for {other}")),
        }
    }

    /// What could be done about it on this platform.
    pub fn offer(&self) -> Offer {
        match self.key {
            // Proprietary, and a driver. Their installer, run by them, after
            // their licence has been read by the person it binds.
            "vb-cable" => {
                if cfg!(windows) {
                    Offer::Page(self.page)
                } else {
                    Offer::NotOnThisPlatform
                }
            }
            "blackhole" => {
                if cfg!(target_os = "macos") {
                    brew(&["install", "--cask", "blackhole-2ch"])
                } else {
                    Offer::NotOnThisPlatform
                }
            }
            "pipewire" => {
                if cfg!(target_os = "linux") {
                    Offer::PartOfTheSystem(
                        "PipeWire is part of your distribution's audio stack. If it is \
                         missing, install it the way you install anything else on this \
                         system -- VeilVoice replacing a component of your operating \
                         system is not something it should be doing.",
                    )
                } else {
                    Offer::NotOnThisPlatform
                }
            }
            "audacity" => audacity_offer(),
            "gnupg" => gnupg_offer(),
            other => Offer::NoKnownRoute(format!("no route is written for {other}")),
        }
    }
}

/// Every companion this project knows about, in the order a front end should
/// show them.
///
/// The audio routing one comes first because it is the one live mode actually
/// needs; the editor is a convenience. Entries for other platforms are
/// deliberately kept in the list rather than compiled away: [`for_this_platform`]
/// filters, and a reader looking at the source should be able to see the whole
/// set without switching machines.
pub const ALL: &[Companion] = &[
    Companion {
        key: "vb-cable",
        name: "VB-CABLE",
        vendor: "VB-Audio Software",
        licence: "proprietary donationware -- not free software",
        what: "A virtual audio cable: a playback device whose output appears as a \
               recording device.",
        why: "Live mode writes the veiled voice to an output. A virtual cable is what \
              lets a call application pick it up as a microphone. Without one, live \
              mode still runs -- you simply have nowhere useful to send it.",
        page: "https://vb-audio.com/Cable/",
    },
    Companion {
        key: "blackhole",
        name: "BlackHole",
        vendor: "Existential Audio",
        licence: "MIT",
        what: "A virtual audio driver for macOS, doing the same job VB-CABLE does on \
               Windows.",
        why: "The route from live mode into a call on macOS.",
        page: "https://existential.audio/blackhole/",
    },
    Companion {
        key: "pipewire",
        name: "PipeWire",
        vendor: "the PipeWire project",
        licence: "MIT",
        what: "The audio server most current Linux distributions already use.",
        why: "Its null sink is the virtual cable on Linux, so live mode needs no extra \
              software at all on a machine that already runs it.",
        page: "https://pipewire.org/",
    },
    Companion {
        key: "gnupg",
        name: "GnuPG",
        vendor: "the GnuPG project",
        licence: "GPL-3.0-or-later",
        what: "The standard OpenPGP implementation: the `gpg` command that checks \
               signatures.",
        why: "VeilVoice checks a release signature itself, with code in this binary. \
              That is the check telling you a download is genuine, made by a program \
              that came out of that download. GnuPG is a second opinion from software \
              this project did not write, and it is the one worth having.",
        page: "https://gnupg.org/",
    },
    Companion {
        key: "audacity",
        name: "Audacity",
        vendor: "the Audacity team",
        licence: "GPL-2.0-or-later",
        what: "A free audio editor and recorder.",
        why: "Useful for recording a file and for trimming one before veiling it. It is \
              recommended and never embedded: GPL-2.0-or-later cannot be combined with \
              this project's GPL-3.0-or-later.",
        page: "https://www.audacityteam.org/",
    },
];

/// The companions that mean anything on the platform this is running on.
///
/// Audacity is on every platform; each virtual-cable entry is on exactly one.
/// Offering a macOS driver to a Windows user is noise, and noise in a security
/// tool's interface is how the parts that matter stop being read.
pub fn for_this_platform() -> Vec<&'static Companion> {
    ALL.iter()
        .filter(|companion| !matches!(companion.offer(), Offer::NotOnThisPlatform))
        .collect()
}

/// Find one by [`Companion::key`].
pub fn by_key(key: &str) -> Option<&'static Companion> {
    ALL.iter().find(|companion| companion.key == key)
}

/// Run an offer, and return what it printed.
///
/// Refuses anything [`Offer::is_runnable`] rejects. That refusal is the point:
/// a front end that has a "yes" button should not be able to turn it into
/// `sudo` by passing a different offer, and the check lives here rather than
/// in each front end so there is one of it.
pub fn run(offer: &Offer) -> Result<String, String> {
    let Offer::Command { argv, .. } = offer else {
        return Err("this is not something VeilVoice runs for you".to_string());
    };
    if !offer.is_runnable() {
        return Err(
            "this command needs administrator rights, which VeilVoice does not ask for. \
             Run it yourself in a terminal."
                .to_string(),
        );
    }
    let (program, arguments) = argv
        .split_first()
        .ok_or_else(|| "an empty command".to_string())?;
    let output = crate::command(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
    let complaint = String::from_utf8_lossy(&output.stderr);
    if !complaint.trim().is_empty() {
        report.push('\n');
        report.push_str(&complaint);
    }
    if output.status.success() {
        Ok(report)
    } else {
        Err(format!("{program} failed:\n{}", report.trim()))
    }
}

/// Open a companion's own page in the user's browser.
///
/// This is what VeilVoice does instead of installing proprietary software:
/// it takes you to the people who wrote it, so their licence is accepted by
/// the person it binds and their installer is run by the person who chose it.
///
/// Refuses anything that is not `https://`. The URLs here are constants in
/// this file rather than anything a user or a file supplies, so this cannot
/// currently fail -- which is the reason to check now, while it is cheap,
/// rather than after somebody makes the list configurable.
pub fn open_page(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing to open {url}: only https is opened"));
    }
    let mut spawn = if cfg!(windows) {
        // `start` is a `cmd` builtin, not a program. The empty argument is the
        // window title: without it `start` treats the first quoted argument as
        // the title and opens nothing.
        let mut command = crate::command("cmd");
        command.args(["/c", "start", "", url]);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = crate::command("open");
        command.arg(url);
        command
    } else {
        let mut command = crate::command("xdg-open");
        command.arg(url);
        command
    };
    // Spawned rather than waited on: `xdg-open` may not return until the
    // browser it started exits, and a window frozen behind somebody's browser
    // is not what "open their page" should mean.
    spawn
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open a browser: {error}"))
}

// --- the probes -------------------------------------------------------------

/// Is `stem` an executable on this process's `PATH`?
///
/// Walks the entries rather than spawning `where`/`which`, which would cost a
/// subprocess to answer a question about a string this process already holds.
fn on_path(stem: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{stem}.exe"), format!("{stem}.bat")]
    } else {
        vec![stem.to_string()]
    };
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in &names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// The first of `candidates` that exists.
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.exists()).cloned()
}

fn env_path(name: &str, rest: &[&str]) -> Option<PathBuf> {
    let base = std::env::var_os(name)?;
    let mut path = PathBuf::from(base);
    for part in rest {
        path.push(part);
    }
    Some(path)
}

/// VB-CABLE installs an audio driver, and a driver is a file in a directory
/// any user can read. That is a faster and steadier probe than sweeping the
/// uninstall registry, which is large, slow, and describes what was installed
/// rather than what is loaded.
fn detect_vb_cable() -> Presence {
    if !cfg!(windows) {
        return Presence::NotDetected;
    }
    let Some(drivers) = env_path("SystemRoot", &["System32", "drivers"]) else {
        return Presence::Unknown("this system does not say where Windows is".to_string());
    };
    let entries = match std::fs::read_dir(&drivers) {
        Ok(entries) => entries,
        Err(error) => {
            return Presence::Unknown(format!("cannot read {}: {error}", drivers.display()))
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.starts_with("vbaudio") {
            return Presence::Present(entry.path().display().to_string());
        }
    }
    Presence::NotDetected
}

/// BlackHole is a HAL plug-in, and those live in exactly one place.
fn detect_blackhole() -> Presence {
    if !cfg!(target_os = "macos") {
        return Presence::NotDetected;
    }
    let plugins = Path::new("/Library/Audio/Plug-Ins/HAL");
    let entries = match std::fs::read_dir(plugins) {
        Ok(entries) => entries,
        Err(error) => {
            return Presence::Unknown(format!("cannot read {}: {error}", plugins.display()))
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.starts_with("blackhole") {
            return Presence::Present(entry.path().display().to_string());
        }
    }
    Presence::NotDetected
}

/// PipeWire is running or it is not, and `pw-cli` beside it is the sign that
/// the userspace tools are installed. Either name is enough to say "present".
fn detect_pipewire() -> Presence {
    if !cfg!(target_os = "linux") {
        return Presence::NotDetected;
    }
    match on_path("pipewire").or_else(|| on_path("pw-cli")) {
        Some(found) => Presence::Present(found.display().to_string()),
        None => Presence::NotDetected,
    }
}

/// Audacity is on `PATH` on Unix and in one of three directories on Windows.
fn detect_audacity() -> Presence {
    if let Some(found) = on_path("audacity") {
        return Presence::Present(found.display().to_string());
    }
    let mut candidates = Vec::new();
    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(path) = env_path(variable, &["Audacity", "audacity.exe"]) {
                candidates.push(path);
            }
        }
        if let Some(path) = env_path("LOCALAPPDATA", &["Programs", "Audacity", "audacity.exe"]) {
            candidates.push(path);
        }
    }
    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/Applications/Audacity.app"));
    }
    match first_existing(&candidates) {
        Some(found) => Presence::Present(found.display().to_string()),
        None => Presence::NotDetected,
    }
}

// --- the offers -------------------------------------------------------------

fn brew(arguments: &[&str]) -> Offer {
    if on_path("brew").is_none() {
        return Offer::NoKnownRoute(
            "Homebrew is not installed, and VeilVoice is not going to install a package \
             manager for you. Get it from https://brew.sh, or download the software from \
             its own page."
                .to_string(),
        );
    }
    let mut argv = vec!["brew".to_string()];
    argv.extend(arguments.iter().map(|part| part.to_string()));
    Offer::Command {
        argv,
        via: "Homebrew",
        // Homebrew installs into a prefix the user owns, on purpose.
        needs_privilege: false,
    }
}

/// The route to Audacity differs per platform, and on Linux per distribution.
///
/// The package managers here are the same list `install/install.sh` uses, in
/// the same order, so the two agree about what this machine is. The ones that
/// need `sudo` are marked as needing it rather than being run: this program is
/// not going to prompt for a root password.
/// GnuPG, which is on `PATH` or is not.
///
/// No hunt through program directories, unlike the audio companions. A `gpg`
/// that is installed but not on `PATH` is one the commands printed beside the
/// verifier would not find either, so reporting it as present would be
/// reporting something the reader cannot use.
fn detect_gnupg() -> Presence {
    for name in ["gpg", "gpg2"] {
        if let Some(found) = on_path(name) {
            return Presence::Present(found.display().to_string());
        }
    }
    Presence::NotDetected
}

/// How GnuPG would be installed here.
///
/// On Windows this is Gpg4win through winget, which is the packaging almost
/// everybody means by "GnuPG on Windows". The other route on Windows is a
/// `gpg` inside WSL, which is not an install of anything on Windows itself
/// and is offered separately by `veilvoice-gnupg`.
fn gnupg_offer() -> Offer {
    if cfg!(windows) {
        return if on_path("winget").is_some() {
            Offer::Command {
                argv: vec![
                    "winget".to_string(),
                    "install".to_string(),
                    "--id".to_string(),
                    "GnuPG.Gpg4win".to_string(),
                    "-e".to_string(),
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ],
                via: "winget",
                needs_privilege: false,
            }
        } else {
            Offer::NoKnownRoute(
                "winget is not on this system. Gpg4win can be downloaded from \
                 gpg4win.org, or a `gpg` inside WSL can be used instead."
                    .to_string(),
            )
        };
    }
    if cfg!(target_os = "macos") {
        return brew(&["install", "gnupg"]);
    }
    for (manager, install) in UNIX_PACKAGE_MANAGERS {
        if on_path(manager).is_none() {
            continue;
        }
        let mut argv = vec!["sudo".to_string(), (*manager).to_string()];
        argv.extend(install.iter().map(|part| (*part).to_string()));
        // Debian and Ubuntu call it `gnupg`; so do Fedora and Arch.
        argv.push("gnupg".to_string());
        return Offer::Command {
            argv,
            via: manager,
            needs_privilege: true,
        };
    }
    Offer::NoKnownRoute(
        "no package manager this program recognises is on PATH. Install GnuPG the \
         way you install anything else on this system."
            .to_string(),
    )
}

fn audacity_offer() -> Offer {
    if cfg!(windows) {
        return if on_path("winget").is_some() {
            Offer::Command {
                argv: vec![
                    "winget".to_string(),
                    "install".to_string(),
                    "--id".to_string(),
                    "Audacity.Audacity".to_string(),
                    "-e".to_string(),
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ],
                via: "winget",
                // winget elevates itself for a machine-scope package, with
                // Windows asking the user rather than this program.
                needs_privilege: false,
            }
        } else {
            Offer::NoKnownRoute(
                "winget is not on this system. Download Audacity from its own page.".to_string(),
            )
        };
    }
    if cfg!(target_os = "macos") {
        return brew(&["install", "--cask", "audacity"]);
    }
    // Linux and the BSDs: whichever package manager is actually here.
    for (manager, install) in UNIX_PACKAGE_MANAGERS {
        if on_path(manager).is_none() {
            continue;
        }
        let mut argv = vec!["sudo".to_string(), (*manager).to_string()];
        argv.extend(install.iter().map(|part| (*part).to_string()));
        argv.push("audacity".to_string());
        return Offer::Command {
            argv,
            via: manager,
            needs_privilege: true,
        };
    }
    Offer::NoKnownRoute(
        "no package manager this program recognises is on PATH. Install Audacity the \
         way you install anything else on this system."
            .to_string(),
    )
}

/// Package managers, and the arguments that install one named package
/// non-interactively. The same set and the same order as `install/install.sh`.
const UNIX_PACKAGE_MANAGERS: &[(&str, &[&str])] = &[
    ("apt-get", &["install", "-y"]),
    ("dnf", &["install", "-y"]),
    ("pacman", &["-S", "--noconfirm"]),
    ("zypper", &["install", "-y"]),
    ("apk", &["add"]),
    ("pkg", &["install", "-y"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_companion_has_a_probe_and_a_route() {
        for companion in ALL {
            // A probe that is not written returns Unknown with that wording,
            // which is the failure this catches.
            let presence = companion.detect();
            if let Presence::Unknown(reason) = &presence {
                assert!(
                    !reason.contains("no probe is written"),
                    "{} has no probe",
                    companion.key
                );
            }
            if let Offer::NoKnownRoute(reason) = companion.offer() {
                assert!(
                    !reason.contains("no route is written"),
                    "{} has no route",
                    companion.key
                );
            }
        }
    }

    #[test]
    fn detection_changes_nothing_and_is_repeatable() {
        for companion in ALL {
            let first = companion.detect();
            let second = companion.detect();
            assert_eq!(
                first, second,
                "{} answered differently twice: a probe has a side effect",
                companion.key
            );
        }
    }

    /// The keys a front end stores must be stable and unique.
    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<&str> = ALL.iter().map(|c| c.key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two companions share a key");
    }

    /// Every companion names its author and its licence. Somebody being asked
    /// to install software is entitled to both before they answer, and an
    /// empty string here would render as a blank line nobody notices.
    #[test]
    fn each_one_says_who_wrote_it_and_under_what_licence() {
        for companion in ALL {
            assert!(!companion.vendor.trim().is_empty(), "{}", companion.key);
            assert!(!companion.licence.trim().is_empty(), "{}", companion.key);
            assert!(!companion.what.trim().is_empty(), "{}", companion.key);
            assert!(!companion.why.trim().is_empty(), "{}", companion.key);
            assert!(
                companion.page.starts_with("https://"),
                "{} must link over https",
                companion.key
            );
        }
    }

    /// VB-CABLE is proprietary donationware, and the offer must never become a
    /// download-and-run. This is a locked position, not a default.
    #[test]
    fn proprietary_software_is_only_ever_a_link() {
        let cable = by_key("vb-cable").expect("VB-CABLE is in the list");
        assert!(cable.licence.contains("proprietary"));
        match cable.offer() {
            Offer::Page(url) => assert_eq!(url, cable.page),
            Offer::NotOnThisPlatform => {} // not Windows; nothing is offered
            other => panic!("VB-CABLE must only ever be a link, got {other:?}"),
        }
    }

    /// A front end must not be able to hand `run` something that needs root.
    #[test]
    fn run_refuses_anything_needing_privilege() {
        let offer = Offer::Command {
            argv: vec!["sudo".to_string(), "apt-get".to_string()],
            via: "apt-get",
            needs_privilege: true,
        };
        assert!(!offer.is_runnable());
        let error = run(&offer).expect_err("a privileged command must be refused");
        assert!(error.contains("administrator"), "{error}");

        assert!(run(&Offer::Page("https://example.invalid")).is_err());
        assert!(run(&Offer::NotOnThisPlatform).is_err());
    }

    /// Any command that starts with `sudo` must be marked as needing
    /// privilege, or `run` would happily spawn it and block on a password
    /// prompt no window can answer.
    #[test]
    fn a_sudo_command_is_always_marked_privileged() {
        for companion in ALL {
            if let Offer::Command {
                argv,
                needs_privilege,
                ..
            } = companion.offer()
            {
                if argv.first().map(|first| first == "sudo").unwrap_or(false) {
                    assert!(
                        needs_privilege,
                        "{} would run sudo without saying so",
                        companion.key
                    );
                }
            }
        }
    }

    /// Presence has three states and the middle one is not a claim about the
    /// machine. If this ever reads "not installed" the wording has drifted.
    #[test]
    fn not_detected_does_not_claim_absence() {
        let text = Presence::NotDetected.describe();
        assert!(
            text.contains("not found where it usually installs"),
            "{text}"
        );
        assert!(!text.contains("not installed"), "{text}");
        assert!(Presence::Unknown("no reason".into())
            .describe()
            .contains("could not tell"));
        assert!(!Presence::Unknown("x".into()).is_present());
        assert!(!Presence::NotDetected.is_present());
        assert!(Presence::Present("/somewhere".into()).is_present());
    }

    /// The platform list is a filter, never an empty screen: Audacity applies
    /// everywhere, so there is always at least one entry.
    #[test]
    fn this_platform_has_at_least_the_editor() {
        let here = for_this_platform();
        assert!(
            here.iter().any(|companion| companion.key == "audacity"),
            "Audacity applies on every platform"
        );
        for companion in here {
            assert!(!matches!(companion.offer(), Offer::NotOnThisPlatform));
        }
    }

    /// Exactly one virtual-cable companion is shown per platform. Two would
    /// mean somebody is being offered a driver for an operating system they
    /// are not running.
    #[test]
    fn one_virtual_cable_at_most_per_platform() {
        let cables = for_this_platform()
            .into_iter()
            .filter(|companion| matches!(companion.key, "vb-cable" | "blackhole"))
            .count();
        assert!(cables <= 1, "{cables} virtual cables offered at once");
    }

    /// Only https, and the check is here rather than at each call site.
    #[test]
    fn only_https_pages_are_opened() {
        for refused in [
            "http://example.invalid",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "",
        ] {
            let error = open_page(refused).expect_err("should refuse");
            assert!(error.contains("only https"), "{error}");
        }
    }

    /// Every page in the list must be one `open_page` would accept, or a
    /// button in the interface leads to an error message.
    #[test]
    fn every_page_is_one_that_could_be_opened() {
        for companion in ALL {
            assert!(
                companion.page.starts_with("https://"),
                "{} would be refused by open_page",
                companion.key
            );
        }
    }

    #[test]
    fn a_missing_key_is_none() {
        assert!(by_key("not-a-companion").is_none());
        assert!(by_key("audacity").is_some());
    }

    #[test]
    fn nothing_on_path_is_found_for_a_nonsense_name() {
        assert!(on_path("this-program-does-not-exist-42").is_none());
    }
}
