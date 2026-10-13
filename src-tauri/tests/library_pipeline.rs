#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/fixtures.rs"]
mod fixtures;
#[path = "../src/library.rs"]
mod library;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/parser.rs"]
mod parser;
#[path = "../src/transport.rs"]
mod transport;

use library::Library;
use model::{LessonToolRequest, PartSetting, ThinStrategy};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "keyloom-library-test-{}-{time}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_import_query_open_play_duplicate_source_move_restart_and_remove() {
    let directory = TestDirectory::new();
    let root = directory.path().join("managed library");
    let source = directory.write("Downloads 日本語/easy melody.midi", &fixtures::demo_midi());
    let mut library = Library::open(root.clone()).unwrap();
    assert!(library
        .list("unmatched active filter", true)
        .unwrap()
        .is_empty());
    let result = library.import_path(&source).unwrap();
    assert!(!result.duplicate);
    assert_eq!(library.list("", false).unwrap()[0].id, result.entry.id);
    assert!(library
        .list("unmatched active filter", true)
        .unwrap()
        .is_empty());
    // Direct Open is by the returned ID and bypasses all current library filters.
    let song = library.load(&result.entry.id).unwrap();
    let mut engine = transport::Engine::new();
    engine.load(song.clone(), library.settings(&result.entry.id).unwrap());
    engine.play();
    let voices = engine.advance(100.0);
    assert!(engine.snapshot().playing);
    assert!(engine.snapshot().position_ms > 0.0);
    assert!(
        !voices.is_empty(),
        "real imported song must schedule playback voices"
    );
    let second_source =
        directory.write("same bytes different filename.MID", &fixtures::demo_midi());
    for _ in 0..20 {
        let duplicate = library.import_path(&second_source).unwrap();
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.entry.id, result.entry.id);
    }
    assert_eq!(library.list("", false).unwrap().len(), 1);
    let second = directory.write(
        "different folder/easy melody.midi",
        &fixtures::single_note(7),
    );
    let other = library.import_path(&second).unwrap();
    assert_ne!(other.entry.id, result.entry.id);
    assert_eq!(library.list("", false).unwrap().len(), 2);
    let mut entry = result.entry.clone();
    entry.title = "夜の Original exercise".into();
    entry.artist = "Local author".into();
    entry.tags = "game, scales".into();
    entry.arrangement = "Easy two parts".into();
    entry.favorite = true;
    library.update_metadata(&entry).unwrap();
    let mut settings = library.settings(&entry.id).unwrap();
    settings.speed = 0.75;
    settings.mode = "wait".into();
    settings.position_ms = 250.0;
    settings.loop_start_ms = Some(200.0);
    settings.loop_end_ms = Some(1200.0);
    settings.split_pitch = Some(60);
    settings.count_in = true;
    settings.metronome = true;
    settings.parts.push(PartSetting {
        part_id: song.parts[0].id,
        role: "practice".into(),
        hand: "right".into(),
        volume: 0.8,
        solo: true,
    });
    library.save_settings(&entry.id, &settings).unwrap();
    library.mark_played(&entry.id).unwrap();
    fs::rename(&source, directory.path().join("moved source.mid")).unwrap();
    fs::remove_file(&second_source).unwrap();
    assert_eq!(library.load(&entry.id).unwrap(), song);
    drop(library);
    let mut library = Library::open(root).unwrap();
    assert_eq!(library.list("", false).unwrap().len(), 2);
    assert_eq!(library.settings(&entry.id).unwrap(), settings);
    let restored = library.list("LOCAL AUTHOR", true).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].title, entry.title);
    assert_eq!(restored[0].tags, entry.tags);
    assert!(restored[0].last_played.is_some());
    assert_eq!(library.list("夜", true).unwrap().len(), 1);
    assert_eq!(library.list("scales", false).unwrap().len(), 1);
    let export = directory.path().join("exact exported.mid");
    library.export(&entry.id, &export).unwrap();
    assert_eq!(fs::read(export).unwrap(), fixtures::demo_midi());
    let original = directory.path().join("moved source.mid");
    assert!(library
        .export(&entry.id, &original)
        .unwrap_err()
        .contains("new destination"));
    library.remove(&entry.id).unwrap();
    assert!(original.exists());
    assert!(library.load(&entry.id).is_err());
    assert_eq!(library.list("", false).unwrap().len(), 1);
}

#[test]
fn tempo_map_release_sustain_and_stable_ids() {
    let song = parser::parse(&fixtures::tempo_sustain()).unwrap();
    assert_eq!(song.ppq, 480);
    assert_eq!(song.notes[0].end_tick, 240);
    assert_eq!(
        song.notes[0].end_ms, 250.0,
        "key release stays independent of pedal"
    );
    assert_eq!(song.notes[1].start_ms, 500.0);
    assert_eq!(song.notes[1].end_ms, 1500.0);
    assert_eq!(song.duration_ms, 1500.0);
    assert_eq!(
        song.controllers
            .iter()
            .filter(|c| c.controller == 64)
            .count(),
        2
    );
    assert_eq!(song, parser::parse(&fixtures::tempo_sustain()).unwrap());
    let overlaps = parser::parse(&fixtures::overlap_and_missing_release()).unwrap();
    assert_eq!(
        (overlaps.notes[0].start_tick, overlaps.notes[0].end_tick),
        (0, 240)
    );
    assert_eq!(
        (overlaps.notes[1].start_tick, overlaps.notes[1].end_tick),
        (120, 360)
    );
    assert_eq!(overlaps.notes[2].end_tick, 960);
    assert!(overlaps
        .warnings
        .iter()
        .any(|warning| warning.contains("missing note-off")));
}

