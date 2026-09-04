// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice record` -- capture the veiled voice straight into an encrypted
//! file.
//!
//! # What this is, next to the commands beside it
//!
//! `veilvoice live` veils the voice and sends it to a device, keeping nothing.
//! `veilvoice anonymise` veils a recording somebody already made, which means
//! the original exists, in the clear, on their disk, and stays there unless
//! they remember to shred it.
//!
//! This is the third case and the one that leaves the least behind: the
//! microphone goes in, the veiled voice comes out, and the only file that ever
//! exists is the encrypted one.
//!
//! # Never a plaintext file, not even briefly
//!
//! The recording is accumulated in a `Tape`, encoded
//! into a `Secret`, and sealed from there. At no
//! point is there a WAV on disk to be deleted afterwards, because a plaintext
//! file that is written and deleted is exactly what
//! `veilvoice_crypto::shred` explains cannot be reliably taken back on flash
//! storage. Writing one and encrypting it afterwards would leave the original
//! recoverable and the file merely tidy.
//!
//! # Why it stops on a keypress rather than on Ctrl-C
//!
//! Ctrl-C ends the process, and a recording that ends by killing the process
//! is a recording that is never sealed and never written. Catching the signal
//! instead would mean a signal-handling dependency for one command, and this
//! project argues at length against dependencies nobody has read.
//!
//! So it stops on Enter, or after `--seconds`. Both are ordinary control flow,
//! reach the sealing step, and need nothing new in the dependency tree. Ctrl-C
//! still works and still abandons the recording, which is the correct thing for
//! it to do: it is how somebody says "stop, and keep nothing".
//!
//! # In plain words
//!
//! Records you with your voice already disguised, and saves it encrypted.
//!
//! There is never an unencrypted copy of the recording anywhere, not even for a
//! moment, so there is nothing to delete afterwards and nothing to recover from
//! the disk.

use crate::meter;
use crate::theme::{colour, field, heading, ok, paint, warn};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use veilvoice_audio::{devices, record};

/// How the recording is to be protected once it is made.
pub struct Sealing {
    /// Seal to this recipient's public key rather than to a passphrase.
    pub public_key: Option<PathBuf>,
    /// Write it unencrypted. Asks first, loudly.
    pub plaintext: bool,
    /// Answer that prompt with yes.
    pub yes: bool,
}

/// Run a recording session and seal what it captured.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: Option<String>,
    output: Option<String>,
    seconds: Option<f32>,
    to: Option<PathBuf>,
    monitor: bool,
    sealing: Sealing,
    tuning: crate::Tuning,
) -> Result<(), String> {
    // Refuse before recording, not after. Finding out that the output path is
    // unusable once somebody has finished speaking would mean either losing the
    // recording or holding it hostage to a prompt.
    if sealing.plaintext && sealing.public_key.is_some() {
        return Err(
            "--plaintext and --to-public-key ask for opposite things; name one".to_string(),
        );
    }
    let destination = destination(to)?;

    let in_device =
        devices::open(devices::Direction::Input, input.as_deref()).map_err(|e| e.to_string())?;
    // The default is this machine's own output, unlike `live`. Recording is not
    // routing: nobody is on the other end of a cable waiting for it, and
    // sending it to one would put the recording into whatever is listening
    // there as well as into the file.
    let out_device =
        devices::open(devices::Direction::Output, output.as_deref()).map_err(|e| e.to_string())?;

    println!("{}", heading("Record, veiled and encrypted"));
    println!("{}", field("Input", &devices::name_of(&in_device)));
    println!("{}", field("Monitor", &devices::name_of(&out_device)));
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
    println!("{}", field("Writes", &destination.display().to_string()));
    if sealing.plaintext {
        println!("{}", warn("unencrypted, once you have confirmed it"));
    } else if sealing.public_key.is_some() {
        println!("{}", field("Sealed to", "a recipient public key"));
    } else {
        println!(
            "{}",
            field("Sealed with", "a passphrase, asked for at the end")
        );
    }
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Use headphones: speakers plus a microphone is a feedback loop."
        )
    );

    let config = crate::config(tuning);
    let rate = config.sample_rate as u32;
    let (mut recorder, sink) = record::start(rate);
    let session =
        veilvoice_audio::LiveSession::start_recording(&in_device, &out_device, config, Some(sink))
            .map_err(|e| e.to_string())?;

    println!();
    match seconds {
        Some(n) => println!("{}", ok(&format!("recording for {n:.0} seconds"))),
        None => println!("{}", ok("recording. Press Enter to stop and save.")),
    }
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Ctrl-C abandons it instead, keeping nothing."
        )
    );
    println!();

    let stop = stop_signal(seconds);
    const WIDTH: usize = 20;
    let mut in_meter = meter::Channel::default();
    let mut out_meter = meter::Channel::default();

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Draining is what moves the audio out of the ring and into locked
        // memory. Skipping it while the meters draw would lose the recording a
        // ring-length at a time, so it happens every tick regardless of whether
        // anything is being drawn.
        recorder.drain();
        if !monitor {
            continue;
        }
        let s = session.stats();
        let lost = if recorder.dropped() > 0 {
            paint(
                colour::YELLOW,
                &format!("  lost {} samples", recorder.dropped()),
            )
        } else {
            String::new()
        };
        print!(
            "\r  {} {}   {} {}   {} {:>6.1}s{}   ",
            paint(colour::MUTED, " in"),
            in_meter.update(s.input_peak, WIDTH),
            paint(colour::MUTED, "out"),
            out_meter.update(s.output_peak, WIDTH),
            paint(colour::MUTED, "length"),
            recorder.seconds(),
            lost,
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    // Stop the audio before sealing. Argon2id takes a noticeable moment, and
    // samples arriving during it would be recorded after the point the person
    // asked it to stop.
    drop(session);
    println!();
    println!();

    let length = recorder.seconds();
    let wav = recorder.wav().map_err(|e| e.to_string())?;
    report(&recorder, length, wav.len());

    if sealing.plaintext {
        crate::atrest::confirm_plaintext(sealing.yes)?;
        veilvoice_crypto::privatefile::write_owner_only(&destination, wav.expose())
            .map_err(|e| format!("{}: {e}", destination.display()))?;
        println!("{}", field("written", &destination.display().to_string()));
        println!("{}", warn("unencrypted, as you asked"));
        return Ok(());
    }

    let recipient = match &sealing.public_key {
        Some(path) => crate::atrest::Recipient::PublicKey(path),
        None => crate::atrest::Recipient::Password,
    };
    let out = crate::atrest::seal_to_disk(&destination, wav.expose(), recipient)?;
    println!("{}", ok(&format!("sealed to {}", out.display())));
    Ok(())
}

