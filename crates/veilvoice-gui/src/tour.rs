// SPDX-License-Identifier: GPL-3.0-or-later
//! The short walkthrough on a first run, and after an upgrade.
//!
//! # What it is for
//!
//! The window has nine tabs and nothing said what any of them were. Somebody
//! opening this for the first time met a tab strip and had to guess, and two
//! of the nine, Monitor and Lock, are not what their names suggest to a person
//! who has not read the documentation.
//!
//! So: a short stop on each, one sentence each, skippable at any point, and
//! gone for good once seen. It is the paragraph a person would have read in a
//! manual, offered at the moment they would have wanted it.
//!
//! # It sets the application up as well as describing it
//!
//! **Roadmap item 171.** Describing the tabs was all it did, and the things
//! worth deciding in the first minute of using VeilVoice -- a password on the
//! window, what happens to recordings as they are written, whether this copy
//! is installed -- were on tabs somebody had to go and find. A walkthrough
//! that says "there is a Lock tab" and stops there has told somebody where the
//! decision is rather than letting them take it.
//!
//! The setup stops come first, before the descriptions of the tabs, because a
//! passphrase is a decision and a tab is a paragraph: somebody who reads four
//! sentences and closes the window should have been offered the decisions
//! first. See [`Stage`] for the order and [`Already`] for what is left out.
//!
//! **It offers rather than acts, where acting is a commitment.** A password is
//! set here, because setting one is the whole point of asking and it can be
//! changed on the Lock tab afterwards. Installing this copy is not: it writes
//! to somewhere permanent, the Install tab says what it will write, and the
//! most a walkthrough does is put somebody in front of it. That is what
//! [`Outcome::Finished`] carries back.
//!
//! **It adapts rather than asking twice.** A stop with nothing left to offer
//! is dropped before the walkthrough starts rather than shown greyed out: a
//! walkthrough whose steps are mostly already answered teaches people to press
//! next without reading, which is how the one that mattered gets missed.
//!
//! # Why it comes back after an upgrade
//!
//! Only as far as what is new. A tour that replays in full on every upgrade is
//! a tour people learn to skip, and one that never comes back means something
//! added in a later release is never introduced to anybody who was already a
//! user.
//!
//! What is stored is the list of stops that were shown, not a "seen" flag and
//! not the version number. A flag cannot answer the question an upgrade asks,
//! and the version can only answer it indirectly: comparing versions tells you
//! *that* something changed, and the list tells you *what*, which is the thing
//! being shown. It also means a release that adds nothing shows nobody
//! anything, which is the common case and the right behaviour for it.
//!
//! That list is also what carries the setup to the people who most need it.
//! Somebody who has had VeilVoice installed for a year has every tab key
//! stored and none of the setup ones, because the setup stops did not exist
//! when they last toured. They are exactly the person who has never been
//! offered an app lock, so the next launch offers them the setup, once, and
//! not one word about the tabs they have been using all year.
//!
//! # Portable or installed
//!
//! One of the stops says which one this copy is, in those words, because it is
//! the question behind "where did my settings go" and "why is it not in my
//! menu".
//!
//! # It is drawn over the window, not instead of it
//!
//! **Roadmap item 171.** This used to replace the whole body of the window:
//! `update` drew the card and returned, so the tabs it was describing were not
//! on screen while it described them. A card saying what the Monitor tab is
//! for, with no Monitor tab visible anywhere, is a paragraph from a manual
//! rather than a tour of anything.
//!
//! Now it is an [`egui::Modal`]: the window draws normally underneath, dimmed,
//! and the card sits over it. The reader can see the tab strip the card is
//! talking about. The backdrop takes the clicks, so nothing behind it can be
//! pressed by accident, and the tab strip is drawn disabled as well so that it
//! looks as unavailable as it is.
//!
//! **Escape closes it and a click on the shade does not.** Those are usually
//! the same gesture on a modal, and they should not be here: a modal asks a
//! question and clicking away from it means "not now", where a tour is
//! something somebody is stepping through, and a misjudged click halfway along
//! should not throw away the half they have not seen. Escape is deliberate and
//! the buttons are deliberate. A stray click is not.
//!
//! **A passphrase typed here does not outlive the card it was typed into.**
//! Every way out of a stop wipes the fields, including the ways that did not
//! use them, because the field holds what somebody typed whether or not they
//! pressed the button.
//!
//! # It can be asked for again
//!
//! Settings, under Interface. The stored list decides whether the walkthrough
//! *offers* itself; it has nothing to say about whether somebody may ask for
//! it, and before this there was no way to ask. Somebody who skipped it on the
//! day they installed VeilVoice had no route back to it at all, which made the
//! skip button a permanent decision taken in the first thirty seconds.
//!
//! A person asking gets every stop that applies, including the ones they have
//! seen, because "show me that again" is not a question about what is new.

use crate::theme::palette as p;
use egui::{RichText, Ui};

/// How wide the card is allowed to get.
///
/// A reading measure rather than the window's width. At 1400 pixels a sentence
/// runs the whole way across and the eye loses the line coming back; the usual
/// advice is 60 to 80 characters and this is about 75 at the default size. The
/// window can be any width and the card should not get harder to read as it
/// grows.
const CARD_WIDTH: f32 = 720.0;

