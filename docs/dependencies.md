# Dependency and redistribution inventory

Audit target: Windows x86-64 MSVC. This records the versions in the committed lockfiles and the files actually present in the selected dependency packages. It is a technical inventory and explicit distribution decision, not a legal guarantee or a claim that every future release has been reviewed.

## Exact selection and reproduction

Run `node scripts/audit-licenses.mjs` after `npm ci` and `cargo fetch --locked`. It performs no compilation, installations or global configuration changes. `CARGO` can name an alternate Cargo executable. The Windows Rust selection comes from `cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc --features custom-protocol`, including the release protocol feature. The script reads `package-lock.json`, validates installed package versions, preserves license/NOTICE files and MPL source archives, verifies those archives against Cargo.lock checksums, records hashes and fails when a selected package lacks a reviewed license expression or preserved text. It does not independently grant license approval.

The first audit produced **264 Windows Rust dependencies**, **81 locked npm dependencies** (**27 installed**), and **508 total Cargo.lock packages**, including Keyloom and excluded target packages. Rust role classification is conservative graph reachability: 56 runtime, 139 runtime/build, and 69 build-only packages. This is not a symbol-level dead-code audit. The full exact tables are [Rust](../licenses/inventory/rust.md), [npm](../licenses/inventory/npm.md), and [all Cargo lock entries](../licenses/cargo-lock-inventory.json). [inventory.json](../licenses/inventory.json) records features, source links, file checksums and lockfile checksums. Packages outside the Windows graph are explicitly marked unreviewed for other targets; no Linux/macOS release is approved by this audit.

Ten selected packages omitted standalone license files. Their notices were recovered from exact upstream source commits, or from the matching-version parent npm package for a native build-tool binding. `node scripts/audit-licenses.mjs --fetch-supplements` refreshes only those pinned public license texts and the exact Microsoft SDK notice. [Supplement provenance](../licenses/supplement-provenance.json) records URLs and hashes. The ordinary audit uses the checked-in copies without network requests. The selectors MPL text is copied from the same standard MPL 2.0 text shipped by cssparser; selectors' original headers remain in its unchanged source archive.

## Direct dependencies and enabled behavior

| Dependency                      | Exact version     | Role / relevant features                                                       | Terms and redistribution action                                |
| ------------------------------- | ----------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| tauri                           | 2.12.3            | Runtime Windows shell, WebView2; default features and release custom-protocol  | MIT/Apache alternatives; copy all supplied notices             |
| tauri-build                     | 2.7.2             | Build only, config-json                                                        | MIT/Apache alternatives; notices retained                      |
| tauri-plugin-single-instance    | 2.5.3             | Runtime, file-open delivery                                                    | MIT/Apache alternatives; notices retained                      |
| serde / serde_json              | 1.0.229 / 1.0.154 | Runtime canonical model/settings and IPC serialization                         | MIT/Apache alternatives; notices retained                      |
| midir                           | 0.11.1            | Native Windows MIDI, default WinMM backend                                     | MIT; preserve copyright/permission text                        |
| midly                           | 0.5.3             | Canonical parser; default features off, std + strict; no Rayon parsing workers | Unlicense; exact upstream text preserved                       |
| rusqlite / libsqlite3-sys       | 0.40.2 / 0.38.2   | Runtime managed library; bundled SQLite                                        | MIT wrappers plus public-domain SQLite; both notices preserved |
| sha2                            | 0.11.0            | Runtime exact-byte content IDs                                                 | MIT OR Apache-2.0; notices retained                            |
| cpal                            | 0.18.2            | Native Windows WASAPI; default features                                        | Apache-2.0; license and applicable notices retained            |
| crossbeam-queue                 | 0.3.14            | Bounded audio/input communication                                              | MIT OR Apache-2.0; notices retained                            |
| rfd                             | 0.17.2            | Native import/export dialogs                                                   | MIT; notices retained                                          |
| @tauri-apps/api                 | 2.12.3            | Runtime frontend IPC/window API                                                | Apache-2.0 OR MIT; notices retained                            |
| react / react-dom               | 19.3.0 / 19.3.0   | Runtime frontend; scheduler 0.28.0 transitively                                | MIT; notices retained                                          |
| @tauri-apps/cli                 | 2.12.2            | Build/package tool only                                                        | Apache-2.0 OR MIT; matching Windows binding notice retained    |
| typescript                      | 7.0.2             | Build/type check only                                                          | Apache-2.0; notice retained                                    |
| vite / rolldown                 | 8.3.4 / 1.2.13    | Frontend build only                                                            | MIT and bundled dependency notices retained                    |
| prettier                        | 3.8.1             | Build/format check only                                                        | MIT and bundled notices retained                               |
| @types/react / @types/react-dom | 19.3.0 / 19.3.0   | Build only; exact lock selections                                              | MIT; notices retained                                          |

Enabled feature names that mention another platform do not imply its native libraries are shipped. Target-conditioned edges are filtered by Cargo for Windows. All selected features are recorded in the machine inventory. The app does not select RustySynth, SoundFonts, Python, CUDA, codecs, transcription/OMR engines, YouTube tooling, downloaded corpora, or conversion services.

