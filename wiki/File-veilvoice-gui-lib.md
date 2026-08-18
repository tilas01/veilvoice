![lib.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-gui/lib.svg)

# `crates/veilvoice-gui/src/lib.rs`

[[veilvoice-gui|Crate-veilvoice-gui]] &middot; 25 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/lib.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

The VeilVoice desktop application: an egui/eframe front-end, monospace
throughout — anonymise a file, scramble a microphone live, watch what is
listening, manage the app lock, choose how the app looks, and an about
panel that states the honest scope.

The binary lives in `main.rs`; this library exists so the UI logic can be
unit tested without opening a window.

## What calls what

This file defines no functions of its own.

## Items

| Item | Line | Documentation |
|---|---:|---|
| `VERSION` <sub>pub const</sub> | [25](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/lib.rs#L25) | Crate version string, surfaced in the About panel. |
