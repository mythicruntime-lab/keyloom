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
            fixtures::smf(2, 480, &[track.clone()]),
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
