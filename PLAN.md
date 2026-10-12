# Keyloom milestones

Implement the supplied Piano Trainer brief as an original, offline Windows application. The approved follow-up below defines the current scope; no public publishing is authorized.

## Approved follow-up — 12 October 2026

The owner approved the first M4 slice after accepting M0–M3 and Yamaha Light Guide/PC mute checks. This supersedes the original M4 deferral only for: named practice versions, selecting/removing notes with persistent Undo, switching between original and edited versions, and keyboard-range warnings. Imported originals remain untouched. Pitch/time/duration editing, automatic chord simplification, edited MIDI export, recording, M5 and M6 remain deferred. No public publishing is authorized.

Implementation checkpoint: preserve the existing library schema/data while adding separate persisted variants; serialize editor changes with playback and reload paused; extend the existing roll for visual selection plus an accessible bounded note list; verify real native create/edit/undo/switch/play/restart and package Windows 0.2.0. Baseline at `f3f2768` is clean, with the previously passed 102 Rust executions and 11 native exercise/restart checks. One heavyweight build/test runs at a time with two workers.

## Original delivery

1. **M0:** Tauri 2 / React / TypeScript window, native WinMM MIDI input, WASAPI basic practice tone, moving Canvas roll, explicit simulated-input mode. Verify a runnable Windows executable before integrating the library.
2. **M1:** Bounded canonical MIDI parser, crash-safe SQLite managed library, immediate import/open, metadata and settings persistence, native playback and part controls.
3. **M2:** Authoritative Rust transport and chord matching, falling notes, range calibration, listen/wait/timed modes, loops, monitoring and device recovery.
4. **M3:** Generated-fixture regressions, real native import→query→open→play verification, 1,000-song exercise, notices audit, reproducible Windows package and handoff.

One heavy build at a time; Cargo jobs defaults to 2. Lead owns integration, audio/MIDI, packaging, verification, and documentation. Delegates have separate source ownership. General playback MIDI output and SoundFonts are deferred; basic tone is explicitly synthetic. No private songs or downloaded assets are bundled. The owner's subsequent explicit request authorizes the narrow EZ-150 Light Guide output implemented in 0.1.2, without starting M4–M6.

Environment: Windows 11; Node 24.11.1, npm 11.6.2, Rust/Cargo 1.98.0, VS 2022 Build Tools 17.14.21 with C++ tools and Windows SDK 10.0.26100.0, WebView2 154.0.4258.62. Rust exists under the user's Cargo bin directory but is absent from the shell PATH. No baseline source/tests existed.

M0–M3 are implemented. Runnable local milestone commits preserve the spike, library and practice implementation; M3 adds tested Windows installer/portable delivery, exact dependency notices, generated-fixture and real native workflow evidence. The owner confirmed physical Wait for me/reconnect in 0.1.1 and EZ-150 Light Guide illumination, physical matching and pause/stop/song-change cleanup in 0.1.2. The approved first M4 slice above is implemented for 0.2.0; the rest of M4 and M5–M6 remain deferred. Additional hardware checks are distinguished in `HANDOFF.md` and `docs/hardware-checklist.md`.
