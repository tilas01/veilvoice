// SPDX-License-Identifier: GPL-3.0-or-later
//! What an export asks, and the `ffmpeg` command each answer turns into.
//!
//! **Roadmap item 175.** The Browser's export used to offer three things: a
//! page, a video with a black picture, or both. A recording tool asks more
//! than that, and this module is the questions it asks, in the order it asks
//! them:
//!
//! 1. **What to write**: the sound on its own, the picture on its own, or the
//!    two together ([`Content`]).
//! 2. **A container and a codec** ([`Container`], [`VideoCodec`]).
//! 3. **A quality**, from a lossless master down to something small enough to
//!    send in a message ([`Quality`]).
//!
//! The fourth question, how the picture looks, is [`crate::motion`].
//!
//! # The audio is lossless whatever the video is
//!
//! There is no lossy audio codec anywhere in this module, and that is the
//! rule rather than a default. The recording is the point of the file and the
//! picture is a carrier for it: somebody who picks the smallest video there is
//! has chosen a smaller picture, not a worse voice. So the sound goes in as
//! FLAC, ALAC or plain PCM, whichever the container can carry, and a veiled
//! recording that leaves VeilVoice is bit for bit the one that was stored.
//!
//! That rule is also why **WebM is not offered**. WebM carries Vorbis and Opus
//! and nothing lossless, and `ffmpeg` refuses to put FLAC in it. Offering it
//! would mean either breaking the rule for one container or offering a choice
//! that always fails, and neither is honest.
//!
//! # A choice that cannot work is refused before anything runs
//!
//! Not every codec goes in every container, and not every quality means
//! anything for every codec. [`Export::checked`] refuses the combinations that
//! would make `ffmpeg` stop after the frames were drawn, and every refusal
//! names something that would have worked, because "that does not go in a
//! MOV" is only half an answer.
//!
//! # And VeilVoice still does not run it for you
//!
//! Exactly as [`crate::ffmpeg`] says: this builds the argument list. Running
//! it is the caller's, and a front end names the program it found rather than
//! fetching one.
//!
//! # In plain words
//!
//! These are the questions the export asks: sound, picture or both; which kind
//! of file; how good. Whatever you pick for the picture, the sound is saved
//! without losing anything, because the voice is what the file is for.

use std::path::Path;

use crate::size;
use crate::Error;

/// What an export writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Content {
    /// The sound alone, as a lossless audio file.
    Audio,
    /// The picture alone, with no sound track.
    Video,
    /// The picture with the sound in it. The default, because a video of a
    /// conversation with no sound is the unusual request.
    #[default]
    Both,
}

impl Content {
    /// Every choice, in the order they are offered.
    pub const ALL: [Content; 3] = [Content::Both, Content::Audio, Content::Video];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Content::Audio => "Audio only",
            Content::Video => "Video only",
            Content::Both => "Audio and video",
        }
    }

    /// Whether the picture is drawn at all.
    pub fn wants_picture(self) -> bool {
        matches!(self, Content::Video | Content::Both)
    }

    /// Whether the sound goes in.
    pub fn wants_sound(self) -> bool {
        matches!(self, Content::Audio | Content::Both)
    }
}

/// The file a video goes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Container {
    /// MP4, which every phone and every browser plays. The default.
    #[default]
    Mp4,
    /// Matroska, which takes anything and is where a lossless master goes.
    Mkv,
    /// QuickTime, for editing software that wants ProRes.
    Mov,
}

