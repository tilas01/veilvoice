![wav.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-meta/wav.svg)

# `crates/veilvoice-meta/src/wav.rs`

[[veilvoice-meta|Crate-veilvoice-meta]] &middot; 332 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs)

## Contents

- [Why WAV gets its own path](#why-wav-gets-its-own-path)
- [Whitelist, not blacklist](#whitelist-not-blacklist)
  - [What calls what](#what-calls-what)
  - [Items](#items)

Chunk-level RIFF/WAVE metadata removal.

# Why WAV gets its own path

`lofty` handles tags in every other container, but it cannot remove an
ID3v2 block from a WAV file — the attempt fails with an encoding error and
the tag stays put. Silently leaving metadata in place is exactly the failure
this crate exists to prevent, and WAV is the format VeilVoice writes itself,
so it gets a direct implementation rather than a caveat.

# Whitelist, not blacklist

A RIFF file is a flat list of chunks, and metadata hides in a lot of them:
`LIST`/`INFO` (artist, software, comments), `id3 ` and `ID3 `, `bext`
(broadcast extension — originator, date, even a coding history), `iXML`,
`_PMX` (XMP), `axml`, `cart`. Enumerating those would leave every chunk
nobody thought of, and new ones keep being invented.

So this keeps only the chunks needed to decode the audio and drops
everything else. Anything unrecognised is discarded by default, which is the
right bias for a privacy tool: the worst case is a lost non-essential chunk,
not a leaked identity.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_is_wav(["is_wav<br/>pub"])
    n_clean_wav_bytes(["clean_wav_bytes<br/>pub"])
    n_info_chunk["info_chunk"]
    n_show["show"]
    n_clean_wav_bytes --> n_info_chunk
    n_clean_wav_bytes --> n_is_wav
    n_clean_wav_bytes --> n_show
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `KEEP` <sub>const</sub> | [28](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L28) | Chunks required to interpret the audio. |
| `REALISTIC_INFO` <sub>const</sub> | [35](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L35) | Tags written in Policy::Realistic mode, as LIST/INFO sub-chunks. |
| `is_wav` <sub>pub fn</sub> | [42](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L42) | Whether bytes looks like a RIFF/WAVE file. |
| `clean_wav_bytes` <sub>pub fn</sub> | [47](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L47) | Rewrite a WAV, keeping only the chunks needed to decode it. |
| `info_chunk` <sub>fn</sub> | [131](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L131) | Build a bland LIST/INFO chunk. |
| `show` <sub>fn</sub> | [150](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/wav.rs#L150) |  |
