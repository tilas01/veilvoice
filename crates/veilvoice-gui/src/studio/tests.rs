// SPDX-License-Identifier: GPL-3.0-or-later
//! What the Studio has to get right, tested without a window.
//!
//! The drawing needs a running egui context and is checked by the screenshot
//! run. Everything here is the part that would still be wrong if the drawing
//! were perfect: the state machine, the wiping, and the two formatters that
//! turn stored numbers into what a person reads.

use super::*;

#[test]
fn a_new_studio_is_shut_and_stays_shut_until_both_are_given() {
    let studio = Studio::default();
    assert!(!studio.is_open(), "a fresh Studio must not be open");
    assert!(!studio.is_recording());
    assert!(matches!(studio.phase(), Phase::Shut));
}

#[test]
fn one_passphrase_alone_never_derives_a_key() {
    // The property the whole vault rests on, asserted here as well as in the
    // crypto crate, because this is the layer a user actually reaches: if this
    // tab ever passed an empty half through, the vault would degrade to one
    // secret and nothing in the window would say so.
    let mut app = String::from("app-lock");
    let mut nothing = String::new();
    let one = into_secret(&mut app);
    let none = into_secret(&mut nothing);
    assert!(StudioKey::derive(&one, &none).is_err());
    assert!(StudioKey::derive(&none, &one).is_err());
}

#[test]
fn taking_a_passphrase_wipes_the_buffer_it_came_from() {
    let mut typed = String::from("a passphrase somebody typed");
    let secret = into_secret(&mut typed);
    assert_eq!(secret.expose(), b"a passphrase somebody typed");
    assert!(
        typed.is_empty(),
        "the typing buffer still holds the passphrase: {typed:?}"
    );
}

#[test]
fn closing_forgets_the_vault_the_listing_and_both_entries() {
    let mut studio = Studio::default();
    studio.app_entry.push_str("app");
    studio.rest_entry.push_str("rest");
    studio.entries.push(Entry {
        id: "abc".into(),
        name: "a name".into(),
        made: 0,
        bytes: 10,
    });
    studio.selected = Some("abc".into());

    studio.close();

    assert!(
        studio.app_entry.is_empty(),
        "the app entry survived closing"
    );
    assert!(
        studio.rest_entry.is_empty(),
        "the at-rest entry survived closing"
    );
    assert!(studio.entries.is_empty(), "the listing survived closing");
    assert!(studio.selected.is_none());
    assert!(!studio.is_open());
}

#[test]
fn a_length_reads_as_minutes_and_seconds() {
    assert_eq!(length(0.0), "0:00");
    assert_eq!(length(9.4), "0:09");
    assert_eq!(length(60.0), "1:00");
    assert_eq!(length(61.9), "1:01");
    assert_eq!(length(3_600.0), "60:00");
    // A negative length is not a thing, and must not print as one.
    assert_eq!(length(-5.0), "0:00");
    // Neither can reach this from `wav_shape`, whose numerator is a `u32` and
    // whose denominator is at least one, so both are finite. Asserted anyway:
    // `length` is public, and a formatter that panics on a value it was never
    // given is a trap for the next caller rather than a safeguard.
    assert_eq!(length(f64::NAN), "0:00");
    assert!(!length(f64::INFINITY).is_empty());
    assert_eq!(length(f64::NEG_INFINITY), "0:00");
}

#[test]
fn a_size_never_reads_as_zero() {
    // A recording that exists is never "0 KiB". Somebody reading that in a
    // listing concludes the recording is empty, and it is not.
    assert_eq!(size(1), "1 KiB");
    assert_eq!(size(0), "1 KiB");
    assert_eq!(size(2048), "2 KiB");
    assert_eq!(size(1024 * 1024), "1.0 MiB");
    assert_eq!(size(3 * 1024 * 1024 / 2), "1.5 MiB");
}

#[test]
fn the_count_is_written_rather_than_printed_with_an_s() {
    assert_eq!(counted(1), "one recording");
    assert_eq!(counted(0), "0 recordings");
    assert_eq!(counted(7), "7 recordings");
}

