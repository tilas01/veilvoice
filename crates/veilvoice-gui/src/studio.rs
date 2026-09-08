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
//! `veilvoice_audio::LiveSession::start_recording`,
//! which is the same path the command line uses, so the samples that reach the
//! recorder are the **veiled** ones. There is no code path here that captures
//! the microphone before the engine has been through it, because a path that
//! existed would eventually be taken, and the file it produced would be a
//! recording of somebody's real voice sitting in a vault they believed was
//! safe.
//!
//! The recording is assembled inside a `veilvoice_crypto::Secret` and
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
    /// Smoothed input and output peaks, for the bars drawn while recording.
    levels: crate::monitor::Levels,
    /// The take being played, and which one it is.
    ///
    /// Held as a pair so the row that started it can show its own controls. One
    /// at a time: two takes playing over each other is not a feature, and the
    /// second would decrypt a second recording into memory while the first was
    /// still there.
    playing: Option<(String, veilvoice_audio::playback::Playing)>,
    /// The folder picker, while it is open.
    picker: crate::dialog::Pending,
    /// What the picker is open for: which take, and what to make of it.
    choosing: Option<(String, Render)>,

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
        // A picker still open belongs to a vault that is now shut. Its answer
        // must not arrive later and export from a vault nobody opened.
        self.choosing = None;
        // And a take still playing is a decrypted recording in memory. The
        // window is locking; it goes with the vault.
        self.playing = None;
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
        // The bars go back to nothing rather than freezing at the last peak,
        // which would read as a level still arriving.
        self.levels.clear();

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

    /// Play a take, straight out of the vault and out of locked memory.
    ///
    /// # Nothing is written
    ///
    /// The obvious way to hear a WAV is to put it somewhere and hand the path
    /// to something that plays files. That would leave an unencrypted recording
    /// on the disk, which is what the vault exists to prevent, and it would
    /// leave it there until somebody remembered to shred it.
    ///
    /// So the samples go from the sealed record, through
    /// [`Secret`](veilvoice_crypto::Secret), to the audio device. The take is
    /// decrypted whole rather than in pieces, because the container is
    /// authenticated as one piece and an AEAD that let you open the first
    /// second of it would not be authenticating anything. What that buys is the
    /// thing that matters, which is no plaintext file at any point; what it
    /// does not buy is a footprint smaller than the recording, and that is said
    /// here rather than implied.
    fn play(&mut self, id: &str) {
        // Whatever was playing stops first, and its samples go with it. Two
        // takes decrypted at once is twice as much of somebody's voice in
        // memory as the reason for it.
        self.playing = None;

        let Some(vault) = &self.vault else {
            return;
        };
        let wav = match vault.load(id) {
            Ok(wav) => wav,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };
        let Some((rate, _seconds)) = wav_shape(wav.expose()) else {
            self.message = Some((
                "That recording does not have a WAV header this can read, so \
                 there is no way to know what rate to play it at."
                    .into(),
                p::red(),
            ));
            return;
        };

        match veilvoice_audio::playback::start(pcm16(wav.expose()), rate, None) {
            Ok(playing) => {
                self.playing = Some((id.to_string(), playing));
                self.message = None;
            }
            Err(error) => self.message = Some((error.to_string(), p::red())),
        }
    }

    /// Turn a take into a page, a video, or both, in `into`.
    ///
    /// # Leaving the vault is the point, and is said out loud
    ///
    /// Everything written here is **outside** the vault and is not sealed. That
    /// is not a defect: a video nobody can open is not a video. It is the one
    /// thing somebody doing this needs to have understood, so the tab says it
    /// before the button is pressed rather than in a note afterwards.
    ///
    /// The audio is still veiled, because it was veiled before it was ever
    /// stored. What leaves is a recording of a voice that is not anybody's.
    fn export(&mut self, id: &str, what: Render, into: &std::path::Path) {
        let Some(vault) = &self.vault else {
            return;
        };
        let Some(entry) = self.entries.iter().find(|e| e.id == id).cloned() else {
            self.message = Some((
                "That recording is not in the listing any more.".into(),
                p::red(),
            ));
            return;
        };

        let wav = match vault.load(id) {
            Ok(wav) => wav,
            Err(error) => {
                self.message = Some((error.to_string(), p::red()));
                return;
            }
        };
        let Some((_rate, seconds)) = wav_shape(wav.expose()) else {
            self.message = Some((
                "That recording does not have a WAV header this can read, so its \
                 length is unknown and nothing was written."
                    .into(),
                p::red(),
            ));
            return;
        };

        let stem = safe_stem(&entry.name);
        let audio_path = into.join(format!("{stem}.wav"));
        let plan = match plan_for(&entry.name, seconds) {
            Ok(plan) => plan,
            Err(why) => {
                self.message = Some((why, p::red()));
                return;
            }
        };

        // The audio first, because both outputs need it and neither is worth
        // writing without it.
        if let Err(error) =
            veilvoice_crypto::privatefile::write_owner_only(&audio_path, wav.expose())
        {
            self.message = Some((error.to_string(), p::red()));
            return;
        }

        let mut wrote = vec![audio_path.clone()];

        if what.wants_page() {
            match self.write_page(&plan, wav.expose(), &stem, into, &audio_path) {
                Ok(mut paths) => wrote.append(&mut paths),
                Err(why) => {
                    self.message = Some((why, p::red()));
                    return;
                }
            }
        }

        if what.wants_video() {
            let video = into.join(format!("{stem}.mp4"));
            match veilvoice_video::ffmpeg::found() {
                Some(_) => match run_ffmpeg(&audio_path, &video) {
                    Ok(()) => wrote.push(video),
                    Err(why) => {
                        self.message = Some((why, p::red()));
                        return;
                    }
                },
                // The same answer the command line gives: the exact command,
                // rather than an offer to fetch a program this does not ship.
                None => {
                    let argv = veilvoice_video::ffmpeg::black_command(
                        &audio_path,
                        &video,
                        veilvoice_video::ffmpeg::Encoding::default(),
                    );
                    self.message = Some((
                        format!(
                            "The audio and the page are written. `ffmpeg` is not on this \
                             machine, and VeilVoice does not ship or install it, so the \
                             video is not. This is the command:\n\n{}",
                            veilvoice_video::ffmpeg::command_line(&argv)
                        ),
                        p::yellow(),
                    ));
                    return;
                }
            }
        }

        let names: Vec<String> = wrote
            .iter()
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        self.message = Some((
            format!(
                "Wrote {} into {}. None of it is sealed: what leaves the vault is \
                 an ordinary file, and the voice in it is still a voice nobody owns.",
                names.join(", "),
                into.display()
            ),
            p::green(),
        ));
    }

    /// The player page, its subtitles, and the drawing they sit in.
    fn write_page(
        &self,
        plan: &veilvoice_conversation::Conversation,
        wav: &[u8],
        stem: &str,
        into: &std::path::Path,
        audio: &std::path::Path,
    ) -> Result<Vec<std::path::PathBuf>, String> {
        use veilvoice_conversation::subtitles::{self, Format};
        use veilvoice_video::{page, waveform};

        let samples = pcm16(wav);
        let envelope = waveform::envelope(&samples, 900);
        let vtt = subtitles::write(plan, Format::WebVtt);

        let audio_name = audio
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let drawn = page::player(
            plan,
            &envelope,
            &page::Look::default(),
            &audio_name,
            // Carried in the page rather than fetched: a browser treats every
            // `file:` URL as its own origin, so a track read from the file
            // beside the page is refused and the captions silently do not
            // appear.
            &page::inline_vtt(&vtt),
        )
        .map_err(|error| error.to_string())?;

        let html = into.join(format!("{stem}.html"));
        let vtt_path = into.join(format!("{stem}.vtt"));
        veilvoice_crypto::privatefile::write_owner_only(&html, drawn.markup.as_bytes())
            .map_err(|error| error.to_string())?;
        veilvoice_crypto::privatefile::write_owner_only(&vtt_path, vtt.as_bytes())
            .map_err(|error| error.to_string())?;
        Ok(vec![html, vtt_path])
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

                // What is going in and what is coming out, while it happens.
                //
                // Two bars rather than one, and this is the reason: a single
                // output meter answers "is something being recorded" and not
                // "is it being veiled", which is the question somebody at this
                // tab is actually asking. Seeing the input move and the output
                // move differently is the only thing on screen that shows the
                // engine is between them.
                if let Some(session) = &self.session {
                    let stats = session.stats();
                    self.levels.update(stats.input_peak, stats.output_peak);
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Levels").color(p::blue()).small());
                crate::monitor::meter(ui, "in ", self.levels.input, self.levels.hold_input);
                crate::monitor::meter(ui, "out", self.levels.output, self.levels.hold_output);
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

        // A take that has reached its end releases its samples here rather
        // than waiting for somebody to press stop. The buffer is a decrypted
        // recording; it should not outlive the playing of it by however long
        // the window is left open.
        if self.playing.as_ref().is_some_and(|(_, p)| p.finished()) {
            self.playing = None;
        }
        if self.playing.is_some() {
            // Only while something is playing: the position moves, so the
            // window has to redraw, and the rest of the time it must not.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }

        // The folder picker's answer, if it has arrived. Polled rather than
        // waited for, so the window keeps running while it is open.
        if let Some(answer) = self.picker.poll() {
            if let Some((id, what)) = self.choosing.take() {
                match answer {
                    Some(into) => self.export(&id, what, &into),
                    // Cancelled. Not an error, and not worth a message.
                    None => self.message = None,
                }
            }
        }

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
                            // Playing, and where it has got to.
                            //
                            // The progress is read rather than counted here:
                            // the callback knows how many samples it has
                            // actually handed the device, which is the only
                            // number that is true when the device is behind.
                            let this_is_playing =
                                self.playing.as_ref().is_some_and(|(id, _)| id == &entry.id);
                            ui.horizontal(|ui| {
                                if this_is_playing {
                                    if ui.button("  stop  ").clicked() {
                                        act = Some(Act::Stop);
                                    }
                                    if let Some((_, playing)) = &self.playing {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} / {}",
                                                length(playing.position() as f64),
                                                length(playing.duration() as f64)
                                            ))
                                            .color(p::fg()),
                                        );
                                    }
                                } else if ui
                                    .button("  play  ")
                                    .on_hover_text(
                                        "Plays it out of locked memory. Nothing is \
                                         written to the disk, so there is no copy to \
                                         remember to shred afterwards.",
                                    )
                                    .clicked()
                                {
                                    act = Some(Act::Play(entry.id.clone()));
                                }
                            });
                            ui.add_space(4.0);
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
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("take it out:").color(p::muted()).small());
                                if ui
                                    .button("preview page")
                                    .on_hover_text(
                                        "A page that plays it, draws the waveform and \
                                         carries its own captions. Opens in a browser \
                                         with nothing installed.",
                                    )
                                    .clicked()
                                {
                                    act = Some(Act::Export(entry.id.clone(), Render::Preview));
                                }
                                if ui
                                    .button("render video")
                                    .on_hover_text(
                                        "An MP4 with a black picture, for somewhere that \
                                         will not accept an audio file. Needs ffmpeg, \
                                         which VeilVoice does not ship.",
                                    )
                                    .clicked()
                                {
                                    act = Some(Act::Export(entry.id.clone(), Render::Video));
                                }
                                if ui.button("both").clicked() {
                                    act = Some(Act::Export(entry.id.clone(), Render::Both));
                                }
                            });
                            ui.label(
                                RichText::new(
                                    "Anything taken out is written unsealed. The voice in \
                                     it is still veiled; the file is an ordinary file.",
                                )
                                .color(p::yellow())
                                .small(),
                            );
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
            Act::Play(id) => self.play(&id),
            Act::Stop => {
                // Dropping it stops the audio and releases the samples. There
                // is no stop that keeps the buffer: a decrypted recording
                // outliving the reason it was decrypted is the leak the vault
                // exists to prevent.
                self.playing = None;
            }
            Act::Export(id, what) => {
                // Off the render loop. `rfd`'s blocking picker freezes the
                // window until it is answered, which `dialog` exists to avoid
                // and which a test in that module forbids.
                //
                // Asked for every time rather than remembered: this writes an
                // unsealed copy of something that is in a vault, and a
                // remembered folder is how the second one lands somewhere the
                // first was deliberately kept out of.
                self.choosing = Some((id, what));
                self.picker.start(crate::dialog::Ask::Folder);
            }
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

