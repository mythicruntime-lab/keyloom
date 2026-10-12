use crate::{
    audio::{Audio, Sound},
    light_guide::GuideNotes,
    midi,
    model::{Song, SongSettings},
    transport::{Engine, InputSource, TransportSnapshot, VoiceEvent},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub light_guide: bool,
    pub monitor: String,
    pub simulated: bool,
    pub low: u8,
    pub high: u8,
    pub labels: bool,
    pub note_numbers: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            device_id: None,
            output_device_id: None,
            light_guide: false,
            monitor: "yamaha".into(),
            simulated: false,
            low: 36,
            high: 96,
            labels: true,
            note_numbers: false,
        }
    }
}
#[derive(Clone, Serialize)]
pub struct InputEvent {
    pub sequence: u64,
    pub source: String,
    pub kind: String,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
}
#[derive(Clone, Serialize)]
pub struct AppStatus {
    pub transport: TransportSnapshot,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub device_connected: bool,
    pub device_error: Option<String>,
    pub output_device_id: Option<String>,
    pub output_device_name: Option<String>,
    pub output_connected: bool,
    pub output_error: Option<String>,
    pub light_guide: bool,
    pub light_test_active: bool,
    pub lit_notes: Vec<u8>,
    pub output_messages: u64,
    pub audio_available: bool,
    pub audio_error: Option<String>,
    pub audio_frames: u64,
    pub audio_nonzero_frames: u64,
    pub events: Vec<InputEvent>,
    pub held: Vec<u8>,
    pub dropped_events: u64,
}
pub enum Command {
    Load {
        id: String,
        song: Song,
        settings: SongSettings,
    },
    Play,
    Pause,
    Stop,
    Seek(f64),
    Configure(SongSettings),
    Panic,
    Preferences(Preferences, SyncSender<()>),
    Connect(Option<String>, SyncSender<Result<(), String>>),
    ConnectOutput(Option<String>, SyncSender<Result<(), String>>),
    TestLight(SyncSender<Result<(), String>>),
    Input {
        pitch: u8,
        on: bool,
        velocity: u8,
    },
    Unload(SyncSender<()>),
    Checkpoint(SyncSender<Option<(String, SongSettings)>>),
    Shutdown(SyncSender<Result<(), String>>),
}
#[derive(Clone)]
pub struct Runtime {
    pub tx: SyncSender<Command>,
    pub status: Arc<Mutex<AppStatus>>,
}
impl Runtime {
    pub fn start(preferences: Preferences) -> Self {
        let engine = Engine::new();
        let initial = AppStatus {
            transport: engine.snapshot(),
            device_id: preferences.device_id.clone(),
            device_name: None,
            device_connected: false,
            device_error: None,
            output_device_id: preferences.output_device_id.clone(),
            output_device_name: None,
            output_connected: false,
            output_error: None,
            light_guide: preferences.light_guide,
            light_test_active: false,
            lit_notes: Vec::new(),
            output_messages: 0,
            audio_available: false,
            audio_error: Some("Opening Windows audio…".into()),
            audio_frames: 0,
            audio_nonzero_frames: 0,
            events: Vec::new(),
            held: Vec::new(),
            dropped_events: 0,
        };
        let (tx, rx) = mpsc::sync_channel(128);
        let status = Arc::new(Mutex::new(initial));
        let handle = Self {
            tx,
            status: status.clone(),
        };
        std::thread::Builder::new().name("keyloom-transport".into()).spawn(move||{
            let (audio,audio_error)=Audio::open();let (midi_tx,midi_rx)=mpsc::sync_channel(512);let dropped=Arc::new(AtomicU64::new(0));
            let mut worker=Worker{engine,audio,preferences,connection:None,device_name:None,device_error:None,audio_error,held:[[false;128];32],pedals:[false;32],events:Vec::with_capacity(64),sequence:0,current:None,settings:SongSettings::default(),output:None,guide:GuideNotes::new(),output_name:None,output_error:None,test_until:None,output_messages:0,unreleased_outputs:Vec::new()};
            if let Some(id)=worker.preferences.device_id.clone(){if id.contains("ambiguous]"){worker.device_error=Some("Ambiguous saved port; select the input explicitly".into());}else{worker.connect(Some(id),&midi_rx,midi_tx.clone(),dropped.clone()).ok();}}
            if let Some(id)=worker.preferences.output_device_id.clone(){if id.contains("ambiguous]"){worker.output_error=Some("Ambiguous saved output; select it explicitly".into());}else{worker.connect_output(Some(id)).ok();}}
            let mut previous=Instant::now();let mut published=Instant::now();let mut checked=Instant::now();let mut old_dropped=0;
            loop {
                let wait=if worker.engine.is_playing() {Duration::from_millis(3)}else{Duration::from_millis(12)};
                let command=match rx.recv_timeout(wait){Ok(command)=>Some(command),Err(mpsc::RecvTimeoutError::Timeout)=>None,Err(_)=>break};
                let now=Instant::now();let elapsed=now.duration_since(previous).as_secs_f64()*1000.0;previous=now;
                let events=worker.engine.advance(elapsed);worker.send_events(events);
                if let Some(command)=command {match command {
                    Command::Shutdown(reply)=>{worker.reset();let result=worker.cleanup_result();worker.output=None;worker.connection=None;reply.send(result).ok();break;},
                    Command::Unload(reply)=>{worker.reset();worker.current=None;worker.settings=SongSettings::default();worker.engine=Engine::new();reply.send(()).ok();},
                    Command::Checkpoint(reply)=>{reply.send(worker.checkpoint()).ok();},
                    Command::Load{id,song,settings}=>{worker.reset();worker.current=Some(id);worker.settings=settings.clone();worker.engine.load(song,settings);},
                    Command::Play=>{worker.clear_guide();worker.engine.play();},Command::Pause=>{worker.engine.pause();worker.reset();},Command::Stop=>{worker.engine.stop();worker.reset();},
                    Command::Seek(ms)=>{worker.engine.seek(ms);worker.reset();},Command::Configure(settings)=>{worker.clear_guide();worker.settings=settings.clone();worker.engine.configure(settings);},
                    Command::Panic=>{worker.engine.pause();worker.reset();},
                    // Only Connect owns device selection; a stale preferences
                    // payload cannot relabel or disconnect an active MIDI port.
                    Command::Preferences(mut p,reply)=>{p.device_id=worker.preferences.device_id.clone();p.output_device_id=worker.preferences.output_device_id.clone();if worker.preferences.monitor!=p.monitor||worker.preferences.simulated!=p.simulated {worker.engine.pause();worker.reset();}
                    if worker.preferences.light_guide!=p.light_guide || worker.preferences.low!=p.low || worker.preferences.high!=p.high {worker.clear_guide();}worker.preferences=p;reply.send(()).ok();},
                    Command::Connect(id,reply)=>{let result=worker.connect(id,&midi_rx,midi_tx.clone(),dropped.clone());reply.send(result).ok();},
                    Command::ConnectOutput(id,reply)=>{let result=worker.connect_output(id);reply.send(result).ok();},
                    Command::TestLight(reply)=>{let result=worker.test_light();reply.send(result).ok();},
                    Command::Input{pitch,on,velocity}=>{if worker.preferences.simulated&&pitch<128 {worker.input(midi::Message{status:if on{0x90}else{0x80},a:pitch,b:velocity.min(127)},true);}},
                }}
                for _ in 0..512 {match midi_rx.try_recv(){Ok(message)=>worker.input(message,false),Err(_)=>break}}
                let loss=dropped.load(Ordering::Relaxed)+worker.audio.sender.dropped.load(Ordering::Relaxed);
                if loss!=old_dropped {worker.engine.pause();worker.reset();worker.device_error=Some("Input/audio queue overflow; practice paused. Press Play after checking the connection.".into());old_dropped=loss;}
                if checked.elapsed()>=Duration::from_secs(2) {checked=Instant::now();if worker.connection.is_some(){if let Ok(available)=midi::devices(){if !available.iter().any(|d|Some(&d.id)==worker.preferences.device_id.as_ref()){worker.connection=None;worker.engine.pause();worker.reset();worker.device_error=Some("MIDI input disconnected. Refresh and reconnect the same device.".into());}}}
                if worker.output.is_some(){if let Ok(available)=midi::output_devices(){if !available.iter().any(|d|Some(&d.id)==worker.preferences.output_device_id.as_ref()){worker.engine.pause();worker.reset();worker.output=None;worker.output_error=Some("MIDI output disconnected. Reconnect to clear the keyboard's guide notes.".into());}}}}
                let pending=worker.engine.drain_events();worker.send_events(pending);
                worker.sync_guide();
                if published.elapsed()>=Duration::from_millis(30) {published=Instant::now();let snapshot=worker.engine.snapshot();worker.settings.position_ms=snapshot.position_ms;
                    let audio_error=if worker.audio.error.load(Ordering::Acquire){Some("Windows audio device changed or failed. Restart Keyloom to reopen audio.".into())}else{worker.audio_error.clone()};
                    if let Ok(mut shared)=status.lock(){*shared=AppStatus{transport:snapshot,device_id:worker.preferences.device_id.clone(),device_name:worker.device_name.clone(),device_connected:worker.connection.is_some(),device_error:worker.device_error.clone(),output_device_id:worker.preferences.output_device_id.clone(),output_device_name:worker.output_name.clone(),output_connected:worker.output.is_some(),output_error:worker.output_status_error(),light_guide:worker.preferences.light_guide,light_test_active:worker.test_until.is_some(),lit_notes:worker.guide.lit_notes(),output_messages:worker.output_messages,audio_available:audio_error.is_none(),audio_error,audio_frames:worker.audio.frames.load(Ordering::Relaxed),audio_nonzero_frames:worker.audio.nonzero_frames.load(Ordering::Relaxed),events:worker.events.clone(),held:(0..128).filter(|p|worker.held.iter().any(|channel|channel[*p])).map(|p|p as u8).collect(),dropped_events:loss};}
                }
            }
            worker.reset();drop(worker.connection);
        }).expect("create Keyloom runtime worker");
        handle
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.tx
            .try_send(command)
            .map_err(|_| "Keyloom transport is busy or closed; retry the action".into())
    }
    /// Off-UI callers use this ordered barrier before saving or switching songs.
    /// It includes prior queued commands rather than the 30 ms presentation cache.
    pub fn checkpoint(&self) -> Result<Option<(String, SongSettings)>, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Checkpoint(tx))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "Keyloom transport checkpoint timed out".into())
    }
    /// Keep the process alive until the worker has released hardware guide notes.
    pub fn shutdown(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Shutdown(tx))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "MIDI cleanup timed out".to_string())?
    }
    pub fn set_preferences(&self, value: Preferences) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Preferences(value, tx))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "Preferences update timed out".into())
    }
}
struct Worker {
    engine: Engine,
    audio: Audio,
    preferences: Preferences,
    connection: Option<midir::MidiInputConnection<()>>,
    device_name: Option<String>,
    device_error: Option<String>,
    audio_error: Option<String>,
    held: [[bool; 128]; 32],
    pedals: [bool; 32],
    events: Vec<InputEvent>,
    sequence: u64,
    current: Option<String>,
    settings: SongSettings,
    output: Option<midir::MidiOutputConnection>,
    guide: GuideNotes,
    output_name: Option<String>,
    output_error: Option<String>,
    test_until: Option<Instant>,
    output_messages: u64,
    unreleased_outputs: Vec<String>,
}
impl Worker {
    fn cleanup_result(&self) -> Result<(), String> {
        if self.unreleased_outputs.is_empty() && self.guide.lit_notes().is_empty() {
            Ok(())
        } else {
            Err(format!("Guide notes could not be released on {}. Reconnect that output in Keyloom to clear them, or switch the keyboard off.", self.unreleased_outputs.join(", ")))
        }
    }

