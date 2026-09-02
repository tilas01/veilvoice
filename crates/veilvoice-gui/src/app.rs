// SPDX-License-Identifier: GPL-3.0-or-later
//! The VeilVoice desktop application: seven tabs, one window, no menus.
//!
//! One window, seven tabs, no menus and no settings file to hunt for. This file
//! owns the window: the tab strip, the state behind it, and the rules about
//! what the user is allowed to do before they have answered the questions that
//! matter. The tabs themselves live partly here and partly in siblings --
//! [`crate::security`] draws the lock tab and the unlock screen,
//! [`crate::prefs`] draws settings.
//!
//! # The seven tabs, and why these seven
//!
//! | Tab | What it is |
//! |---|---|
//! | **anonymise file** | Process a recording on disk. The default path. |
//! | **live scramble** | Scramble a microphone in real time into a virtual cable. |
//! | **monitor** | Which applications currently hold the microphone and camera. |
//! | **lock** | The app lock, and a plain statement of what it is worth. |
//! | **settings** | Colour scheme, animation, and where those choices are kept. |
//! | **install** | Whether this copy is portable or installed, and the optional companions. |
//! | **about** | Versions, licence, and the honest scope. |
//!
//! There is no "advanced" tab and no hidden pane. Everything the program can
//! do is reachable in one click from the strip, because a privacy tool whose
//! important controls are buried is a privacy tool whose important controls do
//! not get used.
//!
//! # Nothing slow runs on the UI thread
//!
//! [`VeilVoiceApp::start_job`] spawns a worker and hands back an
//! [`std::sync::mpsc`] receiver; [`VeilVoiceApp::poll_job`] drains it with
//! `try_recv` once per frame. The window keeps painting while a job runs.
//!
//! That split is not tidiness. A long recording takes real time to process,
//! and sealing it runs Argon2id at 256 MiB, which is **deliberately** slow --
//! that is the whole point of a memory-hard KDF. Doing either on the UI thread
//! means a frozen window and an operating system offering to kill the
//! application, in the middle of the operation the user cares most about
//! completing.
//!
//! `poll_job` handles all three channel outcomes, including
//! `Disconnected` -- a worker that panicked. The user is told the thread
//! stopped rather than watching a progress state that will never finish.
//!
//! # The at-rest choice is enforced here, not merely offered
//!
//! Recordings are encrypted at rest by default (locked decision 4.10), and a
//! job **cannot start** until the user has answered the modal that appears if
//! they try to turn that off. The rule is asserted by a test in this file
//! rather than left as a property of the layout code, because "the button was
//! disabled" is a claim about pixels and "the job refuses to start" is a claim
//! about behaviour.
//!
//! The worker encodes the WAV **in memory** and seals it before anything is
//! written, so a recording that is going to be encrypted never touches the disk
//! in the clear -- not even briefly, not even in a temporary file that would
//! be deleted afterwards. Deleting a file does not remove its contents from a
//! flash device; not writing it does.
//!
//! # Nothing that talks to the operating system runs on this thread
//!
//! The device monitor is the one that got this wrong and shipped. It was polled
//! straight from `update`, and asking Windows which applications hold the
//! microphone cost about a hundred and ninety subprocesses -- so the window
//! froze for seconds at a time, every two seconds. Both halves are fixed:
//! `veilvoice-watch` now costs two subprocesses, and [`crate::watchfeed`] keeps
//! even that on a thread of its own.
//!
//! The rule this file keeps, and the reason the defect is worth a paragraph:
//! **`update` may read state and paint it, and may start work, and may never
//! wait for any.** A job, a lock operation, a monitor scan and an install all
//! go to a worker and come back through a channel.
//!
//! # The monitor indicator
//!
//! [`VeilVoiceApp::watch_indicator`] shows, in the header, whether anything is
//! holding the microphone or camera right now, and clicking it goes to the
//! monitor tab. It is polled on a timer rather than watched continuously,
//! because the underlying platform code enumerates processes and doing that
//! every frame would cost more than the rest of the window put together.
//!
//! What it reports is bounded by what the platform allows, and
//! `veilvoice_watch::support()` states that bound rather than letting an empty
//! list imply an empty machine. The indicator must never present "we could not
//! see" as "nothing is there".
//!
//! # A policy tightens the controls, and the tightening is not the drawing code
//!
//! [`crate::policy::InForce`] holds whatever `veilvoice policy` fixed on this
//! machine. Fixed controls are drawn disabled with the reason underneath, but
//! that is a courtesy: the values a job actually uses come from
//! [`VeilVoiceApp::posture`], which applies the policy every time it is asked.
//! A policy that held only while a checkbox was drawn would not be a policy,
//! and this file already keeps that rule for the at-rest choice.
//!
//! # Where the honest limits are stated
//!
//! The about tab carries the scope text, and the lock tab carries
//! `veilvoice_crypto::lock::SCOPE`. Neither is decoration: tests fail the build
//! if that wording is softened, because a user who over-trusts the app lock is
//! left worse off than one who never had it. If you are editing text in this
//! file and a test starts failing, it is that rule, and it is working.
//!
//! # In plain words
//!
//! The window itself: the tabs along the top, what each one shows, and the state
//! they all share.
//!
//! One window with tabs, no menus, and no settings file to go hunting for.
//! Everything VeilVoice can do is reachable from something visible.
//!
//! The one rule this file follows without exception is that painting the window
//! never waits for anything. Reading a recording or running the engine takes
//! seconds; if that happened here the window would stop responding, so it is
//! started on another thread and the answer is collected later.

use crate::policy::InForce;
use crate::security::Security;
use crate::setup::Setup;
use crate::theme::palette as p;
use crate::watchfeed::WatchFeed;
use egui::{Color32, RichText};
use std::path::PathBuf;
use std::sync::mpsc;
use veilvoice_audio::devices;
use veilvoice_core::{AccentConfig, DeidConfig};

/// The things VeilVoice does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    /// Process a file on disk.
    File,
    /// Scramble a microphone in real time.
    Live,
    /// Several people in one recording, each with a name and a colour.
    Group,
    /// Who is using the microphone and camera.
    Watch,
    /// The app lock, and what it is worth.
    Security,
    /// Check a download against the signed list of hashes.
    Verify,
    /// Colour scheme, animation, and where those choices are kept.
    Preferences,
    /// Portable or installed, and the optional third-party companions.
    Setup,
    /// Versions, licence and honest scope.
    About,
}

impl Tab {
    /// The name this tab answers to on the command line.
    ///
    /// Lower case and stable. These are what `--tab` accepts and what
    /// `tools/shots/gui.ps1` names each picture after, so changing one renames
    /// a screenshot and breaks a link in the README. They are not the labels
    /// on screen, which are written for a reader and may be reworded.
    pub fn key(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Live => "live",
            Self::Group => "group",
            Self::Watch => "monitor",
            Self::Security => "lock",
            Self::Verify => "verify",
            Self::Preferences => "settings",
            Self::Setup => "install",
            Self::About => "about",
        }
    }

    /// Every tab, in the order the window shows them.
    pub const ALL: &'static [Tab] = &[
        Tab::File,
        Tab::Live,
        Tab::Group,
        Tab::Watch,
        Tab::Security,
        Tab::Verify,
        Tab::Preferences,
        Tab::Setup,
        Tab::About,
    ];

    /// The tab with this name, if it is one.
    pub fn from_key(key: &str) -> Option<Tab> {
        let key = key.trim().to_ascii_lowercase();
        Self::ALL.iter().copied().find(|tab| tab.key() == key)
    }
}

/// Result of a background file job.
enum JobDone {
    Ok {
        output: PathBuf,
        secs: f32,
        speed: f32,
        metadata: Vec<String>,
    },
    Failed(String),
}

/// Application state.
pub struct VeilVoiceApp {
    tab: Tab,
    jetbrains: bool,

    /// Whether frames are being counted, from `VEILVOICE_FRAME_LOG`.
    frame_log: bool,
    /// Frames drawn since the last report.
    frames: u32,
    /// When that report was, in the window's own clock.
    frames_since: f64,

    /// Whether the window has been fitted to the screen yet.
    ///
    /// The fit happens once, on the first frame, because that is the
    /// first moment the monitor size is known. See
    /// [`VeilVoiceApp::fit_to_the_screen`].
    fitted: bool,

    // Shared engine settings.
    intensity: f32,
    neutralise_accent: bool,
    reseed_secs: f32,
    /// The randomised ratchet range for this run, drawn at launch.
    ///
    /// F-73: `DeidConfig::with_random_reseed_range` was documented as "the
    /// front ends call this at launch" and was called by nothing but its own
    /// test, so every shipped copy rolled on the same fixed two-second period.
    /// Drawn once here rather than per render, because it is a property of the
    /// session -- a fresh one for every file would be no worse, but it would
    /// make the value shown beside the slider a lie the moment it was read.
    reseed_range: Option<(f32, f32)>,

    /// The input-file picker, while it is open.
    choosing_input: crate::dialog::Pending,

    /// The safety catch. On by default; see `veilvoice_failsafe`.
    failsafe: veilvoice_failsafe::Guard,
    /// What Failsafe last found, so the panel and the notice agree.
    failsafe_finding: Option<veilvoice_failsafe::Finding>,

    /// What is being shown to the reader right now, if anything.
    ///
    /// One at a time. A stack of cards covering the window is how somebody
    /// dismisses six warnings without reading any of them.
    notice: Option<crate::notify::Notice>,

    // File mode.
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    clean_metadata: bool,
    job: Option<mpsc::Receiver<JobDone>>,
    status: Option<(String, Color32)>,
    last_metadata: Vec<String>,

    // Group mode. Off unless the saved preference says to start it on, and
    // the mode itself is never saved -- see `crate::group`.
    group: crate::group::Group,

    // Live mode.
    inputs: Vec<devices::DeviceInfo>,
    outputs: Vec<devices::DeviceInfo>,
    chosen_input: Option<String>,
    chosen_output: Option<String>,
    session: Option<veilvoice_audio::LiveSession>,
    live_error: Option<String>,
    /// The smoothed levels, updated once a frame from the session.
    ///
    /// One copy, because the live tab and the monitor strip both draw them and
    /// two copies is two bars that disagree by a frame. Whichever of them
    /// somebody happens to be looking at is then the wrong one.
    levels: crate::monitor::Levels,
    /// A rolling average of how long a frame takes, in milliseconds.
    ///
    /// Shown on the About tab. Somebody reporting that the window is slow can
    /// then say how slow, and whoever reads that report can tell a window
    /// drawing at sixty frames a second from one drawing at eight. Describing
    /// an interface is not measuring it, and this is the smallest thing that
    /// turns one into the other.
    frame_ms: f32,
    /// Whether the running session is a preview to this machine's own output
    /// rather than the real thing going to a cable. Shown in the interface,
    /// because a person who thinks they are live and is not, or the other way
    /// round, is the whole problem this pair of buttons exists to prevent.
    previewing: bool,