/// What a take is to be turned into.
///
/// Three, because the two useful things are genuinely separate and doing both
/// is the common case: the page is something to look at now, the video is
/// something to send somewhere that will not take an audio file, and somebody
/// who wants the second usually wants to check the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Render {
    /// The self-contained player page, and the audio and subtitles beside it.
    Preview,
    /// An MP4, through `ffmpeg`.
    Video,
    /// Both.
    Both,
}

impl Render {
    fn wants_page(self) -> bool {
        matches!(self, Render::Preview | Render::Both)
    }

    fn wants_video(self) -> bool {
        matches!(self, Render::Video | Render::Both)
    }
}

/// The sample rate and frame count a canonical WAV header states.
///
/// Read from the recording's own header rather than assumed, because the
/// recorder writes the rate the **device agreed to**, which is not always the
/// rate that was asked for. A duration computed from the wrong rate puts every
/// subtitle in the wrong place, and the video would be the wrong length.
fn wav_shape(wav: &[u8]) -> Option<(u32, f64)> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let rate = u32::from_le_bytes(wav[24..28].try_into().ok()?);
    let bytes_per_sample = u16::from_le_bytes(wav[34..36].try_into().ok()?) as u32 / 8;
    let data = u32::from_le_bytes(wav[40..44].try_into().ok()?) as f64;
    let channels = u16::from_le_bytes(wav[22..24].try_into().ok()?) as u32;
    let per_second = rate
        .checked_mul(bytes_per_sample.max(1))?
        .checked_mul(channels.max(1))?;
    if per_second == 0 {
        return None;
    }
    Some((rate, data / per_second as f64))
}

