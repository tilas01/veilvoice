// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice`, the command-line interface.
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

mod accel;
mod appctl;
mod atrest;
mod capture;
mod conversation;
mod decoy;
mod failsafe;
mod guard;
mod gui;
mod input;
mod lock;
mod mandate;
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
    about = "Irreversible voice de-identification, fully offline.",
    long_about = "VeilVoice destroys the biometric voiceprint of a speaker: the \
pitch, the formants, the timbre and the melody of an accent. It keeps the words clean and \
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

        /// Listen to yourself veiled, instead of sending it anywhere.
        ///
        /// Routes the veiled voice to this machine's **default output** rather
        /// than to a virtual cable, so it goes to your headphones and to
        /// nothing else. This is the way to find out what you sound like, and
        /// that the microphone is the one you meant, before an interview
        /// starts rather than during it.
        ///
        /// Use headphones. Speakers plus a microphone is a feedback loop.
        #[arg(long)]
        preview: bool,

        /// Do not draw the level meters.
        ///
        /// The meters are on by default because the two questions in a live
        /// session are "is it hearing me" and "is anything coming out", and a
        /// bar answers both at a glance. This turns them off for a terminal
        /// that is being logged or read by something other than a person.
        #[arg(long)]
        no_monitor: bool,
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

    /// The graphics hardware here, and what it is good for.
    ///
    /// Lists the devices, says which can encode video, and suggests one. It
    /// also says, with the measurement behind it, why veiling a voice does not
    /// use a graphics card: it is already about a hundred times faster than
    /// real time, and moving that work onto a card would slow it down.
    Accel,

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
    ///
    /// `veilvoice g` is the same command. It looks beside this program first,
    /// then where an install puts it, then on your PATH -- and if it finds
    /// nothing it says where it looked. The window opens and this terminal is
    /// yours again immediately; closing it will not close the window.
    #[command(alias = "g")]
    Gui {
        /// Open it without printing anything.
        #[arg(long, short)]
        quiet: bool,
    },

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

    /// The two things VeilVoice insists on, unless you say otherwise.
    ///
    /// By default it wants a password for itself and encrypts every recording
    /// where it is stored. Both are on without anybody choosing them, because
    /// both protect the same thing: de-identification takes the voiceprint out
    /// of a recording and leaves every word that was said in it.
    ///
    /// This is the one place in VeilVoice that can make it *less* strict, and
    /// it is the opposite tool to `veilvoice policy`. A sealed policy is
    /// somebody setting rules for somebody else and can only tighten; this is
    /// your own baseline for your own machine, and you may relax it. Doing so
    /// is written down with the date, so the choice is never a mystery later.
    Mandate {
        #[command(subcommand)]
        what: MandateCommand,
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

    /// A second passphrase that opens an empty VeilVoice, and its limits.
    ///
    /// A decoy is a way to comply with somebody standing over you without
    /// handing over your recordings. **It does not give you deniability**:
    /// VeilVoice is open source and this feature is documented, so anybody who
    /// recognises the program can ask you for the other passphrase.
    ///
    /// **No passphrase destroys anything, deliberately.** On modern storage a
    /// write does not overwrite, so a feature that claimed to would be lying to
    /// you at the worst possible moment.
    Decoy,

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
    /// Check a download, and what does the checking
    ///
    /// With nothing after it, this finds a release near you and checks all of
    /// it: the signature over the hash list, every archive against that list,
    /// the contents manifest, and every file you extracted, one by one. That
    /// is the whole of what used to be a separate `veilvoice-verify` program,
    /// which no longer ships because this is where it belonged.
    ///
    /// `veilvoice verify --help` lists the rest: `key`, `sums`, `file`,
    /// `gnupg`, `release`, and the build-it-yourself half -- `deps`, `build`
    /// and `reproduce`. They are printed below the flags, from the verifier's
    /// own help, so the two cannot disagree.
    ///
    /// `--how` explains how a release is verified and what GnuPG adds that
    /// VeilVoice cannot add for itself.
    ///
    /// `--script` writes a short shell script that does the check with `gpg`
    /// and `sha256sum` and nothing from this project. That is the point of it.
    /// The program telling you a download is genuine came out of that
    /// download, so the check worth most is the one made by software this
    /// project did not write.
    #[command(after_long_help = veilvoice_verify::help_text())]
    Verify {
        /// Explain how a release is verified, without checking anything.
        ///
        /// This was what `verify` printed with no arguments, before the
        /// verifier was folded in and the useful default became doing the
        /// check rather than describing it.
        #[arg(long)]
        how: bool,
        /// Write the verification script to standard output instead.
        #[arg(long)]
        script: bool,
        /// Write a script that rebuilds the release from source and compares
        /// the result with the published binary. A stronger check than a
        /// hash: a hash proves the file is the one whose hash was signed, and
        /// says nothing about what is inside it.
        #[arg(long)]
        build_script: bool,
        /// Which system the script is for: linux, macos, bsd or windows.
        /// Defaults to this one.
        #[arg(long, value_name = "SYSTEM")]
        system: Option<String>,
        /// The macOS spelling of the verification script, which uses `shasum`
        /// rather than `sha256sum`. A shorthand for `--system macos`.
        #[arg(long)]
        macos: bool,
        /// A verifier command and its arguments, passed through untouched.
        ///
        /// Not modelled here on purpose: the verifier has its own parser, its
        /// own help and its own documented exit statuses, and a second parser
        /// in this file is how the two drift apart.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Turn a veiled recording into a video with a black picture
    ///
    /// For posting somewhere that will not accept an audio file. The picture is
    /// black for the length of the recording and is not the point.
    ///
    /// Needs `ffmpeg`, which VeilVoice does not ship and will not install: it
    /// prints the exact command when the tool is not there.
    Video {
        /// The veiled recording to put in it.
        audio: PathBuf,
        /// Where to write the video. Defaults to the recording's name with
        /// `.mp4`.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Print the command rather than running it.
        #[arg(long)]
        dry_run: bool,
    },

    /// Take the sound out of a recording made somewhere else
    ///
    /// OBS and everything like it write containers holding a video stream and
    /// an audio stream. VeilVoice reads audio, so this pulls the audio out into
    /// a WAV that `veilvoice anonymise` can take.
    ///
    /// Needs `ffmpeg`, for the same reason and with the same behaviour.
    Import {
        /// The recording to take the sound out of.
        source: PathBuf,
        /// Where to write the WAV. Defaults to the source's name with `.wav`.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Print the command rather than running it.
        #[arg(long)]
        dry_run: bool,
    },

    /// Encrypted volumes this machine has: Cryptomator and VeraCrypt
    ///
    /// Reports what is installed and what is mounted right now. It never opens,
    /// closes or unlocks anything, and never asks for a volume password:
    /// mounting your encrypted storage is your act, taken in the tool you
    /// chose.
    Volumes,
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

/// What `veilvoice mandate` can do.
#[derive(Subcommand)]
enum MandateCommand {
    /// What is required now, and where it is written down.
    Status,

    /// Stop insisting on a requirement. Asks before it does it.
    Relax {
        /// Stop requiring a password for VeilVoice itself.
        #[arg(long)]
        app_lock: bool,
        /// Stop requiring recordings to be encrypted where they are stored.
        #[arg(long)]
        encryption: bool,
        /// Proceed rather than explaining what would happen.
        #[arg(long)]
        yes: bool,
    },

    /// Insist on a requirement again.
    Insist {
        /// Require a password for VeilVoice itself.
        #[arg(long)]
        app_lock: bool,
        /// Require recordings to be encrypted where they are stored.
        #[arg(long)]
        encryption: bool,
    },

    /// Go back to insisting on both.
    Reset,

    /// Every change that has been made, oldest first.
    History,
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

/// What checking a release actually involves, and who does which part.
///
/// Written out here rather than left to the website, because somebody on a
/// machine with no browser is exactly the person most likely to be checking a
/// download by hand.
fn explain_verification(
    flavour: veilvoice_gnupg::script::Flavour,
    system: veilvoice_check::reproduce::System,
) {
    let survey = veilvoice_gnupg::backend::look();
    println!("Checking a VeilVoice release");
    println!();
    println!("  Three files, all published together on the releases page:");
    println!("    the archive          the thing you downloaded");
    println!("    SHA256SUMS           a hash for every file in the release");
    println!("    SHA256SUMS.asc       a signature over that list");
    println!();
    println!("  https://github.com/tilas01/veilvoice/releases/latest");
    println!();
    println!("  The signing key is published beside them as");
    println!("  `veilvoice-signing-key.asc`, and is in the repository at");
    println!("  website/assets/veilvoice-signing-key.asc, so it can be fetched");
    println!("  from somewhere other than the release being checked.");
    println!();
    println!("    fingerprint  {}", veilvoice_check::FINGERPRINT);
    println!();
    println!("  Compare that against the fingerprint on the website and in");
    println!("  README.md. It is the one step nothing can do for you.");
    println!();
    println!("What is already in this binary");
    println!();
    println!("  The signature check itself, in Rust, with the key above compiled");
    println!("  in. Nothing needs installing for it, on any platform. It is what");
    println!("  `veilvoice-verify` uses, and it also checks every file extracted");
    println!("  out of the archive against a signed contents list.");
    println!();
    println!("  What it cannot do is vouch for itself. This program came out of");
    println!("  the download it would be checking, so a tampered release ships a");
    println!("  tampered checker. That is not a bug to fix; it is why the second");
    println!("  opinion below is worth having.");
    println!();
    println!("GnuPG, which is not part of VeilVoice");
    match &survey.native {
        Some(path) => println!("  found at {}", path.display()),
        None => println!("  not on PATH. `veilvoice companions --install gnupg` prints the"),
    }
    if survey.native.is_none() {
        println!("  command that installs it; VeilVoice does not run installers.");
    }
    if let Some(wsl) = &survey.wsl {
        println!();
        println!("  WSL is on this machine, at {}.", wsl.program.display());
        println!("  A `gpg` inside it works just as well, and `wsl gpg ...` is how");
        println!("  the commands are spelled. VeilVoice does not start WSL to find");
        println!("  out what is in it unless you ask, because starting it starts a");
        println!("  Linux distribution.");
    }
    println!();
    println!("  Nothing here runs GnuPG on its own. An implementation other than");
    println!("  this one is used when you choose it, in the desktop application");
    println!("  under Verify, and not because one happened to be on PATH.");
    println!();
    println!("The script");
    println!();
    println!("  veilvoice verify --script > {}", flavour.file_name());
    println!();
    println!("  Sixty lines of shell using gpg and the system hash tool. Read it");
    println!("  before running it: the reason to use it rather than this program");
    println!("  is that it is not this program.");
    println!();
    println!("The stronger check");
    println!();
    println!("  veilvoice verify --build-script > {}", system.file_name());
    println!();
    println!("  A hash proves the file is the one whose hash was signed. It says");
    println!("  nothing about what is inside it, because the same person signed");
    println!("  both. That script rebuilds the release from source and compares");
    println!("  the result, so a match means the published binary is what this");
    println!("  source compiles to. It needs git and a Rust toolchain, and it");
    println!("  installs neither.");
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // The verifier is answered here rather than in `run` because it does not
    // fit that function's shape, and bending it to fit would lose the part
    // that matters. `run` returns ok-or-a-message, which collapses to exit 0
    // or 1; the verifier has a documented table of statuses -- 2 means a check
    // failed, 5 means a build differed, and they are deliberately not the same
    // number -- that scripts and CI already branch on. Flattening those into 1
    // would silently break every one of them.
    if let Command::Verify {
        how,
        script,
        build_script,
        ref args,
        ..
    } = cli.command
    {
        // `--how`, `--script` and `--build-script` are this command's own and
        // are handled in `run`. Everything else is the verifier's, including
        // the no-argument case: `veilvoice verify` on its own finds a release
        // nearby and checks it, which is what the standalone program did when
        // somebody double-clicked it.
        if !how && !script && !build_script {
            return veilvoice_verify::run(args.clone());
        }
    }

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
            preview,
            no_monitor,
        } => live(
            input,
            output,
            preview,
            !no_monitor,
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

        // The search lives in `gui`, which looks in three places rather than
        // only beside this binary, and never starts anything by a bare name --
        // Windows resolves those through the current directory first.
        Command::Accel => accel::show(),

        Command::Decoy => decoy::explain(),

        Command::Gui { quiet } => gui::open(quiet),

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
                            "VeilVoice never checks for updates and cannot tell you when \
                             one exists -- it has no network code at all. Watch the \
                             releases page, and verify what you download."
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

        Command::Mandate { what } => match what {
            MandateCommand::Status => mandate::status(),
            MandateCommand::Relax {
                app_lock,
                encryption,
                yes,
            } => mandate::relax(app_lock, encryption, yes),
            MandateCommand::Insist {
                app_lock,
                encryption,
            } => mandate::insist(app_lock, encryption),
            MandateCommand::Reset => mandate::reset(),
            MandateCommand::History => mandate::history(),
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

        Command::Verify {
            how,
            script,
            build_script,
            system,
            macos,
            args,
        } => {
            // The script flags and `--how` are this command's own. Anything
            // else -- a verifier subcommand, or nothing at all -- belongs to
            // the verifier, and is dispatched in `main` so its exit statuses
            // survive. Reaching here with either means the caller asked for a
            // script and a check in one breath, which is two answers to one
            // question.
            if !args.is_empty() {
                return Err(format!(
                    "`{}` is a check, and --script and --how only describe one. Run them separately.",
                    args.join(" ")
                ));
            }
            if how {
                let flavour = match system.as_deref() {
                    Some("macos") => veilvoice_gnupg::script::Flavour::MacOs,
                    _ if macos => veilvoice_gnupg::script::Flavour::MacOs,
                    _ => veilvoice_gnupg::script::Flavour::Linux,
                };
                explain_verification(flavour, veilvoice_check::reproduce::System::here());
                return Ok(());
            }
            let named = system.as_deref().map(|name| {
                veilvoice_check::reproduce::System::from_key(name).ok_or_else(|| {
                    format!(
                        "unknown system {name:?}. One of: {}",
                        veilvoice_check::reproduce::System::ALL
                            .iter()
                            .map(|s| s.key())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
            });
            let system = match named {
                Some(Ok(system)) => system,
                Some(Err(why)) => return Err(why),
                None if macos => veilvoice_check::reproduce::System::MacOs,
                None => veilvoice_check::reproduce::System::here(),
            };
            if build_script {
                print!("{}", veilvoice_check::reproduce::script(system));
                return Ok(());
            }
            // The verification script knows only two spellings, because the
            // only thing that differs is the hash tool, and the BSDs use the
            // same one Linux does for this purpose.
            let flavour = match system {
                veilvoice_check::reproduce::System::MacOs => {
                    veilvoice_gnupg::script::Flavour::MacOs
                }
                _ => veilvoice_gnupg::script::Flavour::Linux,
            };
            if script {
                print!("{}", veilvoice_gnupg::script::shell(flavour));
                return Ok(());
            }
            explain_verification(flavour, system);
            Ok(())
        }

        Command::Companions { install: wanted } => match wanted {
            None => {
                list_companions();
                Ok(())
            }
            Some(name) => install_companion(&name),
        },
        Command::Volumes => {
            list_volumes();
            Ok(())
        }

        Command::Video {
            audio,
            output,
            dry_run,
        } => {
            let out = output.unwrap_or_else(|| audio.with_extension("mp4"));
            let argv = veilvoice_video::ffmpeg::black_command(
                &audio,
                &out,
                veilvoice_video::ffmpeg::Encoding::default(),
            );
            run_ffmpeg("Video", &argv, &out, dry_run)
        }

        Command::Import {
            source,
            output,
            dry_run,
        } => {
            if !veilvoice_video::ffmpeg::is_container(&source) {
                println!(
                    "{}",
                    warn(&format!(
                        "{} is not a container VeilVoice knows how to open; trying anyway",
                        source.display()
                    ))
                );
            }
            let out = output.unwrap_or_else(|| source.with_extension("wav"));
            let argv = veilvoice_video::ffmpeg::extract_command(&source, &out);
            run_ffmpeg("Import", &argv, &out, dry_run)
        }
    }
}

/// Markers 87 and 88. Run an ffmpeg command, or print it when there is no
/// ffmpeg to run.
///
/// The same bargain the rest of this project makes about other people's
/// software: VeilVoice does not ship a codec, does not install one, and does
/// not fail silently when one is missing. It says what it would have run, so
/// somebody can run it themselves or install the tool and try again.
///
/// The command is printed either way, before it runs. A tool that shells out
/// and shows only the result leaves nobody able to check what it did.
fn run_ffmpeg(
    what: &str,
    argv: &[String],
    output: &std::path::Path,
    dry_run: bool,
) -> Result<(), String> {
    println!("{}", heading(what));
    println!("{}", field("Writing", &output.display().to_string()));
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            &format!("  {}", veilvoice_video::ffmpeg::command_line(argv))
        )
    );
    println!();

    if dry_run {
        println!("{}", ok("printed, not run"));
        return Ok(());
    }

    let Some(ffmpeg) = veilvoice_video::ffmpeg::found() else {
        println!(
            "{}",
            warn("ffmpeg is not on this machine, so nothing was run")
        );
        println!();
        for line in veilvoice_video::ffmpeg::describe().lines() {
            println!("{}", paint(colour::MUTED, &format!("  {line}")));
        }
        return Ok(());
    };

    let status = std::process::Command::new(&ffmpeg)
        .args(&argv[1..])
        .status()
        .map_err(|e| format!("could not start {}: {e}", ffmpeg.display()))?;
    if !status.success() {
        return Err(format!(
            "ffmpeg stopped with {status}. The command above is what it was asked to do."
        ));
    }
    // ffmpeg created the file, so it carries ffmpeg's umask, and for `import`
    // that file is the *original* audio pulled out of a container: the
    // untouched voiceprint, which is the most revealing thing this program
    // ever writes. It was being left readable by every account on the machine.
    //
    // Tightened rather than written by us, because the writing is ffmpeg's
    // job. The window between its creation and this line is real and is not
    // closed here; `privatefile::tighten` says so where it is defined, and the
    // line printed below does not overstate what happened.
    let tightened = veilvoice_crypto::privatefile::tighten(output).is_ok();
    println!("{}", ok(&format!("wrote {}", output.display())));
    if tightened && cfg!(unix) {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Set readable only by your account, after ffmpeg wrote it. That is a \n\
                 file permission and nothing more."
            )
        );
    }
    Ok(())
}

/// Report the encrypted volumes this machine is offering.
///
/// Markers 81 to 85, from the command line. Reporting only: the same rule the
/// window follows, and for the same reason. A hidden-volume question cannot be
/// answered here because there is nothing to remember it against, so this
/// prints what a volume would need before the desktop application would write
/// to it rather than pretending to settle it.
fn list_volumes() {
    use veilvoice_setup::volumes::{self, Tool};

    println!("{}", heading("Encrypted volumes"));
    println!("  None of these is part of VeilVoice and none of them is required.");
    println!();
    for tool in Tool::ALL {
        println!("{}", paint(colour::BLUE, tool.name()));
        println!("{}", field("key", tool.key()));
        println!("{}", field("state", &volumes::installed(*tool).describe()));
        println!("{}", field("home", tool.home_page()));
        println!();
    }

    let mounted = volumes::mounted();
    println!("{}", heading("Mounted now"));
    if mounted.is_empty() {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  nothing recognisable is mounted. Unlock a volume in its own program \
                 first; VeilVoice will not do it for you."
            )
        );
    }
    for volume in &mounted {
        println!(
            "{}",
            field(volume.tool.name(), &volume.path.display().to_string())
        );
        if let Some(why) = volume.blocked() {
            println!("{}", warn(why));
        }
    }

    println!();
    println!("{}", paint(colour::MUTED, "  What this is worth:"));
    for line in crate::lock::wrap(volumes::DISK_ADVICE, 66) {
        println!("{}", paint(colour::MUTED, &format!("    {line}")));
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
        "off, so one stream for the whole session".to_string()
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
        // said, which the warning just above says exactly, so at minimum it is
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
        ok("done, and the voiceprint in this file is not recoverable")
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
fn live(
    input: Option<String>,
    output: Option<String>,
    preview: bool,
    monitor: bool,
    tuning: Tuning,
) -> Result<(), String> {
    let in_device =
        devices::open(devices::Direction::Input, input.as_deref()).map_err(|e| e.to_string())?;

    // With no explicit choice, prefer a virtual cable: routing into one is what
    // makes the veiled voice usable by other applications.
    //
    // Except in preview, where the whole point is the opposite. `--preview` is
    // for hearing yourself before anybody else does, so it goes to this
    // machine's default output and to nothing else. Choosing the cable there
    // would send the preview into whatever is listening on it, which is the
    // one thing somebody checking their setup does not want.
    let out_name = match (&output, preview) {
        (Some(name), _) => Some(name.clone()),
        (None, true) => None,
        (None, false) => devices::find_virtual_cable().map(|d| d.name),
    };
    let out_device = devices::open(devices::Direction::Output, out_name.as_deref())
        .map_err(|e| e.to_string())?;

    println!(
        "{}",
        heading(if preview {
            "Live scramble - preview"
        } else {
            "Live scramble"
        })
    );
    println!("{}", field("Input", &devices::name_of(&in_device)));
    println!("{}", field("Output", &devices::name_of(&out_device)));
    if preview {
        // **F-84.** Printed after the device, and it names the device rather
        // than claiming something about it.
        //
        // The first version said "goes to this machine's output and nowhere
        // else" before the output was even named, and that is not always true:
        // `--preview --output <a cable>` keeps the cable, and a machine whose
        // *default* output is a cable does the same thing without being asked.
        // Whatever is listening on that cable then hears the preview.
        //
        // A false reassurance in the one place somebody is checking their
        // setup is worse than no reassurance, because checking is what they
        // came here to do. So the claim is scoped to the named device, and
        // when that device is a cable it is said outright.
        let cable = devices::find_virtual_cable().map(|d| d.name);
        let chosen = devices::name_of(&out_device);
        let into_cable = cable.as_deref() == Some(chosen.as_str());
        println!();
        if into_cable {
            println!(
                "{}",
                warn("that output is a virtual cable, so whatever is listening on it hears this")
            );
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  For a preview only you can hear, run --preview with no --output."
                )
            );
        } else {
            println!(
                "{}",
                paint(
                    colour::YELLOW,
                    "  Preview. The veiled voice goes to the output named above and nowhere else."
                )
            );
        }
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Use headphones: speakers plus a microphone is a feedback loop."
            )
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Listen for a voice that is not yours. That is the check the meters cannot make."
            )
        );
        println!();
    }
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
    if out_name.is_none() && !preview {
        println!(
            "{}",
            warn("no virtual audio cable found, so routing to the default output")
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
    if !monitor {
        // Asked for silence, so say once that it is running and then be quiet.
        // A quiet mode that still prints a bar sixty times a minute is not one.
        println!("{}", ok("running. Ctrl-C to stop."));
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Meters are off. Run without --no-monitor to see the levels."
            )
        );
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  dBFS, peak. 0 is full scale; speech usually sits near -12."
        )
    );
    // The limit, printed where the meters are rather than left to be inferred.
    // A level says sound arrived and sound left. It cannot say the voice was
    // changed: a working meter and a bypassed engine draw the same bar.
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  These show sound arriving and leaving, not that the voice has changed."
        )
    );
    if !preview {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  To hear what you sound like first, stop and run with --preview."
            )
        );
    }
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

