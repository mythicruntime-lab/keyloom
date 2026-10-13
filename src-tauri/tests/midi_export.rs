#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/fixtures.rs"]
mod fixtures;
#[path = "../src/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "../src/parser.rs"]
mod parser;

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

fn non_note_events(bytes: &[u8]) -> Vec<(usize, u64, TrackEventKind<'_>)> {
    let smf = Smf::parse(bytes).unwrap();
    let mut result = Vec::new();
    for (track_index, track) in smf.tracks.into_iter().enumerate() {
        let mut tick = 0;
        for event in track {
            tick += event.delta.as_int() as u64;
            if !matches!(
                event.kind,
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { .. } | MidiMessage::NoteOff { .. },
                    ..
                } | TrackEventKind::Meta(MetaMessage::EndOfTrack)
            ) {
                result.push((track_index, tick, event.kind));
            }
        }
    }
    result
}

fn note_tuples(song: &model::Song) -> Vec<(u32, u8, u8, u8, u64, u64)> {
    let mut notes = song
        .notes
        .iter()
        .map(|note| {
            (
                note.part_id,
                note.channel,
                note.pitch,
                note.velocity,
                note.start_tick,
                note.end_tick,
            )
        })
        .collect::<Vec<_>>();
    notes.sort_unstable();
    notes
}

#[test]
fn type_zero_pitch_move_keeps_non_notes_and_sustain_tie_order() {
    let original = fixtures::smf(
        0,
        480,
        &[fixtures::track(vec![
            (0, vec![0xff, 3, 4, b'T', b'e', b's', b't']),
            (0, vec![0xff, 0x51, 3, 0x07, 0xa1, 0x20]),
            (0, vec![0xff, 0x58, 4, 3, 2, 24, 8]),
            (0, vec![0xc0, 12]),
            (0, vec![0xb0, 7, 100]),
            (0, vec![0xf0, 4, 0x7d, 1, 2, 0xf7]),
            (0, vec![0xf7, 2, 0x01, 0x02]),
            (0, vec![0x90, 60, 90]),
            (120, vec![0xa0, 60, 70]),
            (0, vec![0xd0, 40]),
            (0, vec![0xe0, 1, 64]),
            (0, vec![0xb0, 64, 127]),
            (0, vec![0x80, 60, 47]),
            (0, vec![0x90, 64, 80]),
            (120, vec![0x90, 64, 0]),
            (0, vec![0xb0, 64, 0]),
            (0, vec![0xff, 0x7f, 3, 1, 2, 3]),
            (240, vec![0xff, 0x2f, 0]),
        ])],
    );
    let mut edited = parser::parse(&original).unwrap();
    edited.notes[0].pitch = 62;
    let output = exporter::edited_midi(&original, &edited).unwrap();
    assert_eq!(non_note_events(&output), non_note_events(&original));
    let smf = Smf::parse(&output).unwrap();
    assert_eq!(smf.header.format, midly::Format::SingleTrack);
    assert_eq!(smf.tracks.len(), 1);
    let messages = smf.tracks[0]
        .iter()
        .filter_map(|event| match event.kind {
            TrackEventKind::Midi { message, .. } => Some(message),
            _ => None,
        })
        .collect::<Vec<_>>();
    let sustain = messages.iter().position(|message| matches!(message,
        MidiMessage::Controller { controller, value } if controller.as_int() == 64 && value.as_int() == 127)).unwrap();
    assert!(
        matches!(messages[sustain + 1], MidiMessage::NoteOff { key, vel } if key.as_int() == 62 && vel.as_int() == 47)
    );
    assert!(
        matches!(messages[sustain + 2], MidiMessage::NoteOn { key, vel } if key.as_int() == 64 && vel.as_int() == 80)
    );
    assert!(messages.iter().any(|message| matches!(message, MidiMessage::NoteOn { key, vel } if key.as_int() == 64 && vel.as_int() == 0)));
    assert_eq!(
        note_tuples(&parser::parse(&output).unwrap()),
        note_tuples(&edited)
    );
}