#[test]
fn simultaneous_tempos_use_documented_source_order() {
    let song = parser::parse(&fixtures::smf(
        1,
        480,
        &[
            fixtures::track(vec![
                (0, vec![0xff, 0x51, 3, 0x07, 0xa1, 0x20]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
            fixtures::track(vec![
                (0, vec![0xff, 0x51, 3, 0x0f, 0x42, 0x40]),
                (0, vec![0x90, 60, 80]),
                (480, vec![0x80, 60, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
        ],
    ))
    .unwrap();
    assert_eq!(song.tempos.len(), 1);
    assert_eq!(song.duration_ms, 1000.0);
    assert!(song
        .warnings
        .iter()
        .any(|warning| warning.contains("Multiple tempo")));
}

#[test]
fn malformed_events_and_bounded_byte_mutations_do_not_panic() {
    let incomplete_event = fixtures::smf(0, 480, &[vec![0, 0x90, 60]]);
    assert!(parser::parse(&incomplete_event).is_err());
    let invalid_data = fixtures::smf(0, 480, &[vec![0, 0x90, 0xff, 80, 0, 0xff, 0x2f, 0]]);
    assert!(parser::parse(&invalid_data).is_err());
    let good = fixtures::demo_midi();
    for length in 0..good.len() {
        let result = std::panic::catch_unwind(|| parser::parse(&good[..length]));
        assert!(result.is_ok(), "parser panicked for truncation at {length}");
        assert!(result.unwrap().is_err());
    }
    for index in 0..good.len() {
        let mut bytes = good.clone();
        bytes[index] ^= 0xff;
        assert!(
            std::panic::catch_unwind(|| parser::parse(&bytes)).is_ok(),
            "parser panicked for mutation at {index}"
        );
    }
    let mut too_many_notes = vec![(0, vec![0x90, 60, 80]); parser::MAX_NOTES + 1];
    too_many_notes.push((0, vec![0xff, 0x2f, 0]));
    assert!(
        parser::parse(&fixtures::smf(0, 480, &[fixtures::track(too_many_notes)]))
            .unwrap_err()
            .contains("note safety")
    );
    let tracks = vec![vec![0, 0xff, 0x2f, 0]; parser::MAX_TRACKS + 1];
    assert!(parser::parse(&fixtures::smf(1, 480, &tracks))
        .unwrap_err()
        .contains("256 tracks"));
}

#[test]
fn track_channel_boundaries_chords_percussion_and_running_status() {
    let bytes = fixtures::smf(
        1,
        480,
        &[
            fixtures::track(vec![(0, vec![0x90, 60, 80]), (120, vec![0xff, 0x2f, 0])]),
            fixtures::track(vec![
                (0, vec![0x80, 60, 0]),
                (0, vec![0x90, 64, 80]),
                (0, vec![67, 70]),
                (480, vec![64, 0]),
                (0, vec![67, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
            fixtures::track(vec![
                (0, vec![0x99, 36, 80]),
                (240, vec![0x89, 36, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
        ],
    );
    let song = parser::parse(&bytes).unwrap();
    assert_eq!(song.parts.len(), 3);
    assert_eq!(
        song.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        [0, 16, 41]
    );
    assert!(song.parts[2].percussion);
    assert_eq!(
        song.notes
            .iter()
            .find(|note| note.id == 0)
            .unwrap()
            .end_tick,
        480
    );
    assert_eq!(
        song.notes
            .iter()
            .filter(|note| note.part_id == 16 && note.start_tick == 0)
            .count(),
        2
    );
    assert!(song
        .warnings
        .iter()
        .any(|warning| warning.contains("unmatched note-off")));
}

#[test]
fn malformed_unsupported_and_extreme_files_fail_without_ready_entries() {
    let directory = TestDirectory::new();
    let mut library = Library::open(directory.path().join("library")).unwrap();
    let mut truncated = fixtures::single_note(1);
    truncated.pop();
    let track = fixtures::track(vec![
        (0, vec![0x90, 60, 80]),
        (480, vec![0x80, 60, 0]),
        (0, vec![0xff, 0x2f, 0]),
    ]);
    let cases = [
        ("corrupt.mid", b"not MIDI".to_vec(), "MThd"),
        ("truncated.mid", truncated, "Truncated"),
        (
            "type2.mid",
            fixtures::smf(2, 480, std::slice::from_ref(&track)),
            "type 2",
        ),
        ("smpte.mid", fixtures::smf(0, 0xe728, &[track]), "SMPTE"),
        (
            "zero ppq.mid",
            fixtures::smf(0, 0, &[fixtures::track(vec![(0, vec![0xff, 0x2f, 0])])]),
            "positive",
        ),
        (
            "no end.mid",
            fixtures::smf(0, 480, &[fixtures::track(vec![(0, vec![0x90, 60, 80])])]),
            "End-of-Track",
        ),
        (
            "empty.mid",
            fixtures::smf(0, 480, &[fixtures::track(vec![(0, vec![0xff, 0x2f, 0])])]),
            "no note-on",
        ),
        (
            "bad tempo.mid",
            fixtures::smf(
                0,
                480,
                &[fixtures::track(vec![
                    (0, vec![0xff, 0x51, 3, 0, 0, 0]),
                    (0, vec![0xff, 0x2f, 0]),
                ])],
            ),
            "tempo",
        ),
        (
            "too long.mid",
            fixtures::smf(
                0,
                1,
                &[fixtures::track(vec![
                    (0, vec![0x90, 60, 80]),
                    (0x0fff_ffff, vec![0x80, 60, 0]),
                    (0, vec![0xff, 0x2f, 0]),
                ])],
            ),
            "24-hour",
        ),
    ];
    for (name, bytes, expected) in cases {
        let path = directory.write(name, &bytes);
        let error = library.import_path(&path).unwrap_err();
        assert!(error.contains(expected), "{name}: {error}");
        assert!(library.list("", false).unwrap().is_empty());
    }
    let valid = directory.write("valid.mid", &fixtures::single_note(3));
    assert!(
        library.import_path(&valid).is_ok(),
        "failed batch files must not poison following imports"
    );
    assert!(library.load("../../outside").is_err());
    assert!(library.remove("../outside").is_err());
    assert!(parser::parse(&vec![0; parser::MAX_FILE_BYTES + 1])
        .unwrap_err()
        .contains("16 MiB"));
    let mut events = vec![(0, vec![0xb0, 1, 0]); parser::MAX_EVENTS + 1];
    events.push((0, vec![0xff, 0x2f, 0]));
    assert!(
        parser::parse(&fixtures::smf(0, 480, &[fixtures::track(events)]))
            .unwrap_err()
            .contains("event safety")
    );
}

#[test]
fn interrupted_import_recovery_never_exposes_partial_or_missing_copies() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("safe original.mid", &fixtures::single_note(1));
    let mut library = Library::open(root.clone()).unwrap();
    let id = library.import_path(&source).unwrap().entry.id;
    let mut metadata = library.entry(&id).unwrap();
    metadata.title = "Keep manual title after recovery".into();
    library.update_metadata(&metadata).unwrap();
    drop(library);
    let partial = root.join("midi").join(format!("{}.part", "a".repeat(64)));
    let orphan = root.join("midi").join(format!("{}.mid", "b".repeat(64)));
    fs::write(&partial, b"incomplete copy").unwrap();
    fs::write(&orphan, fixtures::single_note(2)).unwrap();
    fs::remove_file(root.join("midi").join(format!("{id}.mid"))).unwrap();
    let mut library = Library::open(root.clone()).unwrap();
    assert!(!partial.exists());
    assert!(!orphan.exists());
    assert!(library.list("", false).unwrap().is_empty());
    assert!(library.load(&id).is_err());
    let recovered = library.import_path(&source).unwrap();
    assert_eq!(recovered.entry.id, id);
    assert_eq!(recovered.entry.title, metadata.title);
    assert_eq!(library.list("", false).unwrap().len(), 1);
    drop(library);
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.execute("UPDATE songs SET state='deleting' WHERE id=?1", [&id])
        .unwrap();
    drop(db);
    let library = Library::open(root.clone()).unwrap();
    assert!(library.list("", false).unwrap().is_empty());
    assert!(!root.join("midi").join(format!("{id}.mid")).exists());
    assert!(source.exists());
}

#[test]
fn invalid_settings_do_not_overwrite_last_valid_state() {
    let directory = TestDirectory::new();
    let source = directory.write("settings.mid", &fixtures::single_note(1));
    let mut library = Library::open(directory.path().join("library")).unwrap();
    let id = library.import_path(&source).unwrap().entry.id;
    let initial = library.settings(&id).unwrap();
    let mut invalid = initial.clone();
    invalid.speed = f64::NAN;
    assert!(library.save_settings(&id, &invalid).is_err());
    invalid = initial.clone();
    invalid.loop_start_ms = Some(450.0);
    invalid.loop_end_ms = Some(400.0);
    assert!(library.save_settings(&id, &invalid).is_err());
    assert_eq!(library.settings(&id).unwrap(), initial);
}

#[test]
fn long_unicode_paths_are_supported_without_source_mutation() {
    let directory = TestDirectory::new();
    let mut source_folder = directory.path().to_path_buf();
    for index in 0..8 {
        source_folder.push(format!("long folder 日本語 exercise {index}"));
    }
    fs::create_dir_all(&source_folder).unwrap();
    let source = source_folder.join("小さい 練習曲.mid");
    fs::write(&source, fixtures::single_note(99)).unwrap();
    assert!(source.as_os_str().len() > 260);
    let mut library = Library::open(directory.path().join("library")).unwrap();
    let result = library.import_path(&source).unwrap();
    assert_eq!(result.entry.title, "小さい 練習曲");
    library.remove(&result.entry.id).unwrap();
    assert_eq!(fs::read(source).unwrap(), fixtures::single_note(99));
}

#[test]
#[ignore = "bounded opt-in disk workload; run explicitly for M3 measurements"]
fn generated_thousand_song_library_search_benchmark() {
    let directory = TestDirectory::new();
    let mut library = Library::open(directory.path().join("library")).unwrap();
    let start = Instant::now();
    let mut import_timings = Vec::new();
    for index in 0..1000 {
        let source = directory.write(
            &format!("sources/Original exercise {index:04}.mid"),
            &fixtures::single_note(index),
        );
        let imported = library.import_path(&source).unwrap();
        import_timings.push(imported.elapsed_ms);
    }
    assert_eq!(library.list("", false).unwrap().len(), 1000);
    library.list("exercise 09", false).unwrap();
    let search_start = Instant::now();
    let found = library.list("exercise 09", false).unwrap();
    let search_ms = search_start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(found.len(), 100);
    assert!(library.load(&found[0].id).is_ok());
    import_timings.sort_by(f64::total_cmp);
    eprintln!("1000-song real managed pipeline: total {:.1}s; import p50 {:.3}ms p95 {:.3}ms; warm 100-result search {:.3}ms",
        start.elapsed().as_secs_f64(), import_timings[500], import_timings[950], search_ms);
    // Timings are observations, not pass/fail guarantees on a shared workstation.
}

fn original_row(root: &Path, id: &str) -> Vec<rusqlite::types::Value> {
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    let mut statement = db.prepare("SELECT * FROM songs WHERE id=?1").unwrap();
    let columns = statement.column_count();
    statement
        .query_row([id], |row| {
            (0..columns).map(|index| row.get(index)).collect()
        })
        .unwrap()
}

fn sparse_note_fixture() -> Vec<u8> {
    let mut events = Vec::new();
    for pitch in [60, 64, 67, 72] {
        events.push((0, vec![0x90, pitch, 80]));
        events.push((120, vec![0x80, pitch, 0]));
    }
    events.push((0, vec![0xff, 0x2f, 0]));
    fixtures::smf(0, 480, &[fixtures::track(events)])
}

#[test]
fn variant_sparse_ids_run_real_transport_without_mutating_original_row_or_bytes() {
    use sha2::{Digest, Sha256};
    use transport::{InputSource, VoiceEvent};
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let bytes = sparse_note_fixture();
    let source = directory.write("My original.mid", &bytes);
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let canonical = library.load(&original.id).unwrap();
    let before = original_row(&root, &original.id);
    let variant = library
        .create_variant(&original.id, "少ない notes")
        .unwrap();
    assert_eq!(original.original_id, None);
    assert_eq!(variant.original_id.as_deref(), Some(original.id.as_str()));
    assert_eq!(library.variant_state(&original.id).unwrap(), None);
    library.remove_notes(&variant.id, 0, &[0, 2]).unwrap();
    let edited = library.load(&variant.id).unwrap();
    assert_eq!(
        edited.notes.iter().map(|note| note.id).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(edited.parts[0].note_count, 2);
    assert_eq!(edited.parts[0].id, canonical.parts[0].id);
    assert_eq!(edited.duration_ms, canonical.duration_ms);
    assert_eq!(edited.tempos, canonical.tempos);
    assert_eq!(edited.time_signatures, canonical.time_signatures);
    assert_eq!(edited.controllers, canonical.controllers);
    assert_eq!(library.entry(&variant.id).unwrap().note_count, 2);
    assert_eq!(library.list("少ない", false).unwrap()[0].id, variant.id);

    let mut engine = transport::Engine::new();
    engine.load(edited.clone(), library.settings(&variant.id).unwrap());
    engine.play();
    let events = engine.advance(canonical.duration_ms + 50.0);
    let on_ids: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::On { key, .. } => Some(*key),
            _ => None,
        })
        .collect();
    let off_ids: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::Off { key } => Some(*key),
            _ => None,
        })
        .collect();
    assert_eq!(on_ids, vec![1, 3]);
    assert_eq!(off_ids, vec![1, 3]);

    let mut settings = library.settings(&variant.id).unwrap();
    settings.mode = "wait".into();
    settings.speed = 0.75;
    library.save_settings(&variant.id, &settings).unwrap();
    engine.load(edited, settings);
    engine.play();
    engine.advance(1000.0);
    assert_eq!(engine.guide_targets().unwrap().1, vec![64]);
    engine.input(60, true, 80, InputSource::Physical);
    assert!(engine.snapshot().waiting);
    assert_eq!(engine.snapshot().correct, 0);
    engine.input(64, true, 80, InputSource::Physical);
    engine.input(64, false, 0, InputSource::Physical);
    engine.advance(1000.0);
    assert_eq!(engine.guide_targets().unwrap().1, vec![72]);
    engine.input(72, true, 80, InputSource::Physical);
    assert_eq!(engine.snapshot().correct, 2);
    library.mark_played(&variant.id).unwrap();
    assert!(library.entry(&variant.id).unwrap().last_played.is_some());

    assert_eq!(library.load(&original.id).unwrap(), canonical);
    assert_eq!(original_row(&root, &original.id), before);
    let managed = fs::read(root.join("midi").join(format!("{}.mid", original.id))).unwrap();
    assert_eq!(managed, bytes);
    assert_eq!(fs::read(source).unwrap(), bytes);
    let digest: String = Sha256::digest(&managed)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(digest, original.id);
    let exported = directory.path().join("original export.mid");
    library.export(&original.id, &exported).unwrap();
    assert_eq!(fs::read(exported).unwrap(), bytes);
}

#[test]
fn variants_restart_undo_independent_settings_and_copy_current_variant_view() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("pedal original.mid", &fixtures::tempo_sustain());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let canonical = library.load(&original.id).unwrap();
    let original_settings = library.settings(&original.id).unwrap();
    let first = library
        .create_variant(&original.id, "First lesson")
        .unwrap();
    library
        .remove_notes(&first.id, 0, &[canonical.notes[0].id])
        .unwrap();
    let mut settings = library.settings(&first.id).unwrap();
    settings.speed = 0.5;
    settings.mode = "wait".into();
    settings.position_ms = 125.0;
    settings.loop_start_ms = Some(0.0);
    settings.loop_end_ms = Some(1000.0);
    library.save_settings(&first.id, &settings).unwrap();
    let mut first_metadata = library.entry(&first.id).unwrap();
    first_metadata.tags = "private lesson tag".into();
    first_metadata.favorite = true;
    // The public metadata object cannot retarget a variant to another original.
    first_metadata.original_id = Some("f".repeat(64));
    library.update_metadata(&first_metadata).unwrap();
    let second = library.create_variant(&first.id, "Second lesson").unwrap();
    assert_eq!(second.original_id.as_deref(), Some(original.id.as_str()));
    assert_eq!(library.settings(&second.id).unwrap(), settings);
    assert_eq!(
        library.load(&first.id).unwrap(),
        library.load(&second.id).unwrap()
    );
    assert_eq!(
        library.load(&first.id).unwrap().controllers,
        canonical.controllers
    );
    assert_eq!(
        library.variant_state(&second.id).unwrap().unwrap(),
        model::LessonState {
            revision: 0,
            can_undo: false,
            removed_count: 1,
            shifted_count: 0
        }
    );
    assert!(library.undo_notes(&second.id, 0).is_err());
    let remaining = library.load(&second.id).unwrap().notes[0].id;
    library.remove_notes(&second.id, 0, &[remaining]).unwrap();
    library.undo_notes(&second.id, 1).unwrap();
    let mut second_settings = settings.clone();
    second_settings.speed = 1.25;
    library.save_settings(&second.id, &second_settings).unwrap();
    assert_eq!(library.settings(&first.id).unwrap(), settings);
    assert_eq!(library.settings(&original.id).unwrap(), original_settings);
    let versions = library.versions(&second.id).unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0].id, original.id);
    assert_eq!(library.list("private lesson tag", true).unwrap().len(), 2);
    drop(library);

    let mut library = Library::open(root).unwrap();
    assert_eq!(library.settings(&second.id).unwrap(), second_settings);
    assert_eq!(
        library.variant_state(&first.id).unwrap().unwrap(),
        model::LessonState {
            revision: 1,
            can_undo: true,
            removed_count: 1,
            shifted_count: 0
        }
    );
    library.undo_notes(&first.id, 1).unwrap();
    assert_eq!(library.load(&first.id).unwrap(), canonical);
    assert_eq!(
        library.variant_state(&first.id).unwrap().unwrap(),
        model::LessonState {
            revision: 2,
            can_undo: false,
            removed_count: 0,
            shifted_count: 0
        }
    );
    assert_eq!(
        library
            .variant_state(&second.id)
            .unwrap()
            .unwrap()
            .removed_count,
        1
    );
    assert!(library.undo_notes(&first.id, 1).is_err());
    assert!(library.undo_notes(&first.id, 2).is_err());
}

#[test]
fn variant_edit_validation_and_stale_revisions_are_atomic() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("original.mid", &sparse_note_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library.create_variant(&original.id, "Editable").unwrap();
    let initial = library.load(&variant.id).unwrap();
    assert!(library
        .remove_notes(&original.id, 0, &[0])
        .unwrap_err()
        .contains("originals are read-only"));
    for title in [" ".to_string(), "bad\0title".into(), "x".repeat(2049)] {
        assert!(library.create_variant(&original.id, &title).is_err());
    }
    for ids in [vec![], vec![0, 0], vec![u32::MAX], vec![0, u32::MAX]] {
        assert!(library.remove_notes(&variant.id, 0, &ids).is_err());
        assert_eq!(library.load(&variant.id).unwrap(), initial);
        assert_eq!(
            library
                .variant_state(&variant.id)
                .unwrap()
                .unwrap()
                .revision,
            0
        );
    }
    assert!(library.remove_notes(&variant.id, 999, &[0]).is_err());
    library.remove_notes(&variant.id, 0, &[0]).unwrap();
    let after_first = library.load(&variant.id).unwrap();
    assert!(library.remove_notes(&variant.id, 1, &[0, 1]).is_err());
    assert_eq!(library.load(&variant.id).unwrap(), after_first);
    let mut second_editor = Library::open(root).unwrap();
    assert_eq!(
        second_editor
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    library.remove_notes(&variant.id, 1, &[1]).unwrap();
    assert!(second_editor.undo_notes(&variant.id, 1).is_err());
    assert!(second_editor.remove_notes(&variant.id, 1, &[2]).is_err());
    assert_eq!(
        second_editor.variant_state(&variant.id).unwrap().unwrap(),
        model::LessonState {
            revision: 2,
            can_undo: true,
            removed_count: 2,
            shifted_count: 0
        }
    );
    for id in [
        "variant-0",
        "variant--1",
        "variant-01",
        "variant-+1",
        "variant-1/../../outside",
        "variant-9223372036854775808",
    ] {
        assert!(library.load(id).is_err(), "accepted {id}");
        assert!(library.validate_removal(id).is_err(), "accepted {id}");
    }
    assert_eq!(library.list("", false).unwrap().len(), 2);
}

#[test]
fn original_deletion_guards_and_missing_copy_recovery_preserve_variants() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("original.mid", &sparse_note_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Recoverable lesson")
        .unwrap();
    let sibling = library
        .create_variant(&variant.id, "Can delete while hidden")
        .unwrap();
    library.remove_notes(&variant.id, 0, &[0, 2]).unwrap();
    let view = library.load(&variant.id).unwrap();
    let row_before = original_row(&root, &original.id);
    assert!(library
        .validate_removal(&original.id)
        .unwrap_err()
        .contains("practice variants"));
    assert!(library.remove(&original.id).is_err());
    assert_eq!(library.load(&variant.id).unwrap(), view);
    assert_eq!(original_row(&root, &original.id), row_before);
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    assert!(db
        .execute(
            "UPDATE songs SET state='deleting' WHERE id=?1",
            [&original.id]
        )
        .is_err());
    assert!(db
        .execute("DELETE FROM songs WHERE id=?1", [&original.id])
        .is_err());
    assert_eq!(original_row(&root, &original.id), row_before);
    drop(db);
    drop(library);

    let managed = root.join("midi").join(format!("{}.mid", original.id));
    fs::remove_file(&managed).unwrap();
    let mut library = Library::open(root.clone()).unwrap();
    assert!(library.list("", false).unwrap().is_empty());
    assert!(library.load(&variant.id).is_err());
    assert!(library.remove(&original.id).is_err());
    library.remove(&sibling.id).unwrap();
    let recovered = library.import_path(&source).unwrap();
    assert_eq!(recovered.entry.id, original.id);
    assert_eq!(library.list("", false).unwrap().len(), 2);
    assert_eq!(library.load(&variant.id).unwrap(), view);
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    library.undo_notes(&variant.id, 1).unwrap();
    library.remove(&variant.id).unwrap();
    assert_eq!(fs::read(&managed).unwrap(), sparse_note_fixture());
    assert_eq!(library.versions(&original.id).unwrap().len(), 1);
    let replacement = library.create_variant(&original.id, "Fresh ID").unwrap();
    assert_ne!(replacement.id, variant.id);
    assert_ne!(replacement.id, sibling.id);
    library.remove(&replacement.id).unwrap();
    drop(library);
    fs::remove_file(&managed).unwrap();
    let mut library = Library::open(root).unwrap();
    library.validate_removal(&original.id).unwrap();
    library.remove(&original.id).unwrap();
    assert_eq!(fs::read(source).unwrap(), sparse_note_fixture());
}

#[test]
fn empty_variant_is_safe_for_every_practice_mode_and_export_never_writes() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("original.mid", &fixtures::tempo_sustain());
    let mut library = Library::open(root).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Empty lesson")
        .unwrap();
    let original_song = library.load(&original.id).unwrap();
    let ids: Vec<_> = original_song.notes.iter().map(|note| note.id).collect();
    library.remove_notes(&variant.id, 0, &ids).unwrap();
    let empty = library.load(&variant.id).unwrap();
    assert!(empty.notes.is_empty());
    assert!(empty.parts.iter().all(|part| part.note_count == 0));
    assert_eq!(empty.parts.len(), original_song.parts.len());
    assert_eq!(empty.duration_ms, original_song.duration_ms);
    assert_eq!(empty.controllers, original_song.controllers);
    assert_eq!(library.entry(&variant.id).unwrap().note_count, 0);
    for mode in ["listen", "wait", "timed"] {
        let settings = model::SongSettings {
            mode: mode.into(),
            ..Default::default()
        };
        library.save_settings(&variant.id, &settings).unwrap();
        let mut engine = transport::Engine::new();
        engine.load(empty.clone(), library.settings(&variant.id).unwrap());
        engine.play();
        let events = engine.advance(empty.duration_ms + 100.0);
        assert!(!events
            .iter()
            .any(|event| matches!(event, transport::VoiceEvent::On { .. })));
        assert!(!engine.snapshot().playing);
        assert!(!engine.snapshot().waiting);
        assert!(engine.guide_targets().is_none());
    }
    let destination = directory.path().join("must not exist.mid");
    assert!(library
        .export(&variant.id, &destination)
        .unwrap_err()
        .contains("no notes to export"));
    assert!(!destination.exists());
    let existing = directory.write("untouched.mid", b"existing external file");
    assert!(library.export(&variant.id, &existing).is_err());
    assert_eq!(fs::read(existing).unwrap(), b"existing external file");
    library.undo_notes(&variant.id, 1).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), original_song);
}

#[test]
fn variant_undo_is_persistently_bounded_to_the_last_hundred_batches() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let mut events = Vec::new();
    for _ in 0..101 {
        events.push((0, vec![0x90, 60, 80]));
        events.push((1, vec![0x80, 60, 0]));
    }
    events.push((0, vec![0xff, 0x2f, 0]));
    let source = directory.write(
        "101 notes.mid",
        &fixtures::smf(0, 480, &[fixtures::track(events)]),
    );
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Bounded undo")
        .unwrap();
    for id in 0..101 {
        library
            .remove_notes(&variant.id, u64::from(id), &[id])
            .unwrap();
    }
    drop(library);
    let mut library = Library::open(root).unwrap();
    for revision in 101..201 {
        library.undo_notes(&variant.id, revision).unwrap();
    }
    assert_eq!(
        library.variant_state(&variant.id).unwrap().unwrap(),
        model::LessonState {
            revision: 201,
            can_undo: false,
            removed_count: 1,
            shifted_count: 0
        }
    );
    assert_eq!(library.load(&variant.id).unwrap().notes.len(), 100);
    assert!(!library
        .load(&variant.id)
        .unwrap()
        .notes
        .iter()
        .any(|note| note.id == 0));
    assert!(library.undo_notes(&variant.id, 201).is_err());
}

#[test]
fn schema_one_library_upgrades_additively_without_rewriting_song_rows() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("legacy original.mid", &fixtures::single_note(11));
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let before = original_row(&root, &original.id);
    drop(library);
    // This isolated fixture removes only the new schema, reproducing the old
    // schema exactly. Production startup never drops tables or existing data.
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.execute_batch("DROP VIEW library_entries; DROP TRIGGER protect_variant_original_delete;
        DROP TRIGGER protect_variant_original_deleting; DROP TABLE variants; PRAGMA user_version=1;").unwrap();
    drop(db);
    let mut library = Library::open(root.clone()).unwrap();
    assert_eq!(original_row(&root, &original.id), before);
    assert_eq!(library.entry(&original.id).unwrap(), original);
    let variant = library
        .create_variant(&original.id, "First migrated lesson")
        .unwrap();
    assert_eq!(library.versions(&variant.id).unwrap().len(), 2);
    assert_eq!(original_row(&root, &original.id), before);
}

fn stored_variant_json(root: &Path, id: &str) -> (String, String) {
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.query_row(
        "SELECT pitch_json,undo_json FROM variants WHERE 'variant-'||variant_id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap()
}

#[test]
fn sparse_pitch_moves_preserve_originals_and_drive_real_transport_and_mixed_undo() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let bytes = sparse_note_fixture();
    let source = directory.write("unchanged.mid", &bytes);
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let original_song = library.load(&original.id).unwrap();
    let original_before = original_row(&root, &original.id);
    let variant = library.create_variant(&original.id, "Moved notes").unwrap();
    let mut settings = library.settings(&variant.id).unwrap();
    settings.speed = 0.75;
    library.save_settings(&variant.id, &settings).unwrap();
    library.remove_notes(&variant.id, 0, &[0, 2]).unwrap();
    library
        .transpose_notes(&variant.id, 1, &[3, 1], 12)
        .unwrap();
    let edited = library.load(&variant.id).unwrap();
    assert_eq!(
        edited
            .notes
            .iter()
            .map(|note| (note.id, note.pitch))
            .collect::<Vec<_>>(),
        vec![(1, 76), (3, 84)]
    );
    for note in &edited.notes {
        let mut expected = original_song
            .notes
            .iter()
            .find(|source| source.id == note.id)
            .unwrap()
            .clone();
        expected.pitch += 12;
        assert_eq!(
            *note, expected,
            "timing, velocity, part and channel must not change"
        );
    }
    assert_eq!(edited.duration_ms, original_song.duration_ms);
    assert_eq!(edited.tempos, original_song.tempos);
    assert_eq!(edited.controllers, original_song.controllers);
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .shifted_count,
        2
    );
    let mut engine = transport::Engine::new();
    engine.load(edited.clone(), settings.clone());
    engine.play();
    let played: Vec<_> = engine
        .advance(1000.0)
        .into_iter()
        .filter_map(|event| match event {
            transport::VoiceEvent::On { key, pitch, .. } => Some((key, pitch)),
            _ => None,
        })
        .collect();
    assert_eq!(played, vec![(1, 76), (3, 84)]);
    settings.mode = "wait".into();
    engine.load(edited, settings);
    engine.play();
    engine.advance(1000.0);
    assert_eq!(engine.guide_targets().unwrap().1, vec![76]);
    engine.input(76, true, 80, transport::InputSource::Physical);
    assert_eq!(engine.snapshot().correct, 1);

    library.remove_notes(&variant.id, 2, &[1]).unwrap();
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .shifted_count,
        1
    );
    drop(library);
    let mut library = Library::open(root.clone()).unwrap();
    library.undo_notes(&variant.id, 3).unwrap(); // Restore removal, including its shifted pitch.
    assert_eq!(library.load(&variant.id).unwrap().notes[0].pitch, 76);
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .shifted_count,
        2
    );
    library.undo_notes(&variant.id, 4).unwrap(); // Restore pitches, keeping earlier sparse removal.
    assert_eq!(
        library
            .load(&variant.id)
            .unwrap()
            .notes
            .iter()
            .map(|note| (note.id, note.pitch))
            .collect::<Vec<_>>(),
        vec![(1, 64), (3, 72)]
    );
    library.undo_notes(&variant.id, 5).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), original_song);
    assert_eq!(library.settings(&variant.id).unwrap().speed, 0.75);
    assert_eq!(library.settings(&original.id).unwrap().speed, 1.0);
    assert_eq!(original_row(&root, &original.id), original_before);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        fs::read(root.join("midi").join(format!("{}.mid", original.id))).unwrap(),
        bytes
    );
}

