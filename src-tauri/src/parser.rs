use crate::model::{Controller, Note, Part, Song, Tempo, TimeSignature};
use midly::{Format, MetaMessage, MidiMessage, Timing, TrackEventKind};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVENTS: usize = 250_000;
pub const MAX_NOTES: usize = 100_000;
pub const MAX_TRACKS: usize = 256;
pub const MAX_DURATION_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

/// Parse once into the canonical model. IDs follow source track/event order,
/// and are not reassigned when notes are sorted for rendering and playback.
pub fn parse(bytes: &[u8]) -> Result<Song, String> {
    check_container(bytes)?;
    let (header, tracks) = midly::parse(bytes).map_err(|e| format!("Malformed MIDI: {e}"))?;
    if header.format == Format::Sequential {
        return Err("Unsupported MIDI type 2: independent sequences are not supported; export as type 0 or 1.".into());
    }
    let ppq = match header.timing {
        Timing::Metrical(value) if value.as_int() > 0 => value.as_int(),
        Timing::Metrical(_) => {
            return Err("Invalid MIDI timing: pulses per quarter must be positive.".into())
        }
        Timing::Timecode(_, _) => {
            return Err(
                "Unsupported SMPTE MIDI timing; export with pulses-per-quarter timing.".into(),
            )
        }
    };
    let mut notes: Vec<Note> = Vec::new();
    let mut parts = BTreeMap::<u32, Part>::new();
    let mut controllers = Vec::new();
    let mut raw_tempos = Vec::new();
    let mut signatures = Vec::new();
    let mut event_count = 0usize;
    let mut end_tick = 0u64;
    let mut missing_releases = 0usize;
    let mut unmatched_releases = 0usize;
    let mut ignored_expressive = false;
    let mut ignored_sysex = false;
    let mut duplicate_tempos = false;
    for (track_index, track_result) in tracks.enumerate() {
        let track = track_result.map_err(|e| format!("Malformed MIDI track: {e}"))?;
        let mut tick = 0u64;
        let mut pending = BTreeMap::<(u8, u8), VecDeque<usize>>::new();
        let mut name = String::new();
        let mut ended = false;
        for event_result in track {
            let event = event_result
                .map_err(|e| format!("Malformed MIDI event in track {}: {e}", track_index + 1))?;
            event_count += 1;
            if event_count > MAX_EVENTS {
                return Err(format!("MIDI exceeds the {MAX_EVENTS}-event safety limit."));
            }
            if ended {
                return Err(format!(
                    "Malformed MIDI: events follow End-of-Track in track {}.",
                    track_index + 1
                ));
            }
            tick = tick
                .checked_add(event.delta.as_int() as u64)
                .ok_or_else(|| "MIDI tick position overflow.".to_string())?;
            match event.kind {
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    let part_id = track_index as u32 * 16 + channel as u32;
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            if notes.len() >= MAX_NOTES {
                                return Err(format!(
                                    "MIDI exceeds the {MAX_NOTES}-note safety limit."
                                ));
                            }
                            let part = parts.entry(part_id).or_insert_with(|| Part {
                                id: part_id,
                                track: track_index as u32,
                                channel,
                                name: String::new(),
                                note_count: 0,
                                percussion: channel == 9,
                            });
                            part.note_count += 1;
                            let index = notes.len();
                            notes.push(Note {
                                id: index as u32,
                                part_id,
                                channel,
                                pitch: key.as_int(),
                                velocity: vel.as_int(),
                                start_tick: tick,
                                end_tick: tick,
                                start_ms: 0.0,
                                end_ms: 0.0,
                            });
                            pending
                                .entry((channel, key.as_int()))
                                .or_default()
                                .push_back(index);
                        }
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                            if let Some(index) = pending
                                .get_mut(&(channel, key.as_int()))
                                .and_then(VecDeque::pop_front)
                            {
                                notes[index].end_tick = tick;
                            } else {
                                unmatched_releases += 1;
                            }
                        }
                        MidiMessage::Controller { controller, value } => {
                            controllers.push(Controller {
                                part_id,
                                channel,
                                tick,
                                time_ms: 0.0,
                                controller: controller.as_int(),
                                value: value.as_int(),
                            });
                            if controller.as_int() != 64 {
                                ignored_expressive = true;
                            }
                        }
                        _ => ignored_expressive = true,
                    }
                }
                TrackEventKind::Meta(MetaMessage::TrackName(raw)) => {
                    // MIDI text has no universal encoding; use a bounded, safe UTF-8 display.
                    if name.is_empty() {
                        name = String::from_utf8_lossy(&raw[..raw.len().min(512)])
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(160)
                            .collect();
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                    let micros = value.as_int();
                    if micros == 0 {
                        return Err(
                            "Invalid MIDI tempo: microseconds per quarter must be positive.".into(),
                        );
                    }
                    raw_tempos.push((tick, track_index, event_count, micros));
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, power, _, _)) => {
                    let denominator = 1u16
                        .checked_shl(power as u32)
                        .ok_or_else(|| "Invalid MIDI time signature denominator.".to_string())?;
                    if numerator == 0 {
                        return Err("Invalid MIDI time signature numerator.".into());
                    }
                    signatures.push(TimeSignature {
                        tick,
                        numerator,
                        denominator,
                    });
                }
                TrackEventKind::Meta(MetaMessage::EndOfTrack) => ended = true,
                TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => ignored_sysex = true,
                _ => {}
            }
        }
        if !ended {
            return Err(format!(
                "Malformed MIDI: track {} has no End-of-Track event.",
                track_index + 1
            ));
        }
        for queue in pending.values() {
            for &index in queue {
                // A dangling key closes at its own track's end. For a final onset
                // at that exact tick, use one quarter note rather than a stuck voice.
                notes[index].end_tick = tick.max(notes[index].start_tick + ppq as u64);
                end_tick = end_tick.max(notes[index].end_tick);
                missing_releases += 1;
            }
        }
        for part in parts.values_mut().filter(|p| p.track == track_index as u32) {
            part.name = if name.is_empty() {
                format!("Track {} · channel {}", track_index + 1, part.channel + 1)
            } else {
                format!("{} · channel {}", name, part.channel + 1)
            };
        }
        end_tick = end_tick.max(tick);
    }
    if notes.is_empty() {
        return Err("This MIDI has no note-on events to play or practice.".into());
    }
    raw_tempos.sort_by_key(|&(tick, track, order, _)| (tick, track, order));
    let mut tempos = vec![Tempo {
        tick: 0,
        micros_per_quarter: 500_000,
        time_ms: 0.0,
    }];
    let mut previous_raw_tick = None;
    for (tick, _, _, micros_per_quarter) in raw_tempos {
        if previous_raw_tick == Some(tick) {
            duplicate_tempos = true;
        }
        previous_raw_tick = Some(tick);
        let last = tempos.last_mut().expect("default tempo exists");
        if tick == last.tick {
            last.micros_per_quarter = micros_per_quarter;
        } else {
            let time_ms = last.time_ms
                + (tick - last.tick) as f64 * last.micros_per_quarter as f64 / ppq as f64 / 1000.0;
            tempos.push(Tempo {
                tick,
                micros_per_quarter,
                time_ms,
            });
        }
    }
    let duration_ms = tick_to_ms(end_tick, ppq, &tempos);
    if !duration_ms.is_finite() || duration_ms > MAX_DURATION_MS {
        return Err("MIDI exceeds the 24-hour duration safety limit.".into());
    }
    for note in &mut notes {
        note.start_ms = tick_to_ms(note.start_tick, ppq, &tempos);
        note.end_ms = tick_to_ms(note.end_tick, ppq, &tempos);
    }
    for controller in &mut controllers {
        controller.time_ms = tick_to_ms(controller.tick, ppq, &tempos);
    }
    notes.sort_by_key(|n| (n.start_tick, n.id));
    controllers.sort_by_key(|c| (c.tick, c.part_id));
    signatures.sort_by_key(|t| t.tick);
    if signatures
        .first()
        .is_none_or(|signature| signature.tick != 0)
    {
        signatures.insert(
            0,
            TimeSignature {
                tick: 0,
                numerator: 4,
                denominator: 4,
            },
        );
    }
    let mut warnings = Vec::new();
    if missing_releases > 0 {
        warnings.push(format!("Closed {missing_releases} missing note-off(s) at track end, with a one-quarter-note minimum."));
    }
    if unmatched_releases > 0 {
        warnings.push(format!(
            "Ignored {unmatched_releases} unmatched note-off(s)."
        ));
    }
    if duplicate_tempos {
        warnings.push("Multiple tempo events at the same tick: last source track/event wins deterministically.".into());
    }
    if ignored_expressive {
        warnings.push("Piano-training playback uses the basic tone; program changes, pitch bend, pressure and controllers other than sustain are not reproduced. Original events remain in the exported MIDI.".into());
    }
    if ignored_sysex {
        warnings.push("SysEx/device commands are preserved only in the original file and are never sent to a device.".into());
    }
    if parts.values().any(|part| part.percussion) {
        warnings.push("Channel 10 is percussion and is excluded from practice by default.".into());
    }
    Ok(Song {
        ppq,
        duration_ms,
        notes,
        parts: parts.into_values().collect(),
        tempos,
        time_signatures: signatures,
        controllers,
        warnings,
    })
}

