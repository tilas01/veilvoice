// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice` — the command-line interface.
//!
//! Everything VeilVoice does, available without a desktop: it runs over SSH, in
//! a container, and on machines that have no GUI toolkit at all. The same
//! engine backs both this and the graphical app.
//!
//! # What is here
//!
//! Twenty subcommands, and they divide into five groups:
//!
//! * **Audio** -- `anonymise` a file, `live` scramble a microphone, list
//!   `devices`, `conversation` for a recording with several people in it.
//! * **Privacy of the files themselves** -- `clean` metadata, `encrypt`,
//!   `decrypt`, `keygen`, `shred`.
//! * **Watching the machine** -- `watch` the microphone and camera, `guard`
//!   VeilVoice's own files against tampering, `sentry` for canaries and how
//!   fast a folder is changing, `capture` for which screen recorders are
//!   running.
//! * **The app lock** -- `lock set|status|change|remove`, and `policy` for
//!   settings somebody has fixed so the interface cannot turn them off.
//! * **Getting it onto the machine** -- `install`, `uninstall`, `companions`,
//!   and `gui` to open the desktop application.
//!
//! That last group is a front end over [`veilvoice_setup`], which the desktop
//! application's setup tab also calls. The careful part -- editing `PATH` --
//! has one implementation and one set of tests, rather than one per front
//! end.
//!
//! # Two behaviours that surprise people, on purpose
//!
//! **`anonymise` writes `<out>.veil`, not a bare WAV.** Recordings are
//! encrypted at rest by default. `--encrypt=false` opts out and requires
//! `--yes`, because an unsealed recording is the thing somebody later wishes
//! they had not produced. The wiki explains where the WAV went.
//!
//! **The front-ends refuse rather than downgrade.** Asked to encrypt with
//! nothing to encrypt with, this exits with an error instead of writing plain
//! audio and mentioning it. Quiet degradation to a weaker posture is the defect
//! class this project has found in itself most often.
//!
//! # Passphrase prompts cannot be piped
//!
//! `rpassword` needs a real console; piping a passphrase in blocks on
//! `CONIN$` rather than reading it. That is a property of terminal input, not a
//! bug here, and it means anything that prompts cannot be smoke-tested from a
//! non-interactive shell. The layer *beneath* each prompt is therefore tested
//! instead -- see [`crate::atrest`] and [`crate::lock`], where the logic lives
//! precisely so it can be reached without a terminal.
//!
//! # A clap ordering rule worth knowing
//!
//! An argument declared beside `#[command(subcommand)]` must precede the
//! subcommand on the command line unless it is marked `global = true`. So
//! `veilvoice lock --path X status` parses and `veilvoice lock status --path X`
//! does not, except that `--path` is now global specifically so both do.
//!
//! # In plain words
//!
//! This is VeilVoice without a window.
//!
//! Everything the program does, typed instead of clicked: disguise a recording,
//! scramble a microphone while you talk, seal a file, strip a photograph's hidden
//! labels, handle a recording with several people in it.
//!
//! It is the same code underneath, so it works the same way -- over a remote
//! connection, on a machine with no desktop, or from a script that runs it a
//! thousand times.
#![forbid(unsafe_code)]

mod appctl;
mod atrest;
mod capture;
mod conversation;
mod failsafe;
mod guard;
mod input;
mod lock;
mod priv_mode;
// Only the live path draws a meter, and the crate builds without that path on
// the BSDs, where `cpal` has no backend.
#[cfg(feature = "live")]
mod meter;
mod policy;
mod sentry;
mod theme;

use atrest::{prompt_secret, read_new_password};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;
use theme::{colour, err, field, heading, ok, paint, warn};
#[cfg(feature = "live")]
use veilvoice_audio::devices;
use veilvoice_audio::io as audio_io;
use veilvoice_core::{AccentConfig, DeidConfig};
use veilvoice_crypto::{container, hybrid, kdf};
use veilvoice_meta::Policy;
use veilvoice_policy::Requirement;
use veilvoice_sentry::rate::{Limits, Threshold};
use veilvoice_setup::{companions, install};

