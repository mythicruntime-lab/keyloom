# Keyloom M0–M3

Implement the supplied Piano Trainer brief as an original, offline Windows application. No M4–M6 work or public publishing.

1. **M0:** Tauri 2 / React / TypeScript window, native WinMM MIDI input, WASAPI basic practice tone, moving Canvas roll, explicit simulated-input mode. Verify a runnable Windows executable before integrating the library.
2. **M1:** Bounded canonical MIDI parser, crash-safe SQLite managed library, immediate import/open, metadata and settings persistence, native playback and part controls.
3. **M2:** Authoritative Rust transport and chord matching, falling notes, range calibration, listen/wait/timed modes, loops, monitoring and device recovery.
4. **M3:** Generated-fixture regressions, real native import→query→open→play verification, 1,000-song exercise, notices audit, reproducible Windows package and handoff.

One heavy build at a time; Cargo jobs defaults to 2. Lead owns integration, audio/MIDI, packaging, verification, and documentation. Delegates have separate source ownership. MIDI output and SoundFonts are deferred; basic tone is explicitly synthetic. No private songs or downloaded assets are bundled.

Environment: Windows 11; Node 24.11.1, npm 11.6.2, Rust/Cargo 1.98.0, VS 2022 Build Tools 17.14.21 with C++ tools and Windows SDK 10.0.26100.0, WebView2 154.0.4258.62. Rust exists under the user's Cargo bin directory but is absent from the shell PATH. No baseline source/tests existed.
