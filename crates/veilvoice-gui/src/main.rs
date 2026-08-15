// SPDX-License-Identifier: GPL-3.0-or-later
//! VeilVoice desktop application entry point.
// No console window on Windows for a release build; a debug build keeps it so
// panics and `eprintln!` stay visible while developing.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![forbid(unsafe_code)]

/// The window icon, as raw 32x32 RGBA produced by `assets/generate.py`.
///
/// Raw rather than PNG so the application needs no image decoder just to draw
/// its own title bar.
const ICON_RGBA: &[u8] = include_bytes!("../../../assets/icon-32.rgba");
const ICON_SIZE: u32 = 32;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 620.0])
        .with_min_inner_size([560.0, 480.0])
        .with_title("VeilVoice");

    if ICON_RGBA.len() == (ICON_SIZE * ICON_SIZE * 4) as usize {
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData {
            rgba: ICON_RGBA.to_vec(),
            width: ICON_SIZE,
            height: ICON_SIZE,
        }));
    }

    eframe::run_native(
        "VeilVoice",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(veilvoice_gui::VeilVoiceApp::new(cc)))),
    )
}
