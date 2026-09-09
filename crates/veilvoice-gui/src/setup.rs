// SPDX-License-Identifier: GPL-3.0-or-later
//! The setup tab: install this copy, undo that, and the optional companions.
//!
//! This is the graphical front end to [`veilvoice_setup`], and it is a *front
//! end* in the strict sense: it holds no installation logic of its own. Every
//! change to the machine goes through the same functions `veilvoice install`
//! calls, so the careful part (editing `PATH`, which is the one operation here
//! that can damage a machine) has one implementation and one set of tests.
//!
//! # Portable is the default, and this tab says so first
//!
//! The screen opens by telling the user what they already have: a portable
//! copy that needs nothing, or an installed one. Installing is offered as a
//! convenience with an exact list of what it will change, not as a step
//! somebody has to complete before the program is usable. A privacy tool that
//! implies it must be installed has trained its user badly.
//!
//! # Nothing is ticked, because there is nothing to tick
//!
//! The companions are not a checklist with defaults. Each is a row that states
//! what the software is, who wrote it and under what licence, whether it was
//! found on this machine, and a single button that does one thing to one named
//! program. There is no "install recommended extras", because that is the
//! control through which unwanted software historically arrived.
//!
//! Where the answer is a proprietary driver the button opens the vendor's page
//! and nothing else; where it needs root the command is shown and not run.
//! Both of those refusals live in [`veilvoice_setup::companions`], not here, so
//! that a second front end cannot be more permissive than this one.
//!
//! # Nothing slow runs on the UI thread
//!
//! Copying binaries, editing the registry and running a package manager all
//! take real time, because a package manager can take minutes. Each runs on a worker
//! and reports back through an [`std::sync::mpsc`] channel, exactly as the file
//! job in [`crate::VeilVoiceApp`] does, so the window keeps painting and the
//! progress strip keeps moving.
//!
//! The strip honours the reduced-motion decision like everything else: with
//! motion off it is a static bar and a word, not a frozen animation.
//!
//! # In plain words
//!
//! The tab that installs this copy, removes it again, and points at the optional
//! extra software.
//!
//! It is only a front end: every decision about where files go and what gets
//! changed lives in one place shared with the command line, so the two cannot
//! disagree about what "installed" means.
//!
//! Nothing here installs anything belonging to somebody else without being asked.
//! What each thing is, who makes it and what it is for are shown first, and the
//! exact command is shown before the question.

use crate::theme::palette as p;
use egui::{RichText, Ui};
use std::sync::mpsc;
use veilvoice_setup::companions::{self, Companion, Offer, Presence};
use veilvoice_setup::install;

/// What a worker finished doing: lines to show, and whether it went well.
struct Done {
    lines: Vec<String>,
    good: bool,
}

/// One companion as this tab needs it: the facts, plus what was found.
struct Row {
    companion: &'static Companion,
    presence: Presence,
    offer: Offer,
}

/// The setup tab's state.
pub struct Setup {
    status: install::Status,
    rows: Vec<Row>,
    job: Option<mpsc::Receiver<Done>>,
    /// A probe of the companions, running off the paint thread.
    ///
    /// `detect_all` runs a command per companion: `where` or `which`, and on
    /// Windows a PowerShell call that enumerates sound devices. On the paint
    /// thread that is hundreds of milliseconds inside one frame, which is the
    /// window going white and the pointer becoming a spinner. It was doing
    /// exactly that every time somebody pressed "look again".
    probe: Option<mpsc::Receiver<Vec<Row>>>,
    /// What is running, in words, for the progress strip.
    busy: Option<String>,
    /// The last report, and whether it was a success.
    report: Option<(Vec<String>, bool)>,
    /// The uninstall confirmation, which is deliberately a second click.
    confirming_uninstall: bool,
    /// Whether an install should carry a portable copy's settings across, once
    /// the question has been answered.
    ///
    /// `None` until it is, and the install button waits for it: guessing here
    /// either copies somebody's vault onto a machine they do not own, or leaves
    /// somebody who moved off a stick wondering where their recordings went.
    carry: Option<bool>,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new()
    }
}

