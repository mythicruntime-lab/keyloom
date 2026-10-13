# Keyloom handoff

## Current release — 0.4.0, 12 October 2026

The approved slice adds **Simplify and fit** preview tools on practice copies: exact-onset chord thinning, whole-song selection of pitched notes outside/below/above the configured keyboard range, and selected-note octave previews. Chord thinning considers all pitched parts jointly or one chosen nonpercussion part. It retains the highest one note, bass plus highest distinct pitches (one for a unison), or the highest N note events for N=1–8. Equal-pitch ties retain the lower stable note ID. It is a heuristic, not melody extraction; near-simultaneous onsets are not grouped and percussion is excluded.

Preview is read-only: inspect the exact paginated before→after/removal list and switch the roll between **Before** and **After**. **Apply preview** recomputes against the saved revision, saves the complete change as one Undo action and reloads paused. **Cancel preview** saves nothing. Range selection uses current edited pitches across the whole lesson, independent of visible notes, filters and list pages. ±12 octave previews refuse an invalid MIDI pitch rather than clamp it. Existing immediate pitch controls, schema 3, 100-action/200,000-note-change Undo limits and MIDI export behavior remain unchanged.

Validation passed: 141 Rust executions (56 unit, 70 library integration, 15 MIDI export; 97 distinct cases, one optional benchmark ignored), strict Clippy, Rust formatting, TypeScript, Prettier and Windows release build. Each final native run passed 13 exercise checks plus three restart checks. Generated fixtures took 17,213.2/892.7 ms and the owner's easy MIDI took 17,732.9/1,040.9 ms for exercise/restart respectively. These cover actual import→library→play and preview/cancel/apply/Undo/restart/export integration; simulated inputs and MIDI send success do not establish physical playing or optical confirmation. Evidence is in ignored `test-output/simplify-040-final` and `test-output/simplify-040-easy-final`, summarized in `docs/test-report.md`. Local frontend/backend source reviews found no actionable findings; no external review is claimed.

The tested/package executable SHA-256 is `bdc9634a4503344df8293e36d5fc37a9c32c5c232906bfccef9b05931c1ca876`. Installer and portable ZIP verification passed for all 580 portable files, all 562 license/notice resources and checksums. Dependency versions did not change; the license audit passed for 264 Rust and 81 npm packages. Builds/tests used conservative parallelism. These packages remain local; no push or publication is authorized.

Before normal 0.4.0 startup, the library was backed up to ignored `test-output/pre-040-user-library`. Startup preserved all five original rows, both practice-version rows, all five managed MIDI hashes, preferences and schema 3 exactly; evidence is `test-output/pre-040-baseline.json` and `test-output/normal-upgrade-040.json`. This preservation result precedes the owner's manual practice checks and does not claim later last-played/edit metadata stayed unchanged.

**New physical acceptance remains pending.** The owner is checking highest-note thinning on the demo's first chord (C4), Undo restoring C3+C4, and pause cleanup. The UI capture helper could not inspect the active window, so those observations must come from the owner; no new screenshot or hardware pass is inferred. Earlier physical guide/mute acceptance remains separately recorded below. Timing/duration editing, recording and M5–M6 remain deferred.

## Previous release — 0.3.0, 12 October 2026

The approved follow-up adds selected-note semitone/octave moves, persistent mixed pitch/removal Undo, and edited MIDI export to a new file. Originals remain unchanged. Each pitch batch is atomic within MIDI 0–127; selection stays active for repeated moves. Undo retains up to 100 actions/200,000 note changes. Copies inherit current edits with fresh history. Schema-2 versions migrate additively, with older-editor protection for new mixed histories.

Export keeps original tempo, ticks, tracks and non-note events, applying saved removals/pitches. It ignores practice speed, roles, split and loops. Original export remains byte-exact. Existing destinations, empty versions, exceeded limits and ambiguous same-pitch release pairings are refused. Pressure is preserved without retargeting; SysEx is retained in files but never sent to hardware. Timing/duration edits, chord thinning, recording and M5–M6 remain deferred.

Validation: 134 Rust executions (90 distinct cases; one optional benchmark ignored), strict Clippy, formatting, TypeScript, and Windows release build pass. Generated and owner-selected easy MIDI each pass 11 real native exercise checks plus three restart checks, including edit→export→immediate reimport→play, exact notes/timing, mixed Undo/restart, USB port/output, cleanup and PC mute. Native input simulations do not establish physical playing or LED observations. The previous physical guide/mute acceptance remains valid; moved/removed-target and exported-song hardware checks are still pending.

The closed normal library was backed up to ignored `test-output/pre-030-user-library`. Schema-3 upgrade verification preserved all five original rows, both existing practice-version rows, all five MIDI hashes and preferences exactly. Native visual QA then shifted one project-authored demo note and undid it, leaving zero shifted/removed demo notes; the demo revision/history format and last-played metadata advanced normally. The owner's private version was not edited. The real Save dialog and cancellation were checked; its generated demo export is retained only in ignored `test-output/manual-030-export.mid`. See `docs/test-report.md` and `docs/screenshots/pitch-editor-native.jpg`.

