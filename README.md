# Keyloom

An original, offline Windows piano trainer built with Tauri 2, Rust, React, TypeScript and Canvas. Import a MIDI, open it immediately, and practice through a native USB MIDI input. Project-authored code is prepared for MIT licensing; copyright-holder wording remains to be confirmed before public release.

## Run

In a downloaded/extracted portable package, double-click `Keyloom.exe`. In this checkout, use `artifacts/Keyloom-0.4.0-windows-x64/Keyloom.exe`, or install `artifacts/Keyloom_0.4.0_x64-setup.exe`. These are unsigned local builds. Do not bypass Windows security warnings; review the source/build provenance or build locally if Windows blocks execution.

Windows 11 x64 and the installed Microsoft Edge WebView2 Evergreen Runtime are required. WebView2 is already present on the development machine. The package does not download or install it. No administrator access, Python, Node, Rust, account, server, internet connection or NVIDIA setting change is needed to run the built app.

1. Choose **Import MIDI** or drop `.mid` / `.midi` files into the window. The result exposes **Open**, even when search or Favorites hides the song.
2. Open **Keyboard & sound**, refresh devices, select your USB MIDI interface, and connect it. Select Yamaha speakers or PC basic-tone monitoring. Optional Light Guide output is configured separately. Imported SysEx/configuration messages and MIDI Thru are never sent to the keyboard.
3. Set the keyboard's actual lowest/highest keys, or use the endpoint learning buttons. The initial range is only a display starting point; it is not a detected key count.
4. Open **Practice**, select Listen, Wait for me, or Timed, assign parts, and press Play. Play as a note's leading edge reaches the **bright pink line above the keys**. In Wait mode all target pitches require fresh key presses; the pedal does not satisfy notes. For a combined part, the adjustable pitch split is an approximate left/right guide and practice filter, not melody extraction.
5. Use Pause, Restart, Stop, seek, A–B loops, speed, count-in/metronome, or Panic as needed. Keyboard, part, loop, speed and position settings persist.

**Basic practice tone** is a generated synth sound, not a sampled piano. Your Yamaha's own speakers are the default player-monitoring choice. Accompaniment and Listen playback use the PC tone. SoundFonts and authentic General MIDI instrumentation are not included.

## Yamaha EZ-150 Light Guide

