// SPDX-License-Identifier: GPL-3.0-or-later
//! Where veiled recordings are written, and the encrypted volume that may hold
//! them.
//!
//! **Markers 82, 83 and 84.** [`veilvoice_setup::volumes`] finds what is
//! mounted; this decides what to do about it, remembers the answer, and refuses
//! to write anywhere the user has not confirmed.
//!
//! # A destination, not a mode
//!
//! With no destination chosen, a veiled recording lands beside the file it came
//! from, which is what VeilVoice has always done. With one chosen, it lands in
//! that directory instead, under the same name. That is the whole of the
//! feature: the encryption belongs entirely to Cryptomator or VeraCrypt, and
//! nothing here adds any of its own or claims to.
//!
//! # Why a chosen destination can still be refused
//!
//! Because of VeraCrypt's hidden volumes, and the refusal is the point rather
//! than an inconvenience. [`veilvoice_setup::volumes::Hidden`] carries the
//! whole argument; the short version is that writing into the outer volume of
//! a container that has a hidden one can destroy the hidden data, nothing can
//! tell the two apart from outside, and so the only safe behaviour is to ask
//! the person who knows and to write nothing until they have answered.
//!
//! A destination whose question is unanswered is *not* silently downgraded to
//! writing beside the source. It blocks the job and says why. Falling back
//! quietly would put a veiled recording somewhere unencrypted while its owner
//! believed it was in a vault, which is the exact failure this exists to
//! prevent.
//!
//! # Marker 84: detection will fail, and that is planned for
//!
//! Portable installs, custom mount points, a platform neither tool supports.
//! The answer is not a silent fallback: it is a directory the user picks by
//! hand, a declaration of what kind of volume it is, and the same confirmation
//! before anything is written. A hand-picked destination is treated exactly
//! like a detected one, including the hidden-volume question.
//!
//! # In plain words
//!
//! Lets you send every veiled recording straight into a Cryptomator or
//! VeraCrypt folder, instead of leaving it next to the original.
//!
//! If VeilVoice cannot find your folders it asks you to point at one. Either
//! way it asks, once, whether a VeraCrypt container has a hidden volume in it,
//! and will not write anything until you answer, because writing into the
//! wrong half of one of those can destroy what is hidden inside.

use std::path::{Path, PathBuf};
use veilvoice_setup::volumes::{self, Hidden, Tool, Volume};

/// The chosen place for veiled output, if there is one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Destination {
    /// The volume, once one has been chosen.
    pub volume: Option<Volume>,
}

impl Destination {
    /// Rebuild a destination from what was written to the settings file.
    ///
    /// Anything unrecognised produces no destination rather than a guess. A
    /// settings file that has been edited into nonsense must not decide where
    /// recordings go.
    pub fn from_prefs(dir: &str, tool: &str, hidden: &str) -> Self {
        if dir.is_empty() {
            return Self::default();
        }
        let Some(tool) = Tool::from_key(tool) else {
            return Self::default();
        };
        Self {
            volume: Some(Volume {
                path: PathBuf::from(dir),
                tool,
                hidden: hidden_from_key(hidden),
            }),
        }
    }

    /// The three strings the settings file keeps.
    pub fn to_prefs(&self) -> (String, String, String) {
        match &self.volume {
            None => (String::new(), String::new(), String::new()),
            Some(v) => (
                v.path.display().to_string(),
                v.tool.key().to_string(),
                hidden_key(v.hidden).to_string(),
            ),
        }
    }

    /// Whether a job may start.
    ///
    /// True when nothing is chosen, because writing beside the source is
    /// VeilVoice's ordinary behaviour and needs no permission.
    pub fn ready(&self) -> bool {
        self.volume.as_ref().is_none_or(Volume::ready)
    }

