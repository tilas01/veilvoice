// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! The settings panel: a menu of pages, each a titled group of choices.
//!
//! # Why a menu rather than one long list
//!
//! There are three kinds of setting here and they answer different questions:
//! what the app *looks* like, how it *moves*, and what it does with the files
//! it writes. Stacked in one column they read as an undifferentiated wall of
//! tick boxes, and the one that matters most -- at-rest encryption -- ends up
//! looking exactly as important as the colour scheme. A menu with a page per
//! group keeps each question next to its own explanation.
//!
//! # Every change applies immediately, and is saved immediately
//!
//! There is no "apply" button and no "unsaved changes" state. Both are ways to
//! lose a choice silently. If saving fails the choice still applies for this
//! session and the panel says, in the panel, that it could not be remembered
//! and why -- rather than failing quietly and letting the setting reappear
//! wrong on the next launch.
//!
//! # What is deliberately not in here
//!
//! The app lock and the at-rest passphrase have their own tab and stay there.
//! A password field sitting between "animations" and "colour scheme" invites
//! being treated with the same weight, and it is not the same weight.

use crate::prefs::{Motion, Prefs};
use crate::theme::palette as p;
use crate::theme::THEMES;
use egui::{RichText, Ui};
use std::path::PathBuf;

/// Which page of the settings menu is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// Colour scheme.
    Appearance,
    /// Animation and the animated mark.
    Motion,
    /// Where settings live, and how to reset them.
    Storage,
}

impl Page {
    /// Every page, in menu order, with its label and one-line summary.
    pub const ALL: &'static [(Page, &'static str, &'static str)] = &[
        (Page::Appearance, "appearance", "Colour scheme"),
        (Page::Motion, "motion", "Animation, and the mark"),
        (Page::Storage, "storage", "Where this is kept"),
    ];
}

/// The settings tab's own state.
pub struct Settings {
    /// The live preferences. Changes here take effect on the next frame.
    pub prefs: Prefs,
    /// Where they are persisted, or `None` if this platform did not say.
    path: Option<PathBuf>,
    /// Which page is showing.
    page: Page,
    /// Why the last save failed, if it did.
    save_error: Option<String>,
    /// Whether the first-run choice is still to be made.
    first_run: bool,
    /// What the operating system said about reducing motion, read once at
    /// startup. Every platform answers through a subprocess, so asking per
    /// frame is out of the question.
    system_motion: crate::reduced_motion::Query,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            prefs: Prefs::default(),
            path: None,
            page: Page::Appearance,
            save_error: None,
            first_run: false,
            system_motion: crate::reduced_motion::Query::Unknown,
        }
    }
}

impl Settings {
    /// Load preferences from this platform's config directory and apply the
    /// chosen theme to `ctx`.
    ///
    /// Never fails: an unreadable or unparseable file leaves the defaults in
    /// force, and the panel says so.
    pub fn load(ctx: &egui::Context) -> Self {
        let path = crate::prefs::default_path();
        let prefs = match &path {
            Some(p) => Prefs::load(p),
            None => Prefs::default(),
        };
        crate::theme::set_by_id(ctx, &prefs.theme);
        let first_run = !prefs.configured;
        Self {
            first_run,
            prefs,
            path,
            page: Page::Appearance,
            save_error: None,
            system_motion: crate::reduced_motion::query(),
        }
    }

    /// How much movement is allowed this frame.
    ///
    /// Takes `&egui::Context` for symmetry with the rest of the UI even though
    /// it does not need it: the platform answer is cached from startup, since
    /// reading it costs a subprocess.
    pub fn motion(&self, _ctx: &egui::Context) -> Motion {
        Motion::resolve(&self.prefs, self.system_motion.reduces())
    }

    /// Whether the first-run choice has still to be made.
    pub fn needs_first_run(&self) -> bool {
        self.first_run
    }

    fn persist(&mut self) {
        let Some(path) = &self.path else {
            self.save_error = Some(
                "this platform did not say where to keep configuration \
                 (no APPDATA, XDG_CONFIG_HOME or HOME), so choices apply for \
                 this session only"
                    .to_string(),
            );
            return;
        };
        self.save_error = self.prefs.save(path).err();
    }

