![lib.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-watch/lib.svg)

# `crates/veilvoice-watch/src/lib.rs`

[[veilvoice-watch|Crate-veilvoice-watch]] &middot; 401 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs)

## Contents

- [Why this belongs in a voice-privacy tool](#why-this-belongs-in-a-voice-privacy-tool)
- [What it can actually see, per platform](#what-it-can-actually-see-per-platform)
- [What calls what](#what-calls-what)
- [Items](#items)

Find out which applications are using your microphone and camera, right now.

## Why this belongs in a voice-privacy tool

VeilVoice protects the audio you choose to send. This answers a different
and more basic question: *is something listening that you did not choose?*
A de-identified voice on a call is worth very little if a second program is
recording the raw microphone at the same time.

Operating systems have grown indicators for this — the orange dot, the
taskbar icon — but they are small, easily missed, and tell you only that
*something* is active, rarely what. This reports the process, its PID and
how long it has held the device.

## What it can actually see, per platform

Detection is honest about its limits, because a monitor that quietly sees
nothing is worse than no monitor at all — it produces false confidence.
`support` reports what the current platform can do before you rely on it.

| Platform | Microphone | Camera | How |
|---|---|---|---|
| Windows | ✅ | ✅ | The same `CapabilityAccessManager` records the OS privacy indicator uses |
| Linux | ✅ | ✅ | `/proc/*/fd` handles open on `/dev/snd/pcm*` and `/dev/video*` |
| macOS | ❌ | ❌ | No public API exposes it; anything claiming otherwise on macOS is guessing |

On Linux you see every process you have permission to inspect. Without root
that means your own; other users' processes are invisible, and that is a
kernel permission boundary rather than something this crate can work around.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#737aa2","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_fmt["DeviceKind::fmt"]
    n_key(["DeviceUse::key<br/>pub"])
    n_held_for(["DeviceUse::held_for<br/>pub"])
    n_support(["support<br/>pub"])
    n_from["Error::from"]
    n_fmt["Error::fmt"]
    n_scan(["scan<br/>pub"])
    n_alert(["Change::alert<br/>pub"])
    n_describe(["DeviceUse::describe<br/>pub"])
    n_new(["Monitor::new<br/>pub"])
    n_current(["Monitor::current<br/>pub"])
    n_poll(["Monitor::poll<br/>pub"])
    n_diff["Monitor::diff"]
    n_poll --> n_diff
    n_poll --> n_scan
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `VERSION` <sub>pub const</sub> | [45](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L45) | Crate version string, surfaced in the About panel. |
| `DeviceKind` <sub>pub enum</sub> | [49](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L49) | The kind of device being used. |
| `DeviceKind::fmt` <sub>fn</sub> | [57](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L57) |  |
| `DeviceUse` <sub>pub struct</sub> | [67](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L67) | One application holding one device. |
| `DeviceUse::key` <sub>pub fn</sub> | [88](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L88) | A stable key for comparing two scans, so an app is not reported as having stopped and restarted when nothing changed. |
| `DeviceUse::held_for` <sub>pub fn</sub> | [93](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L93) | How long this application has held the device. |
| `Support` <sub>pub struct</sub> | [101](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L101) | What detection is possible here. |
| `support` <sub>pub fn</sub> | [115](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L115) | Report what this platform can detect. |
| `Error` <sub>pub enum</sub> | [149](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L149) | Everything that can go wrong here. |
| `Error::from` <sub>fn</sub> | [157](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L157) |  |
| `Error::fmt` <sub>fn</sub> | [163](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L163) |  |
| `scan` <sub>pub fn</sub> | [177](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L177) | Take one snapshot of what is currently using the microphone and camera. |
| `Change` <sub>pub enum</sub> | [194](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L194) | A change between two scans. |
| `Change::alert` <sub>pub fn</sub> | [203](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L203) | A one-line alert suitable for a notification or an overlay. |
| `DeviceUse::describe` <sub>pub fn</sub> | [213](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L213) | name (pid 1234), or just the name when there is no PID. |
| `Monitor` <sub>pub struct</sub> | [226](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L226) | Watches for changes between scans. |
| `Monitor::new` <sub>pub fn</sub> | [232](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L232) | A monitor that has not yet seen anything. |
| `Monitor::current` <sub>pub fn</sub> | [237](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L237) | The most recent snapshot. |
| `Monitor::poll` <sub>pub fn</sub> | [246](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L246) | Scan, and report what changed since the previous call. |
| `Monitor::diff` <sub>fn</sub> | [251](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-watch/src/lib.rs#L251) | The comparison, split out so it can be tested without a real system. |
