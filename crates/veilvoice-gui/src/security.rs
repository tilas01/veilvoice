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
//! while it is being typed. That window cannot be removed, because something has
//! to receive the keystrokes, but it can be kept short, and it is:
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
    /// Argon2id over the **app-lock** passphrase, so everything VeilVoice
    /// writes is sealed without anybody choosing a second secret.
    ///
    /// **Marker 86.** This reverses a decision the crypto crate states in as
    /// many words, and the reversal is deliberate rather than accidental, so
    /// the cost is written here as well as in the documentation: one passphrase
    /// now opens the application *and* everything it has ever written.
    /// Somebody compelled to unlock VeilVoice in front of another person used
    /// to reveal the session; with this on, they reveal the archive too.
    ///
    /// The container is sealed under the passphrase itself rather than under a
    /// key derived from the lock file. That is what keeps a deleted or damaged
    /// lock from taking the recordings with it: the file carries its own salt
    /// and cost, so `veilvoice decrypt` opens it with the same passphrase on
    /// any machine, with or without a lock.
    AppLock,
}

/// What a background lock operation was trying to do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Unlock,
    Set,
    Change,
    Remove,
    /// Clear an outstanding interference report. Needs the passphrase, which is
    /// the whole reason it is an operation and not a button.
    Acknowledge,
}

/// A finished lock operation: the store as it now stands, and how it went.
type OpResult = (Option<LockStore>, Result<Op, String>, Option<StoreKey>);

