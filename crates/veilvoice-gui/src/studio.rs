// SPDX-License-Identifier: GPL-3.0-or-later
//! The Recording Studio and the Recording Browser.
//!
//! # Two tabs, one vault
//!
//! The Studio records; the Browser is what is in the vault afterwards. They are
//! one module because they are one vault, and a vault opened in two places is
//! two chances to get the unlocking wrong.
//!
//! # The vault needs both passphrases, and asks for both here
//!
//! [`veilvoice_crypto::studio::StudioKey`] is derived from the app lock **and**
//! the at-rest passphrase, and from neither alone. That is the whole point of
//! it: a laptop stolen while VeilVoice is unlocked opens nothing, because the
//! at-rest passphrase was never typed.
//!
//! So this tab asks for both, every time, rather than reaching into whatever
//! the rest of the application happens to be holding. That is a deliberate
//! inconvenience:
//!
//! - The app-lock secret is only kept for the session when
//!   [`Sealing::AppLock`](crate::security::Sealing::AppLock) is chosen. Reading
//!   it when it happens to be there, and prompting when it is not, would make
//!   the vault's strength depend on an unrelated setting, and nobody would
//!   know which they had.
//! - Prompting for both, always, is the only version of this whose security is
//!   the same on every run.
//!
//! Neither passphrase is stored. Both typing buffers are wiped the moment the
//! key is derived, and the derived key lives in page-locked memory for as long
//! as the vault is open. Locking the window closes the vault.
//!
//! # What is recorded is what comes out, never what went in
//!
//! The Studio records through
//! [`LiveSession::start_recording`](veilvoice_audio::LiveSession::start_recording),
//! which is the same path the command line uses, so the samples that reach the
//! recorder are the **veiled** ones. There is no code path here that captures
//! the microphone before the engine has been through it, because a path that
//! existed would eventually be taken, and the file it produced would be a
//! recording of somebody's real voice sitting in a vault they believed was
//! safe.
//!
//! The recording is assembled inside a [`Secret`](veilvoice_crypto::Secret) and
//! handed straight to the vault to be sealed. It is never a plain file, not
//! even briefly.
//!
//! # In plain words
//!
//! Record here, and what you record is kept locked up. Opening the cupboard
//! needs both of your passwords at once, every time, which is what makes it
//! worth having.

use egui::{Color32, RichText, Ui};
use veilvoice_core::DeidConfig;
use veilvoice_crypto::studio::{Entry, Studio as Vault, StudioKey};
use veilvoice_crypto::Secret;
use zeroize::Zeroize;

use crate::theme::palette as p;

/// Move a typed passphrase into page-locked storage and wipe the buffer.
///
/// The same shape as [`crate::security`]'s, and for the same reason: a text
/// widget owns a `String`, so the passphrase exists as ordinary heap bytes
/// while it is being typed. This shortens that window; nothing can close it.
fn into_secret(typed: &mut String) -> Secret {
    let mut bytes = typed.as_bytes().to_vec();
    let secret = Secret::new(&mut bytes);
    typed.zeroize();
    secret
}

/// Where the vault lives: beside the lock file, in this platform's config
/// directory. `None` when the environment does not say where that is, in which
/// case the Studio says so rather than inventing a location.
pub fn default_dir() -> Option<std::path::PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("studio"))
}

/// What the Studio is doing.
#[derive(PartialEq, Eq)]
enum Phase {
    /// The vault is shut. Both passphrases are wanted.
    Shut,
    /// Open, and not recording.
    Idle,
    /// Recording.
    Recording,
}

/// The Studio and the Browser.
///
/// Every field's default is the shut, empty state, so this derives rather
/// than being written out: a hand-written `Default` here would be a second
/// place to remember a new field, and forgetting one would leave it carrying
/// whatever the last session put in it.
#[derive(Default)]
pub struct Studio {
    /// The open vault. `None` is the shut state, and is the default.
    vault: Option<Vault>,
    /// The listing, read when the vault opens and after every change rather
    /// than every frame: a frame is 16 milliseconds and this decrypts a file.
    entries: Vec<Entry>,