impl Container {
    /// Every container, in the order they are offered.
    pub const ALL: [Container; 3] = [Container::Mp4, Container::Mkv, Container::Mov];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4",
            Container::Mkv => "MKV (Matroska)",
            Container::Mov => "MOV (QuickTime)",
        }
    }

    /// The file extension, without its dot.
    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mkv => "mkv",
            Container::Mov => "mov",
        }
    }

    /// The video codecs this container carries, in the order they are offered.
    pub fn video_codecs(self) -> &'static [VideoCodec] {
        match self {
            Container::Mp4 => &[VideoCodec::H264, VideoCodec::H265, VideoCodec::Av1],
            Container::Mkv => &[
                VideoCodec::H264,
                VideoCodec::H265,
                VideoCodec::Av1,
                VideoCodec::Vp9,
                VideoCodec::Ffv1,
            ],
            Container::Mov => &[VideoCodec::H264, VideoCodec::H265, VideoCodec::ProRes],
        }
    }

    /// The lossless audio this container carries.
    ///
    /// One per container rather than a choice, because between FLAC, ALAC and
    /// PCM there is nothing to prefer except what the file can hold and what
    /// plays it: all three give back the same samples.
    ///
    /// * **MP4 takes FLAC**, which every current browser and phone plays from
    ///   an MP4. ALAC would also fit, and browsers do not play it.
    /// * **MKV takes FLAC**, which is what Matroska was built around.
    /// * **MOV takes ALAC**, because the software that asks for a MOV is
    ///   Apple's, and it reads ALAC natively and FLAC not at all.
    pub fn audio_codec(self) -> AudioCodec {
        match self {
            Container::Mp4 | Container::Mkv => AudioCodec::Flac,
            Container::Mov => AudioCodec::Alac,
        }
    }
}

/// How the picture is compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoCodec {
    /// H.264, which plays everywhere. The default.
    #[default]
    H264,
    /// H.265, about half the size of H.264 for the same picture, and slower.
    H265,
    /// AV1, smaller again, and slower again.
    Av1,
    /// VP9, for somebody who wants a codec with no patent pool behind it.
    Vp9,
    /// FFV1, which is lossless: the frames come back exactly as drawn.
    Ffv1,
    /// Apple ProRes, for editing rather than watching.
    ProRes,
}

impl VideoCodec {
    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::H265 => "H.265 (HEVC)",
            VideoCodec::Av1 => "AV1",
            VideoCodec::Vp9 => "VP9",
            VideoCodec::Ffv1 => "FFV1 (lossless)",
            VideoCodec::ProRes => "ProRes",
        }
    }

    /// The `ffmpeg` encoder that writes it.
    ///
    /// Software encoders only. [`crate::ffmpeg::Encoding::encoder`] is where a
    /// hardware one is chosen for the plain render; an export is a file
    /// somebody keeps, and two people exporting the same recording with the
    /// same settings should get the same file whatever graphics card each of
    /// them has.
    pub fn encoder(self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::Av1 => "libsvtav1",
            VideoCodec::Vp9 => "libvpx-vp9",
            VideoCodec::Ffv1 => "ffv1",
            VideoCodec::ProRes => "prores_ks",
        }
    }

    /// Whether every quality is the same quality for this codec.
    ///
    /// FFV1 has no setting that loses anything, so asking it for a small file
    /// is a question it cannot answer.
    pub fn always_lossless(self) -> bool {
        matches!(self, VideoCodec::Ffv1)
    }
}

/// The lossless audio an export carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCodec {
    /// Free Lossless Audio Codec.
    Flac,
    /// Apple Lossless.
    Alac,
    /// Uncompressed, as a WAV holds it.
    Pcm,
}

impl AudioCodec {
    /// The `ffmpeg` encoder that writes it.
    pub fn encoder(self) -> &'static str {
        match self {
            AudioCodec::Flac => "flac",
            AudioCodec::Alac => "alac",
            // Sixteen bits, because that is what the recorder stores; writing
            // twenty-four would be a larger file holding the same samples.
            AudioCodec::Pcm => "pcm_s16le",
        }
    }

    /// What this is called where it is shown.
    pub fn label(self) -> &'static str {
        match self {
            AudioCodec::Flac => "FLAC",
            AudioCodec::Alac => "ALAC",
            AudioCodec::Pcm => "PCM",
        }
    }
}