/// The obfuscated store's key, derived on the worker thread beside the unlock.
///
/// It has to be derived there. It costs a full Argon2id run at the configured
/// cost, and doing that on the UI thread would freeze the window for the
/// fraction of a second the unlock was supposed to have finished in. The
/// unlock is already paying for one run on that thread, so the key rides back
/// with the result rather than being asked for again.
use veilvoice_crypto::hoard::StoreKey;

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
    /// Whether the current lock was the autolock's doing rather than a person's.
    ///
    /// Read by the lock screen for one line of text, and cleared on unlock. It
    /// is not persisted: a lock that survives a restart is a lock whose reason
    /// nobody can still be wondering about.
    auto_locked: bool,
    /// Whether the lock reported interference. Shown once the app is open, not
    /// on the lock screen: telling a stranger at the lock screen that their
    /// last attempt was noticed is telling them something they can use.
    tampered: bool,
    /// The obfuscated store's key, derived beside the unlock that produced it.
    ///
    /// Held for exactly one caller, like the passphrase beside it, and for the
    /// same reason: whoever takes it owns it for the session. It is not
    /// re-derivable without another Argon2id run, so a caller that drops it
    /// leaves the folder unreadable until the next unlock.
    just_unlocked_key: Option<StoreKey>,
    /// The passphrase that just opened the lock, held for exactly one caller to
    /// collect. See [`Security::take_unlock_passphrase`].
    just_unlocked: Option<String>,
    /// The app-lock passphrase, kept for the session so [`Sealing::AppLock`]
    /// can seal with it.
    ///
    /// Only ever populated when that mode is already chosen, which is the
    /// point: a user who has not asked for it keeps the old behaviour, where
    /// the passphrase is wiped the instant it has been checked. Page-locked
    /// and zeroed on drop, like every other secret here, and cleared by
    /// [`Security::lock_now`].
    app_secret: Option<Secret>,

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
            auto_locked: false,
            just_unlocked_key: None,
            tampered: false,
            just_unlocked: None,
            app_secret: None,
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
        let Some(_path) = path else {
            security.load_error =
                Some("cannot find a configuration directory on this platform".into());
            return security;
        };
        match lock::open_default() {
            Ok((Some(mut store), restored)) => {
                if restored {
                    // A copy of the lock had gone and was rebuilt from the
                    // other. Files do not delete themselves, so this is a
                    // report, and it is made to stick: `report_tamper` holds it
                    // in memory now and the next successful unlock writes it
                    // where a restart cannot lose it.
                    store.report_tamper();
                }
                security.tampered = restored || store.tampered();
                security.store = Some(store);
                security.locked = true;
            }
            Ok((None, _)) => {}
            // A lock that will not parse must never read as an absent lock.
            Err(e) => {
                security.load_error = Some(format!("the app lock could not be read: {e}"));
                security.locked = true;
            }
        }
        security
    }

    /// Take the passphrase that just opened the lock, once.
    ///
    /// Returns `Some` on exactly the frame after a successful unlock and `None`
    /// on every other. It exists so [`crate::integrity`] can open a record
    /// sealed under the app-lock passphrase without this module keeping that
    /// passphrase for the life of the session. The caller must wipe what it
    /// gets; the worker that receives it does.
    pub fn take_unlock_passphrase(&mut self) -> Option<String> {
        self.just_unlocked.take()
    }

    /// Collect the obfuscated store's key from the unlock that just happened.
    ///
    /// One caller, one frame, like the passphrase. See
    /// [`crate::vault_store::VaultStore`] for what it opens and what that is
    /// worth.
    pub fn take_unlock_store_key(&mut self) -> Option<StoreKey> {
        self.just_unlocked_key.take()
    }

    /// Start in [`Sealing::AppLock`], because the user asked for that last time.
    ///
    /// Applied at startup, before the window is drawn and so before anything
    /// can be unlocked, which matters: the passphrase is captured as the lock
    /// opens and only when this mode is already chosen.
    pub fn prefer_app_lock_sealing(&mut self, on: bool) {
        if on && self.store.is_some() {
            self.sealing = Sealing::AppLock;
        }
    }

    /// Whether the app-lock sealing mode is currently chosen, so the window can
    /// have the choice remembered.
    pub fn seals_with_app_lock(&self) -> bool {
        self.sealing == Sealing::AppLock
    }

    /// Whether the lock reported having been interfered with.
    ///
    /// Stays true until an unlock acknowledges it, which needs the passphrase,
    /// so nobody can dismiss the banner except the person who can open the app.
    pub fn tampered(&self) -> bool {
        self.tampered
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
    ///
    /// This is the deliberate one: somebody pressed Lock. The screen says
    /// nothing about how it got there, because the person reading it already
    /// knows.
    pub fn lock_now(&mut self) {
        self.lock_inner(false);
    }

    /// Lock because nobody has touched the window for a while.
    ///
    /// Identical to [`Self::lock_now`] except that the lock screen says so.
    /// Coming back to a locked window you did not lock is the moment to
    /// wonder whether somebody else has been at the machine, and answering
    /// that costs nothing and saves a bad minute.
    pub fn lock_after_idle(&mut self) {
        self.lock_inner(true);
    }

    fn lock_inner(&mut self, by_idle: bool) {
        if self.store.is_some() {
            self.locked = true;
            self.auto_locked = by_idle;
            self.wipe_secrets();
        }
    }

    /// Wipe every plaintext secret this struct is holding.
    fn wipe_secrets(&mut self) {
        // The store key goes with the passphrase. It is not a `String`, so it
        // is dropped rather than zeroized here; `Secret` wipes itself on drop,
        // which is the whole reason the key is held in one.
        self.just_unlocked_key = None;
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
        // The one-frame handover to the integrity record, wiped here in case
        // nobody collected it. Locking the window again must not leave a
        // passphrase behind because a caller happened not to look.
        if let Some(mut carried) = self.just_unlocked.take() {
            carried.zeroize();
        }
        // Marker 86's session copy of the app-lock passphrase goes with
        // everything else. Locking the window has to put back the state a
        // fresh launch would be in, or "lock" is a picture of a lock.
        self.app_secret = None;
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
            Sealing::AppLock => self.app_secret.is_some(),
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
            // The passphrase is captured as the lock opens, so this is what a
            // user sees who turned the mode on after unlocking. Saying "lock
            // and unlock" is the actual remedy; "no passphrase" would not be.
            Sealing::AppLock => "lock the app and unlock it again to use this",
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
            Sealing::AppLock => match &self.app_secret {
                // A password plan, because that is exactly what it is: the
                // container is sealed under the app-lock passphrase and
                // carries its own salt, so nothing about opening it later
                // depends on the lock file still existing.
                Some(secret) => Plan::Password(secret.clone()),
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
        let (store, outcome, store_key) = match rx.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => (
                None,
                Err("the lock operation stopped unexpectedly".to_string()),
                None,
            ),
        };
        self.pending = None;
        self.store = store;

        // Whatever the operation was, the store is the authority on whether a
        // report is outstanding. Reading it back here means one place decides,
        // rather than each arm remembering to.
        self.tampered = self.store.as_ref().is_some_and(LockStore::tampered) || self.tampered;

        match outcome {
            Ok(Op::Unlock) => {
                self.locked = false;
                self.auto_locked = false;
                // Moved rather than wiped, for one caller and one frame. The
                // integrity record is sealed under this passphrase and the
                // unlock is the only moment it exists, so wiping it here would
                // mean the record could never be opened. Whoever takes it is
                // responsible for wiping it; `take_unlock_passphrase` says so,
                // and `wipe_secrets` catches the case where nobody does.
                let opened = std::mem::take(&mut self.entry);
                // Marker 86. Kept for the session only when the mode that
                // needs it is already chosen. A user who has not asked for
                // this keeps the old behaviour exactly: the passphrase is
                // wiped the moment it has been checked, and never sits in
                // memory waiting for a feature nobody switched on.
                if self.sealing == Sealing::AppLock {
                    let mut copy = opened.clone();
                    self.app_secret = Some(into_secret(&mut copy));
                }
                self.just_unlocked = Some(opened);
                self.just_unlocked_key = store_key;
                self.message = None;
            }
            Ok(Op::Acknowledge) => {
                self.tampered = false;
                self.wipe_form();
                self.message = Some(("interference report cleared".into(), p::green()));
            }
            Ok(Op::Set) => {
                self.wipe_form();
                self.message = Some(("app lock set".into(), p::green()));
            }
            Ok(Op::Change) => {
                self.wipe_form();
                // F-94. The session copy is the *old* passphrase, and keeping
                // it would seal every recording made from here with a password
                // the user has just replaced and may never type again. Dropped
                // rather than quietly updated: the new one went to the worker
                // thread and was wiped there, so the honest move is to say the
                // mode needs another unlock, which `blocked_reason` already
                // does.
                let had_secret = self.app_secret.take().is_some();
                self.message = Some(if had_secret {
                    (
                        "app lock password changed. Lock and unlock to seal new \
                         recordings with it; ones already written still open with the \
                         old password."
                            .into(),
                        p::yellow(),
                    )
                } else {
                    ("app lock password changed".into(), p::green())
                });
            }
            Ok(Op::Remove) => {
                self.wipe_form();
                self.locked = false;
                self.auto_locked = false;
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

        // Vertically centred rather than pinned near the top. A locked window
        // has one thing in it, and one thing sitting a fifth of the way down
        // an empty panel looks like a page that failed to load. The space
        // above is a third of what is left over, which puts the mark slightly
        // above centre -- where an eye looks first.
        let content_height = 260.0;
        let slack = (ui.available_height() - content_height).max(0.0);
        ui.add_space((slack / 3.0).clamp(16.0, 120.0));

        ui.vertical_centered(|ui| {
            // The mark, moving, in the space the explanation used to take.
            //
            // It is the same soundbar the header draws and the website draws,
            // and it obeys the same motion preference, so somebody who asked
            // their system for less movement gets it at rest. A locked window
            // is a window somebody is looking at while they remember a
            // passphrase; something alive in it is worth more than a paragraph
            // that helps whoever should not be reading it.
            //
            // In the icon's badge here rather than bare. On the header the
            // bars alone read as VeilVoice because the header is full of other
            // VeilVoice furniture; on an empty locked window they read as a
            // stray animation, and the badge is what makes the window say
            // whose it is.
            crate::soundbar::badge(ui, 96.0, motion, ui.input(|i| i.time) as f32);
            ui.add_space(14.0);
            ui.label(
                RichText::new("VeilVoice")
                    .size(26.0)
                    .color(p::fg())
                    .strong(),
            );
            ui.label(RichText::new("locked").color(p::yellow()));

            // Whether the window locked itself, said once and then dropped.
            //
            // Somebody who comes back to a locked window they did not lock
            // should not have to wonder whether they left it open or somebody
            // else closed it. It says which, and then it gets out of the way:
            // the moment a character is typed the line is gone, because by
            // then it has been read and the person is busy.
            //
            // It tells a stranger nothing the empty room did not already: that
            // whoever owns this walked away. What it does not say is when, how
            // long the delay was, or how many attempts have been made, all of
            // which are about the owner rather than about the lock.
            if self.auto_locked && self.entry.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("locked itself after a while unused")
                        .color(p::muted())
                        .small(),
                );
            }
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

        // Centred under the mark, rather than pinned to the left edge while
        // everything above it sits in the middle. `vertical_centered` centres
        // each child it is given, and a `horizontal` row counts as one child,
        // so the label, the field and the button move together as a group.
        crate::layout::centred_row(ui, |ui| {
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
            crate::layout::centred_row(ui, |ui| {
                ui.spinner();
                ui.label(RichText::new("deriving key…").color(p::muted()));
            });
        }

        ui.vertical_centered(|ui| {
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
        });

        // The count of failed attempts is not shown here either. It tells the
        // owner nothing they did not just do, and it tells somebody else how
        // many people have tried and how recently, which is information about
        // the owner rather than about the lock. It is on the security tab,
        // where the person reading it has already proved who they are.
    }

    /// The standing report that the lock file was interfered with.
    ///
    /// **Marker 76.** It is drawn here rather than on the lock screen, and the
    /// distinction matters. The lock screen is read by whoever is holding the
    /// machine, and telling them their edit was noticed tells them to try
    /// something else. This side of the lock is read only by somebody who has
    /// already produced the passphrase.
    ///
    /// It will not go away on its own. Clearing it runs
    /// [`veilvoice_crypto::LockStore::acknowledge`], which asks for the
    /// passphrase again, so the only person who can dismiss the report is the
    /// one who could have opened the lock anyway.
    fn interference_banner(&mut self, ui: &mut egui::Ui) {
        if !self.tampered {
            return;
        }
        let busy = self.busy();
        egui::Frame::new()
            .fill(p::bg_dark())
            .stroke(egui::Stroke::new(1.0, p::red()))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("The app lock was interfered with")
                        .color(p::red())
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Either the stored lock was edited by somebody who did not know \
                         your passphrase, or one of its two copies was deleted and had to \
                         be rebuilt from the other. Your passphrase still works and the \
                         lock is still in force. Nothing here says anything about your \
                         recordings, which have their own password.",
                    )
                    .color(p::fg()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "If this was you, moving files about or restoring a backup, clear \
                         it. If it was not, treat the machine as one somebody else has had \
                         their hands on.",
                    )
                    .color(p::muted())
                    .small(),
                );
                ui.add_space(8.0);
                ui.add_enabled_ui(!busy, |ui| {
                    password_row(ui, "passphrase", &mut self.current);
                });
                if ui
                    .add_enabled(
                        !busy && !self.current.is_empty(),
                        egui::Button::new("clear this report"),
                    )
                    .clicked()
                {
                    let current = std::mem::take(&mut self.current);
                    self.spawn(Op::Acknowledge, current, String::new());
                }
            });
        ui.add_space(12.0);
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

        self.interference_banner(ui);

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
                password_row(ui, "current", &mut self.current);
                password_row(ui, "new", &mut self.fresh);
                password_row(ui, "repeat", &mut self.repeat);
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
                password_row(ui, "password", &mut self.fresh);
                password_row(ui, "repeat", &mut self.repeat);
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

        if self.store.as_ref().is_some_and(|s| !s.every_copy_current()) {
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "The second copy of this lock is kept where only an administrator \
                     can write it, and could not be updated from here. It still holds \
                     the previous password. Run VeilVoice once as an administrator to \
                     finish the change.",
                )
                .color(p::yellow()),
            );
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
            // Offered only where there is a lock to seal with. Showing a mode
            // that cannot work, greyed out, invites the reading that VeilVoice
            // is withholding something.
            if self.store.is_some() {
                ui.selectable_value(&mut self.sealing, Sealing::AppLock, "app lock");
            }
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
                password_row(ui, "repeat", &mut self.passphrase_repeat);
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
            Sealing::AppLock => {
                if self.app_secret.is_some() {
                    ui.label(
                        RichText::new("every recording is sealed with your app-lock password")
                            .color(p::green()),
                    );
                } else {
                    ui.label(
                        RichText::new(
                            "lock the app and unlock it again to start using this. The \
                             password is taken as the lock opens, which is the only \
                             moment it exists.",
                        )
                        .color(p::yellow()),
                    );
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "One password for the application and for everything it writes. \
                         That is the convenience and it is also the whole of the cost: \
                         anybody who makes you unlock VeilVoice has opened every \
                         recording as well, not just this session. Two separate \
                         passwords keep those apart.",
                    )
                    .color(p::muted())
                    .small(),
                );
                ui.label(
                    RichText::new(
                        "Recordings stay openable if the lock is ever removed: each file \
                         carries its own salt, so `veilvoice decrypt` opens it with the \
                         same password on any machine. Forgetting that password still \
                         loses them, and nothing can undo that.",
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
    "Anyone who can read the file (a backup, a cloud sync client, anyone who \
     later gets the disk) can hear all of it.",
    "Deleting it afterwards is not a fix. On an SSD, SD card or USB stick the \
     original blocks can survive every overwrite.",
    "That is why at-rest encryption is the default rather than something you \
     have to go and find.",
    "The file will be created readable only by your account. That is a file \
     permission and nothing more: it does not survive a copy, a backup, or \
     anyone who has the disk.",
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
    /// and runs three passes of Argon2 is not testing the thing it claims to,
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
                // Owner-only, the same as the command line's identical branch.
                //
                // An unencrypted recording is still a recording of everything
                // that was said, so at minimum it is not left readable by every
                // other account on the machine. `veilvoice anonymise --encrypt
                // false` had written 0600 since it was written; this, the
                // window's version of exactly the same decision, wrote 0644.
                //
                // A file permission is a much weaker thing than the encryption
                // being declined here, and the interface says so rather than
                // letting it read as a consolation.
                veilvoice_crypto::privatefile::write_owner_only(path, wav)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
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
/// through `{:?}`, the same rule [`veilvoice_crypto::Secret`] follows.
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
            Ok(()) => {
                // Derived here, while the passphrase is in hand and this
                // thread is already the one paying for Argon2.
                let key = store.store_key(password.as_bytes()).ok();
                (Some(store), Ok(Op::Unlock), key)
            }
            Err(e) => (Some(store), Err(e.to_string()), None),
        },
        (Op::Acknowledge, Some(mut store)) => match store.acknowledge(password.as_bytes()) {
            Ok(()) => (Some(store), Ok(Op::Acknowledge), None),
            Err(e) => (Some(store), Err(e.to_string()), None),
        },
        // **F-141.** Through the vault, which is what `Security::load` reads
        // from. This called `LockStore::create`, which writes the single
        // pre-vault file `applock.bin` -- so a lock set in the window was
        // written somewhere the window never looked. It appeared to work,
        // vanished on the next launch, and the *second* attempt failed with
        // "could not read or write the app-lock file", because the file it was
        // about to create was already there.
        //
        // `path` is still carried for the message below and for `reopen`.
        (Op::Set, None) => match path {
            None => (None, Err("no configuration directory".into()), None),
            Some(path) => {
                let base = path.parent().unwrap_or(&path).to_path_buf();
                match lock::create_in(&base, password.as_bytes(), kdf::KdfParams::default()) {
                    Ok(store) => (Some(store), Ok(Op::Set), None),
                    Err(e) => (None, Err(e.to_string()), None),
                }
            }
        },
        (Op::Change, Some(mut store)) => {
            match store.change_password(password.as_bytes(), replacement.as_bytes()) {
                Ok(()) => (Some(store), Ok(Op::Change), None),
                Err(e) => (Some(store), Err(e.to_string()), None),
            }
        }
        (Op::Remove, Some(store)) => match store.remove(password.as_bytes()) {
            Ok(()) => (None, Ok(Op::Remove), None),
            // `remove` consumed the store, so it is reopened to keep the
            // recorded failure. A lock that vanished because the password was
            // wrong would be a spectacular own goal.
            Err(e) => (reopen(path.as_deref()), Err(e.to_string()), None),
        },
        // The UI never offers these combinations; refusing beats guessing.
        (_, store) => (
            store,
            Err("the app lock changed underneath this action".into()),
            None,
        ),
    };

    password.zeroize();
    replacement.zeroize();
    outcome
}

