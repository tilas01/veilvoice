// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-gui
//!
//! The VeilVoice desktop application: an egui/eframe front-end, monospace
//! throughout — anonymise a file, scramble a microphone live, watch what is
//! listening, manage the app lock, choose how the app looks, and an about
//! panel that states the honest scope.
//!
//! The binary lives in `main.rs`; this library exists so the UI logic can be
//! unit tested without opening a window.
//!
//! That split is worth stating plainly, because it is the reason this crate has
//! tests at all: a binary crate cannot be unit tested, so everything with logic
//! in it -- the app lock's state machine, preference loading, palette
//! resolution, the reduced-motion decision -- lives here where a test can reach
//! it without a display server. `main.rs` holds only what genuinely needs a
//! window.
//!
//! # The modules
//!
//! | Module | What it owns |
//! |---|---|
//! | [`security`] | The unlock screen, the lock tab, and the at-rest controls |
//! | [`prefs`] | Preferences, and recovering from a corrupt preferences file |
//! | [`policy`] | Settings somebody has fixed, and the reason beside each one |
//! | [`settings`] | The settings tab |
//! | [`setup`] | Installing this copy, and the optional companions |
//! | [`theme`] | The palette, shared with the command-line front end |
//! | [`soundbar`] | The animated level meter |
//! | [`reduced_motion`] | Whether to animate at all |
//! | [`watchfeed`] | The device monitor, on a thread that is not this one |
//!
//! # Two rules this crate keeps
//!
//! **The user interface never softens a scope note.** Where a control has a
//! bound -- the app lock is a verifier and not disk encryption, tamper detection
//! detects rather than prevents -- the interface says so next to the control,
//! and tests fail the build if that text changes. Documentation nobody opens
//! does not protect anybody.
//!
//! **Animation is a preference that is honoured, not a decoration.**
//! [`reduced_motion`] resolves the platform's own setting alongside the user's
//! explicit choice, and the whole interface reads that answer rather than each
//! widget deciding for itself.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;
pub mod crashlog;
pub mod group;
pub mod palettes;
pub mod policy;
pub mod prefs;
pub mod reduced_motion;
pub mod security;
pub mod settings;
pub mod setup;
pub mod soundbar;
pub mod theme;
pub mod updates;
pub mod watchfeed;

pub use app::VeilVoiceApp;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
