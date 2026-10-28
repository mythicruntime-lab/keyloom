// Read-only dependency inspection; writes only reproducible license artifacts.
// Run after npm ci / cargo fetch --locked. No builds, installs or registry edits.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, "licenses");
const cargo =
  process.env.CARGO ??
  path.join(process.env.USERPROFILE ?? "", ".cargo", "bin", "cargo.exe");
const rustc = path.join(path.dirname(cargo), "rustc.exe");
const target = "x86_64-pc-windows-msvc";
const read = (file) => fs.readFileSync(file, "utf8");
const hash = (value) => createHash("sha256").update(value).digest("hex");
const relative = (file) => path.relative(root, file).split(path.sep).join("/");
const safeName = (name) => name.replace(/[^a-zA-Z0-9._-]/g, "_");
const write = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, value);
};
const json = (file, value) =>
  write(file, JSON.stringify(value, null, 2) + "\n");
const copy = (source, destination) => {
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
  return {
    file: relative(destination),
    sha256: hash(fs.readFileSync(destination)),
  };
};
const command = (exe, args) =>
  execFileSync(exe, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
const supplementalUrls = [
  [
    "alloc-stdlib-0.3.0",
    "LICENSE",
    "https://raw.githubusercontent.com/dropbox/rust-alloc-no-stdlib/0a81fd6928ea3b33c8cd484aa4575d50ffb98012/LICENSE",
  ],
  [
    "dasp_sample-0.11.0",
    "LICENSE-MIT",
    "https://raw.githubusercontent.com/RustAudio/dasp/97c3bb9b2363c0b46ac1633858bf1054fd02a980/LICENSE-MIT",
  ],
  [
    "dasp_sample-0.11.0",
    "LICENSE-APACHE",
    "https://raw.githubusercontent.com/RustAudio/dasp/97c3bb9b2363c0b46ac1633858bf1054fd02a980/LICENSE-APACHE",
  ],
  [
    "defmt-parser-1.0.0",
    "LICENSE-MIT",
    "https://raw.githubusercontent.com/knurling-rs/defmt/4a8cdb44891ed57b8ff5a023b6bec7137c48708f/LICENSE-MIT",
  ],
  [
    "midly-0.5.3",
    "LICENSE",
    "https://raw.githubusercontent.com/negamartin/midly/816cb2a585b4d5d1fd7aeb0cb35668f21dc00869/LICENSE",
  ],
  [
    "webview2-com-macros-0.8.1",
    "LICENSE",
    "https://raw.githubusercontent.com/wravery/webview2-rs/dffa41a8a46d3f5565eefbff2de57d38d399f158/LICENSE",
  ],
  [
    "webview2-com-sys-0.39.1",
    "LICENSE",
    "https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE",
  ],
  [
    "webview2-com-0.39.1",
    "LICENSE",
    "https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE",
  ],
];
const installerUrls = [
  [
    "NSIS-3.11",
    "COPYING",
    "https://raw.githubusercontent.com/kichik/nsis/v311/COPYING",
  ],
  [
    "nsis-tauri-utils-0.5.3",
    "LICENSE_MIT",
    "https://raw.githubusercontent.com/tauri-apps/nsis-tauri-utils/nsis_tauri_utils-v0.5.3/LICENSE_MIT",
  ],
  [
    "nsis-tauri-utils-0.5.3",
    "LICENSE_APACHE-2.0",
    "https://raw.githubusercontent.com/tauri-apps/nsis-tauri-utils/nsis_tauri_utils-v0.5.3/LICENSE_APACHE-2.0",
  ],
  [
    "nsis-tauri-utils-0.5.3",
    "Cargo.toml",
    "https://raw.githubusercontent.com/tauri-apps/nsis-tauri-utils/nsis_tauri_utils-v0.5.3/Cargo.toml",
  ],
  [
    "nsis-tauri-utils-0.5.3",
    "plugin-Cargo.toml",
    "https://raw.githubusercontent.com/tauri-apps/nsis-tauri-utils/nsis_tauri_utils-v0.5.3/crates/nsis-tauri-utils/Cargo.toml",
  ],
];
if (process.argv.includes("--fetch-installer-notices")) {
  const provenance = [];
  for (const [component, filename, url] of installerUrls) {
    const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
    if (!response.ok)
      throw new Error(
        `Installer notice request failed (${response.status}): ${url}`,
      );
    const text = await response.text();
    if (text.length > 1_000_000 || text.length < 100)
      throw new Error(`Unexpected installer notice size: ${url}`);
    const destination = path.join(output, "installer", component, filename);
    write(destination, text);
    provenance.push({
      component,
      file: relative(destination),
      source: url,
      sha256: hash(text),
    });
  }
  for (const filename of ["LICENSE-MIT", "LICENSE-APACHE-2.0"]) {
    const preserved = copy(
      path.join(root, "node_modules", "@tauri-apps", "cli", filename),
      path.join(output, "installer", "tauri-bundler-2.12.2", filename),
    );
    provenance.push({
      component: "tauri-bundler-2.12.2",
      ...preserved,
      source: "https://www.npmjs.com/package/@tauri-apps/cli/v/2.12.2",
      origin: "exact installed CLI package notice",
    });
  }
  json(path.join(output, "installer", "upstream-provenance.json"), provenance);
}
if (process.argv.includes("--fetch-supplements")) {
  const provenance = [];
  for (const [name, filename, url] of supplementalUrls) {
    const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
    if (!response.ok)
      throw new Error(`License source failed (${response.status}): ${url}`);
    const text = await response.text();
    if (text.length > 1_000_000 || text.length < 100)
      throw new Error(`Unexpected license size: ${url}`);
    const destination = path.join(output, "supplements", name, filename);
    write(destination, text);
    provenance.push({
      package: name,
      file: relative(destination),
      source: url,
      sha256: hash(text),
    });
  }
  const sdkUrl =
    "https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47/License";
  const sdkResponse = await fetch(sdkUrl, {
    signal: AbortSignal.timeout(30_000),
  });
  if (!sdkResponse.ok)
    throw new Error(`SDK license request failed: ${sdkResponse.status}`);
  const html = await sdkResponse.text();
  const text = html
    .match(/<pre[^>]*>([\s\S]*?)<\/pre>/i)?.[1]
    .replaceAll("&quot;", '"')
    .replaceAll("&#39;", "'")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
  if (
    !text?.includes("Microsoft Corporation") ||
    !text.includes("Redistribution and use")
  )
    throw new Error("SDK license extraction needs review");
  const sdkDestination = path.join(
    output,
    "native",
    "WebView2-SDK-1.0.3800.47-LICENSE.txt",
  );
  write(sdkDestination, text.trim() + "\n");
  provenance.push({
    package: "Microsoft.Web.WebView2 1.0.3800.47",
    file: relative(sdkDestination),
    source: sdkUrl,
    sha256: hash(fs.readFileSync(sdkDestination)),
  });
  json(path.join(output, "supplement-provenance.json"), provenance);
}
const metadata = JSON.parse(
  command(cargo, [
    "metadata",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--locked",
    "--format-version",
    "1",
    "--filter-platform",
    target,
    "--features",
    "custom-protocol",
  ]),
);
const lock = JSON.parse(read(path.join(root, "package-lock.json")));
const packages = new Map(metadata.packages.map((value) => [value.id, value]));
const nodes = new Map(metadata.resolve.nodes.map((value) => [value.id, value]));
const roleMap = new Map();
function visit(id, role) {
  const roles = roleMap.get(id) ?? new Set();
  if (roles.has(role)) return;
  roles.add(role);
  roleMap.set(id, roles);
  for (const dependency of nodes.get(id)?.deps ?? []) {
    for (const kind of dependency.dep_kinds) {
      if (kind.kind === "dev") continue;
      const procMacro = packages
        .get(dependency.pkg)
        .targets.some((entry) => entry.kind.includes("proc-macro"));
      visit(
        dependency.pkg,
        role === "build" || kind.kind === "build" || procMacro
          ? "build"
          : "runtime",
      );
    }
  }
}
visit(metadata.resolve.root, "runtime");
function roleFor(id) {
  const roles = roleMap.get(id) ?? new Set();
  return roles.has("runtime")
    ? roles.has("build")
      ? "runtime and build"
      : "runtime"
    : roles.has("build")
      ? "build only"
      : "unselected";
}

const licenseName =
  /^(licen[cs]e|copying|copyright|notice|unlicense|third[-_]?party|thirdpartynotices)([._-]|$)/i;
function licenseFiles(directory) {
  const files = [];
  function walk(folder, inLicenseFolder = false) {
    for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
      if (
        entry.isSymbolicLink() ||
        ["node_modules", ".git", "target"].includes(entry.name)
      )
        continue;
      const full = path.join(folder, entry.name);
      if (entry.isDirectory())
        walk(
          full,
          inLicenseFolder || /^(licenses?|notices?)$/i.test(entry.name),
        );
      else if (
        entry.isFile() &&
        (inLicenseFolder || licenseName.test(entry.name))
      )
        files.push(full);
    }
  }
  walk(directory);
  return files.sort();
}
function preserve(directory, destination) {
  return licenseFiles(directory).map((source) =>
    copy(source, path.join(destination, path.relative(directory, source))),
  );
}
const issues = [];
const reviewedIds = new Set([
  "0BSD",
  "MIT",
  "MIT-0",
  "Apache-2.0",
  "Unlicense",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Zlib",
  "CC0-1.0",
  "Unicode-3.0",
  "MPL-2.0",
  "OR",
  "AND",
]);
function checkLicense(name, expression) {
  if (
    !expression ||
    [...expression.matchAll(/[A-Za-z0-9][A-Za-z0-9.+-]*/g)].some(
      ([id]) => !reviewedIds.has(id),
    )
  ) {
    issues.push(
      `${name}: unreviewed or unknown license expression (${expression ?? "missing"})`,
    );
  }
}
const rust = [];
const mplPackage = metadata.packages.find(
  (entry) => entry.name === "cssparser",
);
copy(
  path.join(path.dirname(mplPackage.manifest_path), "LICENSE"),
  path.join(output, "supplements", "selectors-0.38.0", "LICENSE-MPL-2.0.txt"),
);
for (const [parent, binding] of [
  ["rolldown", "@rolldown/binding-win32-x64-msvc"],
  ["@tauri-apps/cli", "@tauri-apps/cli-win32-x64-msvc"],
]) {
  const directory = path.join(root, "node_modules", parent);
  const bindingDetails = JSON.parse(
    read(path.join(root, "node_modules", binding, "package.json")),
  );
  preserve(
    directory,
    path.join(
      output,
      "supplements",
      `${safeName(binding)}-${bindingDetails.version}`,
    ),
  );
}
for (const entry of metadata.packages
  .filter((entry) => entry.id !== metadata.resolve.root)
  .sort((a, b) =>
    `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`),
  )) {
  const directory = path.dirname(entry.manifest_path);
  const destination = path.join(
    output,
    "rust",
    `${entry.name}-${entry.version}`,
  );
  const texts = preserve(directory, destination);
  if (entry.license_file) {
    const explicit = path.resolve(directory, entry.license_file);
    if (!texts.some((text) => text.sha256 === hash(fs.readFileSync(explicit))))
      texts.push(
        copy(explicit, path.join(destination, path.basename(explicit))),
      );
  }
  // Supplements have separately reviewed, immutable upstream provenance.
  const supplemental = path.join(
    output,
    "supplements",
    `${entry.name}-${entry.version}`,
  );
  if (fs.existsSync(supplemental)) {
    for (const source of licenseFiles(supplemental))
      texts.push(copy(source, path.join(destination, path.basename(source))));
  }
  let sourceArchive;
  if (entry.license?.includes("MPL-2.0")) {
    const registry = path.dirname(directory);
    const archive = path.join(
      path.dirname(path.dirname(registry)),
      "cache",
      path.basename(registry),
      `${entry.name}-${entry.version}.crate`,
    );
    if (fs.existsSync(archive))
      sourceArchive = copy(
        archive,
        path.join(output, "sources", `${entry.name}-${entry.version}.crate`),
      );
    else
      issues.push(
        `${entry.name} ${entry.version}: MPL source archive unavailable`,
      );
  }
  if (!entry.license)
    issues.push(`${entry.name} ${entry.version}: missing declared license`);
  checkLicense(`${entry.name} ${entry.version}`, entry.license);
  if (!texts.length)
    issues.push(`${entry.name} ${entry.version}: no preserved license text`);
  rust.push({
    name: entry.name,
    version: entry.version,
    license: entry.license ?? "UNKNOWN",
    role: roleFor(entry.id),
    source: `https://crates.io/crates/${entry.name}/${entry.version}`,
    source_archive_url: `https://static.crates.io/crates/${entry.name}/${entry.name}-${entry.version}.crate`,
    repository: entry.repository,
    features: nodes.get(entry.id)?.features ?? [],
    texts,
    ...(sourceArchive ? { source_archive: sourceArchive } : {}),
  });
}

