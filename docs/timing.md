# Timing and practice semantics

`transport::Engine` is the only sequencer. Its owner supplies elapsed milliseconds
from Rust's monotonic `Instant`; it does not read system/calendar time and the
frontend does not advance it. A caller can supply deterministic deltas in tests.
Musical milliseconds are original-file time computed from the canonical tempo
map. Each running wall-time interval advances song time by `elapsed × speed`.
Speed is bounded to 25–150% and never changes pitch. Canvas frames only display
the latest position. Audio consumes discrete voice events and converts them to
samples at the actual device sample rate; scheduling is callback/block granular,
not a claim of sample-accurate or measured physical MIDI latency.

## Practice

- Listen schedules every audible part. Wait and Timed schedule accompaniment;
  practice notes are played by the learner through Yamaha or software monitoring.
  Muted parts and parts excluded by Solo are neither sounded nor graded.
  Channel 10/percussion is never a training target, even if labeled Practice.
- Onsets within 30 ms of a group's first onset form a chord. Duplicate pitches
  exactly at one onset are one target. A later repeated pitch begins a new group,
  including fast repetitions within 30 ms. The grouping window is in original
  song time, so changing speed does not change the lesson's groups.
- Wait freezes on the group's first onset and requires every target pitch.
  Chord presses must occur within 150 ms of wall time. Older partial matches
  expire; release and replay those pitches. Wrong extra notes are reported but
  never block a correct chord. Correct counts completed target pitches, not
  an opaque score. Wait does not count misses for taking time.
- Timed runs continuously. The current uncompleted group accepts presses within
  ±150 ms of wall time around its onset (scaled to song time by speed). Each
  unmatched pitch becomes missed when that group's window passes. Valid timed
  pitch credit remains even if a partial chord's input window expires.
  Replaying that pitch cannot receive duplicate credit. Fast passages
  advance to the next group when the current group completes. Matching is in
  onset order, not an automatic nearest-note alignment algorithm.
  Song end and loop B close any remaining onset windows: partial valid chord
  pitches receive credit and unplayed pitches are counted as missed. Completed
  groups are not counted again. Targets at or beyond B are excluded, including
  a near-simultaneous chord pitch whose own onset lies exactly at B.
- Every new target requires a fresh player note-on, preceded by an off if that
  source already holds the pitch. This includes overlapping repeated pitches,
  chord transitions, loop restarts, and seeks. A physically held note is never
  implicitly carried into a new target. Velocity-zero note-on is a release.
  Pedals do not generate presses. Only Physical input is considered; Simulated
  and Playback input is discarded before even updating engine held state.
- Left/right part labels remain presentation metadata. The separate approximate
  pitch split can select **Both**, **Left**, or **Right** for grading within
  Practice parts: Left is below the boundary, Right includes the boundary and
  higher pitches. In Wait/Timed, the other side sounds as accompaniment and is
  excluded from targets. Muted/Solo rules and percussion exclusion still apply.
  Both, or no enabled boundary, grades the complete selected Practice parts.
  Listen plays every audible part regardless of the split. This is a pitch
  heuristic for combined material, not melody or hand extraction; it does not
  transpose pitches or modify canonical notes. Existing saved settings default
  to Both until the user chooses one side.

## Voice lifetime and controls

Wait stops song time, notes, and accompaniment together. On reaching the gate,
scheduled sounding voices receive individual Off events, while their logical
musical lifetime is retained. Completing the target restarts any crossing
accompaniment voices at that position. They restart their envelope; there is no
sample-position resume. Live monitor voices use a separate key range and remain
responsive during waiting. Timeline events exactly at the gate are deferred
until it is completed, so accompaniment does not audibly lead an unanswered chord.

Pause releases scheduled voices and preserves logical voices for Play. Stop and
seek reset audio voices. Seek rebuilds crossing voices once and starts matching
at the first group at or after the position; earlier held notes are not targets.
Changing parts, mode, loop, or approximate split resets active voices/matching
at the same position.
Speed-only or display-only changes preserve the target and sounding voices;
changing speed during count-in scales its remaining wall-time interval.
The application also owns resets on device changes, panic, and shutdown.

A–B uses an inclusive start and exclusive musical endpoint for repeating
targets. At B, voices/controllers reset, crossing accompaniment is reconstructed
at A, and matching rearms. Scores accumulate through loop iterations. More than
64 loop wraps in one owner update pauses with an explicit clock-gap message.
Count-in uses the current meter's numerator (capped at 16) and denominator beat,
with tempo and speed applied. It freezes song position, emits clicks, and ignores
grading until finished. Each Play starts a requested count-in. The metronome
follows the tempo map and time-signature changes; no click is emitted while Wait
is frozen. Catch-up clicks are capped at 128 per interval.
Chord grouping is split at A so a target just before A cannot hide a pitch at A
when seeking or repeating the loop. Changing MIDI input closes the previous
connection and discards its queued messages before opening the selected port.

CC64 sustain is applied by MIDI channel, including controllers in another track
on that channel. Key release remains the canonical `end_ms`; the separately
computed audio release extends to the next pedal-up, or song end when pedal-up
is missing. Program changes, expression, pitch bend, other controllers, drums,
and General MIDI timbres are not faithfully synthesized by the basic tone.
No imported SysEx or device-configuration command is sent to hardware.

## Cost and verification

Loading/configuration precomputes sorted target groups and on/audio-off events.
Normal updates walk only newly due events and the current matching group; they
do not scan all notes. Sustain lookup uses binary search plus cached next-release
indices. Seek scans the song once to reconstruct crossing voices. Metronome
time conversion uses binary search over tempo changes. Snapshot data is bounded
to the current target's pitches and score/state, not the complete song.

Unit tests inject clock deltas and synthetic events tagged Physical for repeated notes,
chords, wrong extras, held transitions, crossing accompaniment, sustain,
seek/pause, speed/tempo, count-in, loops, and timed misses. These are simulated
engine checks and do not verify a Yamaha, USB adapter, native event latency, or
physical key-to-sound latency.