fn reopen(path: Option<&std::path::Path>) -> Option<LockStore> {
    path.and_then(|p| LockStore::open(p).ok().flatten())
}

/// The width every passphrase label is given, so every field starts level.
///
/// Wide enough for "passphrase", which is the longest of them.
const PASSWORD_LABEL_WIDTH: f32 = 82.0;

/// One labelled passphrase field, with the field in the same place every time.
///
/// # Why the label gets a column of its own
///
/// These labels used to be padded with trailing spaces to line the fields up:
/// `"current"`, `"new"`, `"repeat"`, `"password"`, against a
/// bare `"passphrase"`. That aligns nothing outside a terminal. The interface
/// font is proportional, so a space is not the width of a letter and eight
/// letters plus two spaces is not the width of ten letters; and egui gives
/// trailing whitespace no reliable width at all.
///
/// The visible result was that fields sat at slightly different places on
/// different screens, and the screens that differed most were the ones drawn
/// only after setup, because those carry the labels that needed the most
/// padding. Buttons underneath inherited the same drift. Giving the label a
/// fixed column puts every field, and everything lined up beneath it, at one
/// x on every screen.
///
/// Returns the field itself, so a caller, or a test, can ask where it landed.
fn password_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(PASSWORD_LABEL_WIDTH, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // `allocate_ui_with_layout` asks for this width but gives back
                // only what the contents used, so a short label would consume a
                // short column and the field would move left again.
                // `set_min_width` is what makes the column a column.
                ui.set_min_width(PASSWORD_LABEL_WIDTH);
                ui.label(RichText::new(label).color(p::muted()))
            },
        );
        ui.add(
            egui::TextEdit::singleline(value)
                .password(true)
                .desired_width(260.0),
        )
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every passphrase field starts at the same x, whatever its label says.
    ///
    /// The defect: the labels were padded with trailing spaces to fake a
    /// column, which lines nothing up in a proportional font. Screens drawn
    /// only after setup carry the labels that needed the most padding, so
    /// their fields, and the buttons under them, sat at a different place
    /// from the ones present at launch.
    ///
    /// The shortest and the longest label in use are drawn here. If the fields
    /// ever part company again this fails with both positions.
    #[test]
    fn every_passphrase_field_starts_in_the_same_place() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(640.0, 400.0),
            )),
            ..Default::default()
        };

        let mut starts: Vec<(&str, f32)> = Vec::new();
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for label in ["new", "password", "passphrase"] {
                    let mut value = String::new();
                    // Where the field actually landed, read back from the
                    // widget. Deriving it from the column width instead would
                    // be the test agreeing with itself: it would pass whether
                    // or not the column was ever applied.
                    let field = password_row(ui, label, &mut value);
                    starts.push((label, field.rect.left()));
                }
            });
        });

        let first = starts[0].1;
        for (label, at) in &starts {
            assert!(
                (at - first).abs() < 0.5,
                "the field after {label:?} starts at {at}, and the first starts at {first}"
            );
        }
    }

    /// No passphrase label is padded with spaces to fake its width.
    ///
    /// The column is what aligns these now. A label that comes back padded is
    /// somebody reaching for the old trick, and it would drift again the first
    /// time the font changed.
    #[test]
    fn no_passphrase_label_is_padded_with_spaces() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        for line in source.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("password_row(ui, \"") {
                continue;
            }
            let label = trimmed.split('"').nth(1).expect("a quoted label");
            assert_eq!(
                label,
                label.trim(),
                "the label {label:?} is padded with spaces; give it the column instead"
            );
        }
    }

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
    /// Marker 86. The passphrase is kept only for the mode that needs it.
    ///
    /// A user who has not asked for app-lock sealing must keep the old
    /// behaviour exactly: the passphrase is wiped the instant it has been
    /// checked. Holding it "just in case" would be a security regression paid
    /// for by everybody, to make a feature nobody switched on slightly more
    /// convenient.
    #[test]
    fn the_app_lock_passphrase_is_kept_only_when_it_is_going_to_be_used() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let poll = source.find("fn poll(&mut self)").expect("poll exists");
        let end = source[poll..]
            .find("\n    fn wipe_form")
            .map(|at| poll + at)
            .unwrap_or(source.len());
        let body = &source[poll..end];
        assert!(
            body.contains("if self.sealing == Sealing::AppLock"),
            "the capture is unconditional, so every user now carries their \
             app-lock passphrase in memory for the session"
        );
    }

    /// F-94. Changing the app-lock password must not leave the old one sealing
    /// new recordings. A user who changes their password and keeps working
    /// would otherwise produce files that open with a password they have just
    /// replaced, and be told nothing.
    #[test]
    fn changing_the_password_drops_the_passphrase_that_was_sealing_with_it() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let start = source.find("Ok(Op::Change) => {").expect("the arm exists");
        // To the next arm rather than a fixed number of bytes: the first
        // version of this used 900 and reported a message that was there.
        let end = source[start..]
            .find("Ok(Op::Remove)")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let arm = &source[start..end];
        assert!(
            arm.contains("self.app_secret.take()"),
            "the old passphrase survives a password change and keeps sealing \
             recordings with it"
        );
        // Two short phrases rather than one long one: the message is written
        // across several source lines with Rust's string continuation, so the
        // sentence never appears contiguously in the file. The first version of
        // this searched for the whole sentence and failed on a message that was
        // there and correct.
        for phrase in ["Lock and unlock", "old password"] {
            assert!(
                arm.contains(phrase),
                "the message does not mention {phrase:?}: a user whose \
                 already-written recordings still use the previous password has \
                 to be told, or they will delete it"
            );
        }
    }

    /// Marker 86. Locking the window must put the state back where a fresh
    /// launch would leave it, or the lock is a picture of a lock.
    #[test]
    fn locking_the_window_drops_the_sealing_passphrase() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let wipe = source
            .find("fn wipe_secrets(&mut self)")
            .expect("wipe_secrets exists");
        let end = source[wipe..]
            .find("\n    /// ")
            .unwrap_or(source.len() - wipe);
        assert!(
            source[wipe..wipe + end].contains("self.app_secret = None"),
            "the session copy of the app-lock passphrase outlives a lock"
        );
    }

    /// Marker 86. The plan has to be a password plan, because that is what
    /// keeps the recordings openable after the lock is gone.
    #[test]
    fn app_lock_sealing_produces_a_container_that_outlives_the_lock() {
        let mut security = Security::default();
        security.encrypt_recordings = true;
        security.sealing = Sealing::AppLock;

        // Nothing captured yet: refused rather than quietly written in clear.
        assert!(matches!(security.plan(), Plan::Missing));
        assert!(!security.ready_to_write());

        let mut typed = String::from("the app lock passphrase");
        security.app_secret = Some(into_secret(&mut typed));
        assert!(security.ready_to_write());

        let Plan::Password(secret) = security.plan() else {
            panic!(
                "app-lock sealing must produce a password plan, so the file \
                    carries its own salt and needs no lock file to open"
            );
        };
        assert_eq!(secret.expose(), b"the app lock passphrase");
    }

    /// The mode is only offered where there is a lock to seal with.
    #[test]
    fn app_lock_sealing_is_not_offered_without_a_lock() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        assert!(
            source.contains("if self.store.is_some() {")
                && source.contains("Sealing::AppLock, \"app lock\""),
            "the app-lock mode must be offered only when a lock exists"
        );
    }

    /// Marker 76. The report has to be reachable from the tab and only from
    /// the tab, and clearing it has to go through the passphrase rather than
    /// through a flag the drawing code can set.
    #[test]
    fn the_interference_report_is_behind_the_lock_and_behind_the_passphrase() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let tab = source
            .find("    /// The security tab")
            .map(|at| &source[at..])
            .expect("the tab has to exist");
        assert!(
            tab.contains("self.interference_banner(ui)"),
            "the report has to be drawn somewhere its owner will see it"
        );

        let banner = source
            .find("fn interference_banner")
            .map(|at| &source[at..])
            .expect("the banner has to exist");
        assert!(
            banner.contains("Op::Acknowledge"),
            "clearing the report has to run the acknowledgement, which asks for \
             the passphrase, not just clear a flag"
        );
        let clears: usize = banner
            .split("fn tab")
            .next()
            .unwrap_or("")
            .matches("self.tampered = false")
            .count();
        assert_eq!(
            clears, 0,
            "the banner must not clear the report itself; only the finished \
             acknowledgement may, and that has already proved the passphrase"
        );
    }

    #[test]
    fn the_locked_window_tells_a_stranger_nothing() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let start = source
            .find("pub fn unlock_screen")
            .expect("the unlock screen has to exist");
        let end = source[start..]
            .find("\n    /// The standing report")
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
            // Marker 76. The interference report is the same mistake in a new
            // shape: telling whoever is holding the machine that their last
            // edit was noticed tells them to try a different one.
            "interference_banner",
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

    /// An unencrypted recording is still readable only by this account.
    ///
    /// `veilvoice anonymise --encrypt false` had written 0600 since it was
    /// written. This, the window's version of the same decision, wrote 0644,
    /// so turning encryption off in the interface left the recording readable
    /// by every other account on the machine and turning it off on the command
    /// line did not.
    #[cfg(unix)]
    #[test]
    fn a_plaintext_plan_still_writes_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.wav");
        Plan::Plaintext
            .write(&path, b"audio bytes", weak())
            .unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "an unencrypted recording is {mode:o}, so anyone with an account here can play it"
        );
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
    /// A real lock in a temporary directory, at test cost.
    ///
    /// `lock_now` refuses to lock when there is nothing to unlock with, which
    /// is the right behaviour and means these tests need an actual store.
    fn locked_security() -> (Security, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store =
            veilvoice_crypto::LockStore::create(&dir.path().join("applock.bin"), b"pw", weak())
                .unwrap();
        let mut s = Security::default();
        s.store = Some(store);
        (s, dir)
    }

    #[test]
    fn a_deliberate_lock_and_an_idle_one_are_told_apart() {
        let (mut s, _dir) = locked_security();
        s.lock_now();
        assert!(s.is_locked());
        assert!(
            !s.auto_locked,
            "somebody pressed Lock; the screen must not claim the window did it"
        );

        let (mut s, _dir) = locked_security();
        s.lock_after_idle();
        assert!(s.is_locked());
        assert!(s.auto_locked);
    }

    #[test]
    fn the_auto_lock_note_is_not_shown_once_typing_starts() {
        // The note is drawn under `self.auto_locked && self.entry.is_empty()`.
        // Read from the source rather than by driving egui, which needs a
        // context this test suite does not build.
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let start = source.find("pub fn unlock_screen").unwrap();
        let body = &source[start..start + 4000];
        assert!(
            body.contains("self.auto_locked && self.entry.is_empty()"),
            "the note must disappear as soon as a character is typed"
        );
    }

    #[test]
    fn the_lock_screen_shows_the_mark_in_its_badge() {
        let source = include_str!("security.rs").replace("\r\n", "\n");
        let start = source.find("pub fn unlock_screen").unwrap();
        let body = &source[start..start + 4000];
        assert!(
            body.contains("soundbar::badge"),
            "a locked window has one thing in it and it should be the logo"
        );
    }

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