const npm = [];
for (const [location, entry] of Object.entries(lock.packages)
  .filter(([location]) => location)
  .sort(([a], [b]) => a.localeCompare(b))) {
  const directory = path.join(root, location);
  const installed = fs.existsSync(path.join(directory, "package.json"));
  const details = installed
    ? JSON.parse(read(path.join(directory, "package.json")))
    : null;
  const name = details?.name ?? location.split("node_modules/").at(-1);
  if (installed && details.version !== entry.version)
    issues.push(`${name}: installed version differs from package-lock.json`);
  const destination = path.join(
    output,
    "npm",
    `${safeName(name)}-${entry.version}`,
  );
  const texts = installed ? preserve(directory, destination) : [];
  const supplemental = path.join(
    output,
    "supplements",
    `${safeName(name)}-${entry.version}`,
  );
  if (fs.existsSync(supplemental)) {
    for (const source of licenseFiles(supplemental))
      texts.push(copy(source, path.join(destination, path.basename(source))));
  }
  const license = entry.license ?? details?.license ?? "UNKNOWN";
  if (installed) checkLicense(`${name} ${entry.version}`, license);
  if (installed && !texts.length)
    issues.push(`${name} ${entry.version}: no preserved license text`);
  if (license === "UNKNOWN")
    issues.push(`${name} ${entry.version}: missing declared license`);
  npm.push({
    name,
    version: entry.version,
    license,
    role: !installed
      ? "other platform; not installed or shipped"
      : entry.dev
        ? "build only"
        : "runtime frontend",
    installed,
    source: `https://www.npmjs.com/package/${name}/v/${entry.version}`,
    integrity: entry.integrity,
    texts,
  });
}

