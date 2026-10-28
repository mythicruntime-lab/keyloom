# Changelog

User-visible changes to Keyloom are recorded here. Versions up to 0.4.0 were local builds made before the repository was published.

## 0.4.2-beta.1 — beta candidate, not yet published

- When no MIDI output is connected, the Light Guide checkboxes now say why they can't be ticked instead of only explaining on hover. On the practice screen, the hint opens Keyboard & sound.
- Queue preference changes and device-selection refreshes in order so rapid changes retain every acknowledged edit. A failed change no longer blocks later changes.
- Keep an automatic-save warning visible across screens until a retry succeeds. If the final save fails, closing asks whether to discard the latest practice position/settings or keep the window open for retries.
- Add regressions for overlapping preference changes, invalid-change recovery and real database save failures/recovery in isolated native tests. Existing library/schema compatibility is unchanged.
- Reconcile redistribution notices with the recorded maintainer decisions and document the beta's remaining hardware and installer acceptance checks.
- Generate package checksums through .NET so packaging also succeeds when PowerShell cannot auto-load `Get-FileHash` in the shortened build environment.

## 0.4.1 — 2026-10-28

Prepared as the first public release candidate; no GitHub Release was published. No practice, library or editing behavior changed from 0.4.0.

- Changed the app identifier from `app.keyloom.desktop`, which pointed at a domain the project doesn't own, to `io.github.mythicruntime-lab.keyloom`. Application data now lives in `%LOCALAPPDATA%\io.github.mythicruntime-lab.keyloom`. Libraries from 0.4.0 and earlier are not moved automatically; copy the old folder's contents across with Keyloom closed. The installer's publisher stays `keyloom`, so upgrades still find the existing install.
- Rewrote the bundled README for new users and replaced the internal handoff notes with `ROADMAP.md`. The portable ZIP now includes the same `docs` folder as the installer.
- Recorded the release decisions on the statically linked Microsoft runtime and Tauri's prebuilt installer helper in `docs/dependencies.md`.

## 0.4.0 — 2026-10-12

- Added previewed chord thinning across all pitched parts jointly or a chosen part: highest note, bass plus highest distinct pitches, or the highest 1–8 note events at each exact onset tick. Percussion is excluded; ties use stable note IDs. This is a simplification heuristic, not melody extraction.
- Added whole-song selection below, above or outside configured keyboard limits, plus octave-move previews, independent of the visible note page/filter.
- Show exact paginated changes and a Before/After roll. Apply revalidates the saved revision and saves one Undo action on the practice copy; cancel leaves the lesson unchanged.
- Kept immediate pitch controls, schema-3 saves, Undo bounds and edited MIDI export behavior. No dependency changes. Windows builds/packages and 141 Rust executions passed; generated and easy-MIDI native workflows each passed 13 exercise and three restart checks. New physical acceptance remains pending; timing/duration editing, recording and M5–M6 remain deferred.

## 0.3.0 — 2026-10-12

- Added selected-note moves by one semitone or one octave in either direction. Repeated moves retain selection; moves crossing MIDI 0–127 are refused as a whole, while keyboard-range warnings remain advisory.
- Extended saved Undo to mixed pitch/removal edits, bounded to 100 actions and 200,000 individual note-change entries. Existing originals and 0.2.0 practice versions are retained; copied versions inherit current edits with fresh Undo history.
- Added edited MIDI export from the practice toolbar and arrangement details. Export creates a new file, keeps source tempo/timing and non-note events, and ignores practice speed, roles, hand split and loops. Original export remains byte-exact.
- Refuse empty or musically ambiguous edited exports instead of changing note timing. Expressive pressure is preserved without retargeting it to moved pitches; source SysEx is preserved in files but never sent to hardware by Keyloom.
- Timing/duration editing, automatic chord thinning and M5–M6 remain deferred. Release validation and remaining physical checks are recorded separately in the test report and hardware checklist.

## 0.2.0 — 2026-10-12

- Added named practice versions that preserve the imported original and keep independent practice settings.
- Added note selection/removal and saved Undo history, plus switching between originals and practice versions.
- Added editor range warnings and an accessible note list for selecting notes outside the displayed keyboard range.
- Protected originals with dependent practice versions from removal. Edited MIDI export and further note transformations remain deferred.

## 0.1.3 — 2026-10-12

- Light Guide now mutes Keyloom's PC audio, including key-press tones, accompaniment/Listen playback, count-in and metronome. Turn Light Guide off to restore PC audio; the chosen listening preference is retained.
- Apply the mute to already sounding and newly played tones, and restore it on launch when Light Guide was saved on. MIDI input, target matching and Yamaha guide messages are unchanged.
- Show the audio override beside Light Guide and the computer listening option. Added native callback checks for silence, unmute and restart persistence.
- Verified with physical Yamaha keys: PC audio stays silent with Light Guide on while the Yamaha sounds, and PC tones return when Light Guide is switched off.
- Added this changelog to the repository and Windows package resources.

## 0.1.2 — 2026-10-11

- Added Yamaha EZ-150 Light Guide with a separate MIDI output selector, saved toggle and two-second middle-C test. Target notes use MIDI channel 1.
- Clear guide notes on pause, stop, seek, song/settings changes, guide disable, output disconnect, Panic and normal exit; reconnect clears interrupted notes.
- Keep guidance and simulated input separate from physical player input. Simulated keys monitor only and cannot satisfy targets or score.
- Physically verified middle-C illumination, Wait-mode guidance and matching, and pause/stop/song-change light cleanup on the physical keyboard. Guide messages may also sound the Yamaha.

## 0.1.1 — 2026-10-11

- Made the practice strike line bright pink with a pale edge and dark separation, so the note-hit boundary is easier to see. Practice timing is unchanged.

## 0.1.0 — 2026-10-11

- Delivered the M0–M3 Windows application using Tauri, Rust, React, TypeScript and Canvas, with native USB MIDI input and a generated basic practice tone.
- Added a managed MIDI library with immediate import/open, duplicate detection, original-byte export and saved settings/progress.
- Added Listen, Wait for me and Timed practice; part selection, approximate left/right split, keyboard range, speed, count-in/metronome and A–B loops.
- Delivered local Windows installer and portable packages, third-party notices, automated regressions and real native import-to-library-to-play verification. Physical keyboard observations are tracked separately in the hardware checklist and test report.
