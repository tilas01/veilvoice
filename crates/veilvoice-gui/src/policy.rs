// SPDX-License-Identifier: GPL-3.0-or-later
//! The policy in force, and what the interface does about it.
//!
//! A thin layer over [`veilvoice_policy`]: read the plain file once at startup,
//! hand the answers to the controls it fixes, and draw the reason beside each
//! one.
//!
//! # A control that is disabled without a reason is a bug report
//!
//! Every requirement carries its own sentence
//! ([`veilvoice_policy::Requirement::describe`]), and this module draws it
//! under the control it has taken away. That is the whole user-facing point:
//! somebody who cannot turn encryption off should be able to see, without
//! asking anybody, that it was fixed deliberately and by what.
//!
//! # Enforcement is not the drawing code
//!
//! Disabling a checkbox is a claim about pixels. The values a job actually uses
//! come from [`crate::VeilVoiceApp`]'s constrained posture, so a policy holds
//! even if a control is drawn wrongly, and the tests assert the behaviour
//! rather than the layout — the same rule the at-rest dialogue follows.
//!
//! # Reading it costs nothing, and proves nothing
//!
//! [`InForce::load`] never asks for a passphrase and never blocks. It can
//! therefore say only that a policy is in force, not that it is the one
//! somebody sealed; `veilvoice policy verify` is where that question is asked.
//! The reason it is safe to apply an unverified policy is the one-way property
//! [`veilvoice_policy`] is built around, and [`InForce::panel`] states it
//! rather than leaving the reader to infer it.

use crate::theme::palette as p;
use egui::{RichText, Ui};
use std::path::PathBuf;
use veilvoice_policy::{Policy, Posture, Requirement, Verification};

/// The policy this machine is running under, if any.
#[derive(Clone, Debug, Default)]
pub struct InForce {
    policy: Option<Policy>,
    /// Where it was read from, shown so the user can go and look at it.
    from: Option<PathBuf>,
    /// Why there is none, when a file exists and would not parse.
    ///
    /// Reported rather than swallowed: a policy file that does not parse means
    /// requirements somebody wrote are not being applied, and silently running
    /// unrestricted is the failure mode this project has found in itself most
    /// often.
    problem: Option<String>,
}

/// Where the policy files live, beside everything else VeilVoice keeps.
///
/// The same directory the command line uses. Resolved from the app lock's path
/// rather than worked out again, so the two front ends cannot end up looking in
/// different places.
pub fn default_dir() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("").join("policy"))
}

impl InForce {
    /// No policy. What tests and `Default` use, so neither touches the disk.
    pub fn none() -> Self {
        Self::default()
    }

    /// A policy supplied directly, for tests.
    pub fn from_policy(policy: Policy) -> Self {
        Self {
            policy: Some(policy),
            from: None,
            problem: None,
        }
    }

    /// Read the plain policy from the usual place. Never asks for a passphrase.
    pub fn load() -> Self {
        let Some(dir) = default_dir() else {
            return Self::none();
        };
        match Policy::load(&dir) {
            Ok(Some(policy)) => Self {
                policy: Some(policy),
                from: Some(dir),
                problem: None,
            },
            Ok(None) => Self {
                policy: None,
                from: Some(dir),
                problem: None,
            },
            Err(error) => Self {
                policy: None,
                from: Some(dir),
                problem: Some(error.to_string()),
            },
        }
    }

    /// Whether anything is fixed.
    pub fn is_active(&self) -> bool {
        self.policy
            .as_ref()
            .map(|policy| !policy.is_empty())
            .unwrap_or(false)
    }

    /// Whether a particular requirement is in force.
    pub fn requires(&self, requirement: &Requirement) -> bool {
        self.policy
            .as_ref()
            .map(|policy| policy.requires(requirement))
            .unwrap_or(false)
    }

    /// The intensity floor, or 0.0 when none is set.
    pub fn minimum_intensity(&self) -> f32 {
        self.policy
            .as_ref()
            .map(Policy::minimum_intensity)
            .unwrap_or(0.0)
    }

    /// Apply the policy to a posture. Only ever tightens.
    pub fn constrain(&self, posture: Posture) -> Posture {
        match &self.policy {
            Some(policy) => policy.constrain(posture),
            None => posture,
        }
    }

    /// Draw the reason a control is fixed, under that control.
    ///
    /// Does nothing when the requirement is not in force, so a call site can be
    /// unconditional and there is no `if` for somebody to get backwards.
    pub fn note(&self, ui: &mut Ui, requirement: &Requirement) {
        if !self.requires(requirement) {
            return;
        }
        ui.label(
            RichText::new(format!("fixed by policy: {}", requirement.describe()))
                .small()
                .color(p::yellow()),
        );
    }

