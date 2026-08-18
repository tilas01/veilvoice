![veilvoice-gui](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-gui.svg)

# veilvoice-gui

> egui/eframe front-end for VeilVoice: Tokyo Night, monospace, three modes.

[[Reference]] &middot; [the same page in the repository](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/README.md)

## Contents

- [How the crate fits together](#how-the-crate-fits-together)
- [The files](#the-files)

The VeilVoice desktop application: an egui/eframe front-end, monospace
throughout — anonymise a file, scramble a microphone live, watch what is
listening, manage the app lock, choose how the app looks, and an about
panel that states the honest scope.

The binary lives in `main.rs`; this library exists so the UI logic can be
unit tested without opening a window.

## How the crate fits together

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_lib(["lib.rs<br/>25 lines"])
    n_main(["main.rs<br/>67 lines"])
    n_app["app.rs<br/>1062 lines"]
    n_prefs["prefs.rs<br/>386 lines"]
    n_reduced_motion["reduced_motion.rs<br/>273 lines"]
    n_security["security.rs<br/>1030 lines"]
    n_settings["settings.rs<br/>618 lines"]
    n_soundbar["soundbar.rs<br/>349 lines"]
    n_theme["theme.rs<br/>650 lines"]
    n_app --> n_security
    n_app --> n_settings
    n_app --> n_soundbar
    n_app --> n_theme
    n_prefs --> n_theme
    n_security --> n_theme
    n_settings --> n_prefs
    n_settings --> n_reduced_motion
    n_settings --> n_soundbar
    n_settings --> n_theme
    n_soundbar --> n_prefs
    n_soundbar --> n_theme
```

## The files

| File | Lines | What it is |
|---|---:|---|
| [[`app.rs`|File-veilvoice-gui-app]] | 1062 | The VeilVoice desktop application. |
| [[`lib.rs`|File-veilvoice-gui-lib]] | 25 | The VeilVoice desktop application: an egui/eframe front-end, monospace throughout — anonymise a file, scramble a microphone live, watch what is listening, manage the app lock, choose how the app looks, and an about panel that states the honest scope. |
| [[`main.rs`|File-veilvoice-gui-main]] | 67 | Entry point for the desktop application: open a window, hand it to veilvoice_gui::VeilVoiceApp, and get out of the way. |
| [[`prefs.rs`|File-veilvoice-gui-prefs]] | 386 | What the user has chosen about how the app looks and moves. |
| [[`reduced_motion.rs`|File-veilvoice-gui-reduced_motion]] | 273 | Whether the operating system has been asked to reduce motion. |
| [[`security.rs`|File-veilvoice-gui-security]] | 1030 | The application lock, and the at-rest encryption of what VeilVoice writes. |
| [[`settings.rs`|File-veilvoice-gui-settings]] | 618 | The settings panel: a menu of pages, each a titled group of choices. |
| [[`soundbar.rs`|File-veilvoice-gui-soundbar]] | 349 | The animated mark: a row of bars that rise and fall. |
| [[`theme.rs`|File-veilvoice-gui-theme]] | 650 | Colour schemes for the desktop app. |
