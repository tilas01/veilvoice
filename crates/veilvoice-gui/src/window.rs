// SPDX-License-Identifier: GPL-3.0-or-later
//! How big the window opens, and why it is not a constant.
//!
//! # The two failures a single number produces
//!
//! The window used to open at 1100 by 720 always. That is too small for this
//! application: the longest panel is 1288 pixels of content, so it opened on a
//! panel with its bottom third missing, and a reader had to discover the
//! scrollbar to find out there was more.
//!
//! Opening at 1400 by 1000 instead fixes that and breaks something else. A
//! 1366 by 768 laptop is still a common machine, and a window taller than the
//! screen opens with its lower edge, and often its buttons, off the bottom.
//! That looks like a broken program rather than a generous one.
//!
//! So the size is not a constant. It is a preference, clamped to what the
//! screen can actually show, worked out on the first frame when the monitor
//! size is known. On a large display the window opens big enough to read; on a
//! small one it opens as big as fits and scrolls, which is what scrolling is
//! for.
//!
//! # The floor is about width, and it stays where it was
//!
//! Everything is inside one scroll area, so a short window loses nothing.
//! Width is different: the layout is column-based and below roughly 720 across
//! the columns start to overlap. That floor is enforced by the window manager
//! through `with_min_inner_size`, so a person who wants a small window can
//! still drag it down to the floor. Nothing here stops them; it stops the
//! program from *opening* somewhere unreadable, which is a different thing.

/// The size the window opens at when the screen has room for it.
///
/// Measured rather than picked. At 1400 across, the nine tab labels fit with
/// room to spare and the longest panel is 1288 pixels tall; 1000 shows all of
/// every panel but that one, and that one scrolls.
pub const PREFERRED: [f32; 2] = [1400.0, 1000.0];

/// The floor, enforced by the window manager.
pub const MINIMUM: [f32; 2] = [720.0, 520.0];

/// What the window has to leave for the desktop, in pixels.
///
/// A proportion is the wrong model here and was the first thing tried: nine
/// tenths of a 1080-pixel screen is 972, which would shrink the window on the
/// commonest desktop display in the world to avoid a task bar that is 40
/// pixels tall. What the window actually loses is a title bar at the top and
/// a task bar or dock at the bottom, and those are a fixed size whatever the
/// screen is.
///
/// Generous rather than exact, because it cannot be exact: a dock can be
/// enormous, and the cost of over-reserving is a slightly smaller window
/// while the cost of under-reserving is buttons under the task bar.
const CHROME: [f32; 2] = [20.0, 80.0];

/// The size asked for by `--size <W>x<H>`, if one was and it parses.
///
/// A malformed value opens the default window rather than refusing to start.
/// This is a convenience for somebody who wants a bigger window and for the
/// screenshot harness; neither is worth a program that will not open, and a
/// window that is the wrong size says so by being the wrong size.
///
/// Clamped to the minimum the window enforces anyway, so `--size 1x1` gives a
/// usable window instead of one whose columns overlap.
pub fn requested_size() -> Option<[f32; 2]> {
    size_from(std::env::args().skip(1))
}

/// The parsing half, separated from the environment so it can be tested.
///
/// `main` cannot be called from a test and `std::env::args` cannot be set by
/// one, so a parser that reads the environment directly is a parser nothing
/// checks. This takes the arguments instead.
pub fn size_from<I: Iterator<Item = String>>(args: I) -> Option<[f32; 2]> {
    let mut args = args;
    while let Some(arg) = args.next() {
        let value = if let Some(rest) = arg.strip_prefix("--size=") {
            rest.to_string()
        } else if arg == "--size" {
            args.next()?
        } else {
            continue;
        };
        let (width, height) = value.split_once(['x', 'X'])?;
        let width: f32 = width.trim().parse().ok()?;
        let height: f32 = height.trim().parse().ok()?;
        if !width.is_finite() || !height.is_finite() {
            return None;
        }
        return Some([width.max(720.0), height.max(520.0)]);
    }
    None
}