/// How good the picture is, and so how large the file is.
///
/// Four rather than a number, because the number means something different to
/// every codec: a constant rate factor of 23 is ordinary for H.265 and poor for
/// AV1. These are the four things people actually want, and
/// [`Quality::factor`] is what each means to each codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    /// A master: nothing thrown away, for keeping or for editing later.
    Lossless,
    /// No difference anybody would see. The default.
    #[default]
    High,
    /// A good picture at a sensible size.
    Balanced,
    /// Small enough to send in a message.
    Small,
}

impl Quality {
    /// Every quality, best first.
    pub const ALL: [Quality; 4] = [
        Quality::Lossless,
        Quality::High,
        Quality::Balanced,
        Quality::Small,
    ];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Lossless => "Lossless master",
            Quality::High => "High",
            Quality::Balanced => "Balanced",
            Quality::Small => "Small, for a message",
        }
    }

    /// The constant rate factor this quality means for `codec`, if it uses one.
    ///
    /// `None` for FFV1, which has no factor, and for ProRes, which picks a
    /// profile instead ([`Quality::prores_profile`]). Lossless is not a factor
    /// either: [`Export::checked`] sends it to FFV1.
    pub fn factor(self, codec: VideoCodec) -> Option<u32> {
        let (high, balanced, small) = match codec {
            // The ranges differ, which is the whole reason this is a table:
            // x264 and x265 run to 51, SVT-AV1 and VP9 to 63.
            VideoCodec::H264 => (16, 21, 28),
            VideoCodec::H265 => (18, 24, 30),
            VideoCodec::Av1 => (22, 32, 42),
            VideoCodec::Vp9 => (20, 31, 40),
            VideoCodec::Ffv1 | VideoCodec::ProRes => return None,
        };
        match self {
            Quality::Lossless => None,
            Quality::High => Some(high),
            Quality::Balanced => Some(balanced),
            Quality::Small => Some(small),
        }
    }

    /// The ProRes profile this quality means: 3 is HQ, 2 standard, 0 proxy.
    pub fn prores_profile(self) -> Option<u32> {
        match self {
            Quality::Lossless => None,
            Quality::High => Some(3),
            Quality::Balanced => Some(2),
            Quality::Small => Some(0),
        }
    }
}

/// The lossless audio-only file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AudioFile {
    /// FLAC: about half the size, and plays nearly everywhere. The default.
    #[default]
    Flac,
    /// WAV: larger, and plays absolutely everywhere.
    Wav,
}

impl AudioFile {
    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            AudioFile::Flac => "FLAC",
            AudioFile::Wav => "WAV",
        }
    }

    /// The file extension, without its dot.
    pub fn extension(self) -> &'static str {
        match self {
            AudioFile::Flac => "flac",
            AudioFile::Wav => "wav",
        }
    }
}

/// Every answer an export needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    /// Sound, picture, or both.
    pub content: Content,
    /// The file a video goes into.
    pub container: Container,
    /// How the picture is compressed.
    pub codec: VideoCodec,
    /// How good the picture is.
    pub quality: Quality,
    /// The file the sound goes into when it is exported on its own.
    pub audio_file: AudioFile,
    /// The frame size and rate.
    pub plan: size::Plan,
}

impl Default for Export {
    /// High quality H.264 in an MP4 with FLAC in it, at 1080p and sixty frames
    /// a second.
    ///
    /// Sixty rather than the thirty [`crate::ffmpeg::Encoding`] uses, because
    /// the moving wave [`crate::motion`] draws is motion, and at thirty it
    /// visibly steps. The file is barely larger: most of each frame is a
    /// background that does not change.
    fn default() -> Self {
        Self {
            content: Content::default(),
            container: Container::default(),
            codec: VideoCodec::default(),
            quality: Quality::default(),
            audio_file: AudioFile::default(),
            plan: size::Plan::new(
                size::Preset::Hd1080.size(),
                size::FrameRate::new(60).unwrap_or_default(),
            ),
        }
    }
}

