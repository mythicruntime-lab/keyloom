# Keyloom handoff

## Delivered state

M0–M3 are implemented and software-verified. The app uses the proposed Tauri 2/Rust/React/TypeScript/Canvas architecture with native WinMM input and WASAPI basic-tone output. No M4–M6 features or external services were added. No publishing/pushing, unrelated-project edits, agent-process interference, driver/overlay changes, scheduled tasks or services occurred.

The owner's chosen easy MIDI is Ready in the normal managed library, with 2,171 notes in one combined part. It passed the actual installed import-to-library-to-play workflow without restart. The source remains private and untouched; it is not in Git or any release resource. The original generated First threads exercise is also available, and the owner has since added more songs.

The 0.1.1 display update gives the strike line a fixed bright pink band, pale sharp top edge and dark separation from the notes/keys. The timing boundary and practice behavior are unchanged. The owner reports Wait for me and reconnect working well on the Yamaha; no pedal is owned, so physical pedal testing is not applicable.

## Run and build

- Run `artifacts/Keyloom-0.1.1-windows-x64/Keyloom.exe`, or install `artifacts/Keyloom_0.1.1_x64-setup.exe`.
- Portable ZIP: `artifacts/Keyloom-0.1.1-windows-x64.zip`; hashes: `artifacts/SHA256SUMS.txt`.
- From this checkout: `powershell -File scripts/Launch.ps1`.
- Restore/build: `powershell -File scripts/Build.ps1 -Mode restore`, then `powershell -File scripts/Build.ps1 -Mode package -Jobs 2`.
- Built apps need Windows x64 and installed WebView2; no development tools, Python, network, account or administrator access. These builds are unsigned.

Open Keyboard & sound, select USB MIDI Interface, and choose Yamaha speakers or PC basic tone. Calibrate the actual keyboard endpoints. Open the easy arrangement, choose Wait for me, then Play. Its combined part can use the explicitly approximate split for one-hand practice. MIDI output/Thru is absent; imported SysEx never reaches the Yamaha.

Data lives under `%LOCALAPPDATA%\app.keyloom.desktop`. Import copies sources; remove deletes only the confirmed managed copy. Uninstall retains data unless the interactive delete-data checkbox is selected. Keep this directory when updating/backing up. Normal app shutdown ends all its workers.

## Evidence and checkpoints

- `0f45cf7`: runnable M0 MIDI/audio/display spike.
- `0a62ff1`: M1 canonical managed library and native playback.
- `bbc4cd1`: M2 native practice/matching/part/session behavior.
- M3 package/notice/test evidence is committed with this handoff. Binaries, private data, temporary files and raw test logs stay ignored.
- 73 Rust test executions (43 distinct cases) passed; strict Clippy, TypeScript/build and formatting passed.
- Installed owner-selected MIDI and portable generated exercises each passed eight real native UI checks plus restart. Actual native MIDI port open/close/reopen passed; wait matching used explicitly simulated keys.
- Running-instance file delivery, duplicate imports, source moves, interrupted-import recovery, 1,000-song import/search and retained settings/progress passed.
- Installer/uninstaller tests passed and retained normal data; all 562 license/source resource files matched. Final binaries and checksums are local only.
- Measurements, reproducible commands, screenshots and limits: `docs/test-report.md`.
- 0.1.1: TypeScript, component formatting and Windows production build passed; the actual native import/play exercise passed all eight checks plus persistence restart, using an isolated test library. Wait matching in that test was simulated; physical Wait for me and reconnect results are the owner's report.

## Honest remaining boundaries

Basic physical Wait for me and reconnect are owner-reported passes. Detailed repeated-note/chord/velocity edge cases, audible sound/dropouts, actual range, 4K scaling and end-to-end latency still need confirmation. Physical pedal testing is not applicable to the owner's setup. Follow `docs/hardware-checklist.md`; port opening and simulated targets do not independently prove physical playing.

Sound is a labeled basic synth tone, not a sampled piano. SoundFonts, full General MIDI fidelity, MIDI output, detailed note editing, MusicXML/OMR, transcription, downloading, cloud/accounts and LLM features are deferred. Type 2/SMPTE MIDI is explicitly unsupported. If the Windows audio output changes/fails, restart Keyloom to reopen it. No other-platform claim is made.

Before any public release, confirm copyright-holder wording in LICENSE and review the remaining exact-binary provenance of Tauri's prebuilt installer helper and Microsoft static CRT redistribution rights in `docs/dependencies.md`. MPL source/notice obligations are handled by the shipped unmodified source archives; all dependency terms remain separate from project MIT. Optional upstream taskbar-unpin code was omitted; manually unpin retained taskbar pins after uninstall.

Use normal Windows permissions for temp-directory tests on this machine. Build scripts shorten PATH only in their own process and default to two workers. Switchwall tools were unavailable in this session. No automated background follow-up was installed.