#[test]
fn pitch_boundaries_invalid_batches_stale_edits_and_normalization_are_atomic() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let mut events = Vec::new();
    for pitch in [0, 60, 127] {
        events.push((0, vec![0x90, pitch, 80]));
        events.push((120, vec![0x80, pitch, 0]));
    }
    events.push((0, vec![0xff, 0x2f, 0]));
    let source = directory.write(
        "boundaries.mid",
        &fixtures::smf(0, 480, &[fixtures::track(events)]),
    );
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Boundary lesson")
        .unwrap();
    let initial = library.load(&variant.id).unwrap();
    assert!(library.transpose_notes(&original.id, 0, &[1], 1).is_err());
    for (ids, delta) in [
        (vec![0, 1], -1),
        (vec![1, 2], 1),
        (vec![0], -12),
        (vec![2], 12),
        (vec![1, 1], 1),
        (vec![1, u32::MAX], 1),
        (vec![], 1),
    ] {
        assert!(library
            .transpose_notes(&variant.id, 0, &ids, delta)
            .is_err());
        assert_eq!(library.load(&variant.id).unwrap(), initial);
        assert_eq!(
            library
                .variant_state(&variant.id)
                .unwrap()
                .unwrap()
                .revision,
            0
        );
    }
    for delta in [0, -2, 2, -13, 13, i16::MIN, i16::MAX] {
        assert!(library
            .transpose_notes(&variant.id, 0, &[1], delta)
            .is_err());
    }
    library.transpose_notes(&variant.id, 0, &[0], 12).unwrap();
    library.transpose_notes(&variant.id, 1, &[0], -12).unwrap();
    assert_eq!(stored_variant_json(&root, &variant.id).0, "{}");
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .shifted_count,
        0
    );
    library.transpose_notes(&variant.id, 2, &[1], -1).unwrap();
    library.remove_notes(&variant.id, 3, &[1]).unwrap();
    let before = stored_variant_json(&root, &variant.id);
    assert!(library.transpose_notes(&variant.id, 4, &[1], 1).is_err());
    assert!(library.transpose_notes(&variant.id, 3, &[0], 1).is_err());
    assert_eq!(stored_variant_json(&root, &variant.id), before);
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .shifted_count,
        0
    );
    library.undo_notes(&variant.id, 4).unwrap();
    assert_eq!(library.load(&variant.id).unwrap().notes[1].pitch, 59);
    library.transpose_notes(&variant.id, 5, &[1], 1).unwrap();
    assert_eq!(stored_variant_json(&root, &variant.id).0, "{}");
    library.undo_notes(&variant.id, 6).unwrap();
    assert_eq!(library.load(&variant.id).unwrap().notes[1].pitch, 59);
}