const cargoLockText = read(path.join(root, "src-tauri", "Cargo.lock"));
const cargoLock = cargoLockText
  .split("[[package]]")
  .slice(1)
  .map((block) => {
    const field = (key) =>
      block.match(new RegExp(`^${key} = "([^"\\n]+)"`, "m"))?.[1];
    const selected = rust.find(
      (entry) =>
        entry.name === field("name") && entry.version === field("version"),
    );
    return {
      name: field("name"),
      version: field("version"),
      source: field("source") ?? "project",
      checksum: field("checksum"),
      windows_selected: !!selected,
      license:
        selected?.license ??
        (field("name") === "keyloom"
          ? "MIT; copyright holder pending"
          : "not audited for excluded target"),
      role:
        selected?.role ??
        (field("name") === "keyloom" ? "project" : "not in Windows graph"),
    };
  });
for (const entry of rust.filter((entry) => entry.source_archive)) {
  const locked = cargoLock.find(
    (item) => item.name === entry.name && item.version === entry.version,
  );
  if (entry.source_archive.sha256 !== locked?.checksum)
    issues.push(
      `${entry.name}: bundled MPL source archive differs from Cargo.lock checksum`,
    );
}
const provenanceFile = path.join(output, "supplement-provenance.json");
if (fs.existsSync(provenanceFile)) {
  for (const entry of JSON.parse(read(provenanceFile))) {
    if (
      !fs.existsSync(path.join(root, entry.file)) ||
      hash(fs.readFileSync(path.join(root, entry.file))) !== entry.sha256
    )
      issues.push(`License supplement checksum mismatch: ${entry.file}`);
  }
} else
  issues.push(
    "Missing supplement provenance; retrieve the pinned upstream notices first",
  );
