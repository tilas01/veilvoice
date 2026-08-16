// SPDX-License-Identifier: GPL-3.0-or-later
//! The VeilVoice desktop application.

use crate::security::Security;
use crate::theme::palette as p;
use egui::{Color32, RichText};
use std::path::PathBuf;
use std::sync::mpsc;
use veilvoice_audio::devices;
use veilvoice_core::{AccentConfig, DeidConfig};

/// The things VeilVoice does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    /// Process a file on disk.
    File,
    /// Scramble a microphone in real time.
    Live,
    /// Who is using the microphone and camera.
    Watch,
    /// The app lock, and what it is worth.
    Security,
    /// Versions, licence and honest scope.
    About,
}

/// Result of a background file job.
enum JobDone {
    Ok {
        output: PathBuf,
        secs: f32,
        speed: f32,
        metadata: Vec<String>,
    },
    Failed(String),
}

/// Application state.
pub struct VeilVoiceApp {
    tab: Tab,
    jetbrains: bool,

    // Shared engine settings.
    intensity: f32,
    neutralise_accent: bool,
    reseed_secs: f32,

    // File mode.
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    clean_metadata: bool,
    job: Option<mpsc::Receiver<JobDone>>,
    status: Option<(String, Color32)>,
    last_metadata: Vec<String>,

    // Live mode.
    inputs: Vec<devices::DeviceInfo>,
    outputs: Vec<devices::DeviceInfo>,
    chosen_input: Option<String>,
    chosen_output: Option<String>,
    session: Option<veilvoice_audio::LiveSession>,
    live_error: Option<String>,
    meter_in: f32,
    meter_out: f32,

    // The app lock, and at-rest encryption of what jobs write.
    security: Security,

    // Device monitor.
    watch: veilvoice_watch::Monitor,
    watch_support: veilvoice_watch::Support,
    watch_error: Option<String>,
    watch_log: Vec<String>,
    watch_next_poll: f64,
}

impl Default for VeilVoiceApp {
    fn default() -> Self {
        let inputs = devices::list(devices::Direction::Input).unwrap_or_default();
        let outputs = devices::list(devices::Direction::Output).unwrap_or_default();
        // Default the output to a virtual cable when one exists: routing there
        // is what lets other applications hear the veiled voice at all.
        let chosen_output = outputs
            .iter()
            .find(|d| d.is_virtual_cable)
            .or_else(|| outputs.iter().find(|d| d.is_default))
            .map(|d| d.name.clone());
        let chosen_input = inputs
            .iter()
            .find(|d| d.is_default)
            .or_else(|| inputs.first())
            .map(|d| d.name.clone());

        Self {
            tab: Tab::File,
            jetbrains: false,
            intensity: 1.0,
            neutralise_accent: true,
            reseed_secs: 2.0,
            input: None,
            output: None,
            clean_metadata: true,
            job: None,
            status: None,
            last_metadata: Vec::new(),
            inputs,
            outputs,
            chosen_input,
            chosen_output,
            session: None,
            live_error: None,
            meter_in: 0.0,
            meter_out: 0.0,
            security: Security::default(),
            watch: veilvoice_watch::Monitor::new(),
            watch_support: veilvoice_watch::support(),
            watch_error: None,
            watch_log: Vec::new(),
            watch_next_poll: 0.0,
        }
    }
}

impl VeilVoiceApp {
    /// Build the app, applying theme and fonts to `ctx`.
    ///
    /// This is where the lock file is read, rather than in `Default`: tests and
    /// anything else constructing the app must not touch the real one.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let jetbrains = crate::theme::install_fonts(&cc.egui_ctx);
        crate::theme::install(&cc.egui_ctx);
        Self {
            jetbrains,
            security: Security::load(),
            ..Default::default()
        }
    }

    fn config(&self) -> DeidConfig {
        DeidConfig {
            intensity: self.intensity,
            accent: AccentConfig {
                enabled: self.neutralise_accent,
                ..AccentConfig::default()
            },
            reseed_secs: self.reseed_secs,
            ..DeidConfig::default()
        }
    }
}