#[test]
fn schema_two_removal_history_upgrades_without_losing_undo_despite_wrong_version_flag() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("legacy lesson.mid", &sparse_note_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Old removal history")
        .unwrap();
    library.remove_notes(&variant.id, 0, &[0, 2]).unwrap();
    let source_before = original_row(&root, &original.id);
    drop(library);
    // Isolated schema-2 fixture: old records had only Vec<Vec<u32>> history and
    // no pitch column. The misleading user_version must not skip migration.
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.execute_batch("DROP TRIGGER protect_mixed_variant_history; DROP TRIGGER protect_mixed_variant_insert;
        UPDATE variants SET undo_json='[[0,2]]'; ALTER TABLE variants DROP COLUMN pitch_json; PRAGMA user_version=3;").unwrap();
    drop(db);
    let mut library = Library::open(root.clone()).unwrap();
    assert_eq!(
        library.variant_state(&variant.id).unwrap().unwrap(),
        model::LessonState {
            revision: 1,
            can_undo: true,
            removed_count: 2,
            shifted_count: 0
        }
    );
    assert_eq!(stored_variant_json(&root, &variant.id).1, "[[0,2]]");
    library.transpose_notes(&variant.id, 1, &[1], 1).unwrap();
    let history: serde_json::Value =
        serde_json::from_str(&stored_variant_json(&root, &variant.id).1).unwrap();
    assert_eq!(history["version"], 1);
    assert_eq!(history["operations"].as_array().unwrap().len(), 2);
    library.undo_notes(&variant.id, 2).unwrap();
    library.undo_notes(&variant.id, 3).unwrap();
    assert_eq!(
        library.load(&variant.id).unwrap(),
        library.load(&original.id).unwrap()
    );
    assert_eq!(original_row(&root, &original.id), source_before);
    drop(library);
    // Simulate an older executable overwriting this informational version flag.
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.execute_batch("PRAGMA user_version=2;").unwrap();
    drop(db);
    let library = Library::open(root.clone()).unwrap();
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .revision,
        4
    );
    assert_eq!(stored_variant_json(&root, &variant.id).0, "{}");
}

