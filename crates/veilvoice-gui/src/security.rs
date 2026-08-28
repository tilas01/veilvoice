// SPDX-License-Identifier: GPL-3.0-or-later
//! The application lock, and the at-rest encryption of what VeilVoice writes.
//!
//! # Two passwords, and why
//!
//! There are two, deliberately:
//!
//! - the **app lock**, which decides whether VeilVoice will open at all, and
//! - the **recording passphrase**, which encrypts the files it produces.
//!
//! Collapsing them into one would mean that unlocking the app also unseals
//! every recording it has ever written, which is the opposite of what a lock is
//! for. [`veilvoice_crypto::lock`] additionally domain-separates its verifier,
//! so even a user who types the same string in both places does not end up with
//! two copies of one value.
//!
//! # What the lock is worth
//!
//! Not much against an attacker with the disk, and the UI says so in
//! [`veilvoice_crypto::lock::SCOPE`], shown on the unlock screen itself rather
//! than buried in an about page. It stops the person who picks up your unlocked
//! laptop. It does not stop someone who takes the drive.
//!
//! # A limitation of typing a password into a window
//!
//! A text field owns a `String`, so a passphrase exists as ordinary heap bytes
//! while it is being typed. That window cannot be removed — something has to
//! receive the keystrokes — but it can be kept short, and it is:
//!
//! - the typing buffer is wiped the moment the passphrase is confirmed;
//! - the confirmed passphrase is held only as a [`veilvoice_crypto::Secret`],
//!   page-locked and zeroized on drop, for the rest of the session;
//! - locking the app, or changing the passphrase, wipes both.
//!
//! It used to be kept as a plain `String` for the whole session, which was a
//! much larger window for no benefit.
//!
//! None of this defends against someone who can read this process's memory. If
//! they can, they have already won, and `docs/WHITEPAPER.md` §7 says so rather
//! than implying otherwise. What it does is stop a passphrase lingering in a
//! heap allocation long after it was needed, where a core dump or a swapped
//! page could pick it up.
//!
//! # In plain words
//!
//! The lock on the window, and the encryption of the files VeilVoice writes.
//!
//! There are two passphrases and they do different jobs. One opens the
//! application. The other encrypts a recording, and it is asked for separately
//! because they protect different things and losing one should not mean losing the
//! other.
//!
//! The panel says what the lock is worth and what it is not: it stops somebody who
//! picks up your unlocked computer, and it does not stop somebody who has the
//! disk. Encrypting the recording is what protects the recording.

use crate::theme::palette as p;
use egui::{Color32, RichText};
use std::path::PathBuf;
use std::sync::mpsc;
use veilvoice_crypto::{container, kdf, lock, LockStore, Secret};
use zeroize::Zeroize;

/// Move a typed passphrase out of its `String` and into page-locked storage,
/// wiping the buffer it came from.
///
/// No `unsafe`, so the intermediate `Vec` is a genuine second copy for a
/// moment; `Secret::new` wipes it before returning. Writing through
/// `String::as_bytes_mut` would avoid the copy and is not worth an `unsafe`
/// block in a crate that has none.
fn into_secret(typed: &mut String) -> Secret {
    let mut bytes = typed.as_bytes().to_vec();
    let secret = Secret::new(&mut bytes);
    typed.zeroize();
    secret
}

/// How the recording that comes out of a job is protected.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sealing {
    /// Argon2id over a passphrase held for this session.
    Password,
    /// X25519 + ML-KEM-768 to a recipient's public key file.
    PublicKey,
}

/// What a background lock operation was trying to do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Unlock,
    Set,
    Change,
    Remove,
}

/// A finished lock operation: the store as it now stands, and how it went.
type OpResult = (Option<LockStore>, Result<Op, String>);

/// Everything about locking the app and sealing its output.
pub struct Security {
    /// Where the lock file lives, or `None` if the platform did not say.
    path: Option<PathBuf>,
    /// The configured lock, if there is one.
    store: Option<LockStore>,
    /// A problem reading the lock, reported rather than treated as "no lock".
    load_error: Option<String>,
    /// Whether the app is currently locked. Only ever true with a `store`.
    locked: bool,

    // --- unlock screen ---
    entry: String,
    message: Option<(String, Color32)>,
    pending: Option<mpsc::Receiver<OpResult>>,

    // --- set / change form ---
    current: String,
    fresh: String,
    repeat: String,