## Explicit decisions for licenses beyond the preferred simple permissive set

The existing Tauri dependency graph introduces MPL-2.0 components: cssparser 0.37.0, dtoa-short 0.3.5, option-ext 0.2.0, selectors 0.38.0, and build-only cssparser-macros 0.7.1. They remain unchanged. Replacing the framework to avoid these small transitive components would be disproportionate and is unnecessary for an MIT-authored application: the MPL applies to covered files and their source distribution, while new project files can remain separately licensed. This follows the [Mozilla MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) and the full license preserved with the distribution.

The concrete distribution approach is to ship exact unchanged source archives in `licenses/sources`, the MPL text, each source's original notices, and the source-availability notice in `THIRD_PARTY_NOTICES.md`. Their archive checksums are in the inventory. If any MPL source is modified later, its modified covered source and notices must be distributed under the MPL; the project MIT license must not replace them. Lightning CSS 1.33.0 is a build tool under MPL and is not part of the runtime frontend. Shipping the development tooling would require revisiting its binary/source obligations.

Unicode-3.0 and combined Apache/MIT/Unicode notices are retained exactly. `AND` expressions are not treated as elective alternatives. No GPL/AGPL, noncommercial, research-only or unknown-license component appears in the selected Cargo/npm package-level Windows runtime inventory. This statement excludes separately installed Microsoft system/runtime components and unselected-platform lock entries. The Rust standard-library notice collection includes other-target/toolchain license texts conservatively; it is not evidence that those components are in Keyloom.

## Native code and installer composition

