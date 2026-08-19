![lock.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-cli/lock.svg)

# `crates/veilvoice-cli/src/lock.rs`

[[veilvoice-cli|Crate-veilvoice-cli]] &middot; 239 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

`veilvoice lock` — manage the application lock from the command line.

The lock guards the desktop app: with one set, VeilVoice asks for a password
before it will show anything or start a live scramble. Managing it from here
exists because a headless machine still has a config directory, and because
anything the GUI can do to a file on disk should be inspectable without the
GUI.

Every path through this module prints `veilvoice_crypto::lock::SCOPE`, for
one reason: a lock the user believes is stronger than it is has made them
*less* safe, not more.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#737aa2","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_resolve["resolve"]
    n_print_scope["print_scope"]
    n_wrap(["wrap<br/>pub"])
    n_run(["run<br/>pub"])
    n_status["status"]
    n_set["set"]
    n_change["change"]
    n_remove["remove"]
    n_open_or_explain["open_or_explain"]
    n_change --> n_open_or_explain
    n_print_scope --> n_wrap
    n_remove --> n_open_or_explain
    n_run --> n_change
    n_run --> n_remove
    n_run --> n_resolve
    n_run --> n_set
    n_run --> n_status
    n_set --> n_print_scope
    n_status --> n_print_scope
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Action` <sub>pub enum</sub> | [21](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L21) |  |
| `resolve` <sub>fn</sub> | [33](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L33) | Resolve the lock file, preferring an explicit --path. |
| `print_scope` <sub>fn</sub> | [45](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L45) | Print the honest scope note, wrapped for a terminal. |
| `wrap` <sub>pub fn</sub> | [54](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L54) | Greedy word wrap. |
| `run` <sub>pub fn</sub> | [72](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L72) |  |
| `status` <sub>fn</sub> | [85](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L85) |  |
| `set` <sub>fn</sub> | [119](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L119) |  |
| `change` <sub>fn</sub> | [162](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L162) |  |
| `remove` <sub>fn</sub> | [174](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L174) |  |
| `open_or_explain` <sub>fn</sub> | [185](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/lock.rs#L185) |  |
