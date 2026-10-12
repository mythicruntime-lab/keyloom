# Keyloom 0.1.0 verification

Verified on 11 October 2026, Windows 11 x64, Node 24.11.1/npm 11.6.2, Rust/Cargo 1.98.0, VS 2022 Build Tools 17.14.21, Windows SDK 10.0.26100.0 and WebView2 154.0.4258.62. One heavy build/test at a time, two Cargo workers. No other projects, agent processes, driver/overlay settings, services or scheduled tasks were changed.

## Automated regression

The final Rust suite passed **73 executions**: 34 unit and 39 integration executions, representing 43 distinct cases (model/engine cases also run in the integration binary). No failures. The opt-in 1,000-song benchmark was skipped in that suite because it had already passed separately.

Coverage includes MIDI 0/1, tempo changes/conflicts, sustain versus key release, running status, track/channel isolation, chords, repeated/overlapping notes, missing releases, malformed/truncated/extreme files, format-2/SMPTE rejection, byte duplicates, different arrangements, long paths containing spaces/Japanese characters, immediate canonical import/query/open/play, source moves, export/remove, interrupted imports/deletes, persisted metadata/settings/position, and old settings compatibility. Practice tests use deterministic elapsed deltas and explicitly injected input: wait gates, fresh repeated presses, wrong extras, partial timed-chord credit, loops, seek, speed, split-hand accompaniment, percussion exclusion, source isolation and unavailable/overflowing audio queues.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features custom-protocol -- --test-threads=2
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features custom-protocol -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run format:check
npm run build
```

Strict Clippy, Rust formatting, Prettier, TypeScript and Vite production build passed. Review caught and fixed lost timed partial-chord credit and concurrent device/preferences selection ordering. Final native inspection prompted idle polling reduction and a practice-height correction when an import banner is present; the resulting UI was rebuilt and retested natively.

## Actual native workflows

M0 compiled and launched a real Tauri window with moving Canvas roll, native MIDI enumeration and WASAPI callback operation. [M0 screenshot](screenshots/m0-native.png) records that integration spike.

The final installed executable passed eight exercise checks plus restart. It imported the owner's selected easy MIDI while search and Favorites hid it, exposed direct Open, played through the native transport, imported a second arrangement during practice without restart, repeated 12 byte-identical imports without duplicates, saved metadata/favorite/speed/position, and satisfied a wait chord with explicitly simulated input. Restart preserved both arrangements and settings. The real **USB MIDI Interface** opened/closed/reopened twice while native audio remained available. Physical notes and cable removal were not verified.

The final portable executable independently passed the same eight-check exercise and restart using original generated MIDI. Its explicit test mode is dormant on ordinary launch and uses isolated data directories. Neither the private MIDI nor test databases/logs are shipped.

A separate ordinary-launch check started a second OS process with a generated MIDI path. That process forwarded the file and exited 0; the first process stayed running and its actual library immediately showed two arrangements. The normal library retains the owner's private easy arrangement and the original demo. The private source was not modified.

```powershell
powershell -File scripts/Native-Smoke.ps1 -Device 'USB MIDI Interface'
# Optional owner-selected local song, never bundled:
powershell -File scripts/Native-Smoke.ps1 -Midi 'C:\Music\easy song.mid' -Device 'USB MIDI Interface'
```

Use a fresh isolated TestRoot and close other Keyloom instances first. Raw local evidence is under ignored `test-output`: `native-debug-1`, `release-easy-1`, `release-final-generated`, `installed-easy-final`, `portable-final-generated`, `second-instance.json`, installation/uninstallation and idle reports. Each exercise/restart returned exit 0. An early wrapper incorrectly inspected stale PowerShell LASTEXITCODE after a script; the native reports were checked directly and were successful. ZIP packaging was verified in Windows PowerShell 5.1 after correcting out-of-range upstream timestamps in staged copies and checksum-command compatibility.

Actual [windowed practice](screenshots/practice-native.png), [fullscreen practice](screenshots/fullscreen-native.png) and [device setup](screenshots/devices-native.png) screenshots accompany delivery. The final banner layout, fullscreen entry and return were checked in the real portable window. Captures show original generated music, not the owner's private arrangement.

## Measurements, not guarantees

| Measurement | Observed result |
| --- | --- |
| Installed easy MIDI visible acknowledgment | 65.4 ms |
| Installed easy MIDI ready/direct Open | 134.6 ms |
| First installed process launch to first UI IPC | 811.9 ms |
| Immediate warm process restart to first UI IPC | 763.9 ms |
| Installed practice, 120 RAF intervals | mean 8.47 ms, p95 8.50 ms, maximum 16.70 ms |
| Installed exercise audio callback | 197,856 frames; 55,679 nonzero; no audio error or dropped queue events |
| Installed exercise sampled app + WebView2 peak working set | 449 MiB, seven processes |
| Installed exercise sampled aggregate CPU | 7.906 CPU-seconds at 4.669 s process age; includes startup and UI testing |
| Final portable visible idle Library, 10.01 s | 2.594 aggregate CPU-seconds; 25.91% of one core, 1.30% of 20 logical processors; 389 MiB across seven processes |
| Earlier idle Library before UI throttling | 2.18% of total CPU; reduced in final build |
| Generated 1,000-entry real managed import | 11.4 s total; per-file p50 10.203 ms / p95 12.755 ms |
| Warm 1,000-entry query returning 100 rows | 1.221 ms |

The first-process number includes a fresh application process/profile, not a flushed Windows disk cache; a true reboot-cold launch was not measured. Working sets are summed and can double-count shared pages. CPU samples include only the app and its descendant processes. Native audio remains open while idle, so idle cost is measurable; it is not a zero-resource claim. Frame cadence is not GPU presentation or physical key-to-sound latency. Callback activity proves sample generation, not audible speaker output. The benchmark used generated original notes and was a bounded, opt-in workload, run once.

## Package and retention

The NSIS 3.11 current-user installer built with solid zlib. Silent installation to a workspace test folder exited 0, and the installed app passed the private-MIDI workflow. All **562** files under `licenses`, plus top-level LICENSE and THIRD_PARTY_NOTICES.md, matched the source bytes. Silent install did not add optional Open-with registration or change MIDI defaults. Silent uninstall exited 0, removed the app and retained the existing normal library/preferences byte-for-byte. The interactive optional association prompt was source-reviewed; its Yes path was not manually exercised. Interactive app-data deletion was intentionally not tested on the owner's library.

Tauri temporarily patches a three-byte bundle marker from `UNK` to `NSS` when packaging. The installed executable was byte-identical to the portable build after accounting for that exact, single marker; this explains their different hashes. No other binary differences were ignored. Final documentation-only repacking leaves the tested application executable unchanged. ZIP contents and final checksums accompany delivery. Builds are unsigned; WebView2 Evergreen must already be installed. Linux and other Windows machines were not tested.

## Remaining human checks

Physical Yamaha keys, velocity, pedal, audible monitoring, cable unplug/replug, actual keyboard range, long playing sessions, true end-to-end latency and the owner's 4K scaling remain unverified. Windowed/fullscreen native captures do not prove 4K behavior. See [hardware checklist](hardware-checklist.md). The source and package are ready for local use; this is not a production-readiness or unrestricted-public-redistribution claim. See [dependency decisions](dependencies.md) for the copyright placeholder, installer binary provenance and Microsoft CRT rights questions.