impl eframe::App for VeilVoiceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job();

        // The gate comes before everything: while locked, no device list, no
        // file names and no live session are reachable or even drawn.
        if self.security.is_locked() {
            self.session = None;
            egui::CentralPanel::default().show(ctx, |ui| self.security.unlock_screen(ui));
            // The rate limit counts down whether or not anything else moves.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            return;
        }

        let dialogue_open = self.security.disable_dialogue(ctx);

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("VEILVOICE").size(20.0).color(p::FG).strong());
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).color(p::MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("offline").color(p::GREEN).small());
                    if self.security.has_lock()
                        && ui
                            .button(RichText::new("lock").color(p::YELLOW).small())
                            .on_hover_text("Lock the app and clear the session passphrase")
                            .clicked()
                    {
                        self.security.lock_now();
                    }
                    // A monitor you have to go looking for is not doing its
                    // job, so the warning rides the header on every tab.
                    self.watch_indicator(ui);
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (Tab::File, "anonymise file"),
                    (Tab::Live, "live scramble"),
                    (Tab::Watch, "monitor"),
                    (Tab::Security, "lock"),
                    (Tab::About, "about"),
                ] {
                    let selected = self.tab == tab;
                    let text =
                        RichText::new(label).color(if selected { p::BLUE } else { p::MUTED });
                    if ui.selectable_label(selected, text).clicked() {
                        self.tab = tab;
                    }
                }
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // While the "unencrypted?" question is open, clicks must not land
            // on the window behind it.
            ui.add_enabled_ui(!dialogue_open, |ui| match self.tab {
                Tab::File => self.file_tab(ui),
                Tab::Live => self.live_tab(ui),
                Tab::Watch => self.watch_tab(ui),
                Tab::Security => self.security.tab(ui),
                Tab::About => self.about_tab(ui),
            });
        });

        self.poll_watch(ctx.input(|i| i.time));

        // The live meters only move if something repaints them, and the
        // monitor has to keep ticking even while the window is idle.
        if self.session.is_some() || self.job.is_some() || self.security.is_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else if self.watch_support.microphone || self.watch_support.camera {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl VeilVoiceApp {
    fn poll_job(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(JobDone::Ok {
                output,
                secs,
                speed,
                metadata,
            }) => {
                self.status = Some((
                    format!(
                        "done in {secs:.1}s ({speed:.0}x realtime) → {}",
                        output.display()
                    ),
                    p::GREEN,
                ));
                self.last_metadata = metadata;
                self.job = None;
            }
            Ok(JobDone::Failed(message)) => {
                self.status = Some((message, p::RED));
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = Some(("the processing thread stopped unexpectedly".into(), p::RED));
                self.job = None;
            }
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("SETTINGS").color(p::BLUE).small());
        ui.add(
            egui::Slider::new(&mut self.intensity, 0.0..=1.0)
                .text("intensity")
                .fixed_decimals(2),
        );
        ui.checkbox(
            &mut self.neutralise_accent,
            "neutralise accent and intonation",
        );
        ui.label(
            RichText::new(if self.neutralise_accent {
                "every speaker is mapped onto one canonical register and vocal tract"
            } else {
                "the speaker's accent, intonation and vocal tract are left intact"
            })
            .color(p::MUTED)
            .small(),
        );

        ui.add(
            egui::Slider::new(&mut self.reseed_secs, 0.0..=30.0)
                .text("seed roll (s)")
                .fixed_decimals(1),
        );
        ui.label(
            RichText::new(if self.reseed_secs <= 0.0 {
                "one modulation stream for the whole session"
            } else {
                "the modulation stream rolls forward; earlier audio is sealed off behind it"
            })
            .color(p::MUTED)
            .small(),
        );
    }

    fn file_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("INPUT").color(p::BLUE).small());
        ui.horizontal(|ui| {
            if ui.button("choose file…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "audio",
                        &["wav", "mp3", "flac", "ogg", "m4a", "aac", "opus"],
                    )
                    .pick_file()
                {
                    let mut out = path.clone();
                    out.set_extension("veiled.wav");
                    self.input = Some(path);
                    self.output = Some(out);
                    self.status = None;
                }
            }
            match &self.input {
                Some(path) => ui.label(RichText::new(path.display().to_string()).color(p::CYAN)),
                None => ui.label(RichText::new("no file selected").color(p::MUTED)),
            };
        });

        ui.add_space(8.0);
        self.settings(ui);
        ui.checkbox(&mut self.clean_metadata, "strip metadata from the result");

        ui.add_space(12.0);
        self.security.recording_controls(ui);

        ui.add_space(12.0);
        let busy = self.job.is_some();
        let ready = self.input.is_some() && !busy && self.security.ready_to_write();
        let button = ui.add_enabled(
            ready,
            egui::Button::new(RichText::new("  anonymise  ").strong()),
        );
        if button.clicked() {
            self.start_job();
        }
        if let Some(reason) = self.security.blocked_reason() {
            ui.label(RichText::new(reason).color(p::YELLOW).small());
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("processing…").color(p::MUTED));
            });
        }

        if let Some((message, colour)) = &self.status {
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(*colour));
        }
        if !self.last_metadata.is_empty() {
            ui.label(
                RichText::new(format!(
                    "metadata removed: {}",
                    self.last_metadata.join(", ")
                ))
                .color(p::MUTED)
                .small(),
            );
        }

        ui.add_space(16.0);
        ui.separator();
        ui.label(
            RichText::new(
                "The words survive on purpose — a scrambler you cannot understand is \
                 useless. Encrypting the result at rest is what keeps them from being \
                 read off the disk afterwards, which is why it is on by default.",
            )
            .color(p::MUTED)
            .small(),
        );
    }

    fn start_job(&mut self) {
        let Some(input) = self.input.clone() else {
            return;
        };
        let output = self.output.clone().unwrap_or_else(|| {
            let mut o = input.clone();
            o.set_extension("veiled.wav");
            o
        });
        let config = self.config();
        let clean = self.clean_metadata;
        let plan = self.security.plan();
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.status = None;
        self.last_metadata.clear();

        // Off the UI thread: a long file would otherwise freeze the window, and
        // Argon2id at 256 MiB is deliberately slow on top of that.
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let result = (|| -> Result<(PathBuf, f32, Vec<String>), String> {
                let audio = veilvoice_audio::io::load(&input).map_err(|e| e.to_string())?;
                let veiled =
                    veilvoice_audio::deidentify(&audio, config).map_err(|e| e.to_string())?;

                // Encoded in memory, so a recording that is going to be sealed
                // never lands on the disk in the clear even briefly.
                let mut wav = veilvoice_audio::io::wav_bytes(&veiled).map_err(|e| e.to_string())?;
                let mut removed = Vec::new();
                if clean {
                    if let Ok((cleaned, report)) =
                        veilvoice_meta::clean_wav_bytes(&wav, veilvoice_meta::Policy::Strip)
                    {
                        wav = cleaned;
                        removed = report.removed;
                    }
                }
                let written =
                    plan.write(&output, &wav, veilvoice_crypto::kdf::KdfParams::default())?;
                Ok((written, audio.duration_secs(), removed))
            })();

            let secs = started.elapsed().as_secs_f32();
            let _ = tx.send(match result {
                Ok((output, duration, metadata)) => JobDone::Ok {
                    output,
                    secs,
                    speed: duration / secs.max(1e-6),
                    metadata,
                },
                Err(message) => JobDone::Failed(message),
            });
        });
    }

    fn live_tab(&mut self, ui: &mut egui::Ui) {
        let running = self.session.is_some();

        ui.add_space(4.0);
        ui.label(RichText::new("DEVICES").color(p::BLUE).small());
        ui.add_enabled_ui(!running, |ui| {
            device_picker(ui, "input ", &self.inputs, &mut self.chosen_input);
            device_picker(ui, "output", &self.outputs, &mut self.chosen_output);
        });

        let routed = self
            .chosen_output
            .as_ref()
            .and_then(|name| self.outputs.iter().find(|d| &d.name == name))
            .map(|d| d.is_virtual_cable)
            .unwrap_or(false);
        if !routed {
            ui.label(
                RichText::new(
                    "no virtual cable selected — other applications will not receive this",
                )
                .color(p::YELLOW)
                .small(),
            );
        }

        ui.add_space(8.0);
        ui.add_enabled_ui(!running, |ui| self.settings(ui));

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if !running {
                if ui.button(RichText::new("  start  ").strong()).clicked() {
                    self.start_live();
                }
            } else if ui.button(RichText::new("  stop  ").strong()).clicked() {
                self.session = None;
                self.meter_in = 0.0;
                self.meter_out = 0.0;
            }
            if running {
                ui.label(RichText::new("● live").color(p::GREEN));
            }
        });

        if let Some(message) = &self.live_error {
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(p::RED));
        }

        if let Some(session) = &self.session {
            let stats = session.stats();
            // Meters fall smoothly rather than flickering with every frame.
            self.meter_in = (self.meter_in * 0.7).max(stats.input_peak);
            self.meter_out = (self.meter_out * 0.7).max(stats.output_peak);

            ui.add_space(12.0);
            ui.label(RichText::new("LEVELS").color(p::BLUE).small());
            meter(ui, "in ", self.meter_in);
            meter(ui, "out", self.meter_out);

            ui.add_space(12.0);
            ui.label(RichText::new("PERFORMANCE").color(p::BLUE).small());
            field(
                ui,
                "processing",
                &format!("{:.2} ms/block", stats.process.ema_block_ms()),
            );
            field(
                ui,
                "engine latency",
                &format!("{:.1} ms", stats.process.algorithmic_latency_ms),
            );
            field(
                ui,
                "realtime factor",
                &format!("{:.3}", stats.process.last_realtime_factor()),
            );
            if stats.dropped > 0 || stats.starved > 0 {
                ui.label(
                    RichText::new(format!(
                        "glitches: {} dropped, {} starved",
                        stats.dropped, stats.starved
                    ))
                    .color(p::YELLOW)
                    .small(),
                );
            }
        }
    }

    fn start_live(&mut self) {
        self.live_error = None;
        let result = (|| {
            let input = devices::open(devices::Direction::Input, self.chosen_input.as_deref())?;
            let output = devices::open(devices::Direction::Output, self.chosen_output.as_deref())?;
            veilvoice_audio::LiveSession::start(&input, &output, self.config())
        })();
        match result {
            Ok(session) => self.session = Some(session),
            Err(e) => self.live_error = Some(e.to_string()),
        }
    }

    /// Re-scan on a timer rather than every frame.
    fn poll_watch(&mut self, now: f64) {
        if !(self.watch_support.microphone || self.watch_support.camera) {
            return;
        }
        if now < self.watch_next_poll {
            return;
        }
        self.watch_next_poll = now + 2.0;

        match self.watch.poll() {
            Ok(changes) => {
                self.watch_error = None;
                for change in changes {
                    self.watch_log.push(change.alert());
                }
                // A log that grows without bound is a memory leak with a UI.
                let overflow = self.watch_log.len().saturating_sub(50);
                self.watch_log.drain(..overflow);
            }
            Err(e) => self.watch_error = Some(e.to_string()),
        }
    }

    /// The always-visible indicator.
    fn watch_indicator(&mut self, ui: &mut egui::Ui) {
        if !(self.watch_support.microphone || self.watch_support.camera) {
            return;
        }
        let active = self.watch.current();
        if active.is_empty() {
            return;
        }

        let camera = active
            .iter()
            .any(|u| u.kind == veilvoice_watch::DeviceKind::Camera);
        let colour = if camera { p::RED } else { p::YELLOW };
        let names: Vec<&str> = active.iter().map(|u| u.app.as_str()).collect();
        let label = format!(
            "* {} IN USE - {}",
            if camera { "CAMERA" } else { "MIC" },
            names.join(", ")
        );

        if ui
            .label(RichText::new(label).color(colour).small().strong())
            .on_hover_text("Open the monitor tab for detail")
            .clicked()
        {
            self.tab = Tab::Watch;
        }
    }

    fn watch_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("WHAT IS LISTENING").color(p::BLUE).small());
        ui.label(
            RichText::new(self.watch_support.explanation)
                .color(p::MUTED)
                .small(),
        );

        // An empty list from a platform that cannot see is not good news, and
        // must never be allowed to read like it.
        if !(self.watch_support.microphone || self.watch_support.camera) {
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "This platform exposes no way to tell which application is using \
                     the microphone or camera, so nothing is shown. That is not the \
                     same as nothing being active.",
                )
                .color(p::YELLOW),
            );
            return;
        }

        if let Some(e) = &self.watch_error {
            ui.label(RichText::new(e).color(p::RED));
        }

        ui.add_space(10.0);
        let active: Vec<_> = self.watch.current().to_vec();
        if active.is_empty() {
            ui.label(RichText::new("Nothing is using the microphone or camera.").color(p::GREEN));
        } else {
            for entry in &active {
                let colour = if entry.kind == veilvoice_watch::DeviceKind::Camera {
                    p::RED
                } else {
                    p::YELLOW
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("*").color(colour));
                    ui.label(RichText::new(entry.describe()).color(p::FG).strong());
                    ui.label(RichText::new(entry.kind.to_string()).color(colour).small());
                });
                if let Some(path) = &entry.path {
                    ui.label(RichText::new(format!("    {path}")).color(p::MUTED).small());
                }
                if let Some(held) = entry.held_for() {
                    ui.label(
                        RichText::new(format!("    held for {}s", held.as_secs()))
                            .color(p::MUTED)
                            .small(),
                    );
                }
                ui.add_space(6.0);
            }
        }

        if !self.watch_log.is_empty() {
            ui.add_space(14.0);
            ui.separator();
            ui.label(RichText::new("RECENT").color(p::BLUE).small());
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for line in self.watch_log.iter().rev() {
                        ui.label(RichText::new(line).color(p::MUTED).small());
                    }
                });
        }
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        field(ui, "app", env!("CARGO_PKG_VERSION"));
        field(ui, "engine", veilvoice_core::VERSION);
        field(ui, "audio", veilvoice_audio::VERSION);
        field(ui, "metadata", veilvoice_meta::VERSION);
        field(ui, "monitor", veilvoice_watch::VERSION);
        field(ui, "crypto", veilvoice_crypto::VERSION);
        field(ui, "licence", "GPL-3.0-or-later");
        field(ui, "network access", "none, by construction");
        field(
            ui,
            "typeface",
            if self.jetbrains {
                "JetBrains Mono"
            } else {
                "built-in monospace"
            },
        );

        ui.add_space(16.0);
        ui.label(RichText::new("WHAT THIS PROTECTS").color(p::BLUE).small());
        ui.label(
            RichText::new(
                "The biometric voiceprint — pitch, formants, timbre, micro-timing and \
                 the melody of an accent — is destroyed and cannot be recovered from the \
                 output. Each frame's measured phase is discarded, and every speaker is \
                 mapped onto one canonical register and vocal tract.",
            )
            .color(p::FG),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("WHAT IT DOES NOT").color(p::YELLOW).small());
        ui.label(
            RichText::new(
                "The words are preserved on purpose, so de-identification alone does \
                 not keep the message secret — which is why the result is encrypted at \
                 rest by default. Nor can any signal-level transform change which \
                 phonemes you produced, so a strong regional accent may still be \
                 audible even though its melody is gone.",
            )
            .color(p::FG),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("THE APP LOCK").color(p::YELLOW).small());
        ui.label(RichText::new(veilvoice_crypto::lock::SCOPE).color(p::FG));
    }
}