/// How much of the window's height the card may occupy before its text
/// scrolls.
///
/// A share rather than a number of pixels, because the thing being protected
/// is the shade around the card: a card that reaches the edges is not an
/// overlay any more, it is the replacement this stopped being. Below this the
/// text scrolls inside the card, so a short window never hides the buttons,
/// which is the same rule the tab bodies follow.
const CARD_HEIGHT_SHARE: f32 = 0.8;

/// One card: the tab it is about, and what that tab is for.
///
/// The keys match `Tab::key`, and `app.rs` has a test that every tab has a
/// card and every card has a tab, so a tab added without a sentence fails the
/// build rather than shipping unexplained.
pub const CARDS: &[(&str, &str, &str)] = &[
    (
        "file",
        "Anonymise file",
        "A recording in, the same words in a voice nobody owns out. Encrypted \
         at rest by default, because the words survive on purpose and a file \
         anybody can read is a transcript anybody can read.",
    ),
    (
        "group",
        "Group",
        "One recording with several people in it. Each gets a different voice, \
         so a listener can still follow who is who, and every voiceprint is \
         destroyed just as thoroughly.",
    ),
    (
        "studio",
        "Studio",
        "The same thing on a microphone as you speak, into a virtual cable \
         that other programs can listen to, and a locked vault to keep what \
         was said in if you want one. The vault opens with both of your \
         passphrases at once, the one on this application and the one on your \
         recordings, and neither on its own.",
    ),
    (
        "browser",
        "Browser",
        "What is in the vault. The names and dates are sealed with the \
         recordings, so a disk shows how many files there are and roughly how \
         large, and nothing about what any of them is.",
    ),
    (
        "monitor",
        "Monitor",
        "Not a level meter. It watches for another program picking up a real \
         microphone while you are being veiled, which is the way this can \
         quietly fail to protect you.",
    ),
    (
        "lock",
        "Lock",
        "A passphrase on this application, separate from the one on any \
         recording. Worth what a lock on a drawer is worth: it stops somebody \
         at your keyboard, not somebody with your disk. This tab also shows \
         what VeilVoice's own files looked like when it first ran, and whether \
         they still do. That record is taken without being asked for, and is \
         sealed with your passphrase if you set one here.",
    ),
    (
        "verify",
        "Verify",
        "Check that a VeilVoice download is the one that was published, using \
         the signature and, if you have GnuPG, your own copy of it as well.",
    ),
    (
        "settings",
        "Settings",
        "Theme, animation, autolock, and what the interface tells you. \
         Everything here is stored beside the application and goes nowhere.",
    ),
    (
        "install",
        "Install",
        "Put this copy somewhere permanent, or leave it where it is. Either \
         works. The tab takes itself away once there is nothing left to do.",
    ),
    (
        "about",
        "About",
        "Versions, what drew the window, what the lock covers, and somewhere \
         to report a fault.",
    ),
];

/// One stop on the walkthrough.
///
/// **Roadmap item 171.** The tour used to be a list of tabs and nothing else.
/// It is a walkthrough now, and the setup a new reader needs comes before the
/// description of what the tabs are, because a passphrase is a decision and a
/// tab is a paragraph: somebody who reads four sentences and closes the window
/// should have been offered the decisions first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// A passphrase on the window, and on VeilVoice's own files.
    AppLock,
    /// Sealing every recording with it.
    AtRest,
    /// A second passphrase that opens an empty VeilVoice.
    Decoy,
    /// Installed for good, or portable on purpose.
    Install,
    /// The headings in Settings, and what each one covers.
    SettingsHeadings,
    /// A tab, by its position in [`CARDS`].
    Tab(usize),
}

impl Stage {
    /// The name this stop is remembered by.
    ///
    /// Stored in the same list the tab keys have always been stored in, which
    /// is why the setup stops carry a prefix: one namespace, and a tab called
    /// `install` must not be confused with the stop about installing.
    ///
    /// These strings are written into somebody's settings file, so they are
    /// **fixed**. Renaming one is telling every existing reader they have not
    /// seen that stop, and the tour comes back.
    pub fn key(self) -> String {
        match self {
            Self::AppLock => "setup-app-lock".to_string(),
            Self::AtRest => "setup-at-rest".to_string(),
            Self::Decoy => "setup-decoy".to_string(),
            Self::Install => "setup-install".to_string(),
            Self::SettingsHeadings => "setup-settings".to_string(),
            Self::Tab(at) => CARDS[at].0.to_string(),
        }
    }

