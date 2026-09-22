// SPDX-License-Identifier: GPL-3.0-or-later
//! What this machine answers, so the settings it starts on were measured here.
//!
//! # The difference this draws
//!
//! A default is a decision taken on behalf of somebody who has not been asked.
//! There are two ways to take one. It can be written down, which means it was
//! decided once by somebody who has never seen the machine it will run on, or
//! it can be measured, which means the machine was asked and answered.
//!
//! Most of VeilVoice's starting values are already the second kind and were
//! not described that way. The frame rate is `0`, which means the display's
//! own rate, and [`crate::pace`] measures it from the intervals between frames
//! rather than assuming sixty. Whether anything animates is resolved every
//! frame against the operating system's own reduce-motion setting, which
//! [`crate::reduced_motion`] reads from the platform. Neither of those is a
//! number somebody chose.
//!
//! The drawing path was the one that was still written down: hardware
//! acceleration was asked for on every machine, because asking is the safe
//! direction and a machine that cannot give a hardware context is given a
//! software one instead. That is true and it is not free. On a machine whose
//! only graphics device is a software renderer, the request is made, refused
//! and fallen back from on every single launch, and the answer was never in
//! doubt: there is no hardware there to ask for.
//!
//! So this module asks.
//!
//! # What it asks, and what it costs
//!
//! One question, through [`veilvoice_video::accel::look`], which lists the
//! graphics devices this system reports: `lspci` on Linux, the management
//! interface on Windows, the system profiler on macOS. That is a subprocess,
//! and a subprocess on the path that decides how the window is made would be a
//! poor trade if it ran every launch.
//!
//! It does not. The answer is wanted exactly once, on the run that has no
//! settings file to read, and it is written into that file with everything
//! else. Every later launch reads it back. Within a single process it is
//! cached in a [`OnceLock`] as well, because the window is created in `main`
//! before [`crate::settings::Settings::load`] runs and both want the answer.
//!
//! # It decides where to start, and never what is allowed
//!
//! **Roadmap item 170.** Every value here stays a setting. The probe moves the
//! point a first run begins from; it removes nothing, and the Settings tab and
//! the first-run card both show what was found and let it be overridden. A
//! measurement that could not be taken says so and falls back to asking for
//! hardware, which is what every build before this one did on every machine.
//!
//! # Finding a device is not proof it works
//!
//! The same caveat [`veilvoice_video::accel::Adapter::caveat`] states about
//! encoders applies here, in one direction only. A machine that lists a real
//! graphics card may still have a driver that accepts a hardware context and
//! then draws badly, which from inside the process looks exactly like success
//! and which nothing here can detect. That is why the setting exists and why
//! the first-run card names the symptom. What the probe can establish is the
//! other direction: a machine that lists *no* hardware at all will not produce
//! any, and asking it to is a request that can only be refused.
//!
//! # In plain words
//!
//! The first time you open VeilVoice it looks at what your computer has,
//! instead of assuming. If your machine has no graphics hardware, it does not
//! spend every launch asking for some. It writes down what it found, so it
//! only has to look once, and you can change any of it afterwards.

use std::sync::OnceLock;

/// What was asked of this machine, and what it said.
///
/// Built once by [`look`]. Every field records both the answer and the
/// sentence explaining it, because a starting value nobody can see the reason
/// for is a written-down default wearing a measurement's clothes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    /// Whether to ask the graphics driver for a hardware context.
    pub acceleration: bool,
    /// Why, in the words the first-run card and the Settings tab show.
    pub acceleration_reason: String,
    /// Whether anything could be established about the graphics devices.
    ///
    /// False when the platform has no route written for it, or the tool that
    /// lists devices is missing or refused. The window is drawn the same way
    /// either way; this is what stops the interface claiming a measurement it
    /// did not take.
    pub graphics_answered: bool,
}

/// Names a graphics device has when it is a renderer running on the processor.
///
/// Matched case-insensitively against the device name the system reported, as
/// a substring, because every one of these appears inside a longer string:
/// Mesa reports `llvmpipe (LLVM 15.0.7, 256 bits)` and Windows reports
/// `Microsoft Basic Display Adapter`.
///
/// The list is deliberately short and deliberately unambiguous. Every entry is
/// a renderer with no hardware behind it at all. Paravirtual devices are *not*
/// here and must not be added: a VMware or VirtIO display adapter can be
/// backed by the host's real card, and treating one as software would turn the
/// acceleration off on a virtual machine that had it.
const SOFTWARE_RENDERERS: &[&str] = &[
    // Mesa's three processor renderers, in the order they are common.
    "llvmpipe",
    "softpipe",
    "swrast",
    // Google's, which appears in containers and on some remote desktops.
    "swiftshader",
    // What Windows reports when no display driver is installed, and what it
    // reports for a session with no display at all.
    "microsoft basic display",
    "microsoft basic render",
];

