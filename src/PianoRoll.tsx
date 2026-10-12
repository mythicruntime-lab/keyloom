import { useEffect, useMemo, useRef } from "react";
import {
  AppStatus,
  Note,
  Preferences,
  Song,
  SongSettings,
  noteName,
  partSetting,
} from "./types";

interface Props {
  song: Song;
  settings: SongSettings;
  status: AppStatus;
  preferences: Preferences;
  onNote: (pitch: number, on: boolean) => void;
}
interface Key {
  pitch: number;
  x: number;
  width: number;
  black: boolean;
}
const isBlack = (pitch: number) => [1, 3, 6, 8, 10].includes(pitch % 12);
function keyboard(low: number, high: number, width: number): Key[] {
  let whites = 0;
  const all: Key[] = [];
  for (let pitch = 0; pitch <= 127; pitch++) {
    const black = isBlack(pitch);
    all.push({
      pitch,
      x: black ? whites - 0.3 : whites,
      width: black ? 0.6 : 1,
      black,
    });
    if (!black) whites++;
  }
  const keys = all.filter((k) => k.pitch >= low && k.pitch <= high);
  const left = Math.min(...keys.map((k) => k.x));
  const right = Math.max(...keys.map((k) => k.x + k.width));
  return keys.map((k) => ({
    ...k,
    x: ((k.x - left) * width) / (right - left),
    width: (k.width * width) / (right - left),
  }));
}

// The max-end tree skips offscreen note intervals, including long sustained notes.
// Musical time comes only from Rust; interpolation is display-only and bounded.
function noteIndex(notes: Note[]) {
  const sorted = [...notes].sort((a, b) => a.start_ms - b.start_ms);
  const maxEnd = new Float64Array(Math.max(4, sorted.length * 4));
  const build = (node: number, low: number, high: number): number => {
    if (low === high) return (maxEnd[node] = sorted[low].end_ms);
    const mid = (low + high) >>> 1;
    return (maxEnd[node] = Math.max(
      build(node * 2, low, mid),
      build(node * 2 + 1, mid + 1, high),
    ));
  };
  if (sorted.length) build(1, 0, sorted.length - 1);
  return (from: number, to: number, draw: (note: Note) => void) => {
    const visit = (node: number, low: number, high: number) => {
      if (sorted[low].start_ms > to || maxEnd[node] < from) return;
      if (low === high) {
        draw(sorted[low]);
        return;
      }
      const mid = (low + high) >>> 1;
      visit(node * 2, low, mid);
      visit(node * 2 + 1, mid + 1, high);
    };
    if (sorted.length) visit(1, 0, sorted.length - 1);
  };
}