    /// Whether this stop has anything to say, given what is already set up.
    ///
    /// This is the "adapts to what is already set rather than asking again"
    /// half of roadmap item 171. A stop with nothing to offer is dropped before
    /// the tour starts rather than shown greyed out, because a walkthrough
    /// whose steps are mostly already answered teaches people to press next
    /// without reading, which is how the one that mattered gets missed.
    fn applies(self, already: &Already) -> bool {
        match self {
            // A lock that exists needs no offer. Everything the stop would say
            // about what it is worth is on the Lock tab, which the reader has
            // already been to.
            Self::AppLock => !already.app_lock,
            // The decoy passphrase is offered where one can be set. Nothing
            // yet can: `veilvoice_crypto::decoy` is written and tested and
            // nothing stores a pair, which is roadmap item 173's half of this.
            // The stop is here, with its words, so that item is a wiring
            // change rather than a second design.
            Self::Decoy => already.decoy_can_be_set,
            // The rest always have something to say. At-rest encryption is on
            // by default since roadmap item 170, so its stop reports rather than
            // asks, and the reader should be told once that every recording is
            // being encrypted. Which copy this is, and what the Settings
            // headings cover, are facts rather than questions.
            _ => true,
        }
    }
}

/// What is already true, so the tour can leave out what it would only repeat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Already {
    /// Whether a passphrase is set on the window.
    pub app_lock: bool,
    /// Whether this copy has been installed rather than run from a folder.
    pub installed: bool,
    /// Whether a decoy passphrase can be set at all.
    ///
    /// False in this release. See [`Stage::applies`].
    pub decoy_can_be_set: bool,
}

/// Every stop, in the order a reader meets them.
///
/// Built rather than written down, so a tab added to [`CARDS`] joins the tour
/// without anybody remembering to add it here.
pub fn stages() -> Vec<Stage> {
    let mut all = vec![
        Stage::AppLock,
        Stage::AtRest,
        Stage::Decoy,
        Stage::Install,
        Stage::SettingsHeadings,
    ];
    all.extend((0..CARDS.len()).map(Stage::Tab));
    all
}

/// Where the tour is up to.
#[derive(Default)]
pub struct Tour {
    /// Which stop is showing. `None` means it is not running.
    at: Option<usize>,
    /// The stops this run is showing.
    showing: Vec<Stage>,
    /// A passphrase being typed on the app-lock stop, and its confirmation.
    ///
    /// Held in `String`s only because that is what the text widget requires,
    /// and taken out of them the moment they are used. The same shape the
    /// first-run card uses, for the same reason.
    lock_entry: String,
    lock_repeat: String,
    /// Set once the lock has been asked for, so the stop stops offering and
    /// waits for the worker instead.
    lock_requested: bool,
}

/// Every stop's name, for storing once the tour has run.
///
/// The name is historical: this used to be the tab keys and nothing else, and
/// the settings file still calls the list `toured_tabs` for the reason
/// [`Stage::key`] gives. What it holds now is every stop.
pub fn all_keys() -> Vec<String> {
    stages().into_iter().map(|stage| stage.key()).collect()
}

