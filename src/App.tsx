import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PianoRoll from "./PianoRoll";
import {
  AppStatus,
  DEFAULT_PREFERENCES,
  Device,
  EMPTY_STATUS,
  ImportJob,
  ImportOutcome,
  ImportResult,
  LibraryEntry,
  MidiEvent,
  OpenSong,
  PartSetting,
  Preferences,
  SongSettings,
  noteName,
  partSetting,
  isSplitAccompaniment,
  timeLabel,
} from "./types";

type View = "library" | "practice" | "devices";
type IconName =
  | "library"
  | "practice"
  | "devices"
  | "plus"
  | "play"
  | "pause"
  | "stop"
  | "search"
  | "star"
  | "arrow"
  | "expand"
  | "close"
  | "refresh";
function Icon({ name, size = 19 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, string> = {
    library: "M4 4h5v16H4zM12 4h4v16h-4zM19 5l3 14",
    practice: "M3 5h18v14H3zM7 5v9h3V5m4 0v9h3V5M9 14v5m6-5v5",
    devices: "M9 3v6m6-6v6M6 9h12v3a6 6 0 0 1-12 0V9m6 9v4",
    plus: "M12 5v14M5 12h14",
    play: "M8 4l12 8-12 8z",
    pause: "M8 5v14M16 5v14",
    stop: "M6 6h12v12H6z",
    search: "M20 20l-5-5M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0",
    star: "m12 3 2.8 5.7 6.3.9-4.5 4.4 1.1 6.2-5.7-3-5.7 3 1.1-6.2L3.2 9.6l6.3-.9z",
    arrow: "M5 12h14m-6-6 6 6-6 6",
    expand: "M9 3H3v6m12-6h6v6M3 15v6h6m6 0h6v-6",
    close: "m6 6 12 12M18 6 6 18",
    refresh:
      "M20 7v5h-5M4 17v-5h5M6 7a7 7 0 0 1 12-2l2 3M4 16l2 3a7 7 0 0 0 12-2",
  };
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}
const native = isTauri();
const messageOf = (error: unknown) =>
  error instanceof Error ? error.message : String(error);
const dateLabel = (value: number) =>
  new Date(value < 1e12 ? value * 1000 : value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

export default function App() {
  const [view, setView] = useState<View>("library");
  const [entries, setEntries] = useState<LibraryEntry[]>([]);
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState(false);
  const [sort, setSort] = useState("imported");
  const [current, setCurrent] = useState<OpenSong | null>(null);
  const [status, setStatus] = useState<AppStatus>(EMPTY_STATUS);
  const [preferences, setPreferences] =
    useState<Preferences>(DEFAULT_PREFERENCES);
  const [deviceList, setDeviceList] = useState<Device[]>([]);
  const [events, setEvents] = useState<MidiEvent[]>([]);
  const [jobs, setJobs] = useState<ImportJob[]>([]);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [dragging, setDragging] = useState(false);
  const [metadata, setMetadata] = useState<LibraryEntry | null>(null);
  const [removing, setRemoving] = useState<LibraryEntry | null>(null);
  const [calibration, setCalibration] = useState<"low" | "high" | null>(null);
  const [showParts, setShowParts] = useState(true);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [seekValue, setSeekValue] = useState<number | null>(null);
  const [loopDraft, setLoopDraft] = useState<number | null>(null);
  const latestSnapshot = useRef(status);
  const latest = useRef({ preferences, calibration, current, status, view });
  latest.current = {
    preferences,
    calibration,
    current,
    status: latestSnapshot.current,
    view,
  };
  const importQueue = useRef(Promise.resolve());
  const openQueue = useRef(Promise.resolve());
  const pendingImports = useRef(0);
  const eventHistory = useRef<MidiEvent[]>([]);
  const refreshStatusSoon = useRef<() => void>(() => {});
  const jobId = useRef(0);
  const listGeneration = useRef(0);
  const openGeneration = useRef(0);
  const configured = useRef<{ id: string; settings: SongSettings } | null>(
    null,
  );
  const configTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const report = useCallback(
    (reason: unknown) => setError(messageOf(reason)),
    [],
  );
  useEffect(() => {
    if (view === "devices") setEvents(eventHistory.current);
  }, [view]);
  useEffect(() => {
    if (!metadata && !removing) return;
    const prior =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMetadata(null);
        setRemoving(null);
        return;
      }
      if (event.key !== "Tab") return;
      const controls = [
        ...document.querySelectorAll<HTMLElement>(
          ".modal button:not(:disabled), .modal input:not(:disabled), .modal select:not(:disabled)",
        ),
      ];
      const first = controls[0],
        last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener("keydown", keydown);
    return () => {
      document.removeEventListener("keydown", keydown);
      prior?.focus();
    };
  }, [!!metadata, !!removing]);

  const reload = useCallback(async () => {
    if (!native) {
      setLoading(false);
      return;
    }
    const generation = ++listGeneration.current;
    try {
      const result = await invoke<LibraryEntry[]>("library_list", {
        query,
        favorites,
      });
      if (generation === listGeneration.current) setEntries(result);
    } catch (reason) {
      report(reason);
    } finally {
      if (generation === listGeneration.current) setLoading(false);
    }
  }, [query, favorites, report]);
  const reloadRef = useRef(reload);
  reloadRef.current = reload;
  useEffect(() => {
    const timer = setTimeout(() => void reload(), 100);
    return () => clearTimeout(timer);
  }, [reload]);

  const updatePreferences = useCallback(
    async (patch: Partial<Preferences>) => {
      const value = { ...latest.current.preferences, ...patch };
      if (value.low >= value.high) {
        setError("The lowest key must be below the highest key.");
        return;
      }
      try {
        const saved = await invoke<Preferences>("preferences", { value });
        latest.current.preferences = saved;
        setPreferences(saved);
      } catch (reason) {
        report(reason);
      }
    },
    [report],
  );
  const applyPreferencesRef = useRef(updatePreferences);
  applyPreferencesRef.current = updatePreferences;
  const refreshDevices = useCallback(async () => {
    try {
      setDeviceList(await invoke<Device[]>("devices"));
    } catch (reason) {
      report(reason);
    }
  }, [report]);

  const importPaths = useCallback((paths: string[]) => {
    if (!paths.length) return;
    const capacity = Math.max(0, 64 - pendingImports.current);
    const newJobs: ImportJob[] = paths
      .slice(0, capacity)
      .map((path) => ({ key: ++jobId.current, path, state: "queued" }));
    if (paths.length > capacity)
      setError(
        `The import queue has room for ${capacity} more files (64 maximum). Import the remaining files when this batch finishes.`,
      );
    if (!newJobs.length) return;
    pendingImports.current += newJobs.length;
    setJobs((existing) => [...newJobs, ...existing].slice(0, 150));
    const work = async () => {
      setBusy(true);
      for (const job of newJobs) {
        setJobs((existing) =>
          existing.map((item) =>
            item.key === job.key ? { ...item, state: "importing" } : item,
          ),
        );
        try {
          const outcomes = await invoke<ImportOutcome[]>("import_files", {
            paths: [job.path],
          });
          const outcome = outcomes[0];
          setJobs((existing) =>
            existing.map((item) =>
              item.key === job.key
                ? {
                    ...item,
                    state: outcome?.result ? "ready" : "failed",
                    result: outcome?.result,
                    error:
                      outcome?.error ??
                      (outcome?.result
                        ? undefined
                        : "Import returned no playable entry."),
                  }
                : item,
            ),
          );
          if (outcome?.result) await reloadRef.current();
        } catch (reason) {
          setJobs((existing) =>
            existing.map((item) =>
              item.key === job.key
                ? { ...item, state: "failed", error: messageOf(reason) }
                : item,
            ),
          );
        }
      }
      pendingImports.current -= newJobs.length;
      setBusy(pendingImports.current > 0);
    };
    importQueue.current = importQueue.current.then(work, work);
  }, []);
  const importRef = useRef(importPaths);
  importRef.current = importPaths;
  const importDialog = () => {
    setBusy(true);
    const work = async () => {
      try {
        const outcomes = await invoke<ImportOutcome[]>("import_dialog");
        const newJobs: ImportJob[] = outcomes.map((outcome) => ({
          key: ++jobId.current,
          path: outcome.path,
          state: outcome.result ? "ready" : "failed",
          result: outcome.result,
          error: outcome.error,
        }));
        setJobs((existing) => [...newJobs, ...existing].slice(0, 150));
        await reloadRef.current();
      } catch (reason) {
        report(reason);
      } finally {
        setBusy(pendingImports.current > 0);
      }
    };
    importQueue.current = importQueue.current.then(work, work);
    return importQueue.current;
  };
  const importDemo = () => {
    setBusy(true);
    const work = async () => {
      try {
        const result = await invoke<ImportResult>("demo_import");
        setJobs((existing) => [
          {
            key: ++jobId.current,
            path: "Original Keyloom exercise",
            state: "ready",
            result,
          },
          ...existing,
        ]);
        await reloadRef.current();
        await openSong(result.entry.id);
      } catch (reason) {
        report(reason);
      } finally {
        setBusy(pendingImports.current > 0);
      }
    };
    importQueue.current = importQueue.current.then(work, work);
    return importQueue.current;
  };

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const addCleanup = (cleanup: () => void) =>
      disposed ? cleanup() : cleanups.push(cleanup);
    void invoke<Preferences>("preferences")
      .then((value) => {
        if (!disposed) {
          latest.current.preferences = value;
          setPreferences(value);
        }
      })
      .catch(report);
    void refreshDevices();
    const consume = async () => {
      try {
        const paths = await invoke<string[]>("take_pending_files");
        // Consuming is destructive; even a development StrictMode effect restart
        // must deliver files already taken from the native pending queue.
        importRef.current(paths);
      } catch (reason) {
        report(reason);
      }
    };
    void listen("files-opened", consume).then(addCleanup).catch(report);
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over")
          setDragging(true);
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          importRef.current(event.payload.paths);
        }
      })
      .then(addCleanup)
      .catch(report);
    void consume();
    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [refreshDevices, report]);

  useEffect(() => {
    if (!native) return;
    let disposed = false,
      lastSequence = -1;
    let inFlight = false;
    let fastUntil = 0;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (inFlight || disposed) return;
      inFlight = true;
      const start = performance.now();
      if (!document.hidden) {
        try {
          const value = await invoke<AppStatus>("status");
          if (disposed) return;
          latestSnapshot.current = value;
          latest.current.status = value;
          setStatus((previous) => {
            const sameDevice =
              previous.device_id === value.device_id &&
              previous.device_connected === value.device_connected &&
              previous.device_error === value.device_error &&
              previous.audio_available === value.audio_available &&
              previous.audio_error === value.audio_error &&
              previous.dropped_events === value.dropped_events;
            const sameKeys =
              latest.current.view === "library" ||
              previous.held.join(",") === value.held.join(",");
            const sameTransport =
              latest.current.view !== "practice" ||
              JSON.stringify(previous.transport) ===
                JSON.stringify(value.transport);
            return sameDevice && sameKeys && sameTransport ? previous : value;
          });
          const incoming = value.events.filter(
            (event) => event.sequence > lastSequence,
          );
          if (incoming.length) {
            lastSequence = Math.max(
              lastSequence,
              ...incoming.map((event) => event.sequence),
            );
            eventHistory.current = [
              ...incoming.reverse(),
              ...eventHistory.current,
            ].slice(0, 100);
            if (latest.current.view === "devices")
              setEvents(eventHistory.current);
            const target = latest.current.calibration;
            const note = incoming.find(
              (event) =>
                event.source.toLowerCase() === "physical" &&
                ["note_on", "noteon", "on", "note on"].includes(
                  event.kind.toLowerCase(),
                ) &&
                event.velocity > 0,
            );
            if (target && note) {
              setCalibration(null);
              latest.current.calibration = null;
              void applyPreferencesRef.current({ [target]: note.pitch });
            }
          }
        } catch (reason) {
          if (!disposed) report(reason);
        }
      }
      inFlight = false;
      const active = latest.current;
      const needsLiveInput =
        active.view === "devices" ||
        active.calibration !== null ||
        (active.view === "practice" &&
          (active.status.transport.playing ||
            active.status.transport.waiting ||
            active.status.device_connected ||
            active.preferences.simulated));
      const interval =
        needsLiveInput || performance.now() < fastUntil ? 33 : 250;
      if (!disposed)
        timer = setTimeout(
          poll,
          document.hidden
            ? 250
            : Math.max(5, interval - (performance.now() - start)),
        );
    };
    const wake = () => {
      // A short burst makes Play/seek/navigation responsive without leaving an
      // idle Library or disconnected paused roll on a permanent 30 Hz IPC loop.
      fastUntil = performance.now() + 250;
      if (!inFlight && !disposed) {
        clearTimeout(timer);
        void poll();
      }
    };
    refreshStatusSoon.current = wake;
    document.addEventListener("visibilitychange", wake);
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", wake);
      refreshStatusSoon.current = () => {};
    };
  }, [report]);
  useEffect(() => {
    refreshStatusSoon.current();
  }, [view, calibration, preferences.simulated, preferences.device_id]);

  const flushSettings = useCallback(async () => {
    if (configTimer.current) {
      clearTimeout(configTimer.current);
      configTimer.current = null;
    }
    const value = configured.current;
    if (!value) return;
    configured.current = null;
    await invoke("save_settings", { id: value.id, settings: value.settings });
  }, []);
  const changeSettings = (patch: Partial<SongSettings>) => {
    const active = latest.current.current;
    if (!active) return;
    const settings = {
      ...active.settings,
      split_hand: active.settings.split_hand ?? "both",
      ...patch,
      position_ms: latest.current.status.transport.position_ms,
    };
    const next = { ...active, settings };
    latest.current.current = next;
    setCurrent(next);
    configured.current = { id: active.entry.id, settings };
    void invoke("control", { action: "configure", settings })
      .then(() => refreshStatusSoon.current())
      .catch(report);
    if (configTimer.current) clearTimeout(configTimer.current);
    configTimer.current = setTimeout(
      () => void flushSettings().catch(report),
      400,
    );
  };
  const control = async (action: string, position_ms?: number) => {
    try {
      await flushSettings();
      await invoke("control", { action, position_ms });
      refreshStatusSoon.current();
    } catch (reason) {
      report(reason);
    }
  };
  const openSong = (id: string) => {
    const generation = ++openGeneration.current;
    const work = async () => {
      try {
        await flushSettings();
        if (generation !== openGeneration.current) return;
        const result = await invoke<OpenSong>("open_song", { id });
        if (generation !== openGeneration.current) return;
        latest.current.current = result;
        setCurrent(result);
        setView("practice");
        setLoopDraft(null);
        setSeekValue(null);
        await reloadRef.current();
      } catch (reason) {
        report(reason);
      }
    };
    openQueue.current = openQueue.current.then(work, work);
    return openQueue.current;
  };
  const inputNote = useCallback(
    (pitch: number, on: boolean) => {
      if (!native || !latest.current.preferences.simulated) return;
      void invoke("input_note", { pitch, on, velocity: on ? 90 : 0 }).catch(
        report,
      );
    },
    [report],
  );
  useEffect(() => {
    const down = new Map<string, number>();
    const keys = "awsedftgyhujkolp;";
    const keyDown = (event: KeyboardEvent) => {
      if (
        event.repeat ||
        event.ctrlKey ||
        event.altKey ||
        event.metaKey ||
        (event.target instanceof HTMLElement &&
          event.target.closest(
            "input, select, textarea, button, [contenteditable]",
          ))
      )
        return;
      if (event.code === "Space" && latest.current.current) {
        event.preventDefault();
        void invoke("control", {
          action: latest.current.status.transport.playing ? "pause" : "play",
        })
          .then(() => refreshStatusSoon.current())
          .catch(report);
        return;
      }
      const index = keys.indexOf(event.key.toLowerCase());
      if (!latest.current.preferences.simulated || index < 0) return;
      event.preventDefault();
      const pitch = 60 + index;
      down.set(event.code, pitch);
      inputNote(pitch, true);
    };
    const keyUp = (event: KeyboardEvent) => {
      const pitch = down.get(event.code);
      if (pitch !== undefined) {
        inputNote(pitch, false);
        down.delete(event.code);
      }
    };
    const release = () => {
      for (const pitch of down.values()) inputNote(pitch, false);
      down.clear();
    };
    window.addEventListener("keydown", keyDown);
    window.addEventListener("keyup", keyUp);
    window.addEventListener("blur", release);
    return () => {
      window.removeEventListener("keydown", keyDown);
      window.removeEventListener("keyup", keyUp);
      window.removeEventListener("blur", release);
      release();
    };
  }, [inputNote, report]);

  const saveMetadata = async (entry: LibraryEntry) => {
    try {
      await invoke("save_metadata", { entry });
      if (current?.entry.id === entry.id)
        setCurrent((active) => (active ? { ...active, entry } : null));
      setMetadata(null);
      await reloadRef.current();
    } catch (reason) {
      report(reason);
    }
  };
  const removeSong = async () => {
    if (!removing) return;
    try {
      await flushSettings();
      await invoke("remove_song", { id: removing.id, confirmed: true });
      if (current?.entry.id === removing.id) {
        setCurrent(null);
        setView("library");
      }
      setRemoving(null);
      await reloadRef.current();
    } catch (reason) {
      report(reason);
    }
  };
  const connectDevice = async (id: string | null) => {
    try {
      await invoke("connect_device", { id });
      await updatePreferences({ device_id: id });
    } catch (reason) {
      report(reason);
    }
  };
  const toggleFullscreen = async () => {
    try {
      const win = getCurrentWindow();
      const value = !(await win.isFullscreen());
      await win.setFullscreen(value);
      setIsFullscreen(value);
    } catch (reason) {
      report(reason);
    }
  };
  const setLoop = () => {
    if (!current) return;
    if (loopDraft === null) {
      setLoopDraft(status.transport.position_ms);
      return;
    }
    const end = status.transport.position_ms;
    if (end <= loopDraft + 100) {
      setError(
        "Move the playhead at least 0.1 seconds after A to set the loop end.",
      );
      return;
    }
    changeSettings({ loop_start_ms: loopDraft, loop_end_ms: end });
    setLoopDraft(null);
  };
  const settings = current?.settings;
  const sortedEntries = useMemo(
    () =>
      [...entries].sort((a, b) =>
        sort === "played"
          ? (b.last_played ?? 0) - (a.last_played ?? 0)
          : sort === "title"
            ? a.title.localeCompare(b.title)
            : b.imported_at - a.imported_at,
      ),
    [entries, sort],
  );
  const readyJobs = jobs.filter((job) => job.state === "ready");
  const showJobs = jobs.length > 0;
  const outOfRange = useMemo(() => {
    if (!current) return 0;
    const parts = new Map(
      current.song.parts.map((part) => [
        part.id,
        partSetting(part, current.settings),
      ]),
    );
    const anySolo = current.settings.parts.some((item) => item.solo);
    return current.song.notes.filter((note) => {
      const config = parts.get(note.part_id);
      return (
        config?.role === "practice" &&
        (!anySolo || config.solo) &&
        !isSplitAccompaniment(current.settings, note.pitch) &&
        (note.pitch < preferences.low || note.pitch > preferences.high)
      );
    }).length;
  }, [current, preferences.low, preferences.high]);
  const currentTargets = status.transport.targets.filter(
    (pitch) => !status.transport.matched.includes(pitch),
  );
  const hiddenImports = readyJobs.some(
    (job) => !entries.some((entry) => entry.id === job.result?.entry.id),
  );

  return (
    <div className={`app-shell ${isFullscreen ? "fullscreen" : ""}`}>
      <aside className="sidebar" inert={!!metadata || !!removing}>
        <div className="brand">
          <span className="brand-mark">
            <i />
            <i />
            <i />
            <i />
          </span>
          <span>
            keyloom
            <span className="brand-caption">A little practice. Every day.</span>
          </span>
        </div>
        <div className="nav-label">YOUR SPACE</div>
        <nav aria-label="Main navigation">
          <button
            className={view === "library" ? "nav-item active" : "nav-item"}
            onClick={() => setView("library")}
          >
            <Icon name="library" /> Library
          </button>
          <button
            className={view === "practice" ? "nav-item active" : "nav-item"}
            onClick={() => setView("practice")}
          >
            <Icon name="practice" /> Practice{" "}
            {current && <span className="nav-dot" />}
          </button>
          <button
            className={view === "devices" ? "nav-item active" : "nav-item"}
            onClick={() => setView("devices")}
          >
            <Icon name="devices" /> Keyboard & sound
          </button>
        </nav>
        <button
          className="import-sidebar"
          onClick={() => void importDialog()}
          disabled={!native || busy}
        >
          <Icon name="plus" /> {busy ? "Import in progress…" : "Import MIDI"}
        </button>
        <p className="sidebar-note">
          Your songs, on your computer.
          <br />
          No account. No waiting to restart.
        </p>
        <div className="sidebar-bottom">
          <button className="device-summary" onClick={() => setView("devices")}>
            <span
              className={`connection-dot ${status.device_connected ? "connected" : ""}`}
            />
            <span>
              {status.device_connected
                ? status.device_name
                : "Connect your keyboard"}
              <small>
                {status.device_connected
                  ? "Native MIDI input connected"
                  : "USB MIDI · setup & monitoring"}
              </small>
            </span>
            <Icon name="arrow" size={15} />
          </button>
          <div className="sidebar-footer">
            <span>KEYLOOM · LOCAL FIRST</span>
            <button
              onClick={() => void control("panic")}
              disabled={!native}
              title="Release all software voices and held inputs"
            >
              Panic
            </button>
          </div>
        </div>
      </aside>
      <main inert={!!metadata || !!removing}>
        {!native && (
          <div className="preview-banner">
            Disconnected browser preview — imports, audio, and MIDI require the
            Windows application.
          </div>
        )}
        {error && (
          <div role="alert" className="alert error">
            <span>{error}</span>
            <button
              className="icon-button"
              onClick={() => setError("")}
              aria-label="Dismiss error"
            >
              <Icon name="close" size={16} />
            </button>
          </div>
        )}
        {notice && (
          <div role="status" className="alert">
            <span>{notice}</span>
            <button
              className="icon-button"
              onClick={() => setNotice("")}
              aria-label="Dismiss notification"
            >
              <Icon name="close" size={16} />
            </button>
          </div>
        )}
        {status.device_error && (
          <div role="status" className="inline-warning device-error">
            {status.device_error}{" "}
            <button className="text-button" onClick={() => setView("devices")}>
              Keyboard settings
            </button>
          </div>
        )}
        {view !== "library" && showJobs && (
          <div className="alert import-banner" role="status">
            <span>
              {jobs.some(
                (job) => job.state === "queued" || job.state === "importing",
              )
                ? "Adding songs to your library…"
                : readyJobs[0]
                  ? `${readyJobs[0].result!.entry.title} · Ready in your library`
                  : "An import needs attention."}
            </span>
            <div className="button-row">
              {readyJobs[0] && (
                <button
                  className="small-button"
                  onClick={() => void openSong(readyJobs[0].result!.entry.id)}
                >
                  Open <Icon name="arrow" size={14} />
                </button>
              )}
              <button
                className="small-button"
                onClick={() => setView("library")}
              >
                View imports
              </button>
              <button
                className="icon-button"
                onClick={() =>
                  setJobs((old) =>
                    old.filter(
                      (job) =>
                        job.state === "queued" || job.state === "importing",
                    ),
                  )
                }
                aria-label="Dismiss completed imports"
              >
                <Icon name="close" size={16} />
              </button>
            </div>
          </div>
        )}
        {view === "library" && (
          <section className="library-view">
            <header className="page-header">
              <div>
                <div className="eyebrow">MAKE ROOM FOR MUSIC</div>
                <h1>
                  Your library<span className="title-dot">.</span>
                </h1>
                <p>Find a melody. Take it one note at a time.</p>
              </div>
              <button
                className="button primary"
                onClick={() => void importDialog()}
                disabled={!native || busy}
              >
                <Icon name="plus" /> Import MIDI
              </button>
            </header>
            <div className="library-intro">
              <div>
                <span className="pill">READY WHEN YOU ARE</span>
                <h2>
                  A song you love.
                  <br />A pace that’s yours.
                </h2>
                <p>
                  Drop an easy MIDI here, choose your hands,
                  <br className="wide-break" /> and start playing on your own
                  keyboard.
                </p>
                <button
                  className="text-button"
                  onClick={() => void importDemo()}
                  disabled={!native || busy}
                >
                  Try the original Keyloom exercise{" "}
                  <Icon name="arrow" size={17} />
                </button>
              </div>
              <div className="intro-art" aria-hidden="true">
                <div className="art-ruler" />
                <div className="art-notes">
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                </div>
                <div className="art-hit" />
                <div className="art-keys">
                  {Array.from({ length: 11 }, (_, index) => (
                    <i key={index} />
                  ))}
                </div>
                <span>FIND YOUR FLOW</span>
              </div>
            </div>
            {showJobs && (
              <ImportPanel
                jobs={jobs}
                entries={entries}
                hiddenImports={hiddenImports}
                onOpen={(id) => void openSong(id)}
                onRetry={(path) => importPaths([path])}
                onClear={() =>
                  setJobs((old) =>
                    old.filter(
                      (job) =>
                        job.state === "queued" || job.state === "importing",
                    ),
                  )
                }
              />
            )}
            <div className="library-toolbar">
              <div className="filter-tabs">
                <button
                  className={!favorites ? "selected" : ""}
                  onClick={() => setFavorites(false)}
                >
                  All songs
                </button>
                <button
                  className={favorites ? "selected" : ""}
                  onClick={() => setFavorites(true)}
                >
                  <Icon name="star" size={15} /> Favorites
                </button>
              </div>
              <div className="search-and-sort">
                <label className="search">
                  <Icon name="search" size={17} />
                  <input
                    aria-label="Search library"
                    placeholder="Search title, artist, or tags"
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                  />
                  {query && (
                    <button
                      className="icon-button"
                      aria-label="Clear search"
                      onClick={() => setQuery("")}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  )}
                </label>
                <select
                  aria-label="Sort library"
                  value={sort}
                  onChange={(event) => setSort(event.target.value)}
                >
                  <option value="imported">Recently imported</option>
                  <option value="played">Recently played</option>
                  <option value="title">Title A–Z</option>
                </select>
              </div>
            </div>
            <div className="section-heading">
              <h2>
                {favorites
                  ? "Your favorites"
                  : sort === "played"
                    ? "Recently played"
                    : sort === "title"
                      ? "All arrangements"
                      : "Recently imported"}
              </h2>
              <span>
                {entries.length}{" "}
                {entries.length === 1 ? "arrangement" : "arrangements"}
              </span>
            </div>
            {loading ? (
              <div className="empty-state">
                <span className="spinner" /> Opening your library…
              </div>
            ) : !entries.length ? (
              <div className="empty-state">
                <div className="empty-icon">
                  <Icon name="library" size={30} />
                </div>
                <h3>
                  {query || favorites
                    ? "No songs match this view"
                    : "Your next favorite melody starts here"}
                </h3>
                <p>
                  {query || favorites
                    ? "Clear the search or favorites filter. New imports also have a direct Open button above."
                    : "Import a .mid or .midi file. We keep a managed copy, so the original stays yours."}
                </p>
                <button
                  className="button secondary"
                  onClick={() =>
                    query || favorites
                      ? (setQuery(""), setFavorites(false))
                      : void importDialog()
                  }
                  disabled={!native}
                >
                  {query || favorites ? "Show all songs" : "Choose a MIDI file"}
                </button>
              </div>
            ) : (
              <div className="song-list">
                <div className="song-list-head">
                  <span>ARRANGEMENT</span>
                  <span>DETAILS</span>
                  <span>ADDED</span>
                  <span />
                </div>
                {sortedEntries.map((entry, index) => (
                  <article
                    className="song-row"
                    key={entry.id}
                    data-song-id={entry.id}
                  >
                    <div className="song-main">
                      <button
                        className={`song-art art-${index % 4}`}
                        onClick={() => void openSong(entry.id)}
                        aria-label={`Open ${entry.title}`}
                      >
                        <Icon name="practice" size={25} />
                      </button>
                      <div>
                        <button
                          className="song-title"
                          onClick={() => void openSong(entry.id)}
                        >
                          {entry.title}
                        </button>
                        <p>
                          {entry.artist || "Artist not set"}
                          {entry.arrangement && <> · {entry.arrangement}</>}
                        </p>
                        {entry.tags && (
                          <span className="song-tags">{entry.tags}</span>
                        )}
                      </div>
                    </div>
                    <div className="song-details">
                      <strong>{timeLabel(entry.duration_ms)}</strong>
                      <span>
                        {entry.parts_count}{" "}
                        {entry.parts_count === 1 ? "part" : "parts"} ·{" "}
                        {entry.note_count.toLocaleString()} notes
                      </span>
                      {sort === "played" && (
                        <small>
                          {entry.last_played
                            ? `Played ${dateLabel(entry.last_played)}`
                            : "Not played yet"}
                        </small>
                      )}
                    </div>
                    <span className="song-date">
                      {dateLabel(entry.imported_at)}
                    </span>
                    <div className="song-actions">
                      <button
                        className={`icon-button favorite ${entry.favorite ? "is-favorite" : ""}`}
                        title={
                          entry.favorite ? "Remove favorite" : "Add favorite"
                        }
                        aria-label={`${entry.favorite ? "Unfavorite" : "Favorite"} ${entry.title}`}
                        onClick={() =>
                          void saveMetadata({
                            ...entry,
                            favorite: !entry.favorite,
                          })
                        }
                      >
                        <Icon name="star" size={19} />
                      </button>
                      <button
                        className="small-button"
                        onClick={() => setMetadata(entry)}
                      >
                        Edit
                      </button>
                      <button
                        className="button song-open"
                        onClick={() => void openSong(entry.id)}
                      >
                        Practice <Icon name="arrow" size={16} />
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
            <footer className="library-footnote">
              <span className="tiny-dot" /> Private by design. Imported files
              are copied into your local managed library.
            </footer>
          </section>
        )}

        {view === "practice" &&
          (!current || !settings ? (
            <section className="no-practice">
              <div className="empty-icon">
                <Icon name="practice" size={34} />
              </div>
              <div className="eyebrow">LET’S PLAY SOMETHING</div>
              <h1>One song. One small step.</h1>
              <p>
                Open an arrangement from your library or try an original
                exercise.
              </p>
              <div className="button-row">
                <button
                  className="button primary"
                  onClick={() => setView("library")}
                >
                  Open library
                </button>
                <button
                  className="button secondary"
                  onClick={() => void importDemo()}
                  disabled={!native || busy}
                >
                  Try an exercise
                </button>
              </div>
            </section>
          ) : (
            <section className="practice-view" data-song-id={current.entry.id}>
              <header className="practice-header">
                <div>
                  <button
                    className="breadcrumb"
                    onClick={() => setView("library")}
                  >
                    Library / Practice
                  </button>
                  <h1>{current.entry.title}</h1>
                  <p>
                    {current.entry.artist || "Your MIDI arrangement"}
                    {current.entry.arrangement
                      ? ` · ${current.entry.arrangement}`
                      : ""}
                  </p>
                </div>
                <div className="button-row">
                  <button
                    className="small-button"
                    onClick={() => setShowParts((value) => !value)}
                  >
                    {showParts ? "Hide parts" : "Show parts"}
                  </button>
                  <button
                    className="icon-button"
                    onClick={() => void toggleFullscreen()}
                    title={isFullscreen ? "Leave full screen" : "Full screen"}
                    aria-label="Toggle full screen"
                  >
                    <Icon name="expand" />
                  </button>
                </div>
              </header>
              <div className="practice-options">
                <div className="mode-switch" aria-label="Practice mode">
                  {[
                    ["listen", "Listen"],
                    ["wait", "Wait for me"],
                    ["timed", "Timed"],
                  ].map(([mode, label]) => (
                    <button
                      key={mode}
                      className={settings.mode === mode ? "selected" : ""}
                      onClick={() => changeSettings({ mode })}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <label className="speed-control">
                  <span>
                    Tempo <strong>{Math.round(settings.speed * 100)}%</strong>
                  </span>
                  <input
                    aria-label="Playback speed"
                    type="range"
                    min="25"
                    max="150"
                    step="5"
                    value={Math.round(settings.speed * 100)}
                    onChange={(event) =>
                      changeSettings({
                        speed: Number(event.target.value) / 100,
                      })
                    }
                  />
                </label>
                <span className="practice-device">
                  <span
                    className={`connection-dot ${status.device_connected ? "connected" : ""}`}
                  />
                  {status.device_connected
                    ? status.device_name
                    : preferences.simulated
                      ? "Simulated input"
                      : "No MIDI input"}
                </span>
              </div>
              {outOfRange > 0 && (
                <div className="inline-warning">
                  {outOfRange} practice notes fall outside{" "}
                  {noteName(preferences.low)}–{noteName(preferences.high)}.{" "}
                  <button
                    className="text-button"
                    onClick={() => setView("devices")}
                  >
                    Adjust keyboard range
                  </button>
                </div>
              )}
              {!status.device_connected &&
                !preferences.simulated &&
                settings.mode !== "listen" && (
                  <div className="inline-warning">
                    Connect a MIDI input in Keyboard & sound, or explicitly
                    enable simulated input for a test.
                  </div>
                )}
              {preferences.simulated && (
                <div className="simulation-notice">
                  SIMULATED INPUT ENABLED · Click keys or use A W S E D F T G Y
                  H U J K O L P ; (C4–E5). These results do not verify your
                  Yamaha.
                </div>
              )}
              <div className="practice-body">
                <div className="roll-panel">
                  <div className="roll-topline">
                    <div className="legend">
                      <span>
                        <i className="right" />
                        Right / unassigned
                      </span>
                      <span>
                        <i className="left" />
                        Left
                      </span>
                      <span>
                        <i className="accompaniment" />
                        Accompaniment
                      </span>
                    </div>
                    <span>
                      {noteName(preferences.low)}–{noteName(preferences.high)}
                    </span>
                  </div>
                  <PianoRoll
                    song={current.song}
                    settings={settings}
                    status={status}
                    preferences={preferences}
                    onNote={inputNote}
                  />
                  <div
                    className={`practice-feedback ${status.transport.waiting ? "waiting" : ""}`}
                    aria-live="polite"
                  >
                    <span>
                      {status.transport.count_in_remaining > 0
                        ? `Count in · ${status.transport.count_in_remaining}`
                        : status.transport.waiting
                          ? `Your turn · ${currentTargets.map((pitch) => noteName(pitch, preferences.note_numbers)).join(" + ") || "release and press the next notes"}`
                          : status.transport.feedback ||
                            (status.transport.playing
                              ? "Follow the notes to the light line"
                              : "Ready at your own pace")}
                    </span>
                    <span>
                      {settings.mode !== "listen" && (
                        <>
                          {status.transport.correct} matched ·{" "}
                          {status.transport.wrong} extra ·{" "}
                          {status.transport.missed} missed
                        </>
                      )}
                    </span>
                  </div>
                </div>
                {showParts && (
                  <aside className="parts-panel">
                    <div className="section-heading">
                      <h2>Make it your own</h2>
                      <span>{current.song.parts.length} parts</span>
                    </div>
                    <p className="help-text">
                      Choose what to practice. Gray accompaniment plays along.
                    </p>
                    {current.song.parts.map((part) => {
                      const config = partSetting(part, settings);
                      const update = (patch: Partial<PartSetting>) =>
                        changeSettings({
                          parts: [
                            ...settings.parts.filter(
                              (item) => item.part_id !== part.id,
                            ),
                            { ...config, ...patch },
                          ],
                        });
                      return (
                        <div className="part-card" key={part.id}>
                          <div className="part-title">
                            <span>
                              {part.name || `Track ${part.track + 1}`}
                            </span>
                            <small>
                              CH {part.channel + 1}
                              {part.percussion ? " · DRUMS" : ""}
                            </small>
                          </div>
                          <select
                            aria-label={`Role for ${part.name}`}
                            value={config.role}
                            onChange={(event) =>
                              update({ role: event.target.value })
                            }
                          >
                            <option value="practice" disabled={part.percussion}>
                              Practice
                            </option>
                            <option value="accompaniment">Accompaniment</option>
                            <option value="muted">Muted</option>
                          </select>
                          <div className="part-controls">
                            <select
                              aria-label={`Hand for ${part.name}`}
                              value={config.hand}
                              disabled={config.role !== "practice"}
                              onChange={(event) =>
                                update({ hand: event.target.value })
                              }
                            >
                              <option value="auto">Unassigned</option>
                              <option value="left">Left hand</option>
                              <option value="right">Right hand</option>
                            </select>
                            <button
                              className={`solo-button ${config.solo ? "selected" : ""}`}
                              aria-pressed={config.solo}
                              onClick={() => update({ solo: !config.solo })}
                            >
                              Solo
                            </button>
                          </div>
                          <label className="volume">
                            <span>Volume</span>
                            <input
                              type="range"
                              min="0"
                              max="100"
                              value={Math.round(config.volume * 100)}
                              aria-label={`Volume for ${part.name}`}
                              onChange={(event) =>
                                update({
                                  volume: Number(event.target.value) / 100,
                                })
                              }
                            />
                            <small>{Math.round(config.volume * 100)}</small>
                          </label>
                        </div>
                      );
                    })}
                    <div className="split-settings">
                      <label className="check">
                        <input
                          type="checkbox"
                          checked={settings.split_pitch !== null}
                          onChange={(event) =>
                            changeSettings({
                              split_pitch: event.target.checked ? 60 : null,
                            })
                          }
                        />
                        Approximate pitch split
                      </label>
                      <p className="help-text">
                        In practice parts, notes below the boundary become left
                        hand; notes at or above it become right hand. This pitch
                        threshold is an approximation, not melody extraction.
                      </p>
                      {settings.split_pitch !== null && (
                        <>
                          <label className="compact-label">
                            Right hand begins at
                            <select
                              value={settings.split_pitch}
                              onChange={(event) =>
                                changeSettings({
                                  split_pitch: Number(event.target.value),
                                })
                              }
                            >
                              {Array.from({ length: 128 }, (_, pitch) => (
                                <option value={pitch} key={pitch}>
                                  {noteName(pitch)} · {pitch}
                                </option>
                              ))}
                            </select>
                          </label>
                          <label className="field-label">
                            Practice after split
                            <select
                              aria-label="Hands to practice after pitch split"
                              value={settings.split_hand ?? "both"}
                              onChange={(event) =>
                                changeSettings({
                                  split_hand: event.target.value,
                                })
                              }
                            >
                              <option value="both">Both hands</option>
                              <option value="left">Left only</option>
                              <option value="right">Right only</option>
                            </select>
                          </label>
                          <p className="help-text">
                            When practicing one side, the other side plays as
                            gray accompaniment.
                          </p>
                        </>
                      )}
                    </div>
                    <div className="parts-disclaimer">
                      Basic practice tone. Instrument programs and arbitrary
                      SysEx are not reproduced.
                    </div>
                  </aside>
                )}
              </div>
              <div className="transport">
                <div className="seek-line">
                  <span>
                    {timeLabel(seekValue ?? status.transport.position_ms)}
                  </span>
                  <input
                    type="range"
                    aria-label="Song position"
                    min="0"
                    max={Math.max(1, current.song.duration_ms)}
                    step="10"
                    value={seekValue ?? status.transport.position_ms}
                    onChange={(event) =>
                      setSeekValue(Number(event.target.value))
                    }
                    onPointerUp={(event) => {
                      const value = Number(event.currentTarget.value);
                      setSeekValue(null);
                      void control("seek", value);
                    }}
                    onKeyUp={(event) => {
                      const value = Number(event.currentTarget.value);
                      setSeekValue(null);
                      void control("seek", value);
                    }}
                  />
                  <span>{timeLabel(current.song.duration_ms)}</span>
                </div>
                <div className="transport-buttons">
                  <div className="button-row">
                    <button
                      className="icon-button"
                      title="Stop and return to beginning"
                      aria-label="Stop"
                      onClick={() => void control("stop")}
                    >
                      <Icon name="stop" size={17} />
                    </button>
                    <button
                      className="button primary play-button"
                      onClick={() =>
                        void control(
                          status.transport.playing ? "pause" : "play",
                        )
                      }
                    >
                      <Icon
                        name={status.transport.playing ? "pause" : "play"}
                        size={18}
                      />
                      {status.transport.playing ? "Pause" : "Play"}
                    </button>
                    <button
                      className="small-button"
                      onClick={() => void control("seek", 0)}
                    >
                      Restart
                    </button>
                  </div>
                  <div className="loop-controls">
                    <button
                      className={`small-button ${loopDraft !== null ? "selected" : ""}`}
                      onClick={setLoop}
                    >
                      {loopDraft !== null ? "Set B" : "Set A"}
                    </button>
                    <span>
                      {loopDraft !== null
                        ? `A ${timeLabel(loopDraft)} → choose B`
                        : settings.loop_start_ms !== null &&
                            settings.loop_end_ms !== null
                          ? `Loop ${timeLabel(settings.loop_start_ms)}–${timeLabel(settings.loop_end_ms)}`
                          : "Loop off"}
                    </span>
                    {(loopDraft !== null ||
                      settings.loop_start_ms !== null) && (
                      <button
                        className="icon-button"
                        title="Clear loop"
                        aria-label="Clear loop"
                        onClick={() => {
                          setLoopDraft(null);
                          changeSettings({
                            loop_start_ms: null,
                            loop_end_ms: null,
                          });
                        }}
                      >
                        <Icon name="close" size={14} />
                      </button>
                    )}
                  </div>
                  <div className="button-row transport-checks">
                    <label className="check">
                      <input
                        type="checkbox"
                        checked={settings.count_in}
                        onChange={(event) =>
                          changeSettings({ count_in: event.target.checked })
                        }
                      />
                      Count in
                    </label>
                    <label className="check">
                      <input
                        type="checkbox"
                        checked={settings.metronome}
                        onChange={(event) =>
                          changeSettings({ metronome: event.target.checked })
                        }
                      />
                      Metronome
                    </label>
                    <button
                      className="small-button"
                      onClick={() => void control("panic")}
                    >
                      Panic
                    </button>
                  </div>
                </div>
              </div>
              {!!current.song.warnings.length && (
                <details className="song-warnings">
                  <summary>
                    {current.song.warnings.length} MIDI import{" "}
                    {current.song.warnings.length === 1 ? "note" : "notes"}
                  </summary>
                  <ul>
                    {current.song.warnings.map((warning, index) => (
                      <li key={index}>{warning}</li>
                    ))}
                  </ul>
                </details>
              )}
            </section>
          ))}

        {view === "devices" && (
          <section className="devices-view">
            <header className="page-header">
              <div>
                <div className="eyebrow">YOUR INSTRUMENT, CONNECTED</div>
                <h1>
                  Keyboard & sound<span className="title-dot">.</span>
                </h1>
                <p>
                  Set your range, choose how you listen, and see every key
                  arrive.
                </p>
              </div>
            </header>
            <div className="settings-grid">
              <section className="settings-card">
                <div className="card-title">
                  <h2>MIDI input</h2>
                  <button
                    className="small-button"
                    onClick={() => void refreshDevices()}
                    disabled={!native}
                  >
                    <Icon name="refresh" size={15} /> Refresh devices
                  </button>
                </div>
                <label className="field-label">
                  Your USB MIDI interface
                  <select
                    aria-label="MIDI input device"
                    value={preferences.device_id ?? ""}
                    onChange={(event) =>
                      void connectDevice(event.target.value || null)
                    }
                    disabled={!native}
                  >
                    <option value="">No input selected</option>
                    {preferences.device_id &&
                      !deviceList.some(
                        (device) => device.id === preferences.device_id,
                      ) && (
                        <option value={preferences.device_id}>
                          Saved interface · currently unavailable
                        </option>
                      )}
                    {deviceList.map((device) => (
                      <option value={device.id} key={device.id}>
                        {device.name}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="device-state">
                  <span
                    className={`connection-dot ${status.device_connected ? "connected" : ""}`}
                  />
                  {status.device_connected
                    ? `Receiving from ${status.device_name}`
                    : "Waiting for a MIDI interface"}
                </div>
                <p className="help-text">
                  Choose the USB adapter that your Yamaha is plugged into.
                  Device selection is saved; Keyloom never silently chooses a
                  different port.
                </p>
                <button
                  className="small-button"
                  onClick={() =>
                    preferences.device_id &&
                    void connectDevice(preferences.device_id)
                  }
                  disabled={!native || !preferences.device_id}
                >
                  Reconnect saved interface
                </button>
              </section>
              <section className="settings-card">
                <h2>How do you want to listen?</h2>
                <label
                  className={`radio-card ${preferences.monitor === "yamaha" ? "selected" : ""}`}
                >
                  <input
                    type="radio"
                    name="monitor"
                    checked={preferences.monitor === "yamaha"}
                    onChange={() =>
                      void updatePreferences({ monitor: "yamaha" })
                    }
                    disabled={!native}
                  />
                  <span>
                    <strong>Through my Yamaha</strong>
                    <small>
                      Play with the keyboard’s own speakers. Input monitoring is
                      off.
                    </small>
                  </span>
                </label>
                <label
                  className={`radio-card ${preferences.monitor === "software" ? "selected" : ""}`}
                >
                  <input
                    type="radio"
                    name="monitor"
                    checked={preferences.monitor === "software"}
                    onChange={() =>
                      void updatePreferences({ monitor: "software" })
                    }
                    disabled={!native}
                  />
                  <span>
                    <strong>Through my computer</strong>
                    <small>
                      Hear a basic synthesized practice tone. Turn down the
                      Yamaha to avoid doubled notes.
                    </small>
                  </span>
                </label>
                <p className="audio-state">
                  <span
                    className={`connection-dot ${status.audio_available ? "connected" : ""}`}
                  />
                  {status.audio_available
                    ? "Native audio output available · basic practice tone"
                    : status.audio_error || "Audio output not available"}
                </p>
                <p className="help-text">
                  Arrangement playback uses PC audio. No SoundFont or sampled
                  piano is bundled. MIDI output and MIDI Thru are off.
                </p>
              </section>
              <section className="settings-card">
                <h2>Fit your keyboard</h2>
                <p className="help-text">
                  Choose the actual lowest and highest keys. We do not infer
                  your key count from the USB adapter.
                </p>
                <div className="range-fields">
                  {(["low", "high"] as const).map((endpoint) => (
                    <div key={endpoint}>
                      <label className="field-label">
                        {endpoint === "low" ? "Lowest key" : "Highest key"}
                        <select
                          aria-label={
                            endpoint === "low"
                              ? "Lowest keyboard key"
                              : "Highest keyboard key"
                          }
                          value={preferences[endpoint]}
                          onChange={(event) =>
                            void updatePreferences({
                              [endpoint]: Number(event.target.value),
                            })
                          }
                          disabled={!native}
                        >
                          {Array.from({ length: 128 }, (_, pitch) => (
                            <option value={pitch} key={pitch}>
                              {noteName(pitch)} · MIDI {pitch}
                            </option>
                          ))}
                        </select>
                      </label>
                      <button
                        className={`small-button ${calibration === endpoint ? "selected" : ""}`}
                        onClick={() =>
                          setCalibration(
                            calibration === endpoint ? null : endpoint,
                          )
                        }
                        disabled={!native || !status.device_connected}
                      >
                        {calibration === endpoint
                          ? "Press the physical key… Cancel"
                          : "Learn from keyboard"}
                      </button>
                    </div>
                  ))}
                </div>
                <div className="range-summary">
                  {preferences.high - preferences.low + 1} keys ·{" "}
                  {noteName(preferences.low)} to {noteName(preferences.high)}
                </div>
                <p className="help-text">
                  Range changes the view only. Notes are never transposed. C4 is
                  MIDI 60.
                </p>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={preferences.labels}
                    onChange={(event) =>
                      void updatePreferences({ labels: event.target.checked })
                    }
                    disabled={!native}
                  />
                  Show note labels
                </label>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={preferences.note_numbers}
                    onChange={(event) =>
                      void updatePreferences({
                        note_numbers: event.target.checked,
                      })
                    }
                    disabled={!native}
                  />
                  Use MIDI note numbers instead of octave names
                </label>
              </section>
              <section className="settings-card test-card">
                <div className="card-title">
                  <h2>Test input</h2>
                  <span className="pill">SIMULATED</span>
                </div>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={preferences.simulated}
                    onChange={(event) =>
                      void updatePreferences({
                        simulated: event.target.checked,
                      })
                    }
                    disabled={!native}
                  />
                  Enable simulated player input
                </label>
                <p className="help-text">
                  Click the on-screen piano or use computer keys A W S E D F T G
                  Y H U J K O L P ;. Simulated input can satisfy practice
                  targets, but it does not verify the physical keyboard.
                </p>
                <div className="test-keys">
                  {[60, 62, 64, 65, 67, 69, 71, 72].map((pitch) => (
                    <button
                      key={pitch}
                      disabled={!preferences.simulated || !native}
                      onPointerDown={(event) => {
                        event.currentTarget.setPointerCapture(event.pointerId);
                        inputNote(pitch, true);
                      }}
                      onPointerUp={() => inputNote(pitch, false)}
                      onPointerCancel={() => inputNote(pitch, false)}
                      onLostPointerCapture={() => inputNote(pitch, false)}
                      onKeyDown={(event) => {
                        if (
                          !event.repeat &&
                          (event.code === "Space" || event.code === "Enter")
                        ) {
                          event.preventDefault();
                          inputNote(pitch, true);
                        }
                      }}
                      onKeyUp={(event) => {
                        if (event.code === "Space" || event.code === "Enter")
                          inputNote(pitch, false);
                      }}
                    >
                      {noteName(pitch)}
                    </button>
                  ))}
                </div>
                <button
                  className="button secondary"
                  onClick={() => void control("panic")}
                  disabled={!native}
                >
                  Panic / release all notes
                </button>
              </section>
            </div>
            <section className="event-monitor settings-card">
              <div className="card-title">
                <div>
                  <h2>Live event monitor</h2>
                  <p className="help-text">
                    Native MIDI messages and explicitly simulated inputs. Latest
                    100 events.
                  </p>
                </div>
                <button
                  className="small-button"
                  onClick={() => {
                    eventHistory.current = [];
                    setEvents([]);
                  }}
                >
                  Clear display
                </button>
              </div>
              <div className="monitor-heading">
                <span>SOURCE</span>
                <span>EVENT</span>
                <span>CHANNEL</span>
                <span>NOTE / CC</span>
                <span>VELOCITY / VALUE</span>
              </div>
              <div className="monitor-scroll">
                {!events.length ? (
                  <p className="monitor-empty">
                    Press a key or pedal to see input here.
                  </p>
                ) : (
                  events.map((event) => (
                    <div
                      className={`monitor-row ${event.source.toLowerCase() === "simulated" ? "simulated" : ""}`}
                      key={event.sequence}
                    >
                      <span>{event.source}</span>
                      <span>{event.kind}</span>
                      <span>{event.channel + 1}</span>
                      <span>
                        {event.pitch}{" "}
                        {event.kind.toLowerCase().includes("note")
                          ? `· ${noteName(event.pitch)}`
                          : ""}
                      </span>
                      <span>{event.velocity}</span>
                    </div>
                  ))
                )}
              </div>
              {status.dropped_events > 0 && (
                <div className="inline-warning">
                  {status.dropped_events} events dropped from bounded
                  input/monitor queues.
                </div>
              )}
            </section>
          </section>
        )}
      </main>
      {dragging && (
        <div className="drop-overlay">
          <div>
            <Icon name="plus" size={38} />
            <h2>Drop your next melody</h2>
            <p>.mid and .midi · copied into your library, ready to play</p>
          </div>
        </div>
      )}
      {metadata && (
        <div className="modal-backdrop" onClick={() => setMetadata(null)}>
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="metadata-title"
            className="modal"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="card-title">
              <h2 id="metadata-title">Arrangement details</h2>
              <button
                className="icon-button"
                onClick={() => setMetadata(null)}
                aria-label="Close details"
              >
                <Icon name="close" />
              </button>
            </div>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void saveMetadata(metadata);
              }}
            >
              {(["title", "artist", "tags", "arrangement"] as const).map(
                (field) => (
                  <label className="field-label" key={field}>
                    {field === "tags"
                      ? "Source / anime / game tags"
                      : field === "arrangement"
                        ? "Arrangement label"
                        : field[0].toUpperCase() + field.slice(1)}
                    <input
                      autoFocus={field === "title"}
                      value={metadata[field]}
                      maxLength={field === "tags" ? 2000 : 500}
                      required={field === "title"}
                      onChange={(event) =>
                        setMetadata({
                          ...metadata,
                          [field]: event.target.value,
                        })
                      }
                      placeholder={
                        field === "arrangement"
                          ? "e.g. Easy piano · both hands"
                          : undefined
                      }
                    />
                  </label>
                ),
              )}
              <label className="check">
                <input
                  type="checkbox"
                  checked={metadata.favorite}
                  onChange={(event) =>
                    setMetadata({ ...metadata, favorite: event.target.checked })
                  }
                />
                Favorite this arrangement
              </label>
              <div className="modal-actions">
                <button
                  type="button"
                  className="small-button"
                  onClick={() => {
                    void invoke("export_song", { id: metadata.id }).catch(
                      report,
                    );
                  }}
                >
                  Export MIDI
                </button>
                <button
                  type="button"
                  className="text-danger"
                  onClick={() => {
                    setRemoving(metadata);
                    setMetadata(null);
                  }}
                >
                  Remove
                </button>
                <button type="submit" className="button primary">
                  Save details
                </button>
              </div>
            </form>
          </section>
        </div>
      )}
      {removing && (
        <div className="modal-backdrop">
          <section
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="remove-title"
            className="modal"
          >
            <h2 id="remove-title">Remove this managed copy?</h2>
            <p>
              “{removing.title}” and its saved practice settings will be removed
              from Keyloom’s library.
            </p>
            <p>
              Your original external MIDI file will not be changed or deleted.
            </p>
            <div className="modal-actions">
              <button
                className="button secondary"
                onClick={() => setRemoving(null)}
              >
                Keep song
              </button>
              <button
                className="button danger"
                onClick={() => void removeSong()}
              >
                Remove managed copy
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

function ImportPanel({
  jobs,
  entries,
  hiddenImports,
  onOpen,
  onRetry,
  onClear,
}: {
  jobs: ImportJob[];
  entries: LibraryEntry[];
  hiddenImports: boolean;
  onOpen: (id: string) => void;
  onRetry: (path: string) => void;
  onClear: () => void;
}) {
  return (
    <section className="import-results" aria-label="Import results">
      <div className="card-title">
        <h3>Import activity</h3>
        <button className="small-button" onClick={onClear}>
          Clear finished
        </button>
      </div>
      {hiddenImports && (
        <p className="help-text">
          A current filter may hide your import below. Open here to go straight
          to practice.
        </p>
      )}
      <div className="import-results-scroll">
        {jobs.map((job) => (
          <div
            className={`import-result ${job.state}`}
            key={job.key}
            data-song-id={job.result?.entry.id}
            data-import-state={job.state}
          >
            <span className="import-status">
              {job.state === "ready" ? (
                "✓"
              ) : job.state === "failed" ? (
                "!"
              ) : (
                <span className="spinner" />
              )}
            </span>
            <div>
              <strong>
                {job.result?.entry.title || job.path.split(/[\\/]/).pop()}
              </strong>
              <small>
                {job.state === "ready"
                  ? `${job.result?.duplicate ? "Already in your library" : "Ready to play"} · ${Math.round(job.result?.elapsed_ms ?? 0)} ms${entries.some((entry) => entry.id === job.result?.entry.id) ? "" : " · outside current view"}`
                  : job.state === "failed"
                    ? job.error
                    : job.state === "importing"
                      ? "Validating and creating your managed copy…"
                      : "Queued"}
              </small>
            </div>
            {job.result && (
              <button
                className="small-button"
                onClick={() => onOpen(job.result!.entry.id)}
              >
                Open <Icon name="arrow" size={14} />
              </button>
            )}
            {job.state === "failed" && (
              <button
                className="small-button"
                onClick={() => onRetry(job.path)}
              >
                Retry
              </button>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