#[derive(Parser)]
#[command(
    name = "veilvoice",
    version,
    about = "Irreversible voice de-identification — fully offline.",
    long_about = "VeilVoice destroys the biometric voiceprint of a speaker — pitch, \
formants, timbre and the melody of an accent — while keeping the words clean and \
transcribable. This command line talks to no servers, ever: the one thing in VeilVoice that reaches the network is the desktop app's check-for-updates button, and it is not here."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// De-identify an audio file and write a WAV.
    Anonymise {
        /// Audio file to read (wav, mp3, flac, ogg, m4a, ...).
        input: PathBuf,
        /// Where to write the result. Defaults to `<input>.veiled.wav`.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// How far pitch and formants are pushed from the original, 0.0–1.0.
        #[arg(long, default_value_t = 1.0)]
        intensity: f32,
        /// Keep the speaker's accent and intonation intact.
        #[arg(long)]
        keep_accent: bool,
        /// Seconds between rolls of the modulation seed. 0 keeps one stream
        /// for the whole session.
        #[arg(long, default_value_t = 2.0)]
        reseed_secs: f32,

        /// Draw each gap from a range instead, in milliseconds: `250,1800`.
        ///
        /// A fixed interval is a fixed thing to observe. With a range, the gap
        /// before every roll is drawn fresh from the modulation stream, so the
        /// ratchet has no period at all.
        ///
        /// Without this, the range is **drawn from the operating system's
        /// random source at launch**, so it is a property of this run rather
        /// than a number compiled into every copy of VeilVoice. Pass
        /// `--reseed-range fixed` to use `--reseed-secs` instead.
        ///
        /// A value that is not a usable range is **refused with the reason**,
        /// never adjusted to fit.
        #[arg(long)]
        reseed_range: Option<String>,
        /// Also strip metadata from the written file.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        clean_metadata: bool,
        /// Encrypt the result at rest. On by default: the words survive
        /// de-identification on purpose, so an unencrypted result is still a
        /// recording of everything that was said.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        encrypt: bool,
        /// Seal to a recipient's public key file instead of a passphrase.
        #[arg(long, value_name = "PUBKEY")]
        encrypt_to: Option<PathBuf>,
        /// Skip the confirmation when writing an unencrypted recording.
        #[arg(long)]
        yes: bool,
    },
    /// Scramble a microphone live, into a device or a virtual cable.
    #[cfg(feature = "live")]
    Live {
        /// Input device name. Defaults to the system default.
        #[arg(short, long)]
        input: Option<String>,
        /// Output device name. Defaults to a virtual cable if one is found.
        #[arg(short, long)]
        output: Option<String>,
        /// How far pitch and formants are pushed from the original, 0.0–1.0.
        #[arg(long, default_value_t = 1.0)]
        intensity: f32,
        /// Keep the speaker's accent and intonation intact.
        #[arg(long)]
        keep_accent: bool,
        /// Seconds between rolls of the modulation seed. 0 keeps one stream
        /// for the whole session.
        #[arg(long, default_value_t = 2.0)]
        reseed_secs: f32,

        /// Draw each gap from a range instead, in milliseconds: `250,1800`.
        ///
        /// A fixed interval is a fixed thing to observe. With a range, the gap
        /// before every roll is drawn fresh from the modulation stream, so the
        /// ratchet has no period at all.
        ///
        /// Without this, the range is **drawn from the operating system's
        /// random source at launch**, so it is a property of this run rather
        /// than a number compiled into every copy of VeilVoice. Pass
        /// `--reseed-range fixed` to use `--reseed-secs` instead.
        ///
        /// A value that is not a usable range is **refused with the reason**,
        /// never adjusted to fit.
        #[arg(long)]
        reseed_range: Option<String>,
    },
    /// List the audio devices this machine offers.
    #[cfg(feature = "live")]
    Devices,
    /// Strip identifying metadata from an audio or image file, in place.
    Clean {
        /// File to clean.
        file: PathBuf,
        /// Whether to leave plausible placeholder tags behind.
        #[arg(long, value_enum, default_value_t = CleanPolicy::Strip)]
        policy: CleanPolicy,
    },
    /// Encrypt a file into a `.veil` container.
    Encrypt {
        /// File to encrypt.
        input: PathBuf,
        /// Where to write the container. Defaults to `<input>.veil`.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Encrypt to a recipient's public key file instead of a password.
        #[arg(long)]
        to: Option<PathBuf>,
    },
    /// Decrypt a `.veil` container.
    Decrypt {
        /// Container to decrypt.
        input: PathBuf,
        /// Where to write the plaintext.
        #[arg(short, long)]
        output: PathBuf,
        /// Private key file, when the container was sealed to a public key.
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// Generate a hybrid post-quantum key pair.
    Keygen {
        /// Where to write the public key.
        #[arg(long, default_value = "veilvoice.pub")]
        public: PathBuf,
        /// Where to write the private key.
        #[arg(long, default_value = "veilvoice.key")]
        secret: PathBuf,
    },
    /// Record and check the integrity of VeilVoice's own files.
    Guard {
        #[command(subcommand)]
        action: guard::Action,
        /// Where the record is kept. Defaults to this platform's config
        /// directory, beside the app lock.
        #[arg(long, global = true)]
        path: Option<PathBuf>,
    },
    /// Manage the application lock that guards the desktop app.
    Lock {
        #[command(subcommand)]
        action: lock::Action,
        /// Lock file to operate on. Defaults to this platform's config
        /// directory. Global, so it reads naturally either side of the action.
        #[arg(long, global = true)]
        path: Option<PathBuf>,
    },
    /// Show which applications are using the microphone and camera.
    Watch {
        /// Print a snapshot and exit instead of watching continuously.
        #[arg(long)]
        once: bool,
        /// Seconds between checks.
        #[arg(long, default_value_t = 2.0)]
        interval: f32,
    },
    /// Securely erase a file, then delete it. Irreversible.
    Shred {
        /// File to destroy.
        file: PathBuf,
        /// Overwrite passes (1-32).
        #[arg(long, default_value_t = 3)]
        passes: u8,
        /// Skip the typed confirmation. For scripts that already mean it.
        #[arg(long)]
        yes: bool,
    },
    /// Show version and build information.
    Info,

    /// Open the desktop application.
    ///
    /// Runs `veilvoice-gui` from beside this program. It is a separate
    /// executable rather than a mode of this one for a reason that cannot be
    /// engineered around without `unsafe`: a Windows PE declares exactly one
    /// subsystem. A console binary that opened a window would flash a console
    /// every time -- which is precisely the defect v0.1.10 shipped -- and a
    /// windowed binary would send this program's output nowhere when run from
    /// a terminal. Switching at run time needs `AttachConsole`/`FreeConsole`,
    /// which is FFI, and every crate here carries `#![forbid(unsafe_code)]`.
    Gui,

    /// Copy VeilVoice somewhere the system can find it, and add it to PATH.
    ///
    /// Entirely optional: VeilVoice runs from wherever it is unpacked, and
    /// nothing has to be installed. This exists so that typing `veilvoice` in
    /// a terminal works. Per-user, no administrator, and everything it does is
    /// undone by `veilvoice uninstall`.
    Install {
        /// Report what is installed, and change nothing.
        #[arg(long)]
        status: bool,
    },

    /// Undo what `install` did: the PATH entry, the uninstall entry, and the
    /// installed copy.
    Uninstall {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Settings fixed so the interface cannot turn them off.
    ///
    /// **Every setting a policy can reach makes VeilVoice stricter.** There is
    /// nothing here that turns protection off, no requirement that lowers the
    /// de-identification floor, and no room in the format to write one. That is
    /// why the policy is read at every launch without a passphrase: the worst
    /// an edited policy file can do is restrict this machine further than its
    /// owner intended, which is a nuisance rather than a privacy failure.
    ///
    /// The passphrase seals a copy, so anybody who has it can prove the policy
    /// in force is the one that was written. It is not enforcement: anything
    /// that can replace VeilVoice's own executable can ignore all of this, and
    /// anything running as you can delete the file.
    Policy {
        #[command(subcommand)]
        what: PolicyCommand,
    },

    /// A recording with several people in it: a voice each, and subtitles.
    ///
    /// Run an interview through `anonymise` and both people come out as the
    /// same voice -- private, and unusable, because nobody can tell a question
    /// from its answer. This gives each speaker their own destination voice and
    /// destroys each voiceprint just as thoroughly.
    ///
    /// **You say who is talking.** Working that out from the audio needs a
    /// trained model; there is none here and no server to ask, and a wrong
    /// guess would either merge two people or invent a third without anything
    /// in the output showing it. So the plan is a text file of turns, or one
    /// microphone per person.
    ///
    /// **What a conversation keeps** is the shape of the conversation: how many
    /// people, who spoke when, and for how long. That is kept on purpose --
    /// it is what makes the result worth listening to -- and it is information
    /// about the conversation.
    Conversation {
        #[command(subcommand)]
        what: ConversationCommand,
    },

    /// Which screen recorders are running, and which you meant to run.
    ///
    /// **VeilVoice does not hide its own window from capture.** You can record
    /// this application with OBS or anything else, deliberately, and nothing
    /// here prevents it. Excluding a window from capture needs `unsafe` FFI,
    /// which every crate in this workspace forbids, so the exclusion is not
    /// built -- see ROADMAP.md.
    ///
    /// What this does is tell you a recorder is running, once, and then stop
    /// telling you if you say you meant it. A monitor that warns every thirty
    /// seconds while you record a tutorial is a monitor you switch off, and
    /// then it is not watching for the recorder you did *not* start.
    ///
    /// Two things it cannot do. It only knows the programs in its table, so an
    /// empty report is not evidence that nothing is recording. And it cannot
    /// tell whether a program that is running is actually capturing anything --
    /// a meeting application being open is not somebody watching your screen.
    Capture {
        #[command(subcommand)]
        what: CaptureCommand,
    },

    /// The safety catch: what it watches for, and what it cannot do.
    ///
    /// Failsafe notices the moment another program picks up a **real**
    /// microphone while you are being veiled -- the accident where you plug in
    /// a headset and your computer quietly switches a call over to it, so your
    /// own voice goes out with nothing on screen looking any different.
    ///
    /// It is on by default. **It cannot stop your computer handing a
    /// microphone over**; it notices within about a second and acts, and the
    /// difference between those two things is printed every time.
    Failsafe {
        /// The device VeilVoice is veiling into, so a program on that cable is
        /// not mistaken for the accident.
        #[arg(long)]
        veiling: Option<String>,
    },

    /// What VeilVoice is running with, and what that lets it see.
    ///
    /// Most of VeilVoice needs no special permissions; the monitoring features
    /// see further as an administrator. This reports which you are getting and
    /// prints the command to run it the other way.
    ///
    /// **It never raises its own privileges, installs a service, or asks for a
    /// password.** Those are changes to your machine and they should be ones
    /// you made on purpose.
    Privilege,

    /// Learn what normally runs here, then notice what does not.
    ///
    /// Run `learn` while you work normally for a few days, `learn --finish` to
    /// close the baseline, and `check` afterwards.
    ///
    /// **It does not block anything and cannot.** It is a way of noticing, not
    /// a lock on the door: a program it calls unknown is still running. Real
    /// enforcement needs a kernel driver or a signed system policy, and neither
    /// is something this project ships.
    Appctl {
        #[command(subcommand)]
        what: AppctlCommand,
    },

    /// What running programs can see your keyboard and mouse.
    ///
    /// Names the programs that are **able** to observe input -- remote-support
    /// tools, macro recorders, password managers, screen readers -- and says
    /// what each one is for. Nearly all of it is software you installed on
    /// purpose.
    ///
    /// It cannot tell you whether anything is logging your keystrokes, and
    /// nothing can: the mechanisms a logger uses are the mechanisms
    /// accessibility software uses, and software written to hide is written to
    /// hide from this. **A result of nothing found does not mean nothing is
    /// watching.** That sentence is printed with every result, not behind a
    /// flag.
    Input {
        #[command(subcommand)]
        what: Option<InputCommand>,
    },

    /// Canaries, and how fast a folder is changing.
    ///
    /// Two early warnings that something is going through your files. Neither
    /// stops anything, and neither names the program responsible.
    ///
    /// A **canary** is a file VeilVoice writes and nothing reads. If it ever
    /// changes, something walked that folder and wrote to everything in it. It
    /// only fires if whatever is running reaches that folder, so a quiet canary
    /// is not evidence that nothing happened.
    ///
    /// A **baseline** records what a folder holds now, and `check` says how
    /// much of it changed since and how fast. That number cannot tell
    /// ransomware from a backup restore, a photo import or a compiler, so it is
    /// reported against thresholds you set rather than as a verdict.
    Sentry {
        #[command(subcommand)]
        what: SentryCommand,
    },

    /// Optional third-party software VeilVoice works with, and whether this
    /// machine already has it.
    ///
    /// Nothing here is part of VeilVoice and nothing here is needed to run it.
    /// A virtual audio cable is what lets live mode feed a veiled microphone
    /// into a call; an audio editor is how most people trim a recording first.
    ///
    /// With no arguments this only *reports*. `--install NAME` is the explicit
    /// yes, one named program at a time, and even then VeilVoice will not run
    /// somebody else's installer: for proprietary software it prints the
    /// vendor's page, and for anything needing root it prints the command
    /// rather than asking for a password.
    Companions {
        /// Install one, by name. Without this, nothing is installed.
        #[arg(long, value_name = "NAME")]
        install: Option<String>,
    },
}

/// What `veilvoice conversation` can do.
#[derive(Subcommand)]
enum ConversationCommand {
    /// Describe a plan: who is in it, which voice each gets, and any overlaps.
    ///
    /// Reads the plan and nothing else. Worth doing before a long render.
    Inspect {
        /// The plan file.
        plan: PathBuf,
    },

    /// Render a recording according to a plan.
    ///
    /// Writes the audio and both subtitle formats. Audio that no turn claims
    /// is **silenced**, never passed through -- it has not been veiled, and a
    /// gap in a plan must not put a real voice into the result. How much went
    /// is printed.
    Render {
        /// The plan file.
        plan: PathBuf,
        /// The recording.
        input: PathBuf,
        /// Where to write the audio. The subtitles take the same name.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 0..1 -- how far the transform pushes.
        #[arg(long, default_value_t = 1.0)]
        intensity: f32,
        /// Leave the speaker's accent and intonation intact.
        #[arg(long)]
        keep_accent: bool,
        /// Seconds between modulation seed rolls; 0 keeps one stream.
        #[arg(long, default_value_t = 2.0)]
        reseed_secs: f32,

        /// Draw each gap from a range instead, in milliseconds: `250,1800`.
        ///
        /// A fixed interval is a fixed thing to observe. With a range, the gap
        /// before every roll is drawn fresh from the modulation stream, so the
        /// ratchet has no period at all.
        ///
        /// Without this, the range is **drawn from the operating system's
        /// random source at launch**, so it is a property of this run rather
        /// than a number compiled into every copy of VeilVoice. Pass
        /// `--reseed-range fixed` to use `--reseed-secs` instead.
        ///
        /// A value that is not a usable range is **refused with the reason**,
        /// never adjusted to fit.
        #[arg(long)]
        reseed_range: Option<String>,

        /// Also write a self-contained HTML player beside the audio.
        ///
        /// The waveform, a circle per speaker that lights when they speak, and
        /// the subtitles. It reads the audio and the WebVTT track by name from
        /// the same directory, so move all of them or none.
        #[arg(long)]
        page: bool,
        /// Picture width in pixels.
        #[arg(long, default_value_t = 1280)]
        width: u32,
        /// Picture height in pixels.
        #[arg(long, default_value_t = 720)]
        height: u32,
        /// Margin around everything, in pixels.
        #[arg(long, default_value_t = 48)]
        padding: u32,
        /// A `#rrggbb` colour, or the path to an image file.
        #[arg(long)]
        background: Option<String>,
        /// Plain black behind everything. Overrides `--background`.
        #[arg(long)]
        black: bool,
        /// Colour scheme, from the nine the website and the app offer.
        ///
        /// Defaults to Tokyo Night. An unknown name is refused, and the error
        /// lists every one it could have been.
        #[arg(long)]
        theme: Option<String>,
        /// Give every speaker the **same** voice.
        ///
        /// More private: the output then carries no trace of *which* speaker
        /// somebody was, so two recordings of the same group cannot be lined up
        /// by voice. The price is that only the names and the picture say who
        /// is speaking -- by ear alone, nobody can. It also has no speaker
        /// limit, because one voice cannot collide with itself.
        #[arg(long)]
        one_voice: bool,
    },

    /// Draw a still of the page, without rendering any audio.
    ///
    /// The layout, the speaker circles and which voice each speaker becomes --
    /// answered in a second rather than in the length of the recording. With
    /// `--ffmpeg` it also prints the command that would turn frames into a
    /// video file, and whether `ffmpeg` is on this machine. **It never runs
    /// it**: this project ships no codec and starts no program you did not.
    Preview {
        /// The plan file.
        plan: PathBuf,
        /// A recording, so the waveform is real rather than flat.
        #[arg(long)]
        audio: Option<PathBuf>,
        /// Which second of the conversation to draw.
        #[arg(long, default_value_t = 0.0)]
        at: f64,
        /// Where to write the SVG. Defaults to the plan's name.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Print the ffmpeg command, and whether ffmpeg is installed.
        #[arg(long)]
        ffmpeg: bool,
        /// Picture width in pixels.
        #[arg(long, default_value_t = 1280)]
        width: u32,
        /// Picture height in pixels.
        #[arg(long, default_value_t = 720)]
        height: u32,
        /// Margin around everything, in pixels.
        #[arg(long, default_value_t = 48)]
        padding: u32,
        /// A `#rrggbb` colour, or the path to an image file.
        #[arg(long)]
        background: Option<String>,
        /// Plain black behind everything. Overrides `--background`.
        #[arg(long)]
        black: bool,
        /// Colour scheme, from the nine the website and the app offer.
        #[arg(long)]
        theme: Option<String>,
        /// Draw it as if every speaker had the same voice.
        #[arg(long)]
        one_voice: bool,
    },
}

/// What `veilvoice capture` can do.
#[derive(Subcommand, Debug)]
enum AppctlCommand {
    /// Record what is running as ordinary.
    Learn {
        /// Close the baseline. Nothing joins it by running after this.
        #[arg(long)]
        finish: bool,
    },
    /// Compare what is running now against the baseline.
    Check,
    /// Allow a program that is not in the baseline.
    Allow {
        /// The executable name, as it appears in `check`.
        program: String,
        /// For this many hours. Without it, permanently.
        #[arg(long)]
        hours: Option<u64>,
    },
    /// Withdraw a grant.
    Revoke {
        /// The executable name.
        program: String,
    },
    /// Every decision this baseline has made.
    Log,
}

#[derive(Subcommand, Debug)]
enum InputCommand {
    /// What is running now that could see input. The default.
    Look,

    /// Everything this build can recognise, running or not.
    ///
    /// So that an empty result can be checked rather than trusted: a reader
    /// who is told nothing was found should be able to see what was looked
    /// for.
    Known,
}

#[derive(Subcommand)]
enum CaptureCommand {
    /// What is running, what is allowed, and what this cannot see.
    Status,

    /// Every program this build knows how to notice.
    List,

    /// Where to point Discord, Signal, Telegram, Element and the rest so your
    /// voice goes through VeilVoice first.
    ///
    /// Prints the route, the menu to change in each program found, and the two
    /// things this does **not** do: it changes only what you send, and it never
    /// reaches inside any of those programs.
    Calls,

    /// Stop notifying about one program.
    ///
    /// Allowed means muted, not hidden: it still appears in `status`.
    Allow {
        /// The program's key, as `list` prints it.
        key: String,
    },

    /// Start notifying about one program again.
    Deny {
        /// The program's key, as `list` prints it.
        key: String,
    },

    /// Look now, and exit non-zero if something unallowed is running.
    ///
    /// For a script that should not start recording something sensitive while
    /// a screen recorder is open. A listing that failed prints the reason and
    /// still exits zero: a check that could not see is not a check that
    /// passed, and it is not a reason to fail somebody's script either.
    Check,
}

/// What `veilvoice policy` can do.
#[derive(Subcommand)]
enum PolicyCommand {
    /// What is in force, and what is known about the seal.
    Status,

    /// Write a policy and seal a copy of it under a passphrase.
    ///
    /// Name at least one requirement. Each one fixes a setting on; none of
    /// them can fix one off.
    Seal {
        /// Recordings must be encrypted at rest.
        #[arg(long)]
        encrypt_recordings: bool,
        /// Metadata must be stripped from what VeilVoice writes.
        #[arg(long)]
        clean_metadata: bool,
        /// Accent neutralisation must stay on.
        #[arg(long)]
        neutralise_accent: bool,
        /// The app lock must be set before VeilVoice can be used.
        #[arg(long)]
        app_lock: bool,
        /// A floor for the de-identification intensity, from 0 to 100.
        #[arg(long, value_name = "0-100")]
        minimum_intensity: Option<u8>,
        /// A line shown beside every control the policy has fixed.
        #[arg(long)]
        note: Option<String>,
        /// Write over a policy that is already in force.
        #[arg(long)]
        replace: bool,
    },

    /// Check the policy in force against its sealed copy.
    ///
    /// The only thing here that needs the passphrase, and nothing calls it at
    /// launch. Exits non-zero if the seal does not match, is missing, or the
    /// plain file has gone.
    Verify,

    /// Delete both policy files.
    ///
    /// Deliberately does not ask for the passphrase, because it could not
    /// usefully: anybody who can run this can delete the same two files with a
    /// file manager.
    Remove {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// What `veilvoice sentry` can do.
#[derive(Subcommand)]
enum SentryCommand {
    /// What is planted, what is watched, and what this is worth.
    Status,

    /// Write a canary into a directory and start watching it.
    Plant {
        /// The directory to put it in.
        dir: PathBuf,
        /// A different filename. The default says what the file is, which a
        /// reader can therefore skip; a name of your own does not.
        #[arg(long)]
        name: Option<String>,
    },

    /// Stop watching a canary, and delete it.
    ///
    /// Use this rather than deleting the file, or the deletion is itself
    /// reported as a change.
    PullUp {
        /// The canary's path, as `status` prints it.
        path: PathBuf,
    },

    /// Record what a directory holds now, to compare against later.
    ///
    /// Running it again replaces the record for that directory. Do that after
    /// a change you know about, or every later check reports it again.
    Baseline {
        /// The directory to record.
        dir: PathBuf,
        /// Stop after this many files, and say the record is partial.
        #[arg(long, default_value_t = Limits::default().max_files)]
        max_files: usize,
        /// How many directories deep to descend.
        #[arg(long, default_value_t = Limits::default().max_depth)]
        max_depth: usize,
    },

    /// Look at every canary and every baseline.
    ///
    /// Exits non-zero only if a **canary** tripped, which is a fact. Churn is a
    /// question at any level, so it never fails the command -- a check that
    /// fails every time somebody copies a folder is a check somebody removes
    /// from their scheduled task.
    Check {
        /// Files touched per minute, above which this is worth mentioning.
        #[arg(long, default_value_t = Threshold::default().files_per_minute)]
        files_per_minute: f64,
        /// Proportion of the watched files touched, from 0.0 to 1.0.
        #[arg(long, default_value_t = Threshold::default().share)]
        share: f32,
        /// Stop after this many files.
        #[arg(long, default_value_t = Limits::default().max_files)]
        max_files: usize,
        /// How many directories deep to descend.
        #[arg(long, default_value_t = Limits::default().max_depth)]
        max_depth: usize,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CleanPolicy {
    /// Remove every tag.
    Strip,
    /// Replace tags with plausible, non-identifying values.
    Realistic,
}

impl From<CleanPolicy> for Policy {
    fn from(p: CleanPolicy) -> Self {
        match p {
            CleanPolicy::Strip => Policy::Strip,
            CleanPolicy::Realistic => Policy::Realistic,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{}", err(&message));
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Anonymise {
            input,
            output,
            intensity,
            keep_accent,
            reseed_secs,
            reseed_range,
            clean_metadata,
            encrypt,
            encrypt_to,
            yes,
        } => anonymise(
            input,
            output,
            Tuning {
                intensity,
                keep_accent,
                reseed_secs,
                reseed_range: reseed_range_from(reseed_range.as_deref())?,
            },
            clean_metadata,
            AtRest {
                encrypt,
                to: encrypt_to,
                yes,
            },
        ),
        #[cfg(feature = "live")]
        Command::Live {
            input,
            output,
            intensity,
            keep_accent,
            reseed_secs,
            reseed_range,
        } => live(
            input,
            output,
            Tuning {
                intensity,
                keep_accent,
                reseed_secs,
                reseed_range: reseed_range_from(reseed_range.as_deref())?,
            },
        ),
        #[cfg(feature = "live")]
        Command::Devices => list_devices(),
        Command::Clean { file, policy } => clean(file, policy.into()),
        Command::Encrypt { input, output, to } => encrypt(input, output, to),
        Command::Decrypt { input, output, key } => decrypt(input, output, key),
        Command::Keygen { public, secret } => keygen(public, secret),
        Command::Guard { action, path } => guard::run(action, path),
        Command::Lock { action, path } => lock::run(action, path),
        Command::Shred { file, passes, yes } => shred(file, passes, yes),
        Command::Watch { once, interval } => watch(once, interval),
        Command::Info => {
            info();
            Ok(())
        }

        Command::Gui => {
            let exe = std::env::current_exe()
                .map_err(|e| format!("cannot find this program on disk: {e}"))?;
            let name = if cfg!(windows) {
                "veilvoice-gui.exe"
            } else {
                "veilvoice-gui"
            };
            let gui = exe
                .parent()
                .ok_or_else(|| "this program has no parent directory".to_string())?
                .join(name);
            if !gui.exists() {
                return Err(format!(
                    "{} is not beside this program.
                       The desktop application ships in the same archive; if you                      unpacked only the command line, download the full archive.",
                    gui.display()
                ));
            }
            // Spawned, not waited on: the terminal should come back
            // immediately, the way every other desktop launcher behaves.
            std::process::Command::new(&gui)
                .spawn()
                .map_err(|e| format!("could not start {}: {e}", gui.display()))?;
            println!("{}", ok(&format!("started {}", gui.display())));
            Ok(())
        }

        Command::Install { status } => {
            let state = install::status();
            println!("{}", heading("Install"));
            println!(
                "{}",
                field(
                    "prefix",
                    &state
                        .prefix
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "not resolvable on this system".into())
                )
            );
            println!(
                "{}",
                field("installed", if state.installed { "yes" } else { "no" })
            );
            println!(
                "{}",
                field(
                    "on PATH",
                    if state.on_path {
                        "yes, in this terminal"
                    } else {
                        "no"
                    }
                )
            );
            // "Installed" and "you are running the installed copy" are
            // different facts, and confusing them is how somebody updates a
            // portable folder and wonders why the installed one is unchanged.
            println!(
                "{}",
                field(
                    "running",
                    &match (&state.running_from, state.running_installed) {
                        (Some(path), true) => format!("the installed copy ({})", path.display()),
                        (Some(path), false) => format!("a portable copy ({})", path.display()),
                        (None, _) => "unknown".to_string(),
                    }
                )
            );
            if status {
                return Ok(());
            }

            match install::install() {
                Ok(report) => {
                    println!();
                    for line in report {
                        println!("{}", ok(&line));
                    }
                    println!();
                    println!("  Open a new terminal for the PATH change to take effect.");
                    println!();
                    // Said plainly, once, where somebody installing will read
                    // it: this program will never tell them an update exists.
                    println!(
                        "{}",
                        warn(
                            "VeilVoice never checks for updates and cannot tell you                              when one exists -- it has no network code at all.                              Watch the releases page, and verify what you download."
                        )
                    );
                    Ok(())
                }
                Err(error) => {
                    println!("{}", err(&error));
                    Err("install failed".to_string())
                }
            }
        }

        Command::Uninstall { yes } => {
            println!("{}", heading("Uninstall"));
            let state = install::status();
            if !state.installed {
                println!("{}", warn("nothing is installed for this user"));
            }
            if !yes {
                println!();
                println!("  This removes the installed copy, the PATH entry and the");
                println!("  uninstall entry. Your recordings, keys and settings are");
                println!("  not touched -- they live elsewhere and are not this");
                println!("  command's business.");
                println!();
                println!("  Re-run with --yes to proceed.");
                return Ok(());
            }
            match install::uninstall() {
                Ok(report) => {
                    for line in report {
                        println!("{}", ok(&line));
                    }
                    Ok(())
                }
                Err(error) => {
                    println!("{}", err(&error));
                    Err("uninstall failed".to_string())
                }
            }
        }

        Command::Conversation { what } => match what {
            ConversationCommand::Inspect { plan } => conversation::inspect(&plan),
            ConversationCommand::Render {
                plan,
                input,
                output,
                intensity,
                keep_accent,
                reseed_secs,
                reseed_range,
                page,
                width,
                height,
                padding,
                background,
                black,
                theme,
                one_voice,
            } => {
                // The picture flags are read whether or not `--page` was given,
                // so `--width 40 --page` and `--width 40` fail the same way.
                // Accepting numbers that describe nothing, silently, because
                // the page happened not to be asked for, is how a flag comes to
                // mean two different things.
                let look =
                    conversation::look_from(width, height, padding, background, black, theme)?;
                conversation::run(
                    &plan,
                    &input,
                    output,
                    config(Tuning {
                        intensity,
                        keep_accent,
                        reseed_secs,
                        reseed_range: reseed_range_from(reseed_range.as_deref())?,
                    }),
                    page.then_some(look),
                    one_voice,
                )
            }
            ConversationCommand::Preview {
                plan,
                audio,
                at,
                output,
                ffmpeg,
                width,
                height,
                padding,
                background,
                black,
                theme,
                one_voice,
            } => conversation::preview(
                &plan,
                audio,
                at,
                conversation::look_from(width, height, padding, background, black, theme)?,
                output,
                ffmpeg,
                one_voice,
            ),
        },

        Command::Failsafe { veiling } => failsafe::show(veiling.as_deref()),

        Command::Privilege => priv_mode::show(),

        Command::Appctl { what } => match what {
            AppctlCommand::Learn { finish } => appctl::learn(finish),
            AppctlCommand::Check => appctl::check(),
            AppctlCommand::Allow { program, hours } => appctl::allow(&program, hours),
            AppctlCommand::Revoke { program } => appctl::revoke(&program),
            AppctlCommand::Log => appctl::log(),
        },

        Command::Input { what } => match what.unwrap_or(InputCommand::Look) {
            InputCommand::Look => input::look(),
            InputCommand::Known => input::known(),
        },

        Command::Capture { what } => match what {
            CaptureCommand::Status => capture::status(),
            CaptureCommand::List => capture::list(),
            CaptureCommand::Calls => capture::calls(),
            CaptureCommand::Allow { key } => capture::allow(&key),
            CaptureCommand::Deny { key } => capture::deny(&key),
            CaptureCommand::Check => {
                if capture::check()? {
                    Err("a screen recorder you have not allowed is running".to_string())
                } else {
                    Ok(())
                }
            }
        },

        Command::Policy { what } => match what {
            PolicyCommand::Status => policy::status(),
            PolicyCommand::Seal {
                encrypt_recordings,
                clean_metadata,
                neutralise_accent,
                app_lock,
                minimum_intensity,
                note,
                replace,
            } => {
                let mut wanted = Vec::new();
                if encrypt_recordings {
                    wanted.push(Requirement::EncryptRecordings);
                }
                if clean_metadata {
                    wanted.push(Requirement::CleanMetadata);
                }
                if neutralise_accent {
                    wanted.push(Requirement::NeutraliseAccent);
                }
                if app_lock {
                    wanted.push(Requirement::AppLock);
                }
                if let Some(hundredths) = minimum_intensity {
                    // Refused rather than clamped, for the reason the whole
                    // project refuses rather than clamps: a user who typed 150
                    // meant something, and quietly turning it into 100 makes
                    // the policy say something they did not write.
                    if hundredths > 100 {
                        return Err(format!(
                            "--minimum-intensity is a percentage from 0 to 100, not {hundredths}"
                        ));
                    }
                    wanted.push(Requirement::MinimumIntensity(hundredths));
                }
                policy::seal(wanted, note, replace)
            }
            PolicyCommand::Verify => {
                if policy::verify()? {
                    Err("the policy does not match its seal".to_string())
                } else {
                    Ok(())
                }
            }
            PolicyCommand::Remove { yes } => policy::remove(yes),
        },

        Command::Sentry { what } => match what {
            SentryCommand::Status => sentry::status(),
            SentryCommand::Plant { dir, name } => sentry::plant(&dir, name.as_deref()),
            SentryCommand::PullUp { path } => sentry::pull_up(&path),
            SentryCommand::Baseline {
                dir,
                max_files,
                max_depth,
            } => sentry::baseline(
                &dir,
                Limits {
                    max_files,
                    max_depth,
                },
            ),
            SentryCommand::Check {
                files_per_minute,
                share,
                max_files,
                max_depth,
            } => {
                if !(0.0..=1.0).contains(&share) {
                    // Refused rather than clamped. A share of 2.0 can never be
                    // met, so clamping it to 1.0 would silently turn a typo
                    // into a threshold that fires on every full rewrite -- and
                    // the user would believe they had asked for something else.
                    return Err(format!(
                        "--share is a proportion from 0.0 to 1.0, not {share}"
                    ));
                }
                if !(files_per_minute.is_finite() && files_per_minute >= 0.0) {
                    return Err(format!(
                        "--files-per-minute must be zero or more, not {files_per_minute}"
                    ));
                }
                let tripped = sentry::check(
                    Threshold {
                        files_per_minute,
                        share,
                    },
                    Limits {
                        max_files,
                        max_depth,
                    },
                )?;
                if tripped {
                    Err("a canary tripped".to_string())
                } else {
                    Ok(())
                }
            }
        },

        Command::Companions { install: wanted } => match wanted {
            None => {
                list_companions();
                Ok(())
            }
            Some(name) => install_companion(&name),
        },
    }
}

/// Report every companion that means anything on this platform.
///
/// Reporting is all this does. The licence and the author are printed beside
/// each one because somebody deciding whether to install software is entitled
/// to both before they decide, not afterwards in a manual.
fn list_companions() {
    println!("{}", heading("Companions"));
    println!("  None of these is part of VeilVoice and none of them is required.");
    println!();
    for companion in companions::for_this_platform() {
        println!("{}", paint(colour::BLUE, companion.name));
        println!("{}", field("key", companion.key));
        println!("{}", field("made by", companion.vendor));
        println!("{}", field("licence", companion.licence));
        let presence = companion.detect();
        let line = presence.describe();
        println!(
            "{}",
            field(
                "on this machine",
                &if presence.is_present() {
                    line
                } else {
                    format!("{line} (this says where VeilVoice looked, not what you have)")
                }
            )
        );
        println!("      {}", companion.what);
        println!("      {}", companion.why);
        if !presence.is_present() {
            println!("{}", field("to install", &offer_line(&companion.offer())));
        }
        println!();
    }
    println!("  Install one with: veilvoice companions --install <key>");
}

/// One line describing what VeilVoice can do about a missing companion.
fn offer_line(offer: &companions::Offer) -> String {
    match offer {
        companions::Offer::Command {
            via,
            needs_privilege,
            ..
        } => {
            let command = offer.command_line().unwrap_or_default();
            if *needs_privilege {
                format!("{command}   (via {via}; needs root, so run it yourself)")
            } else {
                format!("{command}   (via {via})")
            }
        }
        companions::Offer::Page(url) => {
            format!("{url}   (their installer, run by you, under their licence)")
        }
        companions::Offer::PartOfTheSystem(explanation) => explanation.to_string(),
        companions::Offer::NotOnThisPlatform => "not applicable on this platform".to_string(),
        companions::Offer::NoKnownRoute(reason) => reason.clone(),
    }
}

/// Act on one named companion. The name is the explicit yes.
fn install_companion(name: &str) -> Result<(), String> {
    let Some(companion) = companions::by_key(name) else {
        let known: Vec<&str> = companions::for_this_platform()
            .iter()
            .map(|c| c.key)
            .collect();
        return Err(format!(
            "no companion called '{name}'. On this platform: {}",
            known.join(", ")
        ));
    };
    println!("{}", heading(companion.name));
    println!("{}", field("made by", companion.vendor));
    println!("{}", field("licence", companion.licence));

    let presence = companion.detect();
    if presence.is_present() {
        println!(
            "{}",
            ok(&format!("already here -- {}", presence.describe()))
        );
        return Ok(());
    }
    println!("{}", field("looked", &presence.describe()));

    let offer = companion.offer();
    match &offer {
        companions::Offer::Page(url) => {
            // Never fetched and never executed. VB-CABLE is proprietary
            // donationware with its own licence, and a program whose subject
            // is verifying what you run has no business running an unverified
            // third-party installer on somebody's behalf.
            println!();
            println!("{}", warn("VeilVoice does not install this for you."));
            println!("  Get it from {url}, read their licence, and run their");
            println!("  installer yourself. On Windows, reboot afterwards before");
            println!("  using live mode.");
            Ok(())
        }
        companions::Offer::Command {
            via,
            needs_privilege,
            ..
        } => {
            let command = offer.command_line().unwrap_or_default();
            println!("{}", field("via", via));
            println!("{}", field("command", &command));
            if *needs_privilege {
                println!();
                println!(
                    "{}",
                    warn(
                        "this needs root, and VeilVoice does not ask for a password. \
                         Run that command yourself."
                    )
                );
                return Ok(());
            }
            println!();
            match companions::run(&offer) {
                Ok(report) => {
                    for line in report.lines() {
                        println!("  {line}");
                    }
                    println!("{}", ok("done"));
                    Ok(())
                }
                Err(error) => {
                    println!("{}", err(&error));
                    Err("the companion was not installed".to_string())
                }
            }
        }
        _ => {
            println!();
            println!("  {}", offer_line(&offer));
            Ok(())
        }
    }
}

/// The engine settings a user can reach from the command line.
#[derive(Clone, Copy)]
struct Tuning {
    intensity: f32,
    keep_accent: bool,
    reseed_secs: f32,
    /// The randomised roll range, already parsed and already refused if it was
    /// not usable. `None` means the fixed [`Tuning::reseed_secs`] interval.
    reseed_range: Option<(f32, f32)>,
}

/// Turn `--reseed-range` into a range, or into the reason it was not one.
///
/// Three answers, and the middle one is the point of F-73:
///
/// * `Some(text)` -- parse it, and **refuse** anything unusable rather than
///   adjusting it to fit.
/// * `None` -- draw a range from the operating system's random source, so the
///   shipped interval is a property of this launch rather than of the binary.
/// * `"fixed"` -- the caller has asked for the old fixed interval by name,
///   which is the only way to get a predictable ratchet.
fn reseed_range_from(flag: Option<&str>) -> Result<Option<(f32, f32)>, String> {
    match flag {
        Some(text) if text.trim().eq_ignore_ascii_case("fixed") => Ok(None),
        Some(text) => veilvoice_core::parse_reseed_range(text)
            .map(Some)
            .map_err(|why| format!("--reseed-range: {why}")),
        // The default, and the whole of F-73. `with_random_reseed_range` was
        // written, documented as what the front ends do at launch, and called
        // by nothing but its own test for two releases.
        None => Ok(DeidConfig::default()
            .with_random_reseed_range()
            .reseed_range_ms),
    }
}

fn config(t: Tuning) -> DeidConfig {
    DeidConfig {
        intensity: t.intensity.clamp(0.0, 1.0),
        accent: AccentConfig {
            enabled: !t.keep_accent,
            ..AccentConfig::default()
        },
        reseed_secs: t.reseed_secs.max(0.0),
        reseed_range_ms: t.reseed_range,
        ..DeidConfig::default()
    }
}

/// How a randomised roll range reads in the output.
///
/// Reports [`DeidConfig::effective_reseed_range_ms`] rather than what was
/// asked for: the ratchet can only fire on a frame boundary, so the range that
/// takes effect is quantised, and printing the request would tell somebody
/// their interval varies over a span it does not.
fn describe_reseed_range(config: &DeidConfig) -> String {
    match config.effective_reseed_range_ms() {
        Some((lo, hi)) => {
            format!("{lo:.0}-{hi:.0} ms, drawn fresh before every roll -- no period to observe")
        }
        None => describe_reseed(config.reseed_secs),
    }
}

/// How the seed-rolling setting reads in the output.
fn describe_reseed(secs: f32) -> String {
    if secs <= 0.0 {
        "off — one stream for the whole session".to_string()
    } else {
        format!("every {secs}s")
    }
}

/// What to do with the result once it exists.
struct AtRest {
    /// Seal the recording rather than writing it in the clear. Default on.
    encrypt: bool,
    /// Seal to this public key instead of a passphrase.
    to: Option<PathBuf>,
    /// Do not stop to confirm an unencrypted write.
    yes: bool,
}

fn anonymise(
    input: PathBuf,
    output: Option<PathBuf>,
    tuning: Tuning,
    clean_metadata: bool,
    at_rest: AtRest,
) -> Result<(), String> {
    if at_rest.to.is_some() && !at_rest.encrypt {
        return Err("--encrypt-to and --encrypt false ask for opposite things".into());
    }

    let out_path = output.unwrap_or_else(|| {
        let mut p = input.clone();
        p.set_extension("veiled.wav");
        p
    });

    let audio = audio_io::load(&input).map_err(|e| e.to_string())?;
    println!("{}", heading("Input"));
    println!("{}", field("File", &input.display().to_string()));
    println!(
        "{}",
        field("Duration", &format!("{:.2} s", audio.duration_secs()))
    );
    println!(
        "{}",
        field("Sample rate", &format!("{} Hz", audio.sample_rate))
    );

    let started = std::time::Instant::now();
    let veiled = veilvoice_audio::deidentify(&audio, config(tuning)).map_err(|e| e.to_string())?;
    let elapsed = started.elapsed().as_secs_f32();

    // Encoded in memory, so an encrypted result never exists on disk in the
    // clear even for a moment.
    let mut wav = audio_io::wav_bytes(&veiled).map_err(|e| e.to_string())?;
    let mut removed = Vec::new();
    if clean_metadata {
        match veilvoice_meta::clean_wav_bytes(&wav, Policy::Strip) {
            Ok((cleaned, report)) => {
                wav = cleaned;
                removed = report.removed;
            }
            Err(e) => println!("{}", warn(&format!("could not clean metadata: {e}"))),
        }
    }
    if !removed.is_empty() {
        println!("{}", field("Metadata removed", &removed.join(", ")));
    }

    let written = if at_rest.encrypt {
        println!();
        let recipient = match at_rest.to.as_deref() {
            Some(key) => atrest::Recipient::PublicKey(key),
            None => atrest::Recipient::Password,
        };
        atrest::seal_to_disk(&out_path, &wav, recipient)?
    } else {
        atrest::confirm_plaintext(at_rest.yes)?;
        // An unencrypted recording is still a recording of everything that was
        // said — the warning just above says exactly that — so at minimum it is
        // not left readable by every other account on the machine. A file
        // permission is a much weaker thing than the encryption being declined
        // here, and the summary below says so rather than letting it read as a
        // consolation.
        veilvoice_crypto::privatefile::write_owner_only(&out_path, &wav)
            .map_err(|e| format!("{}: {e}", out_path.display()))?;
        out_path.clone()
    };

    println!();
    println!("{}", heading("Result"));
    println!("{}", field("Written", &written.display().to_string()));
    println!(
        "{}",
        field(
            "Speed",
            &format!("{:.1}x realtime", audio.duration_secs() / elapsed.max(1e-6))
        )
    );
    println!(
        "{}",
        field(
            "Accent",
            if tuning.keep_accent {
                "kept"
            } else {
                "neutralised"
            }
        )
    );
    println!(
        "{}",
        field("Seed rolls", &describe_reseed_range(&config(tuning)))
    );
    println!(
        "{}",
        field(
            "At rest",
            &match (at_rest.encrypt, at_rest.to.is_some()) {
                (true, true) => "sealed to a public key (X25519 + ML-KEM-768)".to_string(),
                (true, false) => "sealed with a passphrase (Argon2id)".to_string(),
                (false, _) => "UNENCRYPTED".to_string(),
            }
        )
    );
    println!();
    println!(
        "{}",
        ok("done — the voiceprint in this file is not recoverable")
    );
    if at_rest.encrypt {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Open it again with: veilvoice decrypt <file> -o out.wav"
            )
        );
    } else {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  The words are still there; that is deliberate. To hide the"
            )
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  message as well, encrypt it: veilvoice encrypt"
            )
        );
    }
    Ok(())
}

