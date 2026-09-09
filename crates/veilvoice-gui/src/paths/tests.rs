// SPDX-License-Identifier: GPL-3.0-or-later
//! That the panel reports every location, and reports them truthfully.

use super::*;

#[test]
fn every_entry_is_labelled_and_explained() {
    for entry in all() {
        assert!(!entry.label.is_empty(), "an unlabelled path");
        assert!(
            !entry.note.is_empty(),
            "{} has no note, so a reader is told where without being told what",
            entry.label
        );
    }
}

#[test]
fn no_label_is_used_twice() {
    // Two rows reading "settings" would be two answers to one question, and
    // the reader has no way to tell which is which.
    let mut seen: Vec<&str> = all().iter().map(|w| w.label).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(before, seen.len(), "a label appears twice: {seen:?}");
}

#[test]
fn a_path_that_is_known_is_absolute() {
    // A relative answer is the failure `lock::default_path` exists to avoid:
    // it means a settings file in whatever directory the program started in,
    // and it would be shown here as though it were a location.
    for entry in all() {
        if let Some(path) = entry.path {
            assert!(
                path.is_absolute(),
                "{} is relative: {}",
                entry.label,
                path.display()
            );
        }
    }
}

#[test]
fn everything_kept_between_runs_sits_under_the_settings_folder() {
    // The panel's own claim, checked rather than written: "everything below is
    // in here, and this is what to back up". The program itself is the one
    // entry that is not, because it is not state.
    let all = all();
    let Some(root) = all
        .iter()
        .find(|w| w.label == "settings folder")
        .and_then(|w| w.path.clone())
    else {
        // This system does not say where a configuration directory is, which
        // is a real answer and the reason every path here is an `Option`.
        return;
    };
    for entry in all.iter().filter(|w| w.label != "this program") {
        if let Some(path) = &entry.path {
            assert!(
                path.starts_with(&root),
                "{} is outside the folder this panel says holds everything: {}",
                entry.label,
                path.display()
            );
        }
    }
}

/// **The point of the module.** A location added to this crate tomorrow must
/// appear here, or the About tab goes quietly out of date and nothing notices.
///
/// The list is read out of the crate's own source rather than written down
/// beside it, which is the same rule the rest of this repository follows: a
/// fact in two places is derived in one of them or checked.
#[test]
fn every_place_this_crate_keeps_something_is_reported() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let shown: Vec<String> = all().iter().map(|w| w.label.to_string()).collect();

    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&src).expect("src/") {
        let entry = entry.expect("a directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let module = path.file_stem().unwrap().to_string_lossy().into_owned();
        if module == "paths" {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let has_location =
            source.contains("pub fn default_path()") || source.contains("pub fn default_dir()");
        if !has_location {
            continue;
        }
        // The row is recognised by the module it comes from appearing in the
        // panel's own source, which is where the call to it is.
        let panel = include_str!("../paths.rs");
        if !panel.contains(&format!("crate::{module}::default")) {
            missing.push(module);
        }
    }
    assert!(
        missing.is_empty(),
        "these modules keep something between runs and the About tab does not \
         say where: {missing:?}. Add a row to `paths::all`, or say in the \
         module why it is not a location a person would look for.\nShown: \
         {shown:?}"
    );
}

#[test]
fn the_arrangement_names_the_folder_that_switches_it() {
    // A sentence telling somebody to make a folder is only useful if it says
    // which folder, and the name has to be the one the crypto crate looks for
    // rather than a second copy of it typed here.
    let said = arrangement();
    assert!(
        said.contains(veilvoice_crypto::lock::PORTABLE_DIR),
        "the arrangement does not name the folder: {said}"
    );
    assert!(!said.is_empty());
}

#[test]
fn carrying_over_never_replaces_anything_at_the_destination() {
    // The failure this forbids: installing a portable copy onto a machine
    // somebody else already uses, and overwriting their vault with the one on
    // the stick. Nothing at the destination is replaced, ever.
    let from = tempfile::tempdir().unwrap();
    let into = tempfile::tempdir().unwrap();

    std::fs::write(from.path().join("settings.conf"), "from the stick").unwrap();
    std::fs::create_dir(from.path().join("studio")).unwrap();
    std::fs::write(from.path().join("studio").join("index.veil"), "sealed").unwrap();
    std::fs::write(into.path().join("settings.conf"), "already here").unwrap();

    let said = copy_new_only(from.path(), into.path()).unwrap();

    assert_eq!(
        std::fs::read_to_string(into.path().join("settings.conf")).unwrap(),
        "already here",
        "an existing file was replaced"
    );
    assert_eq!(
        std::fs::read_to_string(into.path().join("studio").join("index.veil")).unwrap(),
        "sealed",
        "a whole directory did not come across"
    );
    assert!(
        said.iter().any(|line| line.contains("left settings.conf")),
        "the report did not say what was left: {said:?}"
    );
    assert!(
        said[0].contains("one thing carried over"),
        "the count is wrong: {said:?}"
    );
}

#[test]
fn carrying_nothing_over_says_so_rather_than_looking_like_success() {
    let from = tempfile::tempdir().unwrap();
    let into = tempfile::tempdir().unwrap();
    let said = copy_new_only(from.path(), into.path()).unwrap();
    assert_eq!(said, vec!["nothing was carried over".to_string()]);
}

#[test]
fn what_was_left_behind_is_still_there_afterwards() {
    // A copy, not a move. Both work afterwards, and the person decides what to
    // do with the stick rather than finding it emptied.
    let from = tempfile::tempdir().unwrap();
    let into = tempfile::tempdir().unwrap();
    std::fs::write(from.path().join("settings.conf"), "kept").unwrap();
    copy_new_only(from.path(), into.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(from.path().join("settings.conf")).unwrap(),
        "kept"
    );
}
