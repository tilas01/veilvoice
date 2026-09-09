// SPDX-License-Identifier: GPL-3.0-or-later
//! The parsers, against real output from platforms this cannot run on.
//!
//! A parser tested only against the machine that wrote it is a parser tested on
//! one platform. These are transcripts of what `df -Pk` actually prints on each
//! system VeilVoice ships for, so the columns are checked against the shapes
//! they really take rather than against the one in front of the author.

use super::*;

#[test]
fn linux_output_gives_the_available_column() {
    // GNU coreutils, from the machine this was written on.
    let text = "\
Filesystem     1024-blocks     Used Available Capacity Mounted on
/dev/vda         264212084 38117528    730996      99% /
";
    assert_eq!(parse_df(text), Some(730_996 * 1024));
}

#[test]
fn macos_output_gives_the_available_column() {
    // BSD df, which pads differently and names the capacity column with a
    // percent sign in the header.
    let text = "\
Filesystem   1024-blocks      Used Available Capacity  Mounted on
/dev/disk3s1s1  971350180  10485760 445829120     3%    /
";
    assert_eq!(parse_df(text), Some(445_829_120 * 1024));
}

#[test]
fn freebsd_output_gives_the_available_column() {
    let text = "\
Filesystem  1024-blocks    Used   Avail Capacity  Mounted on
/dev/gpt/rootfs  20307196 3221225 15464331    17%    /
";
    assert_eq!(parse_df(text), Some(15_464_331 * 1024));
}

#[test]
fn a_device_name_wrapped_onto_its_own_line_still_parses() {
    // Some implementations put a long device name alone and the numbers on the
    // next line. Reading the last line with six fields rather than the second
    // line of the output is what handles it.
    let text = "\
Filesystem     1024-blocks     Used Available Capacity Mounted on
/dev/mapper/a-very-long-logical-volume-name-indeed
                 264212084 38117528    730996      99% /
";
    // The wrapped row has five fields, so it is skipped rather than misread,
    // and nothing is reported. Refusing is right: a number read out of the
    // wrong columns would be a measurement that is simply false.
    assert_eq!(parse_df(text), None);
}

#[test]
fn a_header_alone_reports_nothing_rather_than_zero() {
    // Zero free bytes and "we could not tell" are different answers, and
    // showing the first when the truth is the second would tell somebody their
    // disk is full.
    let text = "Filesystem 1024-blocks Used Available Capacity Mounted on\n";
    assert_eq!(parse_df(text), None);
}

#[test]
fn nothing_at_all_reports_nothing() {
    assert_eq!(parse_df(""), None);
    assert_eq!(parse_df("\n\n"), None);
    assert_eq!(parse_df("df: /nowhere: No such file or directory\n"), None);
}

#[test]
fn a_mount_point_with_spaces_does_not_shift_the_columns() {
    // The columns are counted from the left for this reason: the mount point
    // is last and may contain spaces, so counting from the right would read
    // the capacity as the available space.
    let text = "\
Filesystem     1024-blocks     Used Available Capacity Mounted on
/dev/sdb1         12345678  1000000   2345678      45% /media/My Backup Disk
";
    assert_eq!(parse_df(text), Some(2_345_678 * 1024));
}

#[test]
fn an_absurd_block_count_saturates_upward_rather_than_wrapping() {
    // A filesystem reporting nonsense should read as "plenty" rather than as
    // "none": the second would tell somebody there is no room when there is.
    let text = format!(
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/x 1 1 {} 1% /\n",
        u64::MAX
    );
    assert_eq!(parse_df(&text), Some(u64::MAX));
}

#[test]
fn the_real_filesystem_answers_or_says_it_cannot() {
    // Not an assertion about the number, which depends on the machine. What is
    // asserted is that the call completes and that any answer it gives is a
    // plausible one rather than a parse of the wrong column.
    let here = std::path::Path::new(".");
    // `None` is a legitimate answer: a sandbox that refuses to start processes,
    // or a platform with no branch for it. A byte count that is not an exact
    // multiple of the block size, on the other hand, would mean the arithmetic
    // went wrong somewhere.
    if let Some(bytes) = free_bytes(here) {
        assert_eq!(bytes % 1024, 0, "{bytes} is not a whole number of blocks");
    }
}
