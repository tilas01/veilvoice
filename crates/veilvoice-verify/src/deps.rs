// SPDX-License-Identifier: GPL-3.0-or-later
//! What this machine needs before it can build VeilVoice, and who ships it.
//!
//! # The rule, which predates this module
//!
//! Installing a build dependency means running somebody else's package
//! manager, as root, on somebody else's machine. The companion setup already
//! makes that trade and it gets the same rule here: **detect what is there,
//! say what each thing is and who ships it, and install only on an explicit
//! yes.** Never silently, never ticked by default, and never as a side effect
//! of asking a question.
//!
//! What it will not do is add a network client to VeilVoice. It shells out to
//! the tool the platform already has, exactly as the verifier does for
//! downloads, so the claim that this project's dependency graph contains no
//! HTTP client is unchanged and still checkable with `cargo tree`.
//!
//! # Why there is a table at all
//!
//! Almost all of VeilVoice is pure Rust and needs nothing but a compiler. The
//! exceptions are real and they are the reason a build fails on a fresh
//! machine with a message about a missing header rather than about a missing
//! package:
//!
//! * **Linux** — `cpal` reaches ALSA through `alsa-sys`, which is a `-sys`
//!   crate: it compiles against ALSA's C headers and asks `pkg-config` where
//!   they are. Neither ships with a base install of most distributions.
//! * **macOS** — CoreAudio comes from Apple's SDK, which arrives with the
//!   Xcode command line tools. Apple's licence does not permit redistributing
//!   it, which is also why this tool cannot build a macOS binary anywhere else.
//! * **Windows** — the MSVC toolchain needs a linker, which comes with the
//!   Visual Studio build tools.
//!
//! Everything else -- the engine, the container format, the app lock, the
//! website generators -- has no system dependency at all.
//!
//! # What "detected" means, and what it does not
//!
//! [`Need::detect`] answers from what is on `PATH` and from `pkg-config`. That
//! is a real answer for a linker or a compiler and a *good* answer for a
//! library, but it is not a build. A machine can pass every probe here and
//! still fail to compile, and this module says so rather than promising
//! otherwise: the build in marker 55 is the only thing that actually knows.
//!
//! # In plain words
//!
//! Before you can build this program yourself, your computer needs a few
//! pieces: a Rust compiler, and -- on Linux and macOS -- one or two things
//! from your operating system that VeilVoice's sound handling is built on top
//! of. This works out which of those you already have and which you do not,
//! tells you exactly what each one is and who makes it, and offers to install
//! the missing ones **only if you say yes**.
//!
//! It will never install anything on its own, and it does not download
//! anything itself -- it asks the software installer your system already came
//! with, so you can see exactly what is being run.

use std::process::Command;

/// Whether a dependency is here, missing, or unanswerable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Found, with whatever the probe could say about it.
    Present(String),
    /// Looked for and not found.
    Missing,
    /// Not needed on this operating system.
    NotOnThisPlatform,
    /// The probe itself could not run.
    ///
    /// Deliberately not [`Presence::Missing`]. "I looked and it is not there"
    /// and "I could not look" lead to different actions, and reporting the
    /// second as the first is how a tool offers to install something that is
    /// already installed.
    Unknown(String),
}

impl Presence {
    /// Whether this counts as satisfied.
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Self::Present(_) | Self::NotOnThisPlatform)
    }

    /// One line, for a report.
    pub fn describe(&self) -> String {
        match self {
            Self::Present(detail) if detail.is_empty() => "found".to_string(),
            Self::Present(detail) => format!("found -- {detail}"),
            Self::Missing => "MISSING".to_string(),
            Self::NotOnThisPlatform => "not needed on this platform".to_string(),
            Self::Unknown(why) => format!("could not tell -- {why}"),
        }
    }
}

/// What could be done about a missing dependency, on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// A command that would install it. **Never run without an explicit yes.**
    Run {
        /// The program.
        program: String,
        /// Its arguments.
        args: Vec<String>,
        /// Who ships what this installs.
        vendor: &'static str,
    },
    /// Something a person has to do themselves, with the reason.
    ///
    /// Downloading Apple's or Microsoft's tooling means accepting a licence,
    /// and a licence has to be accepted by the person it binds rather than by
    /// a program acting on their behalf.
    Yourself(String),
    /// Not needed here.
    NotOnThisPlatform,
    /// Nothing is known. Said rather than guessed.
    Unknown(String),
}

