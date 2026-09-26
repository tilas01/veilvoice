// SPDX-License-Identifier: GPL-3.0-or-later
//! What the user has chosen about how the app looks and moves.
//!
//! # Nothing here is secret, and nothing here is required
//!
//! Preferences are a convenience. Every field has a working default, a missing
//! file is not an error, and a corrupt one is not either -- it falls back to
//! the defaults and says so in the settings panel rather than refusing to
//! start. An app that will not open because its preferences file has a stray
//! byte in it has turned a cosmetic setting into an outage.
//!
//! This is deliberately *not* written through
//! [`veilvoice_crypto::privatefile`]. That module exists for files whose
//! contents are sensitive, and using it here would blur a distinction worth
//! keeping: your choice of colour scheme is not a secret, and treating it like
//! one would make the real protections look like decoration.
//!
//! # The format
//!
//! One `key = value` per line, ASCII, with `#` comments -- readable and
//! editable with any text editor, for the same reason the integrity manifest
//! is a text format. No parser dependency, and no way for a malformed file to
//! do anything more interesting than be ignored.
//!
//! # Animations
//!
//! On by default, and switchable off in two places: the settings panel, and
//! the `VEILVOICE_NO_ANIMATION` environment variable, which wins over the file
//! so that a machine which struggles with them can be fixed without opening
//! the UI that is struggling.
//!
//! The system's own "reduce motion" setting is honoured above both. Someone who
//! has told their operating system they do not want movement has already
//! answered this question, and a privacy tool asking again -- and defaulting to
//! yes -- would be ignoring them.
//!
//! # Two kinds of default, and which one this file holds
//!
//! **Roadmap item 170.** [`Prefs::default`] is the written-down starting
//! point: constants, chosen once, the same on every machine. It is what the
//! tests construct and what a build with nothing to ask falls back to.
//!
//! [`Prefs::starting_point`] is the one a real first run takes, and it is not
//! the same thing. Where a value can be established by asking this machine, it
//! is asked. Which is not most of them: a theme, a notification style and a
//! vault folder are preferences and there is nothing to measure. The one that
//! can be measured is the drawing path, and [`crate::probe`] is where that
//! question is put. The other two the roadmap item names were already measured
//! and were not described that way: `frame_rate` of `0` means the display's own
//! rate, which [`crate::pace`] takes from the intervals between frames, and
//! whether anything animates is resolved against the system's reduce-motion
//! setting every frame by [`Motion::resolve`].
//!
//! The measurement happens once, on the run with no file to read, and is
//! written into the file with everything else. [`Prefs::defaults_measured`]
//! records that it happened, so the interface can say the machine was asked
//! rather than implying it.
//!
//! # In plain words
//!
//! What you have chosen about how VeilVoice looks and behaves, kept in a small
//! text file.
//!
//! The first time it runs, the things that can be worked out from your computer
//! are worked out from your computer rather than guessed, and written down here
//! so it only has to look once. All of them are still settings.
//!
//! Nothing in it is secret and nothing in it is required: delete the file and the
//! application opens with its defaults. You can read it and edit it by hand.
//!
//! A setting this version does not recognise falls back to the default rather than
//! stopping the program, and where the safe direction matters, the default is the
//! one that keeps a protection on.

use std::path::{Path, PathBuf};