#[test]
fn a_stored_date_reads_as_the_date_it_was() {
    // Known instants, checked against the calendar rather than against this
    // function's own output.
    assert_eq!(made_on(0), "1970-01-01");
    assert_eq!(made_on(1_700_000_000), "2023-11-14");
    // A leap day, which is where a hand-written civil-date conversion goes
    // wrong if it is going to.
    assert_eq!(made_on(1_709_164_800), "2024-02-29");
    assert_eq!(made_on(1_709_251_200), "2024-03-01");
    // The turn of a century that is not a leap year.
    assert_eq!(made_on(4_102_444_800), "2100-01-01");
    // Before the epoch, which a naive division gets wrong by a day.
    assert_eq!(made_on(-1), "1969-12-31");
}

#[test]
fn the_vault_lives_beside_the_lock_rather_than_somewhere_of_its_own() {
    // Where it is matters: a vault in a second location is a second thing to
    // find, back up and lose.
    if let (Some(vault), Some(lock)) = (default_dir(), veilvoice_crypto::lock::default_path()) {
        assert_eq!(
            vault.parent(),
            lock.parent(),
            "the vault is not beside the lock file"
        );
        assert_eq!(vault.file_name().unwrap(), "studio");
    }
}

#[test]
fn a_name_cannot_write_outside_the_folder_that_was_chosen() {
    // A take is called whatever somebody typed, and here that name reaches a
    // path. This is the whole reason `safe_stem` exists.
    assert_eq!(safe_stem("../../etc/passwd"), "etc-passwd");
    assert_eq!(safe_stem("a/b"), "a-b");
    assert_eq!(safe_stem("..\\..\\windows"), "windows");
    assert_eq!(safe_stem("/absolute"), "absolute");
    for awkward in ["../../etc/passwd", "a/b", "..\\..\\windows", "/absolute"] {
        let stem = safe_stem(awkward);
        assert!(!stem.contains('/'), "{stem:?} still has a separator");
        assert!(!stem.contains('\\'), "{stem:?} still has a separator");
        assert!(!stem.contains(".."), "{stem:?} can still climb");
    }
}

#[test]
fn a_name_that_is_all_punctuation_still_makes_a_file() {
    // Empty is not a file name, and neither is a string of dashes.
    assert_eq!(safe_stem(""), "take");
    assert_eq!(safe_stem("..."), "take");
    assert_eq!(safe_stem("///"), "take");
    assert_eq!(safe_stem("   "), "take");
}

#[test]
fn a_very_long_name_is_shortened_rather_than_refused() {
    // Some systems refuse a path component past 255 bytes. The name is a label
    // and the full one is still in the vault, so shortening loses nothing.
    let stem = safe_stem(&"a".repeat(500));
    assert_eq!(stem.chars().count(), 60);
}

#[test]
fn an_ordinary_name_is_left_recognisable() {
    // Cleaning must not make every file called "take": somebody has to be able
    // to find what they exported.
    assert_eq!(safe_stem("interview-2"), "interview-2");
    assert_eq!(safe_stem("Ada and Grace"), "Ada-and-Grace");
}

#[test]
fn a_wav_header_gives_up_its_rate_and_length() {
    // 48 kHz, mono, 16-bit, one second of audio.
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36u32 + 96_000).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // channels
    wav.extend_from_slice(&48_000u32.to_le_bytes());
    wav.extend_from_slice(&96_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&96_000u32.to_le_bytes());
    wav.resize(44 + 96_000, 0);

    let (rate, seconds) = wav_shape(&wav).expect("a canonical header should read");
    assert_eq!(rate, 48_000);
    assert!((seconds - 1.0).abs() < 1e-9, "{seconds}");
}