    // --- recording encryption ---
    /// On by default. Turning it off goes through [`Self::confirm_disable`].
    pub encrypt_recordings: bool,
    /// Which container mode sealing uses.
    pub sealing: Sealing,
    /// Recipient public key, for [`Sealing::PublicKey`].
    pub public_key: Option<PathBuf>,
    /// The key picker, while it is open.
    choosing_key: crate::dialog::Pending,
    /// The typing buffer. Held in a `String` only because that is what the
    /// text widget requires, and wiped the moment it is confirmed.
    passphrase: String,
    /// The confirmed session passphrase, in page-locked storage.
    ///
    /// Kept here rather than as the `String` above so that the plaintext
    /// version exists only while it is being typed, instead of for the
    /// whole session. That does not make it safe against someone who can
    /// read this process's memory -- nothing can -- it shortens the window.
    held: Option<Secret>,
    /// Whether `passphrase` has been confirmed against `passphrase_repeat`.
    passphrase_set: bool,
    passphrase_repeat: String,
    /// Whether the "write it unencrypted?" dialogue is open.
    confirm_disable: bool,
    /// A policy requires encryption at rest, so it cannot be turned off here.
    ///
    /// Set once from [`crate::policy::InForce`]. When true the checkbox is
    /// disabled *and* [`Self::encrypt_recordings`] is forced on, because a
    /// disabled checkbox is a claim about pixels and this is a claim about
    /// behaviour.
    pub encryption_pinned: bool,
    /// A policy requires the app lock to be set. Shown on the lock tab when it
    /// is not; never used to refuse entry.
    ///
    /// Refusing would lock somebody out of their own recordings because of a
    /// file in their own configuration directory, which is a worse outcome
    /// than an unlocked application saying plainly that it should be locked.
    pub lock_required: bool,
}

impl Default for Security {
    /// The safe state, and deliberately free of I/O so tests and
    /// `VeilVoiceApp::default()` never touch the real lock file. The running
    /// app calls [`Security::load`].
    fn default() -> Self {
        Self {
            path: None,
            store: None,
            load_error: None,
            locked: false,
            entry: String::new(),
            message: None,
            pending: None,
            current: String::new(),
            fresh: String::new(),
            repeat: String::new(),
            encrypt_recordings: true,
            sealing: Sealing::Password,
            public_key: None,
            choosing_key: crate::dialog::Pending::new(),
            passphrase: String::new(),
            held: None,
            passphrase_set: false,
            passphrase_repeat: String::new(),
            confirm_disable: false,
            encryption_pinned: false,
            lock_required: false,
        }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        self.wipe_secrets();
    }
}

impl Security {
    /// Read the lock file for this machine and start locked if one is set.
    pub fn load() -> Self {
        let path = lock::default_path();
        // Field-by-field rather than struct-update syntax: `Security` has a
        // `Drop` that wipes its secrets, and `..Default::default()` would have
        // to move fields out of a value that owns one.
        let mut security = Self::default();
        security.path = path.clone();
        let Some(path) = path else {
            security.load_error =
                Some("cannot find a configuration directory on this platform".into());
            return security;
        };
        match LockStore::open(&path) {
            Ok(Some(store)) => {
                security.store = Some(store);
                security.locked = true;
            }
            Ok(None) => {}
            // A lock that will not parse must never read as an absent lock.
            Err(e) => {
                security.load_error = Some(format!("the app lock could not be read: {e}"));
                security.locked = true;
            }
        }
        security
    }

    /// Whether the unlock screen should be shown instead of the app.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Whether a lock is configured at all.
    pub fn has_lock(&self) -> bool {
        self.store.is_some()
    }

    /// Lock the app now, wiping the session passphrase with it.
    pub fn lock_now(&mut self) {
        if self.store.is_some() {
            self.locked = true;
            self.wipe_secrets();
        }
    }

    /// Wipe every plaintext secret this struct is holding.
    fn wipe_secrets(&mut self) {
        for field in [
            &mut self.entry,
            &mut self.current,
            &mut self.fresh,
            &mut self.repeat,
            &mut self.passphrase,
            &mut self.passphrase_repeat,
        ] {
            field.zeroize();
        }
        self.held = None;
        self.passphrase_set = false;
    }

    /// Whether a job may start: either encryption is off, or there is something
    /// to encrypt with.
    pub fn ready_to_write(&self) -> bool {
        if !self.encrypt_recordings {
            return true;
        }
        match self.sealing {
            Sealing::Password => self.held.is_some(),
            Sealing::PublicKey => self.public_key.is_some(),
        }
    }

