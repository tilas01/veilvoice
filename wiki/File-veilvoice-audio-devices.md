![devices.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-audio/devices.svg)

# `crates/veilvoice-audio/src/devices.rs`

[[veilvoice-audio|Crate-veilvoice-audio]] &middot; 188 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

Device enumeration and virtual-cable detection.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_looks_virtual["looks_virtual"]
    n_list(["list<br/>pub"])
    n_find_virtual_cable(["find_virtual_cable<br/>pub"])
    n_name_of(["name_of<br/>pub"])
    n_open(["open<br/>pub"])
    n_find_virtual_cable --> n_list
    n_list --> n_looks_virtual
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Direction` <sub>pub enum</sub> | [9](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L9) | Which direction a device carries audio. |
| `DeviceInfo` <sub>pub struct</sub> | [18](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L18) | A device the user can choose. |
| `VIRTUAL_CABLE_HINTS` <sub>const</sub> | [34](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L34) | Name fragments used by the common virtual audio cables. |
| `looks_virtual` <sub>fn</sub> | [46](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L46) |  |
| `list` <sub>pub fn</sub> | [52](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L52) | List the devices available in one direction. |
| `find_virtual_cable` <sub>pub fn</sub> | [85](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L85) | Find the first output device that looks like a virtual audio cable. |
| `name_of` <sub>pub fn</sub> | [95](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L95) | The name of an opened device, or a placeholder when the OS will not say. |
| `open` <sub>pub fn</sub> | [100](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-audio/src/devices.rs#L100) | Look up a device by exact name, or the host default when name is None. |