#[test]
fn something_that_is_not_a_wav_is_refused_rather_than_guessed() {
    // A length guessed from the wrong header puts every subtitle in the wrong
    // place and makes the video the wrong length.
    assert!(wav_shape(b"").is_none());
    assert!(wav_shape(b"RIFF").is_none());
    assert!(wav_shape(&[0u8; 44]).is_none());
    // Right length, right magic, but a rate of zero: a division waiting to
    // happen.
    let mut zero_rate = vec![0u8; 44];
    zero_rate[0..4].copy_from_slice(b"RIFF");
    zero_rate[8..12].copy_from_slice(b"WAVE");
    assert!(wav_shape(&zero_rate).is_none());
}

#[test]
fn a_plan_for_a_take_is_one_speaker_for_the_whole_of_it() {
    let plan = plan_for("Ada", 12.5).expect("a plan");
    assert_eq!(plan.speakers().len(), 1);
    assert_eq!(plan.speakers()[0].name, "Ada");
    assert_eq!(plan.turns().len(), 1);
    assert_eq!(plan.turns()[0].start, 0.0);
    assert_eq!(plan.turns()[0].end, 12.5);
    assert_eq!(plan.turns()[0].speaker, 0);
    assert_eq!(plan.title.as_deref(), Some("Ada"));
}

#[test]
fn what_each_render_choice_asks_for() {
    assert!(Render::Preview.wants_page() && !Render::Preview.wants_video());
    assert!(!Render::Video.wants_page() && Render::Video.wants_video());
    assert!(Render::Both.wants_page() && Render::Both.wants_video());
}

#[test]
fn sixteen_bit_samples_come_back_in_range() {
    let mut wav = vec![0u8; 44];
    wav.extend_from_slice(&i16::MAX.to_le_bytes());
    wav.extend_from_slice(&i16::MIN.to_le_bytes());
    wav.extend_from_slice(&0i16.to_le_bytes());
    let pcm = pcm16(&wav);
    assert_eq!(pcm.len(), 3);
    assert!(pcm[0] > 0.99 && pcm[0] <= 1.0, "{}", pcm[0]);
    assert_eq!(pcm[1], -1.0);
    assert_eq!(pcm[2], 0.0);
    // A trailing odd byte is dropped rather than read past.
    let pcm = pcm16(&[vec![0u8; 44], vec![1u8]].concat());
    assert!(pcm.is_empty());
}

/// A canonical mono 16-bit WAV of `seconds`, with something audible in it.
fn a_wav(seconds: f64) -> Vec<u8> {
    let rate = 48_000u32;
    let frames = (rate as f64 * seconds) as usize;
    let data = frames * 2;
    let mut wav = Vec::with_capacity(44 + data);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data as u32).to_le_bytes());
    for i in 0..frames {
        let v = ((i as f32 / 90.0).sin() * 12_000.0) as i16;
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav
}

/// A Studio with an open vault holding one take.
fn studio_with_a_take(name: &str) -> (tempfile::TempDir, Studio, String) {
    use veilvoice_crypto::studio::{Studio as Vault, StudioKey};

    let dir = tempfile::tempdir().expect("a temporary directory");
    let secret = |b: &[u8]| {
        let mut copy = b.to_vec();
        veilvoice_crypto::Secret::new(&mut copy)
    };
    let key = StudioKey::derive(&secret(b"app-lock"), &secret(b"at-rest")).unwrap();
    let vault = Vault::open(dir.path().join("vault"), key).unwrap();
    let entry = vault.store(name, 1_700_000_000, &a_wav(2.0)).unwrap();

    let id = entry.id.clone();
    let studio = Studio {
        entries: vault.list().unwrap(),
        vault: Some(vault),
        ..Default::default()
    };
    (dir, studio, id)
}