#[cfg(feature = "live")]
fn live(input: Option<String>, output: Option<String>, tuning: Tuning) -> Result<(), String> {
    let in_device =
        devices::open(devices::Direction::Input, input.as_deref()).map_err(|e| e.to_string())?;

    // With no explicit choice, prefer a virtual cable: routing into one is what
    // makes the veiled voice usable by other applications.
    let out_name = match output {
        Some(name) => Some(name),
        None => devices::find_virtual_cable().map(|d| d.name),
    };
    let out_device = devices::open(devices::Direction::Output, out_name.as_deref())
        .map_err(|e| e.to_string())?;

    println!("{}", heading("Live scramble"));
    println!("{}", field("Input", &devices::name_of(&in_device)));
    println!("{}", field("Output", &devices::name_of(&out_device)));
    println!(
        "{}",
        field(
            "Accent",
            if tuning.keep_accent {
                "kept"
            } else {
                "neutralised"
            }
        )
    );
    println!(
        "{}",
        field("Seed rolls", &describe_reseed_range(&config(tuning)))
    );
    if out_name.is_none() {
        println!(
            "{}",
            warn("no virtual audio cable found — routing to the default output")
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Install VB-CABLE (Windows) or BlackHole (macOS) so other"
            ),
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  applications can receive the veiled voice."
            )
        );
    }

    let session = veilvoice_audio::LiveSession::start(&in_device, &out_device, config(tuning))
        .map_err(|e| e.to_string())?;

    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  dBFS, peak. 0 is full scale; speech usually sits near -12."
        )
    );
    println!("{}", paint(colour::MUTED, "  Ctrl-C to stop."));
    println!();

    const WIDTH: usize = 20;
    let mut in_meter = meter::Channel::default();
    let mut out_meter = meter::Channel::default();

    // Sixty times a second would be smoother and would also be sixty terminal
    // writes a second for a bar twenty characters wide. Twenty is fast enough
    // that a syllable moves the bar, and the peak hold is what catches what
    // falls between two frames -- nothing is missed, because the audio thread
    // keeps the maximum and resets it on read.
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let s = session.stats();
        let glitches = if s.dropped > 0 || s.starved > 0 {
            paint(
                colour::YELLOW,
                &format!("  drops {} / starves {}", s.dropped, s.starved),
            )
        } else {
            String::new()
        };
        // Sticky, because clipping is destructive and is over in a
        // millisecond: a warning that has gone before the person looks up was
        // never given.
        let clipped = if in_meter.has_clipped() || out_meter.has_clipped() {
            paint(colour::RED, "  CLIPPED")
        } else {
            String::new()
        };
        print!(
            "\r  {} {}   {} {}   {} {:.1} ms{}{}   ",
            paint(colour::MUTED, " in"),
            in_meter.update(s.input_peak, WIDTH),
            paint(colour::MUTED, "out"),
            out_meter.update(s.output_peak, WIDTH),
            paint(colour::MUTED, "cpu"),
            s.process.ema_block_ms(),
            glitches,
            clipped
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}

