![stft.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-core/stft.svg)

# `crates/veilvoice-core/src/stft.rs`

[[veilvoice-core|Crate-veilvoice-core]] &middot; 246 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Streaming short-time Fourier transform with overlap-add resynthesis.

Structure follows the classic FIFO/overlap-add pipeline (as popularised by
Bernsee's SMB pitch shifter): samples flow in and out one-for-one with a
fixed latency of `n - hop` samples, and a full frame is analysed/synthesised
every `hop` input samples. The caller supplies a closure that rewrites the
complex spectrum in place, keeping the FFT plumbing and the de-identification
maths cleanly separated.

The closure also receives the raw (unwindowed) analysis frame. Accent
neutralisation needs a time-domain view to track f0 — the FFT resolution at
useful frame sizes is far too coarse for that — and handing over the frame
that produced the spectrum keeps the two perfectly aligned. Its newest `hop`
samples are the tail.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#737aa2","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_new(["StftEngine::new<br/>pub"])
    n_latency_samples(["StftEngine::latency_samples<br/>pub"])
    n_process(["StftEngine::process<br/>pub"])
    n_process_frame["StftEngine::process_frame"]
    n_process --> n_process_frame
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `StftEngine` <sub>pub struct</sub> | [23](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs#L23) | Reusable streaming STFT engine (single channel). |
| `StftEngine::new` <sub>pub fn</sub> | [47](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs#L47) | n must be even; hop must divide evenly for constant overlap-add (typical: hop = n/4). |
| `StftEngine::latency_samples` <sub>pub fn</sub> | [85](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs#L85) | End-to-end algorithmic latency (group delay) in samples. |
| `StftEngine::process` <sub>pub fn</sub> | [92](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs#L92) | Process input into output (equal length). |
| `StftEngine::process_frame` <sub>fn</sub> | [141](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-core/src/stft.rs#L141) |  |
