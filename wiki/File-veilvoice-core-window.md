![window.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-core/window.svg)

# `crates/veilvoice-core/src/window.rs`

[[veilvoice-core|Crate-veilvoice-core]] &middot; 54 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/window.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Analysis/synthesis windowing helpers.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_hann(["hann<br/>pub"])
    n_ola_gain(["ola_gain<br/>pub"])
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `hann` <sub>pub fn</sub> | [10](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/window.rs#L10) | Periodic Hann window of length n (the correct variant for STFT overlap-add, as opposed to the symmetric variant used for filter design). |
| `ola_gain` <sub>pub fn</sub> | [26](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/window.rs#L26) | Overlap-add normalisation for a window applied on both analysis and synthesis at the given hop. |