#[cfg(feature = "live")]
fn list_devices() -> Result<(), String> {
    for (label, direction) in [
        ("Inputs", devices::Direction::Input),
        ("Outputs", devices::Direction::Output),
    ] {
        println!("{}", heading(label));
        match devices::list(direction) {
            Ok(list) if list.is_empty() => println!("  {}", paint(colour::MUTED, "none found")),
            Ok(list) => {
                for d in list {
                    let mut marks = Vec::new();
                    if d.is_default {
                        marks.push(paint(colour::GREEN, "default"));
                    }
                    if d.is_virtual_cable {
                        marks.push(paint(colour::PURPLE, "virtual cable"));
                    }
                    let suffix = if marks.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", marks.join(", "))
                    };
                    println!("  {}{}", d.name, suffix);
                }
            }
            Err(e) => println!("  {}", warn(&e.to_string())),
        }
        println!();
    }
    Ok(())
}

fn clean(file: PathBuf, policy: Policy) -> Result<(), String> {
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    let report = if veilvoice_meta::ImageKind::sniff(&bytes).is_some() {
        veilvoice_meta::clean_image_file(&file, policy).map_err(|e| e.to_string())?
    } else {
        veilvoice_meta::clean_audio_file(&file, policy).map_err(|e| e.to_string())?
    };

    if report.changed {
        println!("{}", ok(&format!("cleaned {}", file.display())));
        println!("{}", field("Removed", &report.removed.join(", ")));
    } else {
        println!("{}", ok(&format!("{} was already clean", file.display())));
    }
    Ok(())
}