    // --- the unlock form ---
    app_entry: String,
    rest_entry: String,

    // --- recording ---
    session: Option<veilvoice_audio::LiveSession>,
    recorder: Option<veilvoice_audio::record::Recorder>,
    /// What the next take will be called.
    take_name: String,

    // --- the browser ---
    /// Which take is selected, by identifier.
    selected: Option<String>,
    /// The take being renamed, and the name being typed for it.
    renaming: Option<(String, String)>,
    /// The take a removal is waiting to be confirmed for.
    confirm_remove: Option<String>,

    /// The last thing worth saying, and the colour to say it in.
    message: Option<(String, Color32)>,
}

impl Studio {
    /// Whether the vault is open.
    pub fn is_open(&self) -> bool {
        self.vault.is_some()
    }

    /// Whether a recording is running.
    ///
    /// The window asks, so that closing it, or locking, does not silently
    /// abandon a recording somebody is in the middle of making.
    pub fn is_recording(&self) -> bool {
        self.session.is_some()
    }

    /// What phase the tab is in.
    fn phase(&self) -> Phase {
        match (&self.vault, &self.session) {
            (None, _) => Phase::Shut,
            (Some(_), None) => Phase::Idle,
            (Some(_), Some(_)) => Phase::Recording,
        }
    }

    /// Shut the vault and forget the key.
    ///
    /// Called when the window locks. A recording in progress is stopped first
    /// and **kept**, not discarded: the vault is still open at that moment, and
    /// throwing away a recording because the idle timer fired would be the
    /// worst thing this tab could do.
    pub fn close(&mut self) {
        if self.session.is_some() {
            self.finish_take();
        }
        self.vault = None;
        self.entries.clear();
        self.selected = None;
        self.renaming = None;
        self.confirm_remove = None;
        self.app_entry.zeroize();
        self.rest_entry.zeroize();
    }

    /// Derive the key from both entries and open the vault.
    fn unlock(&mut self) {
        let Some(dir) = default_dir() else {
            self.message = Some((
                "This system does not say where an application should keep its \
                 files, so there is nowhere to put a vault."
                    .into(),
                p::red(),
            ));
            return;
        };

        // Both buffers are consumed and wiped here whatever happens next,
        // including the failure paths below.
        let app = into_secret(&mut self.app_entry);
        let rest = into_secret(&mut self.rest_entry);

        let key = match StudioKey::derive(&app, &rest) {
            Ok(key) => key,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };

        match Vault::open(&dir, key) {
            Ok(vault) => match vault.list() {
                Ok(entries) => {
                    let count = entries.len();
                    self.entries = entries;
                    self.vault = Some(vault);
                    self.message = Some((
                        match count {
                            0 => "Vault open. Nothing in it yet.".to_string(),
                            1 => "Vault open. One recording.".to_string(),
                            n => format!("Vault open. {n} recordings."),
                        },
                        p::green(),
                    ));
                }
                // The index would not open. Almost always the wrong pair of
                // passphrases, and said that way round rather than as a
                // cryptographic verdict, because that is what it usually means.
                Err(_) => {
                    self.message = Some((
                        "That pair did not open this vault. Both passphrases have \
                         to be the ones it was made with, and either one being \
                         wrong looks exactly like this."
                            .into(),
                        p::red(),
                    ));
                }
            },
            Err(error) => self.message = Some((error.to_string(), p::red())),
        }
    }

    /// Start recording into the vault's holding area.
    fn start_take(&mut self, config: DeidConfig, input: Option<&str>, output: Option<&str>) {
        use veilvoice_audio::devices::Direction;

        let rate = config.sample_rate as u32;
        let (recorder, sink) = veilvoice_audio::record::start(rate);

        let input = match veilvoice_audio::devices::open(Direction::Input, input) {
            Ok(device) => device,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };
        let output = match veilvoice_audio::devices::open(Direction::Output, output) {
            Ok(device) => device,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };

        match veilvoice_audio::LiveSession::start_recording(&input, &output, config, Some(sink)) {
            Ok(session) => {
                self.session = Some(session);
                self.recorder = Some(recorder);
                self.message = None;
            }
            Err(error) => self.message = Some((error.to_string(), p::red())),
        }
    }