#[test]
fn pitched_clone_with_empty_history_restarts_and_old_editors_fail_closed() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("copy source.mid", &sparse_note_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let first = library
        .create_variant(&original.id, "Pitch source")
        .unwrap();
    library.transpose_notes(&first.id, 0, &[1, 3], 12).unwrap();
    library.remove_notes(&first.id, 1, &[0]).unwrap();
    let copy = library
        .create_variant(&first.id, "Independent pitched copy")
        .unwrap();
    let expected = library.load(&first.id).unwrap();
    assert_eq!(copy.original_id, Some(original.id));
    assert_eq!(
        library.variant_state(&copy.id).unwrap().unwrap(),
        model::LessonState {
            revision: 0,
            can_undo: false,
            removed_count: 1,
            shifted_count: 2
        }
    );
    let stored = stored_variant_json(&root, &copy.id);
    assert!(
        serde_json::from_str::<Vec<Vec<u32>>>(&stored.1).is_err(),
        "old variant loader must reject even an empty mixed history"
    );
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    assert!(db.execute("UPDATE variants SET undo_json='[]',removed_json='[]',removed_count=0,revision=revision+1 WHERE 'variant-'||variant_id=?1",[&copy.id]).is_err());
    assert_eq!(stored_variant_json(&root, &copy.id), stored);
    drop(db);
    drop(library);
    let mut library = Library::open(root.clone()).unwrap();
    assert_eq!(library.load(&copy.id).unwrap(), expected);
    assert!(library.undo_notes(&copy.id, 0).is_err());
    library.transpose_notes(&copy.id, 0, &[1, 3], -12).unwrap();
    library.undo_notes(&copy.id, 1).unwrap();
    assert_eq!(library.load(&copy.id).unwrap(), expected);
    assert!(!library.variant_state(&copy.id).unwrap().unwrap().can_undo);
    assert!(
        serde_json::from_str::<Vec<Vec<u32>>>(&stored_variant_json(&root, &copy.id).1).is_err()
    );
    assert_eq!(library.load(&first.id).unwrap(), expected);
}