fn encrypt(input: PathBuf, output: Option<PathBuf>, to: Option<PathBuf>) -> Result<(), String> {
    let out_path = output.unwrap_or_else(|| container::veil_path(&input));
    let plaintext = std::fs::read(&input).map_err(|e| e.to_string())?;

    let sealed = match to {
        Some(key_path) => {
            let encoded = std::fs::read(&key_path).map_err(|e| e.to_string())?;
            let pk = hybrid::PublicKey::from_bytes(&encoded).map_err(|e| e.to_string())?;
            container::seal_to_public_key(&pk, &plaintext).map_err(|e| e.to_string())?
        }
        None => {
            let password = read_new_password()?;
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  Deriving key (Argon2id, this is meant to be slow)..."
                )
            );
            container::seal_with_password(password.expose(), &plaintext, kdf::KdfParams::default())
                .map_err(|e| e.to_string())?
        }
    };

    std::fs::write(&out_path, &sealed).map_err(|e| e.to_string())?;
    println!("{}", ok(&format!("encrypted to {}", out_path.display())));
    Ok(())
}

fn decrypt(input: PathBuf, output: PathBuf, key: Option<PathBuf>) -> Result<(), String> {
    let sealed = std::fs::read(&input).map_err(|e| e.to_string())?;

    let plaintext = match key {
        Some(key_path) => {
            let sk = load_secret_key(&key_path)?;
            container::open_with_secret_key(&sk, &sealed).map_err(|e| e.to_string())?
        }
        None => {
            let password = prompt_secret("Passphrase: ")?;
            container::open_with_password(password.expose(), &sealed).map_err(|e| e.to_string())?
        }
    };

    // Owner-only from the moment it exists. This is the *decrypted* contents of
    // something the user chose to encrypt; writing it out world-readable, even
    // for the instant before a chmod, would undo the point of having sealed it.
    veilvoice_crypto::privatefile::write_owner_only(&output, &plaintext)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    println!("{}", ok(&format!("decrypted to {}", output.display())));
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Written so only your account can read it. That is a file permission, \
             not disk encryption."
        )
    );
    Ok(())
}

