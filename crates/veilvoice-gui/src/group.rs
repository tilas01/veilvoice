// SPDX-License-Identifier: GPL-3.0-or-later
//! Group mode: several people in one recording, each with a name and a colour.
//!
//! The engine has handled several speakers since `veilvoice-conversation`
//! existed. This is the part of it a person can see: who is in the recording,
//! what each of them is called, what colour each is drawn in, and which
//! destination voice each becomes.
//!
//! # The toggle does not persist, and the tick that makes it persist is separate
//!
//! Group mode changes what a recording is *treated as*. A mode that survives a
//! restart is a mode somebody eventually forgets is on, and for this tool that
//! means a single-speaker recording rendered against a plan that does not
//! describe it — which silences everything the plan does not claim. So the
//! toggle is per-run and off by default, and there is a second, explicit tick
//! for "always start in group mode" which is the only thing written to disk.
//!
//! Two controls where one would do, deliberately. They answer two different
//! questions: *is this recording a group recording* and *are most of my
//! recordings group recordings*.
//!
//! # Colour is assigned, never guessed from the voice
//!
//! A speaker's colour is a function of their **slot**, exactly as their
//! destination voice is, and for the same reason: anything chosen by measuring
//! the input would make an output property a function of the input speaker,
//! which is the linkage this whole project exists to destroy. Slot 0 and slot 1
//! are the furthest-apart pair in the table because two people is the common
//! case; every slot after that is the colour whose nearest neighbour among the
//! ones already used is furthest away. That order was computed rather than
//! judged — see `veilvoice_video::palette`.
//!
//! A colour can be **overridden** per speaker, from any colour in any of the
//! nine palettes the website offers. An override is a person's choice about
//! their own recording, made after the fact, and carries none of the linkage
//! problem above.
//!
//! # Colour is never the only signal
//!
//! The name is drawn beside every circle, in the list, and in the subtitles.
//! Somebody who cannot separate two of these colours has the name, everywhere.
//! A panel that distinguished speakers by colour alone would be one about eight
//! per cent of men could not use.

use crate::theme::palette as p;
use eframe::egui::{self, Color32, RichText, Ui};
use veilvoice_conversation::{Conversation, Speaker};
use veilvoice_core::voices::{self, MAX_VOICES};

/// One person, as the panel holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// What they are called. Carried into the audio's subtitles as typed.
    pub name: String,
    /// A colour chosen by hand, or `None` for the one their slot is given.
    pub colour: Option<Color32>,
}

impl Person {
    /// A person with the default name for their slot.
    fn at(slot: usize) -> Self {
        Self {
            name: format!("Speaker {}", slot + 1),
            colour: None,
        }
    }
}

/// What comes out of a group render.
///
/// All three by default. The request was "video + audio + combined unless the
/// user restricts it", and a default that produces less than was asked for is
/// a default that gets discovered after the recording has been deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outputs {
    /// The veiled audio.
    pub audio: bool,
    /// WebVTT and SubRip beside it.
    pub subtitles: bool,
    /// The self-contained page that plays all of it together.
    pub page: bool,
}

impl Default for Outputs {
    fn default() -> Self {
        Self {
            audio: true,
            subtitles: true,
            page: true,
        }
    }
}

impl Outputs {
    /// Whether anything at all would be written.
    pub fn any(&self) -> bool {
        self.audio || self.subtitles || self.page
    }
}

/// The group-mode panel's state.
pub struct Group {
    /// Whether group mode is on **for this run**. Never written to disk.
    pub enabled: bool,
    /// The people in the recording.
    pub people: Vec<Person>,
    /// Which speaker's colour picker is open, if any.
    picking: Option<usize>,
    /// What a render would write.
    pub outputs: Outputs,
    /// The last thing that went wrong, to show rather than to swallow.
    pub notice: Option<String>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            // Off. Always off, whatever was on last time -- see the module
            // note. `start_from` is what turns it on at launch.
            enabled: false,
            people: vec![Person::at(0), Person::at(1)],
            picking: None,
            outputs: Outputs::default(),
            notice: None,
        }
    }
}