    /// Stop recording and seal what was captured into the vault.
    fn finish_take(&mut self) {
        // The audio stops first. Sealing takes a noticeable moment, and samples
        // arriving during it would be dropped rather than kept.
        self.session = None;

        let Some(mut recorder) = self.recorder.take() else {
            return;
        };
        recorder.drain();

        if recorder.samples() == 0 {
            self.message = Some((
                "Nothing was captured, so nothing was stored. Check the input \
                 device is the one you are speaking into."
                    .into(),
                p::yellow(),
            ));
            return;
        }

        let seconds = recorder.seconds();
        let dropped = recorder.dropped();

        let wav = match recorder.wav() {
            Ok(wav) => wav,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };

        let Some(vault) = &self.vault else {
            // The vault shut while a recording was running. The recording is
            // still in locked memory here and there is nowhere safe to put it,
            // so say so plainly rather than writing it somewhere it does not
            // belong.
            self.message = Some((
                "The vault closed while this was recording, so there is nowhere \
                 to put it. Open the vault and record again."
                    .into(),
                p::red(),
            ));
            return;
        };

        let name = if self.take_name.trim().is_empty() {
            "untitled".to_string()
        } else {
            self.take_name.trim().to_string()
        };
        let made = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        match vault.store(&name, made, wav.expose()) {
            Ok(entry) => {
                self.selected = Some(entry.id.clone());
                self.entries.push(entry);
                self.take_name.clear();
                let mut said = format!("Stored {name:?}, {}.", length(seconds as f64));
                if dropped > 0 {
                    // Never hidden. A recording that is quietly short is the
                    // failure this whole path is built to avoid.
                    said.push_str(&format!(
                        " {dropped} samples were dropped, so it is slightly short."
                    ));
                }
                self.message = Some((said, p::green()));
            }
            Err(error) => self.message = Some((error.to_string(), p::red())),
        }
    }

    /// Re-read the listing from the vault.
    fn refresh(&mut self) {
        if let Some(vault) = &self.vault {
            match vault.list() {
                Ok(entries) => self.entries = entries,
                Err(error) => self.message = Some((error.to_string(), p::red())),
            }
        }
    }
}