    fn output_status_error(&self) -> Option<String> {
        match (self.output_error.as_ref(), self.cleanup_result().err()) {
            (Some(error), Some(cleanup)) if !self.unreleased_outputs.is_empty() => {
                Some(format!("{error} {cleanup}"))
            }
            (Some(error), _) => Some(error.clone()),
            (None, Some(cleanup)) if !self.unreleased_outputs.is_empty() => Some(cleanup),
            _ => None,
        }
    }
    fn clear_guide(&mut self) {
        self.test_until = None;
        if let Some(output) = self.output.as_mut() {
            let sent = &mut self.output_messages;
            let result = self.guide.cleanup(|bytes| {
                output.send(&bytes).map_err(|e| e.to_string())?;
                *sent += 1;
                Ok(())
            });
            if let Err(error) = result {
                if let Some(id) = &self.preferences.output_device_id {
                    if !self.unreleased_outputs.contains(id) {
                        self.unreleased_outputs.push(id.clone());
                    }
                }
                self.output_error = Some(format!(
                    "Could not release guide notes: {error}. Reconnect the output to clear them."
                ));
                self.output = None;
            }
        }
    }

    fn connect_output(&mut self, id: Option<String>) -> Result<(), String> {
        self.engine.pause();
        self.reset();
        self.output = None;
        self.output_name = None;
        self.preferences.output_device_id = id.clone();
        let Some(id) = id else { return Ok(()) };
        let result = (|| {
            let mut output = midi::connect_output(&id)?;
            // After a lost cable/process, tracked note-offs could not reach the
            // keyboard. Establish a clean channel-1 baseline on explicit open.
            // Never send imported controllers, SysEx, programs or MIDI Thru.
            let mut error = None;
            for pitch in 0..128 {
                match output.send(&[0x80, pitch, 0]) {
                    Ok(()) => self.output_messages += 1,
                    Err(e) => {
                        error.get_or_insert_with(|| e.to_string());
                    }
                }
            }
            if let Some(error) = error {
                return Err(error);
            }
            self.guide = GuideNotes::new();
            self.unreleased_outputs.retain(|old| old != &id);
            self.output = Some(output);
            self.output_name = Some(id);
            Ok(())
        })();
        self.output_error = result.as_ref().err().cloned();
        result
    }

