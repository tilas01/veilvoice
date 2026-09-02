// SPDX-License-Identifier: GPL-3.0-or-later
//! The live monitor: what is going in, and what is coming out, wherever you are.
//!
//! # Why this is not just the meters on the live tab
//!
//! The live tab has drawn an input and an output meter for some time, and they
//! are the right meters. What they were not was *visible*: they are inside one
//! panel, and the moment somebody switched to Group to set up an interview, or
//! to Settings, or to Monitor, the only picture of what their microphone was
//! doing went off screen while the audio carried on.
//!
//! That is the wrong way round for this feature in particular. Live scramble is
//! the mode where the thing being protected is happening *now*, in real time,
//! and where the two questions a person actually has are "is it hearing me" and
//! "is anything coming out". A meter you have to navigate to in order to answer
//! them is a meter that answers them late.
//!
//! So the monitor rides the window. It is on by default, it shows on every tab,
//! and it shows exactly two things plus their state: the level going in, and
//! the level coming out.
//!
//! # Two places it can sit, and one way to switch it off
//!
//! [`Style::Toolbar`] docks it to the bottom of the window, where it takes a
//! strip of height and never covers anything. [`Style::Overlay`] floats it over
//! the panel, bottom right, for somebody who would rather keep the full height
//! for the panel and accept that it sits on top of a corner of it.
//! [`Style::Off`] is offered because a strip somebody does not want is a strip
//! they will resent, and the live tab still has the full meters either way.
//!
//! The overlay is deliberately **not** click-through and **not** draggable: a
//! floating thing that moves is a floating thing somebody loses behind the
//! window edge, and this one has a close button that sets the preference
//! instead.
//!
//! # What it does not claim
//!
//! It shows levels. A level is not proof that the voice is being changed: a
//! working meter and a bypassed engine look identical, and saying so is the
//! difference between a monitor and a reassurance. What tells you the engine is
//! running is that the output is a voice that is not yours, which is what the
//! preview on the live tab is for.
//!
//! # In plain words
//!
//! A small strip along the bottom of the window showing how loud your voice is
//! going in and how loud the veiled voice is coming out, while live scramble is
//! running.
//!
//! It follows you around the application, because the moment you want it is the
//! moment you are doing something else and are not sure the microphone is still
//! working.
//!
//! It cannot tell you that the disguise is working. It can tell you that sound
//! is arriving and sound is leaving, which is the thing that usually goes
//! wrong.

use crate::theme::palette as p;
use egui::RichText;

/// Where the monitor sits, or whether it is shown at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// Docked along the bottom of the window. Covers nothing.
    #[default]
    Toolbar,
    /// Floating over the bottom right of the panel.
    Overlay,
    /// Not shown. The live tab still has the full meters.
    Off,
}

impl Style {
    /// A short name, for a picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Toolbar => "a strip along the bottom",
            Self::Overlay => "a card floating in the corner",
            Self::Off => "only on the live tab",
        }
    }

    /// What this choice costs and buys, in the words a front end should show.
    pub fn note(self) -> &'static str {
        match self {
            Self::Toolbar => {
                "A strip docked to the bottom of the window while live scramble \
                 is running, on every tab. It takes a little height and it \
                 covers nothing."
            }
            Self::Overlay => {
                "A small card floating over the bottom right corner while live \
                 scramble is running. Keeps the full height for the panel and \
                 sits on top of a corner of it."
            }
            Self::Off => {
                "The monitor is not shown. The live tab still has the full \
                 meters, so this means you see them when you are looking at \
                 that tab and not otherwise."
            }
        }
    }

    /// Every style, in the order a picker should offer them.
    pub const ALL: &'static [Style] = &[Style::Toolbar, Style::Overlay, Style::Off];

    /// The identifier written to the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Toolbar => "toolbar",
            Self::Overlay => "overlay",
            Self::Off => "off",
        }
    }

    /// Read a style back.
    ///
    /// An unrecognised value is the default rather than an error, and the
    /// default *shows* the monitor. Of the two ways to be wrong about a
    /// settings file this build cannot read, hiding the only picture of a live
    /// microphone is the worse one.
    pub fn from_key(key: &str) -> Style {
        Self::ALL
            .iter()
            .copied()
            .find(|style| style.key() == key)
            .unwrap_or_default()
    }
}

