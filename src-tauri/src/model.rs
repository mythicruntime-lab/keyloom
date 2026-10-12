use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub ppq: u16,
    pub duration_ms: f64,
    pub notes: Vec<Note>,
    pub parts: Vec<Part>,
    pub tempos: Vec<Tempo>,
    pub time_signatures: Vec<TimeSignature>,
    pub controllers: Vec<Controller>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub id: u32,
    pub part_id: u32,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub end_tick: u64,
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub id: u32,
    pub track: u32,
    pub channel: u8,
    pub name: String,
    pub note_count: usize,
    pub percussion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tempo {
    pub tick: u64,
    pub micros_per_quarter: u32,
    pub time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeSignature {
    pub tick: u64,
    pub numerator: u8,
    pub denominator: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Controller {
    pub part_id: u32,
    pub channel: u8,
    pub tick: u64,
    pub time_ms: f64,
    pub controller: u8,
    pub value: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SongSettings {
    pub speed: f64,
    pub mode: String,
    pub parts: Vec<PartSetting>,
    pub loop_start_ms: Option<f64>,
    pub loop_end_ms: Option<f64>,
    pub position_ms: f64,
    pub split_pitch: Option<u8>,
    pub count_in: bool,
    pub metronome: bool,
}

impl Default for SongSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            mode: "listen".into(),
            parts: Vec::new(),
            loop_start_ms: None,
            loop_end_ms: None,
            position_ms: 0.0,
            split_pitch: None,
            count_in: false,
            metronome: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartSetting {
    pub part_id: u32,
    pub role: String,
    pub hand: String,
    pub volume: f32,
    pub solo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryEntry {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub tags: String,
    pub arrangement: String,
    pub favorite: bool,
    pub imported_at: i64,
    pub last_played: Option<i64>,
    pub duration_ms: f64,
    pub parts_count: usize,
    pub note_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub entry: LibraryEntry,
    pub duplicate: bool,
    pub elapsed_ms: f64,
}