#[test]
fn a_preview_writes_the_audio_the_page_and_the_captions() {
    let (dir, mut studio, id) = studio_with_a_take("interview one");
    let into = dir.path().join("out");
    std::fs::create_dir_all(&into).unwrap();

    studio.export(&id, Render::Preview, &into);

    let wav = into.join("interview-one.wav");
    let html = into.join("interview-one.html");
    let vtt = into.join("interview-one.vtt");
    assert!(wav.is_file(), "no audio: {:?}", studio.message);
    assert!(html.is_file(), "no page: {:?}", studio.message);
    assert!(vtt.is_file(), "no captions: {:?}", studio.message);

    // No video was asked for and none was written.
    assert!(!into.join("interview-one.mp4").exists());

    let page = std::fs::read_to_string(&html).unwrap();
    // The captions are carried, not fetched: a page opened from a folder
    // cannot load a track from the file beside it.
    assert!(
        page.contains("data:text/vtt;base64,"),
        "captions are not inlined"
    );
    assert!(!page.contains("src=\"interview-one.vtt\""));
    // The audio is referenced by relative name, so the files move together.
    assert!(page.contains("interview-one.wav"));
    assert!(
        !page.contains(&into.display().to_string()),
        "an absolute path leaked in"
    );
    // And the speaker is the take, named.
    assert!(page.contains("interview one"));

    // What was said afterwards has to include the warning, because this is the
    // moment a sealed thing became an unsealed one.
    let (said, _) = studio.message.clone().expect("something should be said");
    assert!(
        said.contains("None of it is sealed"),
        "the moment a sealed thing becomes an unsealed one has to be said, and \
         this said: {said}"
    );
    assert!(said.contains("interview-one.wav"), "{said}");
}

#[test]
fn a_take_whose_name_is_a_path_is_written_inside_the_chosen_folder() {
    let (dir, mut studio, id) = studio_with_a_take("../escape");
    let into = dir.path().join("out");
    std::fs::create_dir_all(&into).unwrap();

    studio.export(&id, Render::Preview, &into);

    assert!(
        into.join("escape.wav").is_file(),
        "not written where it was asked for: {:?}",
        studio.message
    );
    // Nothing landed beside the chosen folder.
    assert!(
        !dir.path().join("escape.wav").exists(),
        "the name climbed out of the folder it was given"
    );
}

#[test]
fn exporting_something_that_is_not_in_the_vault_writes_nothing() {
    let (dir, mut studio, _id) = studio_with_a_take("a take");
    let into = dir.path().join("out");
    std::fs::create_dir_all(&into).unwrap();

    studio.export("0123456789abcdef", Render::Both, &into);

    assert_eq!(
        std::fs::read_dir(&into).unwrap().count(),
        0,
        "something was written for a take that is not there"
    );
}

#[test]
fn without_ffmpeg_the_command_is_printed_rather_than_the_video_promised() {
    // VeilVoice does not ship or install ffmpeg, so the honest answer when it
    // is missing is the exact command, which is what the command line gives.
    // This runs whichever way the machine is set up: with ffmpeg the video is
    // written, without it the command is offered, and neither is a silent
    // nothing.
    let (dir, mut studio, id) = studio_with_a_take("a take");
    let into = dir.path().join("out");
    std::fs::create_dir_all(&into).unwrap();

    studio.export(&id, Render::Video, &into);
    let (said, _) = studio.message.clone().expect("something should be said");

    if veilvoice_video::ffmpeg::found().is_some() {
        assert!(into.join("a-take.mp4").is_file(), "{said}");
    } else {
        assert!(
            said.contains("ffmpeg") && said.contains("does not ship"),
            "the missing tool has to be named, and this said: {said}"
        );
        assert!(
            said.contains("-i") || said.contains("ffmpeg "),
            "the command has to be there to copy, and this said: {said}"
        );
        // The audio was still written: it is the input the printed command
        // needs, so offering the command and not the file would be useless.
        assert!(into.join("a-take.wav").is_file(), "{said}");
    }
}

#[test]
fn asking_where_to_put_it_does_not_block_the_window() {
    // The export button opens a folder picker. Opened the blocking way it
    // freezes the window until it is answered, which is what `dialog` exists
    // to avoid; this checks the tab goes through it rather than around it.
    let (_dir, mut studio, id) = studio_with_a_take("a take");
    assert!(!studio.picker.is_open());

    studio.apply(Act::Export(id.clone(), Render::Both));

    assert!(
        studio.picker.is_open(),
        "the export did not open a picker through `dialog`"
    );
    assert_eq!(
        studio.choosing,
        Some((id, Render::Both)),
        "the picker was opened without recording what it is for, so its answer \
         would arrive with nothing to do"
    );
}