fn device_picker(
    ui: &mut egui::Ui,
    label: &str,
    devices: &[devices::DeviceInfo],
    chosen: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::MUTED));
        let current = chosen.clone().unwrap_or_else(|| "system default".into());
        egui::ComboBox::from_id_salt(label)
            .width(360.0)
            .selected_text(RichText::new(current).color(p::CYAN))
            .show_ui(ui, |ui| {
                ui.selectable_value(chosen, None, "system default");
                for device in devices {
                    let mut text = device.name.clone();
                    if device.is_virtual_cable {
                        text.push_str("  ·  virtual cable");
                    }
                    ui.selectable_value(chosen, Some(device.name.clone()), text);
                }
            });
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label:<18}")).color(p::MUTED));
        ui.label(RichText::new(value).color(p::CYAN));
    });
}

fn meter(ui: &mut egui::Ui, label: &str, peak: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::MUTED));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(280.0, 12.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, p::BG_DARK);

        let level = peak.clamp(0.0, 1.0);
        let colour = if level > 0.95 {
            p::RED
        } else if level > 0.7 {
            p::YELLOW
        } else {
            p::GREEN
        };
        let mut filled = rect;
        filled.set_width(rect.width() * level);
        painter.rect_filled(filled, 2.0, colour);
        painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, p::BORDER));

        ui.label(
            RichText::new(format!("{:>5.1} dB", 20.0 * level.max(1e-4).log10())).color(p::MUTED),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_safe_ones() {
        let app = VeilVoiceApp::default();
        assert!(
            app.neutralise_accent,
            "accent neutralisation should default on"
        );
        assert!(app.clean_metadata, "metadata stripping should default on");
        assert_eq!(app.intensity, 1.0);
        assert_eq!(app.reseed_secs, 2.0, "the seed should roll by default");
        assert!(app.session.is_none());
        assert!(
            app.security.encrypt_recordings,
            "recordings should be encrypted at rest by default"
        );
    }

    /// The default is only worth anything if the button honours it: with
    /// encryption on and nothing to encrypt with, a job must not start.
    #[test]
    fn a_job_cannot_start_before_the_at_rest_choice_is_made() {
        let app = VeilVoiceApp::default();
        assert!(!app.security.ready_to_write());
        assert!(app.security.blocked_reason().is_some());
    }

    #[test]
    fn config_reflects_the_controls() {
        let app = VeilVoiceApp {
            intensity: 0.5,
            neutralise_accent: false,
            reseed_secs: 5.0,
            ..Default::default()
        };
        let cfg = app.config();
        assert_eq!(cfg.intensity, 0.5);
        assert!(!cfg.accent.enabled);
        assert_eq!(cfg.reseed_secs, 5.0);
        cfg.checked()
            .expect("every value the sliders can reach must be valid");
    }

    /// The slider's whole range must produce a configuration the engine
    /// accepts, or a user could drag it into an error.
    ///
    /// One app, mutated, rather than thirty-one built from scratch: every
    /// `VeilVoiceApp::default()` enumerates the machine's audio devices through
    /// `cpal`, and doing that thirty-one times in a loop is a slow way to test
    /// arithmetic — and on a headless CI runner, an unnecessary way to lean on
    /// the platform's audio stack.
    #[test]
    fn every_reachable_reseed_setting_is_valid() {
        let mut app = VeilVoiceApp::default();
        for step in 0..=30 {
            app.reseed_secs = step as f32;
            app.config()
                .checked()
                .unwrap_or_else(|e| panic!("reseed_secs={step} rejected: {e}"));
        }
    }

    /// A virtual cable should be preselected when the machine has one, because
    /// routing there is the whole point of live mode.
    #[test]
    fn output_defaults_to_a_virtual_cable_when_present() {
        let app = VeilVoiceApp::default();
        let cables: Vec<_> = app.outputs.iter().filter(|d| d.is_virtual_cable).collect();
        if !cables.is_empty() {
            let chosen = app
                .chosen_output
                .as_deref()
                .expect("something should be chosen");
            assert!(
                cables.iter().any(|c| c.name == chosen),
                "expected a virtual cable, got {chosen}"
            );
        }
    }

    #[test]
    fn building_the_app_without_audio_hardware_does_not_panic() {
        let _ = VeilVoiceApp::default();
    }
}
