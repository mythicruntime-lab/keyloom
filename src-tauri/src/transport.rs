//! Deterministic musical transport. Only the owner supplies monotonic elapsed time.
use crate::model::{Song, SongSettings};
use serde::Serialize;
use std::collections::BTreeSet;

const GROUP_MS: f64 = 30.0;
const INPUT_WINDOW_MS: f64 = 150.0;
const EPS: f64 = 0.000_01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Physical,
    Simulated,
    Playback,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    On {
        key: u32,
        pitch: u8,
        velocity: u8,
        volume: f32,
    },
    Off {
        key: u32,
    },
    Reset,
    Click,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransportSnapshot {
    pub position_ms: f64,
    pub duration_ms: f64,
    pub playing: bool,
    pub waiting: bool,
    pub mode: String,
    pub speed: f64,
    pub targets: Vec<u8>,
    pub matched: Vec<u8>,
    pub correct: u32,
    pub wrong: u32,
    pub missed: u32,
    pub feedback: String,
    pub beat: u32,
    pub count_in_remaining: u8,
}

impl Default for TransportSnapshot {
    fn default() -> Self {
        Self {
            position_ms: 0.0,
            duration_ms: 0.0,
            playing: false,
            waiting: false,
            mode: "listen".into(),
            speed: 1.0,
            targets: Vec::new(),
            matched: Vec::new(),
            correct: 0,
            wrong: 0,
            missed: 0,
            feedback: "Open a MIDI to begin".into(),
            beat: 1,
            count_in_remaining: 0,
        }
    }
}