#[test]
fn a_picker_open_when_the_vault_shuts_cannot_export_afterwards() {
    // Locking the window closes the vault. An answer arriving after that must
    // not export from a vault nobody has opened.
    let (_dir, mut studio, id) = studio_with_a_take("a take");
    studio.apply(Act::Export(id, Render::Preview));
    assert!(studio.choosing.is_some());

    studio.close();

    assert!(
        studio.choosing.is_none(),
        "the pending export outlived the vault it was going to read from"
    );
    assert!(!studio.is_open());
}

#[test]
fn playing_a_take_that_is_not_there_says_so_and_starts_nothing() {
    let (_dir, mut studio, _id) = studio_with_a_take("a take");
    studio.play("0123456789abcdef");
    assert!(
        studio.playing.is_none(),
        "something started for a missing take"
    );
    assert!(
        studio.message.is_some(),
        "nothing was said about the failure"
    );
}

#[test]
fn locking_the_window_stops_a_take_that_is_playing() {
    // The buffer is a decrypted recording. It goes with the vault, rather than
    // sitting in memory behind a lock screen.
    //
    // Read from `close`'s own source rather than exercised, and the reason is
    // worth keeping. This used to call `play` and then `close`, on the
    // assumption that a build machine has no audio device and no stream would
    // start. On the Windows runner that assumption was wrong: a stream did
    // start, and tearing it down took the whole test binary with it. Every
    // test in the crate passed and the process died anyway.
    //
    // A test whose correctness depends on a machine not having a sound card is
    // not testing the thing it names. What matters here is one line in `close`,
    // and that is what this asserts.
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    let body = source
        .split("pub fn close(&mut self) {")
        .nth(1)
        .and_then(|rest| rest.split("\n    }").next())
        .expect("the close method has to be findable");
    assert!(
        body.contains("self.playing = None;"),
        "`close` no longer releases what is playing, so a decrypted take \
         outlives the vault it came out of"
    );
}

#[test]
fn starting_one_take_releases_the_one_before_it() {
    // Two decrypted recordings in memory at once is twice as much of somebody's
    // voice as the reason for it. `play` drops the previous one first, and this
    // is the assertion that keeps that first line in place.
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    let body = source
        .split("fn play(&mut self, id: &str) {")
        .nth(1)
        .and_then(|rest| rest.split("\n    }").next())
        .expect("the play method has to be findable");
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"))
        .unwrap_or("");
    assert_eq!(
        first, "self.playing = None;",
        "the first thing `play` does must be to release whatever was playing, \
         and it now starts with {first:?}"
    );
}