export default function PianoRoll(props: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const current = useRef({ ...props, received: performance.now() });
  const geometry = useRef<{ keys: Key[]; hit: number; height: number }>({
    keys: [],
    hit: 0,
    height: 0,
  });
  const pressed = useRef(new Map<number, number>());
  const requestDraw = useRef<(() => void) | null>(null);
  const visibleNotes = useMemo(() => noteIndex(props.song.notes), [props.song]);
  current.current = {
    ...props,
    received:
      current.current.status === props.status
        ? current.current.received
        : performance.now(),
  };

  useEffect(() => {
    const element = canvas.current!;
    const context = element.getContext("2d", { alpha: false });
    if (!context) return;
    let frame = 0;
    let dirty = true;
    let observed: AppStatus | null = null;
    let observedPrefs: Preferences | null = null;
    let observedSettings: SongSettings | null = null;
    const resize = new ResizeObserver(() => {
      dirty = true;
      requestDraw.current?.();
    });
    resize.observe(element);
    const draw = () => {
      frame = 0;
      const { song, settings, status, preferences, received } = current.current;
      const { transport } = status;
      const animate = transport.playing && !transport.waiting;
      if (
        !document.hidden &&
        (dirty ||
          animate ||
          observed !== status ||
          observedPrefs !== preferences ||
          observedSettings !== settings)
      ) {
        dirty = false;
        observed = status;
        observedPrefs = preferences;
        observedSettings = settings;
        const rect = element.getBoundingClientRect();
        const width = rect.width,
          height = rect.height;
        const dpr = Math.min(window.devicePixelRatio || 1, 2.5);
        if (
          element.width !== Math.round(width * dpr) ||
          element.height !== Math.round(height * dpr)
        ) {
          element.width = Math.round(width * dpr);
          element.height = Math.round(height * dpr);
        }
        context.setTransform(dpr, 0, 0, dpr, 0, 0);
        const keyboardHeight = Math.min(115, height * 0.25);
        const hit = height - keyboardHeight;
        const keys = keyboard(preferences.low, preferences.high, width);
        geometry.current = { keys, hit, height };
        const keyMap = new Map(keys.map((k) => [k.pitch, k]));
        const elapsed = animate
          ? Math.min(50, performance.now() - received) * transport.speed
          : 0;
        const position = Math.min(
          song.duration_ms,
          transport.position_ms + elapsed,
        );
        const horizon = 5200;
        const pixelsPerMs = Math.max(1, hit - 24) / horizon;
        context.fillStyle = "#111b2a";
        context.fillRect(0, 0, width, height);
        for (const key of keys) {
          if (key.black) continue;
          if (key.pitch % 12 === 0) {
            context.fillStyle = "#142132";
            context.fillRect(key.x, 0, key.width, hit);
          }
          context.strokeStyle = "#243142";
          context.lineWidth = 0.7;
          context.beginPath();
          context.moveTo(key.x, 0);
          context.lineTo(key.x, hit);
          context.stroke();
        }
        const startSecond = Math.ceil(position / 1000);
        for (
          let second = startSecond;
          second * 1000 <= position + horizon;
          second++
        ) {
          const y = hit - (second * 1000 - position) * pixelsPerMs;
          context.strokeStyle = "#2a3847";
          context.lineWidth = 1;
          context.beginPath();
          context.moveTo(0, y);
          context.lineTo(width, y);
          context.stroke();
          context.fillStyle = "#758599";
          context.font = "10px 'Segoe UI', sans-serif";
          context.fillText(`${second}s`, 8, y - 5);
        }
        const partMap = new Map(
          song.parts.map((p) => [p.id, partSetting(p, settings)]),
        );
        const anySolo = [...partMap.values()].some((p) => p.solo);
        context.save();
        context.beginPath();
        context.rect(0, 0, width, hit);
        context.clip();
        visibleNotes(position - 150, position + horizon, (note) => {
          const key = keyMap.get(note.pitch);
          if (!key) return;
          const part = partMap.get(note.part_id);
          if (part?.role === "muted" || (anySolo && !part?.solo)) return;
          const hand =
            settings.split_pitch !== null && part?.hand === "auto"
              ? note.pitch < settings.split_pitch
                ? "left"
                : "right"
              : part?.hand;
          const color =
            part?.role === "accompaniment"
              ? "#78899e"
              : hand === "left"
                ? "#f0b985"
                : "#70d6c1";
          const bottom = hit - (note.start_ms - position) * pixelsPerMs;
          const top = hit - (note.end_ms - position) * pixelsPerMs;
          const x = key.x + (key.black ? 1 : 2),
            w = Math.max(2, key.width - (key.black ? 2 : 4));
          context.fillStyle = color;
          context.globalAlpha = part?.role === "accompaniment" ? 0.58 : 0.95;
          context.beginPath();
          context.roundRect(x, top, w, Math.max(7, bottom - top - 2), 4);
          context.fill();
          context.globalAlpha = 1;
          context.fillStyle = "#10282d";
          if (preferences.labels && w > 19 && bottom - top > 20) {
            context.font = `600 ${Math.min(12, w * 0.34)}px 'Segoe UI', sans-serif`;
            context.textAlign = "center";
            context.fillText(
              noteName(note.pitch, preferences.note_numbers),
              x + w / 2,
              Math.min(hit - 6, bottom - 8),
            );
          }
        });
        context.restore();
        context.textAlign = "start";
        const held = new Set(status.held),
          targets = new Set(transport.targets),
          matched = new Set(transport.matched);
        for (const black of [false, true])
          for (const key of keys) {
            if (key.black !== black) continue;
            const keyHeight = key.black
              ? keyboardHeight * 0.61
              : keyboardHeight;
            context.fillStyle = held.has(key.pitch)
              ? "#70d6c1"
              : targets.has(key.pitch)
                ? matched.has(key.pitch)
                  ? "#70d6c1"
                  : "#eeb786"
                : key.black
                  ? "#1b2635"
                  : "#eeeae0";
            context.fillRect(
              key.x + 0.7,
              hit + 3,
              Math.max(1, key.width - 1.4),
              keyHeight - 4,
            );
            if (key.black) {
              context.fillStyle = "#344052";
              context.fillRect(
                key.x + 2,
                hit + keyHeight - 7,
                Math.max(1, key.width - 4),
                3,
              );
            }
            const showLabel =
              preferences.labels &&
              (!key.black || key.width > 30) &&
              key.width > 14;
            if (showLabel && (key.width > 24 || key.pitch % 12 === 0)) {
              context.textAlign = "center";
              context.font = `${key.pitch % 12 === 0 ? "700" : "400"} ${Math.min(11, key.width * 0.35)}px 'Segoe UI', sans-serif`;
              context.fillStyle = key.black ? "#aebac8" : "#62707c";
              context.fillText(
                noteName(key.pitch, preferences.note_numbers),
                key.x + key.width / 2,
                hit + keyHeight - 12,
              );
            }
          }
        context.textAlign = "start";
        context.fillStyle = transport.waiting ? "#f0b985" : "#70d6c1";
        context.fillRect(0, hit, width, 3);
        context.shadowColor = "#70d6c1";
        context.shadowBlur = 12;
        context.fillRect(0, hit, width, 1);
        context.shadowBlur = 0;
      }
      if (animate && !document.hidden) frame = requestAnimationFrame(draw);
    };
    const wake = () => {
      if (!frame && !document.hidden) frame = requestAnimationFrame(draw);
    };
    requestDraw.current = wake;
    const visibility = () => {
      dirty = true;
      wake();
    };
    document.addEventListener("visibilitychange", visibility);
    wake();
    return () => {
      cancelAnimationFrame(frame);
      resize.disconnect();
      document.removeEventListener("visibilitychange", visibility);
      requestDraw.current = null;
      for (const pitch of pressed.current.values())
        current.current.onNote(pitch, false);
      pressed.current.clear();
    };
  }, [visibleNotes]);
  useEffect(() => {
    requestDraw.current?.();
  }, [props.status, props.settings, props.preferences]);

  const release = (pointer: number) => {
    const pitch = pressed.current.get(pointer);
    if (pitch !== undefined) {
      props.onNote(pitch, false);
      pressed.current.delete(pointer);
    }
  };
  return (
    <canvas
      ref={canvas}
      className={`piano-roll ${props.preferences.simulated ? "can-play" : ""}`}
      aria-label="Falling note piano roll. Amber notes are left hand, teal right hand, gray accompaniment. The visible hit line is above the keyboard. Simulated key input requires enabling the test input switch."
      onPointerDown={(event) => {
        if (!props.preferences.simulated) return;
        const rect = event.currentTarget.getBoundingClientRect();
        const x = event.clientX - rect.left,
          y = event.clientY - rect.top;
        const { keys, hit, height } = geometry.current;
        if (y < hit) return;
        const key = [...keys]
          .sort((a, b) => Number(b.black) - Number(a.black))
          .find(
            (k) =>
              x >= k.x &&
              x <= k.x + k.width &&
              (!k.black || y <= hit + (height - hit) * 0.61),
          );
        if (key) {
          event.currentTarget.setPointerCapture(event.pointerId);
          pressed.current.set(event.pointerId, key.pitch);
          props.onNote(key.pitch, true);
        }
      }}
      onPointerUp={(event) => release(event.pointerId)}
      onPointerCancel={(event) => release(event.pointerId)}
      onLostPointerCapture={(event) => release(event.pointerId)}
    />
  );
}
