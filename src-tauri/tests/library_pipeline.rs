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
use model::PartSetting;
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
            removed_count: 1
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
            removed_count: 1
        }
    );
    library.undo_notes(&first.id, 1).unwrap();
    assert_eq!(library.load(&first.id).unwrap(), canonical);
    assert_eq!(
        library.variant_state(&first.id).unwrap().unwrap(),
        model::LessonState {
            revision: 2,
            can_undo: false,
            removed_count: 0
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
            removed_count: 2
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
        .contains("not yet available"));
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
            removed_count: 1
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