#[test]
fn large_pitch_operations_keep_at_most_two_hundred_thousand_undo_note_changes() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let count = 20_001_u32;
    let mut events = Vec::with_capacity(count as usize * 2 + 1);
    for _ in 0..count {
        events.push((0, vec![0x90, 60, 80]));
        events.push((1, vec![0x80, 60, 0]));
    }
    events.push((0, vec![0xff, 0x2f, 0]));
    let source = directory.write(
        "bounded undo.mid",
        &fixtures::smf(0, 480, &[fixtures::track(events)]),
    );
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Bounded pitch history")
        .unwrap();
    let ids: Vec<_> = (0..count).collect();
    for revision in 0..10 {
        library
            .transpose_notes(
                &variant.id,
                revision,
                &ids,
                if revision % 2 == 0 { 1 } else { -1 },
            )
            .unwrap();
    }
    let history: serde_json::Value =
        serde_json::from_str(&stored_variant_json(&root, &variant.id).1).unwrap();
    let operations = history["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 9);
    assert_eq!(
        operations
            .iter()
            .map(|op| op["previous"].as_array().unwrap().len())
            .sum::<usize>(),
        180_009
    );
    drop(library);
    let mut library = Library::open(root).unwrap();
    for revision in 10..19 {
        library.undo_notes(&variant.id, revision).unwrap();
    }
    let state = library.variant_state(&variant.id).unwrap().unwrap();
    assert!(!state.can_undo);
    assert_eq!(state.shifted_count, count as usize);
    assert!(library
        .load(&variant.id)
        .unwrap()
        .notes
        .iter()
        .all(|note| note.pitch == 61));
    assert!(library
        .load(&original.id)
        .unwrap()
        .notes
        .iter()
        .all(|note| note.pitch == 60));
}

#[test]
fn edited_export_reimports_and_plays_while_original_bytes_and_existing_files_stay_exact() {
    for bytes in [fixtures::demo_midi(), fixtures::tempo_sustain()] {
        let directory = TestDirectory::new();
        let root = directory.path().join("library");
        let source = directory.write("Downloads 日本語/original.mid", &bytes);
        let mut library = Library::open(root.clone()).unwrap();
        let original = library.import_path(&source).unwrap().entry;
        let original_song = library.load(&original.id).unwrap();
        let row_before = original_row(&root, &original.id);
        let variant = library
            .create_variant(&original.id, "Exported practice version")
            .unwrap();
        let moved_id = original_song.notes.first().unwrap().id;
        let removed_id = original_song.notes.last().unwrap().id;
        library
            .transpose_notes(&variant.id, 0, &[moved_id], 12)
            .unwrap();
        library.remove_notes(&variant.id, 1, &[removed_id]).unwrap();
        let edited = library.load(&variant.id).unwrap();
        let state_before = library.variant_state(&variant.id).unwrap();
        let edits_before = stored_variant_json(&root, &variant.id);
        drop(library);

        let mut library = Library::open(root.clone()).unwrap();
        let exported = directory.path().join("practice version 日本語.mid");
        library.export(&variant.id, &exported).unwrap();
        assert_ne!(fs::read(&exported).unwrap(), bytes);
        let imported = library.import_path(&exported).unwrap();
        assert!(!imported.duplicate);
        assert_ne!(imported.entry.id, original.id);
        assert_eq!(imported.entry.original_id, None);
        assert!(library.variant_state(&imported.entry.id).unwrap().is_none());
        let reloaded = library.load(&imported.entry.id).unwrap();
        // MIDI has no Keyloom stable IDs. Reimport assigns new dense IDs, but
        // every retained note and all canonical timing/controllers must match.
        let mut expected = edited.clone();
        assert_eq!(expected.notes.len(), reloaded.notes.len());
        for (expected, actual) in expected.notes.iter_mut().zip(&reloaded.notes) {
            expected.id = actual.id;
        }
        assert_eq!(reloaded, expected);
        let mut engine = transport::Engine::new();
        engine.load(reloaded, library.settings(&imported.entry.id).unwrap());
        engine.play();
        assert!(engine.advance(100.0).iter().any(|event| matches!(
            event,
            transport::VoiceEvent::On { pitch, .. } if *pitch == edited.notes[0].pitch
        )));

        let existing = directory.write("existing destination.mid", b"keep this external file");
        assert!(library.export(&variant.id, &existing).is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"keep this external file");
        assert!(library.export(&variant.id, &source).is_err());
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let original_export = directory.path().join("exact original export.mid");
        library.export(&original.id, &original_export).unwrap();
        assert_eq!(fs::read(original_export).unwrap(), bytes);
        assert_eq!(
            fs::read(root.join("midi").join(format!("{}.mid", original.id))).unwrap(),
            bytes
        );
        assert_eq!(library.load(&original.id).unwrap(), original_song);
        assert_eq!(original_row(&root, &original.id), row_before);
        assert_eq!(library.variant_state(&variant.id).unwrap(), state_before);
        assert_eq!(stored_variant_json(&root, &variant.id), edits_before);
    }
}

