![veilvoice-cli](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-cli.svg)

# veilvoice-cli

> Command-line interface for VeilVoice: anonymise files, scramble a microphone live, strip metadata, encrypt recordings.

[[Reference]] &middot; [the same page in the repository](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-cli/README.md)

## Contents

- [How the crate fits together](#how-the-crate-fits-together)
- [The files](#the-files)

`veilvoice` — the command-line interface.

Everything VeilVoice does, available without a desktop: it runs over SSH, in
a container, and on machines that have no GUI toolkit at all. The same
engine backs both this and the graphical app.

## How the crate fits together

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_main(["main.rs<br/>1037 lines"])
    n_atrest["atrest.rs<br/>275 lines"]
    n_guard["guard.rs<br/>306 lines"]
    n_lock["lock.rs<br/>239 lines"]
    n_theme["theme.rs<br/>115 lines"]
    n_atrest --> n_theme
    n_guard --> n_atrest
    n_guard --> n_lock
    n_guard --> n_theme
    n_lock --> n_atrest
    n_lock --> n_theme
```

## The files

| File | Lines | What it is |
|---|---:|---|
| [[`atrest.rs`|File-veilvoice-cli-atrest]] | 275 | Encryption at rest for the recordings VeilVoice writes, and the passphrase prompts that feed it. |
| [[`guard.rs`|File-veilvoice-cli-guard]] | 306 | veilvoice guard -- record what VeilVoice's files should be, and check them. |
| [[`lock.rs`|File-veilvoice-cli-lock]] | 239 | veilvoice lock — manage the application lock from the command line. |
| [[`main.rs`|File-veilvoice-cli-main]] | 1037 | veilvoice — the command-line interface. |
| [[`theme.rs`|File-veilvoice-cli-theme]] | 115 | Tokyo Night colouring for the terminal. |
