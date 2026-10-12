import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppStatus, LibraryEntry } from "./types";

// Explicitly opt-in native integration tests. The normal executable returns null
// from smoke_config. No import, key injection, or test mutation runs in that case.
// The test executable uses an isolated application-data root supplied by the CLI.
interface SmokeConfig {
  phase: "exercise" | "restart";
  paths: string[];
  expected_count: number;
}
interface Check {
  name: string;
  passed: boolean;
  elapsed_ms: number;
  detail?: unknown;
}
const PERSISTED_TITLE = "Keyloom smoke · persisted arrangement";
const PERSISTED_ARTIST = "Original generated test exercise";
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
async function library() {
  return invoke<LibraryEntry[]>("library_list", {
    query: "",
    favorites: false,
  });
}
async function snapshot() {
  return invoke<AppStatus>("status");
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
        "restart retains two canonical arrangements and edited metadata",
        async () => {
          const entries = await library();
          assert(
            entries.length === config.expected_count,
            `Expected ${config.expected_count} arrangements after restart, got ${entries.length}`,
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
            song_id: first.id,
            saved_speed: 0.75,
            saved_mode: "wait",
            saved_position_ms: expectedPosition,
          };
        },
      );
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
        "explicit simulated player input satisfies native wait target; physical keyboard remains unverified",
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
          await waitFor(
            "injected player chord accepted by real native matcher",
            async () =>
              (await snapshot()).transport.correct > waiting.transport.correct,
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