    /// Why a job cannot start yet, for the button's tooltip.
    pub fn blocked_reason(&self) -> Option<&'static str> {
        if self.ready_to_write() {
            return None;
        }
        Some(match self.sealing {
            Sealing::Password => "set a recording passphrase first",
            Sealing::PublicKey => "choose a recipient public key first",
        })
    }

    /// How the next job should protect its output.
    ///
    /// Returns the material by value so the worker thread owns it; the copy
    /// held here stays for the next file.
    pub fn plan(&self) -> Plan {
        if !self.encrypt_recordings {
            return Plan::Plaintext;
        }
        match self.sealing {
            Sealing::Password => match &self.held {
                Some(secret) => Plan::Password(secret.clone()),
                None => Plan::Missing,
            },
            Sealing::PublicKey => match &self.public_key {
                Some(path) => Plan::PublicKey(path.clone()),
                // Unreachable through the UI, which gates on `ready_to_write`,
                // but falling back to plaintext here would silently do the one
                // thing the user did not ask for.
                None => Plan::Missing,
            },
        }
    }

    fn spawn(&mut self, op: Op, password: String, replacement: String) {
        let store = self.store.take();
        let path = self.path.clone();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.message = None;

        // Argon2id at the default cost takes a noticeable fraction of a second;
        // running it on the UI thread would freeze the window mid-keystroke.
        std::thread::spawn(move || {
            let _ = tx.send(run_op(op, store, path, password, replacement));
        });
    }

    /// Collect a finished lock operation. Returns true if anything changed.
    fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        let (store, outcome) = match rx.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => (
                None,
                Err("the lock operation stopped unexpectedly".to_string()),
            ),
        };
        self.pending = None;
        self.store = store;

        match outcome {
            Ok(Op::Unlock) => {
                self.locked = false;
                self.entry.zeroize();
                self.message = None;
            }
            Ok(Op::Set) => {
                self.wipe_form();
                self.message = Some(("app lock set".into(), p::green()));
            }
            Ok(Op::Change) => {
                self.wipe_form();
                self.message = Some(("app lock password changed".into(), p::green()));
            }
            Ok(Op::Remove) => {
                self.wipe_form();
                self.locked = false;
                self.message = Some(("app lock removed".into(), p::yellow()));
            }
            Err(e) => {
                self.entry.zeroize();
                self.message = Some((e, p::red()));
            }
        }
        true
    }

    fn wipe_form(&mut self) {
        self.current.zeroize();
        self.fresh.zeroize();
        self.repeat.zeroize();
    }

    fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether a lock operation is running, so the window keeps repainting and
    /// the spinner actually spins.
    pub fn is_busy(&self) -> bool {
        self.busy()
    }

    /// The full-window unlock screen. Nothing else is drawn while this is up.
    pub fn unlock_screen(&mut self, ui: &mut egui::Ui, motion: crate::prefs::Motion) {
        self.poll();

        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            // The mark, moving, in the space the explanation used to take.
            //
            // It is the same soundbar the header draws and the website draws,
            // and it obeys the same motion preference, so somebody who asked
            // their system for less movement gets it at rest. A locked window
            // is a window somebody is looking at while they remember a
            // passphrase; something alive in it is worth more than a paragraph
            // that helps whoever should not be reading it.
            crate::soundbar::draw(
                ui,
                egui::vec2(120.0, 34.0),
                motion,
                ui.input(|i| i.time) as f32,
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("VeilVoice")
                    .size(24.0)
                    .color(p::fg())
                    .strong(),
            );
            ui.label(RichText::new("locked").color(p::yellow()));
        });
        ui.add_space(24.0);

        // **Marker 74.** A locked window says it is locked and nothing else.
        //
        // It used to say a great deal: what the lock is and is not worth, where
        // the file lives, and that deleting that file starts over and is not a
        // bypass. Every one of those sentences is true, and every one of them
        // is addressed to the wrong person. The reader of a locked window is
        // either its owner, who does not need any of it right now, or somebody
        // who picked the machine up, who should not be handed the location of
        // the file and the news that removing it works.
        //
        // The account of what the lock is worth has not been dropped. It is in
        // `docs/USER_GUIDE.md` and on the security tab of the *unlocked*
        // application, which are the two places its owner reads it.
        if self.load_error.is_some() {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("This copy cannot be unlocked here.").color(p::yellow()));
                ui.label(
                    RichText::new("See the user guide, under the app lock.")
                        .color(p::muted())
                        .small(),
                );
            });
            return;
        }

        let cooldown = self.store.as_ref().and_then(|s| s.cooldown());
        let busy = self.busy();

        ui.horizontal(|ui| {
            ui.label(RichText::new("password").color(p::muted()));
            let field = ui.add_enabled(
                !busy && cooldown.is_none(),
                egui::TextEdit::singleline(&mut self.entry)
                    .password(true)
                    .desired_width(260.0),
            );
            let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = ui
                .add_enabled(
                    !busy && cooldown.is_none() && !self.entry.is_empty(),
                    egui::Button::new(RichText::new("  unlock  ").strong()),
                )
                .clicked();
            if (submitted || clicked) && !self.entry.is_empty() && cooldown.is_none() && !busy {
                let entry = std::mem::take(&mut self.entry);
                self.spawn(Op::Unlock, entry, String::new());
            }
        });

        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("deriving key…").color(p::muted()));
            });
        }

        if let Some(wait) = cooldown {
            ui.label(
                RichText::new(format!(
                    "too many attempts, {} s before the next one",
                    wait.as_secs()
                ))
                .color(p::yellow()),
            );
        } else if let Some((text, colour)) = &self.message {
            ui.label(RichText::new(text).color(*colour));
        }

        // The count of failed attempts is not shown here either. It tells the
        // owner nothing they did not just do, and it tells somebody else how
        // many people have tried and how recently, which is information about
        // the owner rather than about the lock. It is on the security tab,
        // where the person reading it has already proved who they are.
    }

    /// The security tab: manage the lock, and see what it is worth.
    pub fn tab(&mut self, ui: &mut egui::Ui) {
        self.poll();

        // Whatever the key picker answered while the reader was browsing. Taken
        // before anything is drawn, so a chosen key is in place by the time the
        // line that shows it is painted.
        if let Some(path) = self.choosing_key.taken() {
            self.public_key = Some(path);
        }

        ui.add_space(4.0);
        ui.label(RichText::new("App lock").color(p::blue()).small());
        match &self.path {
            Some(path) => {
                ui.label(
                    RichText::new(path.display().to_string())
                        .color(p::muted())
                        .small(),
                );
            }
            None => {
                ui.label(
                    RichText::new(
                        "No configuration directory could be found on this platform, so \
                         a lock cannot be stored. The CLI's `veilvoice lock --path` can \
                         put one wherever you choose.",
                    )
                    .color(p::yellow()),
                );
                return;
            }
        }

        // A policy can require a lock and cannot impose one: setting it needs a
        // passphrase only the user has. So the requirement is stated, loudly,
        // beside the control that satisfies it -- and the application stays
        // usable, because refusing to open would lock somebody out of their own
        // recordings over a file in their own configuration directory.
        if self.lock_required && !self.has_lock() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "fixed by policy: {}",
                    veilvoice_policy::Requirement::AppLock.describe()
                ))
                .color(p::yellow()),
            );
            ui.label(
                RichText::new(
                    "No lock is set. VeilVoice cannot set one for you, because it                      needs a passphrase only you have, so it says so here                      instead of refusing to open.",
                )
                .small()
                .color(p::yellow()),
            );
            ui.add_space(6.0);
        }

        let busy = self.busy();
        if self.has_lock() {
            ui.label(RichText::new("a lock is set").color(p::green()));
            ui.add_space(8.0);

            ui.add_enabled_ui(!busy, |ui| {
                password_row(ui, "current   ", &mut self.current);
                password_row(ui, "new       ", &mut self.fresh);
                password_row(ui, "repeat    ", &mut self.repeat);
            });
            let matched = !self.fresh.is_empty() && self.fresh == self.repeat;

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !busy && matched && !self.current.is_empty(),
                        egui::Button::new("change password"),
                    )
                    .clicked()
                {
                    let (current, fresh) = (
                        std::mem::take(&mut self.current),
                        std::mem::take(&mut self.fresh),
                    );
                    self.repeat.zeroize();
                    self.spawn(Op::Change, current, fresh);
                }
                if ui
                    .add_enabled(
                        !busy && !self.current.is_empty(),
                        egui::Button::new(RichText::new("remove lock").color(p::red())),
                    )
                    .clicked()
                {
                    let current = std::mem::take(&mut self.current);
                    self.spawn(Op::Remove, current, String::new());
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("lock now"))
                    .clicked()
                {
                    self.lock_now();
                }
            });
        } else {
            ui.label(RichText::new("no lock is set").color(p::muted()));
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Use a different password here than the one you use for encrypted \
                     recordings. They are separate on purpose, so that opening the app \
                     is not the same act as unsealing everything it has written.",
                )
                .color(p::muted())
                .small(),
            );
            ui.add_enabled_ui(!busy, |ui| {
                password_row(ui, "password  ", &mut self.fresh);
                password_row(ui, "repeat    ", &mut self.repeat);
            });
            let matched = !self.fresh.is_empty() && self.fresh == self.repeat;
            if ui
                .add_enabled(!busy && matched, egui::Button::new("set app lock"))
                .clicked()
            {
                let fresh = std::mem::take(&mut self.fresh);
                self.repeat.zeroize();
                self.spawn(Op::Set, fresh, String::new());
            }
            if !self.fresh.is_empty() && !matched {
                ui.label(
                    RichText::new("the two entries differ")
                        .color(p::yellow())
                        .small(),
                );
            }
        }

        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("deriving key…").color(p::muted()));
            });
        }
        if let Some((text, colour)) = &self.message {
            ui.label(RichText::new(text).color(*colour));
        }

        ui.add_space(16.0);
        ui.separator();
        ui.label(
            RichText::new("What this lock is worth")
                .color(p::yellow())
                .small(),
        );
        ui.label(RichText::new(lock::SCOPE).color(p::fg()));
    }

    /// The at-rest controls that sit inside the file tab.
    pub fn recording_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("At rest").color(p::blue()).small());

        if self.encryption_pinned {
            // Forced here as well as drawn disabled. The dialogue that turns
            // this off is reachable from more than one frame's worth of state,
            // and a policy that held only while the checkbox was drawn would
            // not be a policy.
            self.encrypt_recordings = true;
            self.confirm_disable = false;
        }

        let mut wanted = self.encrypt_recordings;
        let changed = ui
            .add_enabled(
                !self.encryption_pinned,
                egui::Checkbox::new(&mut wanted, "encrypt the result at rest"),
            )
            .changed();
        if changed && !self.encryption_pinned {
            if wanted {
                self.encrypt_recordings = true;
            } else {
                // Stay on until the warning has been read and answered.
                self.confirm_disable = true;
            }
        }
        if self.encryption_pinned {
            ui.label(
                RichText::new(format!(
                    "fixed by policy: {}",
                    veilvoice_policy::Requirement::EncryptRecordings.describe()
                ))
                .small()
                .color(p::yellow()),
            );
        }

        if !self.encrypt_recordings {
            ui.label(
                RichText::new(
                    "the recording will be written unencrypted, so anyone who reads the \
                     file can still hear every word",
                )
                .color(p::red())
                .small(),
            );
            return;
        }

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.sealing, Sealing::Password, "passphrase");
            ui.selectable_value(&mut self.sealing, Sealing::PublicKey, "public key");
        });

        match self.sealing {
            Sealing::Password if self.held.is_some() => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("passphrase set for this session").color(p::green()));
                    if ui.button("change").clicked() {
                        self.passphrase.zeroize();
                        self.held = None;
                        self.passphrase_set = false;
                    }
                });
            }
            Sealing::Password => {
                password_row(ui, "passphrase", &mut self.passphrase);
                password_row(ui, "repeat    ", &mut self.passphrase_repeat);
                let matched =
                    !self.passphrase.is_empty() && self.passphrase == self.passphrase_repeat;
                if ui
                    .add_enabled(matched, egui::Button::new("use this passphrase"))
                    .clicked()
                {
                    self.held = Some(into_secret(&mut self.passphrase));
                    self.passphrase_repeat.zeroize();
                    self.passphrase_set = true;
                }
                if !self.passphrase.is_empty() && !matched {
                    ui.label(
                        RichText::new("the two entries differ")
                            .color(p::yellow())
                            .small(),
                    );
                }
                ui.label(
                    RichText::new(
                        "Argon2id, 256 MiB. Separate from the app-lock password, and \
                         there is no way to recover it.",
                    )
                    .color(p::muted())
                    .small(),
                );
            }
            Sealing::PublicKey => {
                ui.horizontal(|ui| {
                    if ui.button("choose public key…").clicked() {
                        self.choosing_key
                            .start(crate::dialog::Ask::open_filtered("public key", &["pub"]));
                    }
                    match &self.public_key {
                        Some(path) => {
                            ui.label(RichText::new(path.display().to_string()).color(p::cyan()))
                        }
                        None => ui.label(RichText::new("no key chosen").color(p::muted())),
                    };
                });
                ui.label(
                    RichText::new(
                        "X25519 + ML-KEM-768 hybrid: breaking it requires breaking both, \
                         so a recording stored today survives a quantum adversary later. \
                         Generate a pair with `veilvoice keygen`.",
                    )
                    .color(p::muted())
                    .small(),
                );
            }
        }
    }

    /// The dialogue shown when the user turns at-rest encryption off.
    ///
    /// Returns true while it is open, so the caller can disable the rest of the
    /// window rather than let a click land behind it.
    pub fn disable_dialogue(&mut self, ctx: &egui::Context) -> bool {
        if !self.confirm_disable {
            return false;
        }
        egui::Window::new(RichText::new("Write recordings unencrypted?").color(p::red()))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                for paragraph in DISABLE_WARNING {
                    ui.label(RichText::new(*paragraph).color(p::fg()));
                    ui.add_space(6.0);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("  keep it encrypted  ").strong())
                        .clicked()
                    {
                        self.confirm_disable = false;
                    }
                    if ui
                        .button(RichText::new("write it unencrypted").color(p::red()))
                        .clicked()
                    {
                        self.encrypt_recordings = false;
                        self.confirm_disable = false;
                    }
                });
            });
        true
    }
}

