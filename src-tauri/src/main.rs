#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod midi;
use std::sync::{Mutex,Arc,mpsc,atomic::{AtomicU64,Ordering}};
use serde::Serialize;
struct Spike { audio:audio::AudioSender, connection:Mutex<Option<midir::MidiInputConnection<()>>>, events:Arc<Mutex<Vec<String>>>, tx:mpsc::SyncSender<midi::Message>, dropped:Arc<AtomicU64>, audio_error:Option<String> }
#[tauri::command]
fn devices()->Result<Vec<midi::Device>,String>{midi::devices()}
#[tauri::command]
fn connect_device(id:Option<String>,state:tauri::State<Spike>)->Result<(),String>{
    let mut connection=state.connection.lock().map_err(|e|e.to_string())?;*connection=None;state.audio.send(audio::Sound::Reset);
    if let Some(id)=id {*connection=Some(midi::connect(&id,state.tx.clone(),state.dropped.clone())?);}Ok(())
}
#[tauri::command]
fn input_note(pitch:u8,on:bool,velocity:u8,state:tauri::State<Spike>){if on {state.audio.send(audio::Sound::On{key:pitch as u32,pitch,velocity,volume:0.6})} else {state.audio.send(audio::Sound::Off(pitch as u32))}; if let Ok(mut events)=state.events.lock(){events.push(format!("SIMULATED ch1 {} pitch {pitch} velocity {velocity}",if on {"note-on"}else{"note-off"}));}}
#[derive(Serialize)]
struct Status {events:Vec<String>,audio_error:Option<String>,dropped:u64}
#[tauri::command]
fn status(state:tauri::State<Spike>)->Status {Status{events:state.events.lock().map(|e|e.clone()).unwrap_or_default(),audio_error:state.audio_error.clone(),dropped:state.dropped.load(Ordering::Relaxed)}}
fn main(){
    let (tx,rx)=mpsc::sync_channel::<midi::Message>(512);let events=Arc::new(Mutex::new(Vec::new()));let dropped=Arc::new(AtomicU64::new(0));
    let (ready_tx,ready_rx)=mpsc::sync_channel(1);let events_worker=events.clone();
    std::thread::spawn(move||{let (audio,error)=audio::Audio::open();ready_tx.send((audio.sender.clone(),error)).ok();while let Ok(message)=rx.recv(){let on=message.status&0xf0==0x90&&message.b>0;let kind=if message.status&0xf0==0xb0{"controller"}else if on{"note-on"}else{"note-off"};if let Ok(mut e)=events_worker.lock(){e.push(format!("PHYSICAL ch{} {kind} pitch/controller {} value {}",(message.status&15)+1,message.a,message.b));if e.len()>40{e.remove(0);}}}});
    let (audio,audio_error)=ready_rx.recv().expect("audio initialization worker");
    tauri::Builder::default().manage(Spike{audio,connection:Mutex::new(None),events,tx,dropped,audio_error}).invoke_handler(tauri::generate_handler![devices,connect_device,input_note,status]).run(tauri::generate_context!()).expect("Keyloom window failed");
}
