import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AppStatus,
  Device,
  LibraryEntry,
  OpenSong,
  Preferences,
} from "./types";

// Explicitly opt-in native integration tests. The normal executable returns null
// from smoke_config. No import, key injection, or test mutation runs in that case.
// The test executable uses an isolated application-data root supplied by the CLI.
interface SmokeConfig {
  phase: "exercise" | "restart";
  paths: string[];
  expected_count: number;
  device_id?: string | null;
  output_device_id?: string | null;
  export_path: string;
}
interface Check {
  name: string;
  passed: boolean;
  elapsed_ms: number;
  detail?: unknown;
}
const PERSISTED_TITLE = "Keyloom smoke · persisted arrangement";
const PERSISTED_ARTIST = "Original generated test exercise";
const PERSISTED_VARIANT = "Keyloom smoke · practice version";
const EXPORTED_TITLE = "Keyloom edited export";
const sleep = (milliseconds: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

function assert(condition: unknown, description: string): asserts condition {
  if (!condition) throw new Error(description);
}
async function waitFor<T>(
  description: string,
  read: () => T | Promise<T>,
  timeout = 15000,
): Promise<NonNullable<T>> {
  const end = performance.now() + timeout;
  while (performance.now() < end) {
    const value = await read();
    if (value) return value as NonNullable<T>;
    await sleep(60);
  }
  throw new Error(`Timed out: ${description}`);
}
function get<T extends Element>(
  selector: string,
  root: ParentNode = document,
): T {
  const element = root.querySelector<T>(selector);
  assert(element, `Missing UI element: ${selector}`);
  return element;
}
function button(text: string, root: ParentNode = document): HTMLButtonElement {
  const element = [...root.querySelectorAll<HTMLButtonElement>("button")].find(
    (item) => item.textContent?.trim() === text,
  );
  assert(element, `Missing UI button: ${text}`);
  assert(!element.disabled, `UI button disabled: ${text}`);
  return element;
}
function inputByLabel(
  text: string,
  root: ParentNode = document,
): HTMLInputElement {
  const label = [...root.querySelectorAll<HTMLLabelElement>("label")].find(
    (item) => item.textContent?.trim().startsWith(text),
  );
  assert(label, `Missing UI label: ${text}`);
  return get<HTMLInputElement>("input", label);
}
function setInput(input: HTMLInputElement, value: string) {
  assert(
    !input.disabled,
    `Cannot edit disabled UI input: ${input.getAttribute("aria-label")}`,
  );
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  assert(setter, "Browser input value setter unavailable");
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}
function setSelect(select: HTMLSelectElement, value: string) {
  assert(
    !select.disabled,
    `Cannot edit disabled UI select: ${select.ariaLabel}`,
  );
  assert(
    [...select.options].some((option) => option.value === value),
    `Missing option ${value} in ${select.ariaLabel}`,
  );
  const setter = Object.getOwnPropertyDescriptor(
    HTMLSelectElement.prototype,
    "value",
  )?.set;
  assert(setter, "Browser select value setter unavailable");
  setter.call(select, value);
  select.dispatchEvent(new Event("change", { bubbles: true }));
}
async function selectLesson(id: string) {
  const select = await waitFor(`version selector includes ${id}`, () => {
    const element = document.querySelector<HTMLSelectElement>(
      'select[aria-label="Lesson version"]',
    );
    return element && [...element.options].some((option) => option.value === id)
      ? element
      : null;
  });
  if (select.value !== id) setSelect(select, id);
  await waitFor(`lesson ${id} opened through version selector`, () =>
    document.querySelector(`.practice-view[data-song-id="${id}"]`),
  );
}
async function readOpenLesson(id: string) {
  assert(
    document.querySelector(`.practice-view[data-song-id="${id}"]`),
    "Native lesson verification must match the lesson visible in the actual UI",
  );
  return invoke<OpenSong>("open_song", { id });
}
async function selectNotes(ids: number[]) {
  for (const id of ids) {
    const input = get<HTMLInputElement>(
      `input[aria-label="Select note ${id}"]`,
    );
    if (!input.checked) input.click();
    await waitFor(`note ${id} selected`, () => input.checked);
  }
  await waitFor(
    "editor reports selected note count",
    () =>
      get<HTMLElement>(".lesson-edit-summary").dataset.selectedCount ===
      String(ids.length),
  );
}
async function waitRemovedCount(count: number) {
  await waitFor(
    `editor reports ${count} removed notes`,
    () =>
      document.querySelector<HTMLElement>(".lesson-edit-summary")?.dataset
        .removedCount === String(count),
  );
}
async function waitShiftedCount(count: number) {
  await waitFor(
    `editor reports ${count} shifted notes`,
    () =>
      document.querySelector<HTMLElement>(".lesson-edit-summary")?.dataset
        .shiftedCount === String(count),
  );
}
function musicalNotes(song: OpenSong["song"]) {
  return song.notes
    .map(({ part_id, channel, pitch, velocity, start_tick, end_tick }) => [
      part_id,
      channel,
      pitch,
      velocity,
      start_tick,
      end_tick,
    ])
    .sort((a, b) => {
      for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i]) return a[i] - b[i];
      }
      return 0;
    });
}
async function setKeyboardRange(low: number, high: number) {
  const setEndpoint = async (endpoint: "low" | "high", value: number) => {
    const label = endpoint === "low" ? "Lowest" : "Highest";
    const select = get<HTMLSelectElement>(
      `select[aria-label="${label} keyboard key"]`,
    );
    if (Number(select.value) !== value) setSelect(select, String(value));
    await waitFor(`keyboard ${endpoint}=${value} saved`, async () => {
      const preferences = await invoke<Preferences>("preferences");
      return preferences[endpoint] === value && Number(select.value) === value;
    });
  };
  const current = await invoke<Preferences>("preferences");
  // Expand first so every intermediate UI preference remains a valid range.
  if (low < current.low) await setEndpoint("low", low);
  if (high > current.high) await setEndpoint("high", high);
  await setEndpoint("low", low);
  await setEndpoint("high", high);
}
async function library() {
  return invoke<LibraryEntry[]>("library_list", {
    query: "",
    favorites: false,
  });
}
async function snapshot() {
  return invoke<AppStatus>("status");
}
async function audioSample(expectTone: boolean) {
  const before = await snapshot();
  assert(
    before.audio_available,
    `Native audio unavailable: ${before.audio_error}`,
  );
  await sleep(240);
  const after = await snapshot();
  const frames = after.audio_frames - before.audio_frames;
  const nonzero = after.audio_nonzero_frames - before.audio_nonzero_frames;
  assert(frames > 0, "Native audio callbacks stopped during the sample");
  assert(
    expectTone ? nonzero > 0 : nonzero === 0,
    expectTone
      ? "Expected native nonzero audio frames while PC monitoring is enabled"
      : `Expected muted native output; observed ${nonzero} nonzero frames`,
  );
  return { frames, nonzero_frames: nonzero, audible_output_verified: false };
}
async function setGuide(enabled: boolean) {
  const toggle = get<HTMLInputElement>('input[aria-label="Light Guide"]');
  if (toggle.checked !== enabled) toggle.click();
  await waitFor(`Light Guide ${enabled ? "on" : "off"}`, async () => {
    const state = await snapshot();
    return state.light_guide === enabled && toggle.checked === enabled;
  });
  // Allow the already-running audio callback and published counters to observe
  // the native preference change before taking a steady-state output sample.
  await sleep(100);
}
async function navigate(text: string, selector: string) {
  button(text, get("nav")).click();
  await waitFor(`navigate to ${text}`, () => document.querySelector(selector));
}
async function seek(ms: number) {
  const range = get<HTMLInputElement>('input[aria-label="Song position"]');
  setInput(range, String(ms));
  await sleep(60);
  range.dispatchEvent(
    new KeyboardEvent("keyup", {
      key: "ArrowRight",
      code: "ArrowRight",
      bubbles: true,
    }),
  );
  await waitFor(
    `seek to ${ms} ms`,
    async () => Math.abs((await snapshot()).transport.position_ms - ms) < 2,
  );
}
async function openFromImport(id: string) {
  const row = await waitFor("ready import result", () =>
    document.querySelector<HTMLElement>(
      `.import-result[data-song-id="${id}"][data-import-state="ready"]`,
    ),
  );
  button("Open", row).click();
  await waitFor("direct Open reaches actual practice view", () =>
    document.querySelector(`.practice-view[data-song-id="${id}"]`),
  );
}
async function animationFrames(count = 120) {
  return new Promise<{
    samples: number;
    mean_ms: number;
    p95_ms: number;
    max_ms: number;
    description: string;
  }>((resolve, reject) => {
    const deltas: number[] = [];
    let previous = 0,
      request = 0;
    const timer = window.setTimeout(() => {
      cancelAnimationFrame(request);
      reject(
        new Error(
          "Native WebView animation frame sample timed out; keep the test window visible.",
        ),
      );
    }, 15000);
    const sample = (now: number) => {
      if (previous) deltas.push(now - previous);
      previous = now;
      if (deltas.length >= count) {
        window.clearTimeout(timer);
        const sorted = [...deltas].sort((a, b) => a - b);
        resolve({
          samples: deltas.length,
          mean_ms:
            deltas.reduce((sum, value) => sum + value, 0) / deltas.length,
          p95_ms: sorted[Math.floor(sorted.length * 0.95)],
          max_ms: sorted[sorted.length - 1],
          description:
            "Native WebView requestAnimationFrame cadence while the real practice view is visible. This is not USB-to-sound latency or a full GPU presentation measurement.",
        });
      } else request = requestAnimationFrame(sample);
    };
    request = requestAnimationFrame(sample);
  });
}

