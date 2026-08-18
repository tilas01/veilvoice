![effects.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-core/effects.svg)

# `crates/veilvoice-core/src/effects.rs`

[[veilvoice-core|Crate-veilvoice-core]] &middot; 214 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs)

## Contents

- [How little these contribute, said plainly](#how-little-these-contribute-said-plainly)
- [Why every mix defaults low](#why-every-mix-defaults-low)
- [Real-time constraints](#real-time-constraints)
  - [What calls what](#what-calls-what)
  - [Items](#items)

Light time-domain effects applied after resynthesis.

These run on the continuous output stream (not per FFT frame) and exist to
(a) further decorrelate the signal from the original, and (b) add a few
detuned "voices" so the spectrogram is densely filled rather than showing a
clean harmonic stack — without harming intelligibility, so every mix defaults
low. None of them are invertible in a way that recovers the source voice.

# How little these contribute, said plainly

It would be easy to read a chorus and a reverb as part of the anonymity
argument. They are not, and this crate should not let anybody think they
are. **The voiceprint is destroyed in `crate::spectral`** -- by discarding
measured phase and by mapping every speaker onto one canonical pitch
register and vocal-tract scale. That has already happened before a single
sample reaches this file.

What these three add is *decorrelation at the margins*: a denser
spectrogram, a few detuned voices where there was a clean harmonic stack,
and some odd harmonics smearing whatever residual cues survived. Useful,
cheap, and nowhere near sufficient alone. Set all three mixes to zero and
the output is exactly as unlinkable as before.

The reason to be exact about this is that a filter chain of precisely this
shape -- clip, chorus, reverb -- is what a *voice changer* ships, and a
voice changer offers no anonymity whatsoever. Everything that separates this
project from that one happens upstream of this file.

# Why every mix defaults low

Intelligibility is a requirement, not a preference. Each of these effects
trades clarity for density, and past a fairly low mix the words start to
cost more than the added decorrelation is worth. The defaults sit where a
listener does not notice the effect is there at all; they are a starting
point a user may raise, not a recommendation to raise them.

# Real-time constraints

These run per output sample inside an audio callback, on the continuous
stream rather than per FFT frame. Every buffer is allocated once at
construction: `process` allocates nothing, takes no lock and reads no clock.
`Chorus` and `Reverb` own their delay lines and index them with wrapping
arithmetic, so changing sample rate means building a new one rather than
resizing a live one.

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
| `SoftClip` <sub>pub struct</sub> | [51](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L51) | Symmetric soft-clip (tanh) waveshaper. |
| `SoftClip::new` <sub>pub fn</sub> | [58](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L58) |  |
| `SoftClip::process` <sub>pub fn</sub> | [67](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L67) |  |
| `DelayVoice` <sub>struct</sub> | [75](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L75) | A single modulated delay line, summed into a small ensemble to create the impression of several slightly different voices. |
| `DelayVoice::new` <sub>fn</sub> | [85](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L85) |  |
| `DelayVoice::process` <sub>fn</sub> | [99](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L99) |  |
| `Chorus` <sub>pub struct</sub> | [116](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L116) | Detuned chorus ensemble. |
| `Chorus::new` <sub>pub fn</sub> | [122](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L122) |  |
| `Chorus::process` <sub>pub fn</sub> | [135](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L135) |  |
| `Reverb` <sub>pub struct</sub> | [147](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L147) | Minimal Schroeder-style reverb: one feedback comb + one all-pass. |
| `Reverb::new` <sub>pub fn</sub> | [158](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L158) |  |
| `Reverb::process` <sub>pub fn</sub> | [172](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/effects.rs#L172) |  |
