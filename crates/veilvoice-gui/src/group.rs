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
use std::path::PathBuf;
use std::sync::mpsc;
use veilvoice_conversation::mode::{self as voice_mode, VoiceMode};
use veilvoice_conversation::{Conversation, Speaker};
use veilvoice_core::voices::{self, MAX_VOICES};
use veilvoice_core::DeidConfig;
use veilvoice_video::palette::Palette;
use veilvoice_workspace::{Member, Workspace};

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

    /// The ticked ones, by name, for a project file.
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.audio {
            out.push("audio".to_string());
        }
        if self.subtitles {
            out.push("subtitles".to_string());
        }
        if self.page {
            out.push("page".to_string());
        }
        out
    }

    /// Read back from a project file.
    ///
    /// An unrecognised name is ignored rather than refused: the file's own
    /// parser has already refused anything structurally strange, and an output
    /// this build cannot write is a thing it simply does not write.
    pub fn from_names(names: &[String]) -> Self {
        Self {
            audio: names.iter().any(|n| n == "audio"),
            subtitles: names.iter().any(|n| n == "subtitles"),
            page: names.iter().any(|n| n == "page"),
        }
    }
}

/// An egui colour as `#rrggbb`.
fn hex_of(colour: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.r(), colour.g(), colour.b())
}