The tested executable SHA-256 is `6a91ace00b7e3311a22b692d6e5b9318a18ce3dfafb2b36e3844a226b499ae0c`. Installer and portable ZIP are built; all 580 portable files match their ZIP entries, all 562 license files match the source inventory, and release checksums validate. No dependency versions changed; the local license audit refreshed app-version/lockfile records. Existing public-release provenance/copyright questions remain below. Packages and the scoped release commit are local only; no push is authorized.

## Previous release — 0.2.0, 12 October 2026

The owner-approved first M4 slice is implemented: named practice versions, Canvas/list selection and note removal, persistent Undo for the last 100 removals, original/version switching and keyboard-range warnings. Original MIDI bytes/canonical data stay unchanged; edits live separately with independent settings and revisions. Playback pauses for editing; simulated UI notes cannot score or play during editing. Saving a copy of a version starts from its current notes with fresh Undo history. Originals with dependent versions cannot be removed. Automatic simplification, pitch/time/duration editing, edited MIDI export, recording and M5–M6 remain outside this approval.

Validation passed: 111 Rust executions (55 unit + 56 integration; 71 distinct cases, one optional benchmark ignored), strict Clippy, formatting, TypeScript and Windows production build. Two real native runs each passed ten exercise checks and three restart checks: generated fixtures and the owner's selected 2,171-note easy MIDI. They cover import→library→play, create/remove/Undo/version comparison, independent settings/restart, range warnings and input isolation, plus real USB MIDI port/output and PC-mute regression checks. These runs do not claim physical playing or optical verification of edited versions. Review found and fixed a delayed Play/editor race and misleading post-commit playback-reload failures.

Before upgrading, the closed five-song normal library was backed up to ignored `test-output/pre-020-user-library`. The schema-2 upgrade preserved all five original database rows, MIDI file hashes and preferences exactly. Native visual QA then created `First threads – practice` as a ready-to-try copy in the normal library, selected a visible note on Canvas and finished editing without removing any notes. There are now five originals plus this practice copy. Existing private arrangements were not edited or included in the package. See `docs/test-report.md` and `docs/screenshots/lesson-editor-native.png`.

## Previous audio follow-up — 12 October 2026

The owner clarified that Light Guide should mute Keyloom's PC tone while they listen to the Yamaha. Version 0.1.3 applies a PC master mute while the saved Light Guide toggle is on, including already sounding/new input tones, arrangement playback and clicks. Turning it off restores PC audio while keeping the listening preference. MIDI messages, physical input and matching remain unchanged. This does not claim silent Yamaha illumination; guide Note Ons can still sound the keyboard.

`CHANGELOG.md` covers the local releases and ships in both package formats. Version 0.1.3 passed 102 Rust test executions, strict Clippy, formatting, TypeScript/build, and nine real native exercise checks plus two restart checks. Actual audio callbacks emitted zero nonzero samples with guidance on and resumed tones with it off; simulated keys remained unable to score. See `docs/test-report.md` for measured counters. **Physical listening acceptance passed with the owner on 12 October 2026:** with PC monitoring selected, physical Yamaha key presses sounded the Yamaha while the PC stayed silent with Light Guide on; switching Light Guide off restored PC tones. The owner's confirmation was "Yes—PC is silent with guide on, and returns with guide off." This is separate from the automated sample measurements and the earlier 0.1.2 optical checks. Switchwall tools remain unavailable. No public push or remote is configured.

## Delivered state

M0–M3 and the approved practice-version/removal, pitch/export and 0.4.0 preview slices are implemented and software-verified. The app uses the proposed Tauri 2/Rust/React/TypeScript/Canvas architecture with native WinMM input and WASAPI basic-tone output. Other M4 roadmap work, M5–M6 and external services are deferred. No publishing/pushing, unrelated-project edits, agent-process interference, driver/overlay changes, scheduled tasks or services occurred.

The owner's chosen easy MIDI is Ready in the normal managed library, with 2,171 notes in one combined part. It passed the actual installed import-to-library-to-play workflow without restart. The source remains private and untouched; it is not in Git or any release resource. The original generated First threads exercise is also available, and the owner has since added more songs.

The 0.1.1 display update gives the strike line a fixed bright pink band, pale sharp top edge and dark separation from the notes/keys. The timing boundary and practice behavior are unchanged. The owner reports Wait for me and reconnect working well on the Yamaha; no pedal is owned, so physical pedal testing is not applicable.

