![tests.rs](https://raw.githubusercontent.com/tilas01/veilvoice/main/assets/banners/veilvoice-verify/tests.svg)

# `crates/veilvoice-verify/src/tests.rs`

[[veilvoice-verify|Crate-veilvoice-verify]] &middot; 131 lines &middot; [read the source](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs)

## Contents

- [What calls what](#what-calls-what)
- [Items](#items)

> This file has no `//!` module documentation yet.

## What calls what

```mermaid
%%{init: {"theme":"base","themeVariables":{"background":"#1a1b26","primaryColor":"#1f2335","primaryTextColor":"#c0caf5","primaryBorderColor":"#7aa2f7","secondaryColor":"#16161e","tertiaryColor":"#16161e","lineColor":"#565f89","textColor":"#c0caf5","mainBkg":"#1f2335","nodeBorder":"#7aa2f7","clusterBkg":"#16161e","clusterBorder":"#2f3549","fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace","fontSize":"14px"}}}%%
flowchart TD
    n_the_embedded_key_parses_and_is_the_expected_one["the_embedded_key_parses_and_i…"]
    n_the_embedded_key_carries_no_email_address["the_embedded_key_carries_no_e…"]
    n_the_fingerprint_constant_is_written_out_not_computed["the_fingerprint_constant_is_w…"]
    n_a_hash_is_found_by_its_file_name["a_hash_is_found_by_its_file_n…"]
    n_a_binary_mode_star_is_not_part_of_the_name["a_binary_mode_star_is_not_par…"]
    n_a_file_that_is_not_listed_is_not_found["a_file_that_is_not_listed_is_…"]
    n_a_name_that_merely_contains_the_wanted_one_does_not_match["a_name_that_merely_contains_t…"]
    n_blank_and_comment_lines_are_skipped["blank_and_comment_lines_are_s…"]
    n_a_malformed_line_is_skipped_rather_than_panicking["a_malformed_line_is_skipped_r…"]
    n_digests_compare_case_insensitively_and_ignore_surrounding_space["digests_compare_case_insensit…"]
    n_a_signature_that_is_not_openpgp_is_refused["a_signature_that_is_not_openp…"]
    n_an_empty_signature_is_refused["an_empty_signature_is_refused"]
    n_an_armoured_block_that_is_not_a_signature_is_refused["an_armoured_block_that_is_not…"]
```

## Items

| Item | Line | Documentation |
|---|---:|---|
| `the_embedded_key_parses_and_is_the_expected_one` <sub>fn</sub> | [12](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L12) |  |
| `the_embedded_key_carries_no_email_address` <sub>fn</sub> | [18](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L18) |  |
| `the_fingerprint_constant_is_written_out_not_computed` <sub>fn</sub> | [33](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L33) |  |
| `a_hash_is_found_by_its_file_name` <sub>fn</sub> | [46](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L46) |  |
| `a_binary_mode_star_is_not_part_of_the_name` <sub>fn</sub> | [58](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L58) |  |
| `a_file_that_is_not_listed_is_not_found` <sub>fn</sub> | [70](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L70) |  |
| `a_name_that_merely_contains_the_wanted_one_does_not_match` <sub>fn</sub> | [76](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L76) |  |
| `blank_and_comment_lines_are_skipped` <sub>fn</sub> | [83](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L83) |  |
| `a_malformed_line_is_skipped_rather_than_panicking` <sub>fn</sub> | [92](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L92) |  |
| `digests_compare_case_insensitively_and_ignore_surrounding_space` <sub>fn</sub> | [103](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L103) |  |
| `a_signature_that_is_not_openpgp_is_refused` <sub>fn</sub> | [112](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L112) |  |
| `an_empty_signature_is_refused` <sub>fn</sub> | [119](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L119) |  |
| `an_armoured_block_that_is_not_a_signature_is_refused` <sub>fn</sub> | [125](https://github.com/tilas01/veilvoice/blob/main/crates/veilvoice-verify/src/tests.rs#L125) |  |