/// `#rrggbb` as an egui colour, or `None` for anything that is not one.
fn colour_of(text: &str) -> Option<Color32> {
    veilvoice_video::palette::rgb(text).map(|(r, g, b)| Color32::from_rgb(r, g, b))
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

    /// The recording.
    pub input: Option<PathBuf>,
    /// The plan holding who speaks when. Without it nothing can be rendered:
    /// this panel knows *who*, and a plan is the only honest source of *when*.
    pub plan: Option<PathBuf>,
    /// A title for the page, if the plan does not carry one.
    pub title: String,
    /// Which palette a page is drawn in. Tokyo Night unless changed, and
    /// **not persisted** -- the same shape as the mode toggle above it.
    pub theme: &'static Palette,
    /// A voice each, or one voice between everybody.
    pub voices: VoiceMode,
    /// Which named way of working this panel is set up as.
    pub profile: String,
    /// Where this project was last saved or loaded, if anywhere.
    pub project: Option<PathBuf>,

    /// The worker, while a render is running.
    job: Option<mpsc::Receiver<Result<Vec<PathBuf>, String>>>,
    /// The last render's result, kept until another is started.
    report: Option<Result<Vec<PathBuf>, String>>,
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
            input: None,
            plan: None,
            title: String::new(),
            theme: veilvoice_video::palette::default_palette(),
            voices: VoiceMode::default(),
            profile: veilvoice_workspace::default_profile().id.to_string(),
            project: None,
            job: None,
            report: None,
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
        // The limit is whatever the *mode* can carry, and for a voice each that
        // is how many voices are far enough apart to be told apart -- measured
        // under the engine's configuration, not a number typed here.
        if let Err(why) =
            voice_mode::check(self.people.len() + 1, self.voices, &DeidConfig::default())
        {
            self.notice = Some(why.to_string());
            return;
        }
        self.people.push(Person::at(self.people.len()));
        self.notice = None;
    }

    /// How many people this panel can hold in its current mode.
    pub fn limit(&self) -> usize {
        self.voices.speaker_limit(&DeidConfig::default())
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

        self.profile_controls(ui);
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

        self.voice_mode_controls(ui);
        ui.add_space(12.0);
        self.strip(ui);
        ui.add_space(12.0);
        self.people_list(ui);
        ui.add_space(12.0);
        self.output_controls(ui);
        ui.add_space(12.0);
        self.files_and_theme(ui);
        ui.add_space(12.0);
        self.render_controls(ui);

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

    /// The named ways of working, and the project this panel came from.
    ///
    /// A profile is a *starting point*, not a lock: picking one sets the
    /// controls below and then leaves them alone. Anything else would mean a
    /// preset quietly overriding a choice somebody made after picking it, and
    /// they would find that out in the output.
    fn profile_controls(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("HOW YOU ARE WORKING").color(p::blue()));
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            for one in veilvoice_workspace::BUILT_IN {
                let chosen = self.profile == one.id;
                if ui.selectable_label(chosen, one.name).clicked() && !chosen {
                    self.apply_profile(one);
                }
            }
        });

        if let Some(one) = veilvoice_workspace::profile(&self.profile) {
            ui.label(RichText::new(one.note).color(p::muted()).small());
        } else {
            ui.label(
                RichText::new(format!(
                    "This project was saved under a profile called {:?}, which this \
                     build does not have.",
                    self.profile
                ))
                .color(p::yellow())
                .small(),
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("open project…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("VeilVoice project", &["veilwork", "txt"])
                    .pick_file()
                {
                    match Workspace::load(&path) {
                        Ok(work) => {
                            self.from_workspace(&work);
                            self.project = Some(path);
                        }
                        Err(why) => self.notice = Some(why.to_string()),
                    }
                }
            }
            if ui.button("save project…").clicked() {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("VeilVoice project", &["veilwork"])
                    .set_file_name("project.veilwork");
                if let Some(previous) = self.project.as_ref().and_then(|p| p.parent()) {
                    dialog = dialog.set_directory(previous);
                }
                if let Some(path) = dialog.save_file() {
                    match self.to_workspace().save(&path) {
                        Ok(()) => {
                            self.project = Some(path);
                            self.notice = None;
                        }
                        Err(why) => self.notice = Some(why.to_string()),
                    }
                }
            }
            ui.label(
                RichText::new(match &self.project {
                    Some(path) => path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string()),
                    None => "not saved".to_string(),
                })
                .color(p::muted())
                .small(),
            );
        });
        ui.label(
            RichText::new(
                "  A project holds where your files are, who is in the recording and what \
                 you called them. It holds no audio and no passwords, so it is safe to \
                 keep beside the recording -- but it does hold the names you typed.",
            )
            .color(p::muted())
            .small(),
        );
    }

    /// Set the controls this profile names, and change nothing else.
    fn apply_profile(&mut self, one: &veilvoice_workspace::Profile) {
        self.profile = one.id.to_string();
        self.enabled = one.group;
        // A profile that asks for a voice each cannot be applied to more people
        // than there are voices. Said rather than trimming somebody out of the
        // group to make the preset fit.
        match voice_mode::check(self.people.len(), one.voices, &DeidConfig::default()) {
            Ok(()) => {
                self.voices = one.voices;
                self.notice = None;
            }
            Err(why) => {
                self.notice = Some(format!(
                    "{}. The rest of the profile has been applied.",
                    why
                ));
            }
        }
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

    /// A voice each, or one voice between everybody.
    ///
    /// The second is more private and is not the default, which is the honest
    /// way round: it removes a real trace -- *which* speaker somebody was --
    /// and it costs the ability to follow the recording by ear. Most people
    /// want the first, and the ones who want the second want it for a reason
    /// they already know.
    fn voice_mode_controls(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("VOICES").color(p::blue()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for mode in [VoiceMode::Distinct, VoiceMode::Uniform] {
                if ui
                    .selectable_label(self.voices == mode, mode.label())
                    .clicked()
                    && self.voices != mode
                {
                    // Switching *to* a mode that cannot carry this many people
                    // is refused, with the same words the engine would use.
                    match voice_mode::check(self.people.len(), mode, &DeidConfig::default()) {
                        Ok(()) => {
                            self.voices = mode;
                            self.notice = None;
                        }
                        Err(why) => self.notice = Some(why.to_string()),
                    }
                }
            }
        });
        ui.label(RichText::new(self.voices.note()).color(p::muted()).small());

        let clear = voices::clear_voices(&DeidConfig::default());
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "  {clear} of the {MAX_VOICES} destination voices are far enough apart to \
                 be told apart by ear -- measured, not chosen. Past that, one voice for \
                 everybody is the honest option.",
            ))
            .color(p::muted())
            .small(),
        );
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
                                    RichText::new(self.voices.voice_for(slot).describe())
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
                RichText::new(format!("{} of {}", self.people.len(), self.limit()))
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

    /// This panel as a saveable project.
    pub fn to_workspace(&self) -> Workspace {
        let mut work = Workspace::new();
        work.title = (!self.title.trim().is_empty()).then(|| self.title.trim().to_string());
        work.input = self.input.clone();
        work.plan = self.plan.clone();
        work.profile = self.profile.clone();
        work.theme = self.theme.id.to_string();
        work.outputs = self.outputs.names();
        work.members = self
            .people
            .iter()
            .map(|person| Member {
                name: person.name.clone(),
                colour: person.colour.map(hex_of),
            })
            .collect();
        work
    }

    /// Put a saved project back.
    ///
    /// Anything this build does not recognise -- a profile from a newer
    /// version, a palette that has been renamed -- is **reported and left
    /// alone** rather than quietly replaced. Loading a project and silently
    /// getting different settings is the failure worth avoiding here: the whole
    /// point of the file is that it puts things back where they were.
    pub fn from_workspace(&mut self, work: &Workspace) {
        let mut notes: Vec<String> = Vec::new();

        self.title = work.title.clone().unwrap_or_default();
        self.input = work.input.clone();
        self.plan = work.plan.clone();

        match veilvoice_workspace::profile(&work.profile) {
            Some(found) => {
                self.profile = found.id.to_string();
                self.voices = found.voices;
                self.enabled = found.group;
            }
            None => notes.push(format!(
                "this project was saved under a profile called {:?}, which this build \
                 does not have. The settings it named have been left as they are.",
                work.profile
            )),
        }

        match veilvoice_video::palette::by_id(&work.theme) {
            Some(found) => self.theme = found,
            None => notes.push(format!(
                "this project's palette, {:?}, is not one this build has.",
                work.theme
            )),
        }

        self.outputs = Outputs::from_names(&work.outputs);

        if !work.members.is_empty() {
            self.people = work
                .members
                .iter()
                .map(|member| Person {
                    name: member.name.clone(),
                    colour: member.colour.as_deref().and_then(colour_of),
                })
                .collect();
        }

        // A saved project may hold more people than the current mode can carry
        // -- it was saved under a different one, or by a build with a different
        // measured limit. Said, not silently trimmed.
        if let Err(why) = voice_mode::check(self.people.len(), self.voices, &DeidConfig::default())
        {
            notes.push(why.to_string());
        }

        self.notice = (!notes.is_empty()).then(|| notes.join(" "));
    }

    /// Whether a render is running, so the window keeps repainting.
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Take the worker's answer if it has one. Called once a frame; never waits.
    pub fn drain(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(done) => {
                self.report = Some(done);
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.report = Some(Err("the render stopped without finishing".into()));
                self.job = None;
            }
        }
    }

    /// The recording, the plan, the title and the palette.
    fn files_and_theme(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("THE RECORDING").color(p::blue()));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("choose recording…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("audio", &["wav", "mp3", "flac", "ogg", "m4a", "aac"])
                    .pick_file()
                {
                    self.input = Some(path);
                }
            }
            ui.label(
                RichText::new(match &self.input {
                    Some(path) => file_name(path),
                    None => "no recording chosen".to_string(),
                })
                .color(if self.input.is_some() {
                    p::fg()
                } else {
                    p::muted()
                })
                .small(),
            );
        });

        ui.horizontal(|ui| {
            if ui.button("choose plan…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("plan", &["txt"])
                    .pick_file()
                {
                    self.plan = Some(path);
                }
            }
            ui.label(
                RichText::new(match &self.plan {
                    Some(path) => file_name(path),
                    None => "no plan chosen".to_string(),
                })
                .color(if self.plan.is_some() {
                    p::fg()
                } else {
                    p::muted()
                })
                .small(),
            );
        });

        // Said here rather than discovered afterwards. This panel knows who is
        // in the recording; only a plan knows when each of them speaks, and
        // audio no turn claims is silenced rather than passed through.
        ui.label(
            RichText::new(
                "  The plan says when each person speaks. Without one there is nothing to \
                 render against, and audio no turn claims is silenced rather than passed \
                 through -- so a missing plan would produce a silent file, not a veiled \
                 one. `veilvoice conversation inspect` describes a plan you already have.",
            )
            .color(p::muted())
            .small(),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("title").color(p::muted()).small());
            ui.add(
                egui::TextEdit::singleline(&mut self.title)
                    .desired_width(240.0)
                    .hint_text("taken from the plan if left empty"),
            );
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("page palette").color(p::muted()).small());
            // The same nine the website and this application offer, and the
            // same identifiers, so a page rendered here and a page rendered by
            // the command line in the same theme are the same picture.
            egui::ComboBox::from_id_salt("group-theme")
                .selected_text(self.theme.name)
                .show_ui(ui, |ui| {
                    for palette in veilvoice_video::palette::PALETTES {
                        ui.selectable_value(&mut self.theme, palette, palette.name);
                    }
                });
            ui.label(
                RichText::new("for this run; the app opens in Tokyo Night")
                    .color(p::muted())
                    .small(),
            );
        });
    }

    /// The button, and what came of pressing it.
    fn render_controls(&mut self, ui: &mut Ui) {
        let ready = self.input.is_some() && self.plan.is_some() && self.outputs.any();
        ui.horizontal(|ui| {
            let busy = self.is_busy();
            if ui
                .add_enabled(ready && !busy, egui::Button::new("render"))
                .clicked()
            {
                self.start();
            }
            if busy {
                ui.spinner();
                ui.label(RichText::new("rendering…").color(p::muted()).small());
            } else if !ready {
                ui.label(
                    RichText::new(if !self.outputs.any() {
                        "nothing is ticked to write"
                    } else if self.input.is_none() {
                        "choose a recording"
                    } else {
                        "choose a plan"
                    })
                    .color(p::muted())
                    .small(),
                );
            }
        });

        match &self.report {
            None => {}
            Some(Ok(written)) => {
                ui.add_space(6.0);
                for path in written {
                    ui.label(RichText::new(format!("wrote {}", path.display())).color(p::green()));
                }
                ui.label(
                    RichText::new(
                        "These files are not encrypted. The subtitles hold the names you \
                         typed, and nothing veils a name.",
                    )
                    .color(p::yellow())
                    .small(),
                );
            }
            Some(Err(why)) => {
                ui.add_space(6.0);
                ui.label(RichText::new(why).color(p::red()));
            }
        }
    }

    /// Start a render on a thread of its own.
    ///
    /// `update()` may read, paint and start work; it may never wait for any.
    /// A render reads a whole recording and runs the engine over it, which is
    /// seconds at best and the length of the file at worst.
    fn start(&mut self) {
        let (Some(input), Some(plan_path)) = (self.input.clone(), self.plan.clone()) else {
            return;
        };
        let names: Vec<String> = self.people.iter().map(|one| one.name.clone()).collect();
        let title = self.title.trim().to_string();
        let outputs = self.outputs;
        let theme = self.theme;
        let voices = self.voices;

        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.report = None;
        std::thread::spawn(move || {
            let _ = tx.send(render_now(
                &input, &plan_path, &names, &title, outputs, theme, voices,
            ));
        });
    }
}