fn tool_chords_fixture() -> Vec<u8> {
    fixtures::smf(
        1,
        480,
        &[
            fixtures::track(vec![
                (0, vec![0x90, 60, 80]),
                (0, vec![0x90, 64, 80]),
                (0, vec![0x90, 72, 80]),
                (0, vec![0x90, 72, 80]),
                (1, vec![0x90, 67, 80]),
                (119, vec![0x80, 60, 0]),
                (0, vec![0x80, 64, 0]),
                (0, vec![0x80, 72, 0]),
                (0, vec![0x80, 72, 0]),
                (0, vec![0x80, 67, 0]),
                (120, vec![0x90, 65, 80]),
                (0, vec![0x90, 65, 80]),
                (120, vec![0x80, 65, 0]),
                (0, vec![0x80, 65, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
            fixtures::track(vec![
                (0, vec![0x91, 48, 80]),
                (0, vec![0x91, 72, 80]),
                (120, vec![0x81, 48, 0]),
                (0, vec![0x81, 72, 0]),
                (120, vec![0x91, 65, 80]),
                (120, vec![0x81, 65, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
            fixtures::track(vec![
                (0, vec![0x99, 36, 80]),
                (0, vec![0x99, 42, 80]),
                (120, vec![0x89, 36, 0]),
                (0, vec![0x89, 42, 0]),
                (120, vec![0x99, 38, 80]),
                (120, vec![0x89, 38, 0]),
                (0, vec![0xff, 0x2f, 0]),
            ]),
        ],
    )
}

fn variant_row(root: &Path, id: &str) -> Vec<rusqlite::types::Value> {
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    let mut statement = db
        .prepare("SELECT * FROM variants WHERE 'variant-'||variant_id=?1")
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_row([id], |row| {
            (0..columns).map(|index| row.get(index)).collect()
        })
        .unwrap()
}

fn thin(strategy: ThinStrategy, max_notes: u8, part_id: Option<u32>) -> LessonToolRequest {
    LessonToolRequest::Thin {
        strategy,
        max_notes,
        part_id,
    }
}

#[test]
fn tool_previews_group_exact_ticks_deterministically_across_pitched_parts_without_writes() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let bytes = tool_chords_fixture();
    let source = directory.write("chords.mid", &bytes);
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Preview only")
        .unwrap();
    let song = library.load(&variant.id).unwrap();
    let first_part = song.notes.iter().find(|note| note.id == 0).unwrap().part_id;
    let second_part = song.notes.iter().find(|note| note.id == 7).unwrap().part_id;
    let before = variant_row(&root, &variant.id);
    let original_before = original_row(&root, &original.id);
    let cases = [
        (
            thin(ThinStrategy::Highest, 1, None),
            vec![0, 1, 3, 6, 7, 8, 9],
        ),
        (
            thin(ThinStrategy::BassHighest, 2, None),
            vec![0, 1, 3, 6, 8, 9],
        ),
        // Limit counts note events: both tied high notes survive before ID 8.
        (thin(ThinStrategy::Limit, 2, None), vec![0, 1, 7, 8, 9]),
        (
            thin(ThinStrategy::Highest, 1, Some(first_part)),
            vec![0, 1, 3, 6],
        ),
        (thin(ThinStrategy::Highest, 1, Some(second_part)), vec![7]),
    ];
    for (request, expected) in cases {
        let preview = library.preview_tool(&variant.id, &request, 0).unwrap();
        assert_eq!(
            preview,
            library.preview_tool(&variant.id, &request, 0).unwrap()
        );
        assert_eq!(preview.revision, 0);
        assert_eq!(
            preview
                .changes
                .iter()
                .map(|change| change.note_id)
                .collect::<Vec<_>>(),
            expected
        );
        for change in &preview.changes {
            assert_eq!(change.to_pitch, None);
            assert_eq!(
                change.from_pitch,
                song.notes
                    .iter()
                    .find(|note| note.id == change.note_id)
                    .unwrap()
                    .pitch
            );
            assert_ne!(change.note_id, 4, "one-tick-later onset is its own chord");
            assert!(change.note_id < 10, "percussion is excluded");
        }
        assert_eq!(
            serde_json::to_value(&preview).unwrap()["changes"][0]["to_pitch"],
            serde_json::Value::Null
        );
    }
    // Dropping a preview is Cancel: there is no pending state or stored edit.
    assert_eq!(variant_row(&root, &variant.id), before);
    assert_eq!(original_row(&root, &original.id), original_before);
    assert_eq!(library.load(&variant.id).unwrap(), song);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    drop(library);
    let library = Library::open(root.clone()).unwrap();
    assert_eq!(variant_row(&root, &variant.id), before);
    assert_eq!(library.load(&variant.id).unwrap(), song);
}

#[test]
fn tool_noops_and_invalid_requests_never_advance_revision_or_history() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("chords.mid", &tool_chords_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library.create_variant(&original.id, "Validation").unwrap();
    let song = library.load(&variant.id).unwrap();
    let drums = song.parts.iter().find(|part| part.percussion).unwrap().id;
    let before = variant_row(&root, &variant.id);
    for request in [
        thin(ThinStrategy::Limit, 8, None),
        LessonToolRequest::Shift {
            note_ids: vec![],
            semitones: 12,
        },
    ] {
        assert!(library
            .preview_tool(&variant.id, &request, 0)
            .unwrap()
            .changes
            .is_empty());
        assert!(library
            .apply_tool(&variant.id, &request, 0)
            .unwrap_err()
            .contains("would not change"));
    }
    for request in [
        thin(ThinStrategy::Limit, 0, None),
        thin(ThinStrategy::Highest, 9, None),
        thin(ThinStrategy::BassHighest, 2, Some(drums)),
        thin(ThinStrategy::Highest, 1, Some(u32::MAX)),
        LessonToolRequest::Shift {
            note_ids: vec![0],
            semitones: 1,
        },
        LessonToolRequest::Shift {
            note_ids: vec![0],
            semitones: i16::MIN,
        },
        LessonToolRequest::Shift {
            note_ids: vec![0, 0],
            semitones: 12,
        },
        LessonToolRequest::Shift {
            note_ids: vec![0, u32::MAX],
            semitones: 12,
        },
        LessonToolRequest::Shift {
            note_ids: vec![0; parser::MAX_NOTES + 1],
            semitones: 12,
        },
    ] {
        assert!(library.preview_tool(&variant.id, &request, 0).is_err());
        assert!(library.apply_tool(&variant.id, &request, 0).is_err());
        assert_eq!(variant_row(&root, &variant.id), before);
    }
    let request = thin(ThinStrategy::Highest, 1, None);
    assert!(library.preview_tool(&original.id, &request, 0).is_err());
    assert!(library.apply_tool(&original.id, &request, 0).is_err());
    assert!(library.preview_tool(&variant.id, &request, 1).is_err());
    assert!(library.apply_tool(&variant.id, &request, 1).is_err());
    assert_eq!(variant_row(&root, &variant.id), before);
    let all: Vec<_> = song.notes.iter().map(|note| note.id).collect();
    library.remove_notes(&variant.id, 0, &all).unwrap();
    let empty_before = variant_row(&root, &variant.id);
    assert!(library
        .preview_tool(&variant.id, &request, 1)
        .unwrap()
        .changes
        .is_empty());
    assert!(library.apply_tool(&variant.id, &request, 1).is_err());
    assert_eq!(variant_row(&root, &variant.id), empty_before);
}

#[test]
fn octave_preview_and_apply_validate_whole_batch_and_use_current_effective_pitches() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let mut events = Vec::new();
    for pitch in [0, 60, 127] {
        events.push((0, vec![0x90, pitch, 80]));
        events.push((120, vec![0x80, pitch, 0]));
    }
    events.push((0, vec![0xff, 0x2f, 0]));
    let source = directory.write(
        "boundary.mid",
        &fixtures::smf(0, 480, &[fixtures::track(events)]),
    );
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library.create_variant(&original.id, "Octaves").unwrap();
    let before = variant_row(&root, &variant.id);
    for request in [
        LessonToolRequest::Shift {
            note_ids: vec![1, 2],
            semitones: 12,
        },
        LessonToolRequest::Shift {
            note_ids: vec![1, 0],
            semitones: -12,
        },
    ] {
        assert!(library.preview_tool(&variant.id, &request, 0).is_err());
        assert!(library.apply_tool(&variant.id, &request, 0).is_err());
        assert_eq!(variant_row(&root, &variant.id), before);
    }
    library.transpose_notes(&variant.id, 0, &[1], 1).unwrap();
    let request: LessonToolRequest =
        serde_json::from_str(r#"{"kind":"shift","note_ids":[1,0],"semitones":12}"#).unwrap();
    let preview = library.preview_tool(&variant.id, &request, 1).unwrap();
    assert_eq!(
        preview.changes,
        vec![
            model::NoteChange {
                note_id: 0,
                from_pitch: 0,
                to_pitch: Some(12)
            },
            model::NoteChange {
                note_id: 1,
                from_pitch: 61,
                to_pitch: Some(73)
            },
        ]
    );
    library.apply_tool(&variant.id, &request, 1).unwrap();
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    let shifted = library.load(&variant.id).unwrap();
    assert_eq!(
        shifted
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        [12, 73, 127]
    );
    library.undo_notes(&variant.id, 2).unwrap();
    assert_eq!(
        library
            .load(&variant.id)
            .unwrap()
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        [0, 61, 127]
    );
    library.remove_notes(&variant.id, 3, &[1]).unwrap();
    let before_removed = variant_row(&root, &variant.id);
    assert!(library.preview_tool(&variant.id, &request, 4).is_err());
    assert!(library.apply_tool(&variant.id, &request, 4).is_err());
    assert_eq!(variant_row(&root, &variant.id), before_removed);
}

#[test]
fn stale_tool_apply_refuses_other_connections_edit_and_recomputes_new_revision() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("chords.mid", &tool_chords_fixture());
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Concurrent preview")
        .unwrap();
    let request = thin(ThinStrategy::Highest, 1, None);
    let old = library.preview_tool(&variant.id, &request, 0).unwrap();
    assert!(!old.changes.iter().any(|change| change.note_id == 2));
    let mut other = Library::open(root.clone()).unwrap();
    other.transpose_notes(&variant.id, 0, &[0], 12).unwrap();
    let before = variant_row(&root, &variant.id);
    assert!(library
        .apply_tool(&variant.id, &request, old.revision)
        .is_err());
    assert_eq!(variant_row(&root, &variant.id), before);
    let fresh = library.preview_tool(&variant.id, &request, 1).unwrap();
    assert!(fresh.changes.iter().any(|change| change.note_id == 2));
    assert!(!fresh.changes.iter().any(|change| change.note_id == 0));
    library.apply_tool(&variant.id, &request, 1).unwrap();
    let song = library.load(&variant.id).unwrap();
    assert_eq!(
        song.notes.iter().map(|note| note.id).collect::<Vec<_>>(),
        [0, 10, 11, 4, 5, 12]
    );
    assert_eq!(
        library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    library.undo_notes(&variant.id, 2).unwrap();
    assert_eq!(library.load(&variant.id).unwrap().notes.len(), 13);
    assert_eq!(library.load(&variant.id).unwrap().notes[0].pitch, 72);
}

#[test]
fn tools_share_legacy_compatible_mixed_undo_and_export_reimport_without_schema_changes() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let bytes = tool_chords_fixture();
    let source = directory.write("chords.mid", &bytes);
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let original_song = library.load(&original.id).unwrap();
    let original_before = original_row(&root, &original.id);
    let variant = library.create_variant(&original.id, "Mixed tools").unwrap();
    let settings = model::SongSettings {
        speed: 0.5,
        ..Default::default()
    };
    library.save_settings(&variant.id, &settings).unwrap();
    library.remove_notes(&variant.id, 0, &[12]).unwrap();
    let removed = library.load(&variant.id).unwrap();
    drop(library);
    // Emulate a saved removal-only version from before mixed history existed.
    // The production startup reinstalls the trigger after this fixture setup.
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    db.execute_batch("DROP TRIGGER protect_mixed_variant_history;")
        .unwrap();
    db.execute(
        "UPDATE variants SET undo_json='[[12]]' WHERE 'variant-'||variant_id=?1",
        [&variant.id],
    )
    .unwrap();
    let columns_before: Vec<String> = db
        .prepare("PRAGMA table_info(variants)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let schema_before: i64 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    drop(db);
    let mut library = Library::open(root.clone()).unwrap();
    let shift = LessonToolRequest::Shift {
        note_ids: vec![2],
        semitones: 12,
    };
    library.apply_tool(&variant.id, &shift, 1).unwrap();
    let shifted = library.load(&variant.id).unwrap();
    let request: LessonToolRequest = serde_json::from_str(
        r#"{"kind":"thin","strategy":"highest","max_notes":1,"part_id":null}"#,
    )
    .unwrap();
    library.apply_tool(&variant.id, &request, 2).unwrap();
    let thinned = library.load(&variant.id).unwrap();
    let history: serde_json::Value =
        serde_json::from_str(&stored_variant_json(&root, &variant.id).1).unwrap();
    assert_eq!(history["version"], 1);
    assert_eq!(history["operations"].as_array().unwrap().len(), 3);
    assert_eq!(history["operations"][0]["kind"], "remove");
    assert_eq!(history["operations"][1]["kind"], "pitch");
    assert_eq!(history["operations"][2]["kind"], "remove");
    let exported = directory.path().join("tool result.mid");
    library.export(&variant.id, &exported).unwrap();
    let imported = library.import_path(&exported).unwrap();
    assert!(!imported.duplicate);
    let round_trip = library.load(&imported.entry.id).unwrap();
    let mut expected = thinned.clone();
    assert_eq!(expected.notes.len(), round_trip.notes.len());
    for (expected, actual) in expected.notes.iter_mut().zip(&round_trip.notes) {
        expected.id = actual.id;
    }
    assert_eq!(round_trip.notes, expected.notes);
    assert_eq!(round_trip.tempos, expected.tempos);
    assert_eq!(round_trip.duration_ms, expected.duration_ms);
    let mut engine = transport::Engine::new();
    engine.load(round_trip, library.settings(&imported.entry.id).unwrap());
    engine.play();
    assert!(engine
        .advance(100.0)
        .iter()
        .any(|event| matches!(event, transport::VoiceEvent::On { pitch: 84, .. })));
    drop(library);
    let mut library = Library::open(root.clone()).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), thinned);
    library.undo_notes(&variant.id, 3).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), shifted);
    library.undo_notes(&variant.id, 4).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), removed);
    library.undo_notes(&variant.id, 5).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), original_song);
    assert!(
        !library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .can_undo
    );
    assert_eq!(library.settings(&variant.id).unwrap(), settings);
    assert_eq!(
        library.settings(&original.id).unwrap(),
        model::SongSettings::default()
    );
    assert_eq!(original_row(&root, &original.id), original_before);
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert_eq!(
        fs::read(root.join("midi").join(format!("{}.mid", original.id))).unwrap(),
        bytes
    );
    let db = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
    let columns_after: Vec<String> = db
        .prepare("PRAGMA table_info(variants)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let schema_after: i64 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(columns_after, columns_before);
    assert_eq!(schema_after, schema_before);
}

