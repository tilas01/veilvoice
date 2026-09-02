// SPDX-License-Identifier: GPL-3.0-or-later
//! Which program checks the signature, and who decides.
//!
//! VeilVoice can check a release signature three ways, and the difference
//! between them matters more than it looks.
//!
//! 1. **Built in.** The check in this binary, written in Rust, with no
//!    external program involved. It always works, on every platform, with
//!    nothing installed.
//! 2. **A `gpg` on this machine.** GnuPG itself, the reference
//!    implementation, run as a separate program.
//! 3. **A `gpg` inside WSL**, on Windows. The same GnuPG, in a Linux
//!    distribution alongside Windows, reached through `wsl.exe`.
//!
//! # Why the built-in check is not enough on its own, and why it is the default
//!
//! Both things are true at once.
//!
//! The built-in check is the one telling you a download is genuine, and it
//! came out of that download. A tampered release ships a tampered checker.
//! That is not a hypothetical objection to fix by writing better code: no
//! program can vouch for itself, and this one says so on the tab. GnuPG is a
//! second opinion from software this project did not write, and having one is
//! the entire point of running it.
//!
//! And yet nothing here reaches for GnuPG on its own. **An external checker
//! is used only when the person using VeilVoice has chosen it**, even when one
//! was already installed long before VeilVoice first ran. Running another
//! program is a thing a privacy tool should do because it was asked to, not
//! because it found something on `PATH`; on Windows the WSL route starts a
//! whole Linux distribution, which is not a side effect to have by accident.
//!
//! So: the default is the built-in check, the tab says what else is available,
//! and choosing one is one press. That is the honest arrangement. It is not
//! the most convenient one.
//!
//! # What this module does not do
//!
//! It does not run anything to find out what is here. [`Survey`] is a value
//! somebody else fills in, and every decision below is made from it, so the
//! rules can be tested without a `gpg`, without a WSL, and without a Windows
//! machine.

use std::path::PathBuf;

/// What the person using VeilVoice chose.
///
/// `None` is not "decide for me". It is "nothing has been chosen", and it
/// resolves to the built-in check whatever is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The check built into this binary.
    BuiltIn,
    /// A `gpg` on this machine's `PATH`.
    Native,
    /// A `gpg` inside WSL, on Windows.
    Wsl,
}

impl Choice {
    /// The name this is stored under, in settings and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Choice::BuiltIn => "built-in",
            Choice::Native => "native",
            Choice::Wsl => "wsl",
        }
    }

    /// The choice with this name, if it is one.
    pub fn from_key(key: &str) -> Option<Choice> {
        match key.trim().to_ascii_lowercase().as_str() {
            "built-in" | "builtin" | "internal" => Some(Choice::BuiltIn),
            "native" | "gpg" | "system" => Some(Choice::Native),
            "wsl" => Some(Choice::Wsl),
            _ => None,
        }
    }

    /// Every choice, in the order a front end should offer them.
    pub const ALL: &'static [Choice] = &[Choice::BuiltIn, Choice::Native, Choice::Wsl];
}

/// A `gpg` reached through WSL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wsl {
    /// `wsl.exe` itself.
    pub program: PathBuf,
    /// Where `gpg` is inside the distribution, when it has been looked for
    /// and found. `None` means WSL is here and GnuPG inside it is not.
    pub gpg: Option<String>,
}

/// What is on this machine. Filled in by a caller that is willing to look.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    /// A `gpg` on `PATH`, if there is one.
    pub native: Option<PathBuf>,
    /// WSL, if this is Windows and it is installed.
    pub wsl: Option<Wsl>,
}

impl Survey {
    /// Whether a choice could actually be used right now.
    pub fn supports(&self, choice: Choice) -> bool {
        match choice {
            Choice::BuiltIn => true,
            Choice::Native => self.native.is_some(),
            Choice::Wsl => self.wsl.as_ref().is_some_and(|w| w.gpg.is_some()),
        }
    }
}

/// Which checker will actually run, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// The check in this binary.
    BuiltIn {
        /// Why, in words a reader can act on.
        because: Because,
    },
    /// GnuPG on this machine.
    Native(PathBuf),
    /// GnuPG inside WSL.
    Wsl {
        /// `wsl.exe`.
        program: PathBuf,
        /// `gpg` inside the distribution.
        gpg: String,
    },
}

/// Why the built-in check is the one running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Because {
    /// Nothing else has been chosen. The ordinary case, and not a failure.
    NothingChosen,
    /// It was chosen.
    Chosen,
    /// Something else was chosen and is not there any more.
    ChosenIsMissing(Choice),
}

