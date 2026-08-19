![guard.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-cli/guard.svg)

# `crates/veilvoice-cli/src/guard.rs`

[[veilvoice-cli|Crate-veilvoice-cli]] &middot; 338 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs)

## Contents

- [What the three steps actually do](#what-the-three-steps-actually-do)
- [Why attribution usually fails, and why that is reported rather than hidden](#why-attribution-usually-fails-and-why-that-is-reported-rather-than-hidden)
- [The bound, again](#the-bound-again)
  - [What calls what](#what-calls-what)
  - [Items](#items)

`veilvoice guard` -- record what VeilVoice's files should be, and check them.

Detection, not prevention. See `veilvoice_guard::SCOPE`, which every path
through this module prints, for the same reason the app lock prints its own:
a protection someone over-trusts has made them less safe, not more.

# What the three steps actually do

* **`init`** walks the files that make up this installation and records a
SHA-256 for each. Optionally sealed with a passphrase, so the record
itself cannot be quietly rewritten to match tampered files.
* **`check`** re-walks and reports what is **modified**, **removed** and
**added**. All three matter: an added file in the installation directory
is as interesting as a changed one.
* **`blame`** tries to say *which process* made a change, and says plainly
when it cannot.

# Why attribution usually fails, and why that is reported rather than hidden

Attribution needs the operating system to have been recording. On Linux that
means an `auditd` watch; on Windows a SACL on the path plus the audit policy
enabled, and reading it needs elevation. Neither is on by default on a
normal machine.

So the common answer is "something changed this file and I cannot tell you
what", and this module prints exactly that rather than an empty list. An
empty list reads as *nothing happened*, which is the opposite of the truth,
and is the same mistake as a monitor reporting an empty machine because a
registry query silently matched nothing.

# The bound, again

A manifest running as the user protects nothing from that user, and detects
rather than prevents even when it works. Anything that can write these files
can write the manifest beside them. That is why the passphrase-sealed record
exists, why `veilvoice_guard::SCOPE` is printed on every path through this
module, and why the word "tamper-proof" appears nowhere in it.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#737aa2","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_manifest_path["manifest_path"]
    n_sealed_path["sealed_path"]
    n_print_scope["print_scope"]
    n_default_targets["default_targets"]
    n_run(["run<br/>pub"])
    n_init["init"]
    n_load["load"]
    n_check["check"]
    n_status["status"]
    n_check --> n_load
    n_check --> n_print_scope
    n_check --> n_sealed_path
    n_init --> n_default_targets
    n_init --> n_print_scope
    n_init --> n_sealed_path
    n_load --> n_sealed_path
    n_run --> n_check
    n_run --> n_init
    n_run --> n_manifest_path
    n_run --> n_status
    n_status --> n_print_scope
    n_status --> n_sealed_path
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Action` <sub>pub enum</sub> | [46](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L46) |  |
| `manifest_path` <sub>fn</sub> | [67](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L67) | Where the manifest lives, beside the app lock. |
| `sealed_path` <sub>fn</sub> | [80](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L80) | A sealed manifest sits beside the plain one, with a different suffix. |
| `print_scope` <sub>fn</sub> | [84](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L84) |  |
| `default_targets` <sub>fn</sub> | [93](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L93) | The files worth watching when the user names none: the running binary, and the app lock beside it. |
| `run` <sub>pub fn</sub> | [106](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L106) |  |
| `init` <sub>fn</sub> | [118](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L118) |  |
| `load` <sub>fn</sub> | [181](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L181) | Load whichever form of the record exists, asking for a passphrase only if the sealed one is the one that is there. |
| `check` <sub>fn</sub> | [194](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L194) |  |
| `status` <sub>fn</sub> | [263](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L263) |  |