/// A one-speaker plan spanning a take.
///
/// A studio take is one person at a microphone, so the plan the renderer wants
/// is a single turn from nothing to the end. Built rather than stored: a plan
/// kept beside each take would be a second description of a fact the audio
/// already carries, and the two would disagree the first time a take was
/// trimmed.
fn plan_for(name: &str, seconds: f64) -> Result<veilvoice_conversation::Conversation, String> {
    use veilvoice_conversation::{Speaker, Turn};

    let mut plan = veilvoice_conversation::Conversation::new();
    plan.title = Some(name.to_string());
    plan.add_speaker(Speaker::named(name))
        .map_err(|e| e.to_string())?;
    plan.add_turn(Turn {
        start: 0.0,
        end: seconds,
        speaker: 0,
        text: None,
    })
    .map_err(|e| e.to_string())?;
    Ok(plan)
}

/// Something a browser row asked for.
#[derive(Debug, PartialEq)]
enum Act {
    Select(String),
    StartRename(String, String),
    CancelRename,
    Rename(String, String),
    AskRemove(String),
    CancelRemove,
    Remove(String),
    Export(String, Render),
    Play(String),
    Stop,
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

/// Sixteen-bit PCM from a WAV, as the waveform drawer wants it.
///
/// The header is skipped rather than parsed a second time: [`wav_shape`] has
/// already established this is a canonical 44-byte header, and a reader that
/// disagreed with it about where the data starts would draw a waveform offset
/// from the audio it is meant to describe.
fn pcm16(wav: &[u8]) -> Vec<f32> {
    wav.get(44..)
        .unwrap_or(&[])
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32_768.0)
        .collect()
}