    // The app lock, and at-rest encryption of what jobs write.
    security: Security,
    /// The integrity record, taken at the first launch and checked at every
    /// one after. See [`crate::integrity`].
    integrity: crate::integrity::Integrity,
    /// Where veiled recordings go, and the encrypted volume that may hold
    /// them. See [`crate::storage`].
    storage: crate::storage::Storage,
    /// Seconds since the window was last touched, for the autolock.
    ///
    /// Marker 92. Counted from egui's own frame time rather than the system
    /// clock, so moving the machine's clock neither brings the lock forward nor
    /// pushes it back.
    idle_secs: f32,

    // Colour scheme and animation. Named `preferences` rather than `settings`
    // because this type already has a `settings` method, which is the engine's
    // intensity and accent controls -- a different thing entirely.
    preferences: crate::settings::Settings,

    // Settings somebody else fixed. Read once; never asks for a passphrase.
    policy: InForce,

    // Portable or installed, and the optional companions. Reads the machine
    // on construction and changes nothing until a button is pressed.
    setup: Setup,

    // Device monitor, on a thread of its own. Never polled from here.
    watch: WatchFeed,

    // The manual update check. Holds no clock: the only path into it is the
    // button on the about tab.
    updates: crate::updates::Updates,

    // Checking a download against the signed hashes. Shares its arithmetic
    // with the portable verifier rather than reimplementing it.
    verify: crate::verify::Verify,
}

/// Pick the output to start on: a virtual cable if the machine has one,
/// because routing there is what lets other applications hear the veiled voice
/// at all; otherwise the system default.
///
/// A free function over a device list rather than a step inside `Default`, so
/// the choice can be tested against every arrangement of devices without
/// touching the machine's audio stack. That matters more than it looks:
/// building the app enumerates devices through `cpal`, and several tests doing
/// that at once on a headless runner is a good way to find out what WASAPI does
/// when there are no endpoints and COM is being initialised from four threads
/// at once. The answer was an access violation.
fn preferred_output(outputs: &[devices::DeviceInfo]) -> Option<String> {
    outputs
        .iter()
        .find(|d| d.is_virtual_cable)
        .or_else(|| outputs.iter().find(|d| d.is_default))
        .map(|d| d.name.clone())
}

/// Pick the input to start on: the system default, else whatever is first.
fn preferred_input(inputs: &[devices::DeviceInfo]) -> Option<String> {
    inputs
        .iter()
        .find(|d| d.is_default)
        .or_else(|| inputs.first())
        .map(|d| d.name.clone())
}

impl VeilVoiceApp {
    /// Frames per second, to stderr, when `VEILVOICE_FRAME_LOG` is set.
    ///
    /// # Why a counter and not a frame time
    ///
    /// The About tab already shows how long a frame took, which answers "is
    /// drawing slow". It does not answer the question people actually have,
    /// which is "why is this using processor time when I am not touching it".
    /// An idle window should draw *no* frames. One drawing sixty a second is
    /// costing a laptop its battery whether each frame is fast or not, and a
    /// frame time cannot tell those apart: the fast-drawing runaway looks
    /// healthiest of all.
    ///
    /// Off unless asked for, printed once a second, and to stderr rather than
    /// into the window, because the person reading it is diagnosing rather
    /// than using.
    fn count_frames(&mut self, ctx: &egui::Context) {
        if !self.frame_log {
            return;
        }
        self.frames += 1;
        let now = ctx.input(|i| i.time);
        if self.frames_since == 0.0 {
            self.frames_since = now;
            return;
        }
        let elapsed = now - self.frames_since;
        if elapsed >= 1.0 {
            eprintln!(
                "veilvoice-gui: {:.1} frames/s on the {} tab ({:.1} ms each)",
                self.frames as f64 / elapsed,
                self.tab.key(),
                self.frame_ms
            );
            self.frames = 0;
            self.frames_since = now;
        }
    }

    /// The application with no devices enumerated.
    ///
    /// `Default` calls this after asking the system what it has. Tests that are
    /// not about device selection use it directly, so the suite touches the
    /// platform's audio stack exactly once instead of once per test.
    fn without_devices() -> Self {
        Self {
            frame_log: std::env::var_os("VEILVOICE_FRAME_LOG").is_some(),
            frames: 0,
            frames_since: 0.0,
            fitted: false,
            tab: Tab::File,
            jetbrains: false,
            intensity: 1.0,
            neutralise_accent: true,
            reseed_secs: 2.0,
            // Drawn from the operating system's random source, once, now.
            reseed_range: DeidConfig::default()
                .with_random_reseed_range()
                .reseed_range_ms,
            notice: None,
            choosing_input: crate::dialog::Pending::new(),
            failsafe: veilvoice_failsafe::Guard::new(),
            failsafe_finding: None,
            input: None,
            output: None,
            clean_metadata: true,
            job: None,
            status: None,
            last_metadata: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            chosen_input: None,
            chosen_output: None,
            session: None,
            live_error: None,
            levels: crate::monitor::Levels::default(),
            frame_ms: 0.0,
            previewing: false,
            security: Security::default(),
            integrity: crate::integrity::Integrity::default(),
            storage: crate::storage::Storage::default(),
            idle_secs: 0.0,
            // Off. `VeilVoiceApp::new` is the only place the saved preference
            // is consulted, so no test and no `Default` can open in group mode
            // because of something on this machine's disk.
            group: crate::group::Group::default(),
            preferences: crate::settings::Settings::default(),
            // No policy here, so `without_devices` and `Default` touch no file
            // that belongs to the user. `VeilVoiceApp::new` loads the real one,
            // exactly as it does for the app lock.
            policy: InForce::none(),
            setup: Setup::new(),
            // Idle here, so `without_devices` and `Default` start no thread
            // and touch no machine. `VeilVoiceApp::new` starts the real one.
            watch: WatchFeed::idle(),
            updates: crate::updates::Updates::default(),
            verify: crate::verify::Verify::default(),
        }
    }
}

impl Default for VeilVoiceApp {
    fn default() -> Self {
        let inputs = devices::list(devices::Direction::Input).unwrap_or_default();
        let outputs = devices::list(devices::Direction::Output).unwrap_or_default();
        Self {
            chosen_input: preferred_input(&inputs),
            chosen_output: preferred_output(&outputs),
            inputs,
            outputs,
            ..Self::without_devices()
        }
    }
}

impl VeilVoiceApp {
    /// Build the app, applying theme and fonts to `ctx`.
    ///
    /// This is where the lock file is read, rather than in `Default`: tests and
    /// anything else constructing the app must not touch the real one.
    /// Which tab to open on, if one was named on the command line.
    ///
    /// `veilvoice-gui --tab verify`. It exists so the screenshot tool can put
    /// the window on a tab without clicking: driving the interface with
    /// synthetic mouse events needs the window in the foreground, Windows
    /// refuses to give a background process the foreground, and the refusal is
    /// reported by a return value that nothing was reading. Every capture then
    /// silently showed whichever tab was already open.
    ///
    /// A deep link into a tab is a reasonable thing for an application to have
    /// on its own account, which is why this is a real argument rather than a
    /// hidden one.
    /// What the integrity record found, drawn under the lock controls.
    ///
    /// **Marker 75.** An associated function rather than a method so it borrows
    /// the state it reads and nothing else: `self.security.tab` already holds a
    /// mutable borrow of the same struct on the line above.
    ///
    /// It says which of the two records was consulted, because a sealed record
    /// and a plain one are worth different amounts and a reader who is not told
    /// which they have will assume the better one.
    fn integrity_panel(ui: &mut egui::Ui, state: &crate::integrity::State) {
        use crate::integrity::State;

        ui.add_space(16.0);
        ui.separator();
        ui.label(RichText::new("Its own files").color(p::blue()).small());

        match state {
            State::Idle => {
                ui.label(RichText::new("not checked this session").color(p::muted()));
            }
            State::Working => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("reading and hashing…").color(p::muted()));
                });
            }
            State::Recorded { sealed } => {
                ui.label(
                    RichText::new(if *sealed {
                        "first record taken, sealed with your app-lock passphrase"
                    } else {
                        "first record taken, written in the clear"
                    })
                    .color(p::green()),
                );
            }
            State::Clean { files, sealed } => {
                ui.label(
                    RichText::new(format!(
                        "{files} file(s) match the {} record",
                        if *sealed { "sealed" } else { "plain" }
                    ))
                    .color(p::green()),
                );
            }
            State::Changed(changes) => {
                ui.label(
                    RichText::new("VeilVoice's own files have changed since the record")
                        .color(p::red())
                        .strong(),
                );
                for change in changes.iter().take(8) {
                    ui.label(RichText::new(change).color(p::fg()).small());
                }
                if changes.len() > 8 {
                    ui.label(
                        RichText::new(format!("and {} more", changes.len() - 8))
                            .color(p::muted())
                            .small(),
                    );
                }
                ui.label(
                    RichText::new(
                        "An update you installed looks exactly like this. So does a file \
                         somebody swapped. This cannot tell the two apart.",
                    )
                    .color(p::muted())
                    .small(),
                );
            }
            State::Failed(why) => {
                ui.label(RichText::new(why).color(p::yellow()));
            }
        }

        if matches!(state, State::Recorded { sealed: false }) {
            ui.label(
                RichText::new(
                    "With no app lock set there is no passphrase to seal this with, so the \
                     record is readable. It catches a file that changed by accident. It \
                     does not catch one changed by somebody who also rewrote the record.",
                )
                .color(p::muted())
                .small(),
            );
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new(veilvoice_guard::SCOPE)
                .color(p::muted())
                .small(),
        );
    }

    fn tab_from_arguments() -> Option<Tab> {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if let Some(value) = arg.strip_prefix("--tab=") {
                return Tab::from_key(value);
            }
            if arg == "--tab" {
                return Tab::from_key(&args.next()?);
            }
        }
        None
    }

    /// Build the application, ready for its first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let jetbrains = crate::theme::install_fonts(&cc.egui_ctx);

        // The user's own palettes are read **before** preferences are applied,
        // and the order is load-bearing. `Settings::load` selects the theme
        // named in the preferences file; if that names a custom palette and the
        // table does not hold it yet, the lookup fails, the default is kept,
        // and the user's choice is quietly discarded -- on every single launch,
        // with nothing to indicate why.
        let palette_problems = crate::palettes::default_dir()
            .map(|dir| crate::theme::load_custom(&dir))
            .unwrap_or_default();

        // Preferences second: `Settings::load` applies the chosen colour
        // scheme, so the window opens in it rather than flashing the default
        // for a frame and then switching.
        let mut preferences = crate::settings::Settings::load(&cc.egui_ctx);
        preferences.palette_problems = palette_problems;
        crate::theme::install(&cc.egui_ctx);

        let policy = InForce::load();
        // The one place "always start in group mode" is read. The mode itself
        // is never persisted -- see `crate::group` for why two controls exist
        // where one would look like enough.
        let group = crate::group::Group::start_from(preferences.prefs.always_group);
        let mut app = Self {
            jetbrains,
            security: Security::load(),
            group,
            preferences,
            policy,
            // The one place the monitor thread is started. Everything else
            // constructs an idle feed, so no test and no `Default` reaches the
            // machine.
            watch: WatchFeed::start(cc.egui_ctx.clone()),
            ..Default::default()
        };
        // Marker 86. Before the first frame, and so before anything can be
        // unlocked: `Security` captures the app-lock passphrase as the lock
        // opens, and only when this mode is already the chosen one.
        let seal_with_app_lock = app.preferences.seal_with_app_lock();
        app.security.prefer_app_lock_sealing(seal_with_app_lock);

        // Markers 82 to 84. The remembered destination, and one look at what is
        // mounted. Both at startup rather than per frame: `refresh` reads the
        // mount table, and the draw path reads no files.
        app.storage.destination = app.preferences.destination();
        app.storage.refresh();

        // The integrity record, started before anything is drawn and finished
        // on its own thread. With an app lock set this run is skipped: the
        // record is sealed under the app-lock passphrase, so it can only be
        // read once that passphrase exists, and `poll` starts it again the
        // moment the window unlocks.
        if !app.security.has_lock() {
            app.integrity.start(None);
        }
        app.apply_policy();
        // After the policy, so a named tab is what the window opens on rather
        // than something the policy pass happened to leave selected.
        if let Some(tab) = Self::tab_from_arguments() {
            app.tab = tab;
        }
        app
    }

    /// Bring the controls into line with the policy, once, at startup.
    ///
    /// Not the enforcement -- [`VeilVoiceApp::posture`] is. This is so the
    /// interface *opens* showing the values a job would use, rather than
    /// showing something looser that silently changes when the job runs.
    fn apply_policy(&mut self) {
        let constrained = self.posture();
        self.intensity = constrained.intensity;
        self.neutralise_accent = constrained.neutralise_accent;
        self.clean_metadata = constrained.clean_metadata;
        self.security.encryption_pinned = self
            .policy
            .requires(&veilvoice_policy::Requirement::EncryptRecordings);
        self.security.lock_required = self
            .policy
            .requires(&veilvoice_policy::Requirement::AppLock);
        if self.security.encryption_pinned {
            self.security.encrypt_recordings = true;
        }
    }

    /// The settings as they will actually be used, after the policy.
    ///
    /// Everything that runs a job reads this rather than the fields directly.
    /// The policy can only tighten it, so the worst this can do is process a
    /// recording more thoroughly than the sliders show -- which is the right
    /// direction for the one mistake that is unrecoverable.
    fn posture(&self) -> veilvoice_policy::Posture {
        self.policy.constrain(veilvoice_policy::Posture {
            encrypt_recordings: self.security.encrypt_recordings,
            clean_metadata: self.clean_metadata,
            neutralise_accent: self.neutralise_accent,
            app_lock: self.security.has_lock(),
            intensity: self.intensity,
        })
    }

    fn config(&self) -> DeidConfig {
        let posture = self.posture();
        DeidConfig {
            intensity: posture.intensity,
            accent: AccentConfig {
                enabled: posture.neutralise_accent,
                ..AccentConfig::default()
            },
            reseed_secs: self.reseed_secs,
            reseed_range_ms: self.reseed_range,
            ..DeidConfig::default()
        }
    }
}

