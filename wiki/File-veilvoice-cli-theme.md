![theme.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-cli/theme.svg)

# `crates/veilvoice-cli/src/theme.rs`

[[veilvoice-cli|Crate-veilvoice-cli]] &middot; 115 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Tokyo Night colouring for the terminal.

The same palette the GUI uses, so the two halves of VeilVoice look like one
program. Colour is suppressed when the output is not a terminal, when
`NO_COLOR` is set (the widely-honoured convention), or when `TERM=dumb`, so
piping to a file or a log never produces escape-code soup.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_enabled["enabled"]
    n_paint(["paint<br/>pub"])
    n_ok(["ok<br/>pub"])
    n_warn(["warn<br/>pub"])
    n_err(["err<br/>pub"])
    n_heading(["heading<br/>pub"])
    n_field(["field<br/>pub"])
    n_err --> n_paint
    n_field --> n_paint
    n_heading --> n_paint
    n_ok --> n_paint
    n_paint --> n_enabled
    n_warn --> n_paint
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `colour` <sub>pub mod</sub> | [20](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L20) | Tokyo Night, as 24-bit foreground escape sequences. |
| `enabled` <sub>fn</sub> | [39](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L39) |  |
| `paint` <sub>pub fn</sub> | [53](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L53) | Wrap text in colour, or return it unchanged when colour is off. |
| `ok` <sub>pub fn</sub> | [62](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L62) | A success line. |
| `warn` <sub>pub fn</sub> | [67](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L67) | A warning line. |
| `err` <sub>pub fn</sub> | [72](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L72) | An error line. |
| `heading` <sub>pub fn</sub> | [77](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L77) | A section heading. |
| `field` <sub>pub fn</sub> | [82](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/src/theme.rs#L82) | A label: value line with the value highlighted. |