/// The last component of a path, for showing beside a button.
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Do the render. Runs on a worker thread and touches no interface state.
///
/// The names come from the panel and the turns come from the plan, and the two
/// have to agree about how many people there are. A plan naming three speakers
/// rendered against a panel holding two would put one person's audio in
/// somebody else's voice, which is the one mistake here that cannot be heard in
/// the result -- so it is refused rather than reconciled.
fn render_now(
    input: &std::path::Path,
    plan_path: &std::path::Path,
    names: &[String],
    title: &str,
    outputs: Outputs,
    theme: &'static Palette,
    voices: VoiceMode,
) -> Result<Vec<PathBuf>, String> {
    use veilvoice_conversation::render::{self, Settings};
    use veilvoice_conversation::subtitles::{self, Format};

    let mut plan = Conversation::load(plan_path)
        .map_err(|error| format!("{}: {error}", plan_path.display()))?;
    if plan.len() != names.len() {
        return Err(format!(
            "the plan names {} speaker(s) and this panel holds {}. Rendering one against \
             the other would put somebody's audio in the wrong voice.",
            plan.len(),
            names.len()
        ));
    }
    // The panel's names win: they are what the person just typed, and the plan
    // may have been written before they renamed anybody. The *turns* are the
    // plan's and are untouched.
    plan.rename_speakers(names)
        .map_err(|error| error.to_string())?;
    // Set before anything is decoded: the refusal is cheap and it names the
    // way out, and there is no reason to read a whole recording first.
    plan.set_mode(voices, &DeidConfig::default())
        .map_err(|error| error.to_string())?;
    if !title.is_empty() {
        plan.title = Some(title.to_string());
    }

    let audio = veilvoice_audio::io::load(input).map_err(|error| error.to_string())?;
    let mut settings = Settings::default();
    settings.config.sample_rate = audio.sample_rate as f32;
    let rendered = render::render(&plan, &audio.samples, &settings, None)
        .map_err(|error| error.to_string())?;

    let mut base = input.to_path_buf();
    base.set_extension("veiled.wav");
    let mut written = Vec::new();

    let veiled = veilvoice_audio::io::Audio {
        samples: rendered.samples,
        sample_rate: audio.sample_rate,
    };
    if outputs.audio || outputs.page {
        // The page plays the audio, so it is written whenever the page is --
        // a player pointing at a file that was never written is worse than no
        // player.
        veilvoice_audio::io::save_wav(&base, &veiled).map_err(|error| error.to_string())?;
        written.push(base.clone());
    }

    let vtt = with_extension(&base, "vtt");
    if outputs.subtitles || outputs.page {
        std::fs::write(&vtt, subtitles::write(&plan, Format::WebVtt))
            .map_err(|error| format!("{}: {error}", vtt.display()))?;
        written.push(vtt.clone());
        let srt = with_extension(&base, "srt");
        std::fs::write(&srt, subtitles::write(&plan, Format::SubRip))
            .map_err(|error| format!("{}: {error}", srt.display()))?;
        written.push(srt);
    }

    if outputs.page {
        use veilvoice_video::{page, waveform};
        let look = page::Look::default().themed(theme);
        // The veiled audio's waveform, not the input's: a picture of the
        // original signal beside a file whose point is that the original is
        // gone would be the wrong picture.
        let envelope = waveform::envelope(&veiled.samples, 640);
        let drawn = page::player(&plan, &envelope, &look, &file_name(&base), &file_name(&vtt))
            .map_err(|error| error.to_string())?;
        let html = with_extension(&base, "html");
        std::fs::write(&html, drawn.markup)
            .map_err(|error| format!("{}: {error}", html.display()))?;
        written.push(html);
    }

    Ok(written)
}