0.1.2 adds the explicitly requested Yamaha EZ-150 Light Guide: separate MIDI output selection, channel-1 target Note On/Off, a two-second middle-C test, saved Light Guide toggle and cleanup. **Physical acceptance passed with the owner on 11 October 2026:** middle C lit and cleared; Wait stayed still hands-off, advanced only after physical playing and lit the next correct keys; pause, stop and song changes each cleared the physical lights. These are owner-observed hardware results, not inferred from MIDI send success. Software simulation/playback cannot score; externally echoed MIDI is excluded from supported routing because MIDI carries no physical-origin marker.

## Run and build

- Run `artifacts/Keyloom-0.4.0-windows-x64/Keyloom.exe`, or install `artifacts/Keyloom_0.4.0_x64-setup.exe`.
- Portable ZIP: `artifacts/Keyloom-0.4.0-windows-x64.zip`; hashes: `artifacts/SHA256SUMS.txt`.
- From this checkout: `powershell -File scripts/Launch.ps1`.
- Restore/build: `powershell -File scripts/Build.ps1 -Mode restore`, then `powershell -File scripts/Build.ps1 -Mode package -Jobs 2`.
- Built apps need Windows x64 and installed WebView2; no development tools, Python, network, account or administrator access. These builds are unsigned.

Open Keyboard & sound, select USB MIDI Interface, and choose Yamaha speakers or PC basic tone. Calibrate the actual keyboard endpoints. For Light Guide, connect adapter OUT → Yamaha IN, enable the Yamaha's OVERALL o1 setting, connect the MIDI output and enable Light Guide. Open a song, choose Wait for me, then Play. Its combined part can use the explicitly approximate split for one-hand practice. Output is limited to guide notes; MIDI Thru and imported SysEx are never sent.

Open a song → **Save practice version** → **Create practice version** → **Edit notes**. Select notes on the roll or list, move them with **Semitone down/up** or **Octave down/up**, or remove them. **Undo edit** reverses moves and removals, including after restart. Choose **Done editing** to play; **Lesson version** compares with the untouched original. **Export MIDI** writes all saved notes at original tempo to a new file, ignoring practice speed/roles/loops.

For 0.4.0, expand **Simplify and fit** in the editor. Choose a chord strategy and scope, then **Preview chord thinning**. Range buttons select all matching pitched notes in the lesson; **Preview octave down/up** shows their proposed moves. Review **Before/After** and the change list before **Apply preview**, or **Cancel preview**. A single **Undo edit** reverses the applied batch.

Data lives under `%LOCALAPPDATA%\app.keyloom.desktop`. Import copies sources; remove deletes only the confirmed managed copy or practice-version record. Uninstall retains data unless the interactive delete-data checkbox is selected. Keep this directory when updating/backing up. Normal app shutdown ends all its workers.

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
- 0.1.2: 102 Rust test executions passed (63 distinct cases; one optional benchmark ignored), strict Clippy, TypeScript and production build passed. Nine native exercise checks plus persistence restart passed, including actual USB output, a two-second C4 test, unchanged score from guide/simulated notes, and pause/stop/song-change cleanup. The owner separately verified the physical illumination and playing behavior above.

## Honest remaining boundaries

Basic physical Wait for me, reconnect and Light Guide PC mute/unmute are owner-reported passes. Detailed repeated-note/chord/velocity edge cases, long-session sound/dropouts, actual range, 4K scaling and end-to-end latency still need confirmation. Physical pedal testing is not applicable to the owner's setup. Follow `docs/hardware-checklist.md`; port opening and simulated targets do not independently prove physical playing.

Sound is a labeled basic synth tone, not a sampled piano. SoundFonts, full General MIDI fidelity, general playback MIDI output, time/duration editing, recording, MusicXML/OMR, transcription, downloading, cloud/accounts and LLM features are deferred. Chord thinning is a previewed pitch/onset heuristic and does not identify melody. Guide output may sound the Yamaha as well as light keys; no independent silent-LED protocol is claimed. Listen sends no guide notes. Physically confirming moved/removed/thinned targets and reimported edited MIDI on a practice version, long sessions, cable-loss recovery with lit notes, Timed guidance and failed-driver cleanup remain additional hardware checks. Type 2/SMPTE MIDI is explicitly unsupported. If the Windows audio output changes/fails, restart Keyloom to reopen it. No other-platform claim is made.

Before any public release, confirm copyright-holder wording in LICENSE and review the remaining exact-binary provenance of Tauri's prebuilt installer helper and Microsoft static CRT redistribution rights in `docs/dependencies.md`. MPL source/notice obligations are handled by the shipped unmodified source archives; all dependency terms remain separate from project MIT. Optional upstream taskbar-unpin code was omitted; manually unpin retained taskbar pins after uninstall.

Use normal Windows permissions for temp-directory tests on this machine. Build scripts shorten PATH only in their own process and default to two workers. Switchwall is available in inbox mode. The 0.4.0 review evidence is local source inspection and tests, with no external review claimed. No automated background follow-up was installed.