impl Route {
    /// The command line, for showing before asking.
    ///
    /// Shown in full, every time, before the question. A yes to a command
    /// nobody read is not consent to anything.
    pub fn command_line(&self) -> Option<String> {
        match self {
            Self::Run { program, args, .. } => Some(format!("{program} {}", args.join(" "))),
            _ => None,
        }
    }
}

/// One thing a build needs.
#[derive(Clone, Copy, Debug)]
pub struct Need {
    /// Stable identifier.
    pub key: &'static str,
    /// What it is called.
    pub name: &'static str,
    /// What it is, for somebody who has not met it.
    pub what: &'static str,
    /// Which part of VeilVoice needs it, and why.
    pub why: &'static str,
    /// Whether the build fails without it, or only a feature.
    pub required: bool,
}

/// Everything a build can need, on every platform.
///
/// Kept whole rather than compiled down to this platform's subset:
/// [`for_this_platform`] filters, and somebody reading the source should be
/// able to see what a build needs elsewhere without owning that machine.
pub const ALL: &[Need] = &[
    Need {
        key: "rust",
        name: "The Rust toolchain",
        what: "The compiler and `cargo`, at the version pinned in \
               `rust-toolchain.toml`.",
        why: "Everything. The pinned version is not a preference: a fixed \
              compiler is a prerequisite for a build that comes out the same \
              bytes twice.",
        required: true,
    },
    Need {
        key: "cc",
        name: "A C compiler and linker",
        what: "On Linux, `cc` from GCC or Clang. On Windows, the Visual Studio \
               build tools. On macOS, the Xcode command line tools.",
        why: "Rust does not ship a linker. Every Rust program on every platform \
              needs the system one.",
        required: true,
    },
    Need {
        key: "pkg-config",
        name: "pkg-config",
        what: "The tool a build script asks where a system library's headers \
               are.",
        why: "`alsa-sys` uses it to find ALSA. Without it the build stops with \
              a message about ALSA rather than about pkg-config, which is why \
              it is listed separately.",
        required: true,
    },
    Need {
        key: "alsa",
        name: "ALSA development headers",
        what: "The header files for Linux's sound layer -- `libasound2-dev` on \
               Debian and Ubuntu, `alsa-lib-devel` on Fedora, `alsa-lib` on \
               Arch.",
        why: "Live capture and playback go through `cpal`, which reaches ALSA \
              through a `-sys` crate that compiles against these headers. \
              Everything that is not live mode builds without them.",
        required: false,
    },
];

impl Need {
    /// Look for it. Changes nothing.
    pub fn detect(&self) -> Presence {
        match self.key {
            "rust" => match program_version("cargo", &["--version"]) {
                Some(line) => Presence::Present(line),
                None => Presence::Missing,
            },
            "cc" => detect_linker(),
            "pkg-config" => {
                if !cfg!(target_os = "linux") {
                    return Presence::NotOnThisPlatform;
                }
                match program_version("pkg-config", &["--version"]) {
                    Some(line) => Presence::Present(format!("version {line}")),
                    None => Presence::Missing,
                }
            }
            "alsa" => detect_alsa(),
            other => Presence::Unknown(format!("no probe is written for {other}")),
        }
    }