/// Whether a device name is one of the processor renderers.
///
/// A pure function of the name, so it is tested against the strings real
/// systems produce rather than against whatever this machine happens to have.
pub fn is_software_renderer(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    SOFTWARE_RENDERERS
        .iter()
        .any(|renderer| name.contains(renderer))
}

/// Read a list of graphics devices and decide what to ask the driver for.
///
/// Separated from [`look`] so the decision can be tested against constructed
/// lists. Nothing here runs a subprocess.
pub fn verdict(found: &veilvoice_video::accel::Found) -> Machine {
    if found.adapters.is_empty() {
        // Two different situations that must not read the same. A tool that
        // would not run has told us nothing, and an empty list from a tool
        // that ran is a machine this build does not know how to read. Both
        // land on asking for hardware, which is what happened before this
        // module existed, and both say they are a fallback rather than a
        // finding.
        let why = match found.problems.first() {
            Some(problem) => format!(
                "This machine would not say what it draws with, so the \
                 driver is asked for hardware, which is what every machine \
                 was asked before. It said: {problem}"
            ),
            None => "Nothing was listed as a graphics device here, which is \
                     not the same as there being none. The driver is asked \
                     for hardware, which is what every machine was asked \
                     before."
                .to_string(),
        };
        return Machine {
            acceleration: true,
            acceleration_reason: why,
            graphics_answered: false,
        };
    }

    let hardware: Vec<&str> = found
        .adapters
        .iter()
        .map(|adapter| adapter.name.as_str())
        .filter(|name| !is_software_renderer(name))
        .collect();

    if hardware.is_empty() {
        let names: Vec<&str> = found
            .adapters
            .iter()
            .map(|adapter| adapter.name.as_str())
            .collect();
        return Machine {
            acceleration: false,
            acceleration_reason: format!(
                "The only graphics device here is {}, which draws on the \
                 processor. Asking the driver for hardware it does not have \
                 is a request that can only be refused, so the window starts \
                 on the software path and skips the refusal. The picture is \
                 the same either way. Turn it back on if this machine gains a \
                 graphics card.",
                names.join(", ")
            ),
            graphics_answered: true,
        };
    }

    Machine {
        acceleration: true,
        acceleration_reason: format!(
            "{} is here, so the driver is asked to draw the window. Asking is \
             the safe direction: a machine that cannot give a hardware context \
             is given a software one and the window still opens. Turn it off \
             if the window is black or wrong, which happens on some drivers \
             that accept and then draw badly. The About tab shows what the \
             driver actually gave.",
            hardware.join(", ")
        ),
        graphics_answered: true,
    }
}