    /// Why a job may not start, for the button's tooltip.
    pub fn blocked(&self) -> Option<&'static str> {
        self.volume.as_ref().and_then(Volume::blocked)
    }

    /// Where a recording should be written, given where it would have gone.
    ///
    /// Keeps the file name and replaces the directory. Returns `default`
    /// untouched when nothing is chosen, and also when the destination is not
    /// ready: a caller that ignores [`Destination::ready`] must not be handed a
    /// vault path it was never cleared to use.
    pub fn place(&self, default: &Path) -> PathBuf {
        let Some(volume) = self.volume.as_ref().filter(|v| v.ready()) else {
            return default.to_path_buf();
        };
        match default.file_name() {
            Some(name) => volume.path.join(name),
            // A path with no file name is not something to be clever about.
            None => default.to_path_buf(),
        }
    }

    /// Whether the volume is still mounted, judged against `mounts`.
    ///
    /// **F-93.** The first version of this asked whether the directory existed,
    /// which is the wrong question in the one direction that matters:
    /// unmounting a volume leaves its mount point behind as an ordinary empty
    /// directory. A locked vault therefore looked fine, and VeilVoice would
    /// have written a veiled recording onto the unencrypted disk while its
    /// owner believed it had gone inside. See
    /// [`veilvoice_setup::volumes::covers`].
    pub fn still_mounted(&self, mounts: &[Volume]) -> bool {
        self.volume
            .as_ref()
            .is_none_or(|v| volumes::covers(mounts, &v.path))
    }
}

/// Stable settings-file spellings for [`Hidden`].
fn hidden_key(hidden: Hidden) -> &'static str {
    match hidden {
        Hidden::Unanswered => "unanswered",
        Hidden::NoHiddenVolume => "none",
        Hidden::IsTheHiddenVolume => "inside",
        Hidden::OuterVolumeOfAHiddenPair => "outer",
    }
}

/// The reverse, defaulting to unanswered.
///
/// Deliberately *not* defaulting to "no hidden volume". An unreadable or
/// hand-edited settings file must not be able to answer a question whose wrong
/// answer destroys data.
fn hidden_from_key(key: &str) -> Hidden {
    match key {
        "none" => Hidden::NoHiddenVolume,
        "inside" => Hidden::IsTheHiddenVolume,
        "outer" => Hidden::OuterVolumeOfAHiddenPair,
        _ => Hidden::Unanswered,
    }
}

/// Everything the window shows about encrypted storage.
pub struct Storage {
    /// The chosen destination.
    pub destination: Destination,
    /// What was mounted when this was last refreshed.
    found: Vec<Volume>,
    /// Whether either tool appears installed, and the evidence.
    presence: Vec<(Tool, String, bool)>,
    /// The folder picker, while it is open.
    choosing: crate::dialog::Pending,
    /// Which tool a hand-picked folder was declared to be.
    hand_picked_tool: Tool,
    /// Whether the chosen folder was there at the last refresh.
    ///
    /// Cached rather than recomputed each frame. Deciding it means reading the
    /// mount table, and doing that sixty times a second for an answer that
    /// changes when somebody unlocks a volume is the shape of stutter the draw
    /// path already refuses in `app.rs`. Writing it into a new module would
    /// have been the same defect in a place that guard test does not look.
    present: bool,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            destination: Destination::default(),
            found: Vec::new(),
            presence: Vec::new(),
            choosing: crate::dialog::Pending::default(),
            hand_picked_tool: Tool::Cryptomator,
            present: true,
        }
    }
}

impl Storage {
    /// Look again at what is installed and mounted.
    ///
    /// Called when the tab is opened rather than every frame: it reads the
    /// mount table, which is cheap but is still a file read, and the draw path
    /// does none. See the guard test in `app.rs`.
    pub fn refresh(&mut self) {
        self.found = volumes::mounted();
        self.present = self.destination.still_mounted(&self.found);
        self.presence = volumes::Tool::ALL
            .iter()
            .map(|tool| {
                let presence = volumes::installed(*tool);
                (*tool, presence.describe(), presence.is_present())
            })
            .collect();
    }

    /// What was found at the last refresh.
    pub fn found(&self) -> &[Volume] {
        &self.found
    }

