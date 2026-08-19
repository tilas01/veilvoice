// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice` — the command-line interface.
//!
//! Everything VeilVoice does, available without a desktop: it runs over SSH, in
//! a container, and on machines that have no GUI toolkit at all. The same
//! engine backs both this and the graphical app.
//!
//! # What is here
//!
//! Fourteen subcommands, and they divide into four groups:
//!
//! * **Audio** -- `anonymise` a file, `live` scramble a microphone, list
//!   `devices`.
//! * **Privacy of the files themselves** -- `clean` metadata, `encrypt`,
//!   `decrypt`, `keygen`, `shred`.
//! * **Watching the machine** -- `watch` the microphone and camera, `guard`
//!   VeilVoice's own files against tampering.
//! * **The app lock** -- `lock set|status|change|remove`.
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
#![forbid(unsafe_code)]

mod atrest;
mod guard;
mod lock;
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

#[derive(Parser)]
#[command(
    name = "veilvoice",
    version,
    about = "Irreversible voice de-identification — fully offline.",
    long_about = "VeilVoice destroys the biometric voiceprint of a speaker — pitch, \
formants, timbre and the melody of an accent — while keeping the words clean and \
transcribable. It talks to no servers, ever."
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
        } => live(
            input,
            output,
            Tuning {
                intensity,
                keep_accent,
                reseed_secs,
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
    }
}

/// The engine settings a user can reach from the command line.
#[derive(Clone, Copy)]
struct Tuning {
    intensity: f32,
    keep_accent: bool,
    reseed_secs: f32,
}

fn config(t: Tuning) -> DeidConfig {
    DeidConfig {
        intensity: t.intensity.clamp(0.0, 1.0),
        accent: AccentConfig {
            enabled: !t.keep_accent,
            ..AccentConfig::default()
        },
        reseed_secs: t.reseed_secs.max(0.0),
        ..DeidConfig::default()
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
        field("Seed rolls", &describe_reseed(tuning.reseed_secs))
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
        field("Seed rolls", &describe_reseed(tuning.reseed_secs))
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
    println!("{}", paint(colour::MUTED, "  Ctrl-C to stop."));
    println!();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let s = session.stats();
        let glitches = if s.dropped > 0 || s.starved > 0 {
            paint(
                colour::YELLOW,
                &format!("  drops {} / starves {}", s.dropped, s.starved),
            )
        } else {
            String::new()
        };
        print!(
            "\r  {} in {}  out {}   {} {:.1} ms{}   ",
            paint(colour::MUTED, "lvl"),
            meter(s.input_peak),
            meter(s.output_peak),
            paint(colour::MUTED, "cpu"),
            s.process.ema_block_ms(),
            glitches
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}

/// A small textual level meter.
#[cfg(feature = "live")]
fn meter(peak: f32) -> String {
    const WIDTH: usize = 12;
    let filled = ((peak.clamp(0.0, 1.0)) * WIDTH as f32).round() as usize;
    let bar: String = "█".repeat(filled) + &"·".repeat(WIDTH - filled);
    let shade = if peak > 0.95 {
        colour::RED
    } else if peak > 0.7 {
        colour::YELLOW
    } else {
        colour::GREEN
    };
    paint(shade, &bar)
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
        for peak in [-1.0f32, 0.0, 0.25, 0.5, 1.0, 4.0] {
            let bar = meter(peak);
            assert!(bar.chars().count() >= 12);
        }
        assert!(meter(0.0).starts_with('·'));
        assert!(meter(1.0).starts_with('█'));
    }
}
