import { useEffect, useMemo, useRef, useState } from "react";
import {
  LessonState,
  LessonPreview,
  LessonToolRequest,
  PitchShift,
  Preferences,
  Song,
  noteName,
  timeLabel,
} from "./types";

interface Props {
  song: Song;
  lesson: LessonState;
  preferences: Preferences;
  selected: Set<number>;
  position: number;
  saving: boolean;
  preview: { request: LessonToolRequest; result: LessonPreview } | null;
  previewDisplay: "before" | "after";
  onPreview: (request: LessonToolRequest) => void;
  onCancelPreview: () => void;
  onApplyPreview: () => void;
  onPreviewDisplay: (display: "before" | "after") => void;
  onToggle: (id: number) => void;
  onSelect: (ids: number[]) => void;
  onReplaceSelection: (ids: number[]) => void;
  onClear: () => void;
  onSeek: (ms: number) => void;
  onRemove: () => void;
  onTranspose: (semitones: PitchShift) => void;
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
  preview,
  previewDisplay,
  onPreview,
  onCancelPreview,
  onApplyPreview,
  onPreviewDisplay,
  onToggle,
  onSelect,
  onReplaceSelection,
  onClear,
  onSeek,
  onRemove,
  onTranspose,
  onUndo,
  onDone,
}: Props) {
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(0);
  const [strategy, setStrategy] = useState<
    "highest" | "bass_highest" | "limit"
  >("highest");
  const [maxNotes, setMaxNotes] = useState(2);
  const [partScope, setPartScope] = useState("all");
  const [previewPage, setPreviewPage] = useState(0);
  const editorElement = useRef<HTMLElement>(null);
  const previewElement = useRef<HTMLElement>(null);
  useEffect(() => {
    setPreviewPage(0);
    const editor = editorElement.current,
      panel = previewElement.current;
    if (!preview || !editor || !panel) return;
    editor.scrollTop +=
      panel.getBoundingClientRect().top -
      editor.getBoundingClientRect().top -
      12;
    panel.focus({ preventScroll: true });
  }, [preview?.result]);
  const blocked = saving || !!preview;
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
  const pitchedParts = useMemo(
    () => song.parts.filter((part) => !part.percussion),
    [song],
  );
  const rangeSelections = useMemo(() => {
    const pitched = new Set(pitchedParts.map((part) => part.id));
    const below: number[] = [],
      above: number[] = [];
    for (const note of song.notes) {
      if (!pitched.has(note.part_id)) continue;
      if (note.pitch < preferences.low) below.push(note.id);
      else if (note.pitch > preferences.high) above.push(note.id);
    }
    return { below, above, all: [...below, ...above] };
  }, [song, pitchedParts, preferences.low, preferences.high]);
  const notesById = useMemo(
    () => new Map(song.notes.map((note) => [note.id, note])),
    [song],
  );
  const previewChanges = useMemo(
    () =>
      [...(preview?.result.changes ?? [])].sort(
        (a, b) =>
          (notesById.get(a.note_id)?.start_ms ?? 0) -
            (notesById.get(b.note_id)?.start_ms ?? 0) || a.note_id - b.note_id,
      ),
    [preview?.result, notesById],
  );
  const previewPages = Math.max(
    1,
    Math.ceil(previewChanges.length / PAGE_SIZE),
  );
  const currentPreviewPage = Math.min(previewPage, previewPages - 1);
  const visibleChanges = previewChanges.slice(
    currentPreviewPage * PAGE_SIZE,
    (currentPreviewPage + 1) * PAGE_SIZE,
  );
  const selectedBounds = useMemo(() => {
    let low = 128,
      high = -1,
      count = 0;
    for (const note of song.notes) {
      if (!selected.has(note.id)) continue;
      low = Math.min(low, note.pitch);
      high = Math.max(high, note.pitch);
      count++;
    }
    return { low, high, count };
  }, [song, selected]);
  const canTranspose = (semitones: PitchShift) =>
    selectedBounds.count > 0 &&
    selectedBounds.low + semitones >= 0 &&
    selectedBounds.high + semitones <= 127;
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
    <aside
      ref={editorElement}
      className="lesson-editor"
      aria-label="Practice version note editor"
    >
      <div className="card-title">
        <h2>Edit notes</h2>
        <button className="small-button" disabled={saving} onClick={onDone}>
          Done editing
        </button>
      </div>
      <p className="help-text">
        Playback is paused. Select notes on the roll or in this list, then move
        or remove them from this version. The original is unchanged.
      </p>
      <div
        className="lesson-edit-summary"
        data-selected-count={selected.size}
        data-removed-count={lesson.removed_count}
        data-shifted-count={lesson.shifted_count ?? 0}
        aria-live="polite"
      >
        <strong>{selected.size} selected</strong>
        <span>
          {lesson.removed_count} removed · {lesson.shifted_count ?? 0} shifted
        </span>
      </div>
      <div
        className="lesson-pitch-actions"
        role="group"
        aria-label="Move selected notes"
      >
        {(
          [
            [-12, "Octave down"],
            [-1, "Semitone down"],
            [1, "Semitone up"],
            [12, "Octave up"],
          ] as const
        ).map(([semitones, label]) => (
          <button
            key={semitones}
            className="small-button"
            aria-label={label}
            aria-describedby="pitch-move-limits"
            disabled={blocked || !canTranspose(semitones)}
            onClick={() => onTranspose(semitones)}
          >
            {label}
          </button>
        ))}
      </div>
      <p className="help-text" id="pitch-move-limits">
        The whole selection must stay within MIDI 0–127; notes are never
        clamped. Moving outside your keyboard range is allowed.
      </p>
      <div className="lesson-edit-actions">
        <button
          className="button primary"
          disabled={blocked || !selected.size}
          onClick={onRemove}
        >
          Remove selected notes
        </button>
        <button
          className="small-button"
          disabled={blocked || !lesson.can_undo}
          onClick={onUndo}
        >
          Undo edit
        </button>
        <button
          className="small-button"
          disabled={blocked || !selected.size}
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
      <details className="lesson-tools">
        <summary>Simplify and fit</summary>
        <div className="lesson-tools-body">
          <p className="help-text">
            Chord thinning groups notes that start at the exact same tick.
            Percussion is excluded. This is a pitch heuristic, not melody
            extraction.
          </p>
          <label className="field-label">
            Chord thinning strategy
            <select
              aria-label="Chord thinning strategy"
              value={strategy}
              disabled={saving}
              onChange={(event) => {
                onCancelPreview();
                setStrategy(event.target.value as typeof strategy);
              }}
            >
              <option value="highest">Highest note</option>
              <option value="bass_highest">Bass + highest</option>
              <option value="limit">Limit highest notes</option>
            </select>
          </label>
          {strategy === "limit" && (
            <label className="field-label">
              Maximum chord notes
              <input
                aria-label="Maximum chord notes"
                type="number"
                min={1}
                max={8}
                step={1}
                value={maxNotes}
                disabled={saving}
                onChange={(event) => {
                  onCancelPreview();
                  setMaxNotes(Number(event.target.value));
                }}
              />
            </label>
          )}
          <label className="field-label">
            Chord thinning part
            <select
              aria-label="Chord thinning part"
              value={partScope}
              disabled={saving}
              onChange={(event) => {
                onCancelPreview();
                setPartScope(event.target.value);
              }}
            >
              <option value="all">All pitched parts</option>
              {pitchedParts.map((part) => (
                <option key={part.id} value={String(part.id)}>
                  {part.name || `Track ${part.track + 1}`} · Ch{" "}
                  {part.channel + 1}
                </option>
              ))}
            </select>
          </label>
          <p className="help-text">
            Highest and bass keep one event per chosen pitch. Limit keeps up to
            N highest note events. The All pitched parts option combines parts
            into one group.
          </p>
          <button
            className="small-button"
            disabled={
              saving ||
              !pitchedParts.length ||
              (strategy === "limit" &&
                (!Number.isInteger(maxNotes) || maxNotes < 1 || maxNotes > 8))
            }
            onClick={() =>
              onPreview({
                kind: "thin",
                strategy,
                max_notes:
                  strategy === "limit"
                    ? maxNotes
                    : strategy === "highest"
                      ? 1
                      : 2,
                part_id: partScope === "all" ? null : Number(partScope),
              })
            }
          >
            Preview chord thinning
          </button>
          <div className="lesson-range-tools">
            <strong>Fit your keyboard</strong>
            <p className="help-text">
              Select pitched notes across the whole song using their current
              pitches, regardless of the note-list page or filter. Each button
              replaces the selection.
            </p>
            <div
              className="lesson-range-selection"
              data-below-count={rangeSelections.below.length}
              data-above-count={rangeSelections.above.length}
              data-outside-count={rangeSelections.all.length}
            >
              <button
                className="small-button"
                disabled={saving || !rangeSelections.all.length}
                onClick={() => onReplaceSelection(rangeSelections.all)}
              >
                Select all outside range
              </button>
              <button
                className="small-button"
                disabled={saving || !rangeSelections.below.length}
                onClick={() => onReplaceSelection(rangeSelections.below)}
              >
                Select below range
              </button>
              <button
                className="small-button"
                disabled={saving || !rangeSelections.above.length}
                onClick={() => onReplaceSelection(rangeSelections.above)}
              >
                Select above range
              </button>
            </div>
            <p className="help-text">
              {rangeSelections.below.length} below ·{" "}
              {rangeSelections.above.length} above {noteName(preferences.low)}–
              {noteName(preferences.high)}
            </p>
            <div className="lesson-pitch-actions">
              <button
                className="small-button"
                disabled={saving || !canTranspose(-12)}
                onClick={() =>
                  onPreview({
                    kind: "shift",
                    note_ids: [...selected],
                    semitones: -12,
                  })
                }
              >
                Preview octave down
              </button>
              <button
                className="small-button"
                disabled={saving || !canTranspose(12)}
                onClick={() =>
                  onPreview({
                    kind: "shift",
                    note_ids: [...selected],
                    semitones: 12,
                  })
                }
              >
                Preview octave up
              </button>
            </div>
          </div>
        </div>
      </details>
      {preview && (
        <section
          className="lesson-tool-preview"
          ref={previewElement}
          tabIndex={-1}
          aria-label="Lesson tool preview"
          data-change-count={preview.result.changes.length}
          data-revision={preview.result.revision}
          data-preview-kind={preview.request.kind}
          data-display={previewDisplay}
        >
          <div className="lesson-preview-title">
            <strong>
              {preview.result.changes.length.toLocaleString()} notes would
              change
            </strong>
            <span>Not saved</span>
          </div>
          <p className="help-text">
            Compare the roll before and after. Preview sends no notes. Apply
            saves the whole change as one Undo step.
          </p>
          <div
            className="lesson-preview-display"
            role="group"
            aria-label="Preview display"
          >
            <button
              className="small-button"
              aria-pressed={previewDisplay === "before"}
              disabled={saving}
              onClick={() => onPreviewDisplay("before")}
            >
              Before
            </button>
            <button
              className="small-button"
              aria-pressed={previewDisplay === "after"}
              disabled={saving}
              onClick={() => onPreviewDisplay("after")}
            >
              After
            </button>
          </div>
          <div
            className="lesson-preview-list"
            role="list"
            aria-label="Exact preview changes"
          >
            {visibleChanges.map((change) => {
              const note = notesById.get(change.note_id);
              return (
                <div
                  className="lesson-preview-change"
                  role="listitem"
                  key={change.note_id}
                  data-note-id={change.note_id}
                  data-from-pitch={change.from_pitch}
                  data-to-pitch={
                    change.to_pitch === null ? "removed" : change.to_pitch
                  }
                >
                  <strong>
                    {noteName(change.from_pitch, preferences.note_numbers)} →{" "}
                    {change.to_pitch === null
                      ? "Removed"
                      : noteName(
                          change.to_pitch,
                          preferences.note_numbers,
                        )}{" "}
                    <small>#{change.note_id}</small>
                  </strong>
                  <small>
                    {note
                      ? `${timeLabel(note.start_ms)}.${String(Math.floor(note.start_ms % 1000)).padStart(3, "0")} · ${partNames.get(note.part_id)}`
                      : `Note ${change.note_id}`}
                  </small>
                </div>
              );
            })}
            {!previewChanges.length && (
              <p className="help-text">
                Nothing would change with these settings. No edit will be saved.
              </p>
            )}
          </div>
          <div className="lesson-pagination">
            <button
              className="small-button"
              disabled={saving || currentPreviewPage === 0}
              onClick={() => setPreviewPage(currentPreviewPage - 1)}
            >
              Previous changes
            </button>
            <span>
              {currentPreviewPage + 1} / {previewPages}
            </span>
            <button
              className="small-button"
              disabled={saving || currentPreviewPage + 1 >= previewPages}
              onClick={() => setPreviewPage(currentPreviewPage + 1)}
            >
              Next changes
            </button>
          </div>
          <div className="lesson-preview-actions">
            <button
              className="small-button"
              disabled={saving}
              onClick={onCancelPreview}
            >
              Cancel preview
            </button>
            <button
              className="button primary"
              disabled={saving || !previewChanges.length}
              onClick={onApplyPreview}
            >
              Apply preview
            </button>
          </div>
        </section>
      )}
      {!preview && (
        <>
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
                data-note-pitch={note.pitch}
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
                      {String(Math.floor(note.start_ms % 1000)).padStart(
                        3,
                        "0",
                      )}{" "}
                      · {partNames.get(note.part_id)}
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
                No notes in this view. Change the filter or seek to another part
                of the song.
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
        </>
      )}
      <p className="help-text">
        Changes save after each edit or Undo. Undo keeps up to 100 edits; very
        large selections retain fewer. Note timing and duration stay unchanged.
      </p>
    </aside>
  );
}
