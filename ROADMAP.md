# Keyloom roadmap

Keyloom is an original, offline Windows piano trainer. Release details are in [CHANGELOG.md](CHANGELOG.md); verification evidence is in the [test report](docs/test-report.md) and [hardware checklist](docs/hardware-checklist.md).

## Delivered

- **M0 — native spike (0.1.0):** Tauri 2 / React / TypeScript window, native WinMM MIDI input, WASAPI basic practice tone, moving Canvas roll and an explicit simulated-input mode.
- **M1 — managed library (0.1.0):** bounded canonical MIDI parser, crash-safe SQLite library, immediate import/open, metadata and settings persistence, native playback and part controls.
- **M2 — practice (0.1.0):** authoritative Rust transport and chord matching, falling notes, range calibration, Listen / Wait for me / Timed modes, loops, monitoring and device recovery.
- **M3 — delivery (0.1.0):** generated-fixture regressions, real native import→library→play verification, a 1,000-song exercise, third-party notices audit and reproducible Windows installer/portable packages.
- **Yamaha EZ-150 Light Guide (0.1.2–0.1.3):** separate MIDI output for channel-1 guide notes, cleanup on every stop path, and PC audio mute while the guide is on.
- **M4, practice versions (0.2.0–0.4.0):** named practice copies that leave the original untouched; note removal and semitone/octave moves with persistent Undo; edited MIDI export to a new file; previewed chord thinning and keyboard-range fitting.

## Pending checks

- Physical acceptance of the 0.4.0 preview/apply tools on the Yamaha (thinned targets, Undo, pause cleanup).
- Remaining hardware edge cases in the [hardware checklist](docs/hardware-checklist.md): repeated notes, chords and velocity, long sessions, cable-loss recovery with lit notes, Timed guidance, actual key range, 4K scaling and end-to-end latency.

## Deferred

- Timing and duration editing; recording.
- **M5:** MusicXML and optical music recognition.
- **M6:** transcription and downloading.
- Sampled piano, SoundFonts, full General MIDI fidelity and general playback MIDI output.
- Platforms other than Windows x64.

No cloud service, account system or LLM dependency is planned. No commercial songs, fonts, samples or SoundFonts are distributed.
