export interface Note {
  id: number;
  part_id: number;
  channel: number;
  pitch: number;
  velocity: number;
  start_tick: number;
  end_tick: number;
  start_ms: number;
  end_ms: number;
}
export interface Part {
  id: number;
  track: number;
  channel: number;
  name: string;
  note_count: number;
  percussion: boolean;
}
export interface Song {
  ppq: number;
  duration_ms: number;
  notes: Note[];
  parts: Part[];
  tempos: { tick: number; micros_per_quarter: number; time_ms: number }[];
  time_signatures: { tick: number; numerator: number; denominator: number }[];
  controllers: {
    part_id: number;
    channel: number;
    tick: number;
    time_ms: number;
    controller: number;
    value: number;
  }[];
  warnings: string[];
}
export interface PartSetting {
  part_id: number;
  role: string;
  hand: string;
  volume: number;
  solo: boolean;
}
export interface SongSettings {
  speed: number;
  mode: string;
  parts: PartSetting[];
  loop_start_ms: number | null;
  loop_end_ms: number | null;
  position_ms: number;
  split_pitch: number | null;
  split_hand?: string;
  count_in: boolean;
  metronome: boolean;
}
export interface LibraryEntry {
  id: string;
  original_id?: string | null;
  title: string;
  artist: string;
  tags: string;
  arrangement: string;
  favorite: boolean;
  imported_at: number;
  last_played: number | null;
  duration_ms: number;
  parts_count: number;
  note_count: number;
}
export interface ImportResult {
  entry: LibraryEntry;
  duplicate: boolean;
  elapsed_ms: number;
}
export interface ImportOutcome {
  path: string;
  result?: ImportResult;
  error?: string;
}
export interface ImportJob {
  key: number;
  path: string;
  state: "queued" | "importing" | "ready" | "failed";
  result?: ImportResult;
  error?: string;
}
export interface LessonState {
  revision: number;
  can_undo: boolean;
  removed_count: number;
  shifted_count: number;
}
export type PitchShift = -12 | -1 | 1 | 12;
export type LessonToolRequest =
  | {
      kind: "thin";
      strategy: "highest" | "bass_highest" | "limit";
      max_notes: number;
      part_id: number | null;
    }
  | { kind: "shift"; note_ids: number[]; semitones: -12 | 12 };
export interface LessonPreview {
  revision: number;
  changes: { note_id: number; from_pitch: number; to_pitch: number | null }[];
}
export interface OpenSong {
  entry: LibraryEntry;
  song: Song;
  settings: SongSettings;
  lesson?: LessonState | null;
}
export interface Device {
  id: string;
  name: string;
}
export interface Preferences {
  device_id: string | null;
  output_device_id: string | null;
  light_guide: boolean;
  monitor: string;
  simulated: boolean;
  low: number;
  high: number;
  labels: boolean;
  note_numbers: boolean;
}
export interface TransportSnapshot {
  position_ms: number;
  duration_ms: number;
  playing: boolean;
  waiting: boolean;
  mode: string;
  speed: number;
  targets: number[];
  matched: number[];
  correct: number;
  wrong: number;
  missed: number;
  feedback: string;
  beat: number;
  count_in_remaining: number;
}
export interface MidiEvent {
  sequence: number;
  source: string;
  kind: string;
  channel: number;
  pitch: number;
  velocity: number;
}
export interface AppStatus {
  transport: TransportSnapshot;
  device_id: string | null;
  device_name: string | null;
  device_connected: boolean;
  device_error?: string | null;
  output_device_id: string | null;
  output_device_name: string | null;
  output_connected: boolean;
  output_error: string | null;
  light_guide: boolean;
  light_test_active: boolean;
  lit_notes: number[];
  output_messages: number;
  audio_available: boolean;
  audio_error: string | null;
  audio_frames: number;
  audio_nonzero_frames: number;
  events: MidiEvent[];
  held: number[];
  dropped_events: number;
}
export const DEFAULT_PREFERENCES: Preferences = {
  device_id: null,
  output_device_id: null,
  light_guide: false,
  monitor: "yamaha",
  simulated: false,
  low: 36,
  high: 96,
  labels: true,
  note_numbers: false,
};
export const EMPTY_STATUS: AppStatus = {
  transport: {
    position_ms: 0,
    duration_ms: 0,
    playing: false,
    waiting: false,
    mode: "listen",
    speed: 1,
    targets: [],
    matched: [],
    correct: 0,
    wrong: 0,
    missed: 0,
    feedback: "",
    beat: 0,
    count_in_remaining: 0,
  },
  device_id: null,
  device_name: null,
  device_connected: false,
  output_device_id: null,
  output_device_name: null,
  output_connected: false,
  output_error: null,
  light_guide: false,
  light_test_active: false,
  lit_notes: [],
  output_messages: 0,
  audio_available: false,
  audio_error: null,
  audio_frames: 0,
  audio_nonzero_frames: 0,
  events: [],
  held: [],
  dropped_events: 0,
};
export function noteName(pitch: number, numbers = false): string {
  return numbers
    ? String(pitch)
    : `${["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][pitch % 12]}${Math.floor(pitch / 12) - 1}`;
}
export function timeLabel(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
export function partSetting(part: Part, settings: SongSettings): PartSetting {
  return (
    settings.parts.find((p) => p.part_id === part.id) ?? {
      part_id: part.id,
      role: part.percussion ? "accompaniment" : "practice",
      hand: "auto",
      volume: 0.65,
      solo: false,
    }
  );
}

// Presentation mirrors the engine's pitch-threshold side selection. The engine
// remains authoritative for deciding which notes are playback or player targets.
export function isSplitAccompaniment(
  settings: SongSettings,
  pitch: number,
): boolean {
  if (settings.split_pitch === null) return false;
  const side = settings.split_hand ?? "both";
  return (
    (side === "left" && pitch >= settings.split_pitch) ||
    (side === "right" && pitch < settings.split_pitch)
  );
}