SQLite **3.53.2** is the amalgamation inside libsqlite3-sys 0.38.2 and is compiled through its `bundled` feature. SQLite states its deliverable engine is [public domain](https://www.sqlite.org/copyright.html). The MIT wrapper and SQLite's header are both preserved. SQLCipher is not enabled.

WebView2 Rust wrapper **0.39.1** statically links `WebView2LoaderStatic.lib` on MSVC. The wrapper's pinned source commit `edc2caf886175ccaebe86078c9cfe1ae2a187328` identifies [Microsoft.Web.WebView2 SDK 1.0.3800.47](https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/crates/update-bindings/src/main.rs). Its [official exact-version NuGet license](https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47/License) is BSD-3-Clause, separate from the wrapper's MIT license; the full Microsoft notice is in `licenses/native`. The Evergreen browser runtime is required but **not bundled**. No Microsoft runtime installer or updater is shipped. Microsoft documents the [installed-runtime distribution approach](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

The Rust standard library used here is **rustc 1.98.0**, commit `88d9e12ae178fab0fb5cc050a94da85685d449ea`, with LLVM 22.1.8. Its detailed `COPYRIGHT-library.html` and standard license collection accompany the app; the compiler itself is not shipped. Windows system libraries and the installed WebView2 Runtime remain OS/runtime prerequisites. The Windows build enables `+crt-static`, so Microsoft CRT code is statically linked; Cargo's license inventory does not cover that native code. The decision to ship it is recorded under [Release decisions](#release-decisions-for-041). Microsoft's [Visual C++ redistribution guidance](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files?view=msvc-170) distinguishes those rights from ordinary open-source dependency notices. No loose Visual C++ runtime DLL or redistributable installer is bundled.

The downloaded installer tool reports **NSIS v3.11**. The actual generated `installer.nsi` selects **solid zlib**, matching configuration. The [NSIS license](https://nsis.sourceforge.io/License) covers the core, standard plugins, UI images and zlib module under zlib/libpng terms; the full upstream and cached `COPYING` texts are in `licenses/installer/NSIS-3.11`. Those texts also describe bzip2 and CPL-1.0 LZMA modules, but neither compression module is selected. The cache contains unused compression stubs; their presence is not evidence of inclusion. The inventory records SHA-256 of the inspected zlib stub, modern UI and conservative standard-plugin inputs. A portable application does not contain the NSIS installer stub/plugins.

Tauri CLI **2.12.2** pins [NSIS 3.11](https://github.com/tauri-apps/binary-releases/releases/tag/nsis-3.11) and [nsis_tauri_utils 0.5.3](https://github.com/tauri-apps/nsis-tauri-utils/releases/tag/nsis_tauri_utils-v0.5.3). The cached helper DLL's SHA-1 is `75197fee3c6a814fe035788d1c34ead39349b860`, matching the bundler's expected value; its SHA-256 is in the inventory. Full plugin MIT/Apache texts, its exact source-tag manifests and Tauri template notices are preserved. Its source tag declares internal nsis-plugin-api/nsis-fn, windows-sys and semver dependencies, but does not supply a Cargo.lock or binary SBOM verified by this audit. **The exact transitive/compiler versions inside that upstream prebuilt DLL remain unverified**; it is accepted under [Release decisions](#release-decisions-for-041). This limitation is separate from the complete application Cargo graph. Native AppUserModelID operations use built-in COM macros through `System.dll`; no separate AppId.dll appears in the generated inputs. The WebView2 download branch is disabled by the actual empty installer-mode definition.

An optional upstream `UnpinShortcut` macro adapts [Anders' Stack Overflow answer 42816728](https://stackoverflow.com/a/42816728), whose [Stack Exchange API record](https://api.stackexchange.com/2.3/answers/42816728?site=stackoverflow&filter=withbody) identifies CC BY-SA 3.0. Keyloom excludes this macro's body by replacing it with an empty one-argument macro in its installer hook. The generated template includes `utils.nsh`, then the hook, then all three unpin expansions; the audit checks this ordering and the empty override. Ordinary shortcut deletion remains; users may need to unpin a retained Start/taskbar pin manually. The upstream helper source is not copied into release resources. The unused FileAssociation helper macros are not expanded; Keyloom's optional Open-with hook is project-authored.

`node scripts/audit-licenses.mjs --fetch-installer-notices` refreshes pinned installer notices. The ordinary audit inspects the cache and generated installer when present without compiling. `KEYLOOM_NSIS_DIR` can override the cache location. This is static input inspection, not extracted-binary verification: NSIS `/PPO` reported unresolved plugin registration during preprocessing, although the ordinary installer build succeeded. Final packaging and installed-resource comparison are separate checks and must be reported by the release workflow.

The 0.1.0 local release verification installed the rebuilt package, compared all 562 license/source files and both top-level notice documents byte-for-byte, ran its native import/play/restart test, then uninstalled it while verifying existing library/preferences retention. Those checks passed; see the release-specific [test report](test-report.md) for later package verification and its limits. Package checks do not establish the upstream helper DLL's exact binary provenance. The maintainer's distribution decisions for that DLL and the Microsoft runtime are recorded below.

## Release decisions for 0.4.1

**Status: approved by the maintainer on 28 October 2026.** These decisions continue to apply to the 0.4.2 beta while the toolchain, dependency selections and installer components described here remain unchanged. This records the technical reasoning; it is not legal advice.

### Statically linked Microsoft C/C++ runtime

**Decision:** keep `+crt-static` and ship the runtime statically linked into `Keyloom.exe`.

- **Why:** one self-contained executable, with no Visual C++ Redistributable for users to install. Static linking (`/MT`) is a documented MSVC deployment model, and Rust's `x86_64-pc-windows-msvc` target supports it through `crt-static`.
- **Rights:** the runtime libraries come with the MSVC toolchain. Since [August 2022](https://devblogs.microsoft.com/cppblog/updates-to-visual-studio-build-tools-license-for-c-and-cpp-open-source-projects/) the Build Tools license allows compiling OSI-licensed open-source projects, and an individual developer may use Visual Studio Community, whose terms cover Microsoft's distributable runtime code, for their own free or paid apps. Keyloom is MIT-licensed and maintained by one individual.
- **Alternatives considered:** linking the runtime dynamically makes users install the Redistributable themselves (Windows 11 doesn't guarantee it), and bundling the runtime DLLs raises the same redistribution question. A portable-only release doesn't help, because it contains the same executable.
- **Revisit if** the project gains an organizational owner (Community's terms differ for organizations) or Microsoft changes these terms.

### Tauri's prebuilt NSIS helper DLL

**Decision:** accept `nsis_tauri_utils.dll` 0.5.3 as published by Tauri.

- **What is known:** its source is MIT OR Apache-2.0 at the public `nsis_tauri_utils-v0.5.3` tag. The 34,304-byte DLL was published on 26 January 2026 by that repository's release workflow on GitHub-hosted Windows runners. Tauri CLI 2.12.2 pins it and checks its SHA-1 when downloading. Both license texts and the tag's manifests are preserved in `licenses/installer`.
- **What is not known:** the tag has no `Cargo.lock`, and the release has no SBOM or build attestation, so the exact versions of its Rust dependencies (windows-sys, semver and its internal NSIS plugin crates, all MIT/Apache upstream) and of its compiler can't be verified.
- **Scope:** it runs only inside the installer. It is not part of `Keyloom.exe` or the portable ZIP.
- **Alternatives considered:** a portable-only release loses the Start menu shortcut, uninstall entry and optional Open with registration; building the DLL ourselves would mean maintaining a fork of Tauri's installer tooling.
- **Revisit if** Tauri publishes a lockfile or attestation for the helper, or the Tauri CLI version changes.

## Public-release items

1. Resolved 27 October 2026: `LICENSE` names Tom Pearsall as copyright holder.
2. Include `LICENSE`, `THIRD_PARTY_NOTICES.md`, this decision record, and the **entire `licenses` directory**, including MPL sources, in both installer resources and portable distributions. Verify the packaged copies, not only the source tree.
3. The static Microsoft runtime and the upstream prebuilt installer DLL are covered by the release decisions above. Don't present the DLL's source manifest as a locked binary inventory.
4. Rerun the audit after lockfile/toolchain/installer changes. New unknown or changed licenses require review; an empty missing-text list alone is not a legal sign-off. Preserve the unpin omission or review the CC BY-SA material before reintroducing it.

Private songs, user database files, source paths, logs, samples and hardware captures are excluded from release assets. Generated original exercises and the original icon use the project license. Local system fonts are referenced, not copied.