/// Load a private key file, which is itself a password-locked container.
fn load_secret_key(path: &std::path::Path) -> Result<hybrid::SecretKey, String> {
    let sealed = std::fs::read(path).map_err(|e| e.to_string())?;
    let password = prompt_secret("Key passphrase: ")?;
    let encoded =
        container::open_with_password(password.expose(), &sealed).map_err(|e| e.to_string())?;
    hybrid::SecretKey::from_bytes(&encoded).map_err(|e| e.to_string())
}

fn keygen(public: PathBuf, secret: PathBuf) -> Result<(), String> {
    // Reported early so the user is not asked for a passphrase before being
    // told the file is in the way. The *refusal* that matters is not this one
    // though — it is `write_owner_only_new` below, which asks the kernel to
    // fail if anything is already there. Checking `exists()` and then writing
    // is a race, and it follows a symbolic link planted at the path.
    for path in [&public, &secret] {
        if path.exists() {
            return Err(format!(
                "{} already exists — refusing to overwrite a key file",
                path.display()
            ));
        }
    }

    let (sk, pk) = hybrid::SecretKey::generate().map_err(|e| e.to_string())?;

    // The private key is never written in the clear: it is sealed with the same
    // container format everything else uses, so a stolen key file is worth
    // nothing without the passphrase.
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Choose a passphrase to protect the private key."
        )
    );
    let password = read_new_password()?;
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Deriving key (Argon2id, deliberately slow)..."
        )
    );
    let encoded = sk.to_bytes();
    let sealed = container::seal_with_password(
        password.expose(),
        encoded.expose(),
        kdf::KdfParams::default(),
    )
    .map_err(|e| e.to_string())?;

    // The public key is meant to be shared, so it is written normally. The
    // private key is created owner-only and *exclusively*: the permission is
    // applied by the creation rather than by a chmod afterwards, and the
    // creation fails rather than overwriting anything already at the path.
    std::fs::write(&public, pk.to_bytes()).map_err(|e| format!("{}: {e}", public.display()))?;
    veilvoice_crypto::privatefile::write_owner_only_new(&secret, &sealed)
        .map_err(|e| format!("{}: {e}", secret.display()))?;

    println!("{}", ok(&format!("public key  {}", public.display())));
    println!(
        "{}",
        ok(&format!("private key {} (encrypted)", secret.display()))
    );
    println!();
    println!("{}", field("Algorithm", "X25519 + ML-KEM-768 hybrid"));
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Share the public key freely. Anyone holding it can"
        )
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  encrypt to you; only the private key can open it."
        )
    );
    Ok(())
}