    fn update_guide(&mut self, target: Option<(u64, Vec<u8>)>) -> Result<(), String> {
        let output = self
            .output
            .as_mut()
            .ok_or("Connect a MIDI output for the Light Guide")?;
        let sent = &mut self.output_messages;
        let result = self.guide.update(
            target
                .as_ref()
                .map(|(id, pitches)| (*id, pitches.as_slice())),
            |bytes| {
                output.send(&bytes).map_err(|e| e.to_string())?;
                *sent += 1;
                Ok(())
            },
        );
        if let Err(error) = &result {
            self.engine.pause();
            self.audio.sender.send(Sound::Reset);
            self.clear_guide();
            self.output = None;
            self.output_error = Some(format!(
                "Light Guide output failed: {error}. Practice paused; reconnect the output."
            ));
        }
        result
    }

    fn test_light(&mut self) -> Result<(), String> {
        self.engine.pause();
        self.reset();
        self.update_guide(Some((u64::MAX, vec![60])))?;
        self.test_until = Some(Instant::now() + Duration::from_secs(2));
        Ok(())
    }

    fn sync_guide(&mut self) {
        if self.output.is_none() {
            return;
        }
        if self.test_until.is_some_and(|until| Instant::now() >= until) {
            self.clear_guide();
        }
        let target = if self.test_until.is_some() {
            Some((u64::MAX, vec![60]))
        } else if self.preferences.light_guide && self.connection.is_some() {
            self.engine.guide_targets().map(|(id, pitches)| {
                (
                    id,
                    pitches
                        .into_iter()
                        .filter(|p| *p >= self.preferences.low && *p <= self.preferences.high)
                        .collect(),
                )
            })
        } else {
            None
        };
        self.update_guide(target).ok();
    }