#[test]
fn thinning_a_large_unison_chord_is_one_bounded_undo_batch() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let count = 10_000;
    let mut events = vec![(0, vec![0x90, 60, 80]); count];
    events.push((120, vec![0x80, 60, 0]));
    events.extend(vec![(0, vec![0x80, 60, 0]); count - 1]);
    events.push((0, vec![0xff, 0x2f, 0]));
    let source = directory.write(
        "large chord.mid",
        &fixtures::smf(0, 480, &[fixtures::track(events)]),
    );
    let mut library = Library::open(root.clone()).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library.create_variant(&original.id, "One chord").unwrap();
    let request = thin(ThinStrategy::BassHighest, 2, None);
    let preview = library.preview_tool(&variant.id, &request, 0).unwrap();
    assert_eq!(preview.changes.len(), count - 1);
    assert_eq!(preview.changes[0].note_id, 1);
    library.apply_tool(&variant.id, &request, 0).unwrap();
    let thinned = library.load(&variant.id).unwrap();
    assert_eq!(thinned.notes.len(), 1);
    assert_eq!(thinned.notes[0].id, 0);
    let history: serde_json::Value =
        serde_json::from_str(&stored_variant_json(&root, &variant.id).1).unwrap();
    assert_eq!(history["operations"].as_array().unwrap().len(), 1);
    drop(library);
    let mut library = Library::open(root).unwrap();
    library.undo_notes(&variant.id, 1).unwrap();
    assert_eq!(library.load(&variant.id).unwrap().notes.len(), count);
    assert!(
        !library
            .variant_state(&variant.id)
            .unwrap()
            .unwrap()
            .can_undo
    );
}

#[test]
fn scoped_thinning_uses_sparse_edited_pitches_and_preserves_every_other_part() {
    let directory = TestDirectory::new();
    let root = directory.path().join("library");
    let source = directory.write("chords.mid", &tool_chords_fixture());
    let mut library = Library::open(root).unwrap();
    let original = library.import_path(&source).unwrap().entry;
    let variant = library
        .create_variant(&original.id, "Sparse scoped edit")
        .unwrap();
    library.remove_notes(&variant.id, 0, &[2, 6]).unwrap();
    library.transpose_notes(&variant.id, 1, &[0], 12).unwrap();
    let before = library.load(&variant.id).unwrap();
    let scope = before
        .notes
        .iter()
        .find(|note| note.id == 0)
        .unwrap()
        .part_id;
    let request = thin(ThinStrategy::Highest, 1, Some(scope));
    let preview = library.preview_tool(&variant.id, &request, 2).unwrap();
    assert_eq!(
        preview.changes,
        vec![
            model::NoteChange {
                note_id: 1,
                from_pitch: 64,
                to_pitch: None
            },
            model::NoteChange {
                note_id: 3,
                from_pitch: 72,
                to_pitch: None
            },
        ]
    );
    assert_eq!(library.load(&variant.id).unwrap(), before);
    library.apply_tool(&variant.id, &request, 2).unwrap();
    let after = library.load(&variant.id).unwrap();
    let mut expected = before.clone();
    expected.notes.retain(|note| note.id != 1 && note.id != 3);
    expected
        .parts
        .iter_mut()
        .find(|part| part.id == scope)
        .unwrap()
        .note_count -= 2;
    assert_eq!(after, expected);
    assert_eq!(
        after.notes.iter().find(|note| note.id == 0).unwrap().pitch,
        72
    );
    library.undo_notes(&variant.id, 3).unwrap();
    assert_eq!(library.load(&variant.id).unwrap(), before);
    let state = library.variant_state(&variant.id).unwrap().unwrap();
    assert_eq!(state.removed_count, 2);
    assert_eq!(state.shifted_count, 1);
    assert!(state.can_undo);
}
