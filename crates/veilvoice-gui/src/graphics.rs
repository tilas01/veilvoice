// SPDX-License-Identifier: GPL-3.0-or-later
//! What the window is drawn with, asked for explicitly and then reported.
//!
//! # Why this is not left to a default
//!
//! Drawing went through whatever `eframe::NativeOptions::default()` happened
//! to choose. That was the right backend by luck rather than by decision: the
//! defaults are a property of the version of `eframe` in `Cargo.lock`, and an
//! upgrade can move them without a single line of this project changing. A
//! release that silently stopped using the GPU would look exactly like a
//! release that got slower for no reason.
//!
//! So the four choices that decide how a frame reaches the screen are named
//! here, with the reasoning beside each one, and a test asserts the values
//! rather than trusting them to stay put.
//!
//! # Hardware acceleration is preferred, not required
//!
//! `Preferred` asks the platform for a GPU context and accepts a software one
//! if it cannot give you a GPU. `Required` refuses to start without hardware.
//!
//! Required is the wrong choice here and it is worth saying why, because it
//! sounds like the stronger setting. A virtual machine, a remote desktop
//! session, a server with no graphics card and a laptop that has handed the
//! wrong adapter to a hybrid-graphics driver all refuse a hardware context.
//! On `Required` every one of those becomes a program that does not open at
//! all. A privacy tool that will not run is not more private, and somebody
//! anonymising a recording over SSH with X forwarding has a real reason to be
//! doing it there. Software rendering is slower and it works.
//!
//! # What is actually reported
//!
//! `describe` reads the context that was really created, not the request. It
//! uses `glow`'s parsed version, which is a safe call: this crate forbids
//! unsafe code and nothing here is worth making an exception for. The vendor
//! string it returns comes from the driver, so somebody reporting a slow
//! window can say which driver produced it, and 3.3 on Mesa and 4.6 on a
//! discrete card are different conversations.

/// The rendering backend.
///
/// OpenGL through `glow`. The alternative in `eframe` is `wgpu`, which reaches
/// Vulkan, Metal and Direct3D and is the better long-term answer; it also
/// pulls in a substantially larger dependency graph, and this project counts
/// what it depends on. OpenGL is present on every system in the target list,
/// including the three BSDs.
pub const BACKEND: &str = "OpenGL through glow";

/// Whether frames wait for the display.
///
/// On. Without it the window tears when it is dragged, which is the exact
/// complaint this work exists to fix, and an unbounded frame rate burns a core
/// to draw pictures nobody sees.
pub const VSYNC: bool = true;

/// Multisampling, off.
///
/// Everything drawn here is rectangles and monospace text on axis-aligned
/// pixel boundaries. MSAA costs a full multiple of the fill rate and would
/// have nothing to smooth.
pub const MULTISAMPLING: u16 = 0;

/// One line for the About tab, and for a bug report.
///
/// Given no context it says so rather than guessing: a window that never got
/// a GL context has a different problem from a slow one, and the two should
/// not read the same.
pub fn describe(gl: Option<&eframe::glow::Context>) -> String {
    use eframe::glow::HasContext as _;

    let Some(gl) = gl else {
        return "no OpenGL context".to_string();
    };
    let version = gl.version();
    let flavour = if version.is_embedded { " ES" } else { "" };
    let vendor = version.vendor_info.trim();
    if vendor.is_empty() {
        format!("OpenGL{flavour} {}.{}", version.major, version.minor)
    } else {
        // No punctuation between the number and the driver's own string:
        // that string commonly starts with "(Core Profile)", and a comma in
        // front of it reads as a typo rather than as a separator.
        format!(
            "OpenGL{flavour} {}.{} {vendor}",
            version.major, version.minor
        )
    }
}

/// The options the window is created with.
///
/// Takes the viewport rather than building it, because where the window opens
/// is `window`'s business and this is only about how it is painted.
pub fn options(viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport,
        vsync: VSYNC,
        multisampling: MULTISAMPLING,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gpu_is_asked_for_and_not_demanded() {
        let options = options(egui::ViewportBuilder::default());
        assert!(
            matches!(
                options.hardware_acceleration,
                eframe::HardwareAcceleration::Preferred
            ),
            "Required would refuse to open in a VM or over a remote desktop, \
             and Off would never use the GPU at all"
        );
    }

    #[test]
    fn frames_wait_for_the_display() {
        let options = options(egui::ViewportBuilder::default());
        assert!(options.vsync, "tearing while dragging is what VSYNC is for");
        assert_eq!(options.multisampling, MULTISAMPLING);
    }

    #[test]
    fn the_backend_is_named_rather_than_inherited() {
        let options = options(egui::ViewportBuilder::default());
        assert!(matches!(options.renderer, eframe::Renderer::Glow));
    }

    #[test]
    fn the_viewport_passes_through_untouched() {
        let viewport = egui::ViewportBuilder::default().with_title("VeilVoice");
        let options = options(viewport);
        assert_eq!(options.viewport.title.as_deref(), Some("VeilVoice"));
    }

    #[test]
    fn no_context_is_reported_as_no_context() {
        // A window that never got a context and a window that got a slow one
        // are different bug reports, so they must not read the same.
        assert_eq!(describe(None), "no OpenGL context");
        assert!(
            !describe(None).chars().any(|c| c.is_ascii_digit()),
            "a version number here would read as a context that exists"
        );
    }

    #[test]
    fn the_backend_line_names_what_a_reader_can_check() {
        assert!(BACKEND.contains("OpenGL"));
        assert!(BACKEND.contains("glow"));
    }
}