impl Because {
    /// What to tell the reader.
    pub fn plainly(self) -> String {
        match self {
            Because::NothingChosen => "VeilVoice's own check. No GnuPG has been chosen, \
                                       so none is being run: a second opinion is worth \
                                       having and is not something to start on your \
                                       behalf."
                .to_string(),
            Because::Chosen => "VeilVoice's own check, which is what you chose.".to_string(),
            Because::ChosenIsMissing(choice) => format!(
                "VeilVoice's own check. {} was chosen and is not on this machine now, \
                 so the check was made here rather than skipped.",
                match choice {
                    Choice::Native => "A GnuPG on this machine",
                    Choice::Wsl => "A GnuPG inside WSL",
                    Choice::BuiltIn => "The built-in check",
                }
            ),
        }
    }
}

/// The checker to use, from what was chosen and what is here.
///
/// The one rule worth stating on its own: an unset choice gives the built-in
/// check, whatever is installed. A `gpg` that was on this machine before
/// VeilVoice was ever run is still not used until somebody says to use it.
pub fn resolve(choice: Option<Choice>, survey: &Survey) -> Backend {
    let Some(choice) = choice else {
        return Backend::BuiltIn {
            because: Because::NothingChosen,
        };
    };
    match choice {
        Choice::BuiltIn => Backend::BuiltIn {
            because: Because::Chosen,
        },
        Choice::Native => match &survey.native {
            Some(path) => Backend::Native(path.clone()),
            None => Backend::BuiltIn {
                because: Because::ChosenIsMissing(Choice::Native),
            },
        },
        Choice::Wsl => match survey.wsl.as_ref().and_then(|w| {
            w.gpg.as_ref().map(|gpg| (w.program.clone(), gpg.clone()))
        }) {
            Some((program, gpg)) => Backend::Wsl { program, gpg },
            None => Backend::BuiltIn {
                because: Because::ChosenIsMissing(Choice::Wsl),
            },
        },
    }
}

impl Backend {
    /// Whether an OpenPGP implementation other than this one is doing the work.
    ///
    /// The whole value of choosing one is that it is not this program, so this
    /// is the question the tab actually asks.
    pub fn is_second_opinion(&self) -> bool {
        !matches!(self, Backend::BuiltIn { .. })
    }

    /// A one-line description for a reader.
    pub fn plainly(&self) -> String {
        match self {
            Backend::BuiltIn { because } => because.plainly(),
            Backend::Native(path) => format!("GnuPG at {}", path.display()),
            Backend::Wsl { gpg, .. } => format!("GnuPG inside WSL, at {gpg}"),
        }
    }

    /// How a command written for `gpg` is spelled for this backend.
    ///
    /// A WSL command is the same command with `wsl` in front of it, which is
    /// worth showing rather than hiding: it is what the reader would type, and
    /// it makes plain that the check is happening in the Linux distribution
    /// rather than on Windows.
    pub fn spell(&self, command: &str) -> String {
        match self {
            Backend::Wsl { .. } => format!("wsl {command}"),
            _ => command.to_string(),
        }
    }
}

// --- The part that actually looks ------------------------------------------
//
// Everything above decides things from a `Survey`. This fills one in, and it
// is kept apart deliberately: the rules are what matter and they are testable
// without a `gpg`, a WSL or a Windows machine, which is why none of the tests
// below call any of this.

/// What is here, without running anything.
///
/// `PATH` is read and files are looked at. No program is started, including
/// `wsl.exe`: starting WSL boots a Linux distribution, and that is not
/// something to do because a window opened. So `wsl` is reported as present
/// or absent, and whether GnuPG is *inside* it stays unknown until
/// [`look_in_wsl`] is called, which is a thing the reader asks for.
pub fn look() -> Survey {
    Survey {
        native: crate::on_path(),
        wsl: wsl_program().map(|program| Wsl { program, gpg: None }),
    }
}