impl Studio {
    /// The Recording Studio tab.
    ///
    /// `config` is the engine setting the rest of the window is showing, so a
    /// take is recorded at the strength on screen rather than at a default this
    /// tab chose for itself.
    pub fn tab(
        &mut self,
        ui: &mut Ui,
        config: DeidConfig,
        input: Option<&str>,
        output: Option<&str>,
    ) {
        ui.add_space(4.0);

        match self.phase() {
            Phase::Shut => self.shut_panel(ui),
            Phase::Idle => {
                self.take_form(ui);
                ui.add_space(12.0);
                if ui
                    .button(RichText::new("  start recording  ").strong())
                    .clicked()
                {
                    self.start_take(config, input, output);
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "What is recorded is the veiled voice, not the microphone. The engine \
                         runs first and the recorder only ever sees what comes out of it.",
                    )
                    .color(p::muted())
                    .small(),
                );
            }
            Phase::Recording => {
                let (seconds, dropped) = self
                    .recorder
                    .as_mut()
                    .map(|r| {
                        r.drain();
                        (r.seconds(), r.dropped())
                    })
                    .unwrap_or((0.0, 0));

                ui.horizontal(|ui| {
                    ui.label(RichText::new("● recording").color(p::red()).strong());
                    ui.label(RichText::new(length(seconds as f64)).color(p::fg()));
                });
                if dropped > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{dropped} samples dropped, so this will be slightly short"
                        ))
                        .color(p::yellow())
                        .small(),
                    );
                }
                ui.add_space(12.0);
                if ui
                    .button(RichText::new("  stop and store  ").strong())
                    .clicked()
                {
                    self.finish_take();
                }
                // Repainting while the counter is running, and only then.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
        }

        self.say(ui);
    }

    /// The Recording Browser tab.
    pub fn browser(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);

        if self.vault.is_none() {
            self.shut_panel(ui);
            self.say(ui);
            return;
        }

        if self.entries.is_empty() {
            ui.label(
                RichText::new(
                    "Nothing in the vault yet. The Studio tab is where recordings are made.",
                )
                .color(p::muted()),
            );
            self.say(ui);
            return;
        }

        ui.label(
            RichText::new(format!("{} in the vault", counted(self.entries.len())))
                .color(p::blue())
                .small(),
        );
        ui.add_space(6.0);

        // Collected first: the row buttons borrow `self` mutably, and the list
        // they are drawn from is `self.entries`.
        let rows: Vec<Entry> = self.entries.clone();
        let mut act: Option<Act> = None;

        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                for entry in &rows {
                    let chosen = self.selected.as_deref() == Some(entry.id.as_str());
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(chosen, RichText::new(&entry.name).strong())
                            .clicked()
                        {
                            act = Some(Act::Select(entry.id.clone()));
                        }
                        ui.label(RichText::new(size(entry.bytes)).color(p::muted()).small());
                        ui.label(RichText::new(made_on(entry.made)).color(p::muted()).small());
                    });

                    if chosen {
                        ui.indent(entry.id.as_str(), |ui| {
                            if let Some((id, typing)) = &mut self.renaming {
                                if id == &entry.id {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(typing).desired_width(220.0),
                                        );
                                        if ui.button("save").clicked() {
                                            act =
                                                Some(Act::Rename(entry.id.clone(), typing.clone()));
                                        }
                                        if ui.button("cancel").clicked() {
                                            act = Some(Act::CancelRename);
                                        }
                                    });
                                    return;
                                }
                            }
                            if self.confirm_remove.as_deref() == Some(entry.id.as_str()) {
                                ui.label(
                                    RichText::new(
                                        "Remove this recording? It cannot be brought back.",
                                    )
                                    .color(p::yellow()),
                                );
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(RichText::new("remove it").color(p::red()))
                                        .clicked()
                                    {
                                        act = Some(Act::Remove(entry.id.clone()));
                                    }
                                    if ui.button("keep it").clicked() {
                                        act = Some(Act::CancelRemove);
                                    }
                                });
                                return;
                            }
                            ui.horizontal(|ui| {
                                if ui.button("rename").clicked() {
                                    act = Some(Act::StartRename(
                                        entry.id.clone(),
                                        entry.name.clone(),
                                    ));
                                }
                                if ui.button("remove").clicked() {
                                    act = Some(Act::AskRemove(entry.id.clone()));
                                }
                            });
                        });
                    }
                }
            });

        if let Some(act) = act {
            self.apply(act);
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "The names and dates above are sealed with the recordings. What a vault on a \
                 disk shows is how many files there are and roughly how large each one is, and \
                 nothing else.",
            )
            .color(p::muted())
            .small(),
        );

        self.say(ui);
    }

    /// The panel shown while the vault is shut, in both tabs.
    fn shut_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("The vault is shut").color(p::blue()).small());
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "It opens with both passphrases at once: the one on this application, and the \
                 one on your recordings. Neither alone opens it, which is what makes a stolen \
                 unlocked laptop useless here.",
            )
            .color(p::muted())
            .small(),
        );
        ui.add_space(10.0);

        let mut go = false;
        ui.horizontal(|ui| {
            ui.label("app lock  ");
            go |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.app_entry)
                        .password(true)
                        .desired_width(200.0),
                )
                .lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        ui.horizontal(|ui| {
            ui.label("at rest   ");
            go |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.rest_entry)
                        .password(true)
                        .desired_width(200.0),
                )
                .lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });

        ui.add_space(8.0);
        let ready = !self.app_entry.is_empty() && !self.rest_entry.is_empty();
        if ui
            .add_enabled(
                ready,
                egui::Button::new(RichText::new("  open the vault  ").strong()),
            )
            .clicked()
            || (go && ready)
        {
            self.unlock();
        }
        if !ready {
            ui.label(
                RichText::new("Both are needed. One on its own is refused rather than tried.")
                    .color(p::muted())
                    .small(),
            );
        }
    }

    /// The name field for the next take.
    fn take_form(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("call it  ");
            ui.add(
                egui::TextEdit::singleline(&mut self.take_name)
                    .hint_text("untitled")
                    .desired_width(240.0),
            );
        });
        ui.label(
            RichText::new(
                "A name is a label and nothing veils a name. It is sealed with the recording, \
                 so it is not readable from the disk, and it is still the thing that says who \
                 this is.",
            )
            .color(p::muted())
            .small(),
        );
    }

    /// Show the last message, if there is one.
    fn say(&self, ui: &mut Ui) {
        if let Some((text, colour)) = &self.message {
            ui.add_space(10.0);
            ui.label(RichText::new(text).color(*colour));
        }
    }

    /// Carry out a row action.
    ///
    /// Separate from drawing because every one of these needs `&mut self` while
    /// the loop that produced it is borrowing `self.entries`.
    fn apply(&mut self, act: Act) {
        match act {
            Act::Select(id) => {
                self.selected = Some(id);
                self.renaming = None;
                self.confirm_remove = None;
            }
            Act::StartRename(id, name) => {
                self.confirm_remove = None;
                self.renaming = Some((id, name));
            }
            Act::CancelRename => self.renaming = None,
            Act::Rename(id, name) => {
                if let Some(vault) = &self.vault {
                    match vault.rename(&id, &name) {
                        Ok(()) => {
                            self.renaming = None;
                            self.message = Some((format!("Renamed to {name:?}."), p::green()));
                            self.refresh();
                        }
                        Err(error) => self.message = Some((error.to_string(), p::red())),
                    }
                }
            }
            Act::AskRemove(id) => {
                self.renaming = None;
                self.confirm_remove = Some(id);
            }
            Act::CancelRemove => self.confirm_remove = None,
            Act::Remove(id) => {
                if let Some(vault) = &self.vault {
                    match vault.remove(&id) {
                        Ok(()) => {
                            self.confirm_remove = None;
                            if self.selected.as_deref() == Some(id.as_str()) {
                                self.selected = None;
                            }
                            self.message = Some(("Removed.".into(), p::green()));
                            self.refresh();
                        }
                        Err(error) => self.message = Some((error.to_string(), p::red())),
                    }
                }
            }
        }
    }
}

