use crate::{
    audio::{Audio, Sound},
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
    Preferences(Preferences),
    Connect(Option<String>, SyncSender<Result<(), String>>),
    Input {
        pitch: u8,
        on: bool,
        velocity: u8,
    },
    Unload(SyncSender<()>),
    Checkpoint(SyncSender<Option<(String, SongSettings)>>),
    Shutdown,
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
            let mut worker=Worker{engine,audio,preferences,connection:None,device_name:None,device_error:None,audio_error,held:[[false;128];32],pedals:[false;32],events:Vec::with_capacity(64),sequence:0,current:None,settings:SongSettings::default()};
            if let Some(id)=worker.preferences.device_id.clone(){if id.contains("ambiguous]"){worker.device_error=Some("Ambiguous saved port; select the input explicitly".into());}else{worker.connect(Some(id),&midi_rx,midi_tx.clone(),dropped.clone()).ok();}}
            let mut previous=Instant::now();let mut published=Instant::now();let mut checked=Instant::now();let mut old_dropped=0;
            loop {
                let wait=if worker.engine.is_playing() {Duration::from_millis(3)}else{Duration::from_millis(12)};
                let command=match rx.recv_timeout(wait){Ok(command)=>Some(command),Err(mpsc::RecvTimeoutError::Timeout)=>None,Err(_)=>break};
                let now=Instant::now();let elapsed=now.duration_since(previous).as_secs_f64()*1000.0;previous=now;
                let events=worker.engine.advance(elapsed);worker.send_events(events);
                if let Some(command)=command {match command {
                    Command::Shutdown=>break,
                    Command::Unload(reply)=>{worker.reset();worker.current=None;worker.settings=SongSettings::default();worker.engine=Engine::new();reply.send(()).ok();},
                    Command::Checkpoint(reply)=>{reply.send(worker.checkpoint()).ok();},
                    Command::Load{id,song,settings}=>{worker.reset();worker.current=Some(id);worker.settings=settings.clone();worker.engine.load(song,settings);},
                    Command::Play=>worker.engine.play(),Command::Pause=>{worker.engine.pause();worker.reset();},Command::Stop=>{worker.engine.stop();worker.reset();},
                    Command::Seek(ms)=>{worker.engine.seek(ms);worker.reset();},Command::Configure(settings)=>{worker.settings=settings.clone();worker.engine.configure(settings);},
                    Command::Panic=>{worker.engine.pause();worker.reset();},
                    // Only Connect owns device selection; a stale preferences
                    // payload cannot relabel or disconnect an active MIDI port.
                    Command::Preferences(mut p)=>{p.device_id=worker.preferences.device_id.clone();if worker.preferences.monitor!=p.monitor||worker.preferences.simulated!=p.simulated {worker.engine.pause();worker.reset();}worker.preferences=p;},
                    Command::Connect(id,reply)=>{let result=worker.connect(id,&midi_rx,midi_tx.clone(),dropped.clone());reply.send(result).ok();},
                    Command::Input{pitch,on,velocity}=>{if worker.preferences.simulated&&pitch<128 {worker.input(midi::Message{status:if on{0x90}else{0x80},a:pitch,b:velocity.min(127)},true);}},
                }}
                for _ in 0..512 {match midi_rx.try_recv(){Ok(message)=>worker.input(message,false),Err(_)=>break}}
                let loss=dropped.load(Ordering::Relaxed)+worker.audio.sender.dropped.load(Ordering::Relaxed);
                if loss!=old_dropped {worker.engine.pause();worker.reset();worker.device_error=Some("Input/audio queue overflow; practice paused. Press Play after checking the connection.".into());old_dropped=loss;}
                if checked.elapsed()>=Duration::from_secs(2) {checked=Instant::now();if worker.connection.is_some(){if let Ok(available)=midi::devices(){if !available.iter().any(|d|Some(&d.id)==worker.preferences.device_id.as_ref()){worker.connection=None;worker.engine.pause();worker.reset();worker.device_error=Some("MIDI input disconnected. Refresh and reconnect the same device.".into());}}}}
                let pending=worker.engine.drain_events();worker.send_events(pending);
                if published.elapsed()>=Duration::from_millis(30) {published=Instant::now();let snapshot=worker.engine.snapshot();worker.settings.position_ms=snapshot.position_ms;
                    let audio_error=if worker.audio.error.load(Ordering::Acquire){Some("Windows audio device changed or failed. Restart Keyloom to reopen audio.".into())}else{worker.audio_error.clone()};
                    if let Ok(mut shared)=status.lock(){*shared=AppStatus{transport:snapshot,device_id:worker.preferences.device_id.clone(),device_name:worker.device_name.clone(),device_connected:worker.connection.is_some(),device_error:worker.device_error.clone(),audio_available:audio_error.is_none(),audio_error,audio_frames:worker.audio.frames.load(Ordering::Relaxed),audio_nonzero_frames:worker.audio.nonzero_frames.load(Ordering::Relaxed),events:worker.events.clone(),held:(0..128).filter(|p|worker.held.iter().any(|channel|channel[*p])).map(|p|p as u8).collect(),dropped_events:loss};}
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
}
impl Worker {
    fn checkpoint(&mut self) -> Option<(String, SongSettings)> {
        let snapshot = self.engine.snapshot();
        self.settings.position_ms = snapshot.position_ms;
        self.settings.speed = snapshot.speed;
        self.settings.mode = snapshot.mode;
        self.current.clone().map(|id| (id, self.settings.clone()))
    }
    fn reset(&mut self) {
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