/// Read a file, naming it if that fails.
///
/// `std::io::Error` carries no path. `e.to_string()` on a missing file is the
/// bare sentence "No such file or directory (os error 2)", and that is what
/// three of these commands printed:
///
/// ```text
/// $ veilvoice encrypt clip.wav --to nokey.pub
/// ✗ No such file or directory (os error 2)
/// ```
///
/// Which file? The command names two. `anonymise` answered that question,
/// because `atrest.rs` formats the path in, and `encrypt` and `decrypt` did
/// not, because they did not. The same bug as the verifier's unnamed verdict,
/// in a place where the person can at least see their own command line, which
/// is the only reason it is smaller.
///
/// A function rather than a fourth copy of the format string, because that is
/// how the copies got out of step in the first place.
pub(crate) fn read_named(path: &std::path::Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write a file, naming it if that fails.
///
/// The failures here are the ones a person can act on: a directory that does
/// not exist, a read-only disk, a name they cannot write to. All of them need
/// the path to be actionable at all.
pub(crate) fn write_named(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn clean(file: PathBuf, policy: Policy) -> Result<(), String> {
    let bytes = read_named(&file)?;
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
    let plaintext = read_named(&input)?;

    let sealed = match to {
        Some(key_path) => {
            let encoded = read_named(&key_path)?;
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

    write_named(&out_path, &sealed)?;
    println!("{}", ok(&format!("encrypted to {}", out_path.display())));
    Ok(())
}

fn decrypt(input: PathBuf, output: PathBuf, key: Option<PathBuf>) -> Result<(), String> {
    let sealed = read_named(&input)?;

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
    let sealed = read_named(path)?;
    let password = prompt_secret("Key passphrase: ")?;
    let encoded =
        container::open_with_password(password.expose(), &sealed).map_err(|e| e.to_string())?;
    hybrid::SecretKey::from_bytes(&encoded).map_err(|e| e.to_string())
}

fn keygen(public: PathBuf, secret: PathBuf) -> Result<(), String> {
    // Reported early so the user is not asked for a passphrase before being
    // told the file is in the way. The *refusal* that matters is not this one
    // though. It is `write_owner_only_new` below, which asks the kernel to
    // fail if anything is already there. Checking `exists()` and then writing
    // is a race, and it follows a symbolic link planted at the path.
    for path in [&public, &secret] {
        if path.exists() {
            return Err(format!(
                "{} already exists, so refusing to overwrite a key file",
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
               Full-disk encryption is the reliable answer. Destroy the key
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
            return Err("cancelled, and nothing was touched".into());
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

    /// Every command the documentation shows is a command this program has.
    ///
    /// # The class this exists to close
    ///
    /// F-110 was an example plan in `docs/USER_GUIDE.md` that the parser
    /// refused. F-101 was a download page linking two files that were never
    /// published. F-103 was a screenshot compared against a file written by
    /// the same command, so neither could catch the other. F-71 was two
    /// hand-typed copies of a number that drifted together.
    ///
    /// All the same shape: a document describing the program, and nothing
    /// comparing the two. Prose about behaviour goes stale silently, because
    /// the compiler never reads it and the reader who would notice is the one
    /// who has already been misled.
    ///
    /// So the documentation is read here and checked against clap's own tree.
    /// Not against a list kept beside it, which would be another copy to
    /// drift: against the definition the program is built from.
    ///
    /// # Telling a command from a sentence
    ///
    /// The word "veilvoice" appears in these documents as prose, as sample
    /// output and as a command, and only the last is checkable. The first
    /// version of this test walked every occurrence and quietly skipped
    /// anything it could not resolve, which made it blind to exactly the
    /// defect it was written for: `veilvoice frobnicate` was added to the
    /// guide and the test passed.
    ///
    /// An invocation is therefore taken to be one that a reader could copy:
    /// the whole of an inline code span, or a line inside a fenced block,
    /// beginning with `veilvoice`. That excludes the alert
    /// `● veilvoice is now using your microphone`, which is in a fence and is
    /// output rather than something to type.
    ///
    /// # What it can and cannot say
    ///
    /// It checks that a documented subcommand exists and that a documented
    /// long flag belongs to the subcommand it is shown with. That is the half
    /// a machine can settle. Whether the example *works* is not checkable
    /// here and is what running it is for: F-110 needed somebody to type it.
    #[test]
    fn every_command_the_documentation_shows_exists() {
        let docs = [
            ("README.md", include_str!("../../../README.md")),
            (
                "docs/USER_GUIDE.md",
                include_str!("../../../docs/USER_GUIDE.md"),
            ),
            ("docs/INSTALL.md", include_str!("../../../docs/INSTALL.md")),
        ];

        let root = Cli::command();
        let mut problems: Vec<String> = Vec::new();
        let mut checked = 0_usize;

        for (name, text) in docs {
            let text = text.replace("\r\n", "\n");
            let mut fenced = false;
            for (number, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("```") {
                    fenced = !fenced;
                    continue;
                }

                // Everything a reader could copy and run, from this line.
                let mut candidates: Vec<&str> = Vec::new();
                if fenced {
                    let bare = line.trim_start().trim_start_matches("$ ").trim_start();
                    candidates.push(bare);
                } else {
                    let mut rest = line;
                    while let Some(open) = rest.find('`') {
                        let after = &rest[open + 1..];
                        match after.find('`') {
                            Some(close) => {
                                candidates.push(&after[..close]);
                                rest = &after[close + 1..];
                            }
                            None => break,
                        }
                    }
                }

                for candidate in candidates {
                    let Some(after) = candidate.strip_prefix("veilvoice") else {
                        continue;
                    };
                    if !after.starts_with(' ') {
                        // `veilvoice-verify` and `veilvoice-gui` are other
                        // programs, with arguments of their own.
                        continue;
                    }
                    let mut words = after.split_whitespace().peekable();
                    let mut node = &root;
                    let mut path: Vec<String> = Vec::new();

                    // `verify` hands everything after it to the verifier,
                    // which has its own parser, so clap cannot answer for the
                    // words that follow. Checking them against clap would
                    // report every one of them missing; skipping them would
                    // leave the most safety-critical commands in the whole
                    // documentation unchecked. So they are checked against the
                    // verifier's own help, which is the thing that defines
                    // them.
                    if words.peek().copied() == Some("verify") {
                        words.next();
                        checked += 1;
                        let help = veilvoice_verify::help_text();
                        // A `#` starts a trailing comment in every shell
                        // example here, so the comment is cut off first --
                        // reading past one turns English prose into a list of
                        // commands nobody wrote. Of what is left, only the
                        // first word is a command; the rest are its arguments,
                        // and the flags among them are checked below.
                        let real: Vec<&str> = words.take_while(|w| !w.starts_with('#')).collect();
                        let vetted: Vec<&str> = real
                            .iter()
                            .copied()
                            .enumerate()
                            .filter(|(i, w)| *i == 0 || w.starts_with("--"))
                            .map(|(_, w)| w)
                            .collect();
                        for word in vetted {
                            let known = if let Some(flag) = word.strip_prefix("--") {
                                let flag: String = flag
                                    .chars()
                                    .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                                    .collect();
                                flag.is_empty()
                                    || flag == "help"
                                    || flag == "version"
                                    // This command's own flags, which clap owns.
                                    || root
                                        .find_subcommand("verify")
                                        .map(|v| {
                                            v.get_arguments()
                                                .any(|a| a.get_long() == Some(flag.as_str()))
                                        })
                                        .unwrap_or(false)
                                    || help.contains(&format!("--{flag}"))
                            } else if word.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                                && !word.is_empty()
                            {
                                help.contains(&format!("veilvoice verify {word}"))
                            } else {
                                // A path, a hash, a placeholder: an argument
                                // rather than something to look up.
                                true
                            };
                            if !known {
                                problems.push(format!(
                                    "{name}:{}: `veilvoice verify {word}` is documented, \
                                     and the verifier's own help does not mention it",
                                    number + 1,
                                ));
                            }
                        }
                        continue;
                    }

                    // The leading words are subcommands until one is not.
                    while let Some(word) = words.peek().copied() {
                        if word.starts_with('-')
                            || word.contains('.')
                            || word.contains('/')
                            || word.contains('<')
                            || !word.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                        {
                            break;
                        }
                        match node.find_subcommand(word) {
                            Some(next) => {
                                path.push(word.to_string());
                                node = next;
                                words.next();
                            }
                            None => {
                                problems.push(format!(
                                    "{name}:{}: `veilvoice {}{word}` is documented, and \
                                     there is no such command",
                                    number + 1,
                                    path.iter().map(|p| format!("{p} ")).collect::<String>()
                                ));
                                break;
                            }
                        }
                    }
                    if path.is_empty() {
                        continue;
                    }
                    checked += 1;

                    for word in words {
                        if !word.starts_with("--") {
                            continue;
                        }
                        let flag: String = word
                            .trim_start_matches("--")
                            .chars()
                            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                            .collect();
                        if flag.is_empty() || flag == "help" || flag == "version" {
                            continue;
                        }
                        let known = node.get_arguments().any(|a| {
                            a.get_long() == Some(flag.as_str())
                                || a.get_all_aliases()
                                    .map(|all| all.iter().any(|x| *x == flag))
                                    .unwrap_or(false)
                        });
                        if !known {
                            problems.push(format!(
                                "{name}:{}: `veilvoice {} --{flag}` is documented, and \
                                 `{}` has no `--{flag}`",
                                number + 1,
                                path.join(" "),
                                path.join(" ")
                            ));
                        }
                    }
                }
            }
        }

        assert!(
            checked > 20,
            "only {checked} documented invocations were found, which means this \
             test has stopped reading the documentation rather than that the \
             documentation stopped showing commands"
        );
        assert!(
            problems.is_empty(),
            "the documentation shows commands this program does not have:\n  {}",
            problems.join("\n  ")
        );
    }

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