/// The size to open at on this screen, given the monitor and what was asked
/// for on the command line.
///
/// `--size` wins outright, clamped only by the floor: somebody who names a
/// size means it, and the screenshot harness needs exactly the size it asked
/// for. Otherwise the preferred size is used, shrunk to fit the screen.
pub fn opening_size(monitor: Option<[f32; 2]>, asked_for: Option<[f32; 2]>) -> [f32; 2] {
    if let Some(asked) = asked_for {
        return asked;
    }
    let Some(monitor) = monitor else {
        // No monitor size to consult. The preferred size is the better guess:
        // opening too small has been the actual complaint, and a screen small
        // enough for it to matter will report itself on the next frame.
        return PREFERRED;
    };
    [
        PREFERRED[0].min(monitor[0] - CHROME[0]).max(MINIMUM[0]),
        PREFERRED[1].min(monitor[1] - CHROME[1]).max(MINIMUM[1]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Option<[f32; 2]> {
        size_from(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn a_size_is_read_in_either_spelling() {
        assert_eq!(parse(&["--size", "1400x1000"]), Some([1400.0, 1000.0]));
        assert_eq!(parse(&["--size=1400x1000"]), Some([1400.0, 1000.0]));
        assert_eq!(parse(&["--size", "1400X1000"]), Some([1400.0, 1000.0]));
    }

    #[test]
    fn the_size_is_found_beside_other_arguments() {
        assert_eq!(
            parse(&["--tab", "verify", "--size", "1400x1000"]),
            Some([1400.0, 1000.0])
        );
    }

    /// The window enforces a floor of 720x520, so a smaller request is raised
    /// to it rather than producing a window whose columns overlap.
    #[test]
    fn a_size_below_the_minimum_is_raised_to_it() {
        assert_eq!(parse(&["--size", "1x1"]), Some([720.0, 520.0]));
        assert_eq!(parse(&["--size", "1400x10"]), Some([1400.0, 520.0]));
    }

    /// A value that makes no sense opens the default window. The alternative
    /// is a program that will not start over a convenience, and a window of
    /// the wrong size reports itself by being the wrong size.
    #[test]
    fn nonsense_falls_back_to_the_default() {
        for bad in [
            "",
            "1400",
            "widexhigh",
            "1400x",
            "x1000",
            "nanxnan",
            "infxinf",
        ] {
            assert_eq!(parse(&["--size", bad]), None, "{bad:?} should not parse");
        }
        assert_eq!(parse(&["--tab", "file"]), None);
        assert_eq!(parse(&[]), None);
    }

    /// The help text has to name the option, because the manual page is
    /// generated from that text: an option the program accepts and the help
    /// does not mention is one nobody can find.
    #[test]
    fn the_help_text_mentions_the_size_option() {
        let source = include_str!("main.rs");
        let usage = source
            .split("const USAGE: &str = \"\\\n")
            .nth(1)
            .and_then(|rest| rest.split("\";").next())
            .expect("the usage text has to be findable");
        assert!(
            usage.contains("--size"),
            "`--help` does not mention --size, which the program accepts"
        );
    }
}

#[cfg(test)]
mod opening {
    use super::*;

    /// A big screen gets the size the application actually wants.
    #[test]
    fn a_large_screen_gets_the_preferred_size() {
        assert_eq!(opening_size(Some([2560.0, 1440.0]), None), PREFERRED);
        assert_eq!(opening_size(Some([3840.0, 2160.0]), None), PREFERRED);
    }

    /// The machine this is really about: a window taller than the screen opens
    /// with its buttons off the bottom.
    #[test]
    fn a_small_laptop_gets_a_window_that_fits_on_it() {
        let screen = [1366.0, 768.0];
        let size = opening_size(Some(screen), None);
        assert!(
            size[0] <= screen[0] - CHROME[0] && size[1] <= screen[1] - CHROME[1],
            "{size:?} does not leave room for a title bar and a task bar on a \
             {screen:?} screen"
        );
        assert!(size[0] >= MINIMUM[0] && size[1] >= MINIMUM[1]);
    }

    /// 1080p is the commonest desktop screen there is, and the window should
    /// open at its full preferred size on one. A proportional allowance got
    /// this wrong, which is why the allowance is a number of pixels.
    #[test]
    fn the_commonest_screen_of_all_gets_the_full_size() {
        assert_eq!(opening_size(Some([1920.0, 1080.0]), None), PREFERRED);
    }

    /// Never below the floor, however small the screen claims to be. A window
    /// narrower than this has overlapping columns, which is worse than a
    /// window that does not fit.
    #[test]
    fn a_tiny_screen_still_gets_a_readable_window() {
        let size = opening_size(Some([320.0, 200.0]), None);
        assert_eq!(size, MINIMUM);
    }

    /// `--size` is somebody saying what they want, including the screenshot
    /// harness, which needs the exact size or the pictures are not comparable.
    #[test]
    fn an_explicit_size_is_not_second_guessed() {
        assert_eq!(
            opening_size(Some([1366.0, 768.0]), Some([1400.0, 1900.0])),
            [1400.0, 1900.0]
        );
    }

    /// With nothing known about the screen, the preferred size is the answer:
    /// opening too small is the complaint, and the first frame will report the
    /// monitor if it turns out to matter.
    #[test]
    fn an_unknown_screen_gets_the_preferred_size() {
        assert_eq!(opening_size(None, None), PREFERRED);
    }
}