/// What was captured, and what was actually obtained for it.
///
/// The locking line is the honest one: it says what the operating system
/// granted rather than what was asked for, because [`veilvoice_crypto::tape`]
/// cannot promise a lock and neither can this.
fn report(recorder: &record::Recorder, seconds: f32, bytes: usize) {
    println!("{}", field("length", &format!("{seconds:.1} s")));
    println!("{}", field("size", &format!("{} KiB", bytes / 1024)));
    if recorder.dropped() > 0 {
        println!(
            "{}",
            warn(&format!(
                "{} samples were lost: the recording has a gap",
                recorder.dropped()
            ))
        );
    }
    if recorder.fully_locked() {
        println!(
            "{}",
            field("in memory", "held locked out of the page file throughout")
        );
    } else {
        println!(
            "{}",
            warn("some of the recording could not be locked out of the page file")
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  The operating system limits how much a program may lock, and this"
            )
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  recording was larger than that limit. It was still wiped from memory."
            )
        );
    }
}

/// A flag that becomes true when the recording should stop.
///
/// Either after `seconds`, or when Enter is pressed. The reading thread is
/// detached and blocks on stdin: it is never joined, because a fixed-length
/// recording must not wait for a keypress that is not coming.
fn stop_signal(seconds: Option<f32>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    match seconds {
        Some(n) => {
            let stop = Arc::clone(&stop);
            let millis = (n.max(0.0) * 1000.0) as u64;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(millis));
                stop.store(true, Ordering::Relaxed);
            });
        }
        None => {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                stop.store(true, Ordering::Relaxed);
            });
        }
    }
    stop
}

/// Where the recording goes, defaulting to a timestamped name here.
///
/// The default carries the moment it was made rather than a counter, so two
/// recordings never race for the same name and the file says when it happened
/// without depending on a filesystem timestamp that a copy would not preserve.
fn destination(to: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = to {
        if path.as_os_str().is_empty() {
            return Err("the output path is empty".to_string());
        }
        return Ok(path);
    }
    Ok(PathBuf::from(format!("veilvoice-{}.wav", stamp())))
}

/// `YYYYMMDD-HHMMSS` in UTC, for a filename.
///
/// Built from the same civil-date arithmetic the mandate history uses, rather
/// than from a date crate, for the reason recorded there: one line of output
/// does not justify a dependency nobody has read.
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let text = veilvoice_policy::utc(secs);
    // "YYYY-MM-DD HH:MM:SS UTC" to "YYYYMMDD-HHMMSS".
    let cleaned: String = text
        .trim_end_matches(" UTC")
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ' ')
        .collect();
    cleaned.replace(' ', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_name_is_a_timestamp_a_filesystem_accepts() {
        let path = destination(None).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("veilvoice-"), "{name}");
        assert!(name.ends_with(".wav"), "{name}");
        // Nothing a Windows path rejects, and nothing a shell would split on.
        for bad in [':', ' ', '/', '\\', '*', '?', '"', '<', '>', '|'] {
            assert!(!name.contains(bad), "{name} contains {bad:?}");
        }
    }

    #[test]
    fn a_timestamp_is_the_shape_the_name_promises() {
        let s = stamp();
        let (date, time) = s.split_once('-').expect("a date and a time");
        assert_eq!(date.len(), 8, "{s}");
        assert_eq!(time.len(), 6, "{s}");
        assert!(s.chars().all(|c| c.is_ascii_digit() || c == '-'), "{s}");
    }

    #[test]
    fn an_explicit_destination_is_used_as_given() {
        let path = destination(Some(PathBuf::from("interview.wav"))).unwrap();
        assert_eq!(path, PathBuf::from("interview.wav"));
    }

    #[test]
    fn an_empty_destination_is_refused_rather_than_turned_into_a_default() {
        // Silently substituting a default would write the recording somewhere
        // the person did not name, which for an encrypted file they then have
        // to find is worse than an error.
        assert!(destination(Some(PathBuf::new())).is_err());
    }
}