impl Tour {
    /// Start the tour from the beginning, showing every stop that applies.
    pub fn start(&mut self, already: &Already) {
        self.showing = stages()
            .into_iter()
            .filter(|stage| stage.applies(already))
            .collect();
        self.at = if self.showing.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Start it showing only the stops that are not in `known`.
    ///
    /// Used after an upgrade: somebody who has been using this for months is
    /// shown what is new and nothing else. If nothing is new, nothing runs.
    ///
    /// **Roadmap item 171 widened what "new" means.** It used to be a tab added
    /// in a later release. It is now any stop the reader has not been shown,
    /// which is what carries the setup to the people who most need it:
    /// somebody who has had VeilVoice installed for a year has every tab key
    /// stored and none of the setup ones, so the next launch offers them the
    /// app lock, at-rest encryption and the rest, once, and never again.
    pub fn start_new_only(&mut self, known: &[String], already: &Already) {
        self.showing = stages()
            .into_iter()
            .filter(|stage| stage.applies(already))
            .filter(|stage| {
                let key = stage.key();
                !known.contains(&key)
            })
            .collect();
        self.at = if self.showing.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Whether the tour is on screen.
    pub fn running(&self) -> bool {
        self.at.is_some()
    }

    /// Stop it, and wipe anything typed into it.
    pub fn stop(&mut self) {
        self.at = None;
        self.showing.clear();
        self.wipe();
    }

    /// Clear the typed passphrase fields.
    ///
    /// Called on every exit from the tour rather than only on the one that
    /// sets a lock, because the field holds what somebody typed whether or not
    /// they pressed the button, and a tour closed halfway through should not
    /// leave it sitting in memory until the next one.
    fn wipe(&mut self) {
        use zeroize::Zeroize as _;
        self.lock_entry.zeroize();
        self.lock_repeat.zeroize();
        self.lock_entry.clear();
        self.lock_repeat.clear();
        self.lock_requested = false;
    }

    /// Start it again from the beginning, because somebody asked.
    ///
    /// The same thing [`start`](Self::start) does, and a separate name because
    /// the two mean different things: `start` is the window deciding that a
    /// reader has not seen this, and this is a reader saying "show me that
    /// again". The second one is not a question about which stops are new, so
    /// it shows every stop that applies whatever is stored.
    pub fn restart(&mut self, already: &Already) {
        self.start(already);
    }

    /// Draw the current stop over the window.
    ///
    /// Takes the context rather than a `Ui`, because it is not drawn inside
    /// anything: the window paints its tabs as usual and this goes on top of
    /// all of it. See the module note for why that is the whole point of
    /// roadmap item 171 and not a presentational change.
    ///
    /// Takes the settings and the security state because the setup stops
    /// change both, exactly as the first-run cards do, and through the same
    /// calls: a lock is created by `set_lock_from_setup` here and there, so
    /// there is one path that makes a lock rather than two that can disagree.
    pub fn overlay(
        &mut self,
        ctx: &egui::Context,
        prefs: &mut crate::settings::Settings,
        security: &mut crate::security::Security,
        already: &Already,
    ) -> Outcome {
        let Some(at) = self.at else {
            return Outcome::Running;
        };
        let Some(&stage) = self.showing.get(at) else {
            self.stop();
            return Outcome::finished();
        };
        let last = at + 1 >= self.showing.len();
        let total = self.showing.len();
        // `content_rect` rather than the whole window: it is the area inside
        // the panels, which is what the modal's own backdrop covers, and it is
        // what the card has to fit inside.
        let height = ctx.content_rect().height() * CARD_HEIGHT_SHARE;

        let modal = egui::Modal::new(egui::Id::new("veilvoice-tour"))
            .frame(
                egui::Frame::new()
                    .fill(p::surface())
                    .stroke(egui::Stroke::new(1.0, p::border()))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                ui.set_max_width(CARD_WIDTH);
                let mut press = Press::Stay;

                // The text scrolls and the buttons do not. On a window shorter
                // than the card, a single scroller around the whole thing puts
                // "next" below the fold, and a tour whose only way forward has
                // to be scrolled to is a tour that looks stuck.
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} of {total}", at + 1))
                                .small()
                                .color(p::muted()),
                        );
                        ui.add_space(4.0);
                        press = self.body(ui, stage, prefs, security, already);
                    });

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let next = if last { "done" } else { "next" };
                    if ui.button(next).clicked() {
                        press = if last { Press::Finish } else { Press::Forward };
                    }
                    if at > 0 && ui.button("back").clicked() {
                        press = Press::Back;
                    }
                    if !last && ui.button("skip the rest").clicked() {
                        press = Press::Finish;
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "  Escape closes this. Settings, under Interface, brings it back.",
                    )
                    .small()
                    .color(p::muted()),
                );
                press
            });

        // Escape, and deliberately not a click on the shade. `should_close`
        // treats the two as one gesture, which is right for a dialogue asking
        // a question and wrong for something somebody is stepping through: a
        // misjudged click should not throw away the half they have not read.
        // The other two conditions are `should_close`'s own, and they matter:
        // Escape belongs to whatever is on top, and to an open dropdown before
        // this.
        let escaped = modal.is_top_modal
            && !modal.any_popup_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

        match modal.inner {
            Press::Stay if escaped => {
                self.stop();
                Outcome::finished()
            }
            Press::Stay => Outcome::Running,
            Press::Forward => {
                self.at = Some(at + 1);
                self.wipe();
                Outcome::Running
            }
            Press::Back => {
                self.at = Some(at.saturating_sub(1));
                self.wipe();
                Outcome::Running
            }
            Press::Finish => {
                self.stop();
                Outcome::finished()
            }
            Press::FinishAndShow(tab) => {
                self.stop();
                Outcome::Finished { show: Some(tab) }
            }
        }
    }

    /// What one stop says, and what it offers.
    fn body(
        &mut self,
        ui: &mut Ui,
        stage: Stage,
        prefs: &mut crate::settings::Settings,
        security: &mut crate::security::Security,
        already: &Already,
    ) -> Press {
        match stage {
            Stage::AppLock => self.app_lock(ui, security),
            Stage::AtRest => at_rest(ui, prefs, security),
            Stage::Decoy => decoy(ui),
            Stage::Install => install(ui, prefs, already.installed),
            Stage::SettingsHeadings => settings_headings(ui),
            Stage::Tab(at) => {
                let (_, title, text) = CARDS[at];
                heading(ui, title);
                ui.label(RichText::new(text).color(p::fg()));
                Press::Stay
            }
        }
    }

    /// The app lock, offered to somebody who has not set one.
    ///
    /// The words are the first-run card's, because they are the right words
    /// and two versions of a warning about losing your files is one of them
    /// going stale. The call is the first-run card's too:
    /// `set_lock_from_setup` is the one path that creates a lock, which is
    /// what F-141 cost.
    fn app_lock(&mut self, ui: &mut Ui, security: &mut crate::security::Security) -> Press {
        heading(ui, "A password for VeilVoice itself");
        ui.label(RichText::new(
            "It stops somebody who picks up your unlocked computer from \
             opening VeilVoice, seeing what you have processed, or starting a \
             live scramble.",
        ));
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "It also encrypts VeilVoice's own files and gives them \
                 meaningless names, with decoy files among them, so the folder \
                 says nothing about what you have done. Without a password \
                 none of that is possible: there is no key.",
            )
            .color(p::muted()),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Forget this password and those files are gone. It is not a \
                 lock you can take off; it is the only way back to them.",
            )
            .color(p::yellow()),
        );
        ui.add_space(12.0);

        if self.lock_requested {
            ui.label(RichText::new("setting it now...").color(p::muted()));
            // The worker answers in its own time and the window keeps drawing.
            // Once it has, the stop has nothing left to offer, so it moves on
            // by itself rather than leaving somebody looking at a finished
            // thing wondering whether to press next.
            return if security.has_lock() {
                Press::Forward
            } else {
                Press::Stay
            };
        }

        secret(ui, "password", &mut self.lock_entry);
        secret(ui, "again", &mut self.lock_repeat);
        let matched = !self.lock_entry.is_empty() && self.lock_entry == self.lock_repeat;
        if !self.lock_entry.is_empty() && !matched {
            ui.label(
                RichText::new("the two entries differ")
                    .color(p::yellow())
                    .small(),
            );
        }
        ui.add_space(8.0);
        if ui
            .add_enabled(matched, egui::Button::new("set this password"))
            .clicked()
        {
            let entry = std::mem::take(&mut self.lock_entry);
            self.lock_repeat.clear();
            security.set_lock_from_setup(entry);
            self.lock_requested = true;
        }
        Press::Stay
    }
}

