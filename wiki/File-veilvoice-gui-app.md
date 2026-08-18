![app.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-gui/app.svg)

# `crates/veilvoice-gui/src/app.rs`

[[veilvoice-gui|Crate-veilvoice-gui]] &middot; 986 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

The VeilVoice desktop application.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_preferred_output["preferred_output"]
    n_preferred_input["preferred_input"]
    n_without_devices["VeilVoiceApp::without_devices"]
    n_default["VeilVoiceApp::default"]
    n_new(["VeilVoiceApp::new<br/>pub"])
    n_config["VeilVoiceApp::config"]
    n_update["VeilVoiceApp::update"]
    n_poll_job["VeilVoiceApp::poll_job"]
    n_settings["VeilVoiceApp::settings"]
    n_file_tab["VeilVoiceApp::file_tab"]
    n_start_job["VeilVoiceApp::start_job"]
    n_live_tab["VeilVoiceApp::live_tab"]
    n_start_live["VeilVoiceApp::start_live"]
    n_poll_watch["VeilVoiceApp::poll_watch"]
    n_watch_indicator["VeilVoiceApp::watch_indicator"]
    n_watch_tab["VeilVoiceApp::watch_tab"]
    n_about_tab["VeilVoiceApp::about_tab"]
    n_device_picker["device_picker"]
    n_field["field"]
    n_meter["meter"]
    n_about_tab --> n_field
    n_default --> n_preferred_input
    n_default --> n_preferred_output
    n_default --> n_without_devices
    n_file_tab --> n_settings
    n_file_tab --> n_start_job
    n_live_tab --> n_device_picker
    n_live_tab --> n_field
    n_live_tab --> n_meter
    n_live_tab --> n_settings
    n_live_tab --> n_start_live
    n_start_job --> n_config
    n_start_live --> n_config
    n_update --> n_about_tab
    n_update --> n_file_tab
    n_update --> n_live_tab
    n_update --> n_poll_job
    n_update --> n_poll_watch
    n_update --> n_watch_indicator
    n_update --> n_watch_tab
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `Tab` <sub>enum</sub> | [14](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L14) | The things VeilVoice does. |
| `JobDone` <sub>enum</sub> | [30](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L30) | Result of a background file job. |
| `VeilVoiceApp` <sub>pub struct</sub> | [41](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L41) | Application state. |
| `preferred_output` <sub>fn</sub> | [95](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L95) | Pick the output to start on: a virtual cable if the machine has one, because routing there is what lets other applications hear the veiled voice at all; otherwise the system default. |
| `preferred_input` <sub>fn</sub> | [104](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L104) | Pick the input to start on: the system default, else whatever is first. |
| `VeilVoiceApp::without_devices` <sub>fn</sub> | [118](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L118) | The application with no devices enumerated. |
| `VeilVoiceApp::default` <sub>fn</sub> | [151](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L151) |  |
| `VeilVoiceApp::new` <sub>pub fn</sub> | [169](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L169) | Build the app, applying theme and fonts to ctx. |
| `VeilVoiceApp::config` <sub>fn</sub> | [184](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L184) |  |
| `VeilVoiceApp::update` <sub>fn</sub> | [198](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L198) |  |
| `VeilVoiceApp::poll_job` <sub>fn</sub> | [302](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L302) |  |
| `VeilVoiceApp::settings` <sub>fn</sub> | [336](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L336) |  |
| `VeilVoiceApp::file_tab` <sub>fn</sub> | [373](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L373) |  |
| `VeilVoiceApp::start_job` <sub>fn</sub> | [453](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L453) |  |
| `VeilVoiceApp::live_tab` <sub>fn</sub> | [509](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L509) |  |
| `VeilVoiceApp::start_live` <sub>fn</sub> | [600](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L600) |  |
| `VeilVoiceApp::poll_watch` <sub>fn</sub> | [614](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L614) | Re-scan on a timer rather than every frame. |
| `VeilVoiceApp::watch_indicator` <sub>fn</sub> | [638](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L638) | The always-visible indicator. |
| `VeilVoiceApp::watch_tab` <sub>fn</sub> | [667](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L667) |  |
| `VeilVoiceApp::about_tab` <sub>fn</sub> | [743](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L743) |  |
| `device_picker` <sub>fn</sub> | [794](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L794) |  |
| `field` <sub>fn</sub> | [819](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L819) |  |
| `meter` <sub>fn</sub> | [826](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/app.rs#L826) |  |