if (
  !fs.existsSync(
    path.join(output, "native", "WebView2-SDK-1.0.3800.47-LICENSE.txt"),
  )
)
  issues.push("Missing Microsoft WebView2 SDK notice");

const sysroot = command(rustc, ["--print", "sysroot"]).trim();
const rustVersion = command(rustc, ["-Vv"]).trim();
const rustDocs = path.join(sysroot, "share", "doc", "rust");
const toolchainTexts = [];
for (const name of ["COPYRIGHT-library.html", "README.md"]) {
  const source = path.join(rustDocs, name);
  if (fs.existsSync(source))
    toolchainTexts.push(
      copy(source, path.join(output, "rust-standard-library", name)),
    );
}
if (fs.existsSync(path.join(rustDocs, "licenses"))) {
  for (const source of fs.readdirSync(path.join(rustDocs, "licenses"))) {
    toolchainTexts.push(
      copy(
        path.join(rustDocs, "licenses", source),
        path.join(output, "rust-standard-library", "licenses", source),
      ),
    );
  }
}

const sqlite = metadata.packages.find(
  (entry) => entry.name === "libsqlite3-sys",
);
const sqliteHeader = path.join(
  path.dirname(sqlite.manifest_path),
  "sqlite3",
  "sqlite3.h",
);
const sqliteText = read(sqliteHeader);
const sqliteVersion = sqliteText.match(
  /^#define SQLITE_VERSION\s+"([^"]+)"/m,
)?.[1];
const sqliteNotice = sqliteText.match(/^\/\*[\s\S]*?\*\//)?.[0];
write(
  path.join(output, "native", "SQLite-PUBLIC-DOMAIN.txt"),
  `SQLite ${sqliteVersion}\nSource: libsqlite3-sys ${sqlite.version} / sqlite3/sqlite3.h\nhttps://www.sqlite.org/copyright.html\n\n${sqliteNotice}\n`,
);

const webviewPackage = metadata.packages.find(
  (entry) => entry.name === "webview2-com-sys",
);
const loaderFile = path.join(
  path.dirname(webviewPackage.manifest_path),
  "x64",
  "WebView2LoaderStatic.lib",
);
// Inspect generated inputs only. A successful installer build and installed-file
// verification are separate release checks; this does not claim binary extraction.
const nsisCache =
  process.env.KEYLOOM_NSIS_DIR ??
  path.join(process.env.LOCALAPPDATA ?? "", "tauri", "NSIS");
const generatedInstaller = path.join(
  root,
  "src-tauri",
  "target",
  "release",
  "nsis",
  "x64",
  "installer.nsi",
);
const installerReview = {
  status: "not generated",
  public_release_items: [
    "Verify installed/portable notice copies after the final packaging pass.",
    "nsis_tauri_utils 0.5.3 is an upstream prebuilt DLL: exact transitive/compiler versions are not supplied by its source tag (no published Cargo.lock/SBOM verified).",
    "Confirm Microsoft Build Tools/static Visual C++ runtime redistribution rights under the installed toolchain terms before public distribution.",
  ],
};
if (
  fs.existsSync(generatedInstaller) &&
  fs.existsSync(path.join(nsisCache, "makensis.exe"))
) {
  const script = read(generatedInstaller);
  const hooks = read(path.join(root, "src-tauri", "installer-hooks.nsh"));
  const utilsIndex = script.indexOf('!include "utils.nsh"');
  const hookIndex = script.search(/^!include .*installer-hooks\.nsh"/m);
  const firstUnpin = script.indexOf("!insertmacro UnpinShortcut");
  const emptyOverride =
    /!macroundef UnpinShortcut\s+!macro UnpinShortcut shortcut\s+!macroend/.test(
      hooks,
    );
  const expectedOrder =
    utilsIndex >= 0 && hookIndex > utilsIndex && firstUnpin > hookIndex;
  const compression = script.match(/SetCompressor\s+\/SOLID\s+"([^"]+)"/)?.[1];
  const nsisVersion = command(path.join(nsisCache, "makensis.exe"), [
    "/VERSION",
  ]).trim();
  const pluginRelative = "Plugins/x86-unicode/additional/nsis_tauri_utils.dll";
  const pluginBytes = fs.readFileSync(path.join(nsisCache, pluginRelative));
  const pluginSha1 = createHash("sha1").update(pluginBytes).digest("hex");
  // Tauri writes absolute checkout and NSIS cache paths into the script; hash it
  // with those replaced so the recorded value does not depend on the machine.
  const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const portableScript = script
    .replace(new RegExp(escapeRegExp(root), "gi"), "<repo>")
    .replace(new RegExp(escapeRegExp(nsisCache), "gi"), "<nsis>");
  if (nsisVersion !== "v3.11")
    issues.push(`Installer: unreviewed NSIS version ${nsisVersion}`);
  if (compression !== "zlib")
    issues.push(`Installer: unreviewed compression ${compression}`);
  if (!expectedOrder || !emptyOverride)
    issues.push(
      "Installer: optional CC BY-SA unpin helper exclusion is not verified",
    );
  if (pluginSha1 !== "75197fee3c6a814fe035788d1c34ead39349b860")
    issues.push(
      "Installer: nsis_tauri_utils binary differs from Tauri CLI 2.12.2 pinned release",
    );
  const nativeInputs = [
    "makensis.exe",
    "Stubs/zlib_solid-x86-unicode",
    "Contrib/UIs/modern.exe",
    "Plugins/x86-unicode/System.dll",
    "Plugins/x86-unicode/nsDialogs.dll",
    "Plugins/x86-unicode/StartMenu.dll",
    "Plugins/x86-unicode/LangDLL.dll",
    pluginRelative,
  ];
  Object.assign(installerReview, {
    status:
      "generated inputs inspected; final build and installed copies require separate verification",
    nsis_version: nsisVersion,
    nsis_archive: {
      url: "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip",
      expected_sha1: "ef7ff767e5cbd9edd22add3a32c9b8f4500bb10d",
      verification:
        "Tauri verifies the download; cache file SHA-256 values below identify inspected inputs",
    },
    compression,
    nsis_notice: copy(
      path.join(nsisCache, "COPYING"),
      path.join(output, "installer", "NSIS-3.11", "COPYING-CACHE.txt"),
    ),
    tauri_plugin: {
      version: "0.5.3",
      license: "MIT OR Apache-2.0",
      url: "https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll",
      sha1: pluginSha1,
      sha256: hash(pluginBytes),
      exact_transitive_binary_inventory: "not supplied upstream; unresolved",
    },
    native_inputs: nativeInputs.map((file) => ({
      file,
      sha256: hash(fs.readFileSync(path.join(nsisCache, file))),
      scope:
        "cache input; includes potentially unused standard plugins, not an extracted installer inventory",
    })),
    generated_script_sha256: hash(portableScript),
    hooks_sha256: hash(hooks),
    optional_unpin_helper: {
      excluded_by_empty_override: expectedOrder && emptyOverride,
      source: "https://stackoverflow.com/a/42816728",
      author: "Anders",
      upstream_content_license: "CC BY-SA 3.0",
      source_not_redistributed: true,
      behavior:
        "User manually unpins any retained taskbar/Start pin; normal shortcut deletion remains",
    },
    app_id:
      "Windows COM calls through standard System.dll; no separate AppId.dll in the generated inputs",
    file_association_helper_expanded: /!insertmacro\s+APP_ASSOCIATE/.test(
      script,
    ),
    webview2: script.includes('!define INSTALLWEBVIEW2MODE ""')
      ? "No runtime/bootstrapper bundled or downloaded by this selection"
      : "Recheck generated runtime-install mode",
    verification_limit:
      "Static generated-script and hook checks. NSIS /PPO did not resolve nsis_tauri_utils during preprocessing; no successful expanded-script or binary extraction claim.",
  });
}
const installerProvenance = path.join(
  output,
  "installer",
  "upstream-provenance.json",
);
if (fs.existsSync(installerProvenance)) {
  for (const entry of JSON.parse(read(installerProvenance))) {
    if (
      !fs.existsSync(path.join(root, entry.file)) ||
      hash(fs.readFileSync(path.join(root, entry.file))) !== entry.sha256
    )
      issues.push(`Installer notice checksum mismatch: ${entry.file}`);
  }
} else
  issues.push(
    "Missing installer license provenance; retrieve pinned installer notices first",
  );
const inventory = {
  schema: 1,
  target,
  root_features: ["custom-protocol"],
  lockfile_sha256: {
    cargo: hash(cargoLockText),
    npm: hash(read(path.join(root, "package-lock.json"))),
  },
  counts: {
    cargo_lock_packages: cargoLock.length,
    rust_windows_dependencies: rust.length,
    npm_lock_dependencies: npm.length,
    npm_installed: npm.filter((entry) => entry.installed).length,
  },
  rust,
  npm,
  native: {
    sqlite: {
      version: sqliteVersion,
      license: "Public Domain",
      source: "https://www.sqlite.org/copyright.html",
      wrapper: `libsqlite3-sys ${sqlite.version} (MIT)`,
    },
    webview2_sdk: {
      version: "1.0.3800.47",
      license: "BSD-3-Clause; exact SDK notice preserved separately",
      wrapper: "webview2-com-sys 0.39.1 (MIT)",
      native_file: "x64/WebView2LoaderStatic.lib",
      sha256: hash(fs.readFileSync(loaderFile)),
      runtime: "Evergreen installed separately; not bundled",
    },
  },
  rust_toolchain: { version: rustVersion, texts: toolchainTexts },
  installer: installerReview,
  unresolved: issues,
};
json(path.join(output, "inventory.json"), inventory);
json(path.join(output, "cargo-lock-inventory.json"), cargoLock);
const cell = (value) =>
  String(value ?? "")
    .replaceAll("|", "\\|")
    .replaceAll("\n", " ");
function table(title, entries) {
  return (
    `# ${title}\n\nGenerated by scripts/audit-licenses.mjs from the locked dependency graph. Do not edit by hand. Full text paths are relative to the repository/package root.\n\n| Package | Exact version | Declared license | Role | Preserved text |\n| --- | --- | --- | --- | --- |\n` +
    entries
      .map(
        (entry) =>
          `| [${cell(entry.name)}](${entry.source}) | ${entry.version} | ${cell(entry.license)} | ${entry.role} | ${entry.texts.map((text) => `[${cell(path.basename(text.file))}](../${text.file.replace(/^licenses\//, "")})`).join(", ") || "Not installed / see unresolved inventory"} |`,
      )
      .join("\n") +
    "\n"
  );
}
write(
  path.join(output, "inventory", "rust.md"),
  table("Windows Rust dependency inventory", rust),
);
write(
  path.join(output, "inventory", "npm.md"),
  table("Locked npm dependency inventory", npm),
);
console.log(
  JSON.stringify(
    { counts: inventory.counts, sqlite: sqliteVersion, unresolved: issues },
    null,
    2,
  ),
);
if (issues.length) process.exitCode = 2;
