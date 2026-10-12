pub use crate::model::{ImportResult, LibraryEntry};
use crate::model::{Song, SongSettings};
use crate::parser::{self, MAX_FILE_BYTES};
use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ENTRY_COLUMNS: &str = "id,title,artist,tags,arrangement,favorite,imported_at,last_played,duration_ms,parts_count,note_count";
const MAX_METADATA_CHARS: usize = 2_048;

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
            PRAGMA user_version=1;",
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
            "SELECT {ENTRY_COLUMNS} FROM songs WHERE state='ready'
            AND (?1=0 OR favorite=1) AND (?2='' OR instr(search_text,?2)>0)
            ORDER BY imported_at DESC, rowid DESC"
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

    fn find_entry(&self, id: &str) -> Result<Option<LibraryEntry>, String> {
        validate_id(id)?;
        let sql = format!("SELECT {ENTRY_COLUMNS} FROM songs WHERE id=?1 AND state='ready'");
        self.db
            .query_row(&sql, [id], entry_from_row)
            .optional()
            .map_err(db_error)
    }

    pub fn load(&self, id: &str) -> Result<Song, String> {
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
                "SELECT settings_json FROM songs WHERE id=?1 AND state='ready'",
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
        self.db
            .execute(
                "UPDATE songs SET settings_json=?2 WHERE id=?1 AND state='ready'",
                params![id, value],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn update_metadata(&mut self, entry: &LibraryEntry) -> Result<(), String> {
        validate_id(&entry.id)?;
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
        let search = format!(
            "{}\n{}\n{}\n{}",
            entry.title, entry.artist, entry.tags, entry.arrangement
        )
        .to_lowercase();
        let changed = self.db.execute("UPDATE songs SET title=?2,artist=?3,tags=?4,arrangement=?5,favorite=?6,search_text=?7
            WHERE id=?1 AND state='ready'", params![entry.id, entry.title.trim(), entry.artist.trim(), entry.tags.trim(),
                entry.arrangement.trim(), entry.favorite, search]).map_err(db_error)?;
        if changed != 1 {
            return Err("Song is not ready or does not exist.".into());
        }
        Ok(())
    }

    pub fn mark_played(&mut self, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let changed = self
            .db
            .execute(
                "UPDATE songs SET last_played=?2 WHERE id=?1 AND state='ready'",
                params![id, timestamp()],
            )
            .map_err(db_error)?;
        if changed != 1 {
            return Err("Song is not ready or does not exist.".into());
        }
        Ok(())
    }

    /// Caller confirms deletion. Only the hash-named managed file is removed.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
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
        validate_id(id)?;
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
            if validate_id(stem).is_err() {
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
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Invalid library song ID.".into());
    }
    Ok(())
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<LibraryEntry> {
    Ok(LibraryEntry {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        tags: row.get(3)?,
        arrangement: row.get(4)?,
        favorite: row.get(5)?,
        imported_at: row.get(6)?,
        last_played: row.get(7)?,
        duration_ms: row.get(8)?,
        parts_count: row.get::<_,u32>(9)? as usize,
        note_count: row.get::<_,u32>(10)? as usize,
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