/// Ask this machine, once per process.
///
/// The subprocess behind it runs on the first call and never again, so the
/// two callers that both need the answer before the first frame -- `main`,
/// which creates the window, and [`crate::settings::Settings::load`], which
/// writes the answer down -- cost one look between them.
pub fn look() -> &'static Machine {
    static ANSWER: OnceLock<Machine> = OnceLock::new();
    ANSWER.get_or_init(|| verdict(&veilvoice_video::accel::look()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_video::accel::{Adapter, Found, Vendor};

    /// Build an adapter with just the name, which is all this module reads.
    fn adapter(name: &str) -> Adapter {
        Adapter {
            name: name.to_string(),
            vendor: Vendor::of(name),
            driver: None,
            integrated: false,
        }
    }

    #[test]
    fn the_processor_renderers_are_recognised_by_the_names_systems_print() {
        // Verbatim from the machines that produce them, not tidied. A test
        // that matches only the bare word is a test that passes while the
        // real string goes unrecognised.
        for name in [
            "llvmpipe (LLVM 15.0.7, 256 bits)",
            "LLVMpipe",
            "softpipe",
            "swrast",
            "SwiftShader Device (Subzero)",
            "Microsoft Basic Display Adapter",
            "Microsoft Basic Render Driver",
        ] {
            assert!(
                is_software_renderer(name),
                "{name} draws on the processor and was not recognised"
            );
        }
    }

    #[test]
    fn real_cards_are_not_mistaken_for_software() {
        for name in [
            "NVIDIA GeForce RTX 4070",
            "AMD Radeon RX 7900 XTX",
            "Intel UHD Graphics 630",
            "Apple M2",
            "Mesa Intel(R) Iris(R) Xe Graphics",
        ] {
            assert!(
                !is_software_renderer(name),
                "{name} is hardware and was treated as software"
            );
        }
    }

    /// A paravirtual display can be backed by the host's real card. Calling one
    /// software would switch acceleration off on every virtual machine that
    /// actually had it.
    #[test]
    fn paravirtual_displays_are_left_alone() {
        for name in [
            "VMware SVGA II Adapter",
            "Red Hat, Inc. Virtio GPU",
            "Cirrus Logic GD 5446",
            "QXL paravirtual graphic card",
        ] {
            assert!(
                !is_software_renderer(name),
                "{name} may have a real card behind it"
            );
        }
    }

    #[test]
    fn a_machine_with_a_card_is_asked_for_hardware() {
        let machine = verdict(&Found {
            adapters: vec![adapter("NVIDIA GeForce RTX 4070")],
            problems: Vec::new(),
        });
        assert!(machine.acceleration);
        assert!(machine.graphics_answered);
        assert!(
            machine.acceleration_reason.contains("GeForce"),
            "the reason names what was found: {}",
            machine.acceleration_reason
        );
    }

    #[test]
    fn a_machine_with_only_a_processor_renderer_starts_on_software() {
        let machine = verdict(&Found {
            adapters: vec![adapter("llvmpipe (LLVM 15.0.7, 256 bits)")],
            problems: Vec::new(),
        });
        assert!(
            !machine.acceleration,
            "there is no hardware here to ask for"
        );
        assert!(machine.graphics_answered);
        assert!(machine.acceleration_reason.contains("llvmpipe"));
    }

    /// The case that decides this is not a blunt "is there a GPU": a laptop
    /// listing both its real graphics and a fallback renderer has hardware.
    #[test]
    fn one_real_device_beside_a_software_one_is_still_hardware() {
        let machine = verdict(&Found {
            adapters: vec![
                adapter("llvmpipe (LLVM 15.0.7, 256 bits)"),
                adapter("Intel UHD Graphics 630"),
            ],
            problems: Vec::new(),
        });
        assert!(machine.acceleration);
        assert!(
            machine
                .acceleration_reason
                .contains("Intel UHD Graphics 630"),
            "the reason names the hardware, not the renderer beside it: {}",
            machine.acceleration_reason
        );
        assert!(
            !machine.acceleration_reason.contains("llvmpipe"),
            "naming the software renderer as the reason for asking would be \
             the wrong sentence"
        );
    }

    /// A tool that would not run has told us nothing, and nothing is not a
    /// finding. This is the case every CI runner and every machine without
    /// `lspci` takes, so it has to land on the behaviour that came before.
    #[test]
    fn a_machine_that_would_not_answer_is_asked_for_hardware_and_says_so() {
        let machine = verdict(&Found {
            adapters: Vec::new(),
            problems: vec!["lspci is not installed".into()],
        });
        assert!(
            machine.acceleration,
            "an unanswered question must not turn the GPU off"
        );
        assert!(
            !machine.graphics_answered,
            "the interface must not claim a measurement it did not take"
        );
        assert!(
            machine
                .acceleration_reason
                .contains("lspci is not installed"),
            "the reason quotes what the machine said: {}",
            machine.acceleration_reason
        );
    }

    #[test]
    fn an_empty_list_from_a_tool_that_ran_is_also_not_a_finding() {
        let machine = verdict(&Found::default());
        assert!(machine.acceleration);
        assert!(!machine.graphics_answered);
        assert!(
            !machine.acceleration_reason.is_empty(),
            "every answer carries its reason"
        );
    }

    /// The reasons are shown to a reader, so they follow the house style.
    #[test]
    fn no_reason_contains_an_em_dash() {
        let cases = [
            verdict(&Found {
                adapters: vec![adapter("NVIDIA GeForce RTX 4070")],
                problems: Vec::new(),
            }),
            verdict(&Found {
                adapters: vec![adapter("llvmpipe")],
                problems: Vec::new(),
            }),
            verdict(&Found::default()),
        ];
        for machine in cases {
            assert!(
                !machine.acceleration_reason.contains('\u{2014}'),
                "an em dash reached a sentence somebody reads: {}",
                machine.acceleration_reason
            );
        }
    }

    /// Asking twice must not look twice. The subprocess is the reason this
    /// module is cached at all.
    #[test]
    fn the_answer_is_taken_once_and_reused() {
        let first = look();
        let second = look();
        assert!(
            std::ptr::eq(first, second),
            "the second look ran the probe again"
        );
    }

    /// Whatever this machine is, the probe returns a usable answer rather than
    /// failing. It runs on every developer's machine and in CI, so it must not
    /// depend on what is installed.
    #[test]
    fn the_real_probe_answers_on_whatever_machine_this_is() {
        let machine = look();
        assert!(
            !machine.acceleration_reason.is_empty(),
            "an answer with no reason is a written-down default again"
        );
        if !machine.graphics_answered {
            assert!(
                machine.acceleration,
                "the fallback is the behaviour that came before"
            );
        }
    }
}
