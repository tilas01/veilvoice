![effects.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-core/effects.svg)

# `crates/veilvoice-core/src/effects.rs`

[[veilvoice-core|Crate-veilvoice-core]] &middot; 177 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Light time-domain effects applied after resynthesis.

These run on the continuous output stream (not per FFT frame) and exist to
(a) further decorrelate the signal from the original, and (b) add a few
detuned "voices" so the spectrogram is densely filled rather than showing a
clean harmonic stack — without harming intelligibility, so every mix defaults
low. None of them are invertible in a way that recovers the source voice.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_new(["SoftClip::new<br/>pub"])
    n_process(["SoftClip::process<br/>pub"])
    n_new["DelayVoice::new"]
    n_process["DelayVoice::process"]
    n_new(["Chorus::new<br/>pub"])
    n_process(["Chorus::process<br/>pub"])
    n_new(["Reverb::new<br/>pub"])
    n_process(["Reverb::process<br/>pub"])
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `SoftClip` <sub>pub struct</sub> | [14](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L14) | Symmetric soft-clip (tanh) waveshaper. |
| `SoftClip::new` <sub>pub fn</sub> | [21](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L21) |  |
| `SoftClip::process` <sub>pub fn</sub> | [30](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L30) |  |
| `DelayVoice` <sub>struct</sub> | [38](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L38) | A single modulated delay line, summed into a small ensemble to create the impression of several slightly different voices. |
| `DelayVoice::new` <sub>fn</sub> | [48](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L48) |  |
| `DelayVoice::process` <sub>fn</sub> | [62](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L62) |  |
| `Chorus` <sub>pub struct</sub> | [79](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L79) | Detuned chorus ensemble. |
| `Chorus::new` <sub>pub fn</sub> | [85](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L85) |  |
| `Chorus::process` <sub>pub fn</sub> | [98](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L98) |  |
| `Reverb` <sub>pub struct</sub> | [110](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L110) | Minimal Schroeder-style reverb: one feedback comb + one all-pass. |
| `Reverb::new` <sub>pub fn</sub> | [121](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L121) |  |
| `Reverb::process` <sub>pub fn</sub> | [135](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L135) |  |