/// Replace the last extension, keeping any `.veiled` before it.
fn with_extension(path: &std::path::Path, extension: &str) -> PathBuf {
    let mut out = path.to_path_buf();
    out.set_extension(extension);
    out
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

    /// The limit is the *measured* one -- how many voices can be told apart --
    /// not the ten the table holds.
    ///
    /// Written as a bounded loop rather than `while len < MAX_VOICES`, which is
    /// what it was: `add` now refuses at nine, so that loop never terminated
    /// and the test suite hung rather than failed. A test that cannot fail
    /// cannot pass either.
    #[test]
    fn a_ninth_speaker_is_refused_and_the_refusal_names_the_way_out() {
        let mut group = Group::default();
        for _ in 0..MAX_VOICES + 4 {
            group.add();
        }
        let clear = voices::clear_voices(&DeidConfig::default());
        assert_eq!(clear, 8, "the measured clear limit");
        assert_eq!(group.len(), clear, "the panel stops where the voices do");

        let notice = group.notice.clone().expect("the refusal must be explained");
        assert!(notice.contains("one voice for everybody"), "{notice}");
    }

    /// And switching to one voice lifts it, which is the point of the refusal
    /// naming that mode.
    #[test]
    fn one_voice_for_everybody_carries_more_people() {
        let mut group = Group {
            voices: VoiceMode::Uniform,
            ..Group::default()
        };
        for _ in 0..MAX_VOICES + 4 {
            group.add();
        }
        assert_eq!(group.len(), MAX_VOICES, "up to what a plan can hold");
        assert_eq!(group.limit(), MAX_VOICES);
    }

    /// Switching *back* with too many people is refused rather than silently
    /// dropping somebody or handing two people one voice.
    #[test]
    fn switching_back_to_a_voice_each_is_refused_when_there_are_too_many() {
        let config = DeidConfig::default();
        let mut group = Group {
            voices: VoiceMode::Uniform,
            ..Group::default()
        };
        for _ in 0..MAX_VOICES + 4 {
            group.add();
        }
        assert!(voice_mode::check(group.len(), VoiceMode::Distinct, &config).is_err());
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

    /// A render cannot start without both files and something to write. This
    /// is the state the button is disabled in, asserted rather than trusted to
    /// the interface.
    #[test]
    fn a_render_needs_a_recording_a_plan_and_an_output() {
        let mut group = Group::default();
        assert!(group.input.is_none() && group.plan.is_none());
        group.start();
        assert!(!group.is_busy(), "nothing to render");

        group.input = Some(PathBuf::from("talk.wav"));
        group.start();
        assert!(!group.is_busy(), "still no plan");
    }

    /// A worker that dies without answering must not leave the panel saying
    /// "rendering" forever.
    #[test]
    fn a_render_that_stops_without_finishing_is_reported() {
        let (tx, rx) = mpsc::channel::<Result<Vec<PathBuf>, String>>();
        drop(tx);
        let mut group = Group {
            job: Some(rx),
            ..Group::default()
        };
        group.drain();
        assert!(!group.is_busy());
        match group.report {
            Some(Err(ref why)) => assert!(why.contains("without finishing"), "{why}"),
            other => panic!("expected a reported failure, got {other:?}"),
        }
    }

    /// The palette is Tokyo Night unless changed, and is not persisted -- the
    /// same shape as the mode toggle above it.
    #[test]
    fn the_page_palette_starts_at_tokyo_night_every_time() {
        assert_eq!(Group::default().theme.id, "tokyo-night");
        assert_eq!(Group::start_from(true).theme.id, "tokyo-night");
    }

    /// A plan and a panel that disagree about how many people there are is
    /// refused, because the result would be somebody's audio in the wrong
    /// voice and nobody could hear it.
    #[test]
    fn rendering_against_a_plan_with_a_different_count_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.txt");
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("A")).unwrap();
        plan.add_speaker(Speaker::named("B")).unwrap();
        plan.add_speaker(Speaker::named("C")).unwrap();
        plan.save(&path).unwrap();

        let error = render_now(
            std::path::Path::new("nothing.wav"),
            &path,
            &["one".to_string(), "two".to_string()],
            "",
            Outputs::default(),
            veilvoice_video::palette::default_palette(),
            VoiceMode::default(),
        )
        .expect_err("three against two");
        assert!(error.contains("wrong voice"), "{error}");
        assert!(error.contains('3') && error.contains('2'), "{error}");
    }

    /// The whole point of a project file: what you had is what you get back.
    #[test]
    fn a_panel_round_trips_through_a_project_file() {
        let mut group = Group {
            enabled: true,
            title: "Two people".into(),
            input: Some(PathBuf::from("talk.wav")),
            plan: Some(PathBuf::from("plan.txt")),
            theme: veilvoice_video::palette::by_id("gruvbox").unwrap(),
            voices: VoiceMode::Uniform,
            profile: veilvoice_workspace::GROUP_ONE_VOICE.id.to_string(),
            outputs: Outputs {
                audio: true,
                subtitles: false,
                page: true,
            },
            ..Group::default()
        };
        group.people[0].name = "Alex".into();
        group.people[1].name = "Sam".into();
        group.people[1].colour = Some(Color32::from_rgb(0x73, 0xda, 0xca));

        let text = group.to_workspace().to_text();
        let read_back = Workspace::parse(&text).expect("should parse");

        let mut restored = Group::default();
        restored.from_workspace(&read_back);

        assert_eq!(restored.title, "Two people");
        assert_eq!(restored.input, group.input);
        assert_eq!(restored.plan, group.plan);
        assert_eq!(restored.theme.id, "gruvbox");
        assert_eq!(restored.voices, VoiceMode::Uniform);
        assert_eq!(restored.profile, group.profile);
        assert_eq!(restored.outputs, group.outputs);
        assert_eq!(restored.people.len(), 2);
        assert_eq!(restored.people[0].name, "Alex");
        assert_eq!(restored.people[1].name, "Sam");
        assert_eq!(restored.people[1].colour, group.people[1].colour);
        assert!(restored.enabled, "a group profile opens in group mode");
    }

    /// A colour chosen by hand survives; one left automatic stays automatic
    /// rather than being frozen into whatever it happened to look like.
    #[test]
    fn an_automatic_colour_is_saved_as_automatic() {
        let group = Group::default();
        assert!(group.people[0].colour.is_none());
        let work = group.to_workspace();
        assert!(work.members[0].colour.is_none());

        let mut restored = Group::default();
        restored.from_workspace(&work);
        assert!(
            restored.people[0].colour.is_none(),
            "it must come back automatic, not pinned to today's table"
        );
    }

    /// A profile this build does not have is reported and the settings left
    /// alone. Silently opening under different settings is the failure worth
    /// avoiding: the point of the file is that it puts things back.
    #[test]
    fn an_unknown_profile_is_reported_rather_than_silently_replaced() {
        let mut work = Workspace::new();
        work.profile = "from-a-newer-build".into();
        let mut group = Group::default();
        let before = group.voices;
        group.from_workspace(&work);
        assert_eq!(group.voices, before, "nothing may have changed");
        let notice = group.notice.expect("it has to say so");
        assert!(notice.contains("from-a-newer-build"), "{notice}");
    }

    /// And an unknown palette likewise.
    #[test]
    fn an_unknown_palette_is_reported_rather_than_silently_replaced() {
        let mut work = Workspace::new();
        work.theme = "solarised".into();
        let mut group = Group::default();
        group.from_workspace(&work);
        assert_eq!(group.theme.id, "tokyo-night");
        assert!(group.notice.unwrap().contains("solarised"));
    }

    /// A project saved with more people than the current mode can carry is
    /// loaded and *reported*, not trimmed. Dropping somebody out of a group to
    /// make a preset fit is a thing nobody would notice until the render.
    #[test]
    fn a_project_with_too_many_people_for_the_mode_is_reported_not_trimmed() {
        let mut work = Workspace::new();
        work.profile = veilvoice_workspace::GROUP_VOICES.id.to_string();
        work.members = (0..9)
            .map(|n| veilvoice_workspace::Member {
                name: format!("P{n}"),
                colour: None,
            })
            .collect();

        let mut group = Group::default();
        group.from_workspace(&work);
        assert_eq!(group.people.len(), 9, "everybody is still here");
        let notice = group.notice.expect("it has to say so");
        assert!(notice.contains("one voice for everybody"), "{notice}");
    }

    /// Picking a profile sets what it names and leaves everything else. A
    /// preset that overrode a later choice would be found out in the output.
    #[test]
    fn a_profile_sets_what_it_names_and_nothing_else() {
        let mut group = Group {
            title: "kept".into(),
            ..Group::default()
        };
        group.people[0].name = "Alex".into();

        group.apply_profile(&veilvoice_workspace::GROUP_ONE_VOICE);
        assert_eq!(group.voices, VoiceMode::Uniform);
        assert!(group.enabled);
        assert_eq!(group.title, "kept", "a profile is not a reset");
        assert_eq!(group.people[0].name, "Alex");
    }

    #[test]
    fn outputs_survive_being_named_and_read_back() {
        for outputs in [
            Outputs::default(),
            Outputs {
                audio: true,
                subtitles: false,
                page: false,
            },
            Outputs {
                audio: false,
                subtitles: false,
                page: false,
            },
        ] {
            assert_eq!(Outputs::from_names(&outputs.names()), outputs);
        }
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
