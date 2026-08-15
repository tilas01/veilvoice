// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice` — the command-line interface.
//!
//! Everything VeilVoice does, available without a desktop: it runs over SSH, in
//! a container, and on machines that have no GUI toolkit at all. The same
//! engine backs both this and the graphical app.
#![forbid(unsafe_code)]

mod theme;

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
        } => anonymise(
            input,
            output,
            Tuning {
                intensity,
                keep_accent,
                reseed_secs,
            },
            clean_metadata,
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
        Command::Shred { file, passes, yes } => shred(file, passes, yes),
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

fn anonymise(
    input: PathBuf,
    output: Option<PathBuf>,
    tuning: Tuning,
    clean_metadata: bool,
) -> Result<(), String> {
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

    audio_io::save_wav(&out_path, &veiled).map_err(|e| e.to_string())?;

    if clean_metadata {
        match veilvoice_meta::clean_audio_file(&out_path, Policy::Strip) {
            Ok(report) if report.changed => {
                println!("{}", field("Metadata removed", &report.removed.join(", ")));
            }
            Ok(_) => {}
            Err(e) => println!("{}", warn(&format!("could not clean metadata: {e}"))),
        }
    }

    println!();
    println!("{}", heading("Result"));
    println!("{}", field("Written", &out_path.display().to_string()));
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
    println!();
    println!(
        "{}",
        ok("done — the voiceprint in this file is not recoverable")
    );
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

/// Read a password twice, without echoing it, and check the two agree.
fn read_new_password() -> Result<Vec<u8>, String> {
    let first = rpassword::prompt_password("Passphrase: ").map_err(|e| e.to_string())?;
    if first.is_empty() {
        return Err("passphrase must not be empty".into());
    }
    let again = rpassword::prompt_password("Repeat: ").map_err(|e| e.to_string())?;
    if first != again {
        return Err("passphrases do not match".into());
    }
    Ok(first.into_bytes())
}

fn encrypt(input: PathBuf, output: Option<PathBuf>, to: Option<PathBuf>) -> Result<(), String> {
    let out_path = output.unwrap_or_else(|| {
        let mut p = input.clone();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        p.set_extension(if ext.is_empty() {
            "veil".into()
        } else {
            format!("{ext}.veil")
        });
        p
    });
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
            container::seal_with_password(&password, &plaintext, kdf::KdfParams::default())
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
            let password = rpassword::prompt_password("Passphrase: ").map_err(|e| e.to_string())?;
            container::open_with_password(password.as_bytes(), &sealed)
                .map_err(|e| e.to_string())?
        }
    };

    std::fs::write(&output, &plaintext).map_err(|e| e.to_string())?;
    println!("{}", ok(&format!("decrypted to {}", output.display())));
    Ok(())
}

/// Load a private key file, which is itself a password-locked container.
fn load_secret_key(path: &std::path::Path) -> Result<hybrid::SecretKey, String> {
    let sealed = std::fs::read(path).map_err(|e| e.to_string())?;
    let password = rpassword::prompt_password("Key passphrase: ").map_err(|e| e.to_string())?;
    let encoded =
        container::open_with_password(password.as_bytes(), &sealed).map_err(|e| e.to_string())?;
    hybrid::SecretKey::from_bytes(&encoded).map_err(|e| e.to_string())
}

fn keygen(public: PathBuf, secret: PathBuf) -> Result<(), String> {
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
    let sealed =
        container::seal_with_password(&password, encoded.expose(), kdf::KdfParams::default())
            .map_err(|e| e.to_string())?;

    std::fs::write(&public, pk.to_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(&secret, &sealed).map_err(|e| e.to_string())?;
    restrict_permissions(&secret);

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

/// Make a private key readable only by its owner, where the OS supports it.
fn restrict_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    // On Windows the file inherits the user profile's ACL, which already
    // excludes other unprivileged users; there is no portable tightening to do.
    #[cfg(not(unix))]
    let _ = path;
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
    println!("{}", field("License", "GPL-3.0-or-later"));
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