impl VeilVoiceApp {
    /// Open at a size this screen can actually show, once, on the first frame.
    ///
    /// The size the window is *created* with has to be chosen before there is
    /// a window, and therefore before anything knows how big the screen is.
    /// So it is created at the preferred size and corrected here, on the first
    /// frame, when egui can say what the monitor is.
    ///
    /// Once only. Re-fitting on every frame would undo a resize the moment
    /// somebody made one, which is a window that fights its user; and because
    /// a resize causes a frame, it would also be a loop.
    ///
    /// Skipped entirely when `--size` was given: that is somebody, or the
    /// screenshot harness, saying exactly what they want.
    fn fit_to_the_screen(&mut self, ctx: &egui::Context) {
        if self.fitted {
            return;
        }
        self.fitted = true;
        if crate::window::requested_size().is_some() {
            return;
        }
        let monitor = ctx.input(|i| i.viewport().monitor_size);
        let Some(monitor) = monitor else {
            return;
        };
        let want = crate::window::opening_size(Some([monitor.x, monitor.y]), None);
        let now = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
        // Only when it actually differs, and by enough to be a real
        // difference rather than a rounding one. Sending the command
        // unconditionally would make the window flicker on every launch.
        if now.is_none_or(|size| (size.x - want[0]).abs() > 1.0 || (size.y - want[1]).abs() > 1.0) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                want[0], want[1],
            )));
        }
    }
}