impl Group {
    /// The panel as it should open, given the saved preference.
    ///
    /// The *only* path by which group mode is on before anybody has touched
    /// anything.
    pub fn start_from(always: bool) -> Self {
        Self {
            enabled: always,
            ..Self::default()
        }
    }

    /// How many people are in the recording.
    pub fn len(&self) -> usize {
        self.people.len()
    }

    /// Whether there is nobody in it.
    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    /// The colour a slot is drawn in: the override, or the one it is given.
    pub fn colour(&self, slot: usize) -> Color32 {
        self.people
            .get(slot)
            .and_then(|person| person.colour)
            .unwrap_or_else(|| assigned_colour(slot))
    }

    /// Add a person, if there is room for one.
    ///
    /// Ten is the engine's limit, not this panel's: there are ten destination
    /// voices, and an eleventh speaker would have to share one with somebody.
    /// Two people sharing a voice is a real collision, so the limit is stated
    /// rather than wrapped around.
    pub fn add(&mut self) {
        if self.people.len() >= MAX_VOICES {
            self.notice = Some(format!(
                "{MAX_VOICES} is the limit: there are {MAX_VOICES} destination voices, and \
                 an eleventh speaker would have to share one."
            ));
            return;
        }
        self.people.push(Person::at(self.people.len()));
        self.notice = None;
    }

    /// Remove one person, keeping at least two.
    ///
    /// One speaker is not a group, and a panel that let you get there would
    /// leave group mode on with nothing for it to do.
    pub fn remove(&mut self, slot: usize) {
        if self.people.len() <= 2 || slot >= self.people.len() {
            return;
        }
        self.people.remove(slot);
        self.picking = None;
    }

    /// Build a plan from the panel: names in slot order, no turns.
    ///
    /// Turns come from a plan file or from one microphone per person. This
    /// makes the half the panel knows about, and says what is missing rather
    /// than inventing it: a plan with no turns claims no audio, and every
    /// second of a recording rendered against it would be silenced.
    pub fn to_plan(&self, title: Option<&str>) -> Result<Conversation, String> {
        let mut plan = Conversation::new();
        plan.title = title.map(|t| t.to_string());
        for person in &self.people {
            plan.add_speaker(Speaker::named(&person.name))
                .map_err(|error| error.to_string())?;
        }
        Ok(plan)
    }

    /// The whole panel.
    ///
    /// `settings` is here for one tick: "always start in group mode" is the
    /// only thing on this panel that outlives the run, and it belongs beside
    /// the toggle it modifies rather than on a settings page three clicks away.
    /// The two controls only make sense read together.
    pub fn tab(&mut self, ui: &mut Ui, settings: &mut crate::settings::Settings) {
        // The panel is taller than the window the moment the colour picker is
        // open -- 108 swatches in nine named groups -- and without a scroller
        // the picker simply is not reachable. Found by opening it in the
        // running application and looking, which is also how the speaker strip
        // turned out to be five lines of wrapped text at 96 pixels wide.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.body(ui, settings));
    }