/// Something a browser row asked for.
enum Act {
    Select(String),
    StartRename(String, String),
    CancelRename,
    Rename(String, String),
    AskRemove(String),
    CancelRemove,
    Remove(String),
}

/// "One recording" or "four recordings", so the interface does not say
/// "1 recordings".
pub fn counted(n: usize) -> String {
    match n {
        1 => "one recording".to_string(),
        n => format!("{n} recordings"),
    }
}

/// A Unix time as a date somebody reads.
///
/// Deliberately the date and not the time of day. A vault listing sitting open
/// on a screen in an office says enough by naming the recordings; the minute
/// each was made is detail nobody browsing needs and somebody looking over a
/// shoulder might.
pub fn made_on(unix: i64) -> String {
    // Civil date from a Unix day count, by the usual algorithm. No dependency
    // for this: the crate graph is read by people, and a date formatter is not
    // worth a line in it.
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// A length in seconds, as `m:ss`, for somewhere a person reads.
pub fn length(seconds: f64) -> String {
    let whole = seconds.max(0.0) as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// A size in bytes, rounded to something a person can compare.
pub fn size(bytes: usize) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else {
        format!("{:.0} KiB", (bytes as f64 / 1024.0).max(1.0))
    }
}

#[cfg(test)]
#[path = "studio/tests.rs"]
mod tests;