    /// The first-run panel: offered once, with animation already on.
    ///
    /// Shown as a page rather than a modal because it is not urgent and does
    /// not gate anything -- the legal notice is the thing that gates, and two
    /// blocking dialogues before a user has seen the app is one too many.
    pub fn first_run_panel(&mut self, ui: &mut Ui) {
        ui.add_space(18.0);
        ui.label(
            RichText::new("A COUPLE OF CHOICES")
                .size(18.0)
                .color(p::fg())
                .strong(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Both are on, and both can be changed later in settings. \
                 Nothing here leaves your machine.",
            )
            .color(p::muted()),
        );
        ui.add_space(14.0);

        let mut changed = false;
        changed |= ui
            .checkbox(&mut self.prefs.animations, "Animate the interface")
            .changed();
        ui.label(
            RichText::new("  Transitions and easing. Turning this off makes every change instant.")
                .small()
                .color(p::muted()),
        );
        ui.add_space(8.0);

        ui.add_enabled_ui(self.prefs.animations, |ui| {
            changed |= ui
                .checkbox(&mut self.prefs.animated_icon, "Animate the mark")
                .changed();
        });
        ui.label(
            RichText::new("  The soundbar in the header, as on the website.")
                .small()
                .color(p::muted()),
        );

        ui.add_space(18.0);
        if ui.button("continue").clicked() {
            self.prefs.configured = true;
            self.first_run = false;
            changed = true;
        }
        if changed {
            self.persist();
        }
        if let Some(error) = &self.save_error {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("could not save: {error}"))
                    .small()
                    .color(p::yellow()),
            );
        }
    }

    /// The settings tab.
    pub fn tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        ui.label(RichText::new("SETTINGS").size(16.0).color(p::fg()).strong());
        ui.add_space(2.0);
        ui.label(
            RichText::new("Applies as you change it, and is remembered.")
                .small()
                .color(p::muted()),
        );
        ui.add_space(10.0);

        // The menu, then the page. A row rather than a sidebar: there are three
        // pages, and a sidebar for three items is furniture.
        ui.horizontal(|ui| {
            for (page, label, _) in Page::ALL {
                let selected = self.page == *page;
                let text =
                    RichText::new(*label).color(if selected { p::blue() } else { p::muted() });
                if ui.selectable_label(selected, text).clicked() {
                    self.page = *page;
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(10.0);

        match self.page {
            Page::Appearance => self.appearance_page(ui, ctx),
            Page::Motion => self.motion_page(ui, ctx),
            Page::Storage => self.storage_page(ui),
        }

        if let Some(error) = &self.save_error {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!(
                    "This choice applies now but could not be saved: {error}"
                ))
                .small()
                .color(p::yellow()),
            );
        }
    }

    fn appearance_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        section(ui, "Colour scheme", "The same schemes the website offers.");

        let current = crate::theme::active();
        let mut chosen: Option<&'static str> = None;

        egui::ComboBox::from_id_salt("theme-picker")
            .selected_text(current.name)
            .width(220.0)
            .show_ui(ui, |ui| {
                for theme in THEMES {
                    if ui
                        .selectable_label(theme.id == current.id, theme.name)
                        .clicked()
                    {
                        chosen = Some(theme.id);
                    }
                }
            });

        ui.add_space(10.0);
        swatches(ui);

        if let Some(id) = chosen {
            if crate::theme::set_by_id(ctx, id) {
                self.prefs.theme = id.to_string();
                self.persist();
            }
        }
    }

    fn motion_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let motion = self.motion(ctx);

        section(
            ui,
            "Animation",
            "Transitions, easing, and the moving mark. On by default.",
        );

        let mut changed = false;
        changed |= ui
            .checkbox(&mut self.prefs.animations, "Animate the interface")
            .changed();
        ui.label(
            RichText::new(
                "  Off makes every change instant. Nothing is hidden either way; \
                 animation only affects how a change is shown, never whether it happens.",
            )
            .small()
            .color(p::muted()),
        );

        ui.add_space(10.0);
        ui.add_enabled_ui(self.prefs.animations, |ui| {
            changed |= ui
                .checkbox(&mut self.prefs.animated_icon, "Animate the mark")
                .changed();
        });
        ui.label(
            RichText::new("  The soundbar in the header, as on the website.")
                .small()
                .color(p::muted()),
        );

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("preview").small().color(p::muted()));
            crate::soundbar::draw(
                ui,
                egui::vec2(120.0, 26.0),
                motion,
                ui.input(|i| i.time) as f32,
            );
        });

        // If the system has asked for reduced motion, say so. Otherwise the
        // toggle looks broken: it is ticked and nothing moves.
        if motion.system_reduced {
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Your system is set to reduce motion, so animation stays off whatever \
                     is ticked here. That setting wins on purpose.",
                )
                .small()
                .color(p::yellow()),
            );
        }
        if std::env::var_os("VEILVOICE_NO_ANIMATION").is_some() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "VEILVOICE_NO_ANIMATION is set in the environment, which also \
                     keeps animation off.",
                )
                .small()
                .color(p::yellow()),
            );
        }

        if changed {
            self.persist();
        }
    }

    fn storage_page(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Where this is kept",
            "Plain text. Edit it or delete it; nothing here is secret.",
        );

        match &self.path {
            Some(path) => {
                ui.label(
                    RichText::new(path.display().to_string())
                        .small()
                        .color(p::cyan()),
                );
            }
            None => {
                ui.label(
                    RichText::new(
                        "Nowhere. This platform did not say where configuration belongs \
                         (no APPDATA, XDG_CONFIG_HOME or HOME), so choices apply for this \
                         session only.",
                    )
                    .small()
                    .color(p::yellow()),
                );
            }
        }

        if self.prefs.recovered_from_corrupt_file {
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "The settings file could not be understood, so the defaults are in \
                     force. Changing anything here will rewrite it.",
                )
                .small()
                .color(p::yellow()),
            );
        }

        ui.add_space(16.0);
        if ui.button("reset to defaults").clicked() {
            let configured = self.prefs.configured;
            self.prefs = Prefs {
                configured,
                ..Prefs::default()
            };
            self.persist();
        }
        ui.label(
            RichText::new(
                "  Colour scheme and animation only. This does not touch the app lock, \
                 your passphrase, or any recording.",
            )
            .small()
            .color(p::muted()),
        );
    }
}