impl Export {
    /// Refuse a combination `ffmpeg` would refuse, and say what would work.
    ///
    /// An audio-only export is always fine: nothing about the picture applies.
    pub fn checked(&self) -> Result<(), Error> {
        if !self.content.wants_picture() {
            return Ok(());
        }
        if !self.container.video_codecs().contains(&self.codec) {
            let could: Vec<&str> = self
                .container
                .video_codecs()
                .iter()
                .map(|codec| codec.label())
                .collect();
            let elsewhere: Vec<&str> = Container::ALL
                .iter()
                .filter(|container| container.video_codecs().contains(&self.codec))
                .map(|container| container.label())
                .collect();
            return Err(Error::Malformed(format!(
                "{} does not go in {}. {} takes {}; {} goes in {}.",
                self.codec.label(),
                self.container.label(),
                self.container.label(),
                could.join(", "),
                self.codec.label(),
                elsewhere.join(" or ")
            )));
        }
        if self.quality == Quality::Lossless && !self.codec.always_lossless() {
            return Err(Error::Malformed(format!(
                "{} throws some of the picture away at every setting. A lossless \
                 master is FFV1, in an MKV.",
                self.codec.label()
            )));
        }
        if self.codec.always_lossless() && self.quality != Quality::Lossless {
            return Err(Error::Malformed(format!(
                "{} is lossless at every setting, so it cannot make a {} file. \
                 Choose Lossless master, or H.264 for a smaller one.",
                self.codec.label(),
                self.quality.label().to_lowercase()
            )));
        }
        Ok(())
    }

