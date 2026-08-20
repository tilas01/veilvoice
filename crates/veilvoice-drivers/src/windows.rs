// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows: `driverquery.exe`, which the system already ships.
//!
//! # Why a subprocess
//!
//! The native answer is `EnumDeviceDrivers` or a service-control enumeration,
//! and both are FFI. `#![forbid(unsafe_code)]` holds in this crate as it does
//! everywhere else in the workspace, so this shells out to a tool Windows
//! installs by default — the same trade `veilvoice-watch` makes for the
//! registry and `veilvoice-verify` makes for downloading.
//!
//! `driverquery.exe` is resolved by absolute path under `%SystemRoot%`. Never
//! by bare name: Windows searches the current directory before most of `PATH`,
//! so running from a folder containing a `driverquery.exe` somebody else wrote
//! would run that one instead. This is a security tool asking what is loaded in
//! the kernel; it is a poor place to be relaxed about which program answers.
//!
//! # The format
//!
//! `/FO CSV /NH` gives one quoted record per line:
//!
//! ```text
//! "ACPI","Microsoft ACPI Driver","Kernel ","1/1/1970 12:00:00 AM"
//! ```
//!
//! Module name, display name, driver type, link date. The link date is kept —
//! unlike the Linux load address it is a property of the file rather than of
//! this boot, so it does not change under a machine that has not changed.
//!
//! # What it lists, and what that means
//!
//! Installed drivers, which is a superset of what is loaded right now. A driver
//! appearing here is therefore "something installed a driver", not "something
//! is running in the kernel" — a real distinction, and the front end wording
//! keeps it.

#[cfg(any(target_os = "windows", test))]
use crate::Module;

/// Split one CSV line into its quoted fields.
///
/// Written out rather than pulled in, because the workspace does not carry a
/// CSV crate and this format is four quoted fields with no escaping in
/// practice. A doubled quote inside a field is handled anyway: it costs three
/// lines, and a display name containing one is exactly the sort of thing that
/// would otherwise be found by a user rather than by a test.
#[cfg(any(target_os = "windows", test))]
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

/// Parse `driverquery /FO CSV /NH` output.
///
/// Ungated so the parser is exercised by the test suite on every platform.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn parse_driverquery(text: &str) -> Vec<Module> {
    let mut modules = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = csv_fields(line);
        let name = fields.first().map(|f| f.trim()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        // A header row survives `/NH` on some locales, and looks like a driver
        // called "Module Name". Dropping it by name is crude and correct: no
        // real driver is called that, and the alternative is a permanent
        // phantom entry in every report.
        if name.eq_ignore_ascii_case("Module Name") {
            continue;
        }
        let display = fields.get(1).map(|f| f.trim()).unwrap_or_default();
        let kind = fields.get(2).map(|f| f.trim()).unwrap_or_default();
        let date = fields.get(3).map(|f| f.trim()).unwrap_or_default();

        let mut detail = String::new();
        for part in [display, kind, date] {
            if part.is_empty() {
                continue;
            }
            if !detail.is_empty() {
                detail.push_str(", ");
            }
            detail.push_str(part);
        }
        modules.push(Module {
            name: name.to_string(),
            detail,
        });
    }
    modules
}

