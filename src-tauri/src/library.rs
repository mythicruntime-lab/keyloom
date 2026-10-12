pub use crate::model::{ImportResult, LessonState, LibraryEntry};
use crate::model::{Song, SongSettings};
use crate::parser::{self, MAX_FILE_BYTES};
use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ENTRY_COLUMNS: &str = "id,title,artist,tags,arrangement,favorite,imported_at,last_played,duration_ms,parts_count,note_count,original_id";
const MAX_METADATA_CHARS: usize = 2_048;
const MAX_UNDO_BATCHES: usize = 100;

struct VariantEdit {
    original_id: String,
    removed: BTreeSet<u32>,
    undo: Vec<Vec<u32>>,
    revision: u64,
}

pub struct Library {
    root: PathBuf,
    db: Connection,
}

impl Library {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("midi")).map_err(io_error)?;
        let db = Connection::open(root.join("library.sqlite3")).map_err(db_error)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS songs (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '', arrangement TEXT NOT NULL DEFAULT '',
                favorite INTEGER NOT NULL DEFAULT 0, imported_at INTEGER NOT NULL,
                last_played INTEGER, duration_ms REAL NOT NULL, parts_count INTEGER NOT NULL,
                note_count INTEGER NOT NULL, source_path TEXT NOT NULL,
                song_json TEXT NOT NULL, settings_json TEXT NOT NULL,
                search_text TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'ready');
            CREATE INDEX IF NOT EXISTS songs_recent ON songs(state, imported_at DESC);
            CREATE INDEX IF NOT EXISTS songs_favorites ON songs(state, favorite);
            CREATE TABLE IF NOT EXISTS variants (
                variant_id INTEGER PRIMARY KEY AUTOINCREMENT,
                original_id TEXT NOT NULL,
                title TEXT NOT NULL, artist TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '', arrangement TEXT NOT NULL DEFAULT '',
                favorite INTEGER NOT NULL DEFAULT 0, imported_at INTEGER NOT NULL,
                last_played INTEGER, settings_json TEXT NOT NULL,
                removed_json TEXT NOT NULL DEFAULT '[]', undo_json TEXT NOT NULL DEFAULT '[]',
                removed_count INTEGER NOT NULL DEFAULT 0 CHECK(removed_count>=0),
                revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), search_text TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS variants_original ON variants(original_id);
            CREATE TRIGGER IF NOT EXISTS protect_variant_original_delete
                BEFORE DELETE ON songs WHEN EXISTS(SELECT 1 FROM variants WHERE original_id=OLD.id)
                BEGIN SELECT RAISE(ABORT,'Remove practice variants before deleting their original MIDI.'); END;
            CREATE TRIGGER IF NOT EXISTS protect_variant_original_deleting
                BEFORE UPDATE OF state ON songs WHEN NEW.state='deleting'
                    AND EXISTS(SELECT 1 FROM variants WHERE original_id=OLD.id)
                BEGIN SELECT RAISE(ABORT,'Remove practice variants before deleting their original MIDI.'); END;
            CREATE VIEW IF NOT EXISTS library_entries AS
                SELECT id,title,artist,tags,arrangement,favorite,imported_at,last_played,
                    duration_ms,parts_count,note_count,NULL AS original_id,
                    settings_json,search_text,state,rowid AS sort_id FROM songs
                UNION ALL
                SELECT 'variant-'||v.variant_id,v.title,v.artist,v.tags,v.arrangement,
                    v.favorite,v.imported_at,v.last_played,s.duration_ms,s.parts_count,
                    s.note_count-v.removed_count,v.original_id,
                    v.settings_json,v.search_text,s.state,v.variant_id
                FROM variants v JOIN songs s ON s.id=v.original_id;
            PRAGMA user_version=2;",
        )
        .map_err(db_error)?;
        let mut library = Self { root, db };
        library.recover()?;
        Ok(library)
    }

    pub fn import_path(&mut self, path: &Path) -> Result<ImportResult, String> {
        let start = Instant::now();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case("mid") && !extension.eq_ignore_ascii_case("midi") {
            return Err("Choose a .mid or .midi file.".into());
        }
        let bytes = read_bounded(path)?;
        let id = content_id(&bytes);
        if let Some(entry) = self.find_entry(&id)? {
            if self.managed_path(&id)?.is_file() {
                return Ok(ImportResult {
                    entry,
                    duplicate: true,
                    elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                });
            }
            self.db
                .execute("UPDATE songs SET state='missing' WHERE id=?1", [&id])
                .map_err(db_error)?;
        }
        // Validate and serialize before any managed copy or Ready record exists.
        let song = parser::parse(&bytes)?;
        let song_json = serde_json::to_string(&song).map_err(json_error)?;
        let settings_json = serde_json::to_string(&SongSettings::default()).map_err(json_error)?;
        let title: String = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect();
        let title = if title.is_empty() {
            "Untitled MIDI".to_string()
        } else {
            title
        };
        let imported_at = timestamp();
        let destination = self.managed_path(&id)?;
        let temporary = self.root.join("midi").join(format!("{id}.part"));
        // A previous interrupted import can leave an uncommitted copy. It must
        // match the current source hash before being reused.
        if destination.exists() {
            if content_id(&read_bounded(&destination)?) != id {
                return Err(
                    "The managed MIDI copy is damaged; remove its library entry before retrying."
                        .into(),
                );
            }
        } else {
            let write_result = (|| -> Result<(), String> {
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(io_error)?;
                file.write_all(&bytes).map_err(io_error)?;
                file.sync_all().map_err(io_error)?;
                drop(file);
                fs::rename(&temporary, &destination).map_err(io_error)?;
                Ok(())
            })();
            if write_result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            write_result?;
        }
        let tx = self.db.transaction().map_err(db_error)?;
        // Existing metadata/settings survive recovery and a byte-identical retry.
        tx.execute("INSERT INTO songs
            (id,title,imported_at,duration_ms,parts_count,note_count,source_path,song_json,settings_json,search_text,state)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'ready')
            ON CONFLICT(id) DO UPDATE SET song_json=excluded.song_json, state='ready'",
            params![id, title, imported_at, song.duration_ms, song.parts.len() as u32, song.notes.len() as u32,
                path.to_string_lossy(), song_json, settings_json, title.to_lowercase()]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        let entry = self.entry(&id)?;
        Ok(ImportResult {
            entry,
            duplicate: false,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// The same Ready repository is used for import results, filtering and Open.
    pub fn list(&self, query: &str, favorites: bool) -> Result<Vec<LibraryEntry>, String> {
        if query.chars().count() > MAX_METADATA_CHARS {
            return Err("Search text is too long.".into());
        }
        let sql = format!(
            "SELECT {ENTRY_COLUMNS} FROM library_entries WHERE state='ready'
            AND (?1=0 OR favorite=1) AND (?2='' OR instr(search_text,?2)>0)
            ORDER BY imported_at DESC, sort_id DESC, id"
        );
        let mut statement = self.db.prepare_cached(&sql).map_err(db_error)?;
        let rows = statement
            .query_map(
                params![favorites, query.trim().to_lowercase()],
                entry_from_row,
            )
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }

    pub fn entry(&self, id: &str) -> Result<LibraryEntry, String> {
        self.find_entry(id)?
            .ok_or_else(|| "This song is not available in the managed library.".into())
    }

    /// A variant owns only metadata, settings and reversible note-ID removals.
    /// Its root original's bytes, row and canonical model remain untouched.
    pub fn create_variant(&mut self, source_id: &str, title: &str) -> Result<LibraryEntry, String> {
        let mut source = self.entry(source_id)?;
        self.load(source_id)?; // Reject a missing managed source before creating a version.
        source.title = title.trim().to_string();
        validate_metadata(&source)?;
        let original_id = source
            .original_id
            .as_deref()
            .unwrap_or(source_id)
            .to_string();
        let removed = if source.original_id.is_some() {
            self.variant_edit(source_id)?.removed
        } else {
            BTreeSet::new()
        };
        let removed_json = serde_json::to_string(&removed).map_err(json_error)?;
        let settings_json =
            serde_json::to_string(&self.settings(source_id)?).map_err(json_error)?;
        let tx = self.db.transaction().map_err(db_error)?;
        let changed = tx
            .execute(
                "INSERT INTO variants(original_id,title,artist,tags,arrangement,favorite,
                imported_at,settings_json,removed_json,removed_count,search_text)
             SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11
             FROM songs WHERE id=?1 AND state='ready'",
                params![
                    original_id,
                    source.title,
                    source.artist,
                    source.tags,
                    source.arrangement,
                    source.favorite,
                    timestamp(),
                    settings_json,
                    removed_json,
                    removed.len() as u32,
                    search_text(&source)
                ],
            )
            .map_err(db_error)?;
        if changed != 1 {
            return Err("Original MIDI is not ready or does not exist.".into());
        }
        let id = format!("variant-{}", tx.last_insert_rowid());
        tx.commit().map_err(db_error)?;
        self.entry(&id)
    }

    pub fn variant_state(&self, id: &str) -> Result<Option<LessonState>, String> {
        self.entry(id)?;
        if variant_number(id)?.is_none() {
            return Ok(None);
        }
        let edit = self.variant_edit(id)?;
        Ok(Some(LessonState {
            revision: edit.revision,
            can_undo: !edit.undo.is_empty(),
            removed_count: edit.removed.len(),
        }))
    }

    /// Returns the root original first, then its named variants newest first.
    pub fn versions(&self, id: &str) -> Result<Vec<LibraryEntry>, String> {
        let entry = self.entry(id)?;
        let original_id = entry.original_id.as_deref().unwrap_or(id);
        let sql = format!(
            "SELECT {ENTRY_COLUMNS} FROM library_entries
            WHERE state='ready' AND (id=?1 OR original_id=?1)
            ORDER BY original_id IS NOT NULL, imported_at DESC, sort_id DESC"
        );
        let mut statement = self.db.prepare_cached(&sql).map_err(db_error)?;
        let rows = statement
            .query_map([original_id], entry_from_row)
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }

    pub fn remove_notes(
        &mut self,
        id: &str,
        expected_revision: u64,
        note_ids: &[u32],
    ) -> Result<(), String> {
        let mut edit = self.variant_edit(id)?;
        check_revision(edit.revision, expected_revision)?;
        if note_ids.is_empty() || note_ids.len() > parser::MAX_NOTES {
            return Err("Select at least one note within the song's note limit.".into());
        }
        let original = self.load(&edit.original_id)?;
        let original_ids: HashSet<u32> = original.notes.iter().map(|note| note.id).collect();
        let batch: BTreeSet<u32> = note_ids.iter().copied().collect();
        if batch.len() != note_ids.len() {
            return Err("A note removal cannot contain repeated note IDs.".into());
        }
        for note in &batch {
            if !original_ids.contains(note) {
                return Err(format!(
                    "Unknown stable note ID {note}; no notes were changed."
                ));
            }
            if edit.removed.contains(note) {
                return Err(format!(
                    "Note {note} is already removed; refresh this practice variant."
                ));
            }
        }
        edit.removed.extend(batch.iter().copied());
        edit.undo.push(batch.into_iter().collect());
        if edit.undo.len() > MAX_UNDO_BATCHES {
            edit.undo.remove(0);
        }
        self.save_variant_edit(id, expected_revision, &edit)
    }

    pub fn undo_notes(&mut self, id: &str, expected_revision: u64) -> Result<(), String> {
        let mut edit = self.variant_edit(id)?;
        check_revision(edit.revision, expected_revision)?;
        self.load(&edit.original_id)?;
        let batch = edit
            .undo
            .pop()
            .ok_or("No note removal is available to undo.")?;
        for note in batch {
            edit.removed.remove(&note);
        }
        self.save_variant_edit(id, expected_revision, &edit)
    }

    /// Preflight may be called before unloading a currently playing song. The
    /// removal operation repeats this check, and SQL triggers also protect old
    /// application versions sharing this database from deleting a parent.
    pub fn validate_removal(&self, id: &str) -> Result<(), String> {
        validate_id(id)?;
        if let Some(number) = variant_number(id)? {
            let exists: bool = self
                .db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM variants WHERE variant_id=?1)",
                    [number],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            return if exists {
                Ok(())
            } else {
                Err("Song does not exist.".into())
            };
        }
        let exists: bool = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM songs WHERE id=?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !exists {
            return Err("Song does not exist.".into());
        }
        let dependent: bool = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM variants WHERE original_id=?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if dependent {
            return Err("Remove practice variants before deleting their original MIDI.".into());
        }
        Ok(())
    }

    fn variant_edit(&self, id: &str) -> Result<VariantEdit, String> {
        validate_id(id)?;
        let number = variant_number(id)?
            .ok_or("Create a practice variant before editing notes; originals are read-only.")?;
        let record = self
            .db
            .query_row(
                "SELECT v.original_id,v.removed_json,v.undo_json,v.revision FROM variants v
             JOIN songs s ON s.id=v.original_id WHERE v.variant_id=?1 AND s.state='ready'",
                [number],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(db_error)?
            .ok_or("Practice variant is unavailable; recover its original MIDI first.")?;
        validate_original_id(&record.0)?;
        if record.1.len() > parser::MAX_NOTES * 12
            || record.2.len() > parser::MAX_NOTES * 12 + MAX_UNDO_BATCHES * 2
        {
            return Err("Saved practice edits exceed the note safety limit.".into());
        }
        let removed: BTreeSet<u32> = serde_json::from_str(&record.1).map_err(json_error)?;
        let undo: Vec<Vec<u32>> = serde_json::from_str(&record.2).map_err(json_error)?;
        let mut undo_notes = HashSet::new();
        if removed.len() > parser::MAX_NOTES
            || undo.len() > MAX_UNDO_BATCHES
            || record.3 < 0
            || undo.iter().any(|batch| {
                batch.is_empty()
                    || batch
                        .iter()
                        .any(|note| !removed.contains(note) || !undo_notes.insert(*note))
            })
        {
            return Err("Saved practice edit history is invalid.".into());
        }
        Ok(VariantEdit {
            original_id: record.0,
            removed,
            undo,
            revision: record.3 as u64,
        })
    }

    fn save_variant_edit(
        &mut self,
        id: &str,
        expected_revision: u64,
        edit: &VariantEdit,
    ) -> Result<(), String> {
        let number = variant_number(id)?.ok_or("Original MIDI cannot be edited.")?;
        let next = expected_revision
            .checked_add(1)
            .filter(|next| *next <= i64::MAX as u64)
            .ok_or("Practice revision limit reached; create a new variant.")?;
        let removed_json = serde_json::to_string(&edit.removed).map_err(json_error)?;
        let undo_json = serde_json::to_string(&edit.undo).map_err(json_error)?;
        // One compare-and-swap statement commits the full batch, history, count
        // and revision together. A stale second editor cannot overwrite it.
        let changed = self
            .db
            .execute(
                "UPDATE variants SET removed_json=?2,undo_json=?3,removed_count=?4,revision=?5
             WHERE variant_id=?1 AND revision=?6 AND EXISTS
                (SELECT 1 FROM songs WHERE id=variants.original_id AND state='ready')",
                params![
                    number,
                    removed_json,
                    undo_json,
                    edit.removed.len() as u32,
                    next as i64,
                    expected_revision as i64
                ],
            )
            .map_err(db_error)?;
        if changed != 1 {
            return Err(
                "Practice variant changed or became unavailable; refresh before editing.".into(),
            );
        }
        Ok(())
    }

    fn find_entry(&self, id: &str) -> Result<Option<LibraryEntry>, String> {
        validate_id(id)?;
        let sql =
            format!("SELECT {ENTRY_COLUMNS} FROM library_entries WHERE id=?1 AND state='ready'");
        self.db
            .query_row(&sql, [id], entry_from_row)
            .optional()
            .map_err(db_error)
    }

    pub fn load(&self, id: &str) -> Result<Song, String> {
        validate_id(id)?;
        if variant_number(id)?.is_some() {
            let edit = self.variant_edit(id)?;
            let mut song = self.load(&edit.original_id)?;
            song.notes.retain(|note| !edit.removed.contains(&note.id));
            let mut counts = std::collections::HashMap::new();
            for note in &song.notes {
                *counts.entry(note.part_id).or_insert(0_usize) += 1;
            }
            for part in &mut song.parts {
                part.note_count = counts.get(&part.id).copied().unwrap_or(0);
            }
            return Ok(song);
        }
        if !self.managed_path(id)?.is_file() {
            return Err(
                "The managed MIDI copy is missing. Import its original again to recover it.".into(),
            );
        }
        let value: String = self
            .db
            .query_row(
                "SELECT song_json FROM songs WHERE id=?1 AND state='ready'",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| "Song is not ready or does not exist.".to_string())?;
        serde_json::from_str(&value).map_err(json_error)
    }

    pub fn settings(&self, id: &str) -> Result<SongSettings, String> {
        validate_id(id)?;
        let value: String = self
            .db
            .query_row(
                "SELECT settings_json FROM library_entries WHERE id=?1 AND state='ready'",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| "Song is not ready or does not exist.".to_string())?;
        serde_json::from_str(&value).map_err(json_error)
    }

    pub fn save_settings(&mut self, id: &str, settings: &SongSettings) -> Result<(), String> {
        let song = self.load(id)?;
        validate_settings(settings, &song)?;
        let value = serde_json::to_string(settings).map_err(json_error)?;
        let sql = if variant_number(id)?.is_some() {
            "UPDATE variants SET settings_json=?2 WHERE 'variant-'||variant_id=?1"
        } else {
            "UPDATE songs SET settings_json=?2 WHERE id=?1 AND state='ready'"
        };
        let changed = self.db.execute(sql, params![id, value]).map_err(db_error)?;
        if changed != 1 {
            return Err("Song is not ready or does not exist.".into());
        }
        Ok(())
    }

    pub fn update_metadata(&mut self, entry: &LibraryEntry) -> Result<(), String> {
        validate_id(&entry.id)?;
        validate_metadata(entry)?;
        self.entry(&entry.id)?;
        let search = search_text(entry);
        let sql = if variant_number(&entry.id)?.is_some() {
            "UPDATE variants SET title=?2,artist=?3,tags=?4,arrangement=?5,favorite=?6,search_text=?7
            WHERE 'variant-'||variant_id=?1"
        } else {
            "UPDATE songs SET title=?2,artist=?3,tags=?4,arrangement=?5,favorite=?6,search_text=?7
            WHERE id=?1 AND state='ready'"
        };
        let changed = self
            .db
            .execute(
                sql,
                params![
                    entry.id,
                    entry.title.trim(),
                    entry.artist.trim(),
                    entry.tags.trim(),
                    entry.arrangement.trim(),
                    entry.favorite,
                    search
                ],
            )
            .map_err(db_error)?;
        if changed != 1 {
            return Err("Song is not ready or does not exist.".into());
        }
        Ok(())
    }

    pub fn mark_played(&mut self, id: &str) -> Result<(), String> {
        self.entry(id)?;
        let sql = if variant_number(id)?.is_some() {
            "UPDATE variants SET last_played=?2 WHERE 'variant-'||variant_id=?1"
        } else {
            "UPDATE songs SET last_played=?2 WHERE id=?1 AND state='ready'"
        };
        let changed = self
            .db
            .execute(sql, params![id, timestamp()])
            .map_err(db_error)?;
        if changed != 1 {
            return Err("Song is not ready or does not exist.".into());
        }
        Ok(())
    }

    /// Caller confirms deletion. Only the hash-named managed file is removed.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.validate_removal(id)?;
        if let Some(number) = variant_number(id)? {
            self.db
                .execute("DELETE FROM variants WHERE variant_id=?1", [number])
                .map_err(db_error)?;
            return Ok(());
        }
        let path = self.managed_path(id)?;
        let changed = self
            .db
            .execute("UPDATE songs SET state='deleting' WHERE id=?1", [id])
            .map_err(db_error)?;
        if changed != 1 {
            return Err("Song does not exist.".into());
        }
        remove_if_exists(&path)?;
        self.db
            .execute("DELETE FROM songs WHERE id=?1", [id])
            .map_err(db_error)?;
        Ok(())
    }

    /// Exact-byte export, using create_new so an external original is never overwritten.
    pub fn export(&self, id: &str, destination: &Path) -> Result<(), String> {
        validate_id(id)?;
        if variant_number(id)?.is_some() {
            return Err(
                "Edited MIDI export is not yet available. Export the original MIDI instead.".into(),
            );
        }
        self.entry(id)?;
        let bytes = read_bounded(&self.managed_path(id)?)?;
        if content_id(&bytes) != id {
            return Err("The managed MIDI copy failed its content-hash check.".into());
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(|error| format!("Export requires a new destination file: {error}"))?;
        let result = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(destination);
            return Err(io_error(error));
        }
        Ok(())
    }

    fn managed_path(&self, id: &str) -> Result<PathBuf, String> {
        validate_original_id(id)?;
        Ok(self.root.join("midi").join(format!("{id}.mid")))
    }

    fn recover(&mut self) -> Result<(), String> {
        let records = {
            let mut statement = self
                .db
                .prepare("SELECT id,state FROM songs")
                .map_err(db_error)?;
            let records = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(db_error)?;
            records.collect::<Result<Vec<_>, _>>().map_err(db_error)?
        };
        let mut known = HashSet::new();
        for (id, state) in records {
            let path = self.managed_path(&id)?;
            if state == "deleting" {
                remove_if_exists(&path)?;
                self.db
                    .execute("DELETE FROM songs WHERE id=?1", [&id])
                    .map_err(db_error)?;
            } else {
                if !path.is_file() {
                    self.db
                        .execute("UPDATE songs SET state='missing' WHERE id=?1", [&id])
                        .map_err(db_error)?;
                }
                known.insert(id);
            }
        }
        // No startup song parsing, source-folder watching, or external scanning.
        for entry in fs::read_dir(self.root.join("midi")).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if validate_original_id(stem).is_err() {
                continue;
            }
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default();
            if extension == "part" || (extension == "mid" && !known.contains(stem)) {
                remove_if_exists(&path)?;
            }
        }
        Ok(())
    }
}