#[test]
fn type_one_removal_preserves_tracks_tempo_controllers_and_original_duration() {
    let original = fixtures::demo_midi();
    let mut edited = parser::parse(&original).unwrap();
    // Remove the whole melody part, including its final onset; keep the empty
    // source track and all metadata rather than merging tracks or renumbering.
    edited.notes.retain(|note| note.part_id != 16);
    edited.notes[0].pitch += 12;
    let output = exporter::edited_midi(&original, &edited).unwrap();
    let smf = Smf::parse(&output).unwrap();
    assert_eq!(smf.header.format, midly::Format::Parallel);
    assert_eq!(smf.tracks.len(), 3);
    assert_eq!(non_note_events(&output), non_note_events(&original));
    let reloaded = parser::parse(&output).unwrap();
    assert_eq!(reloaded.duration_ms, edited.duration_ms);
    assert_eq!(reloaded.tempos, edited.tempos);
    assert_eq!(reloaded.time_signatures, edited.time_signatures);
    assert_eq!(reloaded.controllers, edited.controllers);
    assert_eq!(note_tuples(&reloaded), note_tuples(&edited));
}

#[test]
fn tempo_changes_and_sustain_survive_removal_without_retiming() {
    let original = fixtures::tempo_sustain();
    let mut edited = parser::parse(&original).unwrap();
    edited.notes.remove(0);
    let output = exporter::edited_midi(&original, &edited).unwrap();
    let reloaded = parser::parse(&output).unwrap();
    assert_eq!(non_note_events(&output), non_note_events(&original));
    assert_eq!(reloaded.tempos, edited.tempos);
    assert_eq!(reloaded.controllers, edited.controllers);
    assert_eq!(reloaded.notes[0].start_ms, edited.notes[0].start_ms);
    assert_eq!(reloaded.notes[0].end_ms, edited.notes[0].end_ms);
    assert_eq!(reloaded.duration_ms, edited.duration_ms);
}

#[test]
fn overlapping_fifo_notes_and_missing_releases_roundtrip() {
    let original = fixtures::overlap_and_missing_release();
    let edited = parser::parse(&original).unwrap();
    let output = exporter::edited_midi(&original, &edited).unwrap();
    let reloaded = parser::parse(&output).unwrap();
    assert_eq!(note_tuples(&reloaded), note_tuples(&edited));
    assert_eq!(reloaded.duration_ms, edited.duration_ms);
    assert!(!reloaded
        .warnings
        .iter()
        .any(|warning| warning.contains("missing note-off")));
    // The source canonical duration stays intact even if its dangling note is removed.
    let mut removed = edited.clone();
    removed.notes.pop();
    let output = exporter::edited_midi(&original, &removed).unwrap();
    assert_eq!(
        parser::parse(&output).unwrap().duration_ms,
        edited.duration_ms
    );
}

