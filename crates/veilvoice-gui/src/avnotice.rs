// SPDX-License-Identifier: GPL-3.0-or-later
//! Noticing when antivirus software has closed VeilVoice, and saying so kindly.
//!
//! # The problem this is for
//!
//! A newly released application that few people have run yet has, in the
//! language of Windows antivirus, "low reputation". A low-reputation program
//! that reads the microphone and writes encrypted files is exactly the shape a
//! heuristic scanner is tuned to be suspicious of, and some of them will close
//! it -- a false positive, but indistinguishable from a crash to the person
//! sitting in front of it.
//!
//! VeilVoice is offline, reproducible and signed, so a user *can* establish
//! that it is what it says it is. But that is cold comfort if the window just
//! vanished and they have no idea why.
//!
//! # What this does, and what it does not
//!
//! It does **not** try to evade anything, hide from anything, or stop an
//! antivirus doing its job. That would be both wrong and futile. It does the
//! opposite: it helps the user understand what happened so they can make their
//! own decision.
//!
//! On a clean exit VeilVoice removes a small marker file. So on the next
//! launch, a marker still present means the previous run ended without getting
//! to its own shutdown -- and if VeilVoice had crashed, it would have written a
//! crash report on the way down (see [`crate::crashlog`]). A marker present
//! with no crash report is the signature of the process being terminated from
//! outside.
//!
//! When that has happened *and* a known antivirus product is on the machine,
//! the next launch shows a plain notice: which product was found, that a
//! low-reputation app is sometimes stopped by mistake, that they would normally
//! have seen an alert from that product, and that adding an exclusion is worth
//! doing **only** if they are actually seeing the problem. Nothing is changed
//! on the system and nothing is suppressed; it is one paragraph of context.
//!
//! # Why the decision is a pure function
//!
//! Whether to show the notice depends on three facts -- was the last exit
//! unclean, was there a crash report, is an antivirus present -- and getting
//! that logic wrong means either crying wolf or staying silent when it would
//! have helped. [`diagnose`] takes those three as arguments so every branch is
//! tested from any machine, and the platform probing that gathers them is kept
//! separate and thin.

use std::path::PathBuf;

/// An antivirus product recognised on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// The name to show the user, e.g. "Microsoft Defender".
    pub name: &'static str,
}

/// The notice to put in front of the user, once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The products found, most likely culprit first.
    pub products: Vec<Product>,
}

impl Notice {
    /// The message, assembled from the products found.
    ///
    /// Plain, specific, and non-alarming: it names what was found, says the
    /// likely cause, and puts the decision back in the user's hands rather
    /// than telling them to disable their protection.
    pub fn message(&self) -> String {
        let names: Vec<&str> = self.products.iter().map(|p| p.name).collect();
        let found = match names.as_slice() {
            [] => "your antivirus".to_string(),
            [one] => one.to_string(),
            [first, rest @ ..] => format!("{first} (and {})", rest.join(", ")),
        };
        format!(
            "VeilVoice closed unexpectedly last time, and it did not crash on \
             its own. It looks like it was stopped from outside.\n\n{found} is \
             installed on this machine, and a new application that few people \
             have run yet is sometimes stopped by antivirus as a precaution, \
             even when there is nothing wrong with it. If that is what \
             happened you would usually have seen an alert from {found} too.\n\n\
             VeilVoice is offline, reproducible and signed, so you can check it \
             is genuine. The Verify tab and the website walk through how. If, \
             and only if, you keep seeing it closed, adding an exclusion for \
             VeilVoice in {found} will stop it. Nothing here has changed any of \
             your settings."
        )
    }
}

/// Decide whether to show the notice.
///
/// `Some` only when the last exit was unclean, VeilVoice did **not** leave a
/// crash report (so it did not fall over on its own), and at least one
/// antivirus product is present. Any other combination is `None`: a clean exit
/// is nothing to explain, a crash has its own report, and with no antivirus
/// present there is nobody to name and the notice would be a guess.
pub fn diagnose(
    unclean_prior_exit: bool,
    had_crash_report: bool,
    products: &[Product],
) -> Option<Notice> {
    if unclean_prior_exit && !had_crash_report && !products.is_empty() {
        Some(Notice {
            products: products.to_vec(),
        })
    } else {
        None
    }
}

/// The marker whose presence on startup means the last run did not exit cleanly.
pub fn marker_path() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|p| p.with_file_name("session.running"))
}

/// A session's clean-shutdown marker.
///
/// Write it when the window opens, remove it when the window closes normally.
/// If the process is killed, the file is left behind, which is exactly the
/// signal [`diagnose`] reads.
pub struct Session {
    marker: Option<PathBuf>,
    /// Whether a marker was already there when this session began.
    prior_was_unclean: bool,
}