    /// What could be done about it here.
    pub fn route(&self) -> Route {
        match self.key {
            // rustup's own installer, and not run by this program. It writes to
            // the home directory, alters the shell profile and downloads a
            // toolchain, and somebody should type that themselves knowing all
            // three.
            "rust" => Route::Yourself(
                "Install it from https://rustup.rs, which is how the Rust project \
                 ships it. This program does not run that installer for you: it \
                 downloads a compiler, writes to your home directory and edits \
                 your shell profile, and all three are yours to agree to."
                    .to_string(),
            ),

            "cc" => {
                if cfg!(target_os = "linux") {
                    linux_package("build-essential", "gcc", "base-devel")
                } else if cfg!(target_os = "macos") {
                    Route::Run {
                        program: "xcode-select".to_string(),
                        args: vec!["--install".to_string()],
                        vendor: "Apple",
                    }
                } else if cfg!(windows) {
                    Route::Yourself(
                        "Install the Visual Studio Build Tools from Microsoft, with \
                         the \"Desktop development with C++\" workload. It is \
                         several gigabytes and it carries Microsoft's licence, which \
                         is yours to accept rather than this program's."
                            .to_string(),
                    )
                } else {
                    Route::Unknown(
                        "no route is written for this operating system; install a C \
                         compiler the way you install anything else here"
                            .to_string(),
                    )
                }
            }

            "pkg-config" => {
                if cfg!(target_os = "linux") {
                    linux_package("pkg-config", "pkgconf-pkg-config", "pkgconf")
                } else {
                    Route::NotOnThisPlatform
                }
            }

            "alsa" => {
                if cfg!(target_os = "linux") {
                    linux_package("libasound2-dev", "alsa-lib-devel", "alsa-lib")
                } else {
                    Route::NotOnThisPlatform
                }
            }

            other => Route::Unknown(format!("no route is written for {other}")),
        }
    }
}

/// What this platform needs, in the order to report them.
pub fn for_this_platform() -> Vec<&'static Need> {
    ALL.iter()
        .filter(|need| need.detect() != Presence::NotOnThisPlatform)
        .collect()
}

/// A package under the three names the major families give it.
///
/// The package manager on `PATH` decides which name is used. A machine with
/// none of them gets [`Route::Yourself`] with all three names in it, because a
/// reader on a distribution nobody here has heard of can still translate.
fn linux_package(debian: &'static str, fedora: &'static str, arch: &'static str) -> Route {
    for (manager, vendor, args) in [
        (
            "apt-get",
            "Debian, Ubuntu and derivatives",
            vec!["install", debian],
        ),
        ("dnf", "Fedora and Red Hat", vec!["install", fedora]),
        ("pacman", "Arch and derivatives", vec!["-S", arch]),
        ("zypper", "openSUSE", vec!["install", fedora]),
        ("apk", "Alpine", vec!["add", debian]),
    ] {
        if which(manager).is_some() {
            return Route::Run {
                program: "sudo".to_string(),
                args: std::iter::once(manager.to_string())
                    .chain(args.into_iter().map(str::to_string))
                    .collect(),
                vendor,
            };
        }
    }
    Route::Yourself(format!(
        "No package manager this program recognises is on PATH. The package is \
         called {debian} on Debian and Ubuntu, {fedora} on Fedora, and {arch} on \
         Arch; install the equivalent for your system."
    ))
}

/// Whether a program is on `PATH`, and where.
///
/// Asks the system's own resolver rather than walking `PATH` here: the rules
/// differ per platform (`PATHEXT` on Windows, for one) and reimplementing them
/// is how a probe reports something as missing that is sitting right there.
fn which(program: &str) -> Option<String> {
    let (finder, args) = if cfg!(windows) {
        ("where", vec![program])
    } else {
        ("command", vec!["-v", program])
    };
    // `command -v` is a shell builtin, so it needs a shell.
    let output = if cfg!(windows) {
        Command::new(finder).args(&args).output().ok()?
    } else {
        Command::new("sh")
            .args(["-c", &format!("command -v {program}")])
            .output()
            .ok()?
    };
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_string())
}

/// The first line a program prints when asked its version.
fn program_version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim().to_string();
    Some(line)
}

/// A linker, by whichever name this platform calls it.
fn detect_linker() -> Presence {
    if cfg!(windows) {
        // F-68. This looked for `link` on PATH and reported whatever it found.
        // On the machine it was first run on that was Git for Windows'
        // `usr/bin/link.exe` -- GNU coreutils' hardlink utility, which shares a
        // name with Microsoft's linker and has nothing whatever to do with
        // building Rust. The report said the linker was present. A build on
        // that machine would have stopped with a linker error.
        //
        // There is no honest probe here. `link.exe` is only on PATH inside a
        // Developer Command Prompt, cargo finds MSVC through the registry
        // instead, and any `link` that *is* on PATH is more likely to be
        // something else. So this says it cannot tell, which is true, and the
        // build says the rest.
        return Presence::Unknown(
            "Rust finds Microsoft's linker through the registry rather than PATH, \
             so there is nothing here to look at. A `link.exe` on PATH is usually \
             something else -- Git for Windows ships one. Only a build can say."
                .to_string(),
        );
    }
    for name in ["cc", "clang", "gcc"] {
        if let Some(path) = which(name) {
            return Presence::Present(path);
        }
    }
    Presence::Missing
}