/// The smoothed levels the monitor and the live tab both draw.
///
/// One copy, updated once a frame from the session, because two copies is two
/// bars that disagree by a frame and one of them is always the one somebody is
/// looking at.
#[derive(Clone, Copy, Debug, Default)]
pub struct Levels {
    /// Smoothed input peak, 0 to 1.
    pub input: f32,
    /// Smoothed output peak, 0 to 1.
    pub output: f32,
    /// The highest input of the last moment or so.
    pub hold_input: f32,
    /// The highest output of the last moment or so.
    pub hold_output: f32,
    /// When the hold was last taken.
    hold_since: Option<std::time::Instant>,
    /// Whether either side has clipped since the session started.
    ///
    /// Sticky on purpose. Clipping is destructive and is over in a
    /// millisecond, so a warning that has gone before the person looks up was
    /// never given.
    pub clipped: bool,
}

/// How long a held peak stays up before it falls back to the current level.
const HOLD: std::time::Duration = std::time::Duration::from_millis(1500);

impl Levels {
    /// Take a new reading.
    pub fn update(&mut self, input_peak: f32, output_peak: f32) {
        use veilvoice_audio::meter;

        // Fall smoothly rather than flickering with every frame.
        self.input = (self.input * 0.7).max(input_peak);
        self.output = (self.output * 0.7).max(output_peak);

        if meter::clipping(input_peak) || meter::clipping(output_peak) {
            self.clipped = true;
        }

        // The hold rises at once and falls back after a second and a half,
        // rather than sticking: otherwise the mark slowly becomes a picture of
        // the loudest thing that ever happened.
        let expired = self
            .hold_since
            .map(|at| at.elapsed() >= HOLD)
            .unwrap_or(true);
        if expired || self.input >= self.hold_input || self.output >= self.hold_output {
            if expired {
                self.hold_input = self.input;
                self.hold_output = self.output;
            } else {
                self.hold_input = self.hold_input.max(self.input);
                self.hold_output = self.hold_output.max(self.output);
            }
            self.hold_since = Some(std::time::Instant::now());
        }
    }

    /// Back to nothing, for when a session stops.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A compact bar, for the strip. The full-height one lives on the live tab.
///
/// Same scale as `veilvoice_audio::meter`, so this bar, the live tab's bar and
/// the one `veilvoice live` draws in a terminal are the same bar at three
/// sizes. A monitor that used a scale of its own would be a fourth opinion
/// about the same number.
fn bar(ui: &mut egui::Ui, label: &str, peak: f32, hold: f32) {
    use veilvoice_audio::meter;

    ui.label(RichText::new(label).color(p::muted()).small());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 10.0), egui::Sense::hover());
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
        // Below -40 is room tone rather than speech. Muted, so a quiet room
        // does not read as a working microphone.
        p::muted()
    };
    let mut filled = rect;
    filled.set_width(rect.width() * meter::position(peak));
    painter.rect_filled(filled, 2.0, colour);

    if meter::position(hold) > meter::position(peak) {
        let x = rect.left() + rect.width() * meter::position(hold);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.5, p::fg()),
        );
    }
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, p::border()),
        egui::StrokeKind::Inside,
    );

    let text = if db <= meter::FLOOR_DB {
        " -inf".to_string()
    } else {
        format!("{db:>5.1}")
    };
    ui.label(
        RichText::new(text)
            .color(if meter::clipping(peak) {
                p::red()
            } else {
                p::muted()
            })
            .small(),
    );
}

/// What the reader did with the monitor this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing.
    None,
    /// The close button: switch the monitor off and remember it.
    Dismiss,
}