#[test]
fn the_studio_meters_what_goes_in_as_well_as_what_comes_out() {
    // One output meter answers "is something being recorded" and not "is it
    // being veiled", which is the question somebody at this tab is asking.
    //
    // Read out of `app.rs` rather than out of this module since marker 130:
    // the voice half of the tab, which is where the meters are, is drawn by
    // the window, because the device lists and the settings widgets it sits
    // beside are the window's. The property is unchanged and so is this test;
    // only the file it reads has moved.
    let source = std::fs::read_to_string("src/app.rs").expect("the window's source");
    let body = source
        .split("fn studio_tab(&mut self, ui: &mut egui::Ui) {")
        .nth(1)
        .and_then(|rest| rest.split("\n    fn ").next())
        .expect("the Studio tab has to be findable");
    assert!(
        body.contains(r#"meter(ui, "in ""#),
        "the Studio does not draw an input meter"
    );
    assert!(
        body.contains(r#"meter(ui, "out""#),
        "the Studio does not draw an output meter"
    );
    // And through the shared one, rather than a second bar that would slowly
    // stop looking like the first.
    assert!(
        body.contains("crate::monitor::meter"),
        "the Studio draws its own meter instead of the shared one"
    );
    // Once a frame, in one place. `stats` resets the peaks as it reads them,
    // so a second reader would see half the level and both bars would be
    // wrong. The window takes the reading in `update` and everything else is
    // shown what it got.
    let studio = std::fs::read_to_string("src/studio.rs").expect("its own source");
    assert_eq!(
        studio.matches("self.levels.update(").count(),
        1,
        "the levels are updated in more than one place, so the peaks are being \
         read twice a frame and each reader sees half of them"
    );
}

#[test]
fn a_new_studio_keeps_the_veiled_voice_and_nothing_else() {
    // The default is the whole safety property of marker 131: nothing reaches a
    // recording of somebody's real voice without being asked for.
    let studio = Studio::default();
    assert_eq!(studio.keep, Keep::Veiled);
    assert!(!studio.keep.wants_plain());
    assert!(studio.keep.wants_veiled());
}

#[test]
fn locking_the_window_puts_the_choice_back_to_the_safe_one() {
    // A choice that survived a lock would be a choice somebody made before
    // lunch deciding what is recorded after it.
    let mut studio = Studio {
        keep: Keep::Plain,
        ..Studio::default()
    };
    studio.close();
    assert_eq!(studio.keep, Keep::Veiled);
    assert!(studio.plain.is_none());
}

#[test]
fn each_side_says_which_of_the_two_it_keeps() {
    assert!(Keep::Veiled.wants_veiled() && !Keep::Veiled.wants_plain());
    assert!(Keep::Plain.wants_plain() && !Keep::Plain.wants_veiled());
    assert!(Keep::Both.wants_veiled() && Keep::Both.wants_plain());
}

#[test]
fn anything_that_keeps_the_real_voice_says_so_before_it_starts() {
    // The wording is the point, not the wiring: this is the one thing the
    // Studio does that makes a recording of somebody, and the sentence has to
    // say that rather than describing a file format.
    for keep in [Keep::Both, Keep::Plain] {
        let cost = keep.cost();
        assert!(
            cost.contains("real voice"),
            "{keep:?} does not say it records the real voice: {cost}"
        );
        assert!(
            cost.contains("hear who was speaking"),
            "{keep:?} does not say what that means for somebody who opens the \
             vault: {cost}"
        );
    }
    // And the safe one says the opposite rather than saying nothing.
    assert!(Keep::Veiled
        .cost()
        .contains("No recording of the real voice"));
}

#[test]
fn every_choice_is_labelled_and_they_are_all_different() {
    let labels: Vec<&str> = [Keep::Veiled, Keep::Both, Keep::Plain]
        .iter()
        .map(|k| k.label())
        .collect();
    for label in &labels {
        assert!(!label.is_empty());
    }
    let mut sorted = labels.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), labels.len(), "two choices read the same");
}

