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
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;
pub mod prefs;
pub mod reduced_motion;
pub mod security;
pub mod settings;
pub mod soundbar;
pub mod theme;

pub use app::VeilVoiceApp;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