/// ALSA's headers, through the tool the build script itself uses.
fn detect_alsa() -> Presence {
    if !cfg!(target_os = "linux") {
        return Presence::NotOnThisPlatform;
    }
    if which("pkg-config").is_none() {
        return Presence::Unknown(
            "pkg-config is not installed, so there is no way to ask where ALSA is".to_string(),
        );
    }
    // Exactly what `alsa-sys`'s build script asks, so the answer here and the
    // answer during a build come from the same source.
    match Command::new("pkg-config")
        .args(["--modversion", "alsa"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Presence::Present(format!("ALSA {version}"))
        }
        Ok(_) => Presence::Missing,
        Err(error) => Presence::Unknown(format!("pkg-config would not run: {error}")),
    }
}

/// What is missing, split by whether a build stops without it.
///
/// Returned rather than printed so the caller decides how loudly to say it --
/// and so the two are never conflated. A missing optional dependency means a
/// build that succeeds with less in it, and reporting that as a failure would
/// send somebody installing things they do not need.
pub fn missing() -> (Vec<&'static Need>, Vec<&'static Need>) {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for need in for_this_platform() {
        // `Unknown` is not `Missing`. A probe that could not run is not
        // evidence of absence, and offering to install over the top of
        // something already there is the mistake that distinction prevents.
        if let Presence::Missing = need.detect() {
            if need.required {
                required.push(need);
            } else {
                optional.push(need);
            }
        }
    }
    (required, optional)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one with this identifier. Here rather than in the module above,
    /// because the tests are the only thing that looks a need up by name --
    /// everything else walks the whole list.
    fn by_key(key: &str) -> Option<&'static Need> {
        ALL.iter().find(|need| need.key == key)
    }

    #[test]
    fn every_entry_is_complete_and_uniquely_keyed() {
        let mut keys: Vec<&str> = ALL.iter().map(|need| need.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two needs share a key");

        for need in ALL {
            assert!(!need.name.is_empty(), "{}", need.key);
            assert!(!need.what.is_empty(), "{}", need.key);
            assert!(!need.why.is_empty(), "{}", need.key);
            assert_eq!(by_key(need.key).map(|n| n.key), Some(need.key));
        }
        assert!(by_key("nothing-like-this").is_none());
    }

    /// Every need says *why* VeilVoice wants it. A list of packages with no
    /// reasons is a list somebody installs without reading, which is the exact
    /// habit this table exists to avoid feeding.
    #[test]
    fn every_need_explains_itself_rather_than_just_naming_a_package() {
        for need in ALL {
            assert!(
                need.why.len() > 40,
                "{}: the reason is too short to be a reason",
                need.key
            );
            assert!(
                need.what.len() > 30,
                "{}: say what it actually is",
                need.key
            );
        }
    }

    /// Detection must not panic, hang, or change anything, on any machine.
    #[test]
    fn looking_is_safe_wherever_this_runs() {
        for need in ALL {
            let presence = need.detect();
            assert!(!presence.describe().is_empty(), "{}", need.key);
            let route = need.route();
            match &route {
                Route::Run { program, args, .. } => {
                    assert!(!program.is_empty());
                    assert!(!args.is_empty());
                    assert!(route.command_line().is_some());
                }
                Route::Yourself(words) => assert!(words.len() > 20, "{}", need.key),
                Route::NotOnThisPlatform | Route::Unknown(_) => {}
            }
        }
    }

    /// A compiler is certainly here: this test is being compiled by one.
    #[test]
    fn the_toolchain_is_found_because_it_is_running_this_test() {
        let rust = by_key("rust").unwrap();
        match rust.detect() {
            Presence::Present(line) => assert!(line.contains("cargo"), "{line}"),
            // `cargo test` can be invoked where `cargo` itself is not on PATH.
            // That is a real answer and not a reason to fail somebody's build.
            other => panic!("cargo should be findable from a cargo test: {other:?}"),
        }
    }

    /// F-68. A probe must not answer from a program that merely shares a name
    /// with the one it is looking for.
    ///
    /// This reported "found" on a Windows machine because Git for Windows ships
    /// `usr/bin/link.exe`, GNU coreutils' hardlink tool. A build on that machine
    /// would have stopped with a linker error after the dependency check had
    /// said everything was fine.
    #[test]
    fn the_windows_linker_is_not_looked_for_by_name_on_path() {
        let source = include_str!("deps.rs");
        let start = source.find("fn detect_linker()").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        assert!(
            !body.contains(concat!("which(", '"', "link", '"', ")")),
            "a `link` on PATH is usually not Microsoft's linker"
        );

        if cfg!(windows) {
            let presence = by_key("cc").unwrap().detect();
            assert!(
                matches!(presence, Presence::Unknown(_)),
                "on Windows only a build can decide: {presence:?}"
            );
            assert!(
                presence.describe().contains("Only a build can say"),
                "{}",
                presence.describe()
            );
        }
    }

    /// Nothing in this module runs a package manager. The routes are values;
    /// something else has to decide to run one, after asking.
    #[test]
    fn no_route_is_ever_taken_by_this_module() {
        let source = include_str!("deps.rs");
        let body = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [".status()", ".spawn()"] {
            assert!(
                !body.contains(forbidden),
                "this module only looks; {forbidden} runs something"
            );
        }
        // `.output()` is used, and only to ask a program its version. Every
        // use is named here so a new one has to be argued for.
        assert_eq!(
            body.matches(".output()").count(),
            4,
            "a new subprocess appeared in a module whose job is to look"
        );
    }

    /// An unanswerable probe must never be reported as an absence.
    #[test]
    fn could_not_tell_is_not_the_same_as_not_there() {
        assert!(!Presence::Missing.is_satisfied());
        assert!(!Presence::Unknown("no probe".into()).is_satisfied());
        assert!(Presence::NotOnThisPlatform.is_satisfied());
        assert!(Presence::Present(String::new()).is_satisfied());

        assert!(Presence::Unknown("x".into())
            .describe()
            .contains("could not tell"));
        assert_eq!(Presence::Missing.describe(), "MISSING");

        // And the split that acts on it: only a definite Missing counts.
        let (required, optional) = missing();
        for need in required.iter().chain(optional.iter()) {
            assert_eq!(need.detect(), Presence::Missing, "{}", need.key);
        }
    }

    /// Optional and required are kept apart, or somebody installs ALSA headers
    /// on a machine that will never run live mode.
    #[test]
    fn a_missing_optional_dependency_is_not_a_failed_build() {
        let alsa = by_key("alsa").unwrap();
        assert!(!alsa.required, "everything but live mode builds without it");
        assert!(alsa.why.contains("not live mode"), "{}", alsa.why);

        for key in ["rust", "cc"] {
            assert!(by_key(key).unwrap().required, "{key}");
        }
    }

    /// The Linux package name is given for all three families whatever happens,
    /// so a reader on a fourth can translate.
    #[test]
    fn a_machine_with_no_known_package_manager_still_gets_the_names() {
        // Exercised directly rather than through `route`, which depends on what
        // is installed on the machine running the test.
        let route = linux_package("libasound2-dev", "alsa-lib-devel", "alsa-lib");
        match route {
            Route::Run {
                program,
                args,
                vendor,
            } => {
                // A manager was found. It must be run through `sudo` and name
                // the package, and it must say whose packaging it is.
                assert_eq!(program, "sudo");
                assert!(args.len() >= 2, "{args:?}");
                assert!(!vendor.is_empty());
            }
            Route::Yourself(words) => {
                for name in ["libasound2-dev", "alsa-lib-devel", "alsa-lib"] {
                    assert!(words.contains(name), "{words}");
                }
            }
            other => panic!("a Linux package should not be {other:?}"),
        }
    }

    /// Nothing here downloads anything itself.
    #[test]
    fn nothing_in_this_module_speaks_http() {
        let source = include_str!("deps.rs");
        for word in ["http://", "https://reqwest", "TcpStream"] {
            assert!(
                !source.split("//!").last().unwrap_or("").contains(word),
                "{word} in a module that is supposed to shell out"
            );
        }
    }
}