Connect the USB adapter's **OUT** plug to the Yamaha's **MIDI IN** (keep the existing Yamaha OUT → adapter IN connection for your playing). On the Yamaha, press **OVERALL** until **o1**, select **On**, and stop any built-in song playback. Yamaha documents channel-1 illumination on page 54 and its Light Guide setting on page 42 of the [EZ-150 manual](https://usa.yamaha.com/files/download/other_assets/0/320940/EZ150E.pdf).

In **Keyboard & sound**, choose the USB interface under **MIDI output**, then **Connect output**. **Test middle C** sends MIDI note 60 on channel 1 for two seconds and pauses practice. Turn on **Light Guide** there or in Practice. It lights the current target chord in Wait for me, or during Timed's matching window. Listen sends no guide notes; muted/accompaniment/percussion and notes outside your configured key range are excluded. Note messages can also sound the keyboard.

Guide notes clear on pause, stop, seek, song/settings changes, guide disable, output disconnect, Panic and normal app exit. Reconnecting sends channel-1 Note Offs to clear interrupted notes. Keep other apps' MIDI Thru/echo off: returned MIDI has no marker proving whether it came from physical keys. Keyloom never routes guidance internally into player input, and simulated on-screen/computer keys now monitor only; they cannot satisfy targets or score.

**Light Guide mutes all Keyloom PC audio** so you can listen to the Yamaha: key-press tones, Listen/accompaniment, count-in and metronome are silent while the toggle is on. Turn Light Guide off to restore PC audio. Your Yamaha/computer listening choice is retained, and the mute survives restart. This changes PC sound only; MIDI guide notes may still sound the Yamaha itself.

## Library and files

### Make an easier practice version

Open a song and choose **Save practice version**, give the copy a name, then **Create practice version**. It appears in the same library and has its own practice settings. The **Lesson version** selector switches between the original and its versions.

In a practice version, choose **Edit notes**. Playback pauses while you select notes on the roll or in the note list. **Semitone down/up** moves the selection by one semitone; **Octave down/up** moves it by twelve. Selection stays active for repeated moves. **Remove selected notes** removes the selection, and **Undo edit** reverses either kind of edit. Use **Done editing** to return to practice. Note timing and duration stay unchanged. A whole move is disabled if any selected pitch would leave MIDI 0–127; notes are never clamped. Keyboard-range warnings still allow deliberate moves outside your physical range.

Immediate note moves and removals save when pressed; preview tools below save only after Apply. Mixed Undo history survives restarts, retaining up to 100 actions and 200,000 individual note-change entries; large selections can retain fewer actions. Saving another practice version copies its current removals and pitches with fresh Undo history. The imported original stays unchanged. Existing original libraries and 0.2.0 practice versions remain compatible.

### Preview a simpler or more reachable part

The approved 0.4.0 tools are under **Simplify and fit** in a practice version's note editor. For chord thinning, choose **All pitched parts** or one nonpercussion part, then a strategy:

- **Highest note** keeps one note at each exact onset tick.
- **Bass + highest** keeps one note at each of the lowest and highest distinct pitches; a unison keeps one.
- **Limit highest notes** keeps the highest N note events, from 1 to 8. Equal-pitch events count separately toward N; ties keep the lower stable note ID.

Choose **Preview chord thinning** to inspect the result. All pitched parts are considered jointly when that scope is selected. Percussion is excluded, and notes that begin even one tick apart are separate groups. This is a pitch-based simplification heuristic, not melody extraction; it may remove musical material you want to keep.

To fit your keyboard, use **Select all outside range**, **Select below range**, or **Select above range**. These replace the selection using every surviving pitched note in the current version, independent of the note-list page/filter and visible roll. A note on either configured endpoint is in range. Choose **Preview octave down** or **Preview octave up** to inspect a twelve-semitone move; a move beyond MIDI 0–127 is refused rather than clamped.

The preview shows every proposed pitch change or removal in a paginated list and switches the roll between **Before** and **After**. **Cancel preview** saves nothing. **Apply preview** saves the entire batch as one **Undo edit** action after checking that the saved version has not changed. Previewing does not play simulated notes, alter the original or change the saved lesson. Existing immediate semitone/octave buttons still save as soon as you press them; only the explicitly labeled preview tools wait for Apply. Finish editing to practice the applied result.

After is shown first. Other edits, Undo and export are disabled while a preview is open; apply or cancel it before continuing. Changing the tool options, range selection, keyboard range or lesson discards the previous preview so it cannot be applied to a different setup.

Choose **Export MIDI** in the practice-version toolbar or arrangement details to save a **new** file. A version export includes all saved notes at original tempo; practice speed, part roles, hand split and loops do not alter the file. Original-song export remains byte-exact. Version export preserves source timing and non-note events, but does not retarget expressive pressure to moved pitches. Ambiguous overlapping same-pitch releases and empty versions cannot be exported; see [MIDI policy](docs/midi-policy.md).

Versions store their edits separately and refer to the original's managed MIDI. Remove dependent versions before removing that original from the library. Normal application data lives under `%LOCALAPPDATA%\app.keyloom.desktop`: `library.sqlite3`, managed `midi` originals and preferences. Source files are copied, never moved. Byte-identical files share one entry; different arrangements remain separate. Export refuses existing destination files. Removal asks for confirmation and deletes only the managed copy/record, never the external original. Keep the data directory when updating. Uninstall can retain this library; do not select an offered application-data deletion option if you want to keep your songs.

MIDI 0/1 with musical tick division are supported. Type 2 and SMPTE division receive explicit unsupported-format errors. See [MIDI policy](docs/midi-policy.md) for malformed input, event budgets, tempo conflicts, sustain and missing-release behavior.

Open a file directly:

```powershell
& .\artifacts\Keyloom-0.4.0-windows-x64\Keyloom.exe 'C:\Music\easy song.mid'
```

A second launch forwards the file to the running instance. During interactive installation you can optionally add Keyloom to Windows **Open with** for MIDI; the installer does not replace your default application. Silent installation skips that optional registration. Uninstall removes its own ordinary shortcuts. If you manually pinned Keyloom to the taskbar, unpin it manually after uninstall.

## Build and check

Requirements: Node 24, Rust 1.98 MSVC, Visual Studio 2022 C++ Build Tools, Windows SDK and WebView2. Exact packages are locked. First-time dependency downloads need internet. Build commands only adjust their own process environment. Cargo defaults to two workers; use `-Jobs 1` to reduce load further.

```powershell
powershell -File scripts/Build.ps1 -Mode restore
powershell -File scripts/Build.ps1 -Mode dev
powershell -File scripts/Build.ps1 -Mode test -Jobs 2
powershell -File scripts/Build.ps1 -Mode package -Jobs 2
powershell -File scripts/Launch.ps1
```

The launcher accounts for this machine's unusually long inherited PATH, which otherwise makes npm's Windows command shell lose executables. It never changes the registry or global environment. Tests that create temporary directories run as the normal Windows user on this workstation.

Packaging writes the installer, portable folder, ZIP and `SHA256SUMS.txt` under `artifacts`. To refresh packages after updating notices/docs, run `powershell -File scripts/Build.ps1 -Mode bundle`. This does not compile changed source; use `Build.ps1 -Mode package` for code changes.

See [test report](docs/test-report.md), [hardware checklist](docs/hardware-checklist.md), [architecture](docs/architecture.md), [timing](docs/timing.md), [dependency decisions](docs/dependencies.md), and [handoff](HANDOFF.md). Reproduce the native UI regression with `scripts/Native-Smoke.ps1` after building. The opt-in test uses an isolated data directory and explicitly simulated keys; it is not a Yamaha test.

## Release boundaries

The approved M4 work supports named practice versions, selected-note removal and semitone/octave moves, previewed chord thinning/range fitting, persistent mixed Undo, and edited MIDI export. Timing/duration editing and recording remain deferred, along with M5–M6: MusicXML/OMR, transcription and downloading. No cloud, account system or LLM dependency exists. No commercial songs, fonts, samples or SoundFonts are distributed. The generated original demo and app icon are project assets. Third-party components retain their own terms; ship `THIRD_PARTY_NOTICES.md` and the complete `licenses` directory, including MPL source archives, with every package.
