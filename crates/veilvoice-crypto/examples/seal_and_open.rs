// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//
// Sealing a recording into a `.veil` container and opening it again, plus
// handling a typed passphrase so that it does not linger any longer than it
// must.
//
//     cargo run -p veilvoice-crypto --example seal_and_open
//
// Compiled on every commit by `cargo clippy --workspace --all-targets`, so the
// copy of this in `docs/USING_THE_CRATES.md` cannot quietly stop being true.

use veilvoice_crypto::{container, kdf::KdfParams, Secret};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Stand in for a WAV. The container does not care what the bytes are.
    let recording: Vec<u8> = b"RIFF....WAVEfmt ....pretend this is audio".to_vec();

    // A passphrase arrives from a prompt or a text field as an ordinary
    // `String`: swappable heap memory that nothing wipes. Copy it into a
    // `Secret`, which is page-locked where the OS allows and zeroized on drop,
    // and zero the copy you made on the way.
    //
    // What this does NOT do is wipe the `String`'s own buffer -- that needs
    // `unsafe`, and every crate here carries `#![forbid(unsafe_code)]`. The
    // residue is audit item A-5, recorded rather than papered over: for as
    // long as something is receiving keystrokes, the bytes are ordinary
    // memory. Shrinking that window from "until the program exits" to "while
    // the user was typing" is the part that was worth doing.
    let typed = String::from("correct horse battery staple");
    let mut buffer = typed.into_bytes();
    let secret = Secret::new(&mut buffer);
    debug_assert!(
        buffer.iter().all(|b| *b == 0),
        "Secret::new must zero what it was given"
    );

    if !secret.is_locked() {
        // Reported rather than assumed: page locking genuinely fails on some
        // systems, and a library that pretends otherwise is worse than one
        // that does not try.
        eprintln!("note: this passphrase could not be page-locked out of swap");
    }

    // The KDF cost travels with the file, which is what lets an old container
    // still open after the defaults rise. Coming back *out* of a file those
    // values are attacker-controlled, which is why they are bounded on parse
    // rather than trusted (findings F-2, F-3, F-20).
    let sealed = container::seal_with_password(secret.expose(), &recording, KdfParams::default())?;
    println!(
        "sealed {} bytes into {} bytes",
        recording.len(),
        sealed.len()
    );

    let opened = container::open_with_password(secret.expose(), &sealed)?;
    assert_eq!(opened, recording);
    println!("opened, and the bytes match");

    // A wrong passphrase is an error, never a partial result.
    match container::open_with_password(b"not the passphrase", &sealed) {
        Ok(_) => panic!("a wrong passphrase must never open a container"),
        Err(why) => println!("wrong passphrase refused: {why}"),
    }

    println!(
        "a sealed recording is written to {}",
        container::veil_path(std::path::Path::new("clean.wav")).display()
    );
    Ok(())
}
