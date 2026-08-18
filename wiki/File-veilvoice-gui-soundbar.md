![soundbar.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-gui/soundbar.svg)

# `crates/veilvoice-gui/src/soundbar.rs`

[[veilvoice-gui|Crate-veilvoice-gui]] &middot; 349 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs)

## Contents

- [The same mark as the website](#the-same-mark-as-the-website)
- [Why it is drawn rather than rendered from a GIF](#why-it-is-drawn-rather-than-rendered-from-a-gif)
- [Cost when it is switched off](#cost-when-it-is-switched-off)
  - [What calls what](#what-calls-what)
  - [Items](#items)

The animated mark: a row of bars that rise and fall.

# The same mark as the website

`website/index.html` draws this in CSS as `.veil` -- a row of `<span>`s with
`@keyframes pulse` taking each between 16% and 82% of the height over
1.9 seconds, each with its own delay so the row ripples rather than pumping
in unison. The left half is drawn in the accent colour and the right half in
the "veiled" secondary, which is the product in one picture and matches the
icon.

This is that, in egui, with the same period, the same height range and the
same delays. Two front-ends showing visibly different marks would be worse
than one showing none.

# Why it is drawn rather than rendered from a GIF

An animated image would be a committed binary blob, and this project's
artwork is generated from source precisely so that nothing in the repository
has to be taken on trust. Sixty lines of shape drawing is auditable; a GIF
is not.

# Cost when it is switched off

With motion disabled the bars are drawn once, at rest, and **no repaint is
requested**. That is the part that matters: an "off" switch that still
schedules a frame every 16 ms has turned the animation off visually and left
the battery cost behind. The caller decides by passing a `Motion`, and the
only way to animate is to ask for it.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_height_fraction["height_fraction"]
    n_draw(["draw<br/>pub"])
    n_colour_for["colour_for"]
    n_draw --> n_colour_for
    n_draw --> n_height_fraction
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `PERIOD` <sub>const</sub> | [37](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L37) | Seconds for one full rise and fall. |
| `DELAYS` <sub>const</sub> | [42](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L42) | Per-bar phase offsets in seconds, matching the animation-delay values in website/index.html. |
| `MIN_FRACTION` <sub>const</sub> | [47](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L47) | Height as a fraction of the available box, matching 16% and 82%. |
| `MAX_FRACTION` <sub>const</sub> | [48](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L48) |  |
| `height_fraction` <sub>fn</sub> | [52](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L52) | How far along its cycle a bar is, in 0..=1, eased the way CSS ease-in-out eases. |
| `draw` <sub>pub fn</sub> | [68](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L68) | Draw the mark at size, returning the response so it can carry a tooltip. |
| `colour_for` <sub>fn</sub> | [114](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-gui/src/soundbar.rs#L114) | The left half in the accent colour, the right in the veiled secondary -- the same split the website and the icon use. |