/// A file name built from what somebody called a recording.
///
/// A name is whatever was typed, and it reaches a **path** here. Everything
/// that is not a letter, a digit, a dash or an underscore becomes a dash, so a
/// take called `../../etc/passwd` or `a/b` cannot write outside the folder that
/// was chosen. Empty after that, and it is `take`: a file called nothing is not
/// a file.
fn safe_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "take".to_string()
    } else {
        // A long name makes a path some systems refuse, and the name is a
        // label rather than an identifier, so shortening loses nothing that
        // is not still in the vault.
        trimmed.chars().take(60).collect()
    }
}

/// Run `ffmpeg` to put the audio in a video with a black picture.
fn run_ffmpeg(audio: &std::path::Path, video: &std::path::Path) -> Result<(), String> {
    let argv = veilvoice_video::ffmpeg::black_command(
        audio,
        video,
        veilvoice_video::ffmpeg::Encoding::default(),
    );
    let Some(program) = veilvoice_video::ffmpeg::found() else {
        return Err("`ffmpeg` went away between the check and the run.".into());
    };
    let output = std::process::Command::new(program)
        .args(argv.iter().skip(1))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    // The last line of ffmpeg's complaint, which is the one that says what
    // was wrong. The whole of it is pages of build configuration.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let last = stderr.lines().rev().find(|line| !line.trim().is_empty());
    Err(format!(
        "`ffmpeg` refused: {}",
        last.unwrap_or("it gave no reason").trim()
    ))
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
