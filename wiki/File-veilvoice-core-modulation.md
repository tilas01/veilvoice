![modulation.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-core/modulation.svg)

# `crates/veilvoice-core/src/modulation.rs`

[[veilvoice-core|Crate-veilvoice-core]] &middot; 280 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Cryptographically-seeded modulation of the effect parameters.

The pitch and formant ratios are never constant: a ChaCha20 CSPRNG picks a
new random target every `frames_per_target` STFT frames, and a one-pole
filter glides continuously toward it. Because the transform is therefore
non-stationary and unpredictable, an attacker cannot "undo" it by assuming a
single fixed shift — there is no single shift to undo, and the target
sequence is unknowable without the seed (which never leaves the process and
is zeroized on drop).

The seed does not stay put either. It is rolled forward every couple of
seconds by default (see `Modulator::reseed`), so the stream driving any
given stretch of audio is closed off permanently once that stretch is past.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_new["Param::new"]
    n_retarget["Param::retarget"]
    n_step["Param::step"]
    n_from_seed(["Modulator::from_seed<br/>pub"])
    n_fill_phase_offsets(["Modulator::fill_phase_offsets<br/>pub"])
    n_reseed(["Modulator::reseed<br/>pub"])
    n_next_frame(["Modulator::next_frame<br/>pub"])
    n_drop["Modulator::drop"]
    n_from_seed --> n_new
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Param` <sub>struct</sub> | [23](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L23) | One smoothly-varying parameter bounded to lo, hi. |
| `Param::new` <sub>fn</sub> | [32](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L32) |  |
| `Param::retarget` <sub>fn</sub> | [42](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L42) |  |
| `Param::step` <sub>fn</sub> | [45](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L45) |  |
| `ModValues` <sub>pub struct</sub> | [53](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L53) | The values handed to the spectral transform for one frame. |
| `Modulator` <sub>pub struct</sub> | [61](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L61) | Non-stationary parameter generator. |
| `Modulator::from_seed` <sub>pub fn</sub> | [73](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L73) | Build from an explicit 32-byte seed (deterministic; used by tests and by session-key-derived seeding). |
| `Modulator::fill_phase_offsets` <sub>pub fn</sub> | [92](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L92) | The 32 fixed per-bin phase offsets consumer needs are derived from the same stream; expose a helper that fills out with values in [0, 2π). |
| `Modulator::reseed` <sub>pub fn</sub> | [120](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L120) | Roll onto a fresh seed, drawn from the current stream. |
| `Modulator::next_frame` <sub>pub fn</sub> | [131](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L131) | Advance one STFT frame and return the parameters to apply. |
| `Modulator::drop` <sub>fn</sub> | [145](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/modulation.rs#L145) |  |