/// Everything the user can choose about presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Stable identifier of the colour scheme, e.g. `tokyo-night`.
    pub theme: String,
    /// Whether transitions and easing run at all.
    pub animations: bool,
    /// Whether the header mark animates as a soundbar.
    pub animated_icon: bool,
    /// Whether the first-run panel has been answered.
    pub configured: bool,
    /// The tabs the tour has already covered, comma separated.
    ///
    /// The tab list rather than a "seen" flag or a version number. A flag
    /// cannot answer the question an upgrade asks, and a version answers it
    /// only indirectly: it says that something changed, where this says what.
    /// Empty means the tour has never run.
    pub toured_tabs: String,
    /// Whether the window asks the platform for a hardware-drawn context.
    ///
    /// **Roadmap item 137.** On, and the setting exists for the machines where on is
    /// the wrong answer. Asking is already the safe direction: `Preferred`
    /// takes a software context when no GPU one is available, so a virtual
    /// machine, a remote desktop or a server with no card still opens.
    ///
    /// What it cannot survive is a driver that *accepts* the request and then
    /// draws badly, which is a real class of machine: a hybrid-graphics laptop
    /// handing over the wrong adapter, or a driver whose OpenGL path is broken
    /// in a way that shows as a black window rather than as a refusal. Nothing
    /// here can detect that, because from inside the process it looks like
    /// success. So it is one setting, off in one place, and the About tab shows
    /// what was asked for beside what the driver actually gave.
    pub acceleration: bool,
    /// Whether the install tab is hidden even on a portable copy.
    ///
    /// The tab already disappears once VeilVoice is installed -- an installed
    /// program offering to install itself is a tab that can only mislead. This
    /// is for somebody who runs the portable copy on purpose and does not want
    /// to be asked again.
    pub hide_install_tab: bool,
    /// Whether the app opens with group mode already on.
    ///
    /// The *mode itself* is deliberately not stored. Group mode changes what a
    /// recording is treated as, and a mode that survives a restart is a mode
    /// somebody eventually forgets is on -- which for this tool means a
    /// single-speaker recording rendered against a plan that does not describe
    /// it. So the toggle is per-run, and this separate, explicit tick is the
    /// only way it starts on.
    pub always_group: bool,
    /// Whether every recording is sealed with the app-lock passphrase.
    ///
    /// **Roadmap item 86.** Persisted, unlike group mode, and the difference is what
    /// happens when somebody forgets it is on. A forgotten group mode renders a
    /// single speaker against a plan that does not describe them, which is
    /// wrong output. A forgotten sealing mode encrypts a file that would
    /// otherwise have been encrypted some other way, which is not. The
    /// direction it can fail in is the safe one, so it survives a restart and
    /// the user is not asked to choose it again every launch.
    ///
    /// **Roadmap item 170 turned it on.** It is the "encryption at rest" the
    /// item names, and the argument for off was that it does nothing without
    /// an app lock, which was true and was the wrong way round: a setting that
    /// does nothing until a protection exists is exactly the setting to have
    /// already chosen when one appears. Off meant that somebody who set an app
    /// lock in the first-run cards got a locked window and unencrypted
    /// recordings, and had to find a third tab to join the two.
    ///
    /// On with no app lock still changes nothing at all, which is what makes
    /// this safe to default: [`crate::security::Security::prefer_app_lock_sealing`]
    /// holds it until there is a lock to apply it to. That holding is new. It
    /// had to be, because the old code read the preference once at startup and
    /// then wrote the live state back over it every frame, which cleared it on
    /// the first frame of every run that had no lock yet. See F-202.
    pub seal_with_app_lock: bool,
    /// The encrypted folder veiled recordings are written into, or empty.
    ///
    /// **Roadmap items 82 to 84.** Three fields rather than one, because the hidden
    /// state has to survive a restart alongside the path: a destination whose
    /// answer was forgotten would ask again, and a user asked the same question
    /// every launch stops reading it.
    pub vault_dir: String,
    /// Which tool that folder belongs to: `cryptomator` or `veracrypt`.
    pub vault_tool: String,
    /// The answer to the hidden-volume question. Anything unrecognised, or
    /// missing, reads as unanswered and blocks writing.
    pub vault_hidden: String,
    /// Whether the window locks itself after a period of no use.
    ///
    /// **Roadmap item 92.** On at half an hour, and the reason it is no longer off is
    /// in [`crate::autolock`]: a default nobody is shown is an answer rather
    /// than a question, and the answer it used to give was no protection at
    /// all to everybody who never opened this tab.
    pub autolock: bool,
    /// How long that period is, in seconds.
    pub autolock_after: u64,
    /// The bottom of the range the interface offers, in seconds.
    pub autolock_floor: u64,
    /// The top of it.
    pub autolock_ceiling: u64,
    /// How notifications are shown: `overlay`, `alert` or `off`.
    ///
    /// Stored as its key rather than as a number, so a settings file stays
    /// readable and so reordering the enum cannot silently change somebody's
    /// choice -- which for the value `off` would mean turning their warnings
    /// back on, or worse, off.
    pub notify_style: String,
    /// Where the live monitor sits: `toolbar`, `overlay` or `off`.
    ///
    /// Stored as its key, like the others. An unreadable value reads back as
    /// the default, which **shows** it: the monitor is the only picture of a
    /// live microphone available from a tab that is not the live one, and a
    /// settings file this build cannot parse must not be the reason it is
    /// missing.
    pub live_monitor: String,
    /// The Failsafe posture: `close`, `warn` or `off`.
    ///
    /// Stored as its key. An unreadable value reads back as the **default**,
    /// which is on -- a settings file this build cannot parse must never be
    /// the reason the safety catch is off.
    pub failsafe: String,
    /// Frames a second to aim for while something is moving. Zero means the
    /// display's own rate, measured rather than asked for: see
    /// [`crate::pace`].
    pub frame_rate: u32,
    /// Whether the header carries a live frame-rate readout.
    pub show_frame_rate: bool,
    /// Whether this file started from a look at the machine it is on.
    ///
    /// **Roadmap item 170.** Persisted, and the reason it is persisted rather
    /// than recomputed is that the look costs a subprocess. It is taken on the
    /// run that finds no settings file, written down with the answer, and read
    /// back for ever after.
    ///
    /// What it is *for* is honesty in the interface. "Asking the driver to
    /// draw the window" reads differently depending on whether this machine
    /// was asked what it has or whether every machine is asked the same
    /// question, and the first-run card says which. False here means the
    /// values came from [`Prefs::default`], which is the written-down set.
    pub defaults_measured: bool,
    /// Set when the file on disk could not be understood, so the settings
    /// panel can say the defaults are in force and why. Never persisted.
    pub recovered_from_corrupt_file: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "tokyo-night".to_string(),
            // On by default, as asked. The system's reduce-motion setting still
            // overrides this at the point of use -- see `Motion`.
            animations: true,
            animated_icon: true,
            configured: false,
            toured_tabs: String::new(),
            acceleration: true,
            hide_install_tab: false,
            always_group: false,
            // Roadmap item 170. On, and it does nothing until there is an app
            // lock to seal with, which is the point: the answer is already
            // chosen when one appears. See the field for why off was wrong.
            seal_with_app_lock: true,
            vault_dir: String::new(),
            vault_tool: String::new(),
            vault_hidden: String::new(),
            autolock: true,
            autolock_after: crate::autolock::DEFAULT_SECS,
            autolock_floor: crate::autolock::FLOOR_SECS,
            autolock_ceiling: crate::autolock::CEILING_SECS,
            notify_style: crate::notify::Style::default().key().to_string(),
            // On, and docked. Veiling as it runs is the mode where what is being
            // protected is happening now, and the two questions a person has
            // are "is it hearing me" and "is anything coming out". Both are
            // answered by a strip that is already on screen.
            live_monitor: crate::monitor::Style::default().key().to_string(),
            failsafe: veilvoice_guard::failsafe::Posture::default()
                .key()
                .to_string(),
            // The display's own rate. A number here would be a guess about
            // somebody else's monitor, and the whole point of `pace` is that
            // the window measures it instead.
            frame_rate: 0,
            // Off: a number that changes sixty times a second in the corner of
            // a privacy tool is a distraction for everybody who is not
            // diagnosing it.
            show_frame_rate: false,
            // False, because this is the written-down set by definition.
            // `starting_point` is what sets it.
            defaults_measured: false,
            recovered_from_corrupt_file: false,
        }
    }
}