/// Report, and keep reporting, what is using the microphone and camera.
fn watch(once: bool, interval: f32) -> Result<(), String> {
    use veilvoice_watch::{Change, DeviceKind, Monitor};

    let support = veilvoice_watch::support();
    println!("{}", heading("Microphone and camera monitor"));
    println!(
        "{}",
        field(
            "Detection",
            if support.microphone && support.camera {
                "microphone and camera"
            } else if support.microphone {
                "microphone only"
            } else {
                "unavailable on this platform"
            }
        )
    );
    println!(
        "{}",
        paint(colour::MUTED, &format!("  {}", support.explanation))
    );

    // An empty list from a platform that cannot see is not good news, and must
    // never be presented as though it were.
    if !support.microphone && !support.camera {
        println!();
        return Err("nothing can be detected here, so nothing is reported".into());
    }
    println!();

    let mut monitor = Monitor::new();
    let sleep = std::time::Duration::from_secs_f32(interval.clamp(0.2, 60.0));

    loop {
        let changes = monitor.poll().map_err(|e| e.to_string())?;
        for change in &changes {
            let (mark, shade) = match change {
                Change::Started(u) if u.kind == DeviceKind::Camera => ("●", colour::RED),
                Change::Started(_) => ("●", colour::YELLOW),
                Change::Stopped(_) => ("○", colour::GREEN),
            };
            println!("  {} {}", paint(shade, mark), change.alert());
        }

        if once {
            let active = monitor.current();
            if active.is_empty() {
                println!("{}", ok("nothing is using the microphone or camera"));
            } else {
                for entry in active {
                    println!(
                        "{}",
                        field(
                            &entry.kind.to_string(),
                            &format!(
                                "{}{}",
                                entry.describe(),
                                entry
                                    .device
                                    .as_deref()
                                    .map(|d| format!("  [{d}]"))
                                    .unwrap_or_default()
                            )
                        )
                    );
                    if let Some(path) = &entry.path {
                        println!("{}", paint(colour::MUTED, &format!("      {path}")));
                    }
                }
            }
            return Ok(());
        }

        if changes.is_empty() && monitor.current().is_empty() {
            print!(
                "\r  {}   ",
                paint(colour::MUTED, "watching - nothing active")
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::thread::sleep(sleep);
    }
}

/// Destroy a file's contents, then delete it.
///
/// Gated behind a typed confirmation rather than a y/n prompt. There is no
/// undo, and a reflexive "y" is exactly the mistake this is guarding against.
fn shred(file: PathBuf, passes: u8, yes: bool) -> Result<(), String> {
    let metadata = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", file.display()));
    }

    println!("{}", heading("Self-destruct"));
    println!("{}", field("File", &file.display().to_string()));
    println!(
        "{}",
        field(
            "Size",
            &format!("{:.1} KiB", metadata.len() as f64 / 1024.0)
        )
    );
    println!("{}", field("Passes", &passes.to_string()));
    println!();
    println!("{}", err("THIS CANNOT BE UNDONE."));
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  On an SSD, SD card or USB stick, wear levelling may leave the
               original blocks in flash where no software can reach them.
               Full-disk encryption is the reliable answer — destroy the key
               and the data goes with it, wherever the drive put it."
        )
    );
    println!();

    if !yes {
        print!("  Type DESTROY to continue: ");
        use std::io::Write;
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if answer.trim() != "DESTROY" {
            return Err("cancelled — nothing was touched".into());
        }
    }

    let report = veilvoice_crypto::shred_file(&file, veilvoice_crypto::Passes::Custom(passes))
        .map_err(|e| e.to_string())?;

    println!();
    println!(
        "{}",
        ok(&format!(
            "overwrote {} bytes in {} passes, then deleted it",
            report.bytes, report.passes
        ))
    );
    if !report.synced {
        println!(
            "{}",
            warn("the OS did not confirm the overwrite reached the device")
        );
    }
    println!();
    println!("{}", paint(colour::MUTED, "  What this does not cover:"));
    for note in &report.caveats {
        println!("{}", paint(colour::MUTED, &format!("   - {note}")));
    }
    Ok(())
}

