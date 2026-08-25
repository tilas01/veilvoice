// SPDX-License-Identifier: GPL-3.0-or-later
//! Run the real check once, by hand.
//!
//! `cargo run -p veilvoice-update --example ask`
//!
//! Not a test: it reaches the network, and a test suite that does is a test
//! suite that fails on a train. This exists so the thing can be tried against
//! the real page before it is believed.
fn main() {
    match veilvoice_update::check("0.1.12") {
        Ok(report) => {
            println!("current {}", report.current);
            println!("latest  {}", report.latest);
            println!("verdict {:?}", report.verdict);
            println!("caveat  {}", report.caveat());
        }
        Err(error) => println!("could not check: {error}"),
    }
}