pub fn tick_to_ms(tick: u64, ppq: u16, tempos: &[Tempo]) -> f64 {
    let index = tempos
        .partition_point(|tempo| tempo.tick <= tick)
        .saturating_sub(1);
    let tempo = &tempos[index];
    tempo.time_ms
        + (tick - tempo.tick) as f64 * tempo.micros_per_quarter as f64 / ppq as f64 / 1000.0
}

/// Check complete chunk boundaries before midly's deliberately tolerant reader.
fn check_container(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("MIDI exceeds the 16 MiB file safety limit.".into());
    }
    if bytes.len() < 14 || &bytes[..4] != b"MThd" {
        return Err("Not a Standard MIDI File: missing MThd header.".into());
    }
    let header_len = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if header_len != 6 {
        return Err("Malformed MIDI: standard header length must be six bytes.".into());
    }
    let format = u16::from_be_bytes([bytes[8], bytes[9]]);
    if format == 2 {
        return Err("Unsupported MIDI type 2: export independent sequences as type 0 or 1.".into());
    }
    if format > 2 {
        return Err("Unsupported MIDI format: only types 0 and 1 are supported.".into());
    }
    let count = u16::from_be_bytes([bytes[10], bytes[11]]) as usize;
    if count == 0 || count > MAX_TRACKS {
        return Err(format!("MIDI must contain 1–{MAX_TRACKS} tracks."));
    }
    if format == 0 && count != 1 {
        return Err("Malformed MIDI type 0: exactly one track is required.".into());
    }
    let mut offset = 14usize;
    for _ in 0..count {
        if bytes.len().saturating_sub(offset) < 8 || &bytes[offset..offset + 4] != b"MTrk" {
            return Err("Malformed or truncated MIDI track header.".into());
        }
        let length = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        offset = offset
            .checked_add(8)
            .and_then(|value| value.checked_add(length))
            .ok_or_else(|| "Invalid MIDI track length.".to_string())?;
        if offset > bytes.len() {
            return Err("Truncated MIDI track data.".into());
        }
    }
    if offset != bytes.len() {
        return Err("Malformed MIDI: trailing data or a mismatched track count.".into());
    }
    Ok(())
}
