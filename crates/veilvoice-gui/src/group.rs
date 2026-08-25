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
use veilvoice_conversation::{Conversation, Speaker};
use veilvoice_core::voices::{self, MAX_VOICES};
use veilvoice_video::palette::Palette;

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

        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.report = None;
        std::thread::spawn(move || {
            let _ = tx.send(render_now(
                &input, &plan_path, &names, &title, outputs, theme,
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
        )
        .expect_err("three against two");
        assert!(error.contains("wrong voice"), "{error}");
        assert!(error.contains('3') && error.contains('2'), "{error}");
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