fn info() {
    println!("{}", heading("VeilVoice"));
    println!("{}", field("Version", env!("CARGO_PKG_VERSION")));
    println!("{}", field("Engine", veilvoice_core::VERSION));
    println!("{}", field("Crypto", veilvoice_crypto::VERSION));
    println!("{}", field("Audio", veilvoice_audio::VERSION));
    println!("{}", field("Metadata", veilvoice_meta::VERSION));
    println!("{}", field("Monitor", veilvoice_watch::VERSION));
    // "Licence" the noun, to match the desktop app and the website. "License"
    // stays only where it is part of the proper name "GNU General Public
    // License" or an SPDX identifier.
    println!("{}", field("Licence", "GPL-3.0-or-later"));
    println!("{}", field("Network access", "none, by construction"));
    println!(
        "{}",
        field(
            "Live audio",
            if cfg!(feature = "live") {
                "available"
            } else {
                "not built in (no device backend for this platform)"
            }
        )
    );
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  VeilVoice destroys the voiceprint, not the words."
        )
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  See docs/WHITEPAPER.md for what that does and does not"
        )
    );
    println!("{}", paint(colour::MUTED, "  protect against."));
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    fn tuning(intensity: f32, keep_accent: bool, reseed_secs: f32) -> Tuning {
        Tuning {
            intensity,
            keep_accent,
            reseed_secs,
            // The fixed interval, so these tests stay deterministic. The
            // randomised range has its own tests in veilvoice-core.
            reseed_range: None,
        }
    }

    #[test]
    fn intensity_is_clamped_into_range() {
        assert_eq!(config(tuning(5.0, false, 2.0)).intensity, 1.0);
        assert_eq!(config(tuning(-1.0, false, 2.0)).intensity, 0.0);
        assert_eq!(config(tuning(0.5, false, 2.0)).intensity, 0.5);
    }

    #[test]
    fn keep_accent_disables_neutralisation() {
        assert!(!config(tuning(1.0, true, 2.0)).accent.enabled);
        assert!(config(tuning(1.0, false, 2.0)).accent.enabled);
    }

    #[test]
    fn reseed_interval_reaches_the_engine_and_cannot_go_negative() {
        assert_eq!(config(tuning(1.0, false, 0.5)).reseed_secs, 0.5);
        assert_eq!(config(tuning(1.0, false, 0.0)).reseed_secs, 0.0);
        assert_eq!(config(tuning(1.0, false, -3.0)).reseed_secs, 0.0);
    }

    /// Encryption at rest is a *default*, not a flag the careful user has to
    /// find. If someone ever flips this, this test is what stops it shipping.
    #[test]
    fn recordings_are_encrypted_at_rest_by_default() {
        let cli = Cli::try_parse_from(["veilvoice", "anonymise", "in.wav"]).unwrap();
        let Command::Anonymise {
            encrypt,
            encrypt_to,
            yes,
            ..
        } = cli.command
        else {
            panic!("expected anonymise");
        };
        assert!(encrypt, "at-rest encryption must default on");
        assert!(encrypt_to.is_none());
        assert!(!yes, "the confirmation must not be pre-answered");

        let off = Cli::try_parse_from(["veilvoice", "anonymise", "in.wav", "--encrypt", "false"])
            .unwrap();
        let Command::Anonymise { encrypt, .. } = off.command else {
            panic!("expected anonymise");
        };
        assert!(!encrypt, "it must still be possible to opt out");
    }

    /// Refused before anything is read or written, so a contradictory command
    /// line cannot half-happen.
    #[test]
    fn asking_for_a_recipient_and_for_plaintext_at_once_is_refused() {
        let result = anonymise(
            PathBuf::from("does-not-need-to-exist.wav"),
            None,
            tuning(1.0, false, 2.0),
            true,
            AtRest {
                encrypt: false,
                to: Some(PathBuf::from("someone.pub")),
                yes: true,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn reseed_setting_reads_clearly() {
        assert!(describe_reseed(0.0).contains("off"));
        assert!(describe_reseed(2.0).contains("2"));
    }

    #[cfg(feature = "live")]
    #[test]
    fn meter_scales_and_never_panics() {
        // The meter itself, its scale and its edge cases, are tested in
        // `meter.rs` beside the code. What is worth checking from here is that
        // the two are still connected: a level a person would call loud must
        // not draw as an empty bar, which is what the linear meter this
        // replaced did to ordinary speech.
        for peak in [-1.0f32, 0.0, 0.25, 0.5, 1.0, 4.0] {
            let bar = meter::render(peak, 0.0, 12);
            assert!(bar.chars().count() >= 12);
        }
        assert!(meter::render(0.0, 0.0, 12).contains('·'));
        assert!(meter::render(1.0, 0.0, 12).contains('█'));
        assert!(
            meter::render(0.251, 0.0, 12).matches('█').count() >= 8,
            "speech at -12 dBFS must fill most of the bar"
        );
    }
}
