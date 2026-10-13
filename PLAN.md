# Keyloom milestones

Implement the supplied Piano Trainer brief as an original, offline Windows application. The approved follow-up below defines the current scope; no public publishing is authorized.

## Approved follow-up — 0.4.0, software delivered, 12 October 2026

Built on clean local commit `b8e625a` with previewed chord thinning and keyboard-range fitting. The 0.4.0 Windows build and packages passed verification: 141 Rust executions (97 distinct cases; one optional benchmark ignored), strict Clippy, formatting, TypeScript and Prettier. Generated and owner-selected easy MIDI each passed 13 real native exercise checks plus three restart checks. Normal startup preserved schema-3 library rows, source hashes and preferences. No dependency changes; the license audit passed. Physical acceptance of the new preview/apply targets remains pending and is tracked separately. Heavy builds/tests ran one at a time with two workers.

Chord tools group only by exact source onset tick, across all pitched parts jointly or one chosen nonpercussion part. They offer highest one note; bass plus highest distinct pitches (one note for a unison); or the highest N note events, N=1–8. Equal-pitch ties retain lower stable note IDs. Percussion is excluded. This is a deterministic simplification heuristic, not melody extraction or grouping of nearby onsets.

Whole-song selection covers surviving pitched notes below, above or outside the configured keyboard endpoints, independent of the editor page/filter; octave previews move selected notes by −12/+12. A read-only exact change list with pagination and a Before/After roll exposes each proposal. Apply recomputes against the expected saved revision, saves one atomic Undo action on a practice copy, and reloads paused; cancel changes nothing. Immediate pitch controls, schema 3, history bounds and verified MIDI export behavior remain unchanged. Timing/duration editing, recording and M5–M6 remain deferred.

### Previous 0.3.0 approval

The next approved slice builds on local commit `c99f528`: move selected notes by a semitone or octave, persist a mixed pitch/removal Undo history, and export an edited practice copy to a new MIDI file. Delivered as 0.3.0; 134 Rust executions and two native runs (11 exercise + three restart checks each) pass. Originals remain unchanged; note timing/duration editing, chord thinning and M5–M6 remain deferred. Validate exported notes and timing by reimporting through the canonical parser, including the real native UI workflow. Baseline: 111 Rust executions and TypeScript pass; no dependency changes planned. One heavy build/test at a time, two workers.

### Previous 0.2.0 approval

The owner approved the first M4 slice after accepting M0–M3 and Yamaha Light Guide/PC mute checks: named practice versions, selecting/removing notes with persistent Undo, switching between original and edited versions, and keyboard-range warnings. Imported originals remain untouched. Pitch moves and edited MIDI export were subsequently approved above; timing/duration editing, automatic chord simplification, recording, M5 and M6 remain deferred.

Implementation checkpoint: preserve the existing library schema/data while adding separate persisted variants; serialize editor changes with playback and reload paused; extend the existing roll for visual selection plus an accessible bounded note list; verify real native create/edit/undo/switch/play/restart and package Windows 0.2.0. Baseline at `f3f2768` is clean, with the previously passed 102 Rust executions and 11 native exercise/restart checks. One heavyweight build/test runs at a time with two workers.

## Original delivery

1. **M0:** Tauri 2 / React / TypeScript window, native WinMM MIDI input, WASAPI basic practice tone, moving Canvas roll, explicit simulated-input mode. Verify a runnable Windows executable before integrating the library.
2. **M1:** Bounded canonical MIDI parser, crash-safe SQLite managed library, immediate import/open, metadata and settings persistence, native playback and part controls.
3. **M2:** Authoritative Rust transport and chord matching, falling notes, range calibration, listen/wait/timed modes, loops, monitoring and device recovery.
4. **M3:** Generated-fixture regressions, real native import→query→open→play verification, 1,000-song exercise, notices audit, reproducible Windows package and handoff.

One heavy build at a time; Cargo jobs defaults to 2. Lead owns integration, audio/MIDI, packaging, verification, and documentation. Delegates have separate source ownership. General playback MIDI output and SoundFonts are deferred; basic tone is explicitly synthetic. No private songs or downloaded assets are bundled. The owner's subsequent explicit request authorizes the narrow EZ-150 Light Guide output implemented in 0.1.2, without starting M4–M6.

Environment: Windows 11; Node 24.11.1, npm 11.6.2, Rust/Cargo 1.98.0, VS 2022 Build Tools 17.14.21 with C++ tools and Windows SDK 10.0.26100.0, WebView2 154.0.4258.62. Rust exists under the user's Cargo bin directory but is absent from the shell PATH. No baseline source/tests existed.

M0–M3 and practice-version editing are implemented. Runnable local milestone commits preserve the spike, library and practice implementation; M3 adds tested Windows installer/portable delivery, exact dependency notices, generated-fixture and real native workflow evidence. The owner confirmed physical Wait for me/reconnect in 0.1.1 and EZ-150 Light Guide illumination, physical matching and pause/stop/song-change cleanup in 0.1.2. Version 0.2.0 added saved versions/removals; 0.3.0 added pitch moves/export; 0.4.0 adds the software-verified preview tools above. Other M4 features and M5–M6 remain deferred. Additional hardware checks are distinguished in `HANDOFF.md` and `docs/hardware-checklist.md`.
