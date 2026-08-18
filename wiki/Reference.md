# Reference

Every crate and every source file, generated from the doc comments in the code by `tools/docs/generate.py`. The same pages are in the repository and on the website; all three come out of one generator, so they cannot disagree.

## [[veilvoice-audio|Crate-veilvoice-audio]]

Real-time capture and playback (cpal), lock-free ring buffers, virtual-cable routing and file import for VeilVoice.

- [[`devices.rs`|File-veilvoice-audio-devices]] &mdash; Enumerating audio devices, and guessing which of them are virtual cables.
- [[`io.rs`|File-veilvoice-audio-io]] &mdash; Reading and writing audio files.
- [[`lib.rs`|File-veilvoice-audio-lib]] &mdash; Everything between the sound hardware and veilvoice_core: device enumeration, file import and export, and the real-time capture → de-identify → playback path.
- [[`live.rs`|File-veilvoice-audio-live]] &mdash; Live microphone scrambling.

## [[veilvoice-cli|Crate-veilvoice-cli]]

Command-line interface for VeilVoice: anonymise files, scramble a microphone live, strip metadata, encrypt recordings.

- [[`atrest.rs`|File-veilvoice-cli-atrest]] &mdash; Encryption at rest for the recordings VeilVoice writes, and the passphrase prompts that feed it.
- [[`guard.rs`|File-veilvoice-cli-guard]] &mdash; veilvoice guard -- record what VeilVoice's files should be, and check them.
- [[`lock.rs`|File-veilvoice-cli-lock]] &mdash; veilvoice lock — manage the application lock from the command line.
- [[`main.rs`|File-veilvoice-cli-main]] &mdash; veilvoice — the command-line interface.
- [[`theme.rs`|File-veilvoice-cli-theme]] &mdash; Tokyo Night colouring for the terminal.

## [[veilvoice-core|Crate-veilvoice-core]]

Irreversible voice de-identification DSP engine: cryptographically-modulated pitch/formant scrambling with preserved intelligibility.

- [[`accent.rs`|File-veilvoice-core-accent]] &mdash; Accent and speaker-trait neutralisation.
- [[`chain.rs`|File-veilvoice-core-chain]] &mdash; The assembled de-identification chain and its live performance statistics.
- [[`effects.rs`|File-veilvoice-core-effects]] &mdash; Light time-domain effects applied after resynthesis.
- [[`lib.rs`|File-veilvoice-core-lib]] &mdash; The security-critical heart of VeilVoice: an irreversible, cryptographically modulated voice de-identification engine.
- [[`modulation.rs`|File-veilvoice-core-modulation]] &mdash; Cryptographically-seeded modulation of the effect parameters.
- [[`pitch.rs`|File-veilvoice-core-pitch]] &mdash; Monophonic fundamental-frequency tracker (decimated YIN).
- [[`spectral.rs`|File-veilvoice-core-spectral]] &mdash; Frequency-domain de-identification transform.
- [[`stft.rs`|File-veilvoice-core-stft]] &mdash; Streaming short-time Fourier transform with overlap-add resynthesis.
- [[`window.rs`|File-veilvoice-core-window]] &mdash; Analysis and synthesis windowing, and the one constant that keeps overlap-add honest.

## [[veilvoice-crypto|Crate-veilvoice-crypto]]

Argon2id KDF, X25519+ML-KEM-768 hybrid KEM, XChaCha20-Poly1305 at-rest encryption and page-locked amnesic secrets for VeilVoice.

- [[`aead.rs`|File-veilvoice-crypto-aead]] &mdash; Authenticated encryption with XChaCha20-Poly1305.
- [[`amnesia.rs`|File-veilvoice-crypto-amnesia]] &mdash; Amnesic secret storage: page-locked, zeroized, and never printed.
- [[`container.rs`|File-veilvoice-crypto-container]] &mdash; The .veil encrypted container format.
- [[`hybrid.rs`|File-veilvoice-crypto-hybrid]] &mdash; Post-quantum hybrid key encapsulation: X25519 + ML-KEM-768.
- [[`kdf.rs`|File-veilvoice-crypto-kdf]] &mdash; Password-based key derivation with Argon2id.
- [[`lib.rs`|File-veilvoice-crypto-lib]] &mdash; Key derivation, post-quantum-hybrid key agreement, authenticated encryption and amnesic secret storage for VeilVoice.
- [[`lock.rs`|File-veilvoice-crypto-lock]] &mdash; The application lock: an Argon2id password verifier with a rate limit.
- [[`privatefile.rs`|File-veilvoice-crypto-privatefile]] &mdash; Writing a file that only its owner can read.
- [[`shred.rs`|File-veilvoice-crypto-shred]] &mdash; Secure erasure — the self-destruct.