impl Setup {
    /// Read the machine's current state. Changes nothing.
    pub fn new() -> Self {
        Self {
            status: install::status(),
            rows: detect_all(),
            job: None,
            probe: None,
            busy: None,
            carry: None,
            report: None,
            confirming_uninstall: false,
        }
    }

    /// Whether this copy is the installed one.
    ///
    /// Read once, when the panel was built. That is deliberate: the tab row is
    /// drawn every frame and this answer involves the filesystem, so asking per
    /// frame would put a `stat` in the paint path -- which is exactly the shape
    /// of the defect that made the window freeze every couple of seconds.
    pub fn running_installed(&self) -> bool {
        self.status.running_installed
    }

    /// True while a worker is running, so the app can keep repainting.
    pub fn is_busy(&self) -> bool {
        self.job.is_some() || self.probe.is_some()
    }

    /// Drain the worker channel. Called once per frame.
    ///
    /// Handles `Disconnected` as well as a message: a worker that panicked
    /// must leave the interface saying so rather than spinning for ever.
    pub fn poll(&mut self) {
        // The probe first, and separately: it can finish while an install is
        // still running, and one `return` for both would leave its answer in
        // the channel until the install ended.
        if let Some(rx) = &self.probe {
            match rx.try_recv() {
                Ok(rows) => {
                    self.rows = rows;
                    self.probe = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    // A probe that died leaves what was already on screen,
                    // which is the last true answer, rather than an empty list
                    // that would read as "you have none of these".
                    self.probe = None;
                }
            }
        }

        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(Done { lines, good }) => {
                self.report = Some((lines, good));
                self.job = None;
                self.busy = None;
                // Anything a worker did could have changed both of these.
                self.status = install::status();
                self.rows = detect_all();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.report = Some((
                    vec![
                        "the worker stopped without reporting anything. Nothing here \
                          can say how far it got; check the state above before trying \
                          again."
                            .to_string(),
                    ],
                    false,
                ));
                self.job = None;
                self.busy = None;
                self.status = install::status();
            }
        }
    }

    /// Draw the tab.
    pub fn tab(&mut self, ui: &mut Ui, motion: crate::prefs::Motion) {
        self.poll();
        // The scroll area fills the tab rather than shrinking to its content,
        // and the inner width is pinned to the tab's. This screen is mostly
        // long sentences stating limits, and a limit that wraps off the edge
        // of the window is a limit nobody read.
        // The application scrolls every tab in one place now; a second
        // scroller here would trap the wheel in whichever the pointer was over.
        // The width pin stays, because it is what stops the long sentences
        // wrapping off the edge.
        let width = ui.available_width();
        ui.set_max_width(width);
        self.where_this_copy_lives(ui);
        ui.add_space(14.0);
        self.install_controls(ui, motion);
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(12.0);
        self.companion_rows(ui);
    }

    /// Why this tab is here, and how to make it not be.
    ///
    /// Under the header rather than buried in settings, because the question
    /// "why is this program asking to install itself" is asked *here*, while
    /// looking at the tab. The control that answers it lives in settings --
    /// a tab that could hide itself and nothing else could bring it back would
    /// be a one-way door.
    fn visibility_note(&self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "This tab is only here because you are running a portable copy. Once \
                 VeilVoice is installed it disappears by itself: a program offering to \
                 install itself when it already is tells you something untrue about \
                 what you are running. To hide it on a portable copy too, there is a \
                 tick under settings -> interface.",
            )
            .small()
            .color(p::muted()),
        );
        ui.add_space(10.0);
    }

    // --- the state of this copy --------------------------------------------

    fn where_this_copy_lives(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("This copy").color(p::blue()).small());
        ui.add_space(6.0);

        // Portable first, and stated as a working arrangement rather than as
        // something missing. It is how most people will run this.
        self.visibility_note(ui);
        let (headline, colour) = match (self.status.installed, self.status.running_installed) {
            (_, true) => ("you are running the installed copy", p::green()),
            (true, false) => (
                "you are running a portable copy, and an installed one also exists",
                p::yellow(),
            ),
            (false, false) => (
                "you are running a portable copy. Nothing is installed, and nothing needs to be",
                p::fg(),
            ),
        };
        ui.label(RichText::new(headline).color(colour));
        ui.add_space(8.0);

        field(
            ui,
            "running from",
            &self
                .status
                .running_from
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".into()),
        );
        field(
            ui,
            "install goes to",
            &self
                .status
                .prefix
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not resolvable on this system".into()),
        );
        field(
            ui,
            "on PATH",
            if self.status.on_path {
                "yes, for this session"
            } else {
                "no"
            },
        );

        // The two facts above are different, and saying which one "on PATH"
        // answered stops somebody concluding a new terminal will behave the
        // same way this one does.
        ui.label(
            RichText::new(
                "\"on PATH\" is read from this running process. A terminal opened \
                 after an install is a different question, and the answer there is \
                 what an install changes.",
            )
            .small()
            .color(p::muted()),
        );
    }

    // --- install and uninstall ---------------------------------------------

    fn install_controls(&mut self, ui: &mut Ui, motion: crate::prefs::Motion) {
        ui.label(
            RichText::new("Install for this user")
                .color(p::blue())
                .small(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Optional. It exists so that typing `veilvoice` in a terminal works. \
                 No administrator rights are asked for and nothing is written outside \
                 your own account.",
            )
            .color(p::fg()),
        );
        ui.add_space(8.0);

        // Exactly what will change, before the button rather than after it.
        for line in install_changes() {
            ui.label(
                RichText::new(format!("  · {line}"))
                    .small()
                    .color(p::muted()),
            );
        }
        ui.add_space(10.0);

        if self.job.is_some() {
            self.progress(ui, motion);
            return;
        }

        // Only where there is something to carry: a copy keeping its state
        // beside itself. An ordinary copy already keeps it where an installed
        // one looks, so there is no question to ask and none is asked.
        let portable = veilvoice_crypto::lock::is_portable();
        if portable {
            self.carry_question(ui);
        }

        ui.horizontal(|ui| {
            let can_install = can_install(
                self.status.prefix.is_some(),
                self.status.running_installed,
                portable,
                self.carry,
            );
            let install_button = ui.add_enabled(
                can_install,
                egui::Button::new(RichText::new("install").color(p::green())),
            );
            if install_button.clicked() {
                self.confirming_uninstall = false;
                let carry = self.carry == Some(true);
                self.start("installing", move || {
                    let mut said = install::install()?;
                    if carry {
                        // Reported inside the same result, so one press gives
                        // one account of what happened rather than two.
                        said.extend(crate::paths::carry_over()?);
                    }
                    Ok(said)
                });
            }
            if self.status.running_installed {
                install_button
                    .on_hover_text("this program is already running from the install directory");
            }

            if self.status.installed {
                if self.confirming_uninstall {
                    if ui
                        .button(RichText::new("yes, remove it").color(p::red()))
                        .clicked()
                    {
                        self.confirming_uninstall = false;
                        self.start("removing", install::uninstall);
                    }
                    if ui
                        .button(RichText::new("cancel").color(p::muted()))
                        .clicked()
                    {
                        self.confirming_uninstall = false;
                    }
                } else if ui
                    .button(RichText::new("uninstall").color(p::yellow()))
                    .clicked()
                {
                    self.confirming_uninstall = true;
                }
            }
        });

        if self.confirming_uninstall {
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "This removes the installed copy, its PATH entry and its uninstall \
                     entry. Your recordings, keys and settings are somewhere else and \
                     are not touched.",
                )
                .color(p::yellow()),
            );
        }

        if let Some((lines, good)) = &self.report {
            ui.add_space(12.0);
            let colour = if *good { p::green() } else { p::red() };
            for line in lines {
                ui.label(RichText::new(line).color(colour));
            }
            if *good {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Open a new terminal for a PATH change to take effect. VeilVoice \
                         never checks for updates and cannot tell you when one exists. \
                         it has no network code at all.",
                    )
                    .small()
                    .color(p::muted()),
                );
            }
        }
    }

    /// Ask whether an install should take the settings with it.
    ///
    /// A question rather than a default, because the two right answers point in
    /// opposite directions and nothing here can tell which one applies.
    /// Somebody moving off a memory stick onto their own machine wants their
    /// settings and vaults carried over. Somebody installing on a shared or
    /// borrowed machine wants them left exactly where they are, and guessing
    /// wrong there copies their vault onto a computer that is not theirs.
    fn carry_question(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("This copy keeps its settings beside itself")
                .color(p::yellow())
                .small(),
        );
        ui.label(
            RichText::new(
                "An installed copy looks in this platform's own configuration \
                 directory instead. Say what should happen to what is beside \
                 the program before installing.",
            )
            .color(p::fg()),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.carry, Some(true), "carry them across");
            ui.selectable_value(&mut self.carry, Some(false), "leave them here");
        });
        ui.label(
            RichText::new(match self.carry {
                Some(true) => {
                    "Copied, never moved and never overwritten: anything already \
                     in the configuration directory is left alone and named in \
                     the report, and this copy keeps working from the folder \
                     beside it."
                }
                Some(false) => {
                    "Nothing is copied. The installed copy starts with no \
                     settings, and this one carries on using the folder beside \
                     it."
                }
                None => {
                    "Unanswered, so the install button is waiting. There is no \
                     sensible default: one answer is right when moving onto your \
                     own machine and the other when installing on somebody \
                     else's."
                }
            })
            .color(p::muted())
            .small(),
        );
        ui.add_space(10.0);
    }

    /// Start a worker, and remember what it is doing so the strip can say.
    fn start<F>(&mut self, what: &str, work: F)
    where
        F: FnOnce() -> Result<Vec<String>, String> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        self.report = None;
        self.busy = Some(what.to_string());
        self.job = Some(rx);
        std::thread::spawn(move || {
            let done = match work() {
                Ok(lines) => Done { lines, good: true },
                Err(error) => Done {
                    lines: vec![error],
                    good: false,
                },
            };
            // The receiver is gone only if the window closed mid-job, which is
            // not an error worth reporting to a window that is not there.
            let _ = tx.send(done);
        });
    }

    /// The progress strip: a travelling highlight, or a plain bar when motion
    /// is off.
    fn progress(&self, ui: &mut Ui, motion: crate::prefs::Motion) {
        let label = self.busy.clone().unwrap_or_else(|| "working".to_string());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{label}…")).color(p::cyan()));
        });
        ui.add_space(4.0);

        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(420.0), 8.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, p::bg_dark());
        painter.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0, p::border()),
            egui::StrokeKind::Inside,
        );

        if motion.enabled {
            // A quarter-width highlight travelling left to right, so the
            // window is visibly alive without claiming to know a percentage.
            // It does not: `reg.exe` and a package manager report no progress,
            // and a bar that fills to 90% and waits is a lie with a shape.
            let time = ui.input(|i| i.time) as f32;
            let width = rect.width() * 0.25;
            let travel = rect.width() + width;
            let position = (time * 0.45).fract() * travel - width;
            let mut lit = rect;
            lit.min.x = rect.min.x + position.max(0.0);
            lit.max.x = (rect.min.x + position + width).min(rect.max.x);
            if lit.max.x > lit.min.x {
                painter.rect_filled(lit, 3.0, p::blue());
            }
        } else {
            // Still, but not empty: an empty bar reads as "stuck".
            let mut lit = rect;
            lit.set_width(rect.width() * 0.35);
            painter.rect_filled(lit, 3.0, p::blend(p::blue(), p::bg_dark(), 0.45));
        }
    }

    // --- the companions ----------------------------------------------------

    fn companion_rows(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Companion software").color(p::blue()).small());
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "None of this is part of VeilVoice and none of it is required. Nothing \
                 here is pre-selected: each is one button, for one named program, and \
                 VeilVoice never runs somebody else's installer.",
            )
            .color(p::fg()),
        );
        ui.add_space(4.0);
        if self.probe.is_some() {
            // Said rather than left blank. The button that has just been
            // pressed going quiet for half a second is what reads as a freeze,
            // and the fix is as much this line as it is the worker.
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new("looking for each of these on this machine")
                        .color(p::muted())
                        .small(),
                );
            });
        } else if ui
            .button(RichText::new("look again").color(p::muted()).small())
            .clicked()
        {
            let (tx, rx) = mpsc::channel();
            // Detached: nothing waits for it, and a probe that outlives the
            // tab simply sends into a channel nobody reads.
            std::thread::spawn(move || {
                let _ = tx.send(detect_all());
            });
            self.probe = Some(rx);
        }
        ui.add_space(10.0);

        let mut action: Option<(String, Offer)> = None;
        let busy = self.job.is_some();

        for row in &self.rows {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(row.companion.name).color(p::fg()).strong());
                    ui.label(
                        RichText::new(format!("· {}", row.companion.vendor))
                            .small()
                            .color(p::muted()),
                    );
                    ui.label(
                        RichText::new(format!("· {}", row.companion.licence))
                            .small()
                            .color(p::yellow()),
                    );
                });
                ui.label(RichText::new(row.companion.what).small().color(p::fg()));
                ui.label(RichText::new(row.companion.why).small().color(p::muted()));
                ui.add_space(4.0);

                let (text, colour) = match &row.presence {
                    Presence::Present(_) => (row.presence.describe(), p::green()),
                    Presence::NotDetected => (
                        format!(
                            "{}. That is where VeilVoice looked, not a claim about your machine",
                            row.presence.describe()
                        ),
                        p::muted(),
                    ),
                    Presence::Unknown(_) => (row.presence.describe(), p::yellow()),
                };
                ui.label(RichText::new(text).small().color(colour));

                if row.presence.is_present() {
                    return;
                }
                ui.add_space(6.0);
                match &row.offer {
                    Offer::Page(url) => {
                        ui.label(
                            RichText::new(
                                "VeilVoice will not install this for you: it is not free \
                                 software and it is a driver. This opens their page, where \
                                 their licence is yours to accept and their installer is \
                                 yours to run.",
                            )
                            .small()
                            .color(p::muted()),
                        );
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(
                                    RichText::new("open their page").color(p::blue()),
                                ),
                            )
                            .clicked()
                        {
                            action = Some((row.companion.name.to_string(), row.offer.clone()));
                        }
                        ui.label(RichText::new(*url).small().color(p::muted()));
                    }
                    Offer::Command {
                        via,
                        needs_privilege,
                        ..
                    } => {
                        let line = row.offer.command_line().unwrap_or_default();
                        ui.label(
                            RichText::new(format!("via {via}: {line}"))
                                .small()
                                .color(p::cyan()),
                        );
                        if *needs_privilege {
                            ui.label(
                                RichText::new(
                                    "This needs root. VeilVoice does not ask for a password \
                                     and will not run it. Run that command in a terminal, \
                                     where you can see what you are approving.",
                                )
                                .small()
                                .color(p::yellow()),
                            );
                        } else if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(
                                    RichText::new(format!("install {}", row.companion.name))
                                        .color(p::green()),
                                ),
                            )
                            .clicked()
                        {
                            action = Some((row.companion.name.to_string(), row.offer.clone()));
                        }
                    }
                    Offer::PartOfTheSystem(explanation) => {
                        ui.label(RichText::new(*explanation).small().color(p::muted()));
                    }
                    Offer::NoKnownRoute(reason) => {
                        ui.label(RichText::new(reason).small().color(p::yellow()));
                    }
                    // Filtered out by `for_this_platform`, so unreachable in
                    // practice; drawn as nothing rather than as an empty group.
                    Offer::NotOnThisPlatform => {}
                }
            });
            ui.add_space(8.0);
        }

        if let Some((name, offer)) = action {
            match offer {
                Offer::Page(url) => match companions::open_page(url) {
                    Ok(()) => self.report = Some((vec![format!("opened {url}")], true)),
                    Err(error) => self.report = Some((vec![error], false)),
                },
                offer => {
                    self.start(&format!("installing {name}"), move || {
                        companions::run(&offer).map(|output| {
                            let mut lines: Vec<String> = output
                                .lines()
                                .filter(|line| !line.trim().is_empty())
                                .map(|line| line.to_string())
                                .collect();
                            lines.push("done".to_string());
                            lines
                        })
                    });
                }
            }
        }
    }
}