fn validate_settings(settings: &SongSettings, song: &Song) -> Result<(), String> {
    if !settings.speed.is_finite() || !(0.25..=1.5).contains(&settings.speed) {
        return Err("Speed must be between 25% and 150%.".into());
    }
    if !["listen", "wait", "timed"].contains(&settings.mode.as_str()) {
        return Err("Unknown practice mode.".into());
    }
    if !["both", "left", "right"].contains(&settings.split_hand.as_str()) {
        return Err("Choose both, left, or right for approximate pitch-split practice.".into());
    }
    if !settings.position_ms.is_finite()
        || settings.position_ms < 0.0
        || settings.position_ms > song.duration_ms
    {
        return Err("Saved position is outside this song.".into());
    }
    match (settings.loop_start_ms, settings.loop_end_ms) {
        (None, None) => {}
        (Some(start), Some(end))
            if start.is_finite()
                && end.is_finite()
                && start >= 0.0
                && start < end
                && end <= song.duration_ms => {}
        _ => return Err("Loop must have a valid start and end inside this song.".into()),
    }
    if settings.split_pitch.is_some_and(|pitch| pitch > 127) {
        return Err("Split pitch must be a MIDI note from 0 to 127.".into());
    }
    let mut ids = HashSet::new();
    for part in &settings.parts {
        if !song
            .parts
            .iter()
            .any(|candidate| candidate.id == part.part_id)
            || !ids.insert(part.part_id)
        {
            return Err("Part settings contain an unknown or repeated part.".into());
        }
        if !["practice", "accompaniment", "muted"].contains(&part.role.as_str())
            || !["left", "right", "auto"].contains(&part.hand.as_str())
        {
            return Err("Part role or hand label is invalid.".into());
        }
        if !part.volume.is_finite() || !(0.0..=1.0).contains(&part.volume) {
            return Err("Part volume must be between zero and one.".into());
        }
    }
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err("MIDI source must be a regular file.".into());
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err("MIDI exceeds the 16 MiB file safety limit.".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("MIDI exceeds the 16 MiB file safety limit.".into());
    }
    Ok(bytes)
}