## [[veilvoice-guard|Crate-veilvoice-guard]]

Integrity manifest and tamper detection for VeilVoice's own files, with best-effort attribution of what changed them.

- [[`blame.rs`|File-veilvoice-guard-blame]] &mdash; Best-effort attribution: which program changed a file.
- [[`lib.rs`|File-veilvoice-guard-lib]] &mdash; Tamper detection for VeilVoice's own files: a manifest of what they should be, a check of what they are, and a best-effort answer to "what changed them".
- [[`manifest.rs`|File-veilvoice-guard-manifest]] &mdash; The integrity manifest: what the files were, and what they are now.

## [[veilvoice-gui|Crate-veilvoice-gui]]

egui/eframe front-end for VeilVoice: Tokyo Night, monospace, three modes.

- [[`app.rs`|File-veilvoice-gui-app]] &mdash; The VeilVoice desktop application.
- [[`lib.rs`|File-veilvoice-gui-lib]] &mdash; The VeilVoice desktop application: an egui/eframe front-end, monospace throughout — anonymise a file, scramble a microphone live, watch what is listening, manage the app lock, choose how the app looks, and an about panel that states the honest scope.
- [[`main.rs`|File-veilvoice-gui-main]] &mdash; Entry point for the desktop application: open a window, hand it to veilvoice_gui::VeilVoiceApp, and get out of the way.
- [[`prefs.rs`|File-veilvoice-gui-prefs]] &mdash; What the user has chosen about how the app looks and moves.
- [[`reduced_motion.rs`|File-veilvoice-gui-reduced_motion]] &mdash; Whether the operating system has been asked to reduce motion.
- [[`security.rs`|File-veilvoice-gui-security]] &mdash; The application lock, and the at-rest encryption of what VeilVoice writes.
- [[`settings.rs`|File-veilvoice-gui-settings]] &mdash; The settings panel: a menu of pages, each a titled group of choices.
- [[`soundbar.rs`|File-veilvoice-gui-soundbar]] &mdash; The animated mark: a row of bars that rise and fall.
- [[`theme.rs`|File-veilvoice-gui-theme]] &mdash; Colour schemes for the desktop app.

## [[veilvoice-meta|Crate-veilvoice-meta]]

Strip or spoof identifying metadata: audio tags, and image EXIF/GPS.

- [[`audio.rs`|File-veilvoice-meta-audio]] &mdash; Audio tag removal and replacement.
- [[`image.rs`|File-veilvoice-meta-image]] &mdash; Image EXIF/GPS removal.
- [[`lib.rs`|File-veilvoice-meta-lib]] &mdash; Strip or spoof the identifying metadata that rides along with media files.
- [[`wav.rs`|File-veilvoice-meta-wav]] &mdash; Chunk-level RIFF/WAVE metadata removal.

## [[veilvoice-verify|Crate-veilvoice-verify]]

Verify a VeilVoice release without GnuPG installed

- [[`main.rs`|File-veilvoice-verify-main]] &mdash; The portable verifier: check a VeilVoice release without GnuPG installed.
- [[`tests.rs`|File-veilvoice-verify-tests]] &mdash; no module documentation yet

## [[veilvoice-watch|Crate-veilvoice-watch]]

Detect which applications are currently using the microphone and camera, with alerts on change.

- [[`lib.rs`|File-veilvoice-watch-lib]] &mdash; Find out which applications are using your microphone and camera, right now.
- [[`linux.rs`|File-veilvoice-watch-linux]] &mdash; Linux detection, via open file handles in /proc.
- [[`windows.rs`|File-veilvoice-watch-windows]] &mdash; Windows detection, via the Capability Access Manager.
