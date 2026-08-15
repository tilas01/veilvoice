// SPDX-License-Identifier: GPL-3.0-or-later
//! Tokyo Night colouring for the terminal.
//!
//! The same palette the GUI uses, so the two halves of VeilVoice look like one
//! program. Colour is suppressed when the output is not a terminal, when
//! `NO_COLOR` is set (the widely-honoured convention), or when `TERM=dumb`, so
//! piping to a file or a log never produces escape-code soup.

use std::io::IsTerminal;
use std::sync::OnceLock;

/// Tokyo Night, as 24-bit foreground escape sequences.
///
/// The whole palette is defined even though a given build may not use every
/// entry — the device listing is behind the `live` feature, so its colour goes
/// unused on platforms without an audio backend. Keeping the set complete means
/// it stays a straight mirror of the GUI's palette and of `css/themes.css`,
/// which is what makes the three front-ends look like one program.
#[allow(dead_code)]
pub mod colour {
    /// Muted comment grey — secondary text.
    pub const MUTED: &str = "\x1b[38;2;86;95;137m";
    /// Foreground blue — headings and prompts.
    pub const BLUE: &str = "\x1b[38;2;122;162;247m";
    /// Cyan — values and figures.
    pub const CYAN: &str = "\x1b[38;2;125;207;255m";
    /// Green — success.
    pub const GREEN: &str = "\x1b[38;2;158;206;106m";
    /// Yellow — warnings.
    pub const YELLOW: &str = "\x1b[38;2;224;175;104m";
    /// Red — errors.
    pub const RED: &str = "\x1b[38;2;247;118;142m";
    /// Purple — accents.
    pub const PURPLE: &str = "\x1b[38;2;187;154;247m";
    /// Reset to the terminal default.
    pub const RESET: &str = "\x1b[0m";
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        if std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
            return false;
        }
        std::io::stdout().is_terminal()
    })
}

/// Wrap `text` in `colour`, or return it unchanged when colour is off.
pub fn paint(colour: &str, text: &str) -> String {
    if enabled() {
        format!("{colour}{text}{}", colour::RESET)
    } else {
        text.to_string()
    }
}

/// A success line.
pub fn ok(text: &str) -> String {
    format!("{} {}", paint(colour::GREEN, "✓"), text)
}

/// A warning line.
pub fn warn(text: &str) -> String {
    format!("{} {}", paint(colour::YELLOW, "!"), text)
}

/// An error line.
pub fn err(text: &str) -> String {
    format!("{} {}", paint(colour::RED, "✗"), text)
}

/// A section heading.
pub fn heading(text: &str) -> String {
    paint(colour::BLUE, text)
}

/// A `label: value` line with the value highlighted.
pub fn field(label: &str, value: &str) -> String {
    format!(
        "  {:<22} {}",
        paint(colour::MUTED, label),
        paint(colour::CYAN, value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests capture stdout, so colour is off and `paint` must be a no-op —
    /// which is exactly the property that keeps escape codes out of pipes.
    #[test]
    fn colour_is_disabled_when_not_a_terminal() {
        assert_eq!(paint(colour::RED, "hello"), "hello");
    }

    #[test]
    fn helpers_include_their_text() {
        assert!(ok("done").contains("done"));
        assert!(warn("careful").contains("careful"));
        assert!(err("broken").contains("broken"));
        assert!(heading("Section").contains("Section"));
    }

    #[test]
    fn field_shows_both_halves() {
        let line = field("Sample rate", "48000 Hz");
        assert!(line.contains("Sample rate"));
        assert!(line.contains("48000 Hz"));
    }
}