/// `wsl.exe`, if this is Windows and it is installed.
fn wsl_program() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("wsl.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Ask the WSL distribution where its `gpg` is.
///
/// **This starts WSL**, which starts a Linux distribution, which is why it is
/// a separate call and not part of [`look`]. `command -v` is the shell
/// builtin, so this needs nothing installed to answer that nothing is
/// installed.
pub fn look_in_wsl(program: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(["--", "sh", "-lc", "command -v gpg"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!found.is_empty()).then_some(found)
}

/// The command that installs GnuPG inside the WSL distribution.
///
/// Shown rather than run. It needs root inside that distribution, and a
/// program that asks for somebody's password to install something is a
/// program to be suspicious of; run in a terminal, the reader can see what
/// they are approving. This is the same rule the companion list follows.
pub fn install_in_wsl() -> Vec<String> {
    vec![
        "wsl".to_string(),
        "--".to_string(),
        "sudo".to_string(),
        "apt-get".to_string(),
        "install".to_string(),
        "-y".to_string(),
        "gnupg".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_only() -> Survey {
        Survey {
            native: Some(PathBuf::from("/usr/bin/gpg")),
            wsl: None,
        }
    }

    fn wsl_with_gpg() -> Survey {
        Survey {
            native: None,
            wsl: Some(Wsl {
                program: PathBuf::from("C:\\Windows\\System32\\wsl.exe"),
                gpg: Some("/usr/bin/gpg".to_string()),
            }),
        }
    }

    /// The rule this module exists for.
    #[test]
    fn an_installed_gnupg_is_not_used_until_it_is_chosen() {
        let backend = resolve(None, &native_only());
        assert_eq!(
            backend,
            Backend::BuiltIn {
                because: Because::NothingChosen
            },
            "a gpg on PATH was used without anybody asking for it"
        );
        assert!(!backend.is_second_opinion());
    }

    #[test]
    fn the_same_is_true_of_a_gnupg_inside_wsl() {
        assert_eq!(
            resolve(None, &wsl_with_gpg()),
            Backend::BuiltIn {
                because: Because::NothingChosen
            }
        );
    }

    #[test]
    fn choosing_one_uses_it() {
        assert_eq!(
            resolve(Some(Choice::Native), &native_only()),
            Backend::Native(PathBuf::from("/usr/bin/gpg"))
        );
        assert_eq!(
            resolve(Some(Choice::Wsl), &wsl_with_gpg()),
            Backend::Wsl {
                program: PathBuf::from("C:\\Windows\\System32\\wsl.exe"),
                gpg: "/usr/bin/gpg".to_string(),
            }
        );
    }

    /// A choice that has gone away falls back to a check that works, and says
    /// which one it made. Falling back silently would let somebody believe a
    /// second opinion had been taken when none had.
    #[test]
    fn a_choice_that_is_no_longer_installed_falls_back_and_says_so() {
        let backend = resolve(Some(Choice::Native), &Survey::default());
        assert_eq!(
            backend,
            Backend::BuiltIn {
                because: Because::ChosenIsMissing(Choice::Native)
            }
        );
        assert!(backend.plainly().contains("not on this machine"));

        let backend = resolve(Some(Choice::Wsl), &Survey::default());
        assert_eq!(
            backend,
            Backend::BuiltIn {
                because: Because::ChosenIsMissing(Choice::Wsl)
            }
        );
    }

    /// WSL being installed is not the same as GnuPG being installed inside it.
    #[test]
    fn wsl_without_gnupg_in_it_is_not_a_route() {
        let survey = Survey {
            native: None,
            wsl: Some(Wsl {
                program: PathBuf::from("wsl.exe"),
                gpg: None,
            }),
        };
        assert!(!survey.supports(Choice::Wsl));
        assert_eq!(
            resolve(Some(Choice::Wsl), &survey),
            Backend::BuiltIn {
                because: Because::ChosenIsMissing(Choice::Wsl)
            }
        );
    }

    #[test]
    fn a_wsl_command_is_the_same_command_run_through_wsl() {
        let backend = resolve(Some(Choice::Wsl), &wsl_with_gpg());
        assert_eq!(
            backend.spell("gpg --verify SHA256SUMS.asc SHA256SUMS"),
            "wsl gpg --verify SHA256SUMS.asc SHA256SUMS"
        );
        let backend = resolve(Some(Choice::Native), &native_only());
        assert_eq!(
            backend.spell("gpg --verify SHA256SUMS.asc SHA256SUMS"),
            "gpg --verify SHA256SUMS.asc SHA256SUMS"
        );
    }

    #[test]
    fn every_choice_survives_being_written_down_and_read_back() {
        for choice in Choice::ALL {
            assert_eq!(Choice::from_key(choice.key()), Some(*choice));
        }
        assert_eq!(Choice::from_key("nonsense"), None);
    }

    /// The WSL install command is shown, not run, and it is the command a
    /// person would type.
    #[test]
    fn the_wsl_install_command_is_the_one_a_person_would_type() {
        let argv = install_in_wsl();
        assert_eq!(argv.first().map(String::as_str), Some("wsl"));
        assert!(argv.iter().any(|part| part == "sudo"),
                "installing inside the distribution needs root there");
        assert!(argv.iter().any(|part| part == "gnupg"));
    }

    /// Looking must not start anything. The check is on the source, because
    /// the failure would be a Linux distribution booting when a window opened
    /// and no assertion about a return value would catch that.
    #[test]
    fn looking_does_not_run_a_program() {
        let source = include_str!("backend.rs");
        let looking = source
            .split("pub fn look() -> Survey {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("look() exists");
        assert!(
            !looking.contains("Command::new"),
            "look() starts a program; starting wsl.exe boots a Linux \
             distribution, which belongs behind look_in_wsl where somebody \
             asked for it"
        );
    }

    /// The built-in check is always available. It is the only one that can be
    /// promised, because it is the only one that is part of this program.
    #[test]
    fn the_built_in_check_is_always_available() {
        assert!(Survey::default().supports(Choice::BuiltIn));
        assert!(!Survey::default().supports(Choice::Native));
        assert!(!Survey::default().supports(Choice::Wsl));
    }
}
