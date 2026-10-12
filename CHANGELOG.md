# Changelog

User-visible changes to Keyloom are recorded here. Version dates refer to local releases; no public release is implied.

## 0.1.2 — 2026-10-11

- Added Yamaha EZ-150 Light Guide with a separate MIDI output selector, saved toggle and two-second middle-C test. Target notes use MIDI channel 1.
- Clear guide notes on pause, stop, seek, song/settings changes, guide disable, output disconnect, Panic and normal exit; reconnect clears interrupted notes.
- Keep guidance and simulated input separate from physical player input. Simulated keys monitor only and cannot satisfy targets or score.
- Physically verified middle-C illumination, Wait-mode guidance and matching, and pause/stop/song-change light cleanup with the owner. Guide messages may also sound the Yamaha.

## 0.1.1 — 2026-10-11

- Made the practice strike line bright pink with a pale edge and dark separation, so the note-hit boundary is easier to see. Practice timing is unchanged.

## 0.1.0 — 2026-10-11

- Delivered the M0–M3 Windows application using Tauri, Rust, React, TypeScript and Canvas, with native USB MIDI input and a generated basic practice tone.
- Added a managed MIDI library with immediate import/open, duplicate detection, original-byte export and saved settings/progress.
- Added Listen, Wait for me and Timed practice; part selection, approximate left/right split, keyboard range, speed, count-in/metronome and A–B loops.
- Delivered local Windows installer and portable packages, third-party notices, automated regressions and real native import-to-library-to-play verification. Physical keyboard observations are tracked separately in the hardware checklist and test report.
