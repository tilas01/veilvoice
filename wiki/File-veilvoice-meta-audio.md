![audio.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-meta/audio.svg)

# `crates/veilvoice-meta/src/audio.rs`

[[veilvoice-meta|Crate-veilvoice-meta]] &middot; 240 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/audio.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Audio tag removal and replacement.

Tags are handled through `lofty`, which understands ID3v1/ID3v2, Vorbis
comments, MP4 atoms and APE, so one code path covers every format VeilVoice
imports. Only the tag blocks are rewritten — the audio stream is never
re-encoded, so cleaning a file is lossless.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_read_head["read_head"]
    n_clean_audio_file(["clean_audio_file<br/>pub"])
    n_clean_audio_tags(["clean_audio_tags<br/>pub"])
    n_clean_audio_file --> n_read_head
    n_clean_audio_tags --> n_read_head
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `read_head` <sub>fn</sub> | [19](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/audio.rs#L19) | Read just enough of a file to identify its container. |
| `REALISTIC` <sub>const</sub> | [33](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/audio.rs#L33) | Tags written in Policy::Realistic mode. |
| `clean_audio_file` <sub>pub fn</sub> | [43](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/audio.rs#L43) | Strip or replace the tags of an audio file, in place. |
| `clean_audio_tags` <sub>pub fn</sub> | [96](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-meta/src/audio.rs#L96) | Report which tag blocks a file carries, without modifying it. |
