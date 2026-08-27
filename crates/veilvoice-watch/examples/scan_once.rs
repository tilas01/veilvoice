// SPDX-License-Identifier: GPL-3.0-or-later
//! Print what is using the microphone and camera right now.
//!
//! # In plain words
//!
//! Looks once at which programs are using the microphone or camera, prints them,
//! and stops.
//!
//! For checking what the monitor can actually see on a particular machine, without
//! running the whole application to find out.
fn main() {
    let s = veilvoice_watch::support();
    println!("microphone detection: {}", s.microphone);
    println!("camera detection:     {}", s.camera);
    println!("how: {}\n", s.explanation);
    match veilvoice_watch::scan() {
        Ok(list) if list.is_empty() => println!("nothing is using the microphone or camera"),
        Ok(list) => {
            for u in list {
                println!(
                    "{} -> {}  [{}]",
                    u.kind,
                    u.describe(),
                    u.path.unwrap_or_default()
                );
            }
        }
        Err(e) => println!("error: {e}"),
    }
}