    /// Whether anything was found at all, which decides whether marker 84's
    /// guided path is the main offer or the fallback.
    pub fn found_nothing(&self) -> bool {
        self.found.is_empty()
    }

    /// Whether the chosen folder was there when this was last refreshed.
    pub fn present(&self) -> bool {
        self.present
    }

    /// Take a folder the user picked by hand, if the picker has answered.
    pub fn take_hand_picked(&mut self) {
        if let Some(path) = self.choosing.taken() {
            self.destination = Destination {
                volume: Some(Volume::found(path, self.hand_picked_tool)),
            };
            self.present = self.destination.still_mounted(&self.found);
        }
    }

    /// Start the folder picker for marker 84's guided path.
    pub fn pick_by_hand(&mut self, tool: Tool) {
        self.hand_picked_tool = tool;
        self.choosing.start(crate::dialog::Ask::Folder);
    }

    /// Choose one of the detected volumes.
    pub fn choose(&mut self, volume: Volume) {
        self.destination = Destination {
            volume: Some(volume),
        };
        self.present = self.destination.still_mounted(&self.found);
    }

    /// Go back to writing beside the source file.
    pub fn clear(&mut self) {
        self.destination = Destination::default();
        self.present = true;
    }

    /// Answer the hidden-volume question for the chosen destination.
    pub fn answer_hidden(&mut self, answer: Hidden) {
        if let Some(volume) = self.destination.volume.as_mut() {
            volume.hidden = answer;
        }
    }

    /// Which tool a hand-picked folder is being declared as.
    pub fn hand_picked_tool(&self) -> Tool {
        self.hand_picked_tool
    }

    /// Whether either tool was detected, for the guided text.
    pub fn presence(&self) -> &[(Tool, String, bool)] {
        &self.presence
    }
}