#[test]
fn the_unveiled_take_is_named_so_it_can_be_told_from_the_other() {
    // Both sides land in one vault under one name, and the only thing telling
    // them apart in the Browser is the name. A suffix that stopped being added
    // would leave two identical entries, one of which is somebody's real voice.
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    assert!(
        source.contains(r#"(veiled, ""), (plain, " (unveiled)")"#),
        "the unveiled take is no longer named differently from the veiled one"
    );
}

#[test]
fn every_recorder_that_is_running_is_drained_every_frame() {
    // A recorder nobody drains fills its ring and starts dropping samples, so
    // draining only the veiled one would have made an unveiled take quietly
    // short: the exact failure `dropped` exists to report, arrived at by not
    // asking. And the clock has to come from whichever is running, because
    // keeping only the microphone leaves no veiled recorder at all.
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    let at = source
        .find("Phase::Recording => {")
        .expect("the recording panel exists");
    let body = &source[at..];
    assert!(
        body.contains("[self.recorder.as_mut(), self.plain.as_mut()]"),
        "the recording panel no longer drains both recorders, so a second take \
         would be short and its clock would sit at zero"
    );
}

/// **Marker 145.** The Studio's own failsafe stops the Studio, not the take.
///
/// A device that has gone does not come back and will not produce another
/// sample, so a take left running on it records silence and looks like it
/// worked. What was captured up to that point is a real recording of something
/// somebody said, and it is stored rather than discarded, for the same reason
/// locking the window stores one.
#[test]
fn a_device_that_goes_stops_the_studio_and_says_why() {
    let mut studio = Studio {
        setup: Some(Setup {
            config: DeidConfig::default(),
            input: Some("a microphone".to_string()),
            output: None,
            preview: false,
        }),
        trouble: Some(veilvoice_audio::Interference {
            side: veilvoice_audio::Side::Input,
            device_gone: true,
            said: "the device is no longer available".to_string(),
            count: 1,
        }),
        ..Studio::default()
    };

    studio.catch_a_fault();

    assert!(
        studio.setup.is_none(),
        "the session was left running on a device that has gone"
    );
    assert!(!studio.is_veiling());
    let (said, _) = studio.message.clone().expect("it has to say something");
    assert!(
        said.contains("gone"),
        "the message does not say the device went: {said:?}"
    );
}

/// Anything else the platform says is shown and left alone.
///
/// "The mixer said something" is not a reason to end a recording somebody is
/// making. Only a device that has stopped existing is, because that is the one
/// report that means no further sample is coming.
#[test]
fn a_report_that_is_not_a_missing_device_does_not_stop_anything() {
    let mut studio = Studio {
        setup: Some(Setup {
            config: DeidConfig::default(),
            input: None,
            output: None,
            preview: false,
        }),
        trouble: Some(veilvoice_audio::Interference {
            side: veilvoice_audio::Side::Output,
            device_gone: false,
            said: "an underrun".to_string(),
            count: 1,
        }),
        ..Studio::default()
    };

    studio.catch_a_fault();

    assert!(
        studio.setup.is_some(),
        "an underrun ended a session, and it is not a reason to"
    );
    assert!(
        studio.message.is_none(),
        "the failsafe spoke about something it did not act on"
    );
}

/// The failsafe stores. It does not discard, retry, or change device.
///
/// Read out of its own source, because the difference between storing and
/// discarding is one line and the test for it would otherwise need a vault, a
/// passphrase and an Argon2 derivation to observe. What is worth keeping is the
/// decision: `stop_veiling` seals a take on its way through, and dropping the
/// recorders instead would throw one away.
#[test]
fn the_failsafe_stores_rather_than_discarding_or_retrying() {
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    let body = source
        .split("fn catch_a_fault(&mut self) {")
        .nth(1)
        .and_then(|rest| rest.split("\n    }").next())
        .expect("the failsafe has to be findable");

    assert!(
        body.contains("self.stop_veiling();"),
        "the failsafe does not go through `stop_veiling`, which is what seals \
         a take that is still running"
    );
    for discarding in ["self.recorder = None", "self.plain = None", ".take();"] {
        assert!(
            !body.contains(discarding),
            "the failsafe drops a recorder ({discarding}) instead of storing \
             what it holds"
        );
    }
    for retrying in ["start_session", "start_veiling", "start_take"] {
        assert!(
            !body.contains(retrying),
            "the failsafe restarts the audio ({retrying}). The person chose \
             that microphone, and recording them through another one because \
             the first went away is this program deciding that for them"
        );
    }
}

/// **Marker 145.** A take says what it is keeping while it is being made.
///
/// The choice is made on a form that is gone the moment recording starts, so a
/// take keeping somebody's real voice looked exactly like one that does not for
/// the whole of the recording. The marker asks for this to be shown "at all
/// times", and before the button is not all times.
#[test]
fn a_running_take_says_which_voice_it_is_keeping() {
    let source = std::fs::read_to_string("src/studio.rs").expect("its own source");
    let panel = source
        .split("Phase::Recording => {")
        .nth(1)
        .and_then(|rest| rest.split("\n        }").next())
        .expect("the recording panel has to be findable");

    assert!(
        panel.contains("self.keep.label()"),
        "the recording panel does not say which voice the take is keeping"
    );
    assert!(
        panel.contains("self.keep.wants_plain()"),
        "the panel says what is being kept without distinguishing the one that \
         keeps a real voice, which is the only one worth a colour"
    );
}