/// What the window should do once the tour is out of the way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still on screen.
    Running,
    /// Done, skipped or escaped.
    Finished {
        /// A tab the reader asked to be taken to, by its key.
        ///
        /// The tour offers rather than acts where acting is a commitment:
        /// installing this copy is a decision made on the Install tab, and the
        /// most a walkthrough should do is put somebody in front of it. See
        /// the module note.
        show: Option<&'static str>,
    },
}

impl Outcome {
    /// Finished, with nowhere in particular to go.
    fn finished() -> Self {
        Self::Finished { show: None }
    }
}

/// At-rest encryption: on, and what that means.
///
/// Reports rather than asks, because roadmap item 170 made it the default and a
/// reader should be told once that every recording is being encrypted. The
/// switch is here as well, because a default nobody can see is a decision
/// taken away.
fn at_rest(
    ui: &mut Ui,
    prefs: &mut crate::settings::Settings,
    security: &crate::security::Security,
) -> Press {
    heading(ui, "Your recordings are encrypted where they are written");
    ui.label(RichText::new(
        "VeilVoice destroys the voiceprint and keeps the words, on purpose: a \
         veiled recording is still a recording of everything that was said. So \
         it is encrypted before it reaches the disk.",
    ));
    ui.add_space(8.0);

    let mut seal = prefs.seal_with_app_lock();
    if ui
        .checkbox(&mut seal, "Use the VeilVoice password to seal them")
        .changed()
    {
        prefs.set_seal_with_app_lock(seal);
    }
    ui.label(
        RichText::new(if security.has_lock() {
            "  On, and there is a password to seal with, so this is what is \
             happening to every recording you make."
        } else {
            "  On, and waiting. It does nothing until there is a password on \
             VeilVoice, and it takes effect by itself the moment you set one."
        })
        .small()
        .color(p::muted()),
    );
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "  Turn it off and recordings are sealed with the separate \
             recording passphrase instead, which the Lock tab sets. Off does \
             not mean unencrypted: writing a recording in the clear is a \
             different switch, on the Lock tab, and it asks first.",
        )
        .small()
        .color(p::muted()),
    );
    Press::Stay
}

/// The decoy passphrase: what it is, and what it is not.
///
/// The words are `veilvoice_crypto::decoy`'s own, so the window and the command
/// line say the same thing about what it is worth. Shown only where one can be
/// set: see [`Stage::applies`].
fn decoy(ui: &mut Ui) -> Press {
    heading(ui, "A second password that opens an empty VeilVoice");
    for paragraph in veilvoice_crypto::decoy::SCOPE.lines() {
        if paragraph.trim().is_empty() {
            ui.add_space(6.0);
        } else {
            ui.label(RichText::new(paragraph).color(p::fg()));
        }
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(veilvoice_crypto::decoy::WHY_NO_DESTRUCTION)
            .small()
            .color(p::muted()),
    );
    Press::Stay
}

/// Installed or portable, and the offer that follows from it.
fn install(ui: &mut Ui, prefs: &mut crate::settings::Settings, installed: bool) -> Press {
    heading(ui, "This copy");
    if installed {
        ui.label(RichText::new(
            "Installed. It is on this machine for good, it is on your menu or \
             path, and its settings live in your account. Removing it is the \
             same as removing any other program.",
        ));
        return Press::Stay;
    }

    ui.label(RichText::new(
        "Portable. It runs from wherever you put it and installs nothing: move \
         the folder and VeilVoice moves with it, delete the folder and it is \
         gone. That is a perfectly good way to keep using it.",
    ));
    ui.add_space(12.0);
    let mut press = Press::Stay;
    ui.horizontal(|ui| {
        // An offer, not the act. Installing writes to somewhere permanent and
        // a walkthrough is a bad place to commit somebody to that, so the most
        // this does is put them in front of the tab where the decision is
        // made, with everything it says about what will be written.
        if ui.button("Show me the Install tab").clicked() {
            press = Press::FinishAndShow("install");
        }
        let mut hide = prefs.hide_install_tab();
        if ui
            .checkbox(&mut hide, "Stay portable, and stop offering")
            .changed()
        {
            prefs.set_hide_install_tab(hide);
        }
    });
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "  Nothing else changes either way: `veilvoice install` still works \
             from the command line, and Settings, under Interface, brings the \
             tab back.",
        )
        .small()
        .color(p::muted()),
    );
    press
}

