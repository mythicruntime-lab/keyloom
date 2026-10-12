//! Fixed-capacity basic practice synth. The callback has no locks, I/O or heap allocation.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::ArrayQueue;
use std::sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc};

#[derive(Clone, Copy)]
pub enum Sound { On { key: u32, pitch: u8, velocity: u8, volume: f32 }, Off(u32), Reset, ResetScheduled, Click }

#[derive(Clone)]
pub struct AudioSender { queue: Arc<ArrayQueue<Sound>>, reset: Arc<AtomicBool>, pub dropped: Arc<AtomicU64> }
impl AudioSender {
    pub fn send(&self, sound: Sound) {
        if self.queue.push(sound).is_err() { self.dropped.fetch_add(1, Ordering::Relaxed); self.reset.store(true, Ordering::Release); }
    }
}
pub struct Audio { pub sender: AudioSender, pub error: Arc<AtomicBool>, _stream: Option<cpal::Stream> }
#[derive(Clone, Copy)]
struct Voice { key: u32, phase: f32, step: f32, gain: f32, envelope: f32, held: bool, active: bool, age: u64 }
const EMPTY: Voice = Voice { key: 0, phase: 0.0, step: 0.0, gain: 0.0, envelope: 0.0, held: false, active: false, age: 0 };
impl Audio {
    pub fn open() -> (Self, Option<String>) {
        let sender = AudioSender { queue: Arc::new(ArrayQueue::new(2048)), reset: Arc::new(AtomicBool::new(false)), dropped: Arc::new(AtomicU64::new(0)) };
        let error = Arc::new(AtomicBool::new(false));
        let stream = (|| -> Result<cpal::Stream,String> {
            let device = cpal::default_host().default_output_device().ok_or("No Windows audio output device")?;
            let supported = device.default_output_config().map_err(|e|e.to_string())?;
            let config: cpal::StreamConfig = supported.clone().into();
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => build::<f32>(&device,&config,sender.clone(),error.clone()),
                cpal::SampleFormat::I16 => build::<i16>(&device,&config,sender.clone(),error.clone()),
                cpal::SampleFormat::U16 => build::<u16>(&device,&config,sender.clone(),error.clone()),
                other => return Err(format!("Unsupported audio format {other:?}")),
            }.map_err(|e|e.to_string())?;
            stream.play().map_err(|e|e.to_string())?;
            Ok(stream)
        })();
        match stream { Ok(stream)=>(Self{sender,error,_stream:Some(stream)},None), Err(message)=>(Self{sender,error,_stream:None},Some(message)) }
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(device:&cpal::Device, config:&cpal::StreamConfig, sender:AudioSender, error:Arc<AtomicBool>) -> Result<cpal::Stream,cpal::Error> {
    let rate = config.sample_rate as f32;
    let channels = config.channels as usize;
    let mut voices = [EMPTY;128];
    let mut age = 0_u64;
    device.build_output_stream(*config, move |buffer:&mut[T],_| {
        if sender.reset.swap(false,Ordering::AcqRel) { voices.fill(EMPTY); while sender.queue.pop().is_some() {} }
        for _ in 0..512 { let Some(sound)=sender.queue.pop() else { break }; match sound {
            Sound::Reset => voices.fill(EMPTY),
            Sound::ResetScheduled => { for v in &mut voices { if v.key<1_000_000 { *v=EMPTY; } } },
            Sound::Off(key) => { for v in &mut voices { if v.key==key { v.held=false; } } },
            Sound::On {key,pitch,velocity,volume} => {
                let index = voices.iter().position(|v|v.active && v.key==key).or_else(|| voices.iter().position(|v|!v.active)).unwrap_or_else(|| voices.iter().enumerate().min_by_key(|(_,v)|v.age).map(|(i,_)|i).unwrap_or(0));
                age=age.wrapping_add(1);
                voices[index]=Voice{key,phase:0.0,step:440.0*2_f32.powf((pitch as f32-69.0)/12.0)/rate,gain:(velocity as f32/127.0)*volume.clamp(0.0,1.0)*0.12,envelope:0.0,held:true,active:true,age};
            },
            Sound::Click => { let i=voices.iter().position(|v|!v.active).unwrap_or(127); voices[i]=Voice{key:u32::MAX,phase:0.0,step:1200.0/rate,gain:0.06,envelope:1.0,held:false,active:true,age}; }
        }}
        for frame in buffer.chunks_mut(channels) {
            let mut sample=0.0_f32;
            for v in &mut voices { if !v.active { continue; }
                if v.held { v.envelope=(v.envelope+1.0/(rate*0.004)).min(1.0); } else { v.envelope*=1.0-1.0/(rate*0.015); if v.envelope<0.0001 {v.active=false;continue;} }
                let phase=v.phase*std::f32::consts::TAU;
                sample+=(phase.sin()+0.18*(phase*2.0).sin()+0.06*(phase*3.0).sin())*v.gain*v.envelope;
                v.phase=(v.phase+v.step).fract();
            }
            let limited=sample/(1.0+sample.abs());
            for output in frame { *output=T::from_sample(limited); }
        }
    }, move |_| { error.store(true,Ordering::Release); }, None)
}