/// Probe for every companion that applies to this platform.
fn detect_all() -> Vec<Row> {
    companions::for_this_platform()
        .into_iter()
        .map(|companion| Row {
            companion,
            presence: companion.detect(),
            offer: companion.offer(),
        })
        .collect()
}

/// Exactly what an install changes, in the order it changes it.
///
/// Written out beside the button rather than in a manual. An installer that
/// says "install?" and nothing else is asking somebody to consent to something
/// they have not been told.
/// Whether the install button is live.
///
/// A function rather than four conditions in a closure, so the rule that
/// matters can be asserted: **a portable copy cannot be installed until the
/// question about its settings has been answered**. An install that ran on the
/// first click would have taken one of the two answers by default, and the
/// wrong one either copies a vault onto somebody else's machine or leaves a
/// person wondering where their recordings went.
fn can_install(
    somewhere_to_install: bool,
    already_running_installed: bool,
    portable: bool,
    carry: Option<bool>,
) -> bool {
    somewhere_to_install && !already_running_installed && (!portable || carry.is_some())
}

fn install_changes() -> Vec<&'static str> {
    let mut lines = vec![
        "copies the VeilVoice programs beside this one into your own program directory",
        "adds that directory to your PATH, appending to the value it first reads",
    ];
    if cfg!(windows) {
        lines.push("adds an entry to Apps & features, so Windows can list and remove it");
    }
    lines.push("nothing else: no service, no system directory, no administrator rights");
    lines
}