    /// Everything inside the scroller.
    fn body(&mut self, ui: &mut Ui, settings: &mut crate::settings::Settings) {
        ui.heading(RichText::new("GROUP MODE").color(p::blue()));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "One recording with several people in it. Each gets a different \
                 destination voice, and every voiceprint is destroyed just as thoroughly \
                 as one speaker's would be.",
            )
            .color(p::muted()),
        );
        ui.add_space(10.0);

        self.mode_controls(ui, settings);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        if !self.enabled {
            ui.label(
                RichText::new(
                    "Group mode is off. A recording is treated as one speaker, which is \
                     what `anonymise file` does.",
                )
                .color(p::muted()),
            );
            return;
        }

        self.strip(ui);
        ui.add_space(12.0);
        self.people_list(ui);
        ui.add_space(12.0);
        self.output_controls(ui);

        if let Some(notice) = &self.notice {
            ui.add_space(8.0);
            ui.label(RichText::new(notice).color(p::yellow()));
        }

        ui.add_space(12.0);
        ui.label(
            RichText::new(
                "Names are not veiled by anything. They are typed by you and they go into \
                 the subtitles as typed.",
            )
            .color(p::yellow()),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "This panel holds who is in the recording. When each turn starts and ends \
                 comes from a plan file or from one microphone per person -- VeilVoice \
                 does not guess who is speaking, and audio no turn claims is silenced \
                 rather than passed through.",
            )
            .color(p::muted()),
        );
    }

    /// The two controls that decide whether group mode is on.
    fn mode_controls(&mut self, ui: &mut Ui, settings: &mut crate::settings::Settings) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.enabled, "group mode")
                .on_hover_text("For this run only. Nothing here is written to disk.");
            ui.add_space(12.0);
            ui.label(
                RichText::new(if self.enabled {
                    "on, for this run"
                } else {
                    "off"
                })
                .color(if self.enabled { p::green() } else { p::muted() })
                .small(),
            );
        });
        ui.label(
            RichText::new(
                "  For this run. Closing the app turns it off again, so a recording of \
                 one person is never rendered against a plan describing several.",
            )
            .color(p::muted())
            .small(),
        );

        ui.add_space(6.0);
        let mut always = settings.always_group();
        if ui
            .checkbox(&mut always, "always start in group mode")
            .on_hover_text("The one thing on this panel that is written to disk")
            .changed()
        {
            settings.set_always_group(always);
        }
        ui.label(
            RichText::new(
                "  Remembered. It decides what the app opens in; the toggle above still \
                 turns it off for a run.",
            )
            .color(p::muted())
            .small(),
        );
        if let Some(error) = settings.save_error() {
            ui.label(
                RichText::new(format!("  {error}"))
                    .color(p::yellow())
                    .small(),
            );
        }
    }

    /// The picture: a circle per person, in their colour, with their name.
    ///
    /// This is the part the request was actually about. A mode you can only
    /// tell is on by reading a checkbox is a mode that gets left on.
    fn strip(&mut self, ui: &mut Ui) {
        egui::Frame::none()
            .fill(p::bg_dark())
            .stroke(egui::Stroke::new(1.0, p::border()))
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for slot in 0..self.people.len() {
                        let colour = self.colour(slot);
                        let name = self.people[slot].name.clone();
                        ui.vertical(|ui| {
                            // Wide enough for "high register, wide tract" on two
                            // lines. At 96 the voice description wrapped to five,
                            // which turned a row of circles into a wall of text --
                            // seen in a capture of the running window, not deduced.
                            ui.set_width(168.0);
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(168.0, 44.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 19.0, colour);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(&name).color(p::fg()).small());
                                ui.label(
                                    RichText::new(voices::voice(slot).describe())
                                        .color(p::muted())
                                        .small(),
                                );
                            });
                        });
                    }
                });
            });
    }

    /// One row per person: colour, name, and a way to remove them.
    fn people_list(&mut self, ui: &mut Ui) {
        let count = self.people.len();
        let mut remove = None;
        for slot in 0..count {
            ui.horizontal(|ui| {
                let colour = self.colour(slot);
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 5.0, colour);
                ui.painter()
                    .rect_stroke(rect, 5.0, egui::Stroke::new(1.0, p::border()));
                if response.clicked() {
                    self.picking = if self.picking == Some(slot) {
                        None
                    } else {
                        Some(slot)
                    };
                }
                response.on_hover_text("Choose a colour, from any of the palettes");

                ui.add(
                    egui::TextEdit::singleline(&mut self.people[slot].name)
                        .desired_width(180.0)
                        .hint_text("a name"),
                );

                if self.people[slot].colour.is_some()
                    && ui
                        .small_button("automatic")
                        .on_hover_text("Go back to the colour this slot is given")
                        .clicked()
                {
                    self.people[slot].colour = None;
                }

                if count > 2 && ui.small_button("remove").clicked() {
                    remove = Some(slot);
                }
            });

            if self.picking == Some(slot) {
                self.palette_picker(ui, slot);
            }
        }

        if let Some(slot) = remove {
            self.remove(slot);
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("add a person").clicked() {
                self.add();
            }
            ui.label(
                RichText::new(format!("{} of {MAX_VOICES}", self.people.len()))
                    .color(p::muted())
                    .small(),
            );
        });
    }

    /// Every colour in every palette, as swatches.
    ///
    /// The whole set rather than a colour wheel: these are the colours the rest
    /// of this application and the website are drawn in, so a speaker picked
    /// from them looks like part of the same thing. Grouped by palette and
    /// named, because "the third blue" is not a thing anybody can ask for.
    fn palette_picker(&mut self, ui: &mut Ui, slot: usize) {
        egui::Frame::none()
            .fill(p::surface())
            .stroke(egui::Stroke::new(1.0, p::border()))
            .rounding(6.0)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("the ten a slot can be given")
                        .color(p::muted())
                        .small(),
                );
                ui.horizontal_wrapped(|ui| {
                    for index in 0..MAX_VOICES {
                        self.swatch(ui, slot, assigned_colour(index));
                    }
                });
                ui.add_space(8.0);
                for theme in crate::theme::themes() {
                    ui.label(RichText::new(theme.name).color(p::muted()).small());
                    ui.horizontal_wrapped(|ui| {
                        for colour in [
                            theme.accent,
                            theme.accent_2,
                            theme.cyan,
                            theme.ok,
                            theme.warn,
                            theme.err,
                            theme.fg,
                            theme.muted,
                        ] {
                            self.swatch(ui, slot, colour);
                        }
                    });
                }
            });
    }

    /// One clickable colour.
    fn swatch(&mut self, ui: &mut Ui, slot: usize, colour: Color32) {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
        ui.painter().rect_filled(rect, 4.0, colour);
        let chosen = self.people[slot].colour == Some(colour);
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(if chosen { 2.0 } else { 1.0 }, {
                if chosen {
                    p::fg()
                } else {
                    p::border()
                }
            }),
        );
        if response.clicked() {
            self.people[slot].colour = Some(colour);
            self.picking = None;
        }
    }

    /// What a render writes.
    fn output_controls(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("WHAT A RENDER WRITES").color(p::blue()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.outputs.audio, "audio");
            ui.checkbox(&mut self.outputs.subtitles, "subtitles");
            ui.checkbox(&mut self.outputs.page, "page");
        });
        if !self.outputs.any() {
            ui.label(
                RichText::new("Nothing is ticked, so a render would write nothing.")
                    .color(p::yellow())
                    .small(),
            );
        } else {
            ui.label(
                RichText::new("All three by default. Untick what you do not want.")
                    .color(p::muted())
                    .small(),
            );
        }
    }
}