/// Where preferences live: beside the app lock, in this platform's config
/// directory. `None` when the environment does not say where that is, in which
/// case the app runs on defaults and simply does not persist them.
pub fn default_path() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("settings.conf"))
}

impl Prefs {
    /// Read preferences from `path`.
    ///
    /// Never fails. A missing file gives the defaults; an unreadable or
    /// unparseable one gives the defaults with
    /// [`recovered_from_corrupt_file`](Self::recovered_from_corrupt_file) set,
    /// so the UI can be honest about it.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            // Missing is the ordinary case on a first run, and is not worth
            // reporting. An unreadable file is reported below only if it
            // parses to nothing useful, because the distinction the user cares
            // about is "are my settings in force", not "which syscall failed".
            return Self::default();
        };
        Self::parse(&text)
    }

    /// The preferences to open with: the file if there is one, and otherwise a
    /// look at this machine.
    ///
    /// **Roadmap item 170.** This is the one that probes, and it is a separate
    /// function from [`load`](Self::load) on purpose. `load` is named after a
    /// cheap thing and has to stay one: it is called from tests and from
    /// anything that wants to read a settings file without side effects.
    /// Running `lspci` inside it would be the kind of surprise this project
    /// writes findings about.
    ///
    /// `None` is a machine whose platform does not say where an application
    /// should keep its files. It still gets the measured starting point,
    /// because the values are just as right for this session; there is simply
    /// nowhere to remember them, so the look is taken again next launch.
    ///
    /// The probe behind it is cached for the process, so the two callers that
    /// both need this before the first frame cost one look between them: `main`
    /// creates the window from `acceleration`, and
    /// [`crate::settings::Settings::load`] builds the panel from the rest.
    pub fn starting_point(path: Option<&Path>) -> Self {
        match path {
            // `is_file` rather than a failed read, because the two say
            // different things. No file is a first run. A file that exists and
            // will not read is a broken file, and `load` reports that as the
            // defaults being in force, which the panel shows. Measuring over
            // the top of it would replace one honest report with a silent
            // reset.
            Some(path) if path.is_file() => Self::load(path),
            _ => Self::measured(crate::probe::look()),
        }
    }

    /// The starting point for a machine that has just been asked about itself.
    ///
    /// Takes the answer rather than fetching it, so the whole of this can be
    /// tested against a constructed machine without a subprocess.
    ///
    /// Everything not listed here is a preference with nothing to measure, and
    /// comes from [`Self::default`] unchanged.
    ///
    /// `animations` is deliberately **not** set from
    /// [`crate::probe::Machine`], and it is worth saying why, because the
    /// roadmap item asks for animation to follow the system and it looks like
    /// the omission. It already does, in [`Motion::resolve`], which resolves it
    /// every frame against the live setting. Writing the system's answer into
    /// the file here would freeze a moment of it: somebody who turned reduced
    /// motion on for an afternoon and off again would have a still window and a
    /// preference saying off, with nothing to connect the two. Resolved beats
    /// recorded for a value the system can change underneath us.
    pub fn measured(machine: &crate::probe::Machine) -> Self {
        Self {
            acceleration: machine.acceleration,
            defaults_measured: true,
            ..Self::default()
        }
    }

    /// Parse the `key = value` format. Unknown keys are ignored, so a file
    /// written by a newer build still works in an older one.
    pub fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        let mut understood = 0usize;
        let mut lines = 0usize;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            lines += 1;
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "theme" => {
                    // Only accept a theme this build actually has. An unknown
                    // one keeps the default rather than leaving the app
                    // pointing at a scheme that does not exist.
                    if crate::theme::by_id(value).is_some() {
                        prefs.theme = value.to_string();
                        understood += 1;
                    }
                }
                "animations" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.animations = on;
                        understood += 1;
                    }
                }
                "animated_icon" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.animated_icon = on;
                        understood += 1;
                    }
                }
                "toured_tabs" => {
                    prefs.toured_tabs = value.to_string();
                    understood += 1;
                }
                "configured" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.configured = on;
                        understood += 1;
                    }
                }
                "acceleration" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.acceleration = on;
                        understood += 1;
                    }
                }
                "hide_install_tab" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.hide_install_tab = on;
                        understood += 1;
                    }
                }
                "defaults_measured" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.defaults_measured = on;
                        understood += 1;
                    }
                }
                "failsafe" => {
                    // Through `from_key`, so an unrecognised value lands on the
                    // default rather than being stored and acted on. The
                    // default is on.
                    prefs.failsafe = veilvoice_guard::failsafe::Posture::from_key(value)
                        .key()
                        .to_string();
                }
                "notify_style" => {
                    // Anything unrecognised becomes the default rather than an
                    // error, and the default shows something. A file this build
                    // cannot read must never be the reason a warning is silent.
                    prefs.notify_style = crate::notify::Style::from_key(value).key().to_string();
                }
                "live_monitor" => {
                    // Through `from_key`, so anything unrecognised lands on the
                    // default, which shows the monitor.
                    prefs.live_monitor = crate::monitor::Style::from_key(value).key().to_string();
                }
                "always_group" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.always_group = on;
                        understood += 1;
                    }
                }
                "seal_with_app_lock" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.seal_with_app_lock = on;
                        understood += 1;
                    }
                }
                "vault_dir" => {
                    prefs.vault_dir = value.to_string();
                    understood += 1;
                }
                "vault_tool" => {
                    prefs.vault_tool = value.to_string();
                    understood += 1;
                }
                "vault_hidden" => {
                    prefs.vault_hidden = value.to_string();
                    understood += 1;
                }
                "autolock" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.autolock = on;
                        understood += 1;
                    }
                }
                "frame_rate" => {
                    // Clamped on the way in, so a hand-edited file cannot ask
                    // for one frame a second or ten thousand. Zero is the
                    // display and is left alone.
                    if let Ok(hz) = value.parse::<u32>() {
                        prefs.frame_rate = if hz == 0 {
                            0
                        } else {
                            hz.clamp(crate::pace::DISPLAY_FLOOR, crate::pace::DISPLAY_CEILING)
                        };
                        understood += 1;
                    }
                }
                "show_frame_rate" => {
                    if let Some(on) = parse_bool(value) {
                        prefs.show_frame_rate = on;
                        understood += 1;
                    }
                }
                "autolock_after" => {
                    if let Ok(secs) = value.parse() {
                        prefs.autolock_after = secs;
                        understood += 1;
                    }
                }
                "autolock_floor" => {
                    if let Ok(secs) = value.parse() {
                        prefs.autolock_floor = secs;
                        understood += 1;
                    }
                }
                "autolock_ceiling" => {
                    if let Ok(secs) = value.parse() {
                        prefs.autolock_ceiling = secs;
                        understood += 1;
                    }
                }
                _ => {}
            }
        }

        // A file with content, none of which we could use, is a corrupt file.
        // An empty one is just an empty one.
        prefs.recovered_from_corrupt_file = lines > 0 && understood == 0;
        prefs
    }

    /// Serialise to the text format.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# VeilVoice settings. Plain text on purpose: edit it, or delete\n");
        out.push_str("# it to go back to the defaults. Nothing here is secret.\n");
        out.push_str(&format!("theme = {}\n", self.theme));
        out.push_str(&format!("animations = {}\n", self.animations));
        out.push_str(&format!("animated_icon = {}\n", self.animated_icon));
        out.push_str(&format!("configured = {}\n", self.configured));
        out.push_str(&format!("toured_tabs = {}\n", self.toured_tabs));
        out.push_str(&format!("acceleration = {}\n", self.acceleration));
        out.push_str(&format!("hide_install_tab = {}\n", self.hide_install_tab));
        out.push_str(&format!("defaults_measured = {}\n", self.defaults_measured));
        out.push_str(&format!("always_group = {}\n", self.always_group));
        out.push_str(&format!(
            "seal_with_app_lock = {}\n",
            self.seal_with_app_lock
        ));
        out.push_str(&format!("vault_dir = {}\n", self.vault_dir));
        out.push_str(&format!("vault_tool = {}\n", self.vault_tool));
        out.push_str(&format!("vault_hidden = {}\n", self.vault_hidden));
        out.push_str(&format!("autolock = {}\n", self.autolock));
        out.push_str(&format!("autolock_after = {}\n", self.autolock_after));
        out.push_str(&format!("autolock_floor = {}\n", self.autolock_floor));
        out.push_str(&format!("autolock_ceiling = {}\n", self.autolock_ceiling));
        out.push_str(&format!("notify_style = {}\n", self.notify_style));
        out.push_str(&format!("live_monitor = {}\n", self.live_monitor));
        out.push_str(&format!("failsafe = {}\n", self.failsafe));
        out.push_str(&format!("frame_rate = {}\n", self.frame_rate));
        out.push_str(&format!("show_frame_rate = {}\n", self.show_frame_rate));
        out
    }

    /// Write preferences to `path`, creating the directory if needed.
    ///
    /// Returns the reason on failure so the settings panel can show it. A
    /// failure here must never be fatal: the choice still applies for this
    /// session, it simply will not be remembered.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        // Owner-only. This file is not just a theme choice: it records
        // `vault_dir`, which names an encrypted volume, and `vault_hidden`,
        // which is the answer to whether that volume has a hidden one inside
        // it. Somebody reading this learns that a hidden volume exists, which
        // is the single thing a hidden volume is for concealing, and they
        // learn where to look.
        veilvoice_crypto::privatefile::write_owner_only(path, self.to_text().as_bytes())
            .map_err(|e| e.to_string())
    }
}

