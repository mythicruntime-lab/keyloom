import { useMemo, useState } from "react";
import { LessonState, Preferences, Song, noteName, timeLabel } from "./types";

interface Props {
  song: Song;
  lesson: LessonState;
  preferences: Preferences;
  selected: Set<number>;
  position: number;
  saving: boolean;
  onToggle: (id: number) => void;
  onSelect: (ids: number[]) => void;
  onClear: () => void;
  onSeek: (ms: number) => void;
  onRemove: () => void;
  onUndo: () => void;
  onDone: () => void;
}
const PAGE_SIZE = 48;

export default function LessonEditor({
  song,
  lesson,
  preferences,
  selected,
  position,
  saving,
  onToggle,
  onSelect,
  onClear,
  onSeek,
  onRemove,
  onUndo,
  onDone,
}: Props) {
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(0);
  const sorted = useMemo(
    () =>
      [...song.notes].sort((a, b) => a.start_ms - b.start_ms || a.id - b.id),
    [song],
  );
  const partNames = useMemo(
    () =>
      new Map(
        song.parts.map((part) => [
          part.id,
          part.name || `Track ${part.track + 1}`,
        ]),
      ),
    [song],
  );
  const outside = (pitch: number) =>
    pitch < preferences.low || pitch > preferences.high;
  const outsideCount = useMemo(
    () =>
      song.notes.filter(
        (note) => note.pitch < preferences.low || note.pitch > preferences.high,
      ).length,
    [song, preferences.low, preferences.high],
  );
  const filtered = useMemo(
    () =>
      sorted.filter((note) =>
        filter === "outside"
          ? note.pitch < preferences.low || note.pitch > preferences.high
          : filter === "segment"
            ? note.end_ms >= position && note.start_ms <= position + 5200
            : true,
      ),
    [sorted, filter, preferences.low, preferences.high, position],
  );
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const currentPage = Math.min(page, pages - 1);
  const visible = filtered.slice(
    currentPage * PAGE_SIZE,
    (currentPage + 1) * PAGE_SIZE,
  );
  return (
    <aside className="lesson-editor" aria-label="Practice version note editor">
      <div className="card-title">
        <h2>Edit notes</h2>
        <button className="small-button" disabled={saving} onClick={onDone}>
          Done editing
        </button>
      </div>
      <p className="help-text">
        Playback is paused. Select notes on the roll or in this list, then
        remove them from this version. The original is unchanged.
      </p>
      <div
        className="lesson-edit-summary"
        data-selected-count={selected.size}
        data-removed-count={lesson.removed_count}
        aria-live="polite"
      >
        <strong>{selected.size} selected</strong>
        <span>{lesson.removed_count} removed</span>
      </div>
      <div className="lesson-edit-actions">
        <button
          className="button primary"
          disabled={saving || !selected.size}
          onClick={onRemove}
        >
          Remove selected notes
        </button>
        <button
          className="small-button"
          disabled={saving || !lesson.can_undo}
          onClick={onUndo}
        >
          Undo removal
        </button>
        <button
          className="small-button"
          disabled={saving || !selected.size}
          onClick={onClear}
        >
          Clear selection
        </button>
      </div>
      <p
        className="lesson-range-warning"
        data-out-of-range-count={outsideCount}
      >
        {outsideCount} notes outside your keyboard range (
        {noteName(preferences.low)}–{noteName(preferences.high)}). All notes
        remain selectable here.
      </p>
      <label className="field-label">
        Notes to show
        <select
          aria-label="Notes to show"
          value={filter}
          disabled={saving}
          onChange={(event) => {
            setFilter(event.target.value);
            setPage(0);
          }}
        >
          <option value="all">All notes</option>
          <option value="outside">Outside keyboard range</option>
          <option value="segment">At playhead</option>
        </select>
      </label>
      <div className="lesson-list-actions">
        <button
          className="small-button"
          disabled={saving || !visible.length}
          onClick={() => onSelect(visible.map((note) => note.id))}
        >
          Select this page
        </button>
        <span>{filtered.length.toLocaleString()} notes</span>
      </div>
      <div
        className="lesson-note-list"
        role="group"
        aria-label="Selectable notes"
      >
        {visible.map((note) => (
          <div
            className={`lesson-note-row ${selected.has(note.id) ? "selected" : ""}`}
            data-note-id={note.id}
            key={note.id}
          >
            <label>
              <input
                type="checkbox"
                aria-label={`Select note ${note.id}`}
                checked={selected.has(note.id)}
                disabled={saving}
                onChange={() => onToggle(note.id)}
              />
              <span>
                <strong>
                  {noteName(note.pitch, preferences.note_numbers)}{" "}
                  <small>#{note.id}</small>
                </strong>
                <small>
                  {timeLabel(note.start_ms)}.
                  {String(Math.floor(note.start_ms % 1000)).padStart(3, "0")} ·{" "}
                  {partNames.get(note.part_id)}
                </small>
                {outside(note.pitch) && <em>Outside keyboard range</em>}
              </span>
            </label>
            <button
              className="small-button"
              aria-label={`Show note ${note.id} on roll`}
              disabled={saving}
              onClick={() => onSeek(Math.max(0, note.start_ms - 1000))}
            >
              Show
            </button>
          </div>
        ))}
        {!visible.length && (
          <p className="help-text">
            No notes in this view. Change the filter or seek to another part of
            the song.
          </p>
        )}
      </div>
      <div className="lesson-pagination">
        <button
          className="small-button"
          disabled={saving || currentPage === 0}
          onClick={() => setPage(currentPage - 1)}
        >
          Previous
        </button>
        <span>
          Page {currentPage + 1} of {pages}
        </span>
        <button
          className="small-button"
          disabled={saving || currentPage + 1 >= pages}
          onClick={() => setPage(currentPage + 1)}
        >
          Next
        </button>
      </div>
      <p className="help-text">
        Changes save after each removal or Undo. Undo keeps your last 100
        removals. This editor removes notes only. MIDI export for practice
        versions is not available yet.
      </p>
    </aside>
  );
}
