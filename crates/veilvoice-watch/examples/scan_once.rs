// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Print what is using the microphone and camera right now.
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
