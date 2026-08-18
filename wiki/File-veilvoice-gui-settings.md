![settings.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-gui/settings.svg)

# `crates/veilvoice-gui/src/settings.rs`

[[veilvoice-gui|Crate-veilvoice-gui]] &middot; 618 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs)

## Contents

- [Why a menu rather than one long list](#why-a-menu-rather-than-one-long-list)
- [Every change applies immediately, and is saved immediately](#every-change-applies-immediately-and-is-saved-immediately)
- [What is deliberately not in here](#what-is-deliberately-not-in-here)
  - [What calls what](#what-calls-what)
  - [Items](#items)

The settings panel: a menu of pages, each a titled group of choices.

# Why a menu rather than one long list

There are three kinds of setting here and they answer different questions:
what the app *looks* like, how it *moves*, and what it does with the files
it writes. Stacked in one column they read as an undifferentiated wall of
tick boxes, and the one that matters most -- at-rest encryption -- ends up
looking exactly as important as the colour scheme. A menu with a page per
group keeps each question next to its own explanation.

# Every change applies immediately, and is saved immediately

There is no "apply" button and no "unsaved changes" state. Both are ways to
lose a choice silently. If saving fails the choice still applies for this
session and the panel says, in the panel, that it could not be remembered
and why -- rather than failing quietly and letting the setting reappear
wrong on the next launch.

# What is deliberately not in here

The app lock and the at-rest passphrase have their own tab and stay there.
A password field sitting between "animations" and "colour scheme" invites
being treated with the same weight, and it is not the same weight.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_default["Settings::default"]
    n_load(["Settings::load<br/>pub"])
    n_motion(["Settings::motion<br/>pub"])
    n_needs_first_run(["Settings::needs_first_run<br/>pub"])
    n_persist["Settings::persist"]
    n_first_run_panel(["Settings::first_run_panel<br/>pub"])
    n_tab(["Settings::tab<br/>pub"])
    n_appearance_page["Settings::appearance_page"]
    n_motion_page["Settings::motion_page"]
    n_storage_page["Settings::storage_page"]
    n_section["section"]
    n_swatches["swatches"]
    n_appearance_page --> n_persist
    n_appearance_page --> n_section
    n_appearance_page --> n_swatches
    n_first_run_panel --> n_persist
    n_motion_page --> n_motion
    n_motion_page --> n_persist
    n_motion_page --> n_section
    n_storage_page --> n_persist
    n_storage_page --> n_section
    n_tab --> n_appearance_page
    n_tab --> n_motion_page
    n_tab --> n_storage_page
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Page` <sub>pub enum</sub> | [35](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L35) | Which page of the settings menu is showing. |
| `Page::ALL` <sub>pub const</sub> | [46](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L46) | Every page, in menu order, with its label and one-line summary. |
| `Settings` <sub>pub struct</sub> | [54](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L54) | The settings tab's own state. |
| `Settings::default` <sub>fn</sub> | [72](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L72) |  |
| `Settings::load` <sub>pub fn</sub> | [90](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L90) | Load preferences from this platform's config directory and apply the chosen theme to ctx. |
| `Settings::motion` <sub>pub fn</sub> | [113](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L113) | How much movement is allowed this frame. |
| `Settings::needs_first_run` <sub>pub fn</sub> | [118](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L118) | Whether the first-run choice has still to be made. |
| `Settings::persist` <sub>fn</sub> | [122](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L122) |  |
| `Settings::first_run_panel` <sub>pub fn</sub> | [140](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L140) | The first-run panel: offered once, with animation already on. |
| `Settings::tab` <sub>pub fn</sub> | [200](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L200) | The settings tab. |
| `Settings::appearance_page` <sub>fn</sub> | [245](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L245) |  |
| `Settings::motion_page` <sub>fn</sub> | [276](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L276) |  |
| `Settings::storage_page` <sub>fn</sub> | [351](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L351) |  |
| `section` <sub>fn</sub> | [412](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L412) | A titled group with a one-line explanation under it. |
| `swatches` <sub>fn</sub> | [420](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/settings.rs#L420) | The active palette, as a row of swatches, so the choice can be seen rather than only read. |