impl eframe::App for VeilVoiceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.fit_to_the_screen(ctx);
        self.count_frames(ctx);

        // How long the last frame took, smoothed. `stable_dt` rather than `dt`
        // because the raw one spikes whenever the window has been idle and the
        // spike says nothing about how fast the drawing is.
        let dt = ctx.input(|i| i.stable_dt) * 1000.0;
        if dt.is_finite() && dt > 0.0 {
            self.frame_ms = if self.frame_ms == 0.0 {
                dt
            } else {
                self.frame_ms * 0.9 + dt * 0.1
            };
        }

        // Marker 92. Any input at all is use; the passage of a job is not.
        // Somebody who starts a long render and walks away has walked away, and
        // what they are producing is the thing worth locking away.
        let (touched, dt) = ctx.input(|i| {
            let touched = !i.events.is_empty()
                || i.pointer.velocity() != egui::Vec2::ZERO
                || i.raw_scroll_delta != egui::Vec2::ZERO;
            (touched, i.stable_dt)
        });
        if touched || self.security.is_locked() {
            self.idle_secs = 0.0;
        } else if dt.is_finite() {
            self.idle_secs += dt;
        }
        let autolock = self.preferences.autolock();
        if autolock.expired(std::time::Duration::from_secs_f32(self.idle_secs.max(0.0))) {
            self.security.lock_now();
            self.idle_secs = 0.0;
        }

        self.poll_job();

        // The integrity record. Two things happen here and both are cheap: a
        // finished check is collected, and a just-completed unlock hands over
        // the passphrase the sealed record needs. Neither touches the disk on
        // this thread.
        if self.integrity.poll() {
            ctx.request_repaint();
        }
        if let Some(passphrase) = self.security.take_unlock_passphrase() {
            self.integrity.start(Some(passphrase));
        }
        // Marker 86, kept in step. `set_` is a no-op when nothing changed, so
        // this costs a comparison per frame and never a write.
        self.preferences
            .set_seal_with_app_lock(self.security.seals_with_app_lock());

        // Before anything is drawn, and it has to be: this was at the *bottom*
        // of `update`, after the panel that shows the result had already been
        // painted, so a dropped file and the highlight under a hovering one
        // were both a frame late. The comment there said "before anything is
        // drawn", which is how a wrong thing survives a reading -- it agreed
        // with itself.
        //
        // Read whatever tab is open. A file dropped on the window is meant for
        // the verify tab wherever the reader happens to be; telling them
        // nothing because the wrong tab was open would be the interface
        // refusing to do the obvious thing.
        self.verify.take_dropped(ctx);

        // The group panel renders with the engine settings the rest of the
        // application is set to, rather than with the defaults. Copied here,
        // before anything is painted, so the limit it shows and the render it
        // starts are both computed from the same thing (F-67).
        self.group.config = self.config();

        // The gate comes before everything: while locked, no device list, no
        // file names and no live session are reachable or even drawn.
        if self.security.is_locked() {
            self.session = None;
            // The motion preference is resolved here rather than inside the
            // screen: the lock is drawn before the rest of the window exists,
            // and the setting belongs to the application rather than to the
            // lock.
            let motion = self.preferences.motion(ctx);
            egui::CentralPanel::default().show(ctx, |ui| self.security.unlock_screen(ui, motion));
            // The rate limit counts down whether or not anything else moves.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            return;
        }

        let dialogue_open = self.security.disable_dialogue(ctx);

        // The application bar: a band of its own colour across the top, with
        // room to breathe and rounded lower corners so it reads as a surface
        // the content sits under rather than a line somebody drew.
        //
        // Deliberately still a Windows window. The system's own title bar, its
        // buttons and its behaviour are all left alone: an application that
        // draws its own title bar has to reimplement dragging, snapping,
        // maximising and the accessibility that comes with them, and gets some
        // of it subtly wrong on somebody else's machine. This is the band below
        // that, which is ours to make pleasant.
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::none()
                    .fill(p::bg_dark())
                    .inner_margin(egui::Margin {
                        left: 18.0,
                        right: 18.0,
                        top: 12.0,
                        bottom: 10.0,
                    })
                    .rounding(egui::Rounding {
                        nw: 0.0,
                        ne: 0.0,
                        sw: 10.0,
                        se: 10.0,
                    }),
            )
            .show(ctx, |ui| {
                let motion = self.preferences.motion(ctx);
                let time = ui.input(|i| i.time) as f32;
                ui.horizontal(|ui| {
                    // The mark, animated as on the website unless it has been
                    // stilled. `draw` requests no repaint when it is still, so the
                    // toggle saves the work as well as the movement.
                    crate::soundbar::draw(ui, egui::vec2(46.0, 22.0), motion, time)
                        .on_hover_text("VeilVoice");
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("VeilVoice")
                            .size(21.0)
                            .color(p::fg())
                            .strong(),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .color(p::muted())
                            .small(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("offline").color(p::green()).small());
                        // **Marker 78.** The colour scheme, in the header.
                        //
                        // Every one of the website's themes has been in this
                        // application since marker 26, and the picker was on a
                        // page inside Settings, which is a place somebody looks
                        // only if they already believe there is something to
                        // find. The website puts its picker in the header on
                        // every page; so does this now, and Settings keeps the
                        // fuller panel with the swatches and the custom
                        // palettes.
                        self.preferences.theme_picker(ui, ctx);
                        if self.security.has_lock()
                            && ui
                                .button(RichText::new("lock").color(p::yellow()).small())
                                .on_hover_text("Lock the app and clear the session passphrase")
                                .clicked()
                        {
                            self.security.lock_now();
                        }
                        // A monitor you have to go looking for is not doing its
                        // job, so the warning rides the header on every tab.
                        self.watch_indicator(ui);
                    });
                });
                ui.add_space(10.0);
                // The install tab is not always offered. An installed copy never
                // shows it -- a program offering to install itself when it already
                // is tells the user something untrue about what they are running --
                // and a portable copy shows it unless the reader has ticked it away
                // under settings, interface.
                let offer_install = self
                    .preferences
                    .show_install_tab(self.setup.running_installed());
                // A tab that is not shown must not stay selected, or the window
                // keeps drawing a panel with nothing to reach it by. Sent back to
                // the first tab, which is where the app opens anyway.
                if !offer_install && self.tab == Tab::Setup {
                    self.tab = Tab::File;
                }
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (Tab::File, "Anonymise file"),
                        (Tab::Live, "Live scramble"),
                        (Tab::Group, "Group"),
                        (Tab::Watch, "Monitor"),
                        (Tab::Security, "Lock"),
                        (Tab::Verify, "Verify"),
                        (Tab::Preferences, "Settings"),
                        (Tab::Setup, "Install"),
                        (Tab::About, "About"),
                    ] {
                        if tab == Tab::Setup && !offer_install {
                            continue;
                        }
                        let selected = self.tab == tab;
                        let text = RichText::new(label).color(if selected {
                            p::blue()
                        } else {
                            p::muted()
                        });
                        if ui.selectable_label(selected, text).clicked() {
                            self.tab = tab;
                        }
                        // A real gap between tabs, not just the default padding.
                        //
                        // It reads better, and it is load-bearing for
                        // `tools/shots/gui.ps1`, which finds the tabs by scanning
                        // the strip for lit columns separated by gaps. Capitalising
                        // the labels widened them enough to close the space between
                        // the first two, and the scanner read "Anonymise file Live
                        // scramble" as one label and refused to continue -- which
                        // is the failure working as intended, and the fix is to
                        // give it something unambiguous to see.
                        ui.add_space(6.0);
                    }
                });
            });

        // Resolved once above for the header mark, and read again here so
        // the setup tab's progress strip obeys the same answer rather than
        // asking the question a second time in the same frame.
        let motion = self.preferences.motion(ctx);

        // The levels, once a frame, before anything draws them.
        //
        // Here rather than in the live tab, because the monitor strip below is
        // drawn on every tab and the live tab is drawn on one. Reading the
        // session from whichever happened to run would have made the strip
        // freeze the moment somebody navigated away, which is the exact moment
        // this feature exists for.
        if let Some(session) = &self.session {
            let stats = session.stats();
            self.levels.update(stats.input_peak, stats.output_peak);
        }

        // The live monitor, on every tab and above the panel. Docked by
        // default; a floating card or nothing if the reader has said so.
        //
        // Drawn before the central panel so the panel is laid out inside what
        // is left, rather than under a strip that arrives after it.
        if crate::monitor::show(
            ctx,
            self.preferences.live_monitor(),
            self.session.is_some(),
            self.previewing,
            &self.levels,
        ) == crate::monitor::Action::Dismiss
        {
            self.preferences
                .set_live_monitor(crate::monitor::Style::Off);
            self.notice = Some(crate::notify::Notice::note(
                "The live monitor is off. Settings brings it back, and the live                  tab still has the full meters.",
            ));
        }

        // Above the panel content and below the tab strip, so it is seen
        // whatever tab is open. Drawn before the tab body rather than after,
        // for the same reason F-61 moved the dropped-file read to the top:
        // painting a notice under the thing it is about is a notice a frame
        // late and half a window away.
        if let Some(notice) = self.notice.clone() {
            let style = self.preferences.notify_style();
            egui::TopBottomPanel::top("notice").show(ctx, |ui| {
                ui.add_space(6.0);
                if crate::notify::show(ui, style, &notice) {
                    self.notice = None;
                }
                ui.add_space(6.0);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // While the "unencrypted?" question is open, clicks must not land
            // on the window behind it.
            ui.add_enabled_ui(!dialogue_open, |ui| {
                // Offered once, and only once: two choices with sensible
                // defaults, so it is a courtesy rather than a gate.
                if self.preferences.needs_first_run() {
                    self.preferences.first_run_panel(ui);
                    return;
                }
                // Every tab, inside one scroller.
                //
                // This is what "nothing is ever out of reach" actually
                // requires. A window can be any size the person makes it, and
                // several of these panels are taller than a small one: the
                // security tab had no scroller at all, so on a short window the
                // controls below the fold could not be reached by any means --
                // not scrolled to, not tabbed to, not resized into view without
                // making the window taller than the screen.
                //
                // Here rather than in each tab so that a tab added later gets
                // it without anybody remembering to, and so there is exactly
                // one of them: a scroller inside a scroller traps the wheel in
                // whichever the pointer happens to be over.
                //
                // `auto_shrink([false, false])` so a short panel still fills
                // the window rather than collapsing the layout around itself.
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::File => self.file_tab(ui),
                        Tab::Live => self.live_tab(ui),
                        Tab::Group => self.group.tab(ui, &mut self.preferences),
                        Tab::Watch => self.watch_tab(ui),
                        Tab::Security => {
                            self.security.tab(ui);
                            Self::integrity_panel(ui, self.integrity.state());
                            // Markers 82 to 84. Returns true when the choice
                            // changed, which is when it is worth a write to
                            // the settings file rather than every frame.
                            if crate::storage::panel(&mut self.storage, ui) {
                                self.preferences.set_destination(&self.storage.destination);
                            }
                        }
                        Tab::Verify => self.verify.tab(ui),
                        Tab::Preferences => self.preferences.tab(ui, ctx),
                        Tab::Setup => self.setup.tab(ui, motion),
                        Tab::About => self.about_tab(ui),
                    });
            });
        });

        self.watch.drain();
        self.check_failsafe();
        self.updates.drain();
        self.group.drain();
        self.verify.drain();

        // **Marker 79.** How often to come back, decided by what is moving.
        //
        // This was one number, 50 ms, for everything: a live session, a
        // download, a file being veiled. Twenty frames a second is fine for a
        // progress line that changes once a second and is not fine for a meter
        // that follows a voice, which is the one thing here that moves
        // continuously. At 20 Hz a meter steps rather than sweeps, and a window
        // whose only moving part is stepping reads as a window that is
        // struggling.
        //
        // So a live session asks for 16 ms and everything else keeps 50. The
        // cost is bounded and it is paid only while somebody is actually being
        // veiled, which is the moment worth spending it on.
        //
        // Not claimed: that this fixes anything somebody has reported. This
        // machine has no display and the frame time is not measurable from
        // here, which is why the About tab now shows it. A number the person
        // with the problem can read is worth more than a change made blind.
        if self.session.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else if self.updates.is_busy()
            // Hovering, not only busy. An idle window requests no repaint, so
            // dragging a file over it lit nothing up and the file did not
            // appear until the mouse moved for some other reason -- the one
            // moment in this application where the user is waiting for the
            // window to react and the window has decided nothing is happening.
            || self.verify.wants_repaint()
            || self.group.is_busy()
            || self.job.is_some()
            || self.security.is_busy()
            || self.integrity.is_busy()
            || self.setup.is_busy()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else if autolock.enabled && !self.security.is_locked() {
            // Marker 92. Once a second is enough to notice a delay measured in
            // minutes, and it is what makes the lock actually engage: an idle
            // window requests no repaint, so without this the countdown would
            // only advance while somebody was looking at it, which is the one
            // time it should not.
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        // Nothing here for the microphone monitor. It runs on its own thread
        // and asks for a repaint when it has something to report, which is the
        // only moment a repaint is worth anything. Waking twice a second to
        // ask whether it had news is what made an untouched window draw 2.1
        // frames a second on every tab, for ever.
    }
}

impl VeilVoiceApp {
    fn poll_job(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(JobDone::Ok {
                output,
                secs,
                speed,
                metadata,
            }) => {
                self.status = Some((
                    format!(
                        "done in {secs:.1}s ({speed:.0}x realtime) → {}",
                        output.display()
                    ),
                    p::green(),
                ));
                self.last_metadata = metadata;
                self.job = None;
            }
            Ok(JobDone::Failed(message)) => {
                self.status = Some((message, p::red()));
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = Some((
                    "the processing thread stopped unexpectedly".into(),
                    p::red(),
                ));
                self.job = None;
            }
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Settings").color(p::blue()).small());

        // A floor becomes the bottom of the slider's range rather than a value
        // the slider snaps back from. A control that visibly refuses to go
        // where it is dragged reads as broken; one whose range starts higher
        // reads as a decision, which is what it is.
        let floor = self.policy.minimum_intensity();
        if self.intensity < floor {
            self.intensity = floor;
        }
        ui.add(
            egui::Slider::new(&mut self.intensity, floor..=1.0)
                .text("intensity")
                .fixed_decimals(2),
        );
        if floor > 0.0 {
            self.policy.note(
                ui,
                &veilvoice_policy::Requirement::MinimumIntensity((floor * 100.0).round() as u8),
            );
        }

        let accent_fixed = self
            .policy
            .requires(&veilvoice_policy::Requirement::NeutraliseAccent);
        if accent_fixed {
            self.neutralise_accent = true;
        }
        ui.add_enabled(
            !accent_fixed,
            egui::Checkbox::new(
                &mut self.neutralise_accent,
                "neutralise accent and intonation",
            ),
        );
        self.policy
            .note(ui, &veilvoice_policy::Requirement::NeutraliseAccent);
        ui.label(
            RichText::new(if self.neutralise_accent {
                "every speaker is mapped onto one canonical register and vocal tract"
            } else {
                "the speaker's accent, intonation and vocal tract are left intact"
            })
            .color(p::muted())
            .small(),
        );

        // The ratchet. Two ways to set it, and the control that is *not* in
        // force is disabled rather than left looking live -- a slider that
        // silently does nothing is how somebody ends up certain they changed a
        // setting they did not.
        let mut randomised = self.reseed_range.is_some();
        if ui
            .checkbox(&mut randomised, "randomise the seed-roll interval")
            .on_hover_text(
                "A fixed interval is a fixed thing to observe. With this on, the gap \
                 before every roll is drawn fresh, so the ratchet has no period.",
            )
            .changed()
        {
            self.reseed_range = if randomised {
                // Drawn again rather than remembered, so turning it off and on
                // is not a way to get the same range back.
                DeidConfig::default()
                    .with_random_reseed_range()
                    .reseed_range_ms
            } else {
                None
            };
        }

        ui.add_enabled_ui(!randomised, |ui| {
            ui.add(
                egui::Slider::new(&mut self.reseed_secs, 0.0..=30.0)
                    .text("seed roll (s)")
                    .fixed_decimals(1),
            );
        });

        // What the engine will actually do, quantised to whole frames. Showing
        // the range as asked for would describe a spread that does not exist:
        // the ratchet can only fire on a frame boundary.
        let effective = self.config().effective_reseed_range_ms();
        ui.label(
            RichText::new(match effective {
                Some((lo, hi)) => format!(
                    "{lo:.0}-{hi:.0} ms, drawn fresh before every roll, so there is no period to observe"
                ),
                None if self.reseed_secs <= 0.0 => {
                    "one modulation stream for the whole session".to_string()
                }
                None => "the modulation stream rolls forward; earlier audio is sealed \
                         off behind it"
                    .to_string(),
            })
            .color(p::muted())
            .small(),
        );
        if effective.is_some() {
            ui.label(
                RichText::new(
                    "drawn from the operating system's random source at launch, so it \
                     is a property of this run rather than a number compiled in",
                )
                .color(p::muted())
                .small(),
            );
        }
    }