#[derive(Clone)]
struct Group {
    time: f64,
    pitches: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Scheduled {
    time: f64,
    note: usize,
    on: bool,
}

/// Single-thread owned; never called from the audio callback. Scheduled voice keys
/// are canonical note IDs. The audio owner must give live monitoring a separate key range.
pub struct Engine {
    song: Option<Song>,
    settings: SongSettings,
    position: f64,
    playing: bool,
    waiting: bool,
    wall_ms: f64,
    groups: Vec<Group>,
    group_index: usize,
    guide_revision: u64,
    matches: [Option<f64>; 128],
    // Timed pitch/onset credit outlives the shorter live chord-entry state.
    // Each eligible pitch earns credit once even if the rest of its chord is missed.
    timed_credit: [bool; 128],
    held: [bool; 128],
    timeline: Vec<Scheduled>,
    event_index: usize,
    audio_ends: Vec<f64>,
    volumes: Vec<Option<f32>>,
    logical_voices: BTreeSet<usize>,
    sounding_voices: BTreeSet<usize>,
    pending: Vec<VoiceEvent>,
    next_beat: f64,
    count_in_remaining: u8,
    count_in_until: f64,
    correct: u32,
    wrong: u32,
    missed: u32,
    feedback: String,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            song: None,
            settings: SongSettings::default(),
            position: 0.0,
            playing: false,
            waiting: false,
            wall_ms: 0.0,
            groups: Vec::new(),
            group_index: 0,
            guide_revision: 0,
            matches: [None; 128],
            timed_credit: [false; 128],
            held: [false; 128],
            timeline: Vec::new(),
            event_index: 0,
            audio_ends: Vec::new(),
            volumes: Vec::new(),
            logical_voices: BTreeSet::new(),
            sounding_voices: BTreeSet::new(),
            pending: Vec::new(),
            next_beat: 0.0,
            count_in_remaining: 0,
            count_in_until: 0.0,
            correct: 0,
            wrong: 0,
            missed: 0,
            feedback: "Open a MIDI to begin".into(),
        }
    }

    pub fn load(&mut self, song: Song, settings: SongSettings) {
        self.song = Some(song);
        self.settings = settings;
        self.sanitize_settings();
        self.playing = false;
        self.held = [false; 128];
        self.correct = 0;
        self.wrong = 0;
        self.missed = 0;
        self.rebuild();
        self.seek(self.settings.position_ms);
        self.feedback = "Ready".into();
    }

    pub fn play(&mut self) {
        if self.song.is_none() || self.playing {
            return;
        }
        if self.position >= self.duration() {
            self.seek(0.0);
        }
        self.playing = true;
        if self.settings.count_in {
            self.count_in_remaining = self.meter_at(self.position).0.clamp(1, 16) as u8;
            self.count_in_until = self.beat_duration(self.position) / self.settings.speed;
            self.pending.push(VoiceEvent::Click);
        } else if !self.waiting {
            self.resume_voices();
        }
        self.feedback = "Playing".into();
    }

    /// Allocation-free owner-loop state check; presentation snapshots stay at 30 Hz.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.count_in_remaining = 0;
        self.silence_voices();
        self.feedback = "Paused".into();
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.seek(0.0);
        self.correct = 0;
        self.wrong = 0;
        self.missed = 0;
        self.feedback = "Stopped".into();
    }

    /// Seeking rebuilds crossing voices once. It never makes already held keys
    /// count as fresh input and never emits an imported MIDI device command.
    pub fn seek(&mut self, ms: f64) {
        self.bump_guide_revision();
        self.position = finite(ms, 0.0).clamp(0.0, self.duration());
        self.settings.position_ms = self.position;
        self.waiting = false;
        self.count_in_remaining = 0;
        self.matches = [None; 128];
        self.timed_credit = [false; 128];
        self.group_index = self
            .groups
            .partition_point(|g| g.time + EPS < self.position);
        self.event_index = self
            .timeline
            .partition_point(|e| e.time + EPS < self.position);
        self.pending.push(VoiceEvent::Reset);
        self.logical_voices.clear();
        self.sounding_voices.clear();
        if let Some(song) = &self.song {
            for (i, n) in song.notes.iter().enumerate() {
                if self.volumes[i].is_some()
                    && n.start_ms + EPS < self.position
                    && self.audio_ends[i] > self.position + EPS
                {
                    self.logical_voices.insert(i);
                }
            }
        }
        self.next_beat = self.next_beat_at(self.position);
        if self.playing {
            self.resume_voices();
        }
        self.feedback = "Position changed".into();
    }

    /// Part/mode/loop/split changes restart matching at the current position. Pure
    /// speed/presentation changes preserve the active target and voices.
    /// `position_ms` is persisted data; only `seek` changes position.
    pub fn configure(&mut self, settings: SongSettings) {
        let pos = self.position;
        let old = self.settings.clone();
        self.settings = settings;
        self.sanitize_settings();
        if old.mode != self.settings.mode
            || old.parts != self.settings.parts
            || old.split_pitch != self.settings.split_pitch
            || old.split_hand != self.settings.split_hand
            || old.loop_start_ms != self.settings.loop_start_ms
            || old.loop_end_ms != self.settings.loop_end_ms
        {
            self.rebuild();
            self.seek(pos);
        } else {
            self.settings.position_ms = pos;
            if !old.metronome && self.settings.metronome {
                self.next_beat = self.next_beat_at(pos);
            }
            if self.count_in_remaining > 0 {
                self.count_in_until *= old.speed / self.settings.speed;
            }
        }
        self.feedback = "Practice settings updated".into();
    }

    /// Advance uses *wall* milliseconds, even while waiting, so chord-entry
    /// tolerance does not grow when musical time freezes. It returns all queued
    /// command/input events as well as events due during this interval.
    pub fn advance(&mut self, elapsed_ms: f64) -> Vec<VoiceEvent> {
        let elapsed = finite(elapsed_ms, 0.0).max(0.0);
        self.wall_ms += elapsed;
        self.expire_matches();
        if !self.playing {
            return self.drain_events();
        }
        let mut remaining = elapsed;
        if self.count_in_remaining > 0 {
            while self.count_in_remaining > 0 && remaining + EPS >= self.count_in_until {
                remaining = (remaining - self.count_in_until).max(0.0);
                self.count_in_remaining -= 1;
                if self.count_in_remaining > 0 {
                    self.pending.push(VoiceEvent::Click);
                    self.count_in_until = self.beat_duration(self.position) / self.settings.speed;
                }
            }
            if self.count_in_remaining > 0 {
                self.count_in_until -= remaining;
                return self.drain_events();
            }
            if !self.waiting {
                self.resume_voices();
            }
        }
        if self.waiting {
            return self.drain_events();
        }
        // Limit pathological tiny-loop catch-up after a long OS stall. Normal
        // playback is exact; over 64 wraps in one update pauses with an explanation.
        for wrap in 0..65 {
            let loop_range = self.loop_range();
            let boundary = loop_range.map_or(self.duration(), |(_, end)| end);
            // A saved position beyond B first re-enters the configured loop.
            if self.position >= boundary - EPS {
                if let Some((start, _)) = loop_range {
                    self.wrap_loop(start);
                }
            }
            let end = (self.position + remaining * self.settings.speed).min(boundary);
            if self.settings.mode == "wait" {
                if let Some(group) = self.groups.get(self.group_index) {
                    if group.time <= end + EPS && group.time < boundary - EPS {
                        let gate = group.time.max(self.position);
                        self.process_until(gate, false);
                        self.position = gate;
                        self.waiting = true;
                        self.silence_voices();
                        self.feedback = "Play the highlighted pitches".into();
                        break;
                    }
                }
            }
            let consumed = (end - self.position).max(0.0) / self.settings.speed;
            // B is exclusive. Its onsets belong to material outside the loop;
            // the wrap's Reset releases voices whose offs are exactly at B.
            let include_end = loop_range.is_none() || end < boundary - EPS;
            self.process_until(end, include_end);
            self.position = end;
            self.expire_timed_groups();
            remaining = (remaining - consumed).max(0.0);
            if self.position + EPS < boundary {
                break;
            }
            // End-of-song/loop closes the remaining onset windows. An already
            // completed group is behind group_index and cannot be counted twice.
            self.finalize_timed_boundary(boundary, loop_range.is_none());
            if let Some((start, _)) = loop_range {
                if wrap == 64 {
                    self.pause();
                    self.feedback = "Paused after a clock gap spanning over 64 loops".into();
                    break;
                }
                self.wrap_loop(start);
                if remaining <= EPS {
                    break;
                }
            } else {
                self.playing = false;
                self.silence_voices();
                self.feedback = "Finished".into();
                break;
            }
        }
        self.settings.position_ms = self.position;
        self.drain_events()
    }

    pub fn input(&mut self, pitch: u8, on: bool, velocity: u8, source: InputSource) {
        // Screen/test tones and guide/playback output are never player input.
        // Reject their releases too: they must not rearm a physically held key.
        if pitch > 127 || source == InputSource::Simulated || source == InputSource::Playback {
            return;
        }
        let p = usize::from(pitch);
        let down = on && velocity > 0;
        let fresh = down && !self.held[p];
        self.held[p] = down;
        if !fresh || !self.playing || self.count_in_remaining > 0 || self.settings.mode == "listen"
        {
            return;
        }
        self.expire_matches();
        self.expire_timed_groups();
        let Some(group) = self.groups.get(self.group_index) else {
            return;
        };
        let eligible = if self.settings.mode == "wait" {
            self.waiting
        } else {
            (group.time - self.position).abs() / self.settings.speed <= INPUT_WINDOW_MS
        };
        if !eligible {
            return;
        }
        if !group.pitches.contains(&pitch) {
            self.wrong = self.wrong.saturating_add(1);
            self.feedback = format!("Extra pitch {pitch}; target remains available");
            return;
        }
        self.matches[p] = Some(self.wall_ms);
        if self.settings.mode == "timed" {
            self.timed_credit[p] = true;
        }
        if group
            .pitches
            .iter()
            .all(|p| self.matches[usize::from(*p)].is_some())
        {
            self.correct = self.correct.saturating_add(group.pitches.len() as u32);
            self.group_index += 1;
            self.bump_guide_revision();
            self.matches = [None; 128];
            self.timed_credit = [false; 128];
            self.waiting = false;
            self.feedback = "Correct pitches".into();
            self.resume_voices();
        } else {
            self.feedback = "Complete the chord within 150 ms".into();
        }
    }

    /// Read-only intent for an explicitly enabled external Light Guide. This is
    /// never fed to `input`: only the separate physical MIDI path can score.
    ///
    /// Identity is stable for one group, including partial chords, and changes
    /// on group advancement, seek, loop, load, or rebuilt practice selection.
    /// It is scoped to this Engine instance; consumers must clear their cached
    /// identity when an engine/device/session is replaced or this returns None.
    /// The full chord is returned so guide output need not flicker after one hit.
    pub fn guide_targets(&self) -> Option<(u64, Vec<u8>)> {
        if !self.playing || self.count_in_remaining > 0 {
            return None;
        }
        let group = self.groups.get(self.group_index)?;
        let eligible = match self.settings.mode.as_str() {
            "wait" => self.waiting,
            "timed" => (group.time - self.position).abs() / self.settings.speed <= INPUT_WINDOW_MS,
            _ => false,
        };
        eligible.then(|| (self.guide_revision, group.pitches.clone()))
    }

    pub fn snapshot(&self) -> TransportSnapshot {
        let active = self.settings.mode != "listen" && self.count_in_remaining == 0;
        TransportSnapshot {
            position_ms: self.position,
            duration_ms: self.duration(),
            playing: self.playing,
            waiting: self.waiting,
            mode: self.settings.mode.clone(),
            speed: self.settings.speed,
            targets: if active {
                self.groups
                    .get(self.group_index)
                    .map_or_else(Vec::new, |g| g.pitches.clone())
            } else {
                Vec::new()
            },
            matched: self
                .matches
                .iter()
                .enumerate()
                .filter_map(|(p, t)| t.map(|_| p as u8))
                .collect(),
            correct: self.correct,
            wrong: self.wrong,
            missed: self.missed,
            feedback: self.feedback.clone(),
            beat: self.beat_number(self.position),
            count_in_remaining: self.count_in_remaining,
        }
    }

    pub fn drain_events(&mut self) -> Vec<VoiceEvent> {
        std::mem::take(&mut self.pending)
    }

    fn duration(&self) -> f64 {
        self.song
            .as_ref()
            .map_or(0.0, |s| finite(s.duration_ms, 0.0).max(0.0))
    }

    fn sanitize_settings(&mut self) {
        self.settings.speed = finite(self.settings.speed, 1.0).clamp(0.25, 1.5);
        if !matches!(self.settings.mode.as_str(), "listen" | "wait" | "timed") {
            self.settings.mode = "listen".into();
        }
        if !matches!(self.settings.split_hand.as_str(), "both" | "left" | "right") {
            self.settings.split_hand = "both".into();
        }
        for p in &mut self.settings.parts {
            p.volume = if p.volume.is_finite() {
                p.volume.clamp(0.0, 1.0)
            } else {
                1.0
            };
        }
    }

    fn loop_range(&self) -> Option<(f64, f64)> {
        let start = self.settings.loop_start_ms?;
        let end = self.settings.loop_end_ms?;
        if !start.is_finite() || !end.is_finite() {
            return None;
        }
        let start = start.clamp(0.0, self.duration());
        let end = end.clamp(0.0, self.duration());
        (end - start >= 1.0).then_some((start, end))
    }

    fn rebuild(&mut self) {
        self.bump_guide_revision();
        self.groups.clear();
        self.timeline.clear();
        self.audio_ends.clear();
        self.volumes.clear();
        let loop_range = self.loop_range();
        let Some(song) = &self.song else {
            return;
        };
        let any_solo = self.settings.parts.iter().any(|p| p.solo);
        let mut pedal: [Vec<(f64, bool)>; 16] = std::array::from_fn(|_| Vec::new());
        for c in &song.controllers {
            if c.controller == 64 && c.channel < 16 {
                pedal[usize::from(c.channel)].push((c.time_ms, c.value >= 64));
            }
        }
        for channel in &mut pedal {
            channel.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        // For each controller index, cache the next release: note end lookup is
        // logarithmic rather than scanning the pedal track once for every note.
        let next_up: [Vec<f64>; 16] = std::array::from_fn(|ch| {
            let mut result = vec![song.duration_ms; pedal[ch].len()];
            let mut up = song.duration_ms;
            for i in (0..pedal[ch].len()).rev() {
                if !pedal[ch][i].1 {
                    up = pedal[ch][i].0;
                }
                result[i] = up;
            }
            result
        });
        let mut target_notes = Vec::new();
        for (i, note) in song.notes.iter().enumerate() {
            let part = song.parts.iter().find(|p| p.id == note.part_id);
            let setting = self
                .settings
                .parts
                .iter()
                .find(|p| p.part_id == note.part_id);
            let percussion = note.channel == 9 || part.is_some_and(|p| p.percussion);
            let role = setting.map_or(
                if percussion {
                    "accompaniment"
                } else {
                    "practice"
                },
                |p| p.role.as_str(),
            );
            let enabled = role != "muted" && (!any_solo || setting.is_some_and(|p| p.solo));
            let selected_split = self.settings.split_pitch.is_none_or(|boundary| {
                match self.settings.split_hand.as_str() {
                    "left" => note.pitch < boundary,
                    "right" => note.pitch >= boundary,
                    _ => true,
                }
            });
            let target = enabled && role == "practice" && !percussion && selected_split;
            if target && loop_range.is_none_or(|(_, end)| note.start_ms < end - EPS) {
                target_notes.push((note.start_ms, note.pitch));
            }
            let volume = if enabled && (self.settings.mode == "listen" || !target) {
                Some(setting.map_or(1.0, |p| p.volume))
            } else {
                None
            };
            self.volumes.push(volume);
            let ch = usize::from(note.channel.min(15));
            let at = pedal[ch].partition_point(|(time, _)| *time <= note.end_ms + EPS);
            let end = if at > 0 && pedal[ch][at - 1].1 {
                next_up[ch][at - 1].max(note.end_ms)
            } else {
                note.end_ms
            };
            self.audio_ends.push(end);
            if volume.is_some() && end > note.start_ms {
                self.timeline.push(Scheduled {
                    time: note.start_ms,
                    note: i,
                    on: true,
                });
                self.timeline.push(Scheduled {
                    time: end,
                    note: i,
                    on: false,
                });
            }
        }
        self.timeline.sort_by(|a, b| {
            a.time
                .total_cmp(&b.time)
                .then(a.on.cmp(&b.on))
                .then(a.note.cmp(&b.note))
        });
        target_notes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (time, pitch) in target_notes {
            if let Some(group) = self.groups.last_mut() {
                // A chord cannot cross A: seeking to A must retain its pitches
                // rather than skip a group anchored just before the loop.
                let crosses_loop_start = loop_range
                    .is_some_and(|(start, _)| group.time < start - EPS && time >= start - EPS);
                if !crosses_loop_start
                    && time - group.time <= GROUP_MS
                    && !group.pitches.contains(&pitch)
                {
                    group.pitches.push(pitch);
                    group.pitches.sort_unstable();
                    continue;
                }
                // Duplicate pitches at exactly one onset from layered tracks
                // are one physical target; later repetitions remain separate.
                if (time - group.time).abs() < EPS && group.pitches.contains(&pitch) {
                    continue;
                }
            }
            self.groups.push(Group {
                time,
                pitches: vec![pitch],
            });
        }
    }

    fn emit_on(&mut self, index: usize) {
        if self.sounding_voices.insert(index) {
            let note = &self.song.as_ref().unwrap().notes[index];
            self.pending.push(VoiceEvent::On {
                key: note.id,
                pitch: note.pitch,
                velocity: note.velocity,
                volume: self.volumes[index].unwrap_or(0.0),
            });
        }
    }

    fn silence_voices(&mut self) {
        if let Some(song) = &self.song {
            for &i in &self.sounding_voices {
                self.pending.push(VoiceEvent::Off {
                    key: song.notes[i].id,
                });
            }
        }
        self.sounding_voices.clear();
    }

    fn resume_voices(&mut self) {
        let notes: Vec<_> = self.logical_voices.iter().copied().collect();
        for i in notes {
            self.emit_on(i);
        }
    }

    fn process_until(&mut self, end: f64, inclusive: bool) {
        while let Some(event) = self.timeline.get(self.event_index).copied() {
            let due = if inclusive {
                event.time <= end + EPS
            } else {
                event.time < end - EPS
            };
            if !due {
                break;
            }
            self.event_index += 1;
            if event.on {
                self.logical_voices.insert(event.note);
                self.emit_on(event.note);
            } else {
                self.logical_voices.remove(&event.note);
                if self.sounding_voices.remove(&event.note) {
                    self.pending.push(VoiceEvent::Off {
                        key: self.song.as_ref().unwrap().notes[event.note].id,
                    });
                }
            }
        }
        if self.settings.metronome {
            let mut clicks = 0;
            while if inclusive {
                self.next_beat <= end + EPS
            } else {
                self.next_beat < end - EPS
            } {
                if clicks < 128 {
                    self.pending.push(VoiceEvent::Click);
                }
                clicks += 1;
                if clicks >= 128 {
                    self.next_beat = self.next_beat_at(end + 0.001);
                    break;
                }
                self.next_beat = self.next_beat_at(self.next_beat + 0.001);
            }
        }
    }

    fn wrap_loop(&mut self, start: f64) {
        self.seek(start);
        self.feedback = "Loop restarted; release and replay repeated targets".into();
    }

    fn expire_matches(&mut self) {
        let mut expired = false;
        for m in &mut self.matches {
            if m.is_some_and(|t| self.wall_ms - t > INPUT_WINDOW_MS + EPS) {
                *m = None;
                expired = true;
            }
        }
        if expired {
            self.feedback = if self.settings.mode == "timed" {
                "Chord entry expired; on-time pitch credit is retained"
            } else {
                "Chord entry expired; release and replay its pitches"
            }
            .into();
        }
    }

    fn expire_timed_groups(&mut self) {
        if self.settings.mode != "timed" {
            return;
        }
        while let Some(group) = self.groups.get(self.group_index) {
            if self.position <= group.time + INPUT_WINDOW_MS * self.settings.speed + EPS {
                break;
            }
            self.score_unfinished_group();
            self.feedback = "Target window passed".into();
        }
    }

    fn finalize_timed_boundary(&mut self, boundary: f64, inclusive: bool) {
        if self.settings.mode != "timed" {
            return;
        }
        while let Some(group) = self.groups.get(self.group_index) {
            let included = if inclusive {
                group.time <= boundary + EPS
            } else {
                group.time < boundary - EPS
            };
            if !included {
                break;
            }
            self.score_unfinished_group();
        }
    }

    fn score_unfinished_group(&mut self) {
        let group = &self.groups[self.group_index];
        let matched = group
            .pitches
            .iter()
            .filter(|p| self.timed_credit[usize::from(**p)])
            .count() as u32;
        self.correct = self.correct.saturating_add(matched);
        self.missed = self
            .missed
            .saturating_add(group.pitches.len() as u32 - matched);
        self.group_index += 1;
        self.bump_guide_revision();
        self.matches = [None; 128];
        self.timed_credit = [false; 128];
    }

    fn bump_guide_revision(&mut self) {
        self.guide_revision = self.guide_revision.wrapping_add(1);
    }

    fn tick_at(&self, ms: f64) -> f64 {
        let Some(song) = &self.song else {
            return 0.0;
        };
        let i = song.tempos.partition_point(|t| t.time_ms <= ms);
        let (tick, time, micros) = if i > 0 {
            let t = &song.tempos[i - 1];
            (
                t.tick as f64,
                t.time_ms,
                f64::from(t.micros_per_quarter.max(1)),
            )
        } else {
            (0.0, 0.0, 500_000.0)
        };
        tick + (ms - time) * f64::from(song.ppq.max(1)) * 1000.0 / micros
    }

    fn ms_at_tick(&self, tick: f64) -> f64 {
        let Some(song) = &self.song else {
            return 0.0;
        };
        let i = song.tempos.partition_point(|t| t.tick as f64 <= tick);
        let (start, time, micros) = if i > 0 {
            let t = &song.tempos[i - 1];
            (
                t.tick as f64,
                t.time_ms,
                f64::from(t.micros_per_quarter.max(1)),
            )
        } else {
            (0.0, 0.0, 500_000.0)
        };
        time + (tick - start) * micros / f64::from(song.ppq.max(1)) / 1000.0
    }

    fn meter_at(&self, ms: f64) -> (u32, f64, f64) {
        let Some(song) = &self.song else {
            return (4, 480.0, 0.0);
        };
        let tick = self.tick_at(ms);
        let i = song
            .time_signatures
            .partition_point(|t| t.tick as f64 <= tick + EPS);
        if i > 0 {
            let s = &song.time_signatures[i - 1];
            (
                u32::from(s.numerator.max(1)),
                f64::from(song.ppq.max(1)) * 4.0 / f64::from(s.denominator.max(1)),
                s.tick as f64,
            )
        } else {
            (4, f64::from(song.ppq.max(1)), 0.0)
        }
    }

    fn beat_number(&self, ms: f64) -> u32 {
        let (numerator, ticks, anchor) = self.meter_at(ms);
        (((self.tick_at(ms) - anchor) / ticks).floor().max(0.0) as u64 % u64::from(numerator))
            as u32
            + 1
    }

    fn beat_duration(&self, ms: f64) -> f64 {
        let (_, ticks, _) = self.meter_at(ms);
        (self.ms_at_tick(self.tick_at(ms) + ticks) - ms).max(1.0)
    }

    fn next_beat_at(&self, ms: f64) -> f64 {
        let (_, ticks, anchor) = self.meter_at(ms);
        let tick = self.tick_at(ms);
        let mut next_tick = anchor + ((tick - anchor) / ticks).ceil() * ticks;
        if let Some(song) = &self.song {
            let i = song
                .time_signatures
                .partition_point(|s| s.tick as f64 <= tick + EPS);
            if let Some(sig) = song.time_signatures.get(i) {
                next_tick = next_tick.min(sig.tick as f64);
            }
        }
        self.ms_at_tick(next_tick)
    }
}