    fn checkpoint(&mut self) -> Option<(String, SongSettings)> {
        let snapshot = self.engine.snapshot();
        self.settings.position_ms = snapshot.position_ms;
        self.settings.speed = snapshot.speed;
        self.settings.mode = snapshot.mode;
        self.current.clone().map(|id| (id, self.settings.clone()))
    }
    fn reset(&mut self) {
        self.clear_guide();
        self.audio.sender.send(Sound::Reset);
        self.held = [[false; 128]; 32];
        self.pedals = [false; 32];
        for pitch in 0..128 {
            self.engine.input(pitch, false, 0, InputSource::Physical);
            self.engine.input(pitch, false, 0, InputSource::Simulated);
        }
    }
    fn connect(
        &mut self,
        id: Option<String>,
        rx: &mpsc::Receiver<midi::Message>,
        tx: SyncSender<midi::Message>,
        dropped: Arc<AtomicU64>,
    ) -> Result<(), String> {
        self.connection = None;
        // Dropping the old connection first joins/stops its callback. Only then
        // discard queued old-port messages, before any new callback can send.
        // Otherwise a released key from the old device can become a fresh key
        // on the replacement device or silently satisfy the next target.
        while rx.try_recv().is_ok() {}
        self.engine.pause();
        self.reset();
        self.preferences.device_id = id.clone();
        self.device_name = None;
        let result = if let Some(id) = id {
            midi::connect(&id, tx, dropped).map(|connection| {
                self.connection = Some(connection);
                self.device_name = Some(id);
            })
        } else {
            Ok(())
        };
        self.device_error = result.as_ref().err().cloned();
        result
    }
    fn send_events(&self, events: Vec<VoiceEvent>) {
        for event in events {
            self.audio.sender.send(match event {
                VoiceEvent::On {
                    key,
                    pitch,
                    velocity,
                    volume,
                } => Sound::On {
                    key,
                    pitch,
                    velocity,
                    volume,
                },
                VoiceEvent::Off { key } => Sound::Off(key),
                VoiceEvent::Reset => Sound::ResetScheduled,
                VoiceEvent::Click => Sound::Click,
            });
        }
    }
    fn input(&mut self, message: midi::Message, simulated: bool) {
        let channel = (message.status & 15) as usize + if simulated { 16 } else { 0 };
        let pitch = message.a;
        let code = message.status & 0xf0;
        let on = code == 0x90 && message.b > 0;
        let source = if simulated {
            InputSource::Simulated
        } else {
            InputSource::Physical
        };
        let key = 1_000_000 + channel as u32 * 128 + pitch as u32;
        if matches!(code, 0x80 | 0x90) {
            self.held[channel][pitch as usize] = on;
            if on || !source_holds(&self.held, simulated, pitch) {
                self.engine.input(pitch, on, message.b, source);
            }
            if self.preferences.monitor == "software" {
                if on {
                    self.audio.sender.send(Sound::On {
                        key,
                        pitch,
                        velocity: message.b,
                        volume: 0.65,
                    });
                } else if !self.pedals[channel] {
                    self.audio.sender.send(Sound::Off(key));
                }
            }
        } else if code == 0xb0 {
            if pitch == 64 {
                self.pedals[channel] = message.b >= 64;
                if !self.pedals[channel] {
                    for p in 0..128 {
                        if !self.held[channel][p] {
                            self.audio
                                .sender
                                .send(Sound::Off(1_000_000 + channel as u32 * 128 + p as u32));
                        }
                    }
                }
            }
            if matches!(pitch, 120 | 123) {
                for p in 0..128 {
                    self.held[channel][p] = false;
                    if !source_holds(&self.held, simulated, p as u8) {
                        self.engine.input(p as u8, false, 0, source);
                    }
                    self.audio
                        .sender
                        .send(Sound::Off(1_000_000 + channel as u32 * 128 + p as u32));
                }
                self.pedals[channel] = false;
            }
        }
        self.sequence += 1;
        if self.events.len() == 64 {
            self.events.remove(0);
        }
        self.events.push(InputEvent {
            sequence: self.sequence,
            source: if simulated { "simulated" } else { "physical" }.into(),
            kind: if code == 0xb0 {
                "control_change"
            } else if on {
                "note_on"
            } else {
                "note_off"
            }
            .into(),
            channel: message.status & 15,
            pitch,
            velocity: message.b,
        });
    }
}