/// What the user is told before recordings stop being encrypted.
///
/// Kept as data so the test suite can assert it still says the uncomfortable
/// part, exactly as the CLI's equivalent does.
pub const DISABLE_WARNING: &[&str] = &[
    "VeilVoice destroys the voiceprint, not the words. An unencrypted result is \
     still a recording of everything that was said.",
    "Anyone who can read the file (another account on this machine, a backup, a \
     cloud sync client, anyone who later gets the disk) can hear all of it.",
    "Deleting it afterwards is not a fix. On an SSD, SD card or USB stick the \
     original blocks can survive every overwrite.",
    "That is why at-rest encryption is the default rather than something you \
     have to go and find.",
];

/// What a finished job should do with its bytes.
#[derive(Clone, PartialEq, Eq)]
pub enum Plan {
    /// Seal with Argon2id over this passphrase.
    Password(Secret),
    /// Seal to the hybrid public key in this file.
    PublicKey(PathBuf),
    /// Write in the clear, as explicitly chosen.
    Plaintext,
    /// Encryption was asked for with nothing to encrypt to. Never produced by
    /// the UI, and refused rather than downgraded if it ever were.
    Missing,
}

impl Plan {
    /// Seal `wav` if the plan says to, and write it. Returns where it landed.
    ///
    /// Runs on the job thread, never the UI thread: Argon2id is meant to be
    /// slow. `wav` is the in-memory encoding, so an encrypted recording never
    /// exists on disk in the clear.
    ///
    /// `params` is the caller's, rather than being read from
    /// [`kdf::KdfParams::default`] in here. The app passes the default; the
    /// tests pass a cheap profile, because a unit test that allocates 256 MiB
    /// and runs three passes of Argon2 is not testing the thing it claims to —
    /// it is testing the runner's memory, and on a CI machine running several
    /// such tests at once it stops being a test at all.
    pub fn write(
        &self,
        path: &std::path::Path,
        wav: &[u8],
        params: kdf::KdfParams,
    ) -> Result<PathBuf, String> {
        let sealed = match self {
            Plan::Plaintext => {
                std::fs::write(path, wav).map_err(|e| format!("{}: {e}", path.display()))?;
                return Ok(path.to_path_buf());
            }
            Plan::Missing => {
                return Err("encryption was requested but no key or passphrase was set".into())
            }
            Plan::Password(passphrase) => {
                container::seal_with_password(passphrase.expose(), wav, params)
                    .map_err(|e| e.to_string())?
            }
            Plan::PublicKey(key_path) => {
                let encoded =
                    std::fs::read(key_path).map_err(|e| format!("{}: {e}", key_path.display()))?;
                let pk = veilvoice_crypto::hybrid::PublicKey::from_bytes(&encoded)
                    .map_err(|e| e.to_string())?;
                container::seal_to_public_key(&pk, wav).map_err(|e| e.to_string())?
            }
        };
        let out = container::veil_path(path);
        std::fs::write(&out, &sealed).map_err(|e| format!("{}: {e}", out.display()))?;
        Ok(out)
    }
}