/// One of the spellings people write for yes and no, as a `bool`.
///
/// Generous on purpose, because this reads a settings file somebody may have
/// edited by hand, and `on` meaning nothing while `true` works is the kind of
/// thing that reads as the setting being ignored. Anything else answers `None`
/// rather than a default: a value that was meant and not understood is worth
/// reporting, not guessing at.
fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// Whether movement is allowed, and how much.
///
/// Resolved once per frame from three inputs, in order of authority:
///
/// 1. **The operating system's reduce-motion setting.** Someone who has told
///    their system they do not want movement has answered this already.
/// 2. **`VEILVOICE_NO_ANIMATION`**, so a machine that struggles with animation
///    can be fixed without opening the interface that is struggling.
/// 3. **The preference**, which is on by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    /// Whether anything may move at all.
    pub enabled: bool,
    /// Whether the header mark may animate.
    pub icon: bool,
    /// Whether the system asked for reduced motion, so the UI can say that is
    /// why the toggle is doing nothing rather than appearing broken.
    pub system_reduced: bool,
}

impl Motion {
    /// Resolve for this frame.
    pub fn resolve(prefs: &Prefs, system_reduced_motion: bool) -> Self {
        let env_off = std::env::var_os("VEILVOICE_NO_ANIMATION")
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false);
        let enabled = prefs.animations && !env_off && !system_reduced_motion;
        Self {
            enabled,
            icon: enabled && prefs.animated_icon,
            system_reduced: system_reduced_motion,
        }
    }

    /// A duration scaled by whether motion is allowed.
    ///
    /// Returns zero when it is not, so a caller can pass this straight to an
    /// easing function and get an instant result rather than having to branch
    /// at every call site. That matters: a branch nobody wrote is how a stray
    /// animation survives the toggle.
    pub fn secs(&self, wanted: f32) -> f32 {
        if self.enabled {
            wanted
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_ones() {
        let d = Prefs::default();
        assert_eq!(d.theme, "tokyo-night");
        assert!(d.animations, "animations are on by default, as specified");
        assert!(d.animated_icon, "the animated icon is on by default");
        assert!(
            !d.configured,
            "a fresh install has not answered the first run"
        );
    }

    #[test]
    fn it_round_trips_through_its_text_format() {
        let prefs = Prefs {
            theme: "gruvbox".into(),
            animations: false,
            animated_icon: false,
            configured: true,
            toured_tabs: String::new(),
            acceleration: true,
            hide_install_tab: true,
            always_group: true,
            seal_with_app_lock: false,
            vault_dir: String::new(),
            vault_tool: String::new(),
            vault_hidden: String::new(),
            autolock: true,
            autolock_after: 8 * 3_600,
            autolock_floor: 30,
            autolock_ceiling: 7 * 86_400,
            notify_style: "alert".to_string(),
            failsafe: "warn".to_string(),
            live_monitor: "toolbar".to_string(),
            frame_rate: 144,
            show_frame_rate: true,
            defaults_measured: true,
            recovered_from_corrupt_file: false,
        };
        let back = Prefs::parse(&prefs.to_text());
        assert_eq!(back, prefs);
    }

    #[test]
    fn a_hand_edited_frame_rate_is_clamped_and_zero_is_left_alone() {
        // The file is plain text on purpose and says so, so somebody will
        // edit it. One frame a second and ten thousand are both answers the
        // window must not act on.
        assert_eq!(
            Prefs::parse("frame_rate = 1\n").frame_rate,
            crate::pace::DISPLAY_FLOOR
        );
        assert_eq!(
            Prefs::parse("frame_rate = 10000\n").frame_rate,
            crate::pace::DISPLAY_CEILING
        );
        assert_eq!(
            Prefs::parse("frame_rate = 0\n").frame_rate,
            0,
            "zero is the display"
        );
        assert_eq!(Prefs::parse("frame_rate = 144\n").frame_rate, 144);
        // Nonsense leaves the default in place rather than being stored.
        assert_eq!(
            Prefs::parse("frame_rate = fast\n").frame_rate,
            Prefs::default().frame_rate
        );
    }

    #[test]
    fn every_boolean_spelling_people_actually_type_is_accepted() {
        for on in ["true", "yes", "on", "1", "TRUE", "Yes"] {
            assert!(
                Prefs::parse(&format!("animations = {on}")).animations,
                "{on} should mean on"
            );
        }
        for off in ["false", "no", "off", "0", "FALSE", "Off"] {
            assert!(
                !Prefs::parse(&format!("animations = {off}")).animations,
                "{off} should mean off"
            );
        }
    }

    /// A settings file must never be able to stop the app starting.
    #[test]
    fn hostile_and_broken_files_fall_back_to_the_defaults() {
        for text in [
            "",
            "\0\0\0\0",
            "theme",
            "= = = =",
            "theme = ",
            "theme = ../../etc/passwd",
            "theme = <script>alert(1)</script>",
            "animations = perhaps",
            &"a".repeat(100_000),
            "[section]\nkey: value",
        ] {
            let prefs = Prefs::parse(text);
            // Whatever it said, the result is usable.
            assert!(
                crate::theme::by_id(&prefs.theme).is_some(),
                "parsing {:?} left an unknown theme: {}",
                &text[..text.len().min(30)],
                prefs.theme
            );
        }
    }

    /// A file whose every line was rejected should say so, so the settings
    /// panel can explain why the defaults are in force.
    #[test]
    fn a_wholly_unreadable_file_is_reported_rather_than_hidden() {
        assert!(Prefs::parse("nonsense\nmore nonsense").recovered_from_corrupt_file);
        assert!(!Prefs::parse("").recovered_from_corrupt_file);
        assert!(!Prefs::parse("# just a comment").recovered_from_corrupt_file);
        assert!(!Prefs::parse("theme = nord").recovered_from_corrupt_file);
    }

    /// A newer build's keys must not break an older one.
    #[test]
    fn unknown_keys_are_ignored_not_fatal() {
        let prefs = Prefs::parse("theme = nord\nsomething_new = 42\nanimations = false");
        assert_eq!(prefs.theme, "nord");
        assert!(!prefs.animations);
        assert!(!prefs.recovered_from_corrupt_file);
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let prefs = Prefs::load(std::path::Path::new("no-such-settings-file-anywhere.conf"));
        assert_eq!(prefs, Prefs::default());
    }

    /// Roadmap item 170. `load` is named after a cheap thing and has to stay
    /// one: it must not run the probe, because tests and tools read settings
    /// files and none of them asked for a subprocess.
    #[test]
    fn loading_a_file_never_measures_anything() {
        let prefs = Prefs::load(std::path::Path::new("no-such-settings-file-anywhere.conf"));
        assert!(
            !prefs.defaults_measured,
            "`load` took a measurement nobody asked it for"
        );
    }

    /// Roadmap item 170. A first run is the run with no file, and it is the
    /// one that asks the machine.
    #[test]
    fn a_first_run_starts_from_what_the_machine_said() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        let first = Prefs::starting_point(Some(&path));
        assert!(
            first.defaults_measured,
            "a run with no settings file is a first run"
        );
        assert_eq!(
            first.acceleration,
            crate::probe::look().acceleration,
            "the drawing path is the one the machine was asked about"
        );
    }

    /// And the second run is not. Reading the answer back is the whole reason
    /// it is written down.
    #[test]
    fn a_later_run_reads_the_answer_rather_than_taking_it_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        let mut first = Prefs::starting_point(Some(&path));
        // Whatever this machine said, write down the opposite, so reading it
        // back cannot be confused with measuring it again.
        first.acceleration = !crate::probe::look().acceleration;
        first.save(&path).unwrap();

        let second = Prefs::starting_point(Some(&path));
        assert_eq!(
            second.acceleration,
            !crate::probe::look().acceleration,
            "the file's answer was overwritten by a fresh measurement"
        );
        assert!(
            second.defaults_measured,
            "the file records that the answer was measured once"
        );
    }

    /// A machine with nowhere to keep files still gets the measured values for
    /// this session. There is simply nothing to remember them in.
    #[test]
    fn a_machine_with_nowhere_to_save_is_still_measured() {
        let prefs = Prefs::starting_point(None);
        assert!(prefs.defaults_measured);
        assert_eq!(prefs.acceleration, crate::probe::look().acceleration);
    }

    /// A settings file that exists and will not parse is reported as corrupt,
    /// with the defaults in force. Measuring over the top of it would replace
    /// an honest report with a silent reset.
    #[test]
    fn a_corrupt_file_is_reported_rather_than_quietly_remeasured() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        std::fs::write(&path, "nonsense\nmore nonsense\n").unwrap();
        let prefs = Prefs::starting_point(Some(&path));
        assert!(
            prefs.recovered_from_corrupt_file,
            "the panel has to be able to say the defaults are in force"
        );
        assert!(!prefs.defaults_measured);
    }

    /// Roadmap item 170: a better starting point, not a decision taken away.
    /// Every value the probe chooses is still a value somebody can change and
    /// have remembered.
    #[test]
    fn everything_the_probe_chooses_is_still_a_setting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        let mut prefs = Prefs::starting_point(Some(&path));

        for chosen in [true, false] {
            prefs.acceleration = chosen;
            prefs.save(&path).unwrap();
            assert_eq!(
                Prefs::load(&path).acceleration,
                chosen,
                "the drawing path could not be set to {chosen} and kept"
            );
        }
    }

    /// Roadmap item 170. The three values the item says should already be
    /// right, asserted where somebody changing a default will see them fail.
    #[test]
    fn the_starting_point_is_private_and_smooth_without_opening_settings() {
        let d = Prefs::default();
        assert!(d.autolock, "the window locks itself when it is left alone");
        assert!(
            d.seal_with_app_lock,
            "recordings are encrypted at rest once there is a lock to do it with"
        );
        assert_eq!(
            d.frame_rate, 0,
            "zero means the display's own rate, measured rather than assumed"
        );
    }

    /// The system's reduce-motion setting is honoured by resolving it every
    /// frame, not by writing a moment of it into the file. Somebody who turns
    /// it on for an afternoon and off again must not be left with a still
    /// window and a preference they never set.
    #[test]
    fn the_measured_starting_point_does_not_freeze_the_motion_setting() {
        let still = crate::probe::Machine {
            acceleration: false,
            acceleration_reason: "software only".into(),
            graphics_answered: true,
        };
        let prefs = Prefs::measured(&still);
        assert!(
            prefs.animations,
            "the preference stays on and `Motion::resolve` decides per frame"
        );
        assert!(!Motion::resolve(&prefs, true).enabled);
        assert!(Motion::resolve(&prefs, false).enabled);
    }

    #[test]
    fn a_measured_starting_point_takes_the_drawing_path_from_the_machine() {
        for asked in [true, false] {
            let machine = crate::probe::Machine {
                acceleration: asked,
                acceleration_reason: "because".into(),
                graphics_answered: true,
            };
            let prefs = Prefs::measured(&machine);
            assert_eq!(prefs.acceleration, asked);
            assert!(prefs.defaults_measured);
        }
    }

    #[test]
    fn saving_and_loading_round_trips_through_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.conf");
        let prefs = Prefs {
            theme: "dracula".into(),
            animations: false,
            animated_icon: true,
            configured: true,
            toured_tabs: String::new(),
            notify_style: "overlay".into(),
            failsafe: "close".into(),
            live_monitor: "toolbar".into(),
            acceleration: true,
            hide_install_tab: false,
            always_group: false,
            seal_with_app_lock: true,
            vault_dir: "/media/veracrypt1".into(),
            vault_tool: "veracrypt".into(),
            vault_hidden: "none".into(),
            autolock: true,
            autolock_after: 3_600,
            autolock_floor: 60,
            autolock_ceiling: 7 * 86_400,
            frame_rate: 0,
            show_frame_rate: false,
            defaults_measured: true,
            recovered_from_corrupt_file: false,
        };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path), prefs);

        // And readable only by this account. The file records `vault_dir` and
        // `vault_hidden`: where an encrypted volume is, and whether it has a
        // hidden one inside it. The existence of a hidden volume is the single
        // thing a hidden volume conceals, and this was written 0644.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(
                mode, 0o600,
                "settings are {mode:o}, so anyone here can read where the vault is"
            );
        }
    }

    /// The system's reduce-motion setting outranks the preference. Someone who
    /// asked their OS for less movement has already answered.
    #[test]
    fn the_system_setting_wins_over_the_preference() {
        let on = Prefs {
            animations: true,
            animated_icon: true,
            ..Default::default()
        };
        let motion = Motion::resolve(&on, true);
        assert!(!motion.enabled, "the system asked for reduced motion");
        assert!(!motion.icon);
        assert!(motion.system_reduced, "the UI must be able to say why");
        assert_eq!(motion.secs(0.4), 0.0);

        let motion = Motion::resolve(&on, false);
        assert!(motion.enabled);
        assert!(motion.icon);
        assert_eq!(motion.secs(0.4), 0.4);
    }

    #[test]
    fn the_icon_can_be_stilled_without_stilling_everything_else() {
        let prefs = Prefs {
            animations: true,
            animated_icon: false,
            ..Default::default()
        };
        let motion = Motion::resolve(&prefs, false);
        assert!(motion.enabled, "other animation is unaffected");
        assert!(!motion.icon);
    }

    #[test]
    fn the_settings_file_sits_beside_the_app_lock() {
        if let (Some(prefs), Some(lock)) = (default_path(), veilvoice_crypto::lock::default_path())
        {
            assert_eq!(prefs.parent(), lock.parent());
            assert!(prefs.ends_with("settings.conf"));
        }
    }
}