    fn file_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("Input").color(p::blue()).small());
        ui.horizontal(|ui| {
            // Started, not waited for. The picker runs on its own thread and
            // the answer is collected below, so the window keeps painting
            // while somebody browses -- see `crate::dialog`.
            if ui
                .add_enabled(
                    !self.choosing_input.is_open(),
                    egui::Button::new("choose file…"),
                )
                .clicked()
            {
                self.choosing_input.start(crate::dialog::Ask::open_filtered(
                    "audio",
                    &["wav", "mp3", "flac", "ogg", "m4a", "aac", "opus"],
                ));
            }
            if let Some(path) = self.choosing_input.taken() {
                let mut out = path.clone();
                out.set_extension("veiled.wav");
                self.input = Some(path);
                self.output = Some(out);
                self.status = None;
            }
            match &self.input {
                Some(path) => ui.label(RichText::new(path.display().to_string()).color(p::cyan())),
                None => ui.label(RichText::new("no file selected").color(p::muted())),
            };
        });

        ui.add_space(8.0);
        self.settings(ui);
        let metadata_fixed = self
            .policy
            .requires(&veilvoice_policy::Requirement::CleanMetadata);
        if metadata_fixed {
            self.clean_metadata = true;
        }
        ui.add_enabled(
            !metadata_fixed,
            egui::Checkbox::new(&mut self.clean_metadata, "strip metadata from the result"),
        );
        self.policy
            .note(ui, &veilvoice_policy::Requirement::CleanMetadata);

        ui.add_space(12.0);
        self.security.recording_controls(ui);

        ui.add_space(12.0);
        let busy = self.job.is_some();
        // Marker 83. A destination whose hidden-volume question is unanswered
        // blocks the job rather than quietly writing beside the source file.
        // The silent fallback is the failure this exists to prevent: a veiled
        // recording sitting outside a vault while its owner believes it is
        // inside one.
        let ready = self.input.is_some()
            && !busy
            && self.security.ready_to_write()
            && self.storage.destination.ready();
        let button = ui.add_enabled(
            ready,
            egui::Button::new(RichText::new("  anonymise  ").strong()),
        );
        if button.clicked() {
            self.start_job();
        }
        if let Some(reason) = self.security.blocked_reason() {
            ui.label(RichText::new(reason).color(p::yellow()).small());
        }
        if let Some(reason) = self.storage.destination.blocked() {
            ui.label(RichText::new(reason).color(p::yellow()).small());
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("processing…").color(p::muted()));
            });
        }

        if let Some((message, colour)) = &self.status {
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(*colour));
        }
        if !self.last_metadata.is_empty() {
            ui.label(
                RichText::new(format!(
                    "metadata removed: {}",
                    self.last_metadata.join(", ")
                ))
                .color(p::muted())
                .small(),
            );
        }

        ui.add_space(16.0);
        ui.separator();
        ui.label(
            RichText::new(
                "The words survive on purpose. A scrambler you cannot understand is \
                 useless. Encrypting the result at rest is what keeps them from being \
                 read off the disk afterwards, which is why it is on by default.",
            )
            .color(p::muted())
            .small(),
        );
    }

    fn start_job(&mut self) {
        let Some(input) = self.input.clone() else {
            return;
        };
        let output = self.output.clone().unwrap_or_else(|| {
            let mut o = input.clone();
            o.set_extension("veiled.wav");
            o
        });
        // Marker 82. The encrypted destination replaces the folder and keeps
        // the name. `place` returns the original untouched when nothing is
        // chosen, and also when the destination is not cleared to be used, so
        // a job that got past the button somehow still cannot write into a
        // volume whose hidden-volume question is unanswered.
        // F-95. The mount table is read here, at the moment of writing, rather
        // than taken from what the panel last saw. A vault locked since it was
        // chosen leaves its mount point behind as an empty directory, and
        // writing into that puts a veiled recording on the ordinary disk while
        // its owner believes it went into the vault. `start_job` runs on a
        // click and spawns a thread, so this is not the draw path.
        let mounts = veilvoice_setup::volumes::mounted();
        let placed = self.storage.destination.place(&output, &mounts);
        if self.storage.destination.volume.is_some() && placed == output {
            self.status = Some((
                "that encrypted folder is not open now, so nothing was written. \
                 Unlock it in its own program, or choose the ordinary folder again."
                    .to_string(),
                p::red(),
            ));
            return;
        }
        let output = placed;
        let config = self.config();
        let clean = self.posture().clean_metadata;
        let plan = self.security.plan();
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.status = None;
        self.last_metadata.clear();

        // Off the UI thread: a long file would otherwise freeze the window, and
        // Argon2id at 256 MiB is deliberately slow on top of that.
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let result = (|| -> Result<(PathBuf, f32, Vec<String>), String> {
                let audio = veilvoice_audio::io::load(&input).map_err(|e| e.to_string())?;
                let veiled =
                    veilvoice_audio::deidentify(&audio, config).map_err(|e| e.to_string())?;

                // Encoded in memory, so a recording that is going to be sealed
                // never lands on the disk in the clear even briefly.
                let mut wav = veilvoice_audio::io::wav_bytes(&veiled).map_err(|e| e.to_string())?;
                let mut removed = Vec::new();
                if clean {
                    if let Ok((cleaned, report)) =
                        veilvoice_meta::clean_wav_bytes(&wav, veilvoice_meta::Policy::Strip)
                    {
                        wav = cleaned;
                        removed = report.removed;
                    }
                }
                let written =
                    plan.write(&output, &wav, veilvoice_crypto::kdf::KdfParams::default())?;
                Ok((written, audio.duration_secs(), removed))
            })();

            let secs = started.elapsed().as_secs_f32();
            let _ = tx.send(match result {
                Ok((output, duration, metadata)) => JobDone::Ok {
                    output,
                    secs,
                    speed: duration / secs.max(1e-6),
                    metadata,
                },
                Err(message) => JobDone::Failed(message),
            });
        });
    }

    fn live_tab(&mut self, ui: &mut egui::Ui) {
        let running = self.session.is_some();

        ui.add_space(4.0);
        ui.label(RichText::new("Devices").color(p::blue()).small());
        ui.add_enabled_ui(!running, |ui| {
            device_picker(ui, "input ", &self.inputs, &mut self.chosen_input);
            device_picker(ui, "output", &self.outputs, &mut self.chosen_output);
        });

        let routed = self
            .chosen_output
            .as_ref()
            .and_then(|name| self.outputs.iter().find(|d| &d.name == name))
            .map(|d| d.is_virtual_cable)
            .unwrap_or(false);
        if !routed {
            ui.label(
                RichText::new(
                    "no virtual cable selected, so other applications will not receive this",
                )
                .color(p::yellow())
                .small(),
            );
        }

        ui.add_space(8.0);
        ui.add_enabled_ui(!running, |ui| self.settings(ui));

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if !running {
                if ui.button(RichText::new("  start  ").strong()).clicked() {
                    self.start_live();
                }
            } else if ui.button(RichText::new("  stop  ").strong()).clicked() {
                self.session = None;
                self.levels.clear();
                self.previewing = false;
            }
            // Listening to yourself before anybody else does.
            //
            // Same session, one thing different: the veiled voice goes to this
            // machine's own output rather than to a virtual cable, so it
            // reaches your headphones and nothing else. It is the only check
            // that answers the question the meters cannot, which is whether
            // the voice coming out is a voice that is not yours.
            if !running
                && ui
                    .button("  preview to my headphones  ")
                    .on_hover_text(
                        "Hear yourself veiled. The output goes to this machine's \
                         speakers or headphones and to nothing else, so nobody on a \
                         call hears it. Use headphones: speakers plus a microphone \
                         is a feedback loop.",
                    )
                    .clicked()
            {
                self.start_live_preview();
            }
            if running {
                ui.label(
                    RichText::new(if self.previewing {
                        "● preview"
                    } else {
                        "● live"
                    })
                    .color(if self.previewing {
                        p::yellow()
                    } else {
                        p::green()
                    }),
                );
            }
        });

        if let Some(message) = &self.live_error {
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(p::red()));
        }

        if let Some(session) = &self.session {
            let stats = session.stats();
            // The smoothing happens once a frame in `update`, so the strip and
            // this panel are the same numbers rather than two readings taken a
            // frame apart.

            ui.add_space(12.0);
            ui.label(RichText::new("Levels").color(p::blue()).small());
            meter(ui, "in ", self.levels.input, self.levels.hold_input);
            meter(ui, "out", self.levels.output, self.levels.hold_output);
            ui.label(
                RichText::new(
                    "  These say sound is arriving and sound is leaving. They cannot say                      the voice has been changed: a working meter and a bypassed engine                      draw the same bar. Listen to the output to hear that.",
                )
                .small()
                .color(p::muted()),
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Performance").color(p::blue()).small());
            field(
                ui,
                "processing",
                &format!("{:.2} ms/block", stats.process.ema_block_ms()),
            );
            field(
                ui,
                "engine latency",
                &format!("{:.1} ms", stats.process.algorithmic_latency_ms),
            );
            field(
                ui,
                "realtime factor",
                &format!("{:.3}", stats.process.last_realtime_factor()),
            );
            if stats.dropped > 0 || stats.starved > 0 {
                ui.label(
                    RichText::new(format!(
                        "glitches: {} dropped, {} starved",
                        stats.dropped, stats.starved
                    ))
                    .color(p::yellow())
                    .small(),
                );
            }
        }
    }

    fn start_live(&mut self) {
        self.previewing = false;
        self.live_error = None;
        let result = (|| {
            let input = devices::open(devices::Direction::Input, self.chosen_input.as_deref())?;
            let output = devices::open(devices::Direction::Output, self.chosen_output.as_deref())?;
            veilvoice_audio::LiveSession::start(&input, &output, self.config())
        })();
        match result {
            Ok(session) => self.session = Some(session),
            Err(e) => self.live_error = Some(e.to_string()),
        }
    }

    /// The same session, pointed at this machine's own output.
    ///
    /// The chosen output is deliberately ignored: a preview that went to the
    /// virtual cable would be heard by whatever is listening on it, which is
    /// the one place somebody checking their setup does not want it to go.
    /// `None` asks the audio layer for the default device.
    fn start_live_preview(&mut self) {
        self.live_error = None;
        let result = (|| {
            let input = devices::open(devices::Direction::Input, self.chosen_input.as_deref())?;
            let output = devices::open(devices::Direction::Output, None)?;
            veilvoice_audio::LiveSession::start(&input, &output, self.config())
        })();
        match result {
            Ok(session) => {
                self.session = Some(session);
                self.previewing = true;
                // **F-84.** The claim is checked rather than asserted.
                //
                // A preview goes to the default output, and on a machine whose
                // default output *is* a virtual cable, whatever is listening on
                // that cable hears it. Telling somebody the opposite in the one
                // place they are checking their setup is worse than telling
                // them nothing, because checking is what they came to do.
                let cable = devices::find_virtual_cable().map(|d| d.name);
                let default = devices::open(devices::Direction::Output, None)
                    .ok()
                    .map(|d| devices::name_of(&d));
                let into_cable = cable.is_some() && cable == default;
                self.notice = Some(if into_cable {
                    crate::notify::Notice::warn(
                        "Preview, but this machine's default output is a virtual cable, \
                         so whatever is listening on it hears this too.",
                    )
                } else {
                    crate::notify::Notice::note(
                        "Preview: the veiled voice is going to this machine's own output \
                         and nowhere else. Listen for a voice that is not yours.",
                    )
                });
            }
            Err(e) => self.live_error = Some(e.to_string()),
        }
    }

    /// Ask the safety catch what it makes of what is holding a microphone.
    ///
    /// Called once a frame, straight after the watch feed is drained, because
    /// that is where the information arrives. The decision is arithmetic over a
    /// list -- see `veilvoice_failsafe` -- so doing it every frame costs
    /// nothing and means the answer is never a frame out of date.
    ///
    /// **Closing a program is done here and nowhere else**, and only when the
    /// guard has said it may be.
    fn check_failsafe(&mut self) {
        self.failsafe.posture = self.preferences.failsafe();
        if !self.failsafe.posture.is_on() {
            self.failsafe_finding = None;
            return;
        }

        // What VeilVoice is itself veiling, so a program on our own cable is
        // not mistaken for the accident.
        self.failsafe.live = self.session.is_some();
        self.failsafe.veiling = self.chosen_output.clone();

        let holders: Vec<veilvoice_failsafe::Holder> = self
            .watch
            .active()
            .iter()
            .filter(|use_| use_.kind == veilvoice_watch::DeviceKind::Microphone)
            .map(|use_| veilvoice_failsafe::Holder {
                app: use_.app.clone(),
                pid: use_.pid,
                device: use_.device.clone(),
            })
            .collect();
        let problems: Vec<String> = self
            .watch
            .error()
            .map(|e| e.to_string())
            .into_iter()
            .collect();

        let finding = self.failsafe.look(&holders, &problems);

        // Only act on a *change*, or the same program is closed and reported
        // sixty times a second for as long as it takes to die.
        let fresh = self.failsafe_finding.as_ref() != Some(&finding);
        if fresh {
            if let veilvoice_failsafe::Finding::Foreign {
                app,
                pid,
                closeable,
                ..
            } = &finding
            {
                let words = finding.phrasing();
                self.notice = Some(crate::notify::Notice::warn(words.clone()));
                if *closeable {
                    match veilvoice_failsafe::act::close(app, *pid) {
                        Ok(done) => {
                            self.failsafe
                                .record(std::time::SystemTime::now(), app, true, &done)
                        }
                        Err(why) => {
                            self.failsafe
                                .record(std::time::SystemTime::now(), app, false, &why);
                            // Said, not swallowed. A guard that tried and could
                            // not is a different situation from one that did.
                            self.notice = Some(crate::notify::Notice::warn(format!(
                                "{words} It could not be closed: {why}"
                            )));
                        }
                    }
                } else {
                    self.failsafe.record(
                        std::time::SystemTime::now(),
                        app,
                        false,
                        "left alone: protected, or the posture is warn-only",
                    );
                }
            }
        }
        self.failsafe_finding = Some(finding);
    }

    /// Re-scan on a timer rather than every frame.
    /// The always-visible indicator.
    fn watch_indicator(&mut self, ui: &mut egui::Ui) {
        let support = self.watch.support();
        if !(support.microphone || support.camera) {
            return;
        }
        let active = self.watch.active();
        if active.is_empty() {
            return;
        }

        let camera = active
            .iter()
            .any(|u| u.kind == veilvoice_watch::DeviceKind::Camera);
        let colour = if camera { p::red() } else { p::yellow() };
        let names: Vec<&str> = active.iter().map(|u| u.app.as_str()).collect();
        let label = format!(
            "* {} IN USE - {}",
            if camera { "CAMERA" } else { "MIC" },
            names.join(", ")
        );

        if ui
            .label(RichText::new(label).color(colour).small().strong())
            .on_hover_text("Open the monitor tab for detail")
            .clicked()
        {
            self.tab = Tab::Watch;
        }
    }

    fn watch_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("What is listening").color(p::blue()).small());
        let support = self.watch.support();
        ui.label(RichText::new(support.explanation).color(p::muted()).small());

        // An empty list from a platform that cannot see is not good news, and
        // must never be allowed to read like it.
        if !(support.microphone || support.camera) {
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "This platform exposes no way to tell which application is using \
                     the microphone or camera, so nothing is shown. That is not the \
                     same as nothing being active.",
                )
                .color(p::yellow()),
            );
            return;
        }

        if let Some(problem) = self.watch.error() {
            ui.label(RichText::new(problem).color(p::red()));
        }

        ui.add_space(10.0);
        let active: Vec<_> = self.watch.active().to_vec();
        if active.is_empty() {
            ui.label(RichText::new("Nothing is using the microphone or camera.").color(p::green()));
        } else {
            for entry in &active {
                let colour = if entry.kind == veilvoice_watch::DeviceKind::Camera {
                    p::red()
                } else {
                    p::yellow()
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("*").color(colour));
                    ui.label(RichText::new(entry.describe()).color(p::fg()).strong());
                    ui.label(RichText::new(entry.kind.to_string()).color(colour).small());
                });
                if let Some(path) = &entry.path {
                    ui.label(
                        RichText::new(format!("    {path}"))
                            .color(p::muted())
                            .small(),
                    );
                }
                if let Some(held) = entry.held_for() {
                    ui.label(
                        RichText::new(format!("    held for {}s", held.as_secs()))
                            .color(p::muted())
                            .small(),
                    );
                }
                ui.add_space(6.0);
            }
        }

        if !self.watch.log().is_empty() {
            ui.add_space(14.0);
            ui.separator();
            ui.label(RichText::new("Recent").color(p::blue()).small());
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for line in self.watch.log().iter().rev() {
                        ui.label(RichText::new(line).color(p::muted()).small());
                    }
                });
        }
    }

    /// Say so if the last run ended badly, and offer the file.
    ///
    /// A report written to disk that nobody is told about is a report nobody
    /// reads. The crash log exists because this application had no way at all
    /// to explain a failure -- no console, and an abort on panic -- and leaving
    /// its output for the user to stumble across would only half fix that.
    ///
    /// Shown in the about tab rather than as a modal on launch: the previous
    /// run failing is worth knowing and is not worth a dialog in front of
    /// somebody who has just successfully opened the application.
    fn previous_crash(&mut self, ui: &mut egui::Ui) {
        let Some((path, _)) = crate::crashlog::previous() else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("The previous run ended unexpectedly.")
                    .color(p::yellow())
                    .strong(),
            );
        });
        ui.label(
            RichText::new(format!(
                "A report was written to {}. It was written on this machine and sent \n                 nowhere. VeilVoice has no network code at all.",
                path.display()
            ))
            .color(p::muted())
            .size(12.0),
        );
        if ui.button("dismiss this notice").clicked() {
            crate::crashlog::clear();
        }
        ui.add_space(10.0);
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        self.previous_crash(ui);
        field(ui, "app", env!("CARGO_PKG_VERSION"));
        field(ui, "engine", veilvoice_core::VERSION);
        field(ui, "audio", veilvoice_audio::VERSION);
        field(ui, "metadata", veilvoice_meta::VERSION);
        field(ui, "monitor", veilvoice_watch::VERSION);
        field(ui, "crypto", veilvoice_crypto::VERSION);
        field(ui, "licence", "GPL-3.0-or-later");
        // Measured, not described. Somebody reporting that this window is slow
        // can now say how slow, and 16 ms and 120 ms are different problems
        // with different causes.
        field(
            ui,
            "frame time",
            &if self.frame_ms > 0.0 {
                format!(
                    "{:.1} ms ({:.0} a second)",
                    self.frame_ms,
                    1000.0 / self.frame_ms.max(0.1)
                )
            } else {
                "not measured yet".to_string()
            },
        );
        // Precise rather than short. "None" stopped being true the moment the
        // update button existed, and a version string that overstates the thing
        // it is printed beside is worse than no version screen at all.
        field(
            ui,
            "network access",
            "none, except the update check you press",
        );
        field(
            ui,
            "typeface",
            if self.jetbrains {
                "JetBrains Mono"
            } else {
                "built-in monospace"
            },
        );

        ui.add_space(16.0);
        self.updates.section(ui, env!("CARGO_PKG_VERSION"));

        ui.add_space(16.0);
        ui.label(RichText::new("What this protects").color(p::blue()).small());
        ui.label(
            RichText::new(
                "The biometric voiceprint (pitch, formants, timbre, micro-timing and \
                 the melody of an accent) is destroyed and cannot be recovered from the \
                 output. Each frame's measured phase is discarded, and every speaker is \
                 mapped onto one canonical register and vocal tract.",
            )
            .color(p::fg()),
        );

        ui.add_space(12.0);
        ui.label(
            RichText::new("What it does not do")
                .color(p::yellow())
                .small(),
        );
        ui.label(
            RichText::new(
                "The words are preserved on purpose, so de-identification alone does \
                 not keep the message secret, which is why the result is encrypted at \
                 rest by default. Nor can any signal-level transform change which \
                 phonemes you produced, so a strong regional accent may still be \
                 audible even though its melody is gone.",
            )
            .color(p::fg()),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("The app lock").color(p::yellow()).small());
        ui.label(RichText::new(veilvoice_crypto::lock::SCOPE).color(p::fg()));

        ui.add_space(16.0);
        self.policy.panel(ui);
    }
}