/// Draw the row itself. Shared by both styles so they cannot drift apart.
///
/// `preview` changes the word and the colour, and it is not cosmetic. A
/// preview goes to this machine's own output and a live session goes to
/// whatever is listening on the cable, and somebody who has those two the wrong
/// way round is either talking to a call in their own voice or talking to
/// nobody. The strip is the thing on screen, so the strip has to say which.
fn row(ui: &mut egui::Ui, levels: &Levels, preview: bool, closable: bool) -> Action {
    let mut action = Action::None;
    ui.horizontal(|ui| {
        if preview {
            ui.label(RichText::new("preview").color(p::yellow()).small())
                .on_hover_text("Going to this machine's output only. Nobody on a call hears this.");
        } else {
            ui.label(RichText::new("live").color(p::green()).small());
        }
        ui.add_space(6.0);
        bar(ui, "in", levels.input, levels.hold_input);
        ui.add_space(10.0);
        bar(ui, "out", levels.output, levels.hold_output);
        if levels.clipped {
            ui.add_space(8.0);
            ui.label(RichText::new("CLIPPED").color(p::red()).small())
                .on_hover_text(
                    "The signal reached full scale and was cut off. Turn the input \
                     level down; clipping cannot be undone afterwards.",
                );
        }
        if closable {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("close")
                    .on_hover_text(
                        "Hide this strip. The live tab keeps its meters, and \
                         Settings brings the strip back.",
                    )
                    .clicked()
                {
                    action = Action::Dismiss;
                }
            });
        }
    });
    action
}

/// Draw the monitor for this frame.
///
/// Call once per frame from the shell, after the tab strip and before the
/// panel, whether or not a session is running: this returns immediately when
/// there is nothing to show, so the caller has one line rather than a
/// condition it can get wrong in one place and not the other.
pub fn show(
    ctx: &egui::Context,
    style: Style,
    running: bool,
    preview: bool,
    levels: &Levels,
) -> Action {
    if !running || style == Style::Off {
        return Action::None;
    }
    let mut action = Action::None;
    match style {
        Style::Toolbar => {
            egui::TopBottomPanel::bottom("live_monitor").show(ctx, |ui| {
                ui.add_space(4.0);
                action = row(ui, levels, preview, true);
                ui.add_space(4.0);
            });
        }
        Style::Overlay => {
            egui::Area::new(egui::Id::new("live_monitor_overlay"))
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(p::surface())
                        .stroke(egui::Stroke::new(1.0, p::border()))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            action = row(ui, levels, preview, true);
                        });
                });
        }
        Style::Off => {}
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every style survives a round trip through the settings file.
    #[test]
    fn a_style_reads_back_as_itself() {
        for style in Style::ALL {
            assert_eq!(Style::from_key(style.key()), *style);
        }
    }

    /// An unreadable setting shows the monitor rather than hiding it.
    ///
    /// The same rule the notification style follows, for the same reason: of
    /// the two ways to be wrong about a file this build cannot parse, the one
    /// that hides the only picture of a live microphone is the worse one.
    #[test]
    fn an_unknown_setting_still_shows_something() {
        for nonsense in ["", "TOOLBAR", "left", "off ", "1"] {
            assert_eq!(Style::from_key(nonsense), Style::Toolbar, "{nonsense:?}");
        }
        // And the one value that does mean off still means off.
        assert_eq!(Style::from_key("off"), Style::Off);
    }

    /// Every style says what it is and what it costs.
    #[test]
    fn every_style_explains_itself() {
        for style in Style::ALL {
            assert!(!style.label().is_empty());
            assert!(style.note().len() > 40, "{}", style.key());
        }
    }

    /// Clipping stays reported once it has happened.
    #[test]
    fn clipping_is_sticky_because_it_is_over_in_a_millisecond() {
        let mut levels = Levels::default();
        levels.update(1.0, 0.1);
        assert!(levels.clipped);
        for _ in 0..100 {
            levels.update(0.01, 0.01);
        }
        assert!(
            levels.clipped,
            "a clip that scrolls past was never reported"
        );
        levels.clear();
        assert!(!levels.clipped, "stopping the session starts again");
    }

    /// The held peak is at least the current level, always.
    #[test]
    fn the_hold_never_sits_below_the_bar_it_marks() {
        let mut levels = Levels::default();
        for peak in [0.1f32, 0.9, 0.2, 0.05, 0.7] {
            levels.update(peak, peak / 2.0);
            assert!(levels.hold_input >= levels.input, "{levels:?}");
            assert!(levels.hold_output >= levels.output, "{levels:?}");
        }
    }
}