fn field(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label:<18}")).color(p::muted()));
        ui.label(RichText::new(value).color(p::cyan()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tab reads the machine on construction and must not change it.
    #[test]
    fn a_portable_copy_is_not_installed_until_the_question_is_answered() {
        // The one thing this gate exists for. Unanswered means the button is
        // dead, and either answer makes it live: both are real choices and
        // neither is a default.
        assert!(!can_install(true, false, true, None), "it installed anyway");
        assert!(can_install(true, false, true, Some(true)));
        assert!(can_install(true, false, true, Some(false)));

        // A copy that is not portable has nothing to carry, so it is never
        // asked and never waits.
        assert!(can_install(true, false, false, None));
    }

    #[test]
    fn nothing_installs_where_there_is_nowhere_to_install_it() {
        assert!(!can_install(false, false, false, None));
        assert!(!can_install(false, false, true, Some(true)));
        assert!(
            !can_install(true, true, false, None),
            "the installed copy offered to install itself"
        );
    }

    #[test]
    fn opening_the_tab_changes_nothing() {
        let first = Setup::new();
        let second = Setup::new();
        assert_eq!(first.status.installed, second.status.installed);
        assert_eq!(first.status.on_path, second.status.on_path);
        assert_eq!(first.status.prefix, second.status.prefix);
        assert_eq!(first.rows.len(), second.rows.len());
        assert!(first.job.is_none());
        assert!(first.report.is_none());
        assert!(
            !first.confirming_uninstall,
            "the uninstall confirmation must start closed"
        );
    }

    /// Drive the tab once with no window, in both motion states.
    ///
    /// The progress strip does arithmetic on a rectangle, and a rectangle in a
    /// headless context can be zero-width. A panic there would only ever be
    /// seen on somebody's desktop, mid-install.
    fn render(setup: &mut Setup, motion: crate::prefs::Motion) {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| setup.tab(ui, motion));
        });
    }

    fn motion(enabled: bool) -> crate::prefs::Motion {
        crate::prefs::Motion {
            enabled,
            icon: enabled,
            system_reduced: !enabled,
        }
    }

    #[test]
    fn the_tab_renders_without_a_window() {
        let mut setup = Setup::new();
        render(&mut setup, motion(true));
        render(&mut setup, motion(false));
        // Drawing must not have started anything.
        assert!(setup.job.is_none(), "drawing the tab started a worker");
    }

    /// The uninstall confirmation is drawn, and drawing it must not act.
    #[test]
    fn the_uninstall_confirmation_renders_and_does_nothing_by_itself() {
        let mut setup = Setup::new();
        setup.confirming_uninstall = true;
        render(&mut setup, motion(true));
        assert!(setup.job.is_none());
        assert!(setup.report.is_none());
    }

    /// A report from a worker that vanished must say so rather than leaving
    /// the strip running for ever.
    #[test]
    fn a_worker_that_disappears_is_reported() {
        let mut setup = Setup::new();
        let (tx, rx) = mpsc::channel::<Done>();
        setup.job = Some(rx);
        setup.busy = Some("installing".to_string());
        drop(tx);
        setup.poll();
        assert!(setup.job.is_none(), "the job must be cleared");
        assert!(!setup.is_busy());
        let (lines, good) = setup.report.expect("a disappearance must be reported");
        assert!(!good);
        assert!(lines[0].contains("stopped without reporting"), "{lines:?}");
    }

    /// Every companion shown is one that applies here, with a probe result.
    #[test]
    fn every_row_has_been_probed() {
        for row in detect_all() {
            assert!(!matches!(row.offer, Offer::NotOnThisPlatform));
            if let Presence::Unknown(reason) = &row.presence {
                assert!(!reason.contains("no probe is written"), "{reason}");
            }
        }
    }

    /// The list of changes is what the user consents to, so it must name the
    /// three things `veilvoice_setup::install` actually does, and must not
    /// stop mentioning that it asks for no administrator rights.
    #[test]
    fn the_consent_text_names_what_install_changes() {
        let lines = install_changes();
        let joined = lines.join(" ");
        assert!(joined.contains("PATH"), "{joined}");
        assert!(joined.contains("copies"), "{joined}");
        assert!(
            joined.contains("no administrator rights"),
            "the per-user promise must be stated beside the button: {joined}"
        );
        if cfg!(windows) {
            assert!(joined.contains("Apps & features"), "{joined}");
        }
    }

    /// A front end must not offer a button for anything the library refuses to
    /// run. This is the same rule as `companions::run`, checked from the side
    /// that draws the button.
    #[test]
    fn no_button_is_drawn_for_a_command_that_cannot_be_run() {
        for row in detect_all() {
            if let Offer::Command {
                needs_privilege, ..
            } = row.offer
            {
                assert_eq!(
                    !needs_privilege,
                    row.offer.is_runnable(),
                    "{} disagrees with the library about whether it is runnable",
                    row.companion.key
                );
            }
        }
    }
}