/// Deliberately opaque about the passphrase, so a plan cannot reach a log line
/// through `{:?}` — the same rule [`veilvoice_crypto::Secret`] follows.
impl std::fmt::Debug for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password(_) => f.write_str("Plan::Password(redacted)"),
            Self::PublicKey(path) => write!(f, "Plan::PublicKey({})", path.display()),
            Self::Plaintext => f.write_str("Plan::Plaintext"),
            Self::Missing => f.write_str("Plan::Missing"),
        }
    }
}

/// Run one lock operation, off the UI thread.
fn run_op(
    op: Op,
    store: Option<LockStore>,
    path: Option<PathBuf>,
    mut password: String,
    mut replacement: String,
) -> OpResult {
    let outcome: OpResult = match (op, store) {
        // The store is handed back on failure too: it now carries the recorded
        // attempt, and dropping it would reset the rate limit.
        (Op::Unlock, Some(mut store)) => match store.unlock(password.as_bytes()) {
            Ok(()) => (Some(store), Ok(Op::Unlock)),
            Err(e) => (Some(store), Err(e.to_string())),
        },
        (Op::Set, None) => match path {
            None => (None, Err("no configuration directory".into())),
            Some(path) => {
                match LockStore::create(&path, password.as_bytes(), kdf::KdfParams::default()) {
                    Ok(store) => (Some(store), Ok(Op::Set)),
                    Err(e) => (None, Err(e.to_string())),
                }
            }
        },
        (Op::Change, Some(mut store)) => {
            match store.change_password(password.as_bytes(), replacement.as_bytes()) {
                Ok(()) => (Some(store), Ok(Op::Change)),
                Err(e) => (Some(store), Err(e.to_string())),
            }
        }
        (Op::Remove, Some(store)) => match store.remove(password.as_bytes()) {
            Ok(()) => (None, Ok(Op::Remove)),
            // `remove` consumed the store, so it is reopened to keep the
            // recorded failure. A lock that vanished because the password was
            // wrong would be a spectacular own goal.
            Err(e) => (reopen(path.as_deref()), Err(e.to_string())),
        },
        // The UI never offers these combinations; refusing beats guessing.
        (_, store) => (
            store,
            Err("the app lock changed underneath this action".into()),
        ),
    };

    password.zeroize();
    replacement.zeroize();
    outcome
}