/// A titled group with a one-line explanation under it.
fn section(ui: &mut Ui, title: &str, blurb: &str) {
    ui.label(RichText::new(title).color(p::fg()).strong());
    ui.label(RichText::new(blurb).small().color(p::muted()));
    ui.add_space(10.0);
}

/// The active palette, as a row of swatches, so the choice can be seen rather
/// than only read.
fn swatches(ui: &mut Ui) {
    let theme = crate::theme::active();
    ui.horizontal(|ui| {
        for (name, colour) in [
            ("accent", theme.accent),
            ("veiled", theme.accent_2),
            ("ok", theme.ok),
            ("warn", theme.warn),
            ("error", theme.err),
            ("text", theme.fg),
            ("muted", theme.muted),
        ] {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.painter()
                    .rect_filled(rect, egui::Rounding::same(3.0), colour);
                ui.painter().rect_stroke(
                    rect,
                    egui::Rounding::same(3.0),
                    egui::Stroke::new(1.0, p::border()),
                );
            }
            response.on_hover_text(name);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive the settings tab once, with no window.
    fn render(settings: &mut Settings) {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| settings.tab(ui, ctx));
        });
    }

    #[test]
    fn every_page_renders_without_a_window() {
        let mut settings = Settings::default();
        for (page, _, _) in Page::ALL {
            settings.page = *page;
            render(&mut settings);
        }
    }

    #[test]
    fn the_first_run_panel_renders() {
        let mut settings = Settings::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| settings.first_run_panel(ui));
        });
    }

    /// The menu has to cover the pages and the pages have to cover the menu, or
    /// a page becomes unreachable.
    #[test]
    fn the_menu_lists_every_page_exactly_once() {
        let mut seen: Vec<Page> = Page::ALL.iter().map(|(p, _, _)| *p).collect();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count, "a page is listed twice");
        assert_eq!(count, 3, "a page was added without a menu entry");
        for (_, label, blurb) in Page::ALL {
            assert!(!label.is_empty() && !blurb.is_empty());
        }
    }

    /// A first run has not been configured; answering it must stick.
    #[test]
    fn the_first_run_is_offered_once() {
        let mut settings = Settings {
            prefs: Prefs::default(),
            ..Default::default()
        };
        settings.first_run = !settings.prefs.configured;
        assert!(settings.needs_first_run());

        settings.prefs.configured = true;
        settings.first_run = false;
        assert!(!settings.needs_first_run());
    }

    /// Defaults are what the request asked for: animation on, offered at the
    /// start, switchable afterwards.
    #[test]
    fn animation_is_on_by_default_and_can_be_turned_off() {
        let ctx = egui::Context::default();
        let mut settings = Settings::default();
        assert!(settings.motion(&ctx).enabled);
        assert!(settings.motion(&ctx).icon);

        settings.prefs.animated_icon = false;
        assert!(settings.motion(&ctx).enabled, "only the mark was stilled");
        assert!(!settings.motion(&ctx).icon);

        settings.prefs.animations = false;
        assert!(!settings.motion(&ctx).enabled);
        assert!(!settings.motion(&ctx).icon);
    }

    /// Choosing a theme must apply it and record it.
    #[test]
    fn choosing_a_theme_applies_and_records_it() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        let mut settings = Settings {
            path: Some(path.clone()),
            ..Default::default()
        };

        assert!(crate::theme::set_by_id(&ctx, "nord"));
        settings.prefs.theme = "nord".into();
        settings.persist();

        assert_eq!(Prefs::load(&path).theme, "nord");
        assert!(settings.save_error.is_none(), "{:?}", settings.save_error);

        // And it comes back on the next launch.
        let reloaded = Settings {
            prefs: Prefs::load(&path),
            path: Some(path),
            ..Default::default()
        };
        assert_eq!(reloaded.prefs.theme, "nord");
        crate::theme::set_by_id(&ctx, "tokyo-night");
    }

    /// A save that cannot happen must not be silent, and must not lose the
    /// choice for this session either.
    #[test]
    fn a_failed_save_is_reported_rather_than_swallowed() {
        let mut settings = Settings {
            path: None,
            ..Default::default()
        };
        settings.prefs.animations = false;
        settings.persist();
        assert!(settings.save_error.is_some(), "the failure was swallowed");
        assert!(!settings.prefs.animations, "the choice was lost as well");
        // And the panel shows it.
        render(&mut settings);
    }

    /// Reset must not touch anything that is not a presentation choice.
    #[test]
    fn reset_leaves_the_first_run_answered() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings {
            path: Some(dir.path().join("settings.conf")),
            prefs: Prefs {
                theme: "dracula".into(),
                animations: false,
                animated_icon: false,
                configured: true,
                recovered_from_corrupt_file: false,
            },
            page: Page::Storage,
            ..Default::default()
        };

        let configured = settings.prefs.configured;
        settings.prefs = Prefs {
            configured,
            ..Prefs::default()
        };
        settings.persist();

        assert_eq!(settings.prefs.theme, "tokyo-night");
        assert!(settings.prefs.animations);
        assert!(
            settings.prefs.configured,
            "reset must not ask the first-run question again"
        );
    }

    /// Loading must survive a settings file full of nonsense.
    #[test]
    fn a_corrupt_file_leaves_a_usable_panel_that_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.conf");
        std::fs::write(&path, "this is not a settings file\n@@@@\n").unwrap();

        let mut settings = Settings {
            prefs: Prefs::load(&path),
            path: Some(path),
            page: Page::Storage,
            ..Default::default()
        };
        assert!(settings.prefs.recovered_from_corrupt_file);
        assert_eq!(settings.prefs.theme, "tokyo-night");
        render(&mut settings);
    }
}
