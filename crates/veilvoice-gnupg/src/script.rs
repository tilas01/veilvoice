// SPDX-License-Identifier: GPL-3.0-or-later
//! A shell script that checks a release, for people who would rather read one.
//!
//! # Why a script at all, when there is a verifier
//!
//! `veilvoice-verify` does this check, and it does more of it: every extracted
//! file, not just the archive. But it has the problem every such program has,
//! and this project says so on the tab: **it came out of the download it is
//! checking.** A tampered release ships a tampered verifier.
//!
//! This is the answer to that. Sixty lines of shell, using `gpg` and
//! `sha256sum` and nothing else, short enough that somebody can read the whole
//! of it before running it. That is the point of it: not convenience, but that
//! the thing doing the checking is not this project's code.
//!
//! # Why it is generated rather than committed
//!
//! The fingerprint in it is [`veilvoice_check::FINGERPRINT`], and the one thing
//! that must never happen is a script checking against a fingerprint that has
//! drifted from the one the project actually signs with. A committed script is
//! a second copy of that string. This is written from the first copy, every
//! time, so there is no second one to go stale.
//!
//! # What it deliberately does not do
//!
//! It does not install anything, and it does not download the release. It
//! checks files that are already in the current directory, and it says what to
//! run if `gpg` is missing rather than running it. A verification script that
//! fetched things would be a verification script with a network path in it.

/// Which system the script is being written for.
///
/// The only real difference is how the hashes are checked: coreutils calls it
/// `sha256sum`, macOS ships `shasum`, and the BSDs ship `sha256`. WSL is Linux,
/// and is listed separately only because saying so is what a Windows reader
/// needs to hear.
///
/// **The BSDs were missing and fell through to Linux**, which is F-167: this
/// script told a reader on FreeBSD, OpenBSD or NetBSD to run a command that
/// `veilvoice-check`'s reproduce script, in the same release, says they do not
/// have. The command now comes from that module rather than from a second copy
/// here, so the two cannot disagree again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// Linux, and WSL, which is Linux.
    Linux,
    /// macOS, where the hash tool has a different name.
    MacOs,
    /// FreeBSD, OpenBSD and NetBSD, which have a different one again.
    Bsd,
}

impl Flavour {
    /// Every one of them, so a caller writing all the scripts writes all of
    /// them rather than the ones it remembered.
    pub const ALL: &'static [Flavour] = &[Flavour::Linux, Flavour::MacOs, Flavour::Bsd];

    /// The same system, as the reproduce scripts name it.
    ///
    /// The two halves of "check what you downloaded" are this script and
    /// `veilvoice-check`'s, and a reader on one machine may follow either. They
    /// answer to one enumeration so that they cannot describe different
    /// machines.
    fn system(self) -> veilvoice_check::reproduce::System {
        match self {
            Flavour::Linux => veilvoice_check::reproduce::System::Linux,
            Flavour::MacOs => veilvoice_check::reproduce::System::MacOs,
            Flavour::Bsd => veilvoice_check::reproduce::System::Bsd,
        }
    }

    /// The command that checks a file against `SHA256SUMS`.
    ///
    /// Asked of [`veilvoice_check::reproduce::System`] rather than answered
    /// here. This function used to answer it, knew two systems, and was wrong
    /// about the third.
    fn hash_check(self) -> &'static str {
        self.system().hash_check_command()
    }

    /// What to type if GnuPG is not installed.
    fn install_hint(self) -> &'static str {
        match self {
            Flavour::Linux => {
                "sudo apt-get install -y gnupg   (or your package manager's equivalent)"
            }
            Flavour::MacOs => "brew install gnupg",
            // FreeBSD's spelling, which this repository already uses in
            // `install/install.sh`. OpenBSD's `pkg_add` and NetBSD's `pkgin`
            // are not named here, because this project has not run them and
            // does not print commands it has not run. The hedge is the same one
            // the Linux line carries, for the same reason.
            Flavour::Bsd => "sudo pkg install -y gnupg   (or your system's equivalent)",
        }
    }

    /// The name a reader would give the file.
    pub fn file_name(self) -> &'static str {
        match self {
            Flavour::Linux => "verify-veilvoice.sh",
            Flavour::MacOs => "verify-veilvoice-macos.sh",
            Flavour::Bsd => "verify-veilvoice-bsd.sh",
        }
    }
}

