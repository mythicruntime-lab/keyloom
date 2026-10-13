//! MIDI export for saved note removals/pitch moves. This module only returns
//! bytes; original-file export and destination creation belong to the library.
use crate::model::{Note, Song};
use crate::parser::{self, MAX_EVENTS, MAX_FILE_BYTES, MAX_NOTES};
use midly::{MetaMessage, MidiMessage, Smf, TrackEvent, TrackEventKind};
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};

const MAX_DELTA: u64 = (1 << 28) - 1;

/// Preserve source tracks, event order and non-note payloads while applying the
/// canonical lesson's removals and pitch changes. Timing/velocity/part edits are
/// deliberately rejected: the current editor does not author those changes.
pub fn edited_midi(original_bytes: &[u8], edited: &Song) -> Result<Vec<u8>, String> {
    if edited.notes.is_empty() {
        return Err("This practice version has no notes to export. Undo a removal first.".into());
    }
    if edited.notes.len() > MAX_NOTES {
        return Err(format!(
            "Edited MIDI exceeds the {MAX_NOTES}-note safety limit."
        ));
    }
    // This performs the container, format, track, event, note, byte and duration
    // checks before Smf allocates event vectors or we construct an output.
    let original = parser::parse(original_bytes)?;
    if edited.ppq != original.ppq
        || edited.tempos != original.tempos
        || edited.time_signatures != original.time_signatures
        || edited.controllers != original.controllers
        || edited.duration_ms != original.duration_ms
    {
        return Err("Edited MIDI export supports note removals and pitch moves only; its source timing and controllers must remain unchanged.".into());
    }
    let mut source_notes = vec![None; original.notes.len()];
    for note in &original.notes {
        source_notes[note.id as usize] = Some(note);
    }
    let mut kept = vec![None; original.notes.len()];
    for note in &edited.notes {
        let source = source_notes
            .get(note.id as usize)
            .and_then(|note| *note)
            .ok_or("Edited note is not part of the source MIDI. Reopen the practice version.")?;
        if note.pitch > 127 {
            return Err("An edited pitch is outside MIDI's 0–127 range.".into());
        }
        let mut expected = source.clone();
        expected.pitch = note.pitch;
        if *note != expected {
            return Err("Edited MIDI export supports note removals and pitch moves only; note timing, channel, velocity and identity must remain unchanged.".into());
        }
        if kept[note.id as usize].replace(note).is_some() {
            return Err("Duplicate edited note identity. Reopen the practice version.".into());
        }
    }
    let mut smf = Smf::parse(original_bytes).map_err(|error| format!("Malformed MIDI: {error}"))?;
    let mut next_id = 0usize;
    let mut event_count = 0usize;
    for (track_index, track) in smf.tracks.iter_mut().enumerate() {
        let mut pending = BTreeMap::<(u8, u8), VecDeque<usize>>::new();
        let mut events = Vec::new();
        let mut tick = 0u64;
        let mut end_tick = 0u64;
        for (order, event) in track.iter().enumerate() {
            tick += event.delta.as_int() as u64; // Already checked by parser.
            end_tick = end_tick.max(tick);
            let kind = match event.kind {
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel },
                } if vel.as_int() > 0 => {
                    let id = next_id;
                    next_id += 1;
                    pending
                        .entry((channel.as_int(), key.as_int()))
                        .or_default()
                        .push_back(id);
                    // Keep the source's canonical duration even when the final
                    // dangling note was removed from this practice version.
                    end_tick =
                        end_tick.max(source_notes[id].expect("canonical source ID").end_tick);
                    kept[id].map(|note| TrackEventKind::Midi {
                        channel,
                        message: MidiMessage::NoteOn {
                            key: note.pitch.into(),
                            vel,
                        },
                    })
                }
                TrackEventKind::Midi { channel, message }
                    if matches!(
                        message,
                        MidiMessage::NoteOff { .. } | MidiMessage::NoteOn { .. }
                    ) =>
                {
                    let key = match message {
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => key,
                        _ => unreachable!(),
                    };
                    pending
                        .get_mut(&(channel.as_int(), key.as_int()))
                        .and_then(VecDeque::pop_front)
                        .and_then(|id| kept[id])
                        .map(|note| TrackEventKind::Midi {
                            channel,
                            message: match message {
                                MidiMessage::NoteOff { vel, .. } => MidiMessage::NoteOff {
                                    key: note.pitch.into(),
                                    vel,
                                },
                                MidiMessage::NoteOn { vel, .. } => MidiMessage::NoteOn {
                                    key: note.pitch.into(),
                                    vel,
                                },
                                _ => unreachable!(),
                            },
                        })
                }
                TrackEventKind::Meta(MetaMessage::EndOfTrack) => None,
                other => Some(other),
            };
            if let Some(kind) = kind {
                push_event(&mut events, &mut event_count, tick, order, kind)?;
            }
        }
        for id in pending.values().flatten() {
            if let Some(note) = kept[*id] {
                push_event(
                    &mut events,
                    &mut event_count,
                    note.end_tick,
                    track.len() + *id,
                    TrackEventKind::Midi {
                        channel: note.channel.into(),
                        message: MidiMessage::NoteOff {
                            key: note.pitch.into(),
                            vel: 0.into(),
                        },
                    },
                )?;
            }
        }
        // Keep original same-tick ordering, including controller/note ties and
        // zero-duration On→Off pairs. Generated releases follow source events.
        events.sort_by_key(|(tick, order, _)| (*tick, *order));
        push_event(
            &mut events,
            &mut event_count,
            end_tick,
            usize::MAX,
            TrackEventKind::Meta(MetaMessage::EndOfTrack),
        )?;
        let mut previous = 0u64;
        let mut output_track = Vec::with_capacity(events.len());
        for (tick, _, kind) in events {
            let delta = tick - previous;
            if delta > MAX_DELTA {
                return Err(format!("Edited MIDI has a gap larger than MIDI's 28-bit delta limit in track {}. Undo some removals or use the original export; no file was written.", track_index + 1));
            }
            output_track.push(TrackEvent {
                delta: (delta as u32).into(),
                kind,
            });
            previous = tick;
        }
        *track = output_track;
    }
    let mut output = BoundedBytes(Vec::new());
    smf.write_std(&mut output)
        .map_err(|error| format!("Could not encode edited MIDI: {error}"))?;
    let reloaded = parser::parse(&output.0)
        .map_err(|error| format!("Edited MIDI could not be verified: {error}"))?;
    if musical_notes(&reloaded) != musical_notes(edited)
        || reloaded.ppq != edited.ppq
        || reloaded.tempos != edited.tempos
        || reloaded.controllers != edited.controllers
        || reloaded.time_signatures != edited.time_signatures
        || reloaded.duration_ms != edited.duration_ms
    {
        return Err("This practice version cannot be exported without changing its notes or timing. Overlapping notes moved onto the same pitch can have ambiguous MIDI releases. Undo the conflicting pitch move or export the original; no file was written.".into());
    }
    Ok(output.0)
}

fn push_event<'a>(
    events: &mut Vec<(u64, usize, TrackEventKind<'a>)>,
    total: &mut usize,
    tick: u64,
    order: usize,
    kind: TrackEventKind<'a>,
) -> Result<(), String> {
    if *total >= MAX_EVENTS {
        return Err(format!(
            "Edited MIDI exceeds the {MAX_EVENTS}-event safety limit."
        ));
    }
    *total += 1;
    events.push((tick, order, kind));
    Ok(())
}

// IDs are regenerated when removed onsets disappear; compare musical identity.
type MusicalNote = (u32, u8, u8, u8, u64, u64, u64, u64);

fn musical_notes(song: &Song) -> Vec<MusicalNote> {
    let mut notes = song
        .notes
        .iter()
        .map(|note: &Note| {
            (
                note.part_id,
                note.channel,
                note.pitch,
                note.velocity,
                note.start_tick,
                note.end_tick,
                note.start_ms.to_bits(),
                note.end_ms.to_bits(),
            )
        })
        .collect::<Vec<_>>();
    notes.sort_unstable();
    notes
}

struct BoundedBytes(Vec<u8>);
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FILE_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other(
                "Edited MIDI exceeds the 16 MiB file safety limit.",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