export async function runNativeSmoke() {
  if (!isTauri()) return;
  const config = await invoke<SmokeConfig | null>("smoke_config");
  if (!config) return;
  const checks: Check[] = [];
  const start = performance.now();
  let activeCheck = "native UI initialization";
  let frameMetrics: unknown = null;
  const check = async (name: string, action: () => Promise<unknown>) => {
    activeCheck = name;
    const started = performance.now();
    const detail = await action();
    checks.push({
      name,
      passed: true,
      elapsed_ms: performance.now() - started,
      detail,
    });
  };
  try {
    await waitFor("native library UI", () =>
      document.querySelector(".library-view"),
    );
    assert(
      !document.querySelector(".preview-banner"),
      "The integration test must run in the real native application.",
    );
    if (config.phase === "restart") {
      await check(
        "restart retains original arrangements, exported MIDI and edited metadata",
        async () => {
          const entries = await library();
          const originals = entries.filter((entry) => !entry.original_id);
          const variants = entries.filter((entry) => entry.original_id);
          assert(
            originals.length === config.expected_count + 1 &&
              originals.some((entry) => entry.title === EXPORTED_TITLE) &&
              variants.length === 1 &&
              variants[0].title === PERSISTED_VARIANT,
            `Expected ${config.expected_count + 1} imported arrangements and one practice version after restart, got ${originals.length} originals and ${variants.length} versions`,
          );
          const first = entries.find(
            (entry) => entry.title === PERSISTED_TITLE,
          );
          assert(first, "Edited title was not retained across restart");
          assert(
            first.artist === PERSISTED_ARTIST && first.favorite,
            "Metadata/favorite did not survive restart",
          );
          assert(
            first.last_played !== null,
            "Recently played metadata was not retained",
          );
          const row = await waitFor(
            "restored entry in canonical library UI",
            () =>
              document.querySelector<HTMLElement>(
                `.song-row[data-song-id="${first.id}"]`,
              ),
          );
          button("Practice", row).click();
          await waitFor("restored practice view", () =>
            document.querySelector(
              `.practice-view[data-song-id="${first.id}"]`,
            ),
          );
          await waitFor(
            "saved tempo in actual UI",
            () =>
              get<HTMLInputElement>('input[aria-label="Playback speed"]')
                .value === "75",
          );
          assert(
            button("Wait for me").classList.contains("selected"),
            "Wait mode did not survive restart",
          );
          const expectedPosition =
            Math.floor(Math.min(1000, first.duration_ms / 2) / 10) * 10;
          await waitFor(
            "saved transport position after restart",
            async () =>
              Math.abs(
                (await snapshot()).transport.position_ms - expectedPosition,
              ) < 2,
          );
          return {
            library_count: entries.length,
            original_count: originals.length,
            practice_version_count: variants.length,
            song_id: first.id,
            saved_speed: 0.75,
            saved_mode: "wait",
            saved_position_ms: expectedPosition,
          };
        },
      );
      await check(
        "practice version pitch, removals, independent settings and mixed Undo survive restart",
        async () => {
          const entries = await library();
          const first = entries.find(
            (entry) => entry.title === PERSISTED_TITLE,
          )!;
          const variant = entries.find(
            (entry) => entry.title === PERSISTED_VARIANT,
          )!;
          assert(
            variant.original_id === first.id,
            "Practice version lost its original link",
          );
          const original = await readOpenLesson(first.id);
          const removedIds = original.song.notes
            .slice(0, 2)
            .map((note) => note.id);
          await selectLesson(variant.id);
          const saved = await readOpenLesson(variant.id);
          const shifted = saved.song.notes.filter(
            (note) =>
              note.pitch !==
              original.song.notes.find((source) => source.id === note.id)
                ?.pitch,
          );
          assert(
            shifted.length === 1 &&
              saved.lesson?.shifted_count === 1 &&
              shifted[0].pitch ===
                original.song.notes.find((note) => note.id === shifted[0].id)!
                  .pitch +
                  12,
            "The exact octave move did not survive restart",
          );
          assert(
            saved.lesson?.removed_count === removedIds.length &&
              saved.lesson.can_undo &&
              saved.song.notes.length ===
                original.song.notes.length - removedIds.length &&
              saved.song.notes.every((note) => !removedIds.includes(note.id)),
            "Saved practice version did not retain its filtered notes and Undo history",
          );
          const expectedPosition =
            Math.floor(Math.min(500, original.song.duration_ms / 4) / 10) * 10;
          assert(
            saved.settings.speed === 0.5 &&
              saved.settings.mode === "timed" &&
              Math.abs(saved.settings.position_ms - expectedPosition) < 2,
            "Practice version did not retain independent speed, mode and position",
          );
          await waitFor(
            "practice version speed visible after restart",
            () =>
              get<HTMLInputElement>('input[aria-label="Playback speed"]')
                .value === "50",
          );
          button("Edit notes").click();
          await waitRemovedCount(removedIds.length);
          button("Undo edit").click();
          await waitShiftedCount(0);
          const pitchUndone = await readOpenLesson(variant.id);
          assert(
            JSON.stringify(pitchUndone.song.notes) ===
              JSON.stringify(
                original.song.notes.filter(
                  (note) => !removedIds.includes(note.id),
                ),
              ),
            "Mixed Undo did not restore pitch while retaining the earlier removal",
          );
          button("Undo edit").click();
          await waitRemovedCount(0);
          const restored = await readOpenLesson(variant.id);
          assert(
            JSON.stringify(restored.song.notes) ===
              JSON.stringify(original.song.notes) &&
              restored.lesson &&
              restored.lesson.revision > saved.lesson.revision,
            "Undo after restart did not restore the original note data with a new revision",
          );
          button("Done editing").click();
          await selectLesson(first.id);
          const untouched = await readOpenLesson(first.id);
          const expectedOriginalPosition =
            Math.floor(Math.min(1000, first.duration_ms / 2) / 10) * 10;
          assert(
            JSON.stringify(untouched.song) === JSON.stringify(original.song) &&
              untouched.settings.speed === 0.75 &&
              untouched.settings.mode === "wait" &&
              Math.abs(
                untouched.settings.position_ms - expectedOriginalPosition,
              ) < 2,
            "Practice version Undo altered original notes or the original saved settings",
          );
          return {
            original_id: first.id,
            practice_version_id: variant.id,
            removed_note_ids: removedIds,
            saved_revision: saved.lesson.revision,
            undo_revision: restored.lesson.revision,
            saved_speed: 0.5,
            saved_mode: "timed",
            saved_position_ms: expectedPosition,
            original_unchanged: true,
            physical_keyboard_verified: false,
          };
        },
      );
      if (config.output_device_id) {
        await check(
          "saved Light Guide mutes native PC audio after restart",
          async () => {
            const preferences = await invoke<Preferences>("preferences");
            assert(
              preferences.light_guide &&
                preferences.monitor === "software" &&
                preferences.simulated,
              "Guide/software-monitor/simulated preferences did not survive restart",
            );
            const before = await snapshot();
            assert(
              before.light_guide,
              "Saved guide mute was not active at startup",
            );
            await invoke("input_note", { pitch: 60, on: true, velocity: 90 });
            await waitFor("restarted simulated press monitored", async () =>
              (await snapshot()).held.includes(60),
            );
            const sample = await audioSample(false);
            await invoke("input_note", { pitch: 60, on: false, velocity: 0 });
            const after = await snapshot();
            assert(
              after.transport.position_ms === before.transport.position_ms &&
                after.transport.correct === before.transport.correct,
              "Restart mute check changed saved practice position or score",
            );
            return {
              source: "explicit simulated input through native input_note",
              saved_light_guide: true,
              saved_monitor: "software",
              sample,
              physical_keyboard_verified: false,
            };
          },
        );
      }
    } else {
      assert(
        config.paths.length >= 3,
        "Smoke configuration needs two valid original MIDI paths and one corrupt path.",
      );
      let first: LibraryEntry;
      let second: LibraryEntry;
      await check("initial isolated library is empty", async () => {
        const entries = await library();
        assert(
          entries.length === 0,
          "Use a fresh isolated smoke data directory; the initial library is not empty.",
        );
        await waitFor("empty rendered library", () =>
          document.querySelector(".empty-state h3"),
        );
        return { count: entries.length };
      });
      await check(
        "import while search and favorites hide entries; corrupt batch item does not block valid MIDI",
        async () => {
          const search = get<HTMLInputElement>(
            'input[aria-label="Search library"]',
          );
          setInput(search, "keyloom-smoke-intentionally-no-match");
          button("Favorites").click();
          await waitFor("favorite filter applied", () =>
            button("Favorites").classList.contains("selected"),
          );
          const started = performance.now();
          await invoke("smoke_open_files", {
            paths: [config.paths[0], config.paths[2]],
          });
          await waitFor("visible native import acknowledgement", () =>
            document.querySelector(".import-result"),
          );
          const acknowledgement = performance.now() - started;
          await waitFor(
            "one valid native library entry",
            async () => (await library()).length === 1,
          );
          first = (await library())[0];
          await waitFor("corrupt item visibly failed", () =>
            document.querySelector(
              '.import-result[data-import-state="failed"]',
            ),
          );
          await waitFor("valid item visibly ready", () =>
            document.querySelector(
              `.import-result[data-song-id="${first.id}"][data-import-state="ready"]`,
            ),
          );
          assert(
            search.value === "keyloom-smoke-intentionally-no-match",
            "Import reset the active search",
          );
          assert(
            button("Favorites").classList.contains("selected"),
            "Import reset the active favorites filter",
          );
          assert(
            document.querySelectorAll(".song-row").length === 0,
            "The active filter should hide the canonical song row in this test",
          );
          await openFromImport(first.id);
          return {
            acknowledged_ms: acknowledgement,
            ready_ms: performance.now() - started,
            canonical_id: first.id,
            source:
              "real native files-opened event → UI import queue → import_files → library query → direct Open",
          };
        },
      );
      await check(
        "native Play advances authoritative transport and renders practice canvas",
        async () => {
          const canvas = get<HTMLCanvasElement>("canvas.piano-roll");
          assert(
            canvas.width > 100 && canvas.height > 100,
            "Practice canvas has no rendered area",
          );
          button("Play").click();
          await waitFor(
            "native transport advances",
            async () => (await snapshot()).transport.position_ms > 50,
          );
          frameMetrics = await animationFrames();
          const state = await snapshot();
          if (state.transport.playing) button("Pause").click();
          await waitFor(
            "native pause",
            async () => !(await snapshot()).transport.playing,
          );
          return {
            position_ms: state.transport.position_ms,
            native_audio_available: state.audio_available,
            audio_error: state.audio_error,
            canvas_width: canvas.width,
            canvas_height: canvas.height,
            audible_output_verified: false,
          };
        },
      );
      await check(
        "second unique MIDI imported during practice exposes direct Open without restart",
        async () => {
          await invoke("smoke_open_files", { paths: [config.paths[1]] });
          await waitFor(
            "second native library entry",
            async () => (await library()).length === config.expected_count,
          );
          second = (await library()).find((entry) => entry.id !== first.id)!;
          assert(
            second,
            "Second arrangement did not receive a unique canonical ID",
          );
          const banner = await waitFor(
            "global import success while practice remains open",
            () => {
              const element =
                document.querySelector<HTMLElement>(".import-banner");
              return element?.textContent?.includes(second.title)
                ? element
                : null;
            },
          );
          button("Open", banner).click();
          await waitFor("second arrangement practice view", () =>
            document.querySelector(
              `.practice-view[data-song-id="${second.id}"]`,
            ),
          );
          return { first_id: first.id, second_id: second.id, restarts: 0 };
        },
      );
      await check(
        "12 repeated byte-identical imports in one session remain deduplicated and visible",
        async () => {
          await navigate("Library", ".library-view");
          setInput(
            get<HTMLInputElement>('input[aria-label="Search library"]'),
            "",
          );
          button("All songs").click();
          await waitFor(
            "both arrangements visible",
            () =>
              document.querySelectorAll(".song-row").length ===
              config.expected_count,
          );
          button("Clear finished").click();
          await invoke("smoke_open_files", {
            paths: Array.from({ length: 12 }, () => config.paths[0]),
          });
          await waitFor(
            "all duplicate imports ready",
            () =>
              document.querySelectorAll(
                '.import-result[data-import-state="ready"]',
              ).length === 12,
            30000,
          );
          assert(
            !document.querySelector(
              '.import-result[data-import-state="failed"]',
            ),
            "A repeated import failed",
          );
          const entries = await library();
          assert(
            entries.length === config.expected_count,
            "Exact-byte duplicate import created an extra arrangement",
          );
          assert(
            [...document.querySelectorAll(".import-result small")].every(
              (element) =>
                element.textContent?.includes("Already in your library"),
            ),
            "Duplicate results were not honestly labeled",
          );
          return { duplicate_attempts: 12, canonical_count: entries.length };
        },
      );
      await check(
        "metadata edits and tempo changes reach native persistence through actual controls",
        async () => {
          const row = get<HTMLElement>(`.song-row[data-song-id="${first.id}"]`);
          button("Edit", row).click();
          const modal = await waitFor("metadata editor", () =>
            document.querySelector<HTMLElement>(".modal"),
          );
          setInput(inputByLabel("Title", modal), PERSISTED_TITLE);
          await sleep(60);
          setInput(inputByLabel("Artist", modal), PERSISTED_ARTIST);
          await sleep(60);
          inputByLabel("Favorite this arrangement", modal).click();
          await sleep(60);
          button("Save details", modal).click();
          await waitFor("metadata saved", async () =>
            (await library()).some(
              (entry) =>
                entry.id === first.id &&
                entry.title === PERSISTED_TITLE &&
                entry.favorite,
            ),
          );
          await waitFor(
            "metadata dialog closed",
            () => !document.querySelector(".modal"),
          );
          button(
            "Practice",
            get(`.song-row[data-song-id="${first.id}"]`),
          ).click();
          await waitFor("first arrangement reopened", () =>
            document.querySelector(
              `.practice-view[data-song-id="${first.id}"]`,
            ),
          );
          setInput(
            get<HTMLInputElement>('input[aria-label="Playback speed"]'),
            "75",
          );
          await waitFor(
            "native tempo configured",
            async () => (await snapshot()).transport.speed === 0.75,
          );
          return {
            title: PERSISTED_TITLE,
            artist: PERSISTED_ARTIST,
            favorite: true,
            speed: 0.75,
          };
        },
      );
      await check(
        "simulated input is monitored but cannot satisfy the native wait target",
        async () => {
          await navigate("Keyboard & sound", ".devices-view");
          const enabled = inputByLabel("Enable simulated player input");
          if (!enabled.checked) enabled.click();
          await waitFor(
            "native simulated preference enabled",
            async () =>
              (await invoke<{ simulated: boolean }>("preferences")).simulated,
          );
          await navigate("Practice", ".practice-view");
          button("Wait for me").click();
          await waitFor(
            "native wait mode",
            async () => (await snapshot()).transport.mode === "wait",
          );
          button("Restart").click();
          await waitFor(
            "restart at beginning",
            async () => (await snapshot()).transport.position_ms === 0,
          );
          button("Play").click();
          const waiting = await waitFor("native wait target", async () => {
            const value = await snapshot();
            return value.transport.waiting && value.transport.targets.length
              ? value
              : null;
          });
          const position = waiting.transport.position_ms;
          await sleep(180);
          assert(
            Math.abs((await snapshot()).transport.position_ms - position) < 1,
            "Wait mode did not stop authoritative musical position",
          );
          for (const pitch of waiting.transport.targets)
            await invoke("input_note", { pitch, on: true, velocity: 90 });
          await sleep(240);
          const untouched = await snapshot();
          assert(
            untouched.transport.correct === waiting.transport.correct &&
              untouched.transport.waiting &&
              Math.abs(untouched.transport.position_ms - position) < 1 &&
              untouched.transport.matched.length === 0,
            "Simulated notes must not satisfy, advance or partially match physical practice",
          );
          for (const pitch of waiting.transport.targets)
            await invoke("input_note", { pitch, on: false, velocity: 0 });
          const played = await snapshot();
          assert(
            played.events.some(
              (event) =>
                event.source === "simulated" && event.kind === "note_on",
            ),
            "Injected input was not labeled simulated in native event monitor",
          );
          if (played.transport.playing) button("Pause").click();
          await waitFor(
            "pause after simulated exercise",
            async () => !(await snapshot()).transport.playing,
          );
          const savedPosition =
            Math.floor(Math.min(1000, first.duration_ms / 2) / 10) * 10;
          await seek(savedPosition);
          // Let the native published session observe the seek before smoke_report
          // persists it and closes the process for the restart phase.
          await sleep(150);
          return {
            source: "explicit simulated input through native input_note",
            target_pitches: waiting.transport.targets,
            correct_before: waiting.transport.correct,
            correct_after: played.transport.correct,
            saved_position_ms: savedPosition,
            physical_keyboard_verified: false,
          };
        },
      );
      await check(
        "create, edit, compare, undo and play a saved practice version through the native UI",
        async () => {
          const original = await readOpenLesson(first.id);
          assert(
            original.song.notes.length > 2,
            "Practice version smoke needs at least three original notes",
          );
          assert(
            !original.lesson && !original.entry.original_id,
            "Imported fixture must be an original lesson",
          );
          const removedIds = original.song.notes
            .slice(0, 2)
            .map((note) => note.id);
          button("Save practice version").click();
          const name = await waitFor("practice version name input", () =>
            document.querySelector<HTMLInputElement>(
              'input[aria-label="Practice version name"]',
            ),
          );
          setInput(name, PERSISTED_VARIANT);
          await sleep(60);
          button("Create practice version").click();
          const variant = await waitFor(
            "named practice version in native library",
            async () =>
              (await library()).find(
                (entry) => entry.title === PERSISTED_VARIANT,
              ),
          );
          await waitFor("new version opens immediately", () =>
            document.querySelector(
              `.practice-view[data-song-id="${variant.id}"]`,
            ),
          );
          assert(
            variant.original_id === first.id,
            "New practice version did not retain its source original",
          );
          const versions = await invoke<LibraryEntry[]>("lesson_versions", {
            id: variant.id,
          });
          assert(
            versions.length === 2 &&
              versions.some((entry) => entry.id === first.id) &&
              versions.some((entry) => entry.id === variant.id),
            "Native version family must contain the original and its practice version",
          );
          setInput(
            get<HTMLInputElement>('input[aria-label="Playback speed"]'),
            "50",
          );
          await waitFor(
            "independent practice version speed",
            async () => (await snapshot()).transport.speed === 0.5,
          );
          button("Timed").click();
          await waitFor(
            "independent practice version mode",
            async () => (await snapshot()).transport.mode === "timed",
          );
          // Enter editing in the same turn as Play. A Play awaiting settings
          // must not resume the transport after the editor has paused it.
          button("Play").click();
          button("Edit notes").click();
          await waitRemovedCount(0);
          await waitFor(
            "editor paused pending playback",
            async () => !(await snapshot()).transport.playing,
          );
          const editorBefore = await snapshot();
          const play = get<HTMLButtonElement>(".play-button");
          assert(play.disabled, "Play must be disabled while editing notes");
          const canvas = get<HTMLCanvasElement>(
            "canvas.piano-roll.note-editing",
          );
          const rect = canvas.getBoundingClientRect();
          const pointerErrors: string[] = [];
          const captureError = (event: ErrorEvent) =>
            pointerErrors.push(event.message);
          window.addEventListener("error", captureError);
          try {
            play.click();
            for (const [key, code] of [
              [" ", "Space"],
              ["a", "KeyA"],
            ]) {
              window.dispatchEvent(
                new KeyboardEvent("keydown", { key, code, bubbles: true }),
              );
              window.dispatchEvent(
                new KeyboardEvent("keyup", { key, code, bubbles: true }),
              );
            }
            for (const type of ["pointerdown", "pointerup"]) {
              canvas.dispatchEvent(
                new PointerEvent(type, {
                  bubbles: true,
                  pointerId: 731,
                  pointerType: "mouse",
                  clientX: rect.left + rect.width / 2,
                  clientY: rect.bottom - 4,
                  button: 0,
                  buttons: type === "pointerdown" ? 1 : 0,
                }),
              );
            }
            await sleep(180);
          } finally {
            window.removeEventListener("error", captureError);
          }
          const editorAfter = await snapshot();
          assert(
            !pointerErrors.length &&
              !editorAfter.transport.playing &&
              editorAfter.transport.position_ms ===
                editorBefore.transport.position_ms &&
              !editorAfter.events.some(
                (event) =>
                  event.source === "simulated" &&
                  event.sequence >
                    (editorBefore.events.at(-1)?.sequence ?? 0) &&
                  event.kind === "note_on",
              ),
            "Editor allowed playback/simulated keys or raised a pointer error during UI input",
          );
          await selectNotes(removedIds);
          button("Remove selected notes").click();
          await waitRemovedCount(removedIds.length);
          const edited = await readOpenLesson(variant.id);
          assert(
            edited.lesson?.removed_count === removedIds.length &&
              edited.lesson.can_undo &&
              JSON.stringify(edited.song.notes) ===
                JSON.stringify(
                  original.song.notes.filter(
                    (note) => !removedIds.includes(note.id),
                  ),
                ),
            "Native practice version load did not preserve exactly the unselected canonical notes",
          );
          button("Done editing").click();
          await selectLesson(first.id);
          const comparedOriginal = await readOpenLesson(first.id);
          assert(
            JSON.stringify(comparedOriginal.song) ===
              JSON.stringify(original.song) &&
              comparedOriginal.settings.speed === 0.75 &&
              comparedOriginal.settings.mode === "wait" &&
              Math.abs(
                comparedOriginal.settings.position_ms -
                  original.settings.position_ms,
              ) < 2,
            "Practice version editing changed the original notes or settings",
          );
          await selectLesson(variant.id);
          const comparedVersion = await readOpenLesson(variant.id);
          assert(
            comparedVersion.settings.speed === 0.5 &&
              comparedVersion.settings.mode === "timed",
            "Version comparison did not restore independent settings",
          );
          button("Edit notes").click();
          await waitRemovedCount(removedIds.length);
          button("Undo edit").click();
          await waitRemovedCount(0);
          const restored = await readOpenLesson(variant.id);
          assert(
            restored.lesson &&
              restored.lesson.revision > edited.lesson.revision &&
              JSON.stringify(restored.song.notes) ===
                JSON.stringify(original.song.notes),
            "Undo did not restore all removed canonical notes",
          );
          await selectNotes(removedIds);
          button("Remove selected notes").click();
          await waitRemovedCount(removedIds.length);
          const finalEdit = await readOpenLesson(variant.id);
          assert(
            finalEdit.lesson?.can_undo,
            "Final removal must retain Undo history for the restart check",
          );
          button("Done editing").click();

          const preferences = await invoke<Preferences>("preferences");
          const [low, high] = finalEdit.song.notes.some(
            (note) => note.pitch > 1,
          )
            ? [0, 1]
            : [126, 127];
          let outsideCount = 0;
          try {
            await navigate("Keyboard & sound", ".devices-view");
            await setKeyboardRange(low, high);
            await navigate("Practice", ".practice-view");
            outsideCount = finalEdit.song.notes.filter(
              (note) => note.pitch < low || note.pitch > high,
            ).length;
            assert(
              outsideCount > 0,
              "Narrow test keyboard range must exclude some lesson notes",
            );
            await waitFor(
              "version range warning matches native note data",
              () =>
                document.querySelector<HTMLElement>(".lesson-range-warning")
                  ?.dataset.outOfRangeCount === String(outsideCount),
            );
            button("Edit notes").click();
            await waitRemovedCount(removedIds.length);
            setSelect(
              get<HTMLSelectElement>('select[aria-label="Notes to show"]'),
              "outside",
            );
            const rows = await waitFor(
              "outside-range editor filter displays notes",
              () => {
                const candidates = [
                  ...document.querySelectorAll<HTMLElement>("[data-note-id]"),
                ];
                return candidates.length &&
                  get<HTMLSelectElement>('select[aria-label="Notes to show"]')
                    .value === "outside" &&
                  candidates.every((row) => {
                    const note = finalEdit.song.notes.find(
                      (item) => item.id === Number(row.dataset.noteId),
                    );
                    return note && (note.pitch < low || note.pitch > high);
                  })
                  ? candidates
                  : null;
              },
            );
            assert(
              rows.every((row) => {
                const note = finalEdit.song.notes.find(
                  (item) => item.id === Number(row.dataset.noteId),
                );
                return note && (note.pitch < low || note.pitch > high);
              }),
              "Outside-range editor filter included an in-range note",
            );
            button("Done editing").click();
          } finally {
            await navigate("Keyboard & sound", ".devices-view");
            await setKeyboardRange(preferences.low, preferences.high);
            await navigate("Practice", ".practice-view");
          }
          button("Restart").click();
          await waitFor(
            "edited version playback starts from zero",
            async () => (await snapshot()).transport.position_ms === 0,
          );
          button("Play").click();
          await waitFor(
            "edited native lesson is playable",
            async () => (await snapshot()).transport.position_ms > 60,
          );
          button("Pause").click();
          await waitFor(
            "edited lesson pauses",
            async () => !(await snapshot()).transport.playing,
          );
          const variantPosition =
            Math.floor(Math.min(500, original.song.duration_ms / 4) / 10) * 10;
          await seek(variantPosition);
          await selectLesson(first.id);
          const finalOriginal = await readOpenLesson(first.id);
          assert(
            JSON.stringify(finalOriginal.song) ===
              JSON.stringify(original.song) &&
              JSON.stringify(finalOriginal.settings) ===
                JSON.stringify(original.settings),
            "Returning from practice version did not preserve the original lesson and settings",
          );
          const entries = await library();
          assert(
            entries.length === config.expected_count + 1,
            "Practice version should add exactly one library entry",
          );
          return {
            original_id: first.id,
            practice_version_id: variant.id,
            removed_note_ids: removedIds,
            original_note_count: original.song.notes.length,
            practice_note_count: finalEdit.song.notes.length,
            persisted_revision: finalEdit.lesson.revision,
            outside_keyboard_count: outsideCount,
            saved_speed: 0.5,
            saved_mode: "timed",
            saved_position_ms: variantPosition,
            original_unchanged: true,
            keyboard_range_restored: true,
            editor_play_and_simulated_input_blocked: true,
            pending_play_did_not_escape_editor_pause: true,
            physical_keyboard_verified: false,
          };
        },
      );
      await check(
        "pitch controls, mixed Undo and export-to-import-to-play through native UI",
        async () => {
          const entries = await library();
          const first = entries.find(
            (entry) => entry.title === PERSISTED_TITLE,
          )!;
          const variant = entries.find(
            (entry) => entry.title === PERSISTED_VARIANT,
          )!;
          const original = await readOpenLesson(first.id);
          await selectLesson(variant.id);
          const before = await readOpenLesson(variant.id);
          button("Edit notes").click();
          await waitShiftedCount(0);
          const candidate = [...before.song.notes]
            .sort((a, b) => a.start_ms - b.start_ms || a.id - b.id)
            .slice(0, 48)
            .find(
              (note) =>
                note.pitch >= 12 &&
                note.pitch <= 114 &&
                !before.song.notes.some(
                  (other) =>
                    other.id !== note.id &&
                    other.part_id === note.part_id &&
                    other.pitch === note.pitch + 12 &&
                    other.start_tick < note.end_tick &&
                    other.end_tick > note.start_tick,
                ),
            );
          assert(
            candidate,
            "Fixture needs a visible octave-movable note without a merged-pitch overlap",
          );
          const verifyPitch = async (pitch: number) => {
            await waitFor(
              `note ${candidate.id} pitch ${pitch} in actual editor`,
              () =>
                document.querySelector<HTMLElement>(
                  `[data-note-id="${candidate.id}"]`,
                )?.dataset.notePitch === String(pitch),
            );
            const current = await readOpenLesson(variant.id);
            assert(
              JSON.stringify(current.song.notes) ===
                JSON.stringify(
                  before.song.notes.map((note) =>
                    note.id === candidate.id ? { ...note, pitch } : note,
                  ),
                ),
              "Pitch move changed other notes or timing",
            );
            return current;
          };
          for (const [up, down, amount] of [
            ["Semitone up", "Semitone down", 1],
            ["Octave up", "Octave down", 12],
          ] as const) {
            await selectNotes([candidate.id]);
            button(up).click();
            await verifyPitch(candidate.pitch + amount);
            assert(
              get<HTMLElement>(".lesson-edit-summary").dataset.selectedCount ===
                "1",
              "Pitch move lost selection",
            );
            button(down).click();
            await verifyPitch(candidate.pitch);
            button("Undo edit").click();
            await verifyPitch(candidate.pitch + amount);
            button("Undo edit").click();
            await verifyPitch(candidate.pitch);
          }
          await selectNotes([candidate.id]);
          button("Octave up").click();
          await verifyPitch(candidate.pitch + 12);
          button("Remove selected notes").click();
          await waitRemovedCount(3);
          await waitShiftedCount(0);
          button("Undo edit").click();
          await waitRemovedCount(2);
          await waitShiftedCount(1);
          const edited = await verifyPitch(candidate.pitch + 12);
          button("Done editing").click();
          button("Export MIDI", get(".lesson-version-bar")).click();
          await waitFor("export success contains actual new file path", () =>
            document.body.textContent?.includes(config.export_path),
          );
          await invoke("smoke_open_files", { paths: [config.export_path] });
          const exported = await waitFor(
            "exported MIDI immediately added to native library",
            async () =>
              (await library()).find((entry) => entry.title === EXPORTED_TITLE),
          );
          const importBanner = await waitFor(
            "exported MIDI ready banner in practice view",
            () => {
              const banner =
                document.querySelector<HTMLElement>(".import-banner");
              return banner?.textContent?.includes(EXPORTED_TITLE)
                ? banner
                : null;
            },
          );
          button("Open", importBanner).click();
          await waitFor("exported MIDI opened from ready banner", () =>
            document.querySelector(
              `.practice-view[data-song-id="${exported.id}"]`,
            ),
          );
          const reimported = await readOpenLesson(exported.id);
          assert(
            JSON.stringify(musicalNotes(reimported.song)) ===
              JSON.stringify(musicalNotes(edited.song)),
            "Exported MIDI changed edited note pitches, timing, velocities or tracks",
          );
          assert(
            reimported.song.ppq === edited.song.ppq &&
              reimported.song.duration_ms === edited.song.duration_ms &&
              JSON.stringify(reimported.song.tempos) ===
                JSON.stringify(edited.song.tempos) &&
              JSON.stringify(reimported.song.controllers) ===
                JSON.stringify(edited.song.controllers) &&
              reimported.settings.speed === 1,
            "Export baked practice speed or changed original musical timing/controllers",
          );
          button("Listen").click();
          button("Play").click();
          await waitFor(
            "reimported MIDI actually plays",
            async () => (await snapshot()).transport.position_ms > 60,
          );
          button("Pause").click();
          await waitFor(
            "reimported MIDI pauses",
            async () => !(await snapshot()).transport.playing,
          );
          await navigate("Library", ".library-view");
          button(
            "Practice",
            get(`.song-row[data-song-id="${first.id}"]`),
          ).click();
          await waitFor("original restored after exported playback", () =>
            document.querySelector(
              `.practice-view[data-song-id="${first.id}"]`,
            ),
          );
          const untouched = await readOpenLesson(first.id);
          assert(
            JSON.stringify(untouched.song) === JSON.stringify(original.song) &&
              JSON.stringify(untouched.settings) ===
                JSON.stringify(original.settings),
            "Export workflow changed original data/settings",
          );
          return {
            original_id: first.id,
            practice_version_id: variant.id,
            exported_id: exported.id,
            export_path: config.export_path,
            shifted_note_id: candidate.id,
            original_pitch: candidate.pitch,
            edited_pitch: candidate.pitch + 12,
            exported_note_count: reimported.song.notes.length,
            exact_musical_roundtrip: true,
            native_reimport_playback: true,
            physical_keyboard_verified: false,
          };
        },
      );
      if (config.device_id) {
        await check(
          "configured native MIDI port opens, closes, and reopens; no physical cable unplug is simulated",
          async () => {
            const id = config.device_id!;
            const available = await invoke<Device[]>("devices");
            const selected = available.find((device) => device.id === id);
            assert(
              selected,
              "The exact MIDI device configured for this native smoke test is not present.",
            );
            let cleanupRequired = false;
            try {
              for (let attempt = 1; attempt <= 2; attempt++) {
                cleanupRequired = true;
                await invoke("connect_device", { id });
                await waitFor(`native MIDI port open ${attempt}`, async () => {
                  const state = await snapshot();
                  return state.device_connected && state.device_id === id;
                });
                await invoke("connect_device", { id: null });
                await waitFor(`native MIDI port close ${attempt}`, async () => {
                  const state = await snapshot();
                  return !state.device_connected && state.device_id === null;
                });
                cleanupRequired = false;
              }
            } finally {
              if (cleanupRequired) await invoke("connect_device", { id: null });
            }
            return {
              device_id: id,
              device_name: selected.name,
              native_port_open_verified: true,
              native_port_open_count: 2,
              native_port_close_count: 2,
              physical_keyboard_verified: false,
              physical_cable_unplug_verified: false,
              description:
                "Real native input-port open/close/reopen through connect_device. This does not verify physical notes, pedal input, or cable unplug/replug behavior.",
            };
          },
        );
      }
    }
    if (config.phase === "exercise" && config.output_device_id) {
      const first = (await library()).find(
        (entry) => entry.title === PERSISTED_TITLE,
      );
      assert(first, "Persisted first arrangement missing before guide test");
      await check(
        "native channel-1 Light Guide test and cleanup; physical illumination requires owner observation",
        async () => {
          assert(
            config.device_id,
            "Light Guide smoke needs both input and output device IDs",
          );
          await invoke("connect_device", { id: config.device_id });
          await navigate("Keyboard & sound", ".devices-view");
          const select = get<HTMLSelectElement>(
            'select[aria-label="MIDI output"]',
          );
          const setter = Object.getOwnPropertyDescriptor(
            HTMLSelectElement.prototype,
            "value",
          )!.set!;
          setter.call(select, config.output_device_id);
          select.dispatchEvent(new Event("change", { bubbles: true }));
          await sleep(100);
          button("Connect output").click();
          await waitFor(
            "native output connected",
            async () => (await snapshot()).output_connected,
          );
          button("Test middle C").click();
          await waitFor("middle C test Note On sent", async () => {
            const s = await snapshot();
            return s.light_test_active && s.lit_notes.join() === "60";
          });
          await waitFor(
            "middle C automatic Note Off",
            async () => {
              const s = await snapshot();
              return !s.light_test_active && !s.lit_notes.length;
            },
            5000,
          );
          const software = inputByLabel("Through my computer");
          if (!software.checked) software.click();
          await waitFor(
            "software monitoring selected through UI",
            async () =>
              (await invoke<Preferences>("preferences")).monitor === "software",
          );
          await setGuide(false);
          await invoke("input_note", { pitch: 60, on: true, velocity: 90 });
          const beforeMute = await audioSample(true);
          // The note remains held while the UI enables Light Guide: the mute
          // must silence an existing voice as well as newly played PC tones.
          await setGuide(true);
          const heldToneMuted = await audioSample(false);
          await invoke("input_note", { pitch: 60, on: false, velocity: 0 });
          await invoke("input_note", { pitch: 61, on: true, velocity: 90 });
          await waitFor("muted simulated press still monitored", async () =>
            (await snapshot()).held.includes(61),
          );
          const newToneMuted = await audioSample(false);
          await invoke("input_note", { pitch: 61, on: false, velocity: 0 });
          await sleep(320);
          await setGuide(false);
          const releasedTonesRemainSilent = await audioSample(false);
          await invoke("input_note", { pitch: 62, on: true, velocity: 90 });
          const restoredTone = await audioSample(true);
          await invoke("input_note", { pitch: 62, on: false, velocity: 0 });
          await setGuide(true);
          await navigate("Practice", ".practice-view");
          button("Wait for me").click();
          button("Restart").click();
          await waitFor(
            "guide practice reset",
            async () => !(await snapshot()).transport.playing,
          );
          await sleep(100);
          const lightTarget = async () => {
            button("Play").click();
            return waitFor("practice target sent to output", async () => {
              const s = await snapshot();
              return s.transport.waiting && s.lit_notes.length ? s : null;
            });
          };
          const lit = await lightTarget();
          for (const pitch of lit.transport.targets)
            await invoke("input_note", { pitch, on: true, velocity: 90 });
          await sleep(350);
          const targetToneMuted = await audioSample(false);
          const untouched = await snapshot();
          assert(
            untouched.transport.correct === lit.transport.correct &&
              untouched.transport.wrong === lit.transport.wrong &&
              untouched.transport.missed === lit.transport.missed &&
              untouched.transport.position_ms === lit.transport.position_ms &&
              untouched.transport.waiting &&
              untouched.transport.matched.length === 0,
            "Guide output must not count as player input",
          );
          assert(
            untouched.lit_notes.join() === lit.lit_notes.join(),
            "Muted simulated presses changed the guide target",
          );
          for (const pitch of lit.transport.targets)
            await invoke("input_note", { pitch, on: false, velocity: 0 });
          button("Pause").click();
          await waitFor(
            "pause clears guide notes",
            async () => !(await snapshot()).lit_notes.length,
          );
          await sleep(100);
          await lightTarget();
          get<HTMLButtonElement>('button[aria-label="Stop"]').click();
          await waitFor(
            "stop clears guide notes",
            async () => !(await snapshot()).lit_notes.length,
          );
          await sleep(100);
          await lightTarget();
          await navigate("Library", ".library-view");
          // Current settings have changed; changing to another song must clear
          // the old chord before that song opens paused.
          const other = (await library()).find(
            (entry) => entry.id !== first.id,
          )!;
          button(
            "Practice",
            get(`.song-row[data-song-id="${other.id}"]`),
          ).click();
          await waitFor("song change clears guide notes", async () => {
            const s = await snapshot();
            return !s.transport.playing && !s.lit_notes.length;
          });
          const state = await snapshot();
          assert(
            !state.output_error,
            `Guide output error: ${state.output_error}`,
          );
          await invoke("connect_output", { id: null });
          await invoke("connect_device", { id: null });
          // Restore the original smoke persistence contract after the guide exercise.
          await navigate("Library", ".library-view");
          button(
            "Practice",
            get(`.song-row[data-song-id="${first.id}"]`),
          ).click();
          await waitFor("first arrangement restored", () =>
            document.querySelector(
              `.practice-view[data-song-id="${first.id}"]`,
            ),
          );
          await seek(
            Math.floor(Math.min(1000, first.duration_ms / 2) / 10) * 10,
          );
          return {
            output_device_id: config.output_device_id,
            messages_sent: state.output_messages,
            initial_targets: lit.lit_notes,
            pc_audio: {
              before_mute: beforeMute,
              held_tone_muted: heldToneMuted,
              new_tone_muted: newToneMuted,
              released_tones_remain_silent: releasedTonesRemainSilent,
              restored_tone: restoredTone,
              target_tone_muted: targetToneMuted,
              source: "explicit simulated input through native input_note",
              saved_light_guide_for_restart: true,
            },
            physical_illumination_verified: false,
            cleanup: [
              "test timeout",
              "pause",
              "stop",
              "song change",
              "disconnect",
            ],
          };
        },
      );
    }
    await invoke("smoke_report", {
      report: {
        passed: true,
        phase: config.phase,
        elapsed_ms: performance.now() - start,
        checks,
        animation_frames: frameMetrics,
        physical_keyboard_verified: false,
        audio_audibility_verified: false,
      },
    });
  } catch (reason) {
    const error =
      reason instanceof Error
        ? `${reason.message}\n${reason.stack ?? ""}`
        : String(reason);
    checks.push({
      name: activeCheck,
      passed: false,
      elapsed_ms: performance.now() - start,
      detail: error,
    });
    await invoke("smoke_report", {
      report: {
        passed: false,
        phase: config.phase,
        elapsed_ms: performance.now() - start,
        checks,
        error,
        animation_frames: frameMetrics,
        physical_keyboard_verified: false,
      },
    });
  }
}