#[test]
fn zero_duration_and_same_tick_repeated_notes_keep_their_pairing() {
    let original = fixtures::smf(
        0,
        480,
        &[fixtures::track(vec![
            (0, vec![0x90, 60, 90]),
            (0, vec![0x80, 60, 0]),
            (0, vec![0x90, 60, 80]),
            (120, vec![0x80, 60, 0]),
            (0, vec![0x90, 60, 70]),
            (120, vec![0x80, 60, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    );
    let mut edited = parser::parse(&original).unwrap();
    for note in &mut edited.notes {
        note.pitch = 67;
    }
    let output = exporter::edited_midi(&original, &edited).unwrap();
    assert_eq!(
        note_tuples(&parser::parse(&output).unwrap()),
        note_tuples(&edited)
    );
}

#[test]
fn tracks_sharing_a_channel_stay_separate() {
    let original = fixtures::smf(
        1,
        480,
        &[
            fixtures::track(vec![
                (0, vec![0x90, 60, 90]),
                (480, vec![0x80, 60, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
            fixtures::track(vec![
                (120, vec![0x90, 64, 70]),
                (120, vec![0x80, 64, 0]),
                (240, vec![0xff, 0x2f, 0]),
            ]),
        ],
    );
    let mut edited = parser::parse(&original).unwrap();
    edited.notes[1].pitch = 60;
    let output = exporter::edited_midi(&original, &edited).unwrap();
    assert_eq!(
        note_tuples(&parser::parse(&output).unwrap()),
        note_tuples(&edited)
    );
    assert_eq!(Smf::parse(&output).unwrap().tracks.len(), 2);
}

#[test]
fn nested_same_pitch_merge_is_rejected_instead_of_shortening_the_wrong_note() {
    let original = fixtures::smf(
        0,
        480,
        &[fixtures::track(vec![
            (0, vec![0x90, 60, 90]),
            (120, vec![0x90, 64, 70]),
            (120, vec![0x80, 64, 0]),
            (240, vec![0x80, 60, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    );
    let mut edited = parser::parse(&original).unwrap();
    edited.notes[1].pitch = 60;
    let error = exporter::edited_midi(&original, &edited).unwrap_err();
    assert!(error.contains("ambiguous MIDI releases"), "{error}");
}

#[test]
fn unmatched_source_release_cannot_shorten_a_note_moved_onto_its_pitch() {
    let original = fixtures::smf(
        0,
        480,
        &[fixtures::track(vec![
            (0, vec![0x90, 60, 90]),
            (240, vec![0x80, 64, 0]),
            (240, vec![0x80, 60, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    );
    let mut edited = parser::parse(&original).unwrap();
    edited.notes[0].pitch = 64;
    let output = exporter::edited_midi(&original, &edited).unwrap();
    assert_eq!(
        note_tuples(&parser::parse(&output).unwrap()),
        note_tuples(&edited)
    );
}

#[test]
fn empty_invalid_or_oversized_edits_are_rejected() {
    let original = fixtures::single_note(1);
    let valid = parser::parse(&original).unwrap();
    let mut edited = valid.clone();
    edited.notes.clear();
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("no notes"));
    edited = valid.clone();
    edited.notes[0].pitch = 128;
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("0–127"));
    edited = valid.clone();
    edited.notes[0].end_tick += 1;
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("timing"));
    edited = valid.clone();
    edited.notes.push(edited.notes[0].clone());
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("Duplicate"));
    edited = valid.clone();
    edited.notes = vec![edited.notes[0].clone(); parser::MAX_NOTES + 1];
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("note safety limit"));
    let mut oversized = original;
    oversized.resize(parser::MAX_FILE_BYTES + 1, 0);
    assert!(exporter::edited_midi(&oversized, &valid)
        .unwrap_err()
        .contains("16 MiB"));
}

#[test]
fn removal_that_creates_an_unrepresentable_delta_is_rejected() {
    let gap = (1 << 28) - 1;
    let original = fixtures::smf(
        0,
        32_767,
        &[fixtures::track(vec![
            (0, vec![0xff, 0x51, 3, 0, 0, 1]),
            (0, vec![0x90, 60, 90]),
            (1, vec![0x80, 60, 0]),
            (gap, vec![0x90, 64, 80]),
            (gap, vec![0x80, 64, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    );
    let mut edited = parser::parse(&original).unwrap();
    edited.notes.pop();
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("28-bit delta limit"));
}

#[test]
fn generated_releases_cannot_exceed_event_safety_limit() {
    let mut events = vec![(0, vec![0x90, 60, 90])];
    // A source exactly at the event limit is valid, but materializing its
    // missing release would need one additional event.
    events.extend((0..parser::MAX_EVENTS - 2).map(|_| (0, vec![0xff, 1, 0])));
    events.push((0, vec![0xff, 0x2f, 0]));
    let original = fixtures::smf(0, 480, &[fixtures::track(events)]);
    let edited = parser::parse(&original).unwrap();
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("event safety limit"));
}

#[test]
fn generated_release_cannot_push_output_past_byte_limit() {
    // Header + track header (22), delta/status/length (6), On (4), EOT (4).
    let payload_len = parser::MAX_FILE_BYTES - 36;
    let mut track = vec![
        0,
        0xf0,
        ((payload_len >> 21) as u8 & 0x7f) | 0x80,
        ((payload_len >> 14) as u8 & 0x7f) | 0x80,
        ((payload_len >> 7) as u8 & 0x7f) | 0x80,
        (payload_len as u8) & 0x7f,
    ];
    track.resize(6 + payload_len, 0);
    *track.last_mut().unwrap() = 0xf7;
    track.extend([0, 0x90, 60, 90, 0, 0xff, 0x2f, 0]);
    let original = fixtures::smf(0, 480, &[track]);
    assert_eq!(original.len(), parser::MAX_FILE_BYTES);
    let edited = parser::parse(&original).unwrap();
    assert!(exporter::edited_midi(&original, &edited)
        .unwrap_err()
        .contains("16 MiB"));
}