    /// The extension of the file this writes.
    pub fn extension(&self) -> &'static str {
        if self.content.wants_picture() {
            self.container.extension()
        } else {
            self.audio_file.extension()
        }
    }

    /// What will be written, in one line a person reads before pressing the
    /// button.
    ///
    /// The audio half is always stated, and always says lossless, because it
    /// is the promise the whole export is built on and a person choosing
    /// "Small" should see that it does not reach the voice.
    pub fn describe(&self) -> String {
        let audio = if self.content.wants_picture() {
            self.container.audio_codec().label()
        } else {
            self.audio_file.label()
        };
        match self.content {
            Content::Audio => format!("{audio}, lossless"),
            Content::Video => format!(
                "{} in {}, {}, {}, no sound",
                self.codec.label(),
                self.container.label(),
                self.quality.label().to_lowercase(),
                self.plan.size.label()
            ),
            Content::Both => format!(
                "{} in {}, {}, {}, with {audio} audio, lossless",
                self.codec.label(),
                self.container.label(),
                self.quality.label().to_lowercase(),
                self.plan.size.label()
            ),
        }
    }

    /// The command that writes the export.
    ///
    /// For a picture, the frames arrive on standard input as raw RGB, one
    /// after another, at [`Export::plan`]'s size and rate; [`crate::motion`]
    /// draws them. Piped rather than written to disk, because the moving wave
    /// changes every frame, and an hour of it at 1080p would be hundreds of
    /// gigabytes of pictures on the way to a file of a few hundred megabytes.
    ///
    /// `audio` is the veiled WAV. For an audio-only export nothing is read
    /// from standard input.
    pub fn command(&self, audio: &Path, output: &Path) -> Result<Vec<String>, Error> {
        self.checked()?;
        let mut argv: Vec<String> = vec![
            "ffmpeg".into(),
            // Never overwrite, as everywhere else in this crate.
            "-n".into(),
            // Nothing interactive: ffmpeg reads keystrokes from standard input
            // unless told not to, and here standard input is the picture.
            "-nostdin".into(),
        ];

        if !self.content.wants_picture() {
            argv.extend(["-i".into(), audio.display().to_string()]);
            argv.extend(["-vn".into(), "-c:a".into()]);
            argv.push(match self.audio_file {
                AudioFile::Flac => AudioCodec::Flac.encoder().into(),
                AudioFile::Wav => AudioCodec::Pcm.encoder().into(),
            });
            if self.audio_file == AudioFile::Flac {
                // The slowest and smallest. FLAC's levels differ in time
                // spent, never in what comes back out.
                argv.extend(["-compression_level".into(), "8".into()]);
            }
            // Strip whatever metadata the input carried rather than copy it:
            // the vault's WAV has none, and a rule that holds anyway costs
            // one flag.
            argv.extend(["-map_metadata".into(), "-1".into()]);
            argv.push(output.display().to_string());
            return Ok(argv);
        }

        // `-nostdin` is wrong once standard input is the picture: it stops
        // ffmpeg reading it as a keyboard, and `pipe:0` is read as a file,
        // which is the point. Removed rather than left contradicting.
        argv.retain(|arg| arg != "-nostdin");
        argv.extend([
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgb24".into(),
            "-s".into(),
            self.plan.size.geometry(),
            "-r".into(),
            self.plan.fps.get().to_string(),
            "-i".into(),
            "pipe:0".into(),
        ]);
        if self.content.wants_sound() {
            argv.extend(["-i".into(), audio.display().to_string()]);
        }

        argv.extend(["-map".into(), "0:v:0".into()]);
        if self.content.wants_sound() {
            argv.extend(["-map".into(), "1:a:0".into()]);
        }

        argv.extend(["-c:v".into(), self.codec.encoder().into()]);
        argv.extend(self.video_settings());

        if self.content.wants_sound() {
            let audio_codec = self.container.audio_codec();
            argv.extend(["-c:a".into(), audio_codec.encoder().into()]);
            if audio_codec == AudioCodec::Flac {
                argv.extend(["-compression_level".into(), "8".into()]);
                if self.container == Container::Mp4 {
                    // FLAC in MP4 was marked experimental until ffmpeg 6, and
                    // every build before that refuses it without this. It
                    // relaxes nothing else here: every codec is named.
                    argv.extend(["-strict".into(), "experimental".into()]);
                }
            }
            // Stop at the end of the sound, so a frame too many can never
            // leave a moment of silent picture on the end.
            argv.push("-shortest".into());
        } else {
            argv.push("-an".into());
        }

        if matches!(self.container, Container::Mp4 | Container::Mov) {
            // The index at the front, so a phone starts playing before the
            // whole file has arrived.
            argv.extend(["-movflags".into(), "+faststart".into()]);
        }
        argv.extend(["-map_metadata".into(), "-1".into()]);
        argv.push(output.display().to_string());
        Ok(argv)
    }

    /// The encoder settings for the chosen codec and quality.
    fn video_settings(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        match self.codec {
            VideoCodec::Ffv1 => {
                // Version 3 with slice checksums, and the picture kept as RGB:
                // converting to YUV would throw colour away, which is the one
                // thing a lossless master exists not to do.
                out.extend([
                    "-level".into(),
                    "3".into(),
                    "-slicecrc".into(),
                    "1".into(),
                    "-pix_fmt".into(),
                    "bgr0".into(),
                ]);
            }
            VideoCodec::ProRes => {
                let profile = self.quality.prores_profile().unwrap_or(3);
                out.extend([
                    "-profile:v".into(),
                    profile.to_string(),
                    "-pix_fmt".into(),
                    "yuv422p10le".into(),
                ]);
            }
            VideoCodec::Vp9 => {
                let factor = self.quality.factor(self.codec).unwrap_or(31);
                // `-b:v 0` is what makes VP9's `-crf` a constant quality
                // rather than a cap on a bitrate nobody set.
                out.extend([
                    "-crf".into(),
                    factor.to_string(),
                    "-b:v".into(),
                    "0".into(),
                    "-row-mt".into(),
                    "1".into(),
                    "-pix_fmt".into(),
                    "yuv420p".into(),
                ]);
            }
            VideoCodec::H264 | VideoCodec::H265 | VideoCodec::Av1 => {
                let factor = self.quality.factor(self.codec).unwrap_or(21);
                out.extend(["-crf".into(), factor.to_string()]);
                if self.codec != VideoCodec::Av1 {
                    // Slower than the default and worth it: the frames are
                    // flat colour and fine edges, which is what the slower
                    // presets spend their time on.
                    out.extend(["-preset".into(), "slow".into()]);
                }
                if self.codec == VideoCodec::H265 && self.container != Container::Mkv {
                    // Apple's players refuse H.265 in an MP4 or MOV unless the
                    // track is tagged `hvc1`, and play it perfectly once it is.
                    out.extend(["-tag:v".into(), "hvc1".into()]);
                }
                // The pixel format every player and phone accepts, for the
                // reason `crate::ffmpeg::command` gives.
                out.extend(["-pix_fmt".into(), "yuv420p".into()]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(export: &Export) -> Vec<String> {
        export
            .command(Path::new("in.wav"), Path::new("out"))
            .expect("the combination is valid")
    }

    fn value_after(argv: &[String], flag: &str) -> Option<String> {
        argv.iter()
            .position(|arg| arg == flag)
            .and_then(|at| argv.get(at + 1).cloned())
    }

    /// The rule the roadmap row is built on: no lossy audio encoder, ever.
    #[test]
    fn the_audio_is_lossless_whatever_the_video_is() {
        let lossless = ["flac", "alac", "pcm_s16le"];
        for container in Container::ALL {
            for &codec in container.video_codecs() {
                for quality in Quality::ALL {
                    for content in Content::ALL {
                        for audio_file in [AudioFile::Flac, AudioFile::Wav] {
                            let export = Export {
                                content,
                                container,
                                codec,
                                quality,
                                audio_file,
                                ..Export::default()
                            };
                            let Ok(argv) = export.command(Path::new("a.wav"), Path::new("o"))
                            else {
                                continue;
                            };
                            if let Some(audio) = value_after(&argv, "-c:a") {
                                assert!(
                                    lossless.contains(&audio.as_str()),
                                    "{audio} is not lossless, from {export:?}"
                                );
                            }
                            for lossy in ["aac", "libopus", "libvorbis", "libmp3lame", "-b:a"] {
                                assert!(
                                    !argv.iter().any(|arg| arg == lossy),
                                    "{lossy} in {argv:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_container_offers_at_least_one_codec_at_every_lossy_quality() {
        for container in Container::ALL {
            for quality in [Quality::High, Quality::Balanced, Quality::Small] {
                let works = container.video_codecs().iter().any(|&codec| {
                    Export {
                        container,
                        codec,
                        quality,
                        ..Export::default()
                    }
                    .checked()
                    .is_ok()
                });
                assert!(works, "{container:?} has nothing at {quality:?}");
            }
        }
    }

    #[test]
    fn a_lossless_master_is_ffv1_in_matroska_and_keeps_rgb() {
        let export = Export {
            container: Container::Mkv,
            codec: VideoCodec::Ffv1,
            quality: Quality::Lossless,
            ..Export::default()
        };
        let argv = argv(&export);
        assert_eq!(value_after(&argv, "-c:v").as_deref(), Some("ffv1"));
        assert_eq!(value_after(&argv, "-c:a").as_deref(), Some("flac"));
        let formats: Vec<_> = argv
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-pix_fmt")
            .filter_map(|(at, _)| argv.get(at + 1))
            .collect();
        assert!(
            formats.iter().any(|f| *f == "bgr0"),
            "the master must stay RGB, got {formats:?}"
        );
        assert!(!formats.iter().any(|f| f.starts_with("yuv")));
    }

    #[test]
    fn a_refusal_names_what_would_have_worked() {
        let wrong = Export {
            container: Container::Mp4,
            codec: VideoCodec::ProRes,
            ..Export::default()
        };
        let why = wrong.checked().unwrap_err().to_string();
        assert!(why.contains("MOV"), "{why}");
        assert!(why.contains("H.264"), "{why}");

        let lossy_master = Export {
            quality: Quality::Lossless,
            ..Export::default()
        };
        assert!(lossy_master
            .checked()
            .unwrap_err()
            .to_string()
            .contains("FFV1"));

        let small_ffv1 = Export {
            container: Container::Mkv,
            codec: VideoCodec::Ffv1,
            quality: Quality::Small,
            ..Export::default()
        };
        assert!(small_ffv1.checked().is_err());
        assert!(small_ffv1.command(Path::new("a"), Path::new("b")).is_err());
    }

    #[test]
    fn the_picture_comes_from_standard_input_at_the_planned_size_and_rate() {
        let export = Export::default();
        let argv = argv(&export);
        assert_eq!(value_after(&argv, "-f").as_deref(), Some("rawvideo"));
        assert_eq!(value_after(&argv, "-s").as_deref(), Some("1920x1080"));
        assert_eq!(value_after(&argv, "-r").as_deref(), Some("60"));
        assert!(argv.iter().any(|arg| arg == "pipe:0"));
        assert!(
            !argv.iter().any(|arg| arg == "-nostdin"),
            "standard input is the picture"
        );
        assert!(argv.iter().any(|arg| arg == "-shortest"));
        assert_eq!(
            value_after(&argv, "-movflags").as_deref(),
            Some("+faststart")
        );
        assert_eq!(argv.last().map(String::as_str), Some("out"));
    }

    #[test]
    fn video_only_has_no_sound_track_and_reads_no_audio() {
        let export = Export {
            content: Content::Video,
            ..Export::default()
        };
        let argv = argv(&export);
        assert!(argv.iter().any(|arg| arg == "-an"));
        assert!(!argv.iter().any(|arg| arg == "in.wav"));
        assert!(value_after(&argv, "-c:a").is_none());
    }

    #[test]
    fn audio_only_reads_nothing_from_standard_input() {
        for audio_file in [AudioFile::Flac, AudioFile::Wav] {
            let export = Export {
                content: Content::Audio,
                audio_file,
                ..Export::default()
            };
            let argv = argv(&export);
            assert!(argv.iter().any(|arg| arg == "-nostdin"));
            assert!(!argv.iter().any(|arg| arg == "pipe:0"));
            assert!(argv.iter().any(|arg| arg == "-vn"));
            assert_eq!(export.extension(), audio_file.extension());
        }
    }

    #[test]
    fn flac_in_mp4_is_allowed_on_an_older_ffmpeg_too() {
        let argv = argv(&Export::default());
        assert_eq!(value_after(&argv, "-c:a").as_deref(), Some("flac"));
        assert_eq!(
            value_after(&argv, "-strict").as_deref(),
            Some("experimental")
        );
        let mkv = argv_for(Container::Mkv);
        assert!(!mkv.iter().any(|arg| arg == "-strict"));
    }

    fn argv_for(container: Container) -> Vec<String> {
        argv(&Export {
            container,
            ..Export::default()
        })
    }

    #[test]
    fn h265_is_tagged_for_apple_players_outside_matroska() {
        for (container, tagged) in [
            (Container::Mp4, true),
            (Container::Mov, true),
            (Container::Mkv, false),
        ] {
            let argv = argv(&Export {
                container,
                codec: VideoCodec::H265,
                ..Export::default()
            });
            assert_eq!(
                value_after(&argv, "-tag:v").is_some(),
                tagged,
                "{container:?}"
            );
        }
    }

    #[test]
    fn smaller_means_a_higher_factor_for_every_codec_that_has_one() {
        for codec in [
            VideoCodec::H264,
            VideoCodec::H265,
            VideoCodec::Av1,
            VideoCodec::Vp9,
        ] {
            let high = Quality::High.factor(codec).unwrap();
            let balanced = Quality::Balanced.factor(codec).unwrap();
            let small = Quality::Small.factor(codec).unwrap();
            assert!(high < balanced && balanced < small, "{codec:?}");
        }
    }

    #[test]
    fn the_description_always_says_the_audio_is_lossless() {
        for content in Content::ALL {
            for quality in [Quality::High, Quality::Small] {
                let export = Export {
                    content,
                    quality,
                    ..Export::default()
                };
                if content.wants_sound() {
                    assert!(export.describe().contains("lossless"), "{export:?}");
                } else {
                    assert!(export.describe().contains("no sound"));
                }
            }
        }
    }
}