/// Run `driverquery` and parse what it says.
#[cfg(target_os = "windows")]
pub(crate) fn read() -> (Vec<Module>, Vec<String>, Vec<String>) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let program = format!(r"{root}\System32\driverquery.exe");

    let mut command = Command::new(&program);
    // Safe API, so this costs nothing against `#![forbid(unsafe_code)]`. It is
    // what stops a console flashing when the desktop application is the
    // caller -- the defect v0.1.10 shipped.
    command.creation_flags(CREATE_NO_WINDOW);
    let output = match command.args(["/FO", "CSV", "/NH"]).output() {
        Ok(output) => output,
        Err(error) => return (Vec::new(), vec![format!("{program}: {error}")], Vec::new()),
    };
    if !output.status.success() {
        let complaint = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return (
            Vec::new(),
            vec![format!("driverquery failed: {complaint}")],
            Vec::new(),
        );
    }
    // Windows console tools emit the system code page, so this is lossy on
    // purpose: a driver whose display name has a character this cannot decode
    // should still be listed under its module name rather than dropped.
    let text = String::from_utf8_lossy(&output.stdout);
    // No cross-view: there is no second list to compare against, and returning
    // an empty vector of discrepancies must be read as "nothing was checked".
    // `support().cross_view` is what says which, and a test asserts they agree.
    (parse_driverquery(&text), Vec::new(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `driverquery /FO CSV /NH` output, kept verbatim.
    const SAMPLE: &str = "\
\"1394ohci\",\"1394 OHCI Compliant Ho\",\"Kernel \",\"12/6/2019 7:56:22 PM\"
\"3ware\",\"3ware\",\"Kernel \",\"5/18/2015 6:28:03 PM\"
\"ACPI\",\"Microsoft ACPI Driver\",\"Kernel \",\"9/9/2022 3:26:32 AM\"
\"AFD\",\"Ancillary Function Driver for Winsock\",\"Kernel \",\"\"
\"vbaudio_cable64_win10\",\"VB-Audio Virtual Cable\",\"Kernel \",\"3/2/2021 1:11:00 PM\"
";

    #[test]
    fn every_record_becomes_a_module() {
        let modules = parse_driverquery(SAMPLE);
        assert_eq!(modules.len(), 5);
        assert_eq!(modules[0].name, "1394ohci");
        assert_eq!(modules[4].name, "vbaudio_cable64_win10");
    }

    #[test]
    fn the_display_name_type_and_date_are_kept() {
        let modules = parse_driverquery(SAMPLE);
        let acpi = modules.iter().find(|m| m.name == "ACPI").unwrap();
        assert!(
            acpi.detail.contains("Microsoft ACPI Driver"),
            "{}",
            acpi.detail
        );
        assert!(acpi.detail.contains("Kernel"), "{}", acpi.detail);
        assert!(acpi.detail.contains("9/9/2022"), "{}", acpi.detail);
    }

    /// An empty trailing field must not leave a dangling separator.
    #[test]
    fn a_missing_date_does_not_leave_a_trailing_comma() {
        let modules = parse_driverquery(SAMPLE);
        let afd = modules.iter().find(|m| m.name == "AFD").unwrap();
        assert!(!afd.detail.ends_with(", "), "{}", afd.detail);
        assert!(!afd.detail.ends_with(','), "{}", afd.detail);
    }

    /// A comma inside a quoted display name must not split the record. This is
    /// the whole reason the field splitter exists.
    #[test]
    fn a_comma_inside_a_name_does_not_split_the_record() {
        let modules =
            parse_driverquery("\"drv\",\"Something, and more\",\"Kernel \",\"1/1/2020\"\n");
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "drv");
        assert!(
            modules[0].detail.contains("Something, and more"),
            "{}",
            modules[0].detail
        );
    }

    #[test]
    fn a_doubled_quote_inside_a_field_survives() {
        let fields = csv_fields("\"a\",\"he said \"\"hello\"\"\",\"c\"");
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[1], "he said \"hello\"");
    }

    /// `/NH` does not suppress the header on every locale, and a phantom
    /// driver called "Module Name" in every report would be permanent.
    #[test]
    fn a_header_row_that_survived_the_flag_is_dropped() {
        let with_header =
            format!("\"Module Name\",\"Display Name\",\"Driver Type\",\"Link Date\"\n{SAMPLE}");
        let modules = parse_driverquery(&with_header);
        assert_eq!(modules.len(), 5);
        assert!(!modules.iter().any(|m| m.name == "Module Name"));
    }

    #[test]
    fn nothing_and_whitespace_produce_nothing() {
        assert!(parse_driverquery("").is_empty());
        assert!(parse_driverquery("\n\n  \n").is_empty());
        assert!(parse_driverquery("\"\",\"nameless\"\n").is_empty());
    }
}