/// The encrypted-storage panel, drawn on the security tab.
///
/// Returns true when the chosen destination changed, which is the window's cue
/// to have it remembered.
pub fn panel(storage: &mut Storage, ui: &mut egui::Ui) -> bool {
    use crate::theme::palette as p;
    use egui::RichText;

    let before = storage.destination.clone();
    storage.take_hand_picked();

    ui.add_space(16.0);
    ui.separator();
    ui.label(
        RichText::new("Where recordings go")
            .color(p::blue())
            .small(),
    );

    // Read out first, so the closures below borrow the strings rather than the
    // struct they came from.
    let chosen = storage
        .destination
        .volume
        .as_ref()
        .map(|v| (v.path.display().to_string(), v.tool));
    match chosen {
        None => {
            ui.label(RichText::new("beside the file they came from").color(p::muted()));
        }
        Some((shown, tool)) => {
            let mut drop_it = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(shown).color(p::cyan()));
                ui.label(RichText::new(tool.name()).color(p::muted()).small());
                if ui.button("use the ordinary folder again").clicked() {
                    drop_it = true;
                }
            });
            if drop_it {
                storage.clear();
            }
            if !storage.present() {
                ui.label(
                    RichText::new(
                        "that folder is not there now. If the volume is locked, unlock it \
                         in its own program before veiling anything, or VeilVoice will \
                         write outside it.",
                    )
                    .color(p::red()),
                );
            }
            if let Some(why) = storage.destination.blocked() {
                ui.label(RichText::new(why).color(p::yellow()));
            }
        }
    }

    // Marker 83. Asked for VeraCrypt and never for Cryptomator, and asked
    // before anything is written rather than after something goes wrong.
    let needs_answer = storage
        .destination
        .volume
        .as_ref()
        .is_some_and(|v| v.tool == Tool::VeraCrypt);
    if needs_answer {
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Does this container have a hidden volume inside it? VeilVoice cannot \
                 tell, and nothing can: that is what hidden volumes are for. Writing \
                 into the outer volume of a pair can destroy what is hidden in it.",
            )
            .color(p::fg()),
        );
        let current = storage
            .destination
            .volume
            .as_ref()
            .map(|v| v.hidden)
            .unwrap_or(Hidden::Unanswered);
        let mut choice = current;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut choice, Hidden::NoHiddenVolume, "no hidden volume");
            ui.selectable_value(
                &mut choice,
                Hidden::IsTheHiddenVolume,
                "this is the hidden one",
            );
            ui.selectable_value(
                &mut choice,
                Hidden::OuterVolumeOfAHiddenPair,
                "this is the outer one",
            );
        });
        if choice != current {
            storage.answer_hidden(choice);
        }
    }

    ui.add_space(8.0);
    if ui.button("look again").clicked() {
        storage.refresh();
    }

    for volume in storage.found().to_vec() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(volume.path.display().to_string()).color(p::fg()));
            ui.label(RichText::new(volume.tool.name()).color(p::muted()).small());
            if ui.button("write here").clicked() {
                storage.choose(volume.clone());
            }
        });
    }

    // Marker 84. The guided path, which is the main offer when nothing was
    // found rather than a footnote under a list of nothing.
    if storage.found_nothing() {
        ui.label(
            RichText::new("no mounted Cryptomator vault or VeraCrypt volume was found")
                .color(p::muted()),
        );
        for (tool, evidence, present) in storage.presence().to_vec() {
            ui.label(
                RichText::new(format!("{}: {evidence}", tool.name()))
                    .color(if present { p::green() } else { p::muted() })
                    .small(),
            );
        }
        ui.label(
            RichText::new(
                "If one is open and VeilVoice did not see it, point at it by hand. \
                 VeilVoice never opens or closes these itself and never asks for their \
                 password: unlock it in its own program first.",
            )
            .color(p::muted())
            .small(),
        );
        ui.horizontal(|ui| {
            for tool in Tool::ALL {
                if ui
                    .button(format!("choose a {} folder…", tool.name()))
                    .clicked()
                {
                    storage.pick_by_hand(*tool);
                }
            }
        });
    }

    ui.add_space(6.0);
    ui.label(
        RichText::new(volumes::DISK_ADVICE)
            .color(p::muted())
            .small(),
    );

    storage.destination != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(tool: Tool, hidden: Hidden) -> Destination {
        Destination {
            volume: Some(Volume {
                path: PathBuf::from("/media/veracrypt1"),
                tool,
                hidden,
            }),
        }
    }

    /// The panel draws every frame, so nothing in it may touch the disk.
    /// Deciding whether the vault is mounted means reading the mount table, and
    /// the first version of this module did it from the panel: sixty reads a
    /// second for an answer
    /// that changes when somebody unlocks a volume. `app.rs` already refuses
    /// this in its own draw path; a new module is where that guard does not
    /// look, which is exactly why it is worth repeating here.
    #[test]
    fn the_panel_asks_the_disk_nothing() {
        let source = include_str!("storage.rs").replace("\r\n", "\n");
        let start = source.find("pub fn panel(").expect("the panel exists");
        let end = source[start..]
            .find("\n#[cfg(test)]")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let body: String = source[start..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "still_mounted(",
            "try_exists",
            ".exists()",
            "read_to_string",
            "std::fs::",
        ] {
            assert!(
                !body.contains(forbidden),
                "the panel calls {forbidden:?}, which asks the disk once per frame"
            );
        }
    }

    #[test]
    fn no_destination_writes_where_veilvoice_always_did() {
        let d = Destination::default();
        assert!(d.ready(), "the ordinary path needs no permission");
        assert!(d.blocked().is_none());
        assert_eq!(
            d.place(Path::new("/home/u/talk.veiled.wav")),
            Path::new("/home/u/talk.veiled.wav")
        );
    }

    #[test]
    fn a_chosen_vault_keeps_the_file_name_and_replaces_the_folder() {
        let d = vault(Tool::VeraCrypt, Hidden::NoHiddenVolume);
        assert_eq!(
            d.place(Path::new("/home/u/talk.veiled.wav")),
            Path::new("/media/veracrypt1/talk.veiled.wav")
        );
    }

    /// Marker 83's whole point, from the other side: a caller that forgets to
    /// check `ready` must not be handed the vault path anyway.
    #[test]
    fn an_unanswered_destination_never_yields_a_vault_path() {
        let d = vault(Tool::VeraCrypt, Hidden::Unanswered);
        assert!(!d.ready());
        assert!(d.blocked().is_some());
        assert_eq!(
            d.place(Path::new("/home/u/talk.veiled.wav")),
            Path::new("/home/u/talk.veiled.wav"),
            "an unanswered destination must not place a file inside it"
        );
    }

    #[test]
    fn the_outer_volume_of_a_hidden_pair_is_refused_here_too() {
        let d = vault(Tool::VeraCrypt, Hidden::OuterVolumeOfAHiddenPair);
        assert!(!d.ready());
        assert!(d.blocked().expect("a reason").contains("destroy"));
    }

    #[test]
    fn a_cryptomator_vault_needs_no_answer() {
        let d = vault(Tool::Cryptomator, Hidden::Unanswered);
        assert!(d.ready());
        assert_eq!(
            d.place(Path::new("/home/u/talk.veiled.wav")),
            Path::new("/media/veracrypt1/talk.veiled.wav")
        );
    }

    #[test]
    fn a_destination_round_trips_through_the_settings_file() {
        let original = vault(Tool::VeraCrypt, Hidden::IsTheHiddenVolume);
        let (dir, tool, hidden) = original.to_prefs();
        assert_eq!(Destination::from_prefs(&dir, &tool, &hidden), original);

        let none = Destination::default();
        let (dir, tool, hidden) = none.to_prefs();
        assert_eq!(Destination::from_prefs(&dir, &tool, &hidden), none);
    }

    /// A settings file somebody edited must not be able to answer the one
    /// question whose wrong answer destroys data.
    #[test]
    fn a_nonsense_settings_file_leaves_the_question_unanswered() {
        let d = Destination::from_prefs("/media/veracrypt1", "veracrypt", "definitely-fine");
        assert_eq!(
            d.volume.as_ref().expect("a volume").hidden,
            Hidden::Unanswered
        );
        assert!(!d.ready());

        // And an unrecognised tool produces no destination at all.
        assert_eq!(
            Destination::from_prefs("/media/x", "bitlocker", "none"),
            Destination::default()
        );
    }

    /// A vault that has been locked since it was chosen is a directory that is
    /// no longer encrypted, or is gone.
    /// F-93. A locked vault leaves its mount point behind as an ordinary
    /// directory, so existence says yes when the honest answer is no.
    #[test]
    fn a_vault_that_is_no_longer_mounted_is_noticed() {
        let d = vault(Tool::VeraCrypt, Hidden::NoHiddenVolume);
        let mounted = vec![Volume::found(
            PathBuf::from("/media/veracrypt1"),
            Tool::VeraCrypt,
        )];
        assert!(d.still_mounted(&mounted));

        // The directory can still be there. Nothing is mounted on it.
        let empty: Vec<Volume> = Vec::new();
        assert!(
            !d.still_mounted(&empty),
            "an unmounted volume must not read as present just because its \
             mount point is still a directory"
        );

        // No destination is always fine: writing beside the source needs no
        // volume at all.
        assert!(Destination::default().still_mounted(&empty));
    }

    /// A folder chosen inside a mounted vault is inside it.
    #[test]
    fn a_folder_within_a_mounted_vault_counts_as_mounted() {
        let inside = Destination {
            volume: Some(Volume {
                path: PathBuf::from("/home/u/Vault/recordings"),
                tool: Tool::Cryptomator,
                hidden: Hidden::Unanswered,
            }),
        };
        let mounted = vec![Volume::found(
            PathBuf::from("/home/u/Vault"),
            Tool::Cryptomator,
        )];
        assert!(inside.still_mounted(&mounted));
    }
}