impl Session {
    /// Begin a session: note whether the last one ended cleanly, then claim the
    /// marker for this one.
    pub fn begin() -> Self {
        let marker = marker_path();
        let prior_was_unclean = marker.as_ref().is_some_and(|p| p.exists());
        if let Some(path) = &marker {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Best effort: an unwritable marker means the next launch cannot
            // tell an unclean exit from a clean one, which fails safe (no
            // notice) rather than crying wolf.
            let _ = std::fs::write(path, b"running");
        }
        Self {
            marker,
            prior_was_unclean,
        }
    }

    /// Whether the previous session ended without a clean shutdown.
    pub fn prior_was_unclean(&self) -> bool {
        self.prior_was_unclean
    }

    /// End the session cleanly, removing the marker.
    ///
    /// Consumes `self` so it cannot be called twice, and so the ordinary Drop
    /// does not also fire. A session that is never ended -- because the process
    /// was killed -- leaves the marker, which is the point.
    pub fn end(self) {
        if let Some(path) = &self.marker {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Look for antivirus products on this machine.
///
/// Best effort and deliberately conservative: it names a product only from
/// evidence it is really there. Off Windows it returns nothing, because the
/// low-reputation false positive this exists for is overwhelmingly a Windows
/// phenomenon, and naming an antivirus on a machine that has none would be the
/// crying-wolf this is built to avoid.
pub fn detect() -> Vec<Product> {
    #[cfg(windows)]
    {
        detect_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Known products, and a path whose presence is good evidence of them.
///
/// Directories rather than running processes: a directory check needs no
/// process enumeration, no privileges and no unsafe call, and an installed
/// antivirus is what matters here rather than whether it happens to be running
/// this second.
#[cfg(windows)]
const WINDOWS_PRODUCTS: &[(&str, &str)] = &[
    (
        "Microsoft Defender",
        r"ProgramData\Microsoft\Windows Defender",
    ),
    ("Avast", r"Program Files\Avast Software"),
    ("AVG", r"Program Files\AVG"),
    ("Avira", r"Program Files\Avira"),
    ("Bitdefender", r"Program Files\Bitdefender"),
    ("ESET", r"Program Files\ESET"),
    ("Kaspersky", r"Program Files (x86)\Kaspersky Lab"),
    ("Malwarebytes", r"Program Files\Malwarebytes"),
    ("McAfee", r"Program Files\McAfee"),
    ("Norton", r"Program Files\Norton Security"),
    ("Sophos", r"Program Files\Sophos"),
    ("Trend Micro", r"Program Files\Trend Micro"),
    ("Webroot", r"Program Files\Webroot"),
];

#[cfg(windows)]
fn detect_windows() -> Vec<Product> {
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let mut found = Vec::new();
    for (name, tail) in WINDOWS_PRODUCTS {
        let path = format!("{system_drive}\\{tail}");
        if std::path::Path::new(&path).exists() {
            found.push(Product { name });
        }
    }
    // Defender is present on essentially every modern Windows, so if the more
    // specific products matched, lead with those -- they are the ones a user
    // installed on purpose and the more likely source of a deliberate block.
    found.sort_by_key(|p| p.name == "Microsoft Defender");
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(name: &'static str) -> Product {
        Product { name }
    }

    #[test]
    fn a_clean_exit_says_nothing() {
        assert_eq!(diagnose(false, false, &[product("Defender")]), None);
    }

    #[test]
    fn a_crash_of_our_own_says_nothing_here() {
        // A crash has its own report and its own message; this must not also
        // blame the antivirus for something VeilVoice did to itself.
        assert_eq!(diagnose(true, true, &[product("Defender")]), None);
    }

    #[test]
    fn an_unclean_exit_with_no_antivirus_says_nothing() {
        // Nobody to name, so the notice would be a guess.
        assert_eq!(diagnose(true, false, &[]), None);
    }

    #[test]
    fn an_unclean_exit_with_antivirus_and_no_crash_is_the_one_case() {
        let notice = diagnose(true, false, &[product("Bitdefender")]).unwrap();
        assert_eq!(notice.products, vec![product("Bitdefender")]);
    }

    #[test]
    fn the_message_names_the_product_and_does_not_tell_them_to_disable_it() {
        let notice = Notice {
            products: vec![product("ESET")],
        };
        let message = notice.message();
        assert!(message.contains("ESET"));
        assert!(
            message.contains("only if"),
            "an exclusion is advised only if the problem is real"
        );
        assert!(
            !message.to_lowercase().contains("disable"),
            "it must never tell somebody to turn off their antivirus"
        );
        assert!(
            message.contains("signed") || message.contains("Verify"),
            "it should point at how to check VeilVoice is genuine"
        );
    }

    #[test]
    fn several_products_are_all_named() {
        let notice = Notice {
            products: vec![product("Avast"), product("Malwarebytes")],
        };
        let message = notice.message();
        assert!(message.contains("Avast"));
        assert!(message.contains("Malwarebytes"));
    }

    #[test]
    fn the_marker_sits_beside_the_lock() {
        if let (Some(marker), Some(lock)) = (marker_path(), veilvoice_crypto::lock::default_path())
        {
            assert_eq!(marker.parent(), lock.parent());
        }
    }
}
