![guard.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-cli/guard.svg)

# `crates/veilvoice-cli/src/guard.rs`

[[veilvoice-cli|Crate-veilvoice-cli]] &middot; 306 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

`veilvoice guard` -- record what VeilVoice's files should be, and check them.

Detection, not prevention. See `veilvoice_guard::SCOPE`, which every path
through this module prints, for the same reason the app lock prints its own:
a protection someone over-trusts has made them less safe, not more.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
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
| `Action` <sub>pub enum</sub> | [14](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L14) |  |
| `manifest_path` <sub>fn</sub> | [35](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L35) | Where the manifest lives, beside the app lock. |
| `sealed_path` <sub>fn</sub> | [48](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L48) | A sealed manifest sits beside the plain one, with a different suffix. |
| `print_scope` <sub>fn</sub> | [52](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L52) |  |
| `default_targets` <sub>fn</sub> | [61](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L61) | The files worth watching when the user names none: the running binary, and the app lock beside it. |
| `run` <sub>pub fn</sub> | [74](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L74) |  |
| `init` <sub>fn</sub> | [86](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L86) |  |
| `load` <sub>fn</sub> | [149](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L149) | Load whichever form of the record exists, asking for a passphrase only if the sealed one is the one that is there. |
| `check` <sub>fn</sub> | [162](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L162) |  |
| `status` <sub>fn</sub> | [231](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/guard.rs#L231) |  |