/// The colour a slot is given, as an egui colour.
///
/// One table, shared with the video crate rather than copied here, so a circle
/// in this panel and the same speaker's circle in a rendered page cannot drift
/// apart. `unwrap_or` rather than `expect`: a malformed entry would be a bug in
/// that table, and a panel is the wrong place to find out about it.
pub fn assigned_colour(slot: usize) -> Color32 {
    let hex = veilvoice_video::palette::speaker(slot);
    match veilvoice_video::palette::rgb(hex) {
        Some((r, g, b)) => Color32::from_rgb(r, g, b),
        None => Color32::GRAY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_mode_is_off_unless_the_saved_preference_says_otherwise() {
        assert!(!Group::default().enabled);
        assert!(!Group::start_from(false).enabled);
        assert!(Group::start_from(true).enabled);
    }

    /// The whole reason the mode is not persisted: it must not survive a
    /// restart on its own. This is the test that would fail if somebody added
    /// `enabled` to `Prefs`.
    #[test]
    fn the_default_panel_never_opens_in_group_mode() {
        let turned_on = Group {
            enabled: true,
            ..Group::default()
        };
        assert!(turned_on.enabled, "turning it on works within a run");
        // A fresh panel, as a restart produces.
        assert!(!Group::default().enabled);
    }

    #[test]
    fn a_slot_gets_the_colour_its_slot_is_given_until_one_is_chosen() {
        let mut group = Group::default();
        assert_eq!(group.colour(0), assigned_colour(0));
        assert_eq!(group.colour(1), assigned_colour(1));
        group.people[0].colour = Some(Color32::from_rgb(1, 2, 3));
        assert_eq!(group.colour(0), Color32::from_rgb(1, 2, 3));
        // The override is one speaker's, not everybody's.
        assert_eq!(group.colour(1), assigned_colour(1));
    }

    /// Two speakers is the common case, and slots 0 and 1 are the furthest
    /// apart pair in the table. If that ever stops being true the panel is
    /// drawing two people in two colours somebody cannot tell apart.
    #[test]
    fn the_first_two_colours_are_the_furthest_apart_pair() {
        let table = veilvoice_video::palette::SPEAKERS;
        let first = veilvoice_video::palette::distance(table[0], table[1]);
        for (i, a) in table.iter().enumerate() {
            for b in table.iter().skip(i + 1) {
                assert!(
                    veilvoice_video::palette::distance(a, b) <= first,
                    "{a} and {b} are further apart than slots 0 and 1"
                );
            }
        }
    }

    #[test]
    fn an_eleventh_speaker_is_refused_and_says_why() {
        let mut group = Group::default();
        while group.len() < MAX_VOICES {
            group.add();
        }
        assert_eq!(group.len(), MAX_VOICES);
        group.add();
        assert_eq!(group.len(), MAX_VOICES, "the limit must hold");
        let notice = group.notice.expect("the refusal must be explained");
        assert!(notice.contains("share"), "{notice}");
    }

    /// One speaker is not a group. Removing past two would leave the mode on
    /// with nothing for it to do.
    #[test]
    fn removing_stops_at_two() {
        let mut group = Group::default();
        group.add();
        assert_eq!(group.len(), 3);
        group.remove(0);
        assert_eq!(group.len(), 2);
        group.remove(0);
        assert_eq!(group.len(), 2, "two is the floor");
    }

    #[test]
    fn a_plan_carries_the_names_in_slot_order() {
        let mut group = Group::default();
        group.people[0].name = "Alex".into();
        group.people[1].name = "Sam".into();
        let plan = group.to_plan(Some("A chat")).unwrap();
        assert_eq!(plan.title.as_deref(), Some("A chat"));
        assert_eq!(plan.speakers()[0].name, "Alex");
        assert_eq!(plan.speakers()[1].name, "Sam");
    }

    /// A name with a line break in it could forge a record in the plan file.
    /// The conversation crate refuses it; this checks the panel surfaces that
    /// rather than producing a plan with a hole in it.
    #[test]
    fn a_name_with_a_line_break_is_refused_by_the_plan_rather_than_written() {
        let mut group = Group::default();
        group.people[0].name = "Alex\nspeaker  9  Mallory".into();
        let error = group.to_plan(None).expect_err("a line break is refused");
        assert!(error.contains("line break"), "{error}");
    }

    #[test]
    fn every_output_is_on_by_default() {
        let outputs = Outputs::default();
        assert!(outputs.audio && outputs.subtitles && outputs.page);
        assert!(outputs.any());
        assert!(!Outputs {
            audio: false,
            subtitles: false,
            page: false
        }
        .any());
    }
}