#[cfg(test)]
mod companion_tests {
    /// The probe runs off the paint thread.
    ///
    /// `detect_all` runs a command per companion, and on Windows one of them
    /// enumerates sound devices through PowerShell. On the paint thread that is
    /// hundreds of milliseconds inside a single frame, which is the window
    /// going white and the pointer becoming a spinner. Pressing "look again"
    /// did exactly that.
    #[test]
    fn looking_again_does_not_probe_on_the_paint_thread() {
        let source = std::fs::read_to_string("src/setup.rs").expect("its own source");
        let at = source
            .find("fn companion_rows")
            .expect("the companion rows");
        let end = source[at..]
            .find("\n    fn ")
            .map(|offset| at + offset)
            .unwrap_or(source.len());
        let body = &source[at..end];

        assert!(
            body.contains("std::thread::spawn"),
            "the probe is started on the thread that is painting"
        );
        assert!(
            !body.contains("self.rows = detect_all()"),
            "the rows are still being filled in by probing inside the frame"
        );
        assert!(
            body.contains("ui.spinner()"),
            "nothing on screen says the probe is running, so the button reads as dead"
        );
    }

    /// Every message that names a missing ffmpeg says where to get it.
    ///
    /// Naming a thing somebody cannot act on from where they are standing is
    /// the failure this guards: the render said "ffmpeg is not on this machine"
    /// and the tab that installs software had never heard of it.
    #[test]
    fn a_missing_ffmpeg_always_says_where_it_can_be_installed() {
        for file in ["src/studio.rs", "src/group.rs"] {
            let source = std::fs::read_to_string(file).expect("a source file");
            for (at, _) in source.match_indices("`ffmpeg` is not on") {
                // The sentence and what follows it, which is where the offer
                // has to be. Bounded rather than to the end of the file, so a
                // mention of Setup somewhere else entirely cannot satisfy this.
                let window = &source[at..(at + 600).min(source.len())];
                assert!(
                    window.contains("Setup tab"),
                    "{file} says ffmpeg is missing without saying where to get it"
                );
            }
        }
    }
}