fn source_holds(held: &[[bool; 128]; 32], simulated: bool, pitch: u8) -> bool {
    let start = if simulated { 16 } else { 0 };
    held[start..start + 16]
        .iter()
        .any(|channel| channel[usize::from(pitch)])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_preferences_default_to_light_guide_off() {
        let prefs: Preferences =
            serde_json::from_str(r#"{"device_id":"USB MIDI Interface","simulated":true}"#).unwrap();
        assert!(!prefs.light_guide);
        assert!(prefs.output_device_id.is_none());
        assert_eq!(prefs.device_id.as_deref(), Some("USB MIDI Interface"));
    }
    #[test]
    fn a_simulated_hold_cannot_suppress_physical_release() {
        let mut held = [[false; 128]; 32];
        held[0][60] = true;
        held[16][60] = true;
        held[0][60] = false;
        assert!(!source_holds(&held, false, 60));
        assert!(source_holds(&held, true, 60));
        held[16][60] = false;
        assert!(!source_holds(&held, true, 60));
    }
    #[test]
    fn channel_reset_preserves_another_channel_of_the_same_source() {
        let mut held = [[false; 128]; 32];
        held[0][60] = true;
        held[1][60] = true;
        held[0].fill(false);
        assert!(source_holds(&held, false, 60));
        held[1].fill(false);
        assert!(!source_holds(&held, false, 60));
    }
}