fn content_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_id(id: &str) -> Result<(), String> {
    if variant_number(id)?.is_some() {
        return Ok(());
    }
    validate_original_id(id)
}

fn variant_number(id: &str) -> Result<Option<i64>, String> {
    let Some(number) = id.strip_prefix("variant-") else {
        return Ok(None);
    };
    let parsed = number.parse::<i64>().ok().filter(|value| *value > 0);
    match parsed {
        Some(value) if value.to_string() == number => Ok(Some(value)),
        _ => Err("Invalid practice variant ID.".into()),
    }
}

fn validate_original_id(id: &str) -> Result<(), String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Invalid library song ID.".into());
    }
    Ok(())
}

fn check_revision(actual: u64, expected: u64) -> Result<(), String> {
    if actual != expected {
        return Err("Practice variant changed; refresh before editing or undoing.".into());
    }
    Ok(())
}

fn validate_metadata(entry: &LibraryEntry) -> Result<(), String> {
    for value in [&entry.title, &entry.artist, &entry.tags, &entry.arrangement] {
        if value.chars().count() > MAX_METADATA_CHARS || value.contains('\0') {
            return Err(
                "Metadata must be at most 2,048 characters per field, with no null characters."
                    .into(),
            );
        }
    }
    if entry.title.trim().is_empty() {
        return Err("Title cannot be empty.".into());
    }
    Ok(())
}

fn search_text(entry: &LibraryEntry) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        entry.title, entry.artist, entry.tags, entry.arrangement
    )
    .to_lowercase()
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<LibraryEntry> {
    Ok(LibraryEntry {
        id: row.get(0)?,
        original_id: row.get(11)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        tags: row.get(3)?,
        arrangement: row.get(4)?,
        favorite: row.get(5)?,
        imported_at: row.get(6)?,
        last_played: row.get(7)?,
        duration_ms: row.get(8)?,
        parts_count: row.get::<_, u32>(9)? as usize,
        note_count: row.get::<_, u32>(10)? as usize,
    })
}

fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn io_error(error: std::io::Error) -> String {
    format!("File operation failed: {error}")
}
fn db_error(error: rusqlite::Error) -> String {
    format!("Library database operation failed: {error}")
}
fn json_error(error: serde_json::Error) -> String {
    format!("Saved song data is invalid: {error}")
}
fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}