fn finite(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Controller, Note, Part, PartSetting, Tempo, TimeSignature};

    fn song(notes: &[(u8, f64, f64, u32)]) -> Song {
        Song {
            ppq: 480,
            duration_ms: 3000.0,
            notes: notes
                .iter()
                .enumerate()
                .map(|(i, &(pitch, start_ms, end_ms, part_id))| Note {
                    id: i as u32,
                    part_id,
                    channel: part_id as u8,
                    pitch,
                    velocity: 90,
                    start_tick: (start_ms * 0.96) as u64,
                    end_tick: (end_ms * 0.96) as u64,
                    start_ms,
                    end_ms,
                })
                .collect(),
            parts: vec![
                Part {
                    id: 0,
                    track: 0,
                    channel: 0,
                    name: "Practice".into(),
                    note_count: notes.len(),
                    percussion: false,
                },
                Part {
                    id: 1,
                    track: 1,
                    channel: 1,
                    name: "Accompaniment".into(),
                    note_count: 1,
                    percussion: false,
                },
            ],
            tempos: vec![Tempo {
                tick: 0,
                micros_per_quarter: 500_000,
                time_ms: 0.0,
            }],
            time_signatures: vec![TimeSignature {
                tick: 0,
                numerator: 4,
                denominator: 4,
            }],
            controllers: vec![],
            warnings: vec![],
        }
    }

    fn engine(mode: &str, notes: &[(u8, f64, f64, u32)]) -> Engine {
        let mut e = Engine::new();
        let settings = SongSettings {
            mode: mode.into(),
            ..SongSettings::default()
        };
        e.load(song(notes), settings);
        e.play();
        e.drain_events();
        e
    }

    fn press(e: &mut Engine, pitch: u8) {
        // Synthetic deterministic harness injection of the physical-path tag;
        // this does not verify any real keyboard, MIDI adapter, or Light Guide.
        e.input(pitch, true, 100, InputSource::Physical);
    }
    fn release(e: &mut Engine, pitch: u8) {
        e.input(pitch, false, 0, InputSource::Physical);
    }

    #[test]
    fn wait_freezes_and_requires_fresh_repeated_onsets() {
        let mut e = engine("wait", &[(60, 100.0, 200.0, 0), (60, 300.0, 400.0, 0)]);
        e.advance(1000.0);
        assert_eq!(e.snapshot().position_ms, 100.0);
        assert!(e.snapshot().waiting);
        e.input(60, true, 100, InputSource::Playback);
        assert!(e.snapshot().waiting);
        press(&mut e, 60);
        e.advance(1000.0);
        assert_eq!(e.snapshot().position_ms, 300.0);
        press(&mut e, 60);
        assert!(e.snapshot().waiting);
        release(&mut e, 60);
        press(&mut e, 60);
        assert!(!e.snapshot().waiting);
        assert_eq!(e.snapshot().correct, 2);
    }

    #[test]
    fn chord_window_and_wrong_extras_do_not_deadlock() {
        let mut e = engine("wait", &[(60, 0.0, 400.0, 0), (64, 20.0, 400.0, 0)]);
        e.advance(0.0);
        press(&mut e, 70);
        press(&mut e, 60);
        e.advance(151.0);
        press(&mut e, 64);
        assert!(e.snapshot().waiting);
        assert_eq!(e.snapshot().matched, vec![64]);
        release(&mut e, 60);
        press(&mut e, 60);
        assert!(!e.snapshot().waiting);
        assert_eq!(e.snapshot().correct, 2);
        assert_eq!(e.snapshot().wrong, 1);
    }

    #[test]
    fn held_overlapping_transition_requires_rearticulation() {
        let mut e = engine(
            "wait",
            &[
                (60, 0.0, 500.0, 0),
                (60, 200.0, 700.0, 0),
                (64, 200.0, 400.0, 0),
            ],
        );
        e.advance(0.0);
        press(&mut e, 60);
        e.advance(200.0);
        press(&mut e, 64);
        press(&mut e, 60);
        assert!(e.snapshot().waiting);
        release(&mut e, 60);
        press(&mut e, 60);
        assert!(!e.snapshot().waiting);
    }

    #[test]
    fn wait_releases_and_resumes_crossing_accompaniment() {
        let mut e = engine("wait", &[(48, 0.0, 1000.0, 1), (60, 100.0, 300.0, 0)]);
        e.configure(SongSettings {
            mode: "wait".into(),
            parts: vec![PartSetting {
                part_id: 1,
                role: "accompaniment".into(),
                hand: "left".into(),
                volume: 0.4,
                solo: false,
            }],
            ..SongSettings::default()
        });
        e.drain_events();
        let events = e.advance(100.0);
        assert!(events.contains(&VoiceEvent::On {
            key: 0,
            pitch: 48,
            velocity: 90,
            volume: 0.4
        }));
        assert!(events.contains(&VoiceEvent::Off { key: 0 }));
        assert!(e.advance(1000.0).is_empty());
        press(&mut e, 60);
        assert!(e
            .drain_events()
            .iter()
            .any(|e| matches!(e, VoiceEvent::On { key: 0, .. })));
        assert!(e.advance(900.0).contains(&VoiceEvent::Off { key: 0 }));
    }

    #[test]
    fn sustain_changes_audio_release_not_canonical_key_end() {
        let mut s = song(&[(60, 0.0, 100.0, 0)]);
        s.controllers = vec![
            Controller {
                part_id: 0,
                channel: 0,
                tick: 0,
                time_ms: 0.0,
                controller: 64,
                value: 127,
            },
            Controller {
                part_id: 0,
                channel: 0,
                tick: 480,
                time_ms: 500.0,
                controller: 64,
                value: 0,
            },
        ];
        let mut e = Engine::new();
        e.load(
            s,
            SongSettings {
                mode: "listen".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        assert!(!e.advance(200.0).contains(&VoiceEvent::Off { key: 0 }));
        assert_eq!(e.song.as_ref().unwrap().notes[0].end_ms, 100.0);
        assert!(e.advance(300.0).contains(&VoiceEvent::Off { key: 0 }));
    }

    #[test]
    fn seek_and_pause_restore_crossing_voices_without_pitch_change() {
        let mut e = engine("listen", &[(67, 0.0, 1000.0, 0)]);
        e.seek(500.0);
        let events = e.drain_events();
        assert_eq!(events[0], VoiceEvent::Reset);
        assert!(events
            .iter()
            .any(|e| matches!(e, VoiceEvent::On { pitch: 67, .. })));
        e.pause();
        assert!(e.drain_events().contains(&VoiceEvent::Off { key: 0 }));
        e.advance(1000.0);
        assert_eq!(e.snapshot().position_ms, 500.0);
        e.play();
        assert!(e
            .drain_events()
            .iter()
            .any(|e| matches!(e, VoiceEvent::On { pitch: 67, .. })));
    }

    #[test]
    fn speed_and_tempo_map_drive_song_time_and_metronome() {
        let mut s = song(&[(60, 1000.0, 1500.0, 0)]);
        s.tempos.push(Tempo {
            tick: 480,
            time_ms: 500.0,
            micros_per_quarter: 1_000_000,
        });
        let mut e = Engine::new();
        e.load(
            s,
            SongSettings {
                mode: "listen".into(),
                speed: 0.5,
                metronome: true,
                ..SongSettings::default()
            },
        );
        e.play();
        let a = e.advance(1000.0);
        assert_eq!(e.snapshot().position_ms, 500.0);
        assert_eq!(a.iter().filter(|e| **e == VoiceEvent::Click).count(), 2);
        let b = e.advance(2000.0);
        assert_eq!(e.snapshot().position_ms, 1500.0);
        assert_eq!(b.iter().filter(|e| **e == VoiceEvent::Click).count(), 1);
        assert!(b
            .iter()
            .any(|e| matches!(e, VoiceEvent::On { pitch: 60, .. })));
    }

    #[test]
    fn timed_practice_counts_misses_and_only_selected_parts() {
        let mut e = engine(
            "timed",
            &[
                (60, 100.0, 200.0, 0),
                (64, 500.0, 700.0, 0),
                (48, 100.0, 700.0, 1),
            ],
        );
        e.configure(SongSettings {
            mode: "timed".into(),
            parts: vec![PartSetting {
                part_id: 1,
                role: "accompaniment".into(),
                hand: "left".into(),
                volume: 1.0,
                solo: false,
            }],
            ..SongSettings::default()
        });
        e.advance(251.0);
        assert_eq!(e.snapshot().missed, 1);
        e.advance(249.0);
        press(&mut e, 64);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
    }

    #[test]
    fn looping_rearms_targets_and_resets_voices() {
        let mut e = engine("listen", &[(60, 100.0, 900.0, 0)]);
        e.configure(SongSettings {
            mode: "listen".into(),
            loop_start_ms: Some(100.0),
            loop_end_ms: Some(500.0),
            ..SongSettings::default()
        });
        let events = e.advance(600.0);
        assert_eq!(e.snapshot().position_ms, 200.0);
        assert!(events.contains(&VoiceEvent::Reset));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, VoiceEvent::On { key: 0, .. }))
                .count(),
            2
        );
    }

    #[test]
    fn count_in_keeps_position_and_ignores_player_scoring() {
        let mut e = engine("wait", &[(60, 0.0, 500.0, 0)]);
        e.pause();
        e.configure(SongSettings {
            mode: "wait".into(),
            count_in: true,
            ..SongSettings::default()
        });
        e.play();
        press(&mut e, 60);
        e.advance(1999.0);
        assert_eq!(e.snapshot().position_ms, 0.0);
        assert_eq!(e.snapshot().count_in_remaining, 1);
        e.advance(1.0);
        assert!(e.snapshot().waiting);
        assert_eq!(e.snapshot().correct, 0);
        release(&mut e, 60);
        press(&mut e, 60);
        assert_eq!(e.snapshot().correct, 1);
    }

    #[test]
    fn zero_velocity_is_release_and_invalid_clock_is_ignored() {
        let mut e = engine("wait", &[(60, 0.0, 100.0, 0), (60, 200.0, 300.0, 0)]);
        e.advance(f64::NAN);
        press(&mut e, 60);
        e.advance(200.0);
        e.input(60, true, 0, InputSource::Physical);
        press(&mut e, 60);
        assert_eq!(e.snapshot().correct, 2);
        assert_eq!(e.snapshot().position_ms, 200.0);
    }

    #[test]
    fn solo_and_percussion_never_create_unselected_targets() {
        let mut s = song(&[(60, 0.0, 400.0, 0), (48, 0.0, 400.0, 1)]);
        s.notes[1].channel = 9;
        let mut e = Engine::new();
        e.load(
            s.clone(),
            SongSettings {
                mode: "wait".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(0.0);
        assert_eq!(e.snapshot().targets, vec![60]);
        e.configure(SongSettings {
            mode: "wait".into(),
            parts: vec![PartSetting {
                part_id: 1,
                role: "practice".into(),
                hand: "auto".into(),
                volume: 1.0,
                solo: true,
            }],
            ..SongSettings::default()
        });
        assert!(e.snapshot().targets.is_empty());
        assert!(e
            .advance(10.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { key: 1, .. })));
        e.configure(SongSettings {
            mode: "listen".into(),
            parts: vec![PartSetting {
                part_id: 0,
                role: "muted".into(),
                hand: "auto".into(),
                volume: 1.0,
                solo: true,
            }],
            ..SongSettings::default()
        });
        assert!(!e
            .advance(10.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { .. })));
    }

    #[test]
    fn loop_endpoint_does_not_sound_an_outside_note() {
        let mut e = engine("listen", &[(60, 100.0, 200.0, 0), (72, 500.0, 800.0, 0)]);
        e.configure(SongSettings {
            mode: "listen".into(),
            loop_start_ms: Some(100.0),
            loop_end_ms: Some(500.0),
            ..SongSettings::default()
        });
        let events = e.advance(600.0);
        assert!(!events
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 72, .. })));
    }

    #[test]
    fn exact_gate_accompaniment_is_deferred_and_layered_unisons_deduplicate() {
        let mut e = engine(
            "wait",
            &[
                (60, 100.0, 400.0, 0),
                (60, 100.0, 400.0, 0),
                (48, 100.0, 500.0, 1),
            ],
        );
        e.configure(SongSettings {
            mode: "wait".into(),
            parts: vec![PartSetting {
                part_id: 1,
                role: "accompaniment".into(),
                hand: "left".into(),
                volume: 1.0,
                solo: false,
            }],
            ..SongSettings::default()
        });
        let events = e.advance(200.0);
        assert!(!events.iter().any(|v| matches!(v, VoiceEvent::On { .. })));
        assert_eq!(e.snapshot().targets, vec![60]);
        press(&mut e, 60);
        assert_eq!(e.snapshot().correct, 1);
        assert!(e
            .advance(0.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 48, .. })));
    }

    #[test]
    fn stop_clears_scores_and_releases_sustained_voices() {
        let mut e = engine("listen", &[(60, 0.0, 2000.0, 0)]);
        e.advance(300.0);
        e.stop();
        assert!(e.drain_events().contains(&VoiceEvent::Reset));
        assert!(!e.snapshot().playing);
        assert_eq!(e.snapshot().position_ms, 0.0);
        assert!(e.advance(1000.0).is_empty());
    }

    #[test]
    fn speed_change_preserves_partial_chord_and_sounding_note() {
        let mut e = engine("wait", &[(60, 100.0, 300.0, 0), (64, 100.0, 300.0, 0)]);
        e.advance(100.0);
        press(&mut e, 60);
        e.configure(SongSettings {
            mode: "wait".into(),
            speed: 0.5,
            ..SongSettings::default()
        });
        assert!(e.snapshot().waiting);
        assert_eq!(e.snapshot().matched, vec![60]);
        press(&mut e, 64);
        assert_eq!(e.snapshot().correct, 2);
        let mut e = engine("listen", &[(60, 0.0, 2000.0, 0)]);
        e.advance(100.0);
        e.configure(SongSettings {
            mode: "listen".into(),
            speed: 1.5,
            ..SongSettings::default()
        });
        assert!(e.drain_events().is_empty());
        e.advance(100.0);
        assert_eq!(e.snapshot().position_ms, 250.0);
    }

    #[test]
    fn timed_song_end_scores_partial_final_chord_exactly_once() {
        let mut s = song(&[(60, 100.0, 120.0, 0), (64, 100.0, 120.0, 0)]);
        s.duration_ms = 120.0;
        let mut e = Engine::new();
        e.load(
            s,
            SongSettings {
                mode: "timed".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(100.0);
        press(&mut e, 60);
        e.advance(20.0);
        assert!(!e.is_playing());
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
        assert!(e.snapshot().targets.is_empty());
        e.advance(1000.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
    }

    #[test]
    fn timed_end_does_not_recount_completed_groups() {
        let mut s = song(&[(60, 100.0, 150.0, 0), (64, 500.0, 550.0, 0)]);
        s.duration_ms = 550.0;
        let mut e = Engine::new();
        e.load(
            s,
            SongSettings {
                mode: "timed".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(100.0);
        press(&mut e, 60);
        e.advance(450.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
        assert!(e.snapshot().targets.is_empty());
    }

    #[test]
    fn timed_loop_finalizes_partial_chord_but_excludes_pitch_at_b() {
        let mut e = Engine::new();
        e.load(
            song(&[
                (60, 450.0, 600.0, 0),
                (64, 460.0, 600.0, 0),
                (67, 470.0, 600.0, 0),
            ]),
            SongSettings {
                mode: "timed".into(),
                loop_start_ms: Some(0.0),
                loop_end_ms: Some(470.0),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(460.0);
        assert_eq!(e.snapshot().targets, vec![60, 64]);
        press(&mut e, 60);
        e.advance(10.0);
        assert_eq!(e.snapshot().position_ms, 0.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
        e.advance(470.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 3);
        assert_eq!(e.snapshot().targets, vec![60, 64]);
    }

    #[test]
    fn seeking_to_loop_a_keeps_a_target_with_nearby_prior_onset() {
        let mut e = Engine::new();
        e.load(
            song(&[(60, 90.0, 300.0, 0), (64, 100.0, 300.0, 0)]),
            SongSettings {
                mode: "wait".into(),
                position_ms: 100.0,
                loop_start_ms: Some(100.0),
                loop_end_ms: Some(500.0),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(0.0);
        assert_eq!(e.snapshot().targets, vec![64]);
        press(&mut e, 64);
        assert_eq!(e.snapshot().correct, 1);
    }

    #[test]
    fn approximate_right_split_grades_boundary_and_plays_lower_accompaniment() {
        let mut e = Engine::new();
        e.load(
            song(&[
                (48, 0.0, 500.0, 0),
                (60, 0.0, 500.0, 0),
                (72, 0.0, 500.0, 0),
            ]),
            SongSettings {
                mode: "timed".into(),
                split_pitch: Some(60),
                split_hand: "right".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        let events = e.advance(0.0);
        assert_eq!(e.snapshot().targets, vec![60, 72]);
        let pitches: Vec<_> = events
            .iter()
            .filter_map(|v| match v {
                VoiceEvent::On { pitch, .. } => Some(*pitch),
                _ => None,
            })
            .collect();
        assert_eq!(pitches, vec![48]);
        press(&mut e, 60);
        press(&mut e, 72);
        assert_eq!(e.snapshot().correct, 2);
        assert!(e.snapshot().targets.is_empty());
    }

    #[test]
    fn approximate_left_split_grades_lower_notes_and_plays_boundary_and_above() {
        let mut e = Engine::new();
        e.load(
            song(&[
                (48, 0.0, 500.0, 0),
                (60, 0.0, 500.0, 0),
                (72, 0.0, 500.0, 0),
            ]),
            SongSettings {
                mode: "timed".into(),
                split_pitch: Some(60),
                split_hand: "left".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        let events = e.advance(0.0);
        assert_eq!(e.snapshot().targets, vec![48]);
        let pitches: Vec<_> = events
            .iter()
            .filter_map(|v| match v {
                VoiceEvent::On { pitch, .. } => Some(*pitch),
                _ => None,
            })
            .collect();
        assert_eq!(pitches, vec![60, 72]);
        press(&mut e, 48);
        assert_eq!(e.snapshot().correct, 1);
        assert!(e.snapshot().targets.is_empty());
    }

    #[test]
    fn split_hand_and_boundary_changes_rebuild_targets_and_audio() {
        let mut e = Engine::new();
        let mut settings = SongSettings {
            mode: "timed".into(),
            split_pitch: Some(60),
            split_hand: "right".into(),
            ..SongSettings::default()
        };
        e.load(
            song(&[
                (48, 0.0, 500.0, 0),
                (60, 0.0, 500.0, 0),
                (72, 0.0, 500.0, 0),
            ]),
            settings.clone(),
        );
        e.play();
        e.advance(0.0);
        settings.split_hand = "left".into();
        e.configure(settings.clone());
        assert_eq!(e.snapshot().targets, vec![48]);
        assert!(e
            .advance(0.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 60, .. })));
        settings.split_pitch = Some(61);
        e.configure(settings);
        assert_eq!(e.snapshot().targets, vec![48, 60]);
        let events = e.advance(0.0);
        assert!(events
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 72, .. })));
        assert!(!events
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 60, .. })));
    }

    #[test]
    fn both_or_disabled_split_preserves_complete_practice_parts() {
        for (split_pitch, split_hand) in [(Some(60), "both"), (None, "left"), (None, "right")] {
            let mut e = Engine::new();
            e.load(
                song(&[(48, 0.0, 500.0, 0), (72, 0.0, 500.0, 0)]),
                SongSettings {
                    mode: "timed".into(),
                    split_pitch,
                    split_hand: split_hand.into(),
                    ..SongSettings::default()
                },
            );
            e.play();
            assert_eq!(e.snapshot().targets, vec![48, 72]);
            assert!(!e
                .advance(0.0)
                .iter()
                .any(|v| matches!(v, VoiceEvent::On { .. })));
        }
    }

    #[test]
    fn split_never_grades_percussion_and_listen_still_plays_every_audible_note() {
        let mut s = song(&[
            (48, 0.0, 500.0, 0),
            (60, 0.0, 500.0, 0),
            (72, 0.0, 500.0, 0),
        ]);
        s.notes[1].channel = 9;
        let mut e = Engine::new();
        let mut settings = SongSettings {
            mode: "timed".into(),
            split_pitch: Some(60),
            split_hand: "right".into(),
            ..SongSettings::default()
        };
        e.load(s, settings.clone());
        e.play();
        assert_eq!(e.snapshot().targets, vec![72]);
        let events = e.advance(0.0);
        assert!(events
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 60, .. })));
        settings.mode = "listen".into();
        e.configure(settings);
        let events = e.advance(0.0);
        assert!(e.snapshot().targets.is_empty());
        assert_eq!(
            events
                .iter()
                .filter(|v| matches!(v, VoiceEvent::On { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn split_respects_muted_and_solo_parts_regardless_of_hand_labels() {
        let mut e = Engine::new();
        let mut settings = SongSettings {
            mode: "timed".into(),
            split_pitch: Some(60),
            split_hand: "right".into(),
            parts: vec![
                PartSetting {
                    part_id: 0,
                    role: "practice".into(),
                    hand: "right".into(),
                    volume: 1.0,
                    solo: false,
                },
                PartSetting {
                    part_id: 1,
                    role: "practice".into(),
                    hand: "left".into(),
                    volume: 0.4,
                    solo: true,
                },
            ],
            ..SongSettings::default()
        };
        e.load(
            song(&[
                (48, 0.0, 500.0, 0),
                (72, 0.0, 500.0, 0),
                (84, 0.0, 500.0, 1),
            ]),
            settings.clone(),
        );
        e.play();
        assert_eq!(e.snapshot().targets, vec![84]);
        assert!(!e
            .advance(0.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { .. })));
        settings.parts[1].role = "muted".into();
        e.configure(settings);
        assert!(e.snapshot().targets.is_empty());
        assert!(!e
            .advance(0.0)
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { .. })));
    }

    #[test]
    fn wait_split_freezes_and_resumes_the_opposite_hand_accompaniment() {
        let mut e = Engine::new();
        e.load(
            song(&[(48, 0.0, 500.0, 0), (72, 100.0, 500.0, 0)]),
            SongSettings {
                mode: "wait".into(),
                split_pitch: Some(60),
                split_hand: "right".into(),
                ..SongSettings::default()
            },
        );
        e.play();
        let events = e.advance(100.0);
        assert!(e.snapshot().waiting);
        assert_eq!(e.snapshot().targets, vec![72]);
        assert!(events
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 48, .. })));
        assert!(events.contains(&VoiceEvent::Off { key: 0 }));
        press(&mut e, 72);
        assert!(!e.snapshot().waiting);
        assert_eq!(e.snapshot().correct, 1);
        assert!(e
            .drain_events()
            .iter()
            .any(|v| matches!(v, VoiceEvent::On { pitch: 48, .. })));
    }

    #[test]
    fn timed_partial_chord_keeps_on_time_pitch_credit_at_normal_deadline() {
        let mut e = engine("timed", &[(60, 100.0, 500.0, 0), (64, 100.0, 500.0, 0)]);
        e.advance(100.0);
        press(&mut e, 60);
        e.advance(151.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
        assert!(e.snapshot().targets.is_empty());
        e.advance(1000.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
    }

    #[test]
    fn timed_rearticulation_after_chord_expiry_does_not_duplicate_pitch_credit() {
        let mut e = engine("timed", &[(60, 100.0, 500.0, 0), (64, 100.0, 500.0, 0)]);
        // Early C is inside its onset window. Its chord-entry match expires,
        // but its pitch credit must not be lost or counted again on replay.
        press(&mut e, 60);
        e.advance(160.0);
        assert!(e.snapshot().matched.is_empty());
        release(&mut e, 60);
        press(&mut e, 60);
        e.advance(91.0);
        assert_eq!(e.snapshot().correct, 1);
        assert_eq!(e.snapshot().missed, 1);
    }

    #[test]
    fn simulated_and_playback_cannot_score_or_rearm_physical_repetitions() {
        let mut e = engine("wait", &[(60, 0.0, 100.0, 0), (60, 200.0, 300.0, 0)]);
        e.advance(0.0);
        let guide = e.guide_targets();
        for source in [InputSource::Simulated, InputSource::Playback] {
            e.input(60, true, 100, source);
            e.input(70, true, 100, source);
            e.input(60, false, 0, source);
        }
        assert_eq!(e.snapshot().correct, 0);
        assert_eq!(e.snapshot().wrong, 0);
        assert!(e.snapshot().matched.is_empty());
        assert_eq!(e.guide_targets(), guide);
        press(&mut e, 60);
        assert_eq!(e.snapshot().correct, 1);
        e.advance(200.0);
        for source in [InputSource::Simulated, InputSource::Playback] {
            e.input(60, false, 0, source);
            e.input(60, true, 0, source);
        }
        press(&mut e, 60);
        assert!(
            e.snapshot().waiting,
            "nonphysical offs must not rearm the held physical key"
        );
        assert_eq!(e.snapshot().correct, 1);
        release(&mut e, 60);
        press(&mut e, 60);
        assert_eq!(e.snapshot().correct, 2);
    }

    #[test]
    fn simulated_and_playback_do_not_earn_timed_pitch_credit() {
        let mut e = engine("timed", &[(60, 100.0, 500.0, 0), (64, 100.0, 500.0, 0)]);
        e.advance(100.0);
        e.input(60, true, 100, InputSource::Simulated);
        e.input(64, true, 100, InputSource::Playback);
        e.advance(151.0);
        assert_eq!(e.snapshot().correct, 0);
        assert_eq!(e.snapshot().wrong, 0);
        assert_eq!(e.snapshot().missed, 2);
    }

    #[test]
    fn wait_guide_identity_changes_for_repeated_pitch_groups() {
        let mut e = engine("wait", &[(60, 0.0, 100.0, 0), (60, 200.0, 300.0, 0)]);
        assert!(e.guide_targets().is_none());
        e.advance(0.0);
        let first = e.guide_targets().unwrap();
        assert_eq!(first.1, vec![60]);
        assert_eq!(e.guide_targets(), Some(first.clone()));
        press(&mut e, 60);
        assert!(e.guide_targets().is_none());
        e.advance(200.0);
        let next = e.guide_targets().unwrap();
        assert_eq!(next.1, first.1);
        assert_ne!(next.0, first.0);
    }

    #[test]
    fn guide_lifecycle_clears_on_pause_stop_seek_and_revises_on_rebuild_load() {
        let notes = [(60, 100.0, 300.0, 0)];
        let mut e = engine("wait", &notes);
        e.advance(100.0);
        let original = e.guide_targets().unwrap().0;
        e.pause();
        assert!(e.guide_targets().is_none());
        e.play();
        assert_eq!(e.guide_targets().unwrap().0, original);
        e.seek(100.0);
        assert!(e.guide_targets().is_none());
        e.advance(0.0);
        let after_seek = e.guide_targets().unwrap().0;
        assert_ne!(after_seek, original);
        e.configure(SongSettings {
            mode: "wait".into(),
            parts: vec![PartSetting {
                part_id: 0,
                role: "practice".into(),
                hand: "right".into(),
                volume: 0.5,
                solo: false,
            }],
            ..SongSettings::default()
        });
        assert!(e.guide_targets().is_none());
        e.advance(0.0);
        let after_rebuild = e.guide_targets().unwrap().0;
        assert_ne!(after_rebuild, after_seek);
        e.stop();
        assert!(e.guide_targets().is_none());
        e.play();
        e.advance(100.0);
        let after_stop = e.guide_targets().unwrap().0;
        assert_ne!(after_stop, after_rebuild);
        e.load(
            song(&notes),
            SongSettings {
                mode: "wait".into(),
                ..SongSettings::default()
            },
        );
        assert!(e.guide_targets().is_none());
        e.play();
        e.advance(100.0);
        assert_ne!(e.guide_targets().unwrap().0, after_stop);
    }

    #[test]
    fn guide_loop_rearms_same_chord_with_a_new_identity() {
        let mut e = Engine::new();
        e.load(
            song(&[(60, 100.0, 200.0, 0)]),
            SongSettings {
                mode: "wait".into(),
                position_ms: 100.0,
                loop_start_ms: Some(100.0),
                loop_end_ms: Some(300.0),
                ..SongSettings::default()
            },
        );
        e.play();
        e.advance(0.0);
        let first = e.guide_targets().unwrap();
        press(&mut e, 60);
        e.advance(200.0);
        assert_eq!(e.snapshot().position_ms, 100.0);
        e.advance(0.0);
        let repeated = e.guide_targets().unwrap();
        assert_eq!(repeated.1, first.1);
        assert_ne!(repeated.0, first.0);
        press(&mut e, 60);
        assert_eq!(e.guide_targets(), Some(repeated));
    }

    #[test]
    fn timed_guide_window_uses_wall_time_at_current_speed() {
        let mut e = Engine::new();
        e.load(
            song(&[(60, 500.0, 900.0, 0)]),
            SongSettings {
                mode: "timed".into(),
                speed: 0.5,
                ..SongSettings::default()
            },
        );
        e.play();
        assert!(e.guide_targets().is_none());
        e.advance(849.0);
        assert!(e.guide_targets().is_none());
        e.advance(1.0);
        let first = e.guide_targets().unwrap();
        assert_eq!(e.snapshot().position_ms, 425.0);
        e.advance(300.0);
        assert_eq!(e.snapshot().position_ms, 575.0);
        assert_eq!(e.guide_targets(), Some(first));
        e.advance(0.1);
        assert!(e.guide_targets().is_none());
    }

    #[test]
    fn timed_guide_revises_identity_after_a_missed_repeated_pitch() {
        let mut e = engine("timed", &[(60, 100.0, 200.0, 0), (60, 300.0, 400.0, 0)]);
        e.advance(100.0);
        let first = e.guide_targets().unwrap();
        e.advance(151.0);
        let next = e.guide_targets().unwrap();
        assert_eq!(e.snapshot().missed, 1);
        assert_eq!(first.1, next.1);
        assert_ne!(first.0, next.0);
    }

    #[test]
    fn guide_stays_off_during_count_in_and_listen() {
        let mut e = Engine::new();
        e.load(
            song(&[(60, 0.0, 500.0, 0)]),
            SongSettings {
                mode: "wait".into(),
                count_in: true,
                ..SongSettings::default()
            },
        );
        e.play();
        assert!(e.guide_targets().is_none());
        e.advance(1999.0);
        assert!(e.guide_targets().is_none());
        e.advance(1.0);
        assert_eq!(e.guide_targets().unwrap().1, vec![60]);
        e.configure(SongSettings {
            mode: "listen".into(),
            ..SongSettings::default()
        });
        e.advance(0.0);
        assert!(e.is_playing());
        assert!(e.guide_targets().is_none());
    }

    #[test]
    fn guide_uses_filtered_practice_chord_and_is_read_only() {
        let mut s = song(&[
            (48, 0.0, 500.0, 0),
            (60, 0.0, 500.0, 0),
            (72, 0.0, 500.0, 0),
            (84, 0.0, 500.0, 1),
            (90, 0.0, 500.0, 0),
        ]);
        s.notes[4].channel = 9;
        let mut settings = SongSettings {
            mode: "timed".into(),
            split_pitch: Some(60),
            split_hand: "right".into(),
            parts: vec![PartSetting {
                part_id: 0,
                role: "practice".into(),
                hand: "left".into(),
                volume: 1.0,
                solo: true,
            }],
            ..SongSettings::default()
        };
        let mut e = Engine::new();
        e.load(s, settings.clone());
        e.play();
        let mut copy = e.guide_targets().unwrap();
        assert_eq!(copy.1, vec![60, 72]);
        copy.1.clear();
        assert_eq!(e.guide_targets().unwrap().1, vec![60, 72]);
        assert_eq!(e.snapshot().correct, 0);
        press(&mut e, 60);
        assert_eq!(e.guide_targets().unwrap().1, vec![60, 72]);
        assert_eq!(e.guide_targets().unwrap().0, copy.0);
        settings.parts[0].role = "muted".into();
        e.configure(settings.clone());
        assert!(e.guide_targets().is_none());
        settings.parts[0].role = "accompaniment".into();
        e.configure(settings.clone());
        assert!(e.guide_targets().is_none());
        settings.parts[0].role = "practice".into();
        settings.split_hand = "left".into();
        e.configure(settings);
        assert_eq!(e.guide_targets().unwrap().1, vec![48]);
    }
}