fn reopen(path: Option<&std::path::Path>) -> Option<LockStore> {
    path.and_then(|p| LockStore::open(p).ok().flatten())
}

fn password_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::muted()));
        ui.add(
            egui::TextEdit::singleline(value)
                .password(true)
                .desired_width(260.0),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Marker 74.** The locked window explains nothing.
    ///
    /// It used to explain a great deal: what the lock is and is not worth,
    /// where its file lives, and that deleting that file starts over. All true,
    /// all addressed to the wrong person. The reader of a locked window is
    /// either its owner, who does not need any of it at that moment, or
    /// somebody who picked the machine up.
    ///
    /// This reads the source of `unlock_screen` rather than rendering it,
    /// because what is being held is that certain sentences are not reachable
    /// from that function at all. A rendering test would only prove they were
    /// absent from one frame.
    #[test]
    fn the_locked_window_tells_a_stranger_nothing() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let start = source
            .find("pub fn unlock_screen")
            .expect("the unlock screen has to exist");
        let end = source[start..]
            .find("\n    /// The security tab")
            .map(|at| start + at)
            .unwrap_or(source.len());
        // Comments stripped first. The first version of this flagged its own
        // explanation of why the count is not shown, which is the same honest
        // failure `veilvoice-priv`'s subprocess guard records: what matters is
        // what the function *draws*, so that is what is searched.
        let body: String = source[start..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        for forbidden in [
            "lock::SCOPE",
            "path.display()",
            "Delete the lock file",
            "failed attempt",
        ] {
            assert!(
                !body.contains(forbidden),
                "the locked window mentions {forbidden:?}, which is for its owner \
                 and not for whoever is holding the machine"
            );
        }

        // And the account itself has not simply been deleted: the tab, which
        // only an unlocked application draws, still carries it.
        let tab = source
            .find("pub fn tab")
            .map(|at| &source[at..])
            .unwrap_or("");
        assert!(
            tab.contains("lock::SCOPE"),
            "what the lock is worth has to be somewhere its owner reads it"
        );
    }

    /// Cheap on purpose: these tests exercise the plan, not Argon2.
    fn weak() -> kdf::KdfParams {
        kdf::KdfParams::weak_for_tests()
    }

    #[test]
    fn encryption_at_rest_is_the_default() {
        let s = Security::default();
        assert!(
            s.encrypt_recordings,
            "recordings must be encrypted unless the user says otherwise"
        );
        assert!(!s.is_locked(), "no lock file means no lock");
        assert!(!s.has_lock());
    }

    /// A job must not be able to start with encryption on and nothing to
    /// encrypt with, or the "default" would silently degrade to plaintext.
    #[test]
    fn a_job_is_blocked_until_there_is_something_to_encrypt_with() {
        let mut s = Security::default();
        assert!(!s.ready_to_write());
        assert_eq!(s.blocked_reason(), Some("set a recording passphrase first"));

        s.sealing = Sealing::PublicKey;
        assert_eq!(
            s.blocked_reason(),
            Some("choose a recipient public key first")
        );
        s.public_key = Some(PathBuf::from("someone.pub"));
        assert!(s.ready_to_write());

        // Turning encryption off is the one way to proceed with nothing set.
        let mut s = Security::default();
        s.encrypt_recordings = false;
        assert!(s.ready_to_write());
        assert_eq!(s.blocked_reason(), None);
    }

    /// Unticking the box must not take effect until the warning is answered.
    #[test]
    fn disabling_encryption_needs_the_dialogue_to_be_answered() {
        let mut s = Security::default();
        s.confirm_disable = true;
        assert!(
            s.encrypt_recordings,
            "encryption must stay on while the question is open"
        );
        // The dialogue's destructive button is the only thing that clears it.
        s.encrypt_recordings = false;
        s.confirm_disable = false;
        assert!(matches!(s.plan(), Plan::Plaintext));
    }

    #[test]
    fn the_warning_states_the_actual_consequence() {
        let text = DISABLE_WARNING.join(" ").to_lowercase();
        assert!(text.contains("everything that was said"));
        assert!(text.contains("deleting it afterwards is not a fix"));
        assert!(text.contains("default"));
        for reassurance in ["safe", "secure", "protected"] {
            assert!(
                !text.contains(reassurance),
                "reassuring word: {reassurance}"
            );
        }
    }

    #[test]
    fn a_plan_with_nothing_set_refuses_rather_than_writing_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.wav");
        assert!(Plan::Missing.write(&path, b"audio", weak()).is_err());
        assert!(!path.exists(), "nothing may be written on refusal");
    }

    #[test]
    fn a_password_plan_seals_beside_the_recording_and_leaves_no_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.veiled.wav");
        let wav = b"RIFF....WAVEfake but recognisable".to_vec();

        let mut typed = String::from("a recording passphrase");
        let out = Plan::Password(into_secret(&mut typed))
            .write(&path, &wav, weak())
            .unwrap();
        assert!(typed.is_empty(), "the typing buffer must be wiped");
        assert_eq!(out, container::veil_path(&path));
        assert!(!path.exists(), "the plaintext must never reach the disk");

        let sealed = std::fs::read(&out).unwrap();
        assert!(!sealed.windows(4).any(|w| w == b"RIFF"));
        assert_eq!(
            container::open_with_password(b"a recording passphrase", &sealed).unwrap(),
            wav
        );
    }

    #[test]
    fn a_plaintext_plan_writes_the_file_as_asked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.wav");
        let out = Plan::Plaintext
            .write(&path, b"audio bytes", weak())
            .unwrap();
        assert_eq!(out, path);
        assert_eq!(std::fs::read(&path).unwrap(), b"audio bytes");
    }

    /// The confirmed passphrase must not linger as ordinary heap bytes. It used
    /// to be kept as a `String` for the whole session; it is now moved into a
    /// page-locked `Secret` the moment it is confirmed, and the typing buffer
    /// is wiped.
    #[test]
    fn the_confirmed_passphrase_leaves_no_plaintext_buffer_behind() {
        let mut typed = String::from("a recording passphrase");
        let secret = into_secret(&mut typed);

        assert!(typed.is_empty(), "the typing buffer must be wiped");
        assert_eq!(secret.expose(), b"a recording passphrase");
        assert!(
            format!("{secret:?}").contains("redacted"),
            "a secret must not be printable"
        );
    }

    /// And the plan handed to the worker thread carries the `Secret`, not a
    /// copy of the text.
    #[test]
    fn the_plan_carries_page_locked_material_not_a_string() {
        let mut s = Security::default();
        let mut typed = String::from("session passphrase");
        s.held = Some(into_secret(&mut typed));
        s.passphrase_set = true;

        assert!(s.ready_to_write());
        match s.plan() {
            Plan::Password(secret) => assert_eq!(secret.expose(), b"session passphrase"),
            other => panic!("expected a password plan, got {other:?}"),
        }
    }

    /// Locking must take the session passphrase with it, or "locked" would be
    /// a screen rather than a state.
    #[test]
    fn locking_wipes_the_session_passphrase() {
        let mut s = Security::default();
        s.passphrase = "a recording passphrase".into();
        s.passphrase_set = true;
        s.wipe_secrets();
        assert!(s.passphrase.is_empty());
        assert!(!s.passphrase_set);
        assert!(!s.ready_to_write(), "a wiped passphrase must block the job");
    }

    #[test]
    fn locking_does_nothing_when_no_lock_is_configured() {
        let mut s = Security::default();
        s.lock_now();
        assert!(!s.is_locked(), "there is nothing to unlock it with");
    }
}