/// Every heading in Settings, and what it covers.
///
/// Read from `Page::ALL` rather than written out, so a page added to the menu
/// appears here without anybody remembering, and a heading reworded is
/// reworded in one place. That list is also what the menu draws and what
/// `--settings-page` accepts, so this cannot describe a page that is not
/// there.
fn settings_headings(ui: &mut Ui) -> Press {
    heading(ui, "What is in Settings");
    ui.label(RichText::new(
        "Five headings. Everything under them is stored beside the application \
         and goes nowhere.",
    ));
    ui.add_space(10.0);
    for (_, _, title, blurb) in crate::settings::Page::ALL {
        ui.label(RichText::new(*title).color(p::cyan()).strong());
        ui.label(RichText::new(format!("  {blurb}")).color(p::fg()));
        ui.add_space(6.0);
    }
    Press::Stay
}

/// A stop's title.
fn heading(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).size(18.0).color(p::fg()).strong());
    ui.add_space(8.0);
}

/// A password field. The same shape the first-run cards use.
fn secret(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::muted()));
        ui.add(
            egui::TextEdit::singleline(value)
                .password(true)
                .desired_width(240.0),
        );
    });
}

/// What a frame of the card asked for.
///
/// Returned out of the modal rather than acted on inside it, because the
/// closure holds `&mut self` for as long as it runs and the Escape key cannot
/// be read until it has finished. One value out, one decision made, in one
/// place.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Press {
    /// Nothing was pressed.
    Stay,
    /// The next stop.
    Forward,
    /// The one before.
    Back,
    /// Done, or skipped: the two end the tour the same way, because both mean
    /// the reader is finished with it and neither should bring it back on the
    /// next launch.
    Finish,
    /// Done, and take the reader to this tab.
    FinishAndShow(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lock card says the integrity record is taken for you, and the
    /// window is what takes it.
    ///
    /// Both halves, because a sentence in a tour is a promise about behaviour
    /// and this one was written the day the behaviour it describes had been
    /// there for four releases with nothing telling anybody. If the start in
    /// `app.rs` is ever removed, the card is left claiming something untrue to
    /// every new reader, which is the failure this checks for.
    #[test]
    fn the_lock_card_promises_what_the_window_actually_does() {
        let card = CARDS
            .iter()
            .find(|(tab, _, _)| *tab == "lock")
            .expect("a card for the lock tab");
        assert!(
            card.2.contains("record"),
            "the lock card no longer mentions the integrity record: {}",
            card.2
        );
        let app = include_str!("app.rs");
        assert!(
            app.contains("app.integrity.start(None)"),
            "nothing starts the integrity record at launch any more, and the \
             tour still tells every new reader that one was taken"
        );
    }

    #[test]
    fn every_card_has_a_sentence_and_no_two_share_a_tab() {
        let mut keys: Vec<&str> = CARDS.iter().map(|(key, _, _)| *key).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), before, "two cards claim the same tab");
        for (key, title, body) in CARDS {
            assert!(!key.is_empty() && !title.is_empty());
            assert!(
                body.len() > 60,
                "{key} has a sentence too short to explain anything"
            );
        }
    }

    /// Nothing is shown twice, and nothing is remembered under two names.
    ///
    /// The keys go into somebody's settings file and are answered against on
    /// every later launch, so a collision does not show up as a wrong pixel:
    /// it shows up as a stop that never runs again, or one that runs for ever.
    #[test]
    fn every_stop_is_remembered_under_a_name_of_its_own() {
        let mut keys: Vec<String> = all_keys();
        let before = keys.len();
        assert_eq!(before, stages().len(), "a stop is not in the stored list");
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), before, "two stops share a name");
        for (tab, _, _) in CARDS {
            assert!(
                keys.iter().any(|key| key == tab),
                "the {tab} tab is not in the stored list, so its card comes \
                 back on every launch"
            );
        }
    }

    #[test]
    fn a_first_run_sees_everything_there_is_to_see() {
        let already = Already::default();
        let mut tour = Tour::default();
        assert!(!tour.running());
        tour.start(&already);
        assert!(tour.running());
        assert_eq!(
            tour.showing.len(),
            stages().iter().filter(|s| s.applies(&already)).count()
        );
        assert!(
            tour.showing.len() > CARDS.len(),
            "roadmap item 171: a first run is offered the setup as well as the \
             description of the tabs"
        );
    }

    #[test]
    fn an_upgrade_shows_only_what_is_new() {
        // Somebody who has toured everything there was, and then two tabs
        // arrived.
        let known: Vec<String> = all_keys()
            .into_iter()
            .filter(|key| !CARDS.iter().rev().take(2).any(|(tab, _, _)| *tab == *key))
            .collect();
        let mut tour = Tour::default();
        tour.start_new_only(&known, &Already::default());
        assert!(tour.running(), "two new tabs should start a tour");
        assert_eq!(tour.showing.len(), 2);
    }

    #[test]
    fn an_upgrade_that_adds_nothing_shows_nothing() {
        let mut tour = Tour::default();
        tour.start_new_only(&all_keys(), &Already::default());
        assert!(
            !tour.running(),
            "a tour with nothing new to say must not run"
        );
    }

    /// **Roadmap item 171, the part that reaches the people who need it.**
    ///
    /// Somebody who installed this a year ago has every tab key stored and
    /// none of the setup ones, because the setup stops did not exist. They are
    /// exactly who has never been offered an app lock, so the next launch
    /// offers them the setup and not one word about the tabs they have been
    /// using all year.
    #[test]
    fn a_long_standing_reader_is_offered_the_setup_and_nothing_else() {
        let known: Vec<String> = CARDS.iter().map(|(tab, _, _)| (*tab).to_string()).collect();
        let already = Already::default();
        let mut tour = Tour::default();
        tour.start_new_only(&known, &already);
        assert!(tour.running());
        assert!(
            tour.showing
                .iter()
                .all(|stage| !matches!(stage, Stage::Tab(_))),
            "a reader who has seen every tab was shown the tab cards again"
        );
        assert!(
            tour.showing.contains(&Stage::AppLock),
            "the one stop that is worth interrupting somebody for was left out"
        );
    }

    /// A question already answered is not asked again. The same rule the
    /// first-run cards keep, for the same reason.
    #[test]
    fn what_is_already_set_is_not_offered_again() {
        let mut tour = Tour::default();
        tour.start(&Already {
            app_lock: true,
            installed: true,
            decoy_can_be_set: false,
        });
        assert!(
            !tour.showing.contains(&Stage::AppLock),
            "somebody with a password on VeilVoice was asked to set one"
        );
        assert!(
            tour.showing.contains(&Stage::Install),
            "the stop that says which copy this is has something to say either \
             way"
        );
    }

    /// The decoy stop is written and waits for something that can set one.
    ///
    /// `veilvoice_crypto::decoy` is written and tested and nothing stores a
    /// pair, which is roadmap item 173. Showing the stop now would describe a
    /// control that is not there, so it is gated rather than half-drawn, and
    /// this is what fails the day the gate is turned on without the wiring.
    #[test]
    fn the_decoy_stop_waits_until_a_decoy_can_be_set() {
        let mut tour = Tour::default();
        tour.start(&Already::default());
        assert!(!tour.showing.contains(&Stage::Decoy));

        tour.start(&Already {
            decoy_can_be_set: true,
            ..Already::default()
        });
        assert!(tour.showing.contains(&Stage::Decoy));
    }

    /// Roadmap item 171. Asking for it is not a question about what is new, so
    /// it shows every stop that applies whatever is stored.
    #[test]
    fn asking_for_it_again_shows_the_whole_thing() {
        let already = Already::default();
        let mut tour = Tour::default();
        // The state somebody in this position is actually in: they have seen
        // the whole thing, so `start_new_only` would show them nothing.
        tour.start_new_only(&all_keys(), &already);
        assert!(!tour.running());

        tour.restart(&already);
        assert!(tour.running(), "asking for it must start it");
        assert_eq!(
            tour.showing.len(),
            stages().iter().filter(|s| s.applies(&already)).count(),
            "a reader who asked to see it again was shown a subset"
        );
    }

    /// Every stop draws, with nothing set up and with everything set up.
    ///
    /// A walkthrough is the one screen whose every branch a new reader meets
    /// and an existing one never sees again, so a panic in a stop somebody
    /// reaches on their fourth press is a panic nobody here would find by
    /// using the application.
    #[test]
    fn every_stop_draws() {
        for already in [
            Already::default(),
            Already {
                app_lock: true,
                installed: true,
                decoy_can_be_set: true,
            },
        ] {
            for stage in stages() {
                let ctx = egui::Context::default();
                let mut tour = Tour::default();
                let mut prefs = crate::settings::Settings::default();
                let mut security = crate::security::Security::default();
                let _ = crate::headless_frame(&ctx, Default::default(), |ui| {
                    let press = tour.body(ui, stage, &mut prefs, &mut security, &already);
                    assert!(
                        press == Press::Stay,
                        "{:?} acted on a frame nobody pressed anything in",
                        stage
                    );
                });
            }
        }
    }

    /// The passphrase fields are cleared on every way out, not only on the one
    /// that uses them.
    ///
    /// Somebody who types a password into the tour and then closes it has left
    /// it in memory, and "they did not press the button" is not a reason for it
    /// to still be there.
    #[test]
    fn what_was_typed_does_not_outlive_the_tour() {
        let already = Already::default();
        let mut tour = Tour::default();
        tour.start(&already);
        tour.lock_entry.push_str("a password");
        tour.lock_repeat.push_str("a password");
        tour.stop();
        assert!(tour.lock_entry.is_empty() && tour.lock_repeat.is_empty());

        tour.start(&already);
        tour.lock_entry.push_str("a password");
        tour.wipe();
        assert!(
            tour.lock_entry.is_empty(),
            "stepping to the next stop left the last one's password behind"
        );
    }

    /// The app lock is offered once per reader, not once per screen that could
    /// offer it.
    ///
    /// The first-run cards ask for it and the walkthrough starts the moment
    /// they are answered, so without this somebody who skipped that card is
    /// asked the same question again thirty seconds later, which reads as a
    /// program that was not listening. Both halves are checked: that the
    /// window marks the stop when the cards finish, and that a marked stop is
    /// then left out.
    #[test]
    fn the_app_lock_is_not_asked_for_twice_in_one_launch() {
        let app = include_str!("app.rs").replace("\r\n", "\n");
        let at = app
            .find("mark_toured(&[crate::tour::Stage::AppLock.key()])")
            .expect("the first-run cards do not record that they asked");
        let finished = app
            .find("self.preferences.finish_first_run();")
            .expect("the first-run cards are finished somewhere");
        assert!(
            at > finished,
            "the stop is marked somewhere other than where the cards finish"
        );

        let mut tour = Tour::default();
        tour.start_new_only(&[Stage::AppLock.key()], &Already::default());
        assert!(
            !tour.showing.contains(&Stage::AppLock),
            "the stop the first-run cards just offered was offered again"
        );
        assert!(tour.running(), "the rest of the walkthrough still runs");
    }

    /// Roadmap item 171. The window has to store every stop, not the ones this
    /// run happened to show.
    ///
    /// A stop left out because it was already answered has nothing left to
    /// say, and storing it is what stops the next launch offering it. Storing
    /// only what was shown means somebody who set a password before their
    /// first tour is asked to set one on every launch after it.
    #[test]
    fn the_window_stores_every_stop_rather_than_the_ones_it_showed() {
        let app = include_str!("app.rs").replace("\r\n", "\n");
        assert!(
            app.contains("mark_toured(&crate::tour::all_keys())"),
            "the window stores something other than the whole list"
        );
        assert!(
            app.contains("show.and_then(Tab::from_key)"),
            "the window ignores the tab the tour was asked to open"
        );
    }

    /// Roadmap item 171. The tour is drawn over the window rather than instead of
    /// it, and that is a claim about `app.rs` as much as about this file: the
    /// whole point is lost if the window returns before it draws the tabs the
    /// cards are describing.
    ///
    /// Read from the source, because what is asserted is that a statement is
    /// *not reachable*. A rendering test would prove it for one frame.
    #[test]
    fn the_window_still_draws_underneath_the_tour() {
        let app = include_str!("app.rs").replace("\r\n", "\n");
        // Anchored on the outcome rather than on the call: rustfmt breaks a
        // long call across lines, and a test that fails because an argument
        // was added is a test that tells you nothing about what it guards.
        let at = app
            .find("crate::tour::Outcome::Finished")
            .expect("the window draws the tour as an overlay");
        let central = app
            .rfind("egui::CentralPanel::default().show(root, |ui| {")
            .expect("the window has a central panel");
        assert!(
            at > central,
            "the tour is drawn inside the central panel again, so it replaces \
             the tabs it is describing rather than sitting over them"
        );
        assert!(
            !app.contains("self.tour.panel("),
            "the old panel that returned instead of drawing the window is back"
        );
    }

    /// The card has to fit in the window it is drawn over, and its buttons have
    /// to stay reachable. Both are properties of the numbers rather than of a
    /// frame, so both are checked here.
    #[test]
    fn the_card_leaves_the_window_visible_around_it() {
        // Against the real window sizes rather than against the share on its
        // own: what has to hold is that there is window left around the card,
        // and on the shortest window this application will open at, which is
        // where it is tightest.
        for height in [crate::window::MINIMUM[1], 900.0, 1440.0] {
            let card = height * CARD_HEIGHT_SHARE;
            let shade = height - card;
            assert!(
                shade >= 48.0,
                "at {height} the shade around the card is {shade} pixels, too \
                 thin to read as an overlay rather than as a replacement"
            );
            assert!(
                card > 200.0,
                "at {height} the card is {card} pixels and a sentence does not \
                 fit in it"
            );
        }
        for width in [crate::window::MINIMUM[0], 1100.0, 1920.0] {
            assert!(
                CARD_WIDTH <= width,
                "at {width} across, the card is wider than the window and \
                 would be cut off"
            );
        }
    }

    /// Escape is handled and a click on the shade is deliberately not.
    ///
    /// `ModalResponse::should_close` treats the two as one gesture. Using it
    /// would mean a misjudged click halfway through the tour threw away the
    /// half the reader had not seen, so this asserts the code does not call it.
    #[test]
    fn a_stray_click_outside_the_card_does_not_end_the_tour() {
        let source = include_str!("tour.rs").replace("\r\n", "\n");
        let overlay = source.find("pub fn overlay(").expect("the overlay exists");
        // Bounded at the end of the function. Unbounded, this reads its own
        // test as part of the code it is checking and fails on the string it
        // is looking for, which is a guard that can only ever fail.
        let end = source[overlay..]
            .find("\nenum Press {")
            .map(|at| overlay + at)
            .expect("the press enum follows the overlay");
        let body = &source[overlay..end];
        assert!(
            body.contains("Key::Escape"),
            "nothing reads Escape, so the overlay has no keyboard way out"
        );
        assert!(
            !body.contains("should_close()"),
            "should_close() closes on a backdrop click as well, which ends the \
             tour on a stray press"
        );
    }
}