/// The script, with the fingerprint compiled in from the one source of it.
pub fn shell(flavour: Flavour) -> String {
    let fingerprint = veilvoice_check::FINGERPRINT;
    let hash_check = flavour.hash_check();
    let install = flavour.install_hint();
    format!(
        r##"#!/bin/sh
# Check a VeilVoice release, using GnuPG and nothing from VeilVoice.
#
# Run it in the folder holding the archive, SHA256SUMS and SHA256SUMS.asc.
# All three are published together on the releases page:
#
#   https://github.com/tilas01/veilvoice/releases/latest
#
# This script is short on purpose. Read it before you run it: the whole reason
# to use it rather than `veilvoice verify` is that it is not VeilVoice's code
# checking VeilVoice's download.
#
# Generated by `veilvoice verify --script`. The fingerprint below comes from
# the same constant the programs compile in, so the two cannot disagree.

set -eu

FINGERPRINT={fingerprint}

say() {{ printf '%s\n' "$*"; }}
die() {{ say "FAILED: $*"; exit 1; }}

command -v gpg >/dev/null 2>&1 || die "GnuPG is not installed. To install it:
    {install}"

[ -f SHA256SUMS ] || die "no SHA256SUMS here. Download it from the release."
[ -f SHA256SUMS.asc ] || die "no SHA256SUMS.asc here. Download it from the release."

# 1. The key. Import it from the release if it is here, and check that what
#    was imported is the fingerprint this script expects. This is the step
#    that anchors every other one, and it is the step nothing can do for you:
#    compare the fingerprint against the one on the website and in README.md,
#    from a source other than the download you are checking.
if [ -f veilvoice-signing-key.asc ]; then
    gpg --quiet --import veilvoice-signing-key.asc
fi

gpg --with-colons --fingerprint "$FINGERPRINT" >/dev/null 2>&1 \
    || die "the signing key is not in your keyring.
    Import it:  gpg --import veilvoice-signing-key.asc
    It is published beside every release, and is also in the repository at
    website/assets/veilvoice-signing-key.asc"

say "key         $FINGERPRINT is in your keyring"

# 2. The signature over the hash list. Before the hashes, always: whoever
#    could replace the download could replace SHA256SUMS beside it, and the
#    two would agree perfectly. The signature is what makes the list worth
#    comparing against.
#
#    The output is captured rather than piped. A pipeline's status is the
#    status of its LAST command, so `gpg ... | sed` reports whether `sed`
#    worked, and a script that did that would print a pass over a signature
#    GnuPG had just refused. It did, before this comment existed.
if signature=$(gpg --status-fd 1 --verify SHA256SUMS.asc SHA256SUMS 2>&1); then
    :
else
    printf '%s\n' "$signature" | sed 's/^/    /'
    die "GnuPG did not accept the signature over SHA256SUMS."
fi

#    And a good signature is not enough on its own: gpg reports a good
#    signature by ANY key in your keyring, including one an attacker talked
#    you into importing. VALIDSIG carries the fingerprint that actually
#    signed, first the signing key and last its primary key, so either may be
#    the one we expect.
printf '%s\n' "$signature" \
    | awk -v want="$FINGERPRINT" '
        /^\[GNUPG:\] VALIDSIG /  {{ if ($3 == want || $NF == want) hit = 1 }}
        END                     {{ exit !hit }}' \
    || die "the signature is not by $FINGERPRINT.
    GnuPG may still call it good: it is good by some other key. That is not
    the same thing, and it is exactly what a substituted release looks like."

say "signature   SHA256SUMS is signed by $FINGERPRINT"

# 3. The download against the now-trusted list.
#
#    `--ignore-missing` checks the files that are here, which is what somebody
#    who downloaded one archive out of twelve wants. It also means that with
#    NO release file here at all, the hash tool checks nothing and reports a
#    failure, and "a file does not match" would be a frightening and untrue
#    way to say "there was nothing to look at". So that case is separated out
#    and named.
present=0
while read -r _ name; do
    [ -f "$name" ] && present=$((present + 1))
done < SHA256SUMS
if [ "$present" -eq 0 ]; then
    die "none of the files listed in SHA256SUMS is in this folder, so there
    was nothing to check. Download the archive you want into this folder and
    run this again."
fi

{hash_check} || die "a file here does not match the signed list."
say "hashes      $present of the release's files are here, and all of them match"
say ""
say "What this proves: these files are the ones the holder of that key"
say "published. It does not prove they are safe, that the source compiles to"
say "them, or that the key belongs to anybody in particular."
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing that must never drift.
    #[test]
    fn the_script_carries_the_fingerprint_the_programs_use() {
        for flavour in Flavour::ALL.iter().copied() {
            assert!(
                shell(flavour).contains(veilvoice_check::FINGERPRINT),
                "the script does not carry the project's fingerprint"
            );
        }
    }

    /// Signature before hashes. Getting this order wrong makes the whole
    /// script worthless while still printing a pass, so it is checked rather
    /// than trusted to stay written correctly.
    #[test]
    fn the_signature_is_checked_before_the_hashes() {
        for flavour in Flavour::ALL.iter().copied() {
            let script = shell(flavour);
            let signature = script
                .find("gpg --status-fd 1 --verify")
                .expect("verifies the signature");
            let hashes = script
                .find(flavour.hash_check())
                .expect("checks the hashes");
            assert!(
                signature < hashes,
                "the hashes are checked before the signature, which proves the \
                 download matches a list and nothing about the list"
            );
        }
    }

    /// It checks what is here; it does not fetch anything. A verification
    /// script with a network path in it is a different and worse thing.
    /// **F-167.** The two scripts a reader might follow agree about their
    /// machine.
    ///
    /// This repository ships two shell scripts that check a download: this one,
    /// which verifies a signature and a hash list, and `veilvoice-check`'s,
    /// which reproduces the build. A reader on one machine may run either, and
    /// for a while they named different hash tools on the BSDs, because this
    /// module carried its own copy of the answer and that copy knew two
    /// systems.
    ///
    /// The copy is gone and this is what keeps it gone: every flavour asks the
    /// other module, and a flavour added here without a system there would not
    /// compile.
    #[test]
    fn no_script_names_another_machines_hash_tool() {
        // `hash_check` *is* `system().hash_check_command()` now, so comparing
        // the two would be comparing a function with itself. What can still go
        // wrong, and did, is a reader being handed the wrong script: the tool
        // has to reach the text, and no other machine's tool may.
        for flavour in Flavour::ALL.iter().copied() {
            let script = shell(flavour);
            let mine = flavour.system().hash_check_command();
            assert!(
                script.contains(mine),
                "the {flavour:?} script does not run {mine:?}, which is what \
                 that machine checks hashes with"
            );
            for other in Flavour::ALL.iter().copied() {
                if other == flavour {
                    continue;
                }
                let theirs = other.system().hash_check_command();
                // The BSDs' `sha256 -c` is a substring of nothing here, and
                // `sha256sum -c` contains no other tool's name, so a plain
                // containment check is exact for these three. It is asserted
                // rather than assumed, because a fourth tool whose name
                // contains another's would make this test quietly weaker.
                assert!(
                    !mine.contains(theirs) && !theirs.contains(mine),
                    "{mine:?} and {theirs:?} contain one another, so this test \
                     cannot tell them apart any more"
                );
                assert!(
                    !script.contains(theirs),
                    "the {flavour:?} script tells the reader to run {theirs:?}, \
                     which belongs to {other:?}. That is F-167: a reader on one \
                     machine given another machine's command"
                );
            }
        }
    }

    /// **Marker 129.** The guide's per-system table is the program's answer.
    ///
    /// The row this comes from asks for the verification to be written up per
    /// platform "rather than left as a Linux instruction somebody has to
    /// translate". A table of commands in a document is a copy of what the
    /// program prints, and a copy goes stale: F-167 was two copies of exactly
    /// this answer disagreeing. So the table is checked here against the code
    /// it describes, which is what `CLAUDE.md` asks for when a fact cannot be
    /// derived.
    ///
    /// Read out of the guide's source. If the table moves, this fails saying
    /// it cannot find it, rather than passing because there was nothing left
    /// to check.
    #[test]
    fn the_guides_table_of_systems_is_what_the_program_prints() {
        let guide = include_str!("../../../docs/USER_GUIDE.md").replace("\r\n", "\n");
        let table = guide
            .split("| Your system | The script it writes |")
            .nth(1)
            .and_then(|rest| rest.split("\n\n").next())
            .expect("the per-system table has to be findable in the user guide");

        for flavour in Flavour::ALL.iter().copied() {
            let name = flavour.file_name();
            let command = flavour.system().hash_check_command();
            assert!(
                table.contains(&format!("`{name}`")),
                "the guide's table does not name {name:?}, which is the file \
                 `--script --system {}` writes",
                flavour.system().key()
            );
            assert!(
                table.contains(&format!("`{command}`")),
                "the guide's table does not carry {command:?}, which is what \
                 the {flavour:?} script runs"
            );
        }
        // And nothing else: a fourth row would describe a system the program
        // has no script for, which is the failure this is guarding against
        // pointed the other way.
        let rows = table.lines().filter(|line| line.starts_with('|')).count();
        assert_eq!(
            rows,
            Flavour::ALL.len() + 1,
            "the guide's table has {rows} lines including its rule, and the \
             program has {} systems",
            Flavour::ALL.len()
        );
    }

    /// Every flavour has a file name of its own.
    ///
    /// Two flavours sharing one would mean a reader saving the second over the
    /// first, which is how somebody on a BSD ends up running the macOS script.
    #[test]
    fn every_flavour_is_saved_under_its_own_name() {
        let mut seen = std::collections::BTreeSet::new();
        for flavour in Flavour::ALL.iter().copied() {
            assert!(
                seen.insert(flavour.file_name()),
                "{flavour:?} shares a file name with another flavour"
            );
        }
        assert_eq!(seen.len(), Flavour::ALL.len());
    }

    #[test]
    fn the_script_downloads_nothing() {
        for flavour in Flavour::ALL.iter().copied() {
            let script = shell(flavour);
            for fetcher in ["curl ", "wget ", "nc "] {
                assert!(
                    !script.contains(fetcher),
                    "the script runs {fetcher:?}; it is meant to check files \
                     that are already here"
                );
            }
        }
    }

    /// It does not install anything either. It says what to type.
    ///
    /// The distinction is between running `sudo` and *mentioning* it, and the
    /// script does mention it: the message shown when GnuPG is missing is the
    /// command to install it. So this tracks quoting rather than searching for
    /// the word. A first attempt did search for the word and failed on the
    /// help text, which would have meant either deleting a useful message or
    /// keeping a test that could not tell a command from a sentence.
    #[test]
    fn the_script_installs_nothing() {
        for flavour in Flavour::ALL.iter().copied() {
            let script = shell(flavour);
            let mut inside_string = false;
            for line in script.lines() {
                let code = line.trim_start();
                if !inside_string {
                    assert!(
                        !code.starts_with("sudo ") && !code.starts_with("apt-get "),
                        "the script runs {code:?}; it is meant to print such a \
                         command and let the reader run it in a terminal where \
                         they can see what they are approving"
                    );
                }
                // An odd number of unescaped quotes on a line flips whether
                // what follows is inside one.
                if line.matches('"').count() % 2 == 1 {
                    inside_string = !inside_string;
                }
            }
        }
    }

    /// macOS has no `sha256sum`, and a script that calls one there fails at
    /// the last step after appearing to work.
    #[test]
    fn each_flavour_uses_the_hash_tool_that_system_has() {
        assert!(shell(Flavour::Linux).contains("sha256sum -c"));
        assert!(shell(Flavour::MacOs).contains("shasum -a 256 -c"));
        assert!(!shell(Flavour::MacOs).contains("sha256sum -c"));
        // **F-167.** The BSDs ship `sha256`, and this script used not to have
        // a spelling for them at all: they fell through to Linux and were told
        // to run `sha256sum -c`, which the reproduce script in the same
        // release says they do not have.
        assert!(shell(Flavour::Bsd).contains("sha256 -c"));
        assert!(
            !shell(Flavour::Bsd).contains("sha256sum"),
            "the BSD script names a tool this project's other script says the \
             BSDs do not have"
        );
    }

    /// The script says where the files come from. Somebody running it in the
    /// wrong folder needs to know what to fetch and from where.
    #[test]
    fn the_script_says_where_the_files_come_from() {
        let script = shell(Flavour::Linux);
        assert!(script.contains("releases/latest"));
        assert!(script.contains("veilvoice-signing-key.asc"));
    }
}
