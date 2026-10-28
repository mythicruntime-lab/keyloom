# Keyloom third-party notices

Keyloom includes software created by other projects. Its project MIT license does not replace their licenses, copyright notices, disclaimers, or source-availability rights. Keep this file and the entire accompanying `licenses` directory with the installed app and every portable distribution.

The exact Windows dependency inventory is [Rust](licenses/inventory/rust.md) and [npm](licenses/inventory/npm.md). Machine-readable versions, features, roles and license-file SHA-256 values are in [inventory.json](licenses/inventory.json). The [complete Cargo lock inventory](licenses/cargo-lock-inventory.json) distinguishes the Windows selection from other target packages. Full copied license/NOTICE texts are under `licenses/rust`, `licenses/npm`, `licenses/native`, and `licenses/rust-standard-library`. Build-only tools are included conservatively in the inventory; their executables and `node_modules` are not part of the app distribution.

## Main components

| Component                                             | Selected version | Terms                                                 |
| ----------------------------------------------------- | ---------------- | ----------------------------------------------------- |
| Tauri                                                 | 2.12.3           | MIT OR Apache-2.0                                     |
| Tauri single-instance plugin                          | 2.5.3            | MIT OR Apache-2.0                                     |
| React / React DOM                                     | 19.3.0           | MIT                                                   |
| Tauri JavaScript API                                  | 2.12.3           | MIT OR Apache-2.0                                     |
| midir                                                 | 0.11.1           | MIT                                                   |
| midly                                                 | 0.5.3            | Unlicense                                             |
| CPAL                                                  | 0.18.2           | Apache-2.0                                            |
| rusqlite / libsqlite3-sys                             | 0.40.2 / 0.38.2  | MIT                                                   |
| Bundled SQLite                                        | 3.53.2           | Public domain                                         |
| Microsoft WebView2 SDK loader                         | 1.0.3800.47      | BSD-3-Clause                                          |
| Rust standard library                                 | 1.98.0           | Apache-2.0 OR MIT, with preserved third-party notices |
| NSIS installer core, standard plugins and zlib module | 3.11             | zlib/libpng; installer only                           |
| Tauri NSIS utilities plugin                           | 0.5.3            | MIT OR Apache-2.0; installer only                     |

See the full inventory for transitive dependencies, including Unicode data notices and combined license expressions. Where a package offers MIT as an alternative, the MIT option is used; combined `AND` requirements and separately licensed portions remain applicable. All license texts supplied by the selected package are preserved, including alternatives. This selection does not relicense those packages.

## Mozilla Public License components and source availability

These unchanged dependencies are covered by the Mozilla Public License 2.0. Their source remains available under the MPL, independently of Keyloom's project license. The exact source archives are included with the application; `.crate` files are gzip-compressed tar archives containing the original source and notices. These copies avoid requiring network access to obtain the corresponding source.

| Component              | Role                                | Included unchanged source                                                     | Upstream exact source                                                                             |
| ---------------------- | ----------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| cssparser 0.37.0       | Runtime/build dependency            | [cssparser-0.37.0.crate](licenses/sources/cssparser-0.37.0.crate)             | [crates.io source](https://static.crates.io/crates/cssparser/cssparser-0.37.0.crate)              |
| dtoa-short 0.3.5       | Runtime/build dependency            | [dtoa-short-0.3.5.crate](licenses/sources/dtoa-short-0.3.5.crate)             | [crates.io source](https://static.crates.io/crates/dtoa-short/dtoa-short-0.3.5.crate)             |
| option-ext 0.2.0       | Runtime/build dependency            | [option-ext-0.2.0.crate](licenses/sources/option-ext-0.2.0.crate)             | [crates.io source](https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate)             |
| selectors 0.38.0       | Runtime/build dependency            | [selectors-0.38.0.crate](licenses/sources/selectors-0.38.0.crate)             | [crates.io source](https://static.crates.io/crates/selectors/selectors-0.38.0.crate)              |
| cssparser-macros 0.7.1 | Build only; retained conservatively | [cssparser-macros-0.7.1.crate](licenses/sources/cssparser-macros-0.7.1.crate) | [crates.io source](https://static.crates.io/crates/cssparser-macros/cssparser-macros-0.7.1.crate) |

The [full MPL 2.0 text](licenses/rust/cssparser-0.37.0/LICENSE) applies to the covered source. No changes were made to these dependencies. The selectors crate omits a standalone license file; its source headers identify MPL 2.0, and the matching complete standard license is supplied in [LICENSE-MPL-2.0.txt](licenses/rust/selectors-0.38.0/LICENSE-MPL-2.0.txt).

The build tool Lightning CSS 1.33.0 and its installed Windows binding are also MPL-2.0; their preserved notices are in the npm inventory. They are not embedded in Keyloom's frontend or shipped as tools. Redistributing the development toolchain itself requires a separate source/notice audit for those tool binaries.

## Native components and assets

The WebView2 Rust wrappers have MIT terms; the Microsoft native loader has a separate [BSD license and Microsoft copyright notice](licenses/native/WebView2-SDK-1.0.3800.47-LICENSE.txt). Keyloom does not bundle the Evergreen browser runtime, its updater or a runtime installer. The installed Microsoft WebView2 Runtime has its own terms and third-party notices, supplied by Microsoft.

SQLite's bundled engine is dedicated to the public domain. Its source header is preserved in [SQLite-PUBLIC-DOMAIN.txt](licenses/native/SQLite-PUBLIC-DOMAIN.txt). The SQLCipher notices retained within the source-package collection do not mean SQLCipher is enabled or shipped.

Rust standard-library copyright details and embedded dependency notices are preserved in [COPYRIGHT-library.html](licenses/rust-standard-library/COPYRIGHT-library.html). The adjacent standard license collection also includes texts for other target/toolchain components; inclusion of those texts does not imply every such component is linked into this Windows app.

Keyloom's generated MIDI exercises, icon and interface artwork are original project assets. No downloaded music arrangements, commercial samples, SoundFont banks, model weights, artwork, or fonts are bundled. The app uses local Windows/system fonts and a clearly labeled synthetic practice tone. Music you import stays private in your local managed library and is never a release asset.

The installer uses NSIS 3.11 with solid zlib compression. Its core, standard plugins, UI assets and compression notices are preserved in [NSIS COPYING](licenses/installer/NSIS-3.11/COPYING-CACHE.txt). The full text also covers unused bzip2/LZMA components; neither is selected. The Tauri nsis_tauri_utils 0.5.3 plugin's [MIT](licenses/installer/nsis-tauri-utils-0.5.3/LICENSE_MIT) and [Apache](licenses/installer/nsis-tauri-utils-0.5.3/LICENSE_APACHE-2.0) notices accompany it. The maintainer accepted this upstream prebuilt installer DLL for distribution; its exact transitive/compiler provenance remains unverified. See the [recorded release decisions](docs/dependencies.md#release-decisions-for-041). This limitation does not change its preserved declared licenses.

The optional upstream taskbar-unpin macro is excluded from Keyloom's installer; its Stack Overflow-derived source is not redistributed in these resources. Keyloom's hook leaves manual unpinning to the user. Windows AppUserModelID helpers use standard COM/System functionality, without an extra AppId plugin. Microsoft CRT code is statically linked under the Microsoft toolchain's separate terms. The maintainer's decision to distribute it, and the conditions for revisiting that decision, are recorded in the [dependency release decisions](docs/dependencies.md#release-decisions-for-041).