    /// The summary panel, for the about tab.
    pub fn panel(&self, ui: &mut Ui) {
        ui.label(RichText::new("Policy").color(p::blue()).small());

        if let Some(problem) = &self.problem {
            ui.label(
                RichText::new(format!(
                    "a policy file is present and could not be read, so nothing from it \
                     is being applied: {problem}"
                ))
                .color(p::red()),
            );
            return;
        }

        let Some(policy) = &self.policy else {
            ui.label(
                RichText::new(
                    "None. Every setting is yours. `veilvoice policy` can fix some of \
                     them so the interface cannot turn them off.",
                )
                .color(p::fg()),
            );
            return;
        };

        if policy.is_empty() {
            ui.label(RichText::new("A policy file exists and requires nothing.").color(p::fg()));
            return;
        }

        if let Some(note) = policy.note() {
            ui.label(RichText::new(note).color(p::fg()));
            ui.add_space(4.0);
        }
        for requirement in policy.requirements() {
            ui.label(RichText::new(format!("· {}", requirement.describe())).color(p::fg()));
        }
        ui.add_space(6.0);
        if let Some(from) = &self.from {
            ui.label(
                RichText::new(from.display().to_string())
                    .small()
                    .color(p::muted()),
            );
        }
        // Said here rather than left to be inferred: this application has not
        // checked the seal and is not going to, because checking it needs a
        // passphrase and reading a policy must not cost the user a prompt.
        ui.label(
            RichText::new(Verification::Unchecked.describe())
                .small()
                .color(p::muted()),
        );
        ui.label(
            RichText::new("`veilvoice policy verify` checks the seal, and needs the passphrase.")
                .small()
                .color(p::muted()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requiring(requirements: &[Requirement]) -> InForce {
        let mut policy = Policy::new();
        for requirement in requirements {
            policy.require(*requirement);
        }
        InForce::from_policy(policy)
    }

    #[test]
    fn no_policy_constrains_nothing() {
        let none = InForce::none();
        assert!(!none.is_active());
        assert_eq!(none.minimum_intensity(), 0.0);
        let posture = Posture::most_permissive();
        assert_eq!(none.constrain(posture), posture);
        for requirement in [
            Requirement::EncryptRecordings,
            Requirement::CleanMetadata,
            Requirement::NeutraliseAccent,
            Requirement::AppLock,
        ] {
            assert!(!none.requires(&requirement));
        }
    }

    /// An empty policy file is a policy file, and it fixes nothing.
    #[test]
    fn an_empty_policy_is_not_active() {
        assert!(!InForce::from_policy(Policy::new()).is_active());
    }

    #[test]
    fn a_policy_tightens_the_posture_it_is_given() {
        let policy = requiring(&[
            Requirement::EncryptRecordings,
            Requirement::CleanMetadata,
            Requirement::MinimumIntensity(70),
        ]);
        assert!(policy.is_active());
        let after = policy.constrain(Posture::most_permissive());
        assert!(after.encrypt_recordings);
        assert!(after.clean_metadata);
        assert!((after.intensity - 0.7).abs() < 1e-6);
        // And leaves alone what it does not mention.
        assert!(!after.neutralise_accent);
        assert!(!after.app_lock);
    }

    /// Loading must never change the machine, and must never panic on a
    /// machine that has no configuration directory at all.
    #[test]
    fn loading_reads_and_changes_nothing() {
        let first = InForce::load();
        let second = InForce::load();
        assert_eq!(first.is_active(), second.is_active());
        assert_eq!(first.minimum_intensity(), second.minimum_intensity());
    }

    /// Every panel state renders with no window, including the one nobody
    /// wants: a policy file that will not parse.
    #[test]
    fn every_panel_state_renders_without_a_window() {
        let unreadable = InForce {
            policy: None,
            from: Some(PathBuf::from("/somewhere/policy")),
            problem: Some("line 2: unknown keyword".to_string()),
        };
        let states = [
            InForce::none(),
            InForce::from_policy(Policy::new()),
            requiring(&[Requirement::AppLock, Requirement::MinimumIntensity(50)]),
            unreadable,
        ];
        for state in states {
            let ctx = egui::Context::default();
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    state.panel(ui);
                    state.note(ui, &Requirement::AppLock);
                    state.note(ui, &Requirement::EncryptRecordings);
                });
            });
        }
    }

    /// A policy file that will not parse must say so rather than reading as
    /// "no policy" -- the difference is between "nothing was asked for" and
    /// "something was asked for and is not being applied".
    #[test]
    fn an_unreadable_policy_is_reported_rather_than_treated_as_absent() {
        let unreadable = InForce {
            policy: None,
            from: None,
            problem: Some("line 2: unknown keyword".to_string()),
        };
        assert!(!unreadable.is_active());
        assert!(unreadable.problem.is_some());
    }
}
