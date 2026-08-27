// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS: `kmutil showloaded`, falling back to `kextstat`.
//!
//! # Two tools, because Apple is mid-transition
//!
//! `kextstat` is the old one and is deprecated; `kmutil` is the replacement and
//! does not exist on older systems. Both are tried, newest first, and if
//! neither answers the reason is reported rather than an empty list being
//! passed off as "no kernel extensions" — which would be false on every Mac
//! ever made.
//!
//! Both print the same shape, which is why one parser handles both:
//!
//! ```text
//! Index Refs Address            Size       Wired      Name (Version) UUID <Linked Against>
//!     1   88 0                  0          0          com.apple.kpi.bsd (20.6.0)
//!   134    0 0xffffff7f8312d000 0x9000     0x9000     com.example.driver (1.2.3) <7 5 4 1>
//! ```
//!
//! The **address is deliberately dropped**, for the same reason as on Linux: it
//! is zeroed for an unprivileged reader on a machine with kernel-pointer
//! restriction, and changes at every boot on one without. Recording it would
//! make every extension look altered after a restart, and a report that is
//! entirely false positives after every reboot is a report nobody opens twice.
//!
//! # Why a subprocess
//!
//! The native answer is IOKit, which is FFI. `#![forbid(unsafe_code)]` holds
//! here as everywhere else in the workspace, so this asks a tool the system
//! already ships — the same trade the Windows and Linux readers make.
//!
//! # In plain words
//!
//! Asks macOS which system extensions are loaded.
//!
//! There are two tools depending on how new the machine is, because Apple is part
//! way through replacing one with the other. The newer one is tried first and the
//! older is the fallback, so this works on both without being told which you have.

#[cfg(any(target_os = "macos", test))]
use crate::Module;

/// Parse `kextstat`-style output, which `kmutil showloaded` also produces.
///
/// Ungated so the parser is exercised by the test suite on every platform.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn parse_kextstat(text: &str) -> Vec<Module> {
    let mut modules = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // The header. Dropped by its first word rather than by line number,
        // because `kmutil` prints a banner above it on some systems and
        // counting lines would then eat a real extension.
        if trimmed.starts_with("Index") {
            continue;
        }
        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        // index, refs, address, size, wired, name -- six before the name, and
        // anything shorter is a line this does not understand.
        if fields.len() < 6 {
            continue;
        }
        let refs = fields[1];
        let size = fields[3];
        let name = fields[5];
        if name.is_empty() {
            continue;
        }
        // The version follows the name in brackets. Kept: unlike the address
        // it is a property of the extension rather than of this boot, so a
        // machine that has not changed reports no change.
        let version = fields
            .get(6)
            .map(|field| field.trim_matches(['(', ')'].as_ref()))
            .filter(|field| !field.is_empty() && !field.starts_with('<'))
            .unwrap_or("");

        let mut detail = format!("{size} bytes, {refs} refs");
        if !version.is_empty() {
            detail.push_str(&format!(", version {version}"));
        }
        modules.push(Module {
            name: name.to_string(),
            detail,
        });
    }
    modules
}

/// Ask `kmutil`, then `kextstat`.
#[cfg(target_os = "macos")]
pub(crate) fn read() -> (Vec<Module>, Vec<String>, Vec<String>) {
    let attempts: [(&str, &[&str]); 2] = [
        ("/usr/bin/kmutil", &["showloaded", "--list-only"]),
        ("/usr/sbin/kextstat", &["-l"]),
    ];
    let mut problems = Vec::new();
    for (program, arguments) in attempts {
        match std::process::Command::new(program).args(arguments).output() {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let modules = parse_kextstat(&text);
                if !modules.is_empty() {
                    // No cross-view on this platform: there is no second list
                    // to compare against, and an empty discrepancy vector must
                    // be read as "nothing was checked". `support().cross_view`
                    // says which, and a test asserts they agree.
                    return (modules, problems, Vec::new());
                }
                problems.push(format!("{program} answered, and listed nothing"));
            }
            Ok(output) => problems.push(format!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) => problems.push(format!("{program}: {error}")),
        }
    }
    (Vec::new(), problems, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `kextstat -l` output, kept verbatim.
    const SAMPLE: &str = "\
Index Refs Address            Size       Wired      Name (Version) UUID <Linked Against>
    1   88 0                  0          0          com.apple.kpi.bsd (20.6.0) 1B7C1F42
    2   10 0                  0          0          com.apple.kpi.dsep (20.6.0) 8A44B7C1
  134    0 0xffffff7f8312d000 0x9000     0x9000     com.example.driver (1.2.3) 2C1D <7 5 4 1>
";

    #[test]
    fn every_extension_becomes_a_module() {
        let modules = parse_kextstat(SAMPLE);
        assert_eq!(modules.len(), 3);
        assert_eq!(modules[0].name, "com.apple.kpi.bsd");
        assert_eq!(modules[2].name, "com.example.driver");
    }

    #[test]
    fn the_header_is_dropped() {
        assert!(!parse_kextstat(SAMPLE)
            .iter()
            .any(|module| module.name == "Address"));
    }

    /// A banner above the header must not push a real extension out.
    #[test]
    fn a_banner_above_the_header_does_not_eat_an_extension() {
        let with_banner = format!("Kernel extensions currently loaded:\n{SAMPLE}");
        assert_eq!(parse_kextstat(&with_banner).len(), 3);
    }

    #[test]
    fn size_reference_count_and_version_are_kept() {
        let modules = parse_kextstat(SAMPLE);
        let example = modules
            .iter()
            .find(|module| module.name == "com.example.driver")
            .unwrap();
        assert!(
            example.detail.contains("0x9000 bytes"),
            "{}",
            example.detail
        );
        assert!(example.detail.contains("0 refs"), "{}", example.detail);
        assert!(
            example.detail.contains("version 1.2.3"),
            "{}",
            example.detail
        );
    }

    /// The address changes at every boot on a machine that shows it, and is
    /// zeroed on one that does not. Recording it would report every extension
    /// as altered after a restart.
    #[test]
    fn the_load_address_is_not_recorded() {
        for module in parse_kextstat(SAMPLE) {
            assert!(
                !module.detail.contains("0xffffff7f"),
                "the address leaked into {}: {}",
                module.name,
                module.detail
            );
        }
    }

    #[test]
    fn nothing_and_a_short_line_are_survived() {
        assert!(parse_kextstat("").is_empty());
        assert!(parse_kextstat("\n\n  \n").is_empty());
        assert!(parse_kextstat("1 2 3\n").is_empty(), "too few fields");
    }

    /// An extension with no version in brackets must still be listed, with no
    /// dangling "version " on the end of its detail.
    #[test]
    fn a_missing_version_leaves_no_dangling_label() {
        let modules = parse_kextstat("  5   0 0 0x100 0x100 com.example.plain\n");
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "com.example.plain");
        assert!(
            !modules[0].detail.contains("version"),
            "{}",
            modules[0].detail
        );
    }
}