fn device_picker(
    ui: &mut egui::Ui,
    label: &str,
    devices: &[devices::DeviceInfo],
    chosen: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::muted()));
        let current = chosen.clone().unwrap_or_else(|| "system default".into());
        egui::ComboBox::from_id_salt(label)
            .width(360.0)
            .selected_text(RichText::new(current).color(p::cyan()))
            .show_ui(ui, |ui| {
                ui.selectable_value(chosen, None, "system default");
                for device in devices {
                    let mut text = device.name.clone();
                    if device.is_virtual_cable {
                        text.push_str("  ·  virtual cable");
                    }
                    ui.selectable_value(chosen, Some(device.name.clone()), text);
                }
            });
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label:<18}")).color(p::muted()));
        ui.label(RichText::new(value).color(p::cyan()));
    });
}

/// One level meter: a bar on the decibel scale, and the number beside it.
///
/// This was a **linear** bar with a decibel number printed next to it, which is
/// a meter arguing with itself: the number said -12 dB and the bar showed a
/// quarter. Ordinary speech at a sensible recording level peaks near -12 dBFS,
/// so the bar read as near-silence and the only way to fill it was to clip.
///
/// The scale now comes from `veilvoice_audio::meter`, which is where the peaks
/// come from, so this bar and the terminal's are the same bar. `hold` is the
/// highest level of the last moment or so, drawn as a mark: a transient is over
/// before an eye finishes moving, and a bar showing only *now* cannot show one.
fn meter(ui: &mut egui::Ui, label: &str, peak: f32, hold: f32) {
    use veilvoice_audio::meter;

    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::muted()));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(280.0, 12.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, p::bg_dark());

        let db = meter::dbfs(peak);
        let colour = if meter::clipping(peak) {
            p::red()
        } else if db >= -6.0 {
            p::yellow()
        } else if db >= -40.0 {
            p::green()
        } else {
            // Below -40 the signal is room tone rather than speech. Drawn muted,
            // so a quiet room does not read as a working microphone.
            p::muted()
        };
        let mut filled = rect;
        filled.set_width(rect.width() * meter::position(peak));
        painter.rect_filled(filled, 2.0, colour);

        // The held peak, as a hairline. Only where the bar is empty: inside the
        // fill it would be saying what the fill already says.
        if meter::position(hold) > meter::position(peak) {
            let x = rect.left() + rect.width() * meter::position(hold);
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.5, p::fg()),
            );
        }

        painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, p::border()));

        let text = if db <= meter::FLOOR_DB {
            "  -inf dBFS".to_string()
        } else {
            format!("{db:>6.1} dBFS")
        };
        ui.label(RichText::new(text).color(if meter::clipping(peak) {
            p::red()
        } else {
            p::muted()
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Marker 79.** Nothing that waits happens on the thread that draws.
    ///
    /// A window stutters for one of two reasons: it is asked to draw too
    /// rarely, or it is doing something slow between frames. The second is the
    /// one that cannot be tuned away, and it is invisible in a screenshot: the
    /// window simply stops for as long as the call takes.
    ///
    /// So the draw path is read for the calls that wait. Everything this
    /// application does that can block already runs on its own thread and
    /// reports back through a channel: the file dialogs after seven of them
    /// froze the window, the update check, the verifier, the group render, the
    /// key derivation. This keeps that true rather than assuming it.
    ///
    /// Comments are stripped first, for the reason the lock screen's guard
    /// gives: the first version of a test like this flags its own explanation.
    #[test]
    fn the_drawing_thread_never_waits_on_anything() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let body: String = source
            .split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        // The draw path is `update` and everything it reaches. Device
        // enumeration is the one filesystem-shaped call in this file and it
        // lives in `Default`, which runs once before the window opens.
        let update_at = body.find("fn update(&mut self").expect("update exists");
        let drawing = &body[update_at..];

        for waits in [
            "Command::new",
            "std::fs::read",
            "std::fs::write",
            "read_to_string",
            "join()",
            "thread::sleep",
            "devices::list",
            // Each of these is a stat syscall, which is cheap on a warm local
            // disk and is not cheap on a network share or a sleeping drive.
            // One per frame at 60 Hz is sixty of them a second for an answer
            // that changed when a file was dropped, which is where the check
            // that needs them lives. Added after reading the draw path for
            // marker 79 and finding none, so this keeps it that way rather
            // than fixing something.
            ".exists()",
            ".is_file()",
            ".is_dir()",
            "fs::metadata",
            "read_dir(",
            "canonicalize(",
        ] {
            assert!(
                !drawing.contains(waits),
                "the draw path calls {waits:?}, which waits. Move it to a thread \
                 and report back through a channel, as everything else here does."
            );
        }

        // The channels are drained without blocking. Counted rather than
        // searched for, because `try_recv()` contains `recv()` and the first
        // version of this reported the correct call as the fault.
        let blocking = drawing.matches("recv()").count() - drawing.matches("try_recv()").count();
        assert_eq!(
            blocking, 0,
            "a channel on the draw path is read with a blocking recv; use try_recv"
        );
    }

    /// An untouched window draws nothing.
    ///
    /// # The measurement this is here to keep
    ///
    /// With the animations off and nobody touching it, the window drew **2.1
    /// frames a second on every one of the nine tabs, for ever**, and cost 7
    /// to 9 per cent of a core doing it. After this it draws none, and costs
    /// 0.2 per cent. Measured on the same machine, twenty seconds a tab, with
    /// `VEILVOICE_FRAME_LOG=1` counting the frames and `/proc` counting the
    /// time.
    ///
    /// The cause was a pair of reasonable-looking decisions meeting. The
    /// microphone monitor sent an update on every poll whether or not
    /// anything had changed, and this file woke the window twice a second to
    /// ask the channel whether anything had arrived. Each half is the sort of
    /// thing that reads fine in review. Together they are a program that never
    /// sleeps.
    ///
    /// The rule now is that the thread with the news asks for the repaint,
    /// because it is the only thing that knows there is any. This checks the
    /// window is not asking on a timer instead.
    #[test]
    fn the_window_does_not_wake_itself_to_check_on_the_monitor() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        // The code only. This file is read by this test, and the name in the
        // assertion below is itself a match: the first version of this test
        // failed on its own message.
        let source = source.split("\n#[cfg(test)]").next().unwrap();
        let update_at = source.find("fn update(&mut self").expect("update exists");
        let drawing = &source[update_at..];
        assert!(
            !drawing.contains("self.watch.is_watching()"),
            "the draw path asks whether the monitor is running so it can wake \
             on a timer. An idle window then never stops drawing: this cost \
             2.1 frames a second on every tab. The monitor thread asks for a \
             repaint when it has something to report."
        );
    }

    /// The user guide describes the application that exists.
    ///
    /// It said "Five tabs" and documented five, and there are nine. The four
    /// it left out were **group**, **verify**, **settings** and **install** --
    /// among them the verify tab, which is the one this project tells people
    /// to use before running a download it has just told them not to trust.
    ///
    /// This is the fourth finding of one shape in this repository: a document
    /// describing the program, with nothing comparing the two. F-71 was two
    /// hand-typed copies of a number, F-101 a page linking files that were
    /// never published, F-110 an example the parser refused. The remedy is
    /// always the same one, and this is it for the guide.
    ///
    /// Each tab gets a heading of its own, named for the key the tab answers
    /// to, so `veilvoice-gui --tab verify` and the section explaining that tab
    /// cannot come apart. A tab added without a section fails the build here.
    #[test]
    fn the_user_guide_documents_every_tab() {
        let guide = include_str!("../../../docs/USER_GUIDE.md").replace("\r\n", "\n");
        let headings: Vec<&str> = guide
            .lines()
            .filter(|line| line.starts_with("### "))
            .map(|line| line.trim_start_matches("### ").trim())
            .collect();
        let missing: Vec<&str> = Tab::ALL
            .iter()
            .map(|tab| tab.key())
            .filter(|key| {
                !headings
                    .iter()
                    .any(|heading| heading.to_ascii_lowercase().contains(*key))
            })
            .collect();
        assert!(
            missing.is_empty(),
            "docs/USER_GUIDE.md has no section for these tabs: {}. Every tab \
             the window shows needs one, named for the key it answers to, or \
             the guide describes a different application from the one that \
             ships.",
            missing.join(", ")
        );
    }

    /// A count of the tabs, written out in the guide, is a second copy of a
    /// fact and drifts from the first. It already did: "Five tabs", nine tabs.
    #[test]
    fn the_user_guide_does_not_count_the_tabs_by_hand() {
        let guide = include_str!("../../../docs/USER_GUIDE.md").replace("\r\n", "\n");
        for wrong in [
            "Three tabs",
            "Four tabs",
            "Five tabs",
            "Six tabs",
            "Seven tabs",
            "Eight tabs",
            "Ten tabs",
        ] {
            assert!(
                !guide.contains(wrong),
                "docs/USER_GUIDE.md says {wrong:?} and the window shows {}. A \
                 number typed beside a list is a copy of the list's length, \
                 and it goes stale the first time a tab is added.",
                Tab::ALL.len()
            );
        }
    }

    /// The tab names `veilvoice-gui --help` lists have to be the tab names
    /// that exist.
    ///
    /// Written after the first version of that help text named `watch`,
    /// `security` and no `install`, when the keys are `monitor`, `lock` and
    /// `install`. Three wrong names in the one place somebody reads to find
    /// out what the right ones are, and the manual page is generated from that
    /// text, so the error would have shipped inside the package as well.
    #[test]
    fn the_help_text_lists_the_tabs_that_exist() {
        let usage = include_str!("main.rs")
            .split("const USAGE: &str = \"\\\n")
            .nth(1)
            .and_then(|rest| rest.split("\";").next())
            .expect("the usage text has to be findable");
        for tab in Tab::ALL {
            assert!(
                usage.contains(tab.key()),
                "`--help` does not mention the {:?} tab, whose name is {:?}",
                tab,
                tab.key()
            );
        }
    }

    /// Marker 92. A job running is not the window being used.
    ///
    /// The tempting version of an idle timer treats "something is happening" as
    /// "somebody is here", and it is exactly backwards for this program:
    /// somebody who starts a long render and walks away has walked away, and
    /// the recording being produced is the thing worth locking away.
    #[test]
    fn a_running_job_does_not_count_as_using_the_window() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let start = source
            .find("let (touched, dt) = ctx.input(")
            .expect("the idle check exists");
        let end = source[start..]
            .find("self.poll_job();")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let body = &source[start..end];
        for excuse in ["self.job", "is_busy()", "session.is_some()"] {
            assert!(
                !body.contains(excuse),
                "the idle timer consults {excuse:?}, so walking away from a \
                 running job would hold the window unlocked"
            );
        }
        assert!(
            body.contains("self.security.lock_now()"),
            "nothing actually locks the window"
        );
    }

    /// Marker 83. An unanswered hidden-volume question must stop the job, not
    /// quietly redirect it back beside the source file. A user who believes
    /// their recording went into a vault and finds it next to the original is
    /// the failure the whole question exists to prevent.
    #[test]
    fn an_unanswered_vault_question_blocks_the_job_rather_than_redirecting_it() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let start = source
            .find("let ready = self.input.is_some()")
            .expect("the gate exists");
        let gate = &source[start..start + 400];
        assert!(
            gate.contains("self.storage.destination.ready()"),
            "the start button ignores whether the destination is cleared for use"
        );
    }

    /// Marker 75. The record has to be taken without anybody knowing to ask,
    /// and it has to wait for the passphrase when there is one to wait for.
    #[test]
    fn the_integrity_record_runs_itself_at_launch_and_again_at_unlock() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let start = source
            .find("pub fn new(cc:")
            .expect("the constructor exists");
        let end = source[start..]
            .find("\n    /// Bring the controls")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let constructor = &source[start..end];
        assert!(
            constructor.contains("app.integrity.start(None)"),
            "nothing takes the record at launch, so it is still a command \
             somebody has to know to run"
        );
        assert!(
            constructor.contains("if !app.security.has_lock()"),
            "a sealed record cannot be read before the passphrase exists, so \
             the launch run has to stand aside when a lock is set"
        );

        let update_at = source.find("fn update(&mut self").expect("update exists");
        let drawing = &source[update_at..];
        assert!(
            drawing.contains("self.security.take_unlock_passphrase()"),
            "the unlock is the one moment the sealing passphrase exists and \
             nothing collects it"
        );
    }

    /// Every tab has a name, they are unique, and they round trip. The
    /// screenshot tool names each picture after one of these, so a change here
    /// renames a file the README links to.
    #[test]
    fn every_tab_has_a_stable_unique_name() {
        let mut keys: Vec<&str> = Tab::ALL.iter().map(|tab| tab.key()).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two tabs share a name");
        assert_eq!(count, 9);
        for tab in Tab::ALL {
            assert_eq!(Tab::from_key(tab.key()), Some(*tab));
            assert_eq!(Tab::from_key(&tab.key().to_uppercase()), Some(*tab));
        }
        assert_eq!(Tab::from_key("nothing-like-this"), None);
        assert_eq!(Tab::from_key(""), None);
    }

    /// Device selection is tested against synthetic lists, never the machine.
    /// See `preferred_output` for why that is not merely tidier.
    fn device(name: &str, default: bool, cable: bool) -> devices::DeviceInfo {
        devices::DeviceInfo {
            name: name.to_string(),
            is_default: default,
            is_virtual_cable: cable,
        }
    }

    /// The bar and the number beside it have to be saying the same thing.
    ///
    /// They did not: the bar was filled linearly and the number was decibels,
    /// so at ordinary speech the number said -12 and the bar showed a quarter.
    /// Both now come from `veilvoice_audio::meter`, and this is the assertion
    /// that keeps them there.
    #[test]
    fn the_bar_and_the_number_agree_about_the_level() {
        use veilvoice_audio::meter;
        for peak in [0.0f32, 0.001, 0.06, 0.251, 0.5, 0.9, 1.0] {
            let along = meter::position(peak);
            let db = meter::dbfs(peak);
            // The bar's fill is `position`; the number is `dbfs`. One is an
            // affine map of the other, so if they ever stop agreeing this fails.
            let from_db = ((db - meter::FLOOR_DB) / -meter::FLOOR_DB).clamp(0.0, 1.0);
            assert!(
                (along - from_db).abs() < 1e-6,
                "peak {peak}: bar at {along}, number says {db} dBFS"
            );
        }
        // And the thing the old meter got wrong, stated outright.
        assert!(
            meter::position(0.251) > 0.7,
            "speech at -12 dBFS must fill most of the bar"
        );
    }

    #[test]
    fn defaults_are_the_safe_ones() {
        let app = VeilVoiceApp::without_devices();
        assert!(
            app.neutralise_accent,
            "accent neutralisation should default on"
        );
        assert!(app.clean_metadata, "metadata stripping should default on");
        assert_eq!(app.intensity, 1.0);
        assert_eq!(app.reseed_secs, 2.0, "the seed should roll by default");
        assert!(app.session.is_none());
        assert!(
            app.security.encrypt_recordings,
            "recordings should be encrypted at rest by default"
        );
    }

    /// The default is only worth anything if the button honours it: with
    /// encryption on and nothing to encrypt with, a job must not start.
    #[test]
    fn a_job_cannot_start_before_the_at_rest_choice_is_made() {
        let app = VeilVoiceApp::without_devices();
        assert!(!app.security.ready_to_write());
        assert!(app.security.blocked_reason().is_some());
    }

    #[test]
    fn config_reflects_the_controls() {
        let app = VeilVoiceApp {
            intensity: 0.5,
            neutralise_accent: false,
            reseed_secs: 5.0,
            ..VeilVoiceApp::without_devices()
        };
        let cfg = app.config();
        assert_eq!(cfg.intensity, 0.5);
        assert!(!cfg.accent.enabled);
        assert_eq!(cfg.reseed_secs, 5.0);
        cfg.checked()
            .expect("every value the sliders can reach must be valid");
    }

    /// The slider's whole range must produce a configuration the engine
    /// accepts, or a user could drag it into an error.
    #[test]
    fn every_reachable_reseed_setting_is_valid() {
        let mut app = VeilVoiceApp::without_devices();
        for step in 0..=30 {
            app.reseed_secs = step as f32;
            app.config()
                .checked()
                .unwrap_or_else(|e| panic!("reseed_secs={step} rejected: {e}"));
        }
    }

    /// A virtual cable must win, because routing there is the whole point of
    /// live mode. Every arrangement, none of them involving real hardware.
    #[test]
    fn a_virtual_cable_is_preferred_over_the_system_default() {
        let cable = device("CABLE Input (VB-Audio Virtual Cable)", false, true);
        let speakers = device("Speakers", true, false);
        let other = device("HDMI", false, false);

        assert_eq!(
            preferred_output(&[speakers.clone(), cable.clone(), other.clone()]).as_deref(),
            Some(cable.name.as_str()),
            "a cable must beat the system default"
        );
        assert_eq!(
            preferred_output(&[other.clone(), speakers.clone()]).as_deref(),
            Some("Speakers"),
            "with no cable, the default"
        );
        // With neither a cable nor a default, the picker stays on "system
        // default" rather than seizing on an arbitrary device. Output is not
        // input here, and deliberately so: guessing an input wrong means the
        // user hears nothing and fixes it, while guessing an *output* wrong
        // means the veiled voice is quietly playing out of the wrong device.
        assert_eq!(
            preferred_output(std::slice::from_ref(&other)),
            None,
            "an arbitrary output must not be seized on"
        );
        assert_eq!(
            preferred_output(&[]),
            None,
            "an empty machine chooses nothing"
        );
    }

    #[test]
    fn the_default_input_is_preferred_then_the_first() {
        let default = device("Microphone", true, false);
        let first = device("Line In", false, false);
        assert_eq!(
            preferred_input(&[first.clone(), default.clone()]).as_deref(),
            Some("Microphone")
        );
        assert_eq!(
            preferred_input(std::slice::from_ref(&first)).as_deref(),
            Some("Line In"),
            "with no default, the first is better than nothing"
        );
        assert_eq!(preferred_input(&[]), None);
    }

    /// A policy that fixes the engine settings must reach the *job*, not just
    /// the widgets. The fields are deliberately left loose here: if `config`
    /// read them directly, this would fail.
    #[test]
    fn a_policy_constrains_the_settings_a_job_actually_uses() {
        let mut policy = veilvoice_policy::Policy::new();
        policy.require(veilvoice_policy::Requirement::NeutraliseAccent);
        policy.require(veilvoice_policy::Requirement::MinimumIntensity(80));
        policy.require(veilvoice_policy::Requirement::CleanMetadata);

        let app = VeilVoiceApp {
            intensity: 0.1,
            neutralise_accent: false,
            clean_metadata: false,
            policy: InForce::from_policy(policy),
            ..VeilVoiceApp::without_devices()
        };

        let config = app.config();
        assert!(
            (config.intensity - 0.8).abs() < 1e-6,
            "the floor must reach the engine: {}",
            config.intensity
        );
        assert!(
            config.accent.enabled,
            "a required accent neutralisation must reach the engine"
        );
        assert!(
            app.posture().clean_metadata,
            "a required metadata strip must reach the job"
        );
        config
            .checked()
            .expect("a constrained configuration must still be a valid one");
    }

    /// Whatever the sliders say, a policy may only ever make the result more
    /// thoroughly processed. Checked across the slider's whole range.
    #[test]
    fn a_policy_never_loosens_what_a_job_would_do() {
        let mut policy = veilvoice_policy::Policy::new();
        policy.require(veilvoice_policy::Requirement::MinimumIntensity(60));
        for step in 0..=10 {
            let asked = step as f32 / 10.0;
            let app = VeilVoiceApp {
                intensity: asked,
                neutralise_accent: false,
                policy: InForce::from_policy(policy.clone()),
                ..VeilVoiceApp::without_devices()
            };
            assert!(
                app.config().intensity >= asked,
                "asked for {asked}, got {}",
                app.config().intensity
            );
        }
    }

    /// The at-rest requirement is pinned in the state, not merely drawn
    /// disabled -- and pinning it must also close the dialogue that turns it
    /// off, which is reachable from more than one frame's worth of state.
    #[test]
    fn a_required_encryption_is_pinned_rather_than_only_disabled() {
        let mut policy = veilvoice_policy::Policy::new();
        policy.require(veilvoice_policy::Requirement::EncryptRecordings);
        let mut app = VeilVoiceApp {
            policy: InForce::from_policy(policy),
            ..VeilVoiceApp::without_devices()
        };
        app.security.encrypt_recordings = false;
        // The integrity record, started before anything is drawn and finished
        // on its own thread. With an app lock set this run is skipped: the
        // record is sealed under the app-lock passphrase, so it can only be
        // read once that passphrase exists, and `poll` starts it again the
        // moment the window unlocks.
        if !app.security.has_lock() {
            app.integrity.start(None);
        }
        app.apply_policy();
        assert!(app.security.encryption_pinned);
        assert!(app.security.encrypt_recordings);
        assert!(app.posture().encrypt_recordings);
    }

    /// A required lock is announced and never imposed: VeilVoice cannot set a
    /// lock, because that needs a passphrase only the user has.
    #[test]
    fn a_required_lock_is_announced_and_not_imposed() {
        let mut policy = veilvoice_policy::Policy::new();
        policy.require(veilvoice_policy::Requirement::AppLock);
        let mut app = VeilVoiceApp {
            policy: InForce::from_policy(policy),
            ..VeilVoiceApp::without_devices()
        };
        // The integrity record, started before anything is drawn and finished
        // on its own thread. With an app lock set this run is skipped: the
        // record is sealed under the app-lock passphrase, so it can only be
        // read once that passphrase exists, and `poll` starts it again the
        // moment the window unlocks.
        if !app.security.has_lock() {
            app.integrity.start(None);
        }
        app.apply_policy();
        assert!(app.security.lock_required);
        assert!(
            !app.security.has_lock(),
            "nothing may invent a lock the user did not set"
        );
        assert!(
            !app.security.is_locked(),
            "and the application must stay usable"
        );
    }

    /// With no policy -- the ordinary case -- nothing is pinned and nothing is
    /// raised.
    #[test]
    fn without_a_policy_nothing_is_fixed() {
        let mut app = VeilVoiceApp {
            intensity: 0.25,
            neutralise_accent: false,
            clean_metadata: false,
            ..VeilVoiceApp::without_devices()
        };
        // The integrity record, started before anything is drawn and finished
        // on its own thread. With an app lock set this run is skipped: the
        // record is sealed under the app-lock passphrase, so it can only be
        // read once that passphrase exists, and `poll` starts it again the
        // moment the window unlocks.
        if !app.security.has_lock() {
            app.integrity.start(None);
        }
        app.apply_policy();
        assert!(!app.security.encryption_pinned);
        assert!(!app.security.lock_required);
        assert_eq!(app.intensity, 0.25);
        assert!(!app.neutralise_accent);
        assert!(!app.clean_metadata);
        assert_eq!(app.config().intensity, 0.25);
    }

    /// The one test that talks to the machine's audio stack. Kept single, and
    /// last: enumerating devices from several test threads at once on a
    /// headless runner is what produced an access violation in CI.
    #[test]
    fn building_the_app_with_real_device_enumeration_does_not_panic() {
        let app = VeilVoiceApp::default();
        // Whatever the machine has, the choice must be one of its own devices.
        if let Some(chosen) = app.chosen_output.as_deref() {
            assert!(app.outputs.iter().any(|d| d.name == chosen));
        }
        if let Some(chosen) = app.chosen_input.as_deref() {
            assert!(app.inputs.iter().any(|d| d.name == chosen));
        }
    }
}
