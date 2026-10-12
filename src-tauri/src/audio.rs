//! Fixed-capacity basic practice synth. The callback has no locks, I/O or heap allocation.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::ArrayQueue;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
pub enum Sound {
    On {
        key: u32,
        pitch: u8,
        velocity: u8,
        volume: f32,
    },
    Off(u32),
    Reset,
    ResetScheduled,
    Click,
}

#[derive(Clone)]
pub struct AudioSender {
    queue: Arc<ArrayQueue<Sound>>,
    reset: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    pub dropped: Arc<AtomicU64>,
}
impl AudioSender {
    /// Mute only the PC output. Voices still consume note-offs and advance their
    /// envelopes, so unmuting resumes current voices without replaying a backlog.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    pub fn send(&self, sound: Sound) {
        // Yamaha-speaker practice remains usable without a Windows audio device.
        // A dead callback must not accumulate an unconsumed queue or cause a
        // perpetual overflow/reset loop in the transport owner.
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        if self.queue.push(sound).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            self.reset.store(true, Ordering::Release);
        }
    }
}
pub struct Audio {
    pub sender: AudioSender,
    pub error: Arc<AtomicBool>,
    pub frames: Arc<AtomicU64>,
    pub nonzero_frames: Arc<AtomicU64>,
    _stream: Option<cpal::Stream>,
}
#[derive(Clone, Copy)]
struct Voice {
    key: u32,
    phase: f32,
    step: f32,
    gain: f32,
    envelope: f32,
    held: bool,
    active: bool,
    age: u64,
}
const EMPTY: Voice = Voice {
    key: 0,
    phase: 0.0,
    step: 0.0,
    gain: 0.0,
    envelope: 0.0,
    held: false,
    active: false,
    age: 0,
};
impl Audio {
    pub fn open() -> (Self, Option<String>) {
        let sender = AudioSender {
            queue: Arc::new(ArrayQueue::new(2048)),
            reset: Arc::new(AtomicBool::new(false)),
            // No sender escapes before open completes. Enable before starting
            // the callback so a concurrent error can only disable it; success
            // must never overwrite a callback's failure with true.
            enabled: Arc::new(AtomicBool::new(true)),
            muted: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        };
        let error = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicU64::new(0));
        let nonzero_frames = Arc::new(AtomicU64::new(0));
        let stream = (|| -> Result<cpal::Stream, String> {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("No Windows audio output device")?;
            let supported = device.default_output_config().map_err(|e| e.to_string())?;
            let config: cpal::StreamConfig = supported.into();
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => build::<f32>(
                    &device,
                    &config,
                    sender.clone(),
                    error.clone(),
                    frames.clone(),
                    nonzero_frames.clone(),
                ),
                cpal::SampleFormat::I16 => build::<i16>(
                    &device,
                    &config,
                    sender.clone(),
                    error.clone(),
                    frames.clone(),
                    nonzero_frames.clone(),
                ),
                cpal::SampleFormat::U16 => build::<u16>(
                    &device,
                    &config,
                    sender.clone(),
                    error.clone(),
                    frames.clone(),
                    nonzero_frames.clone(),
                ),
                other => return Err(format!("Unsupported audio format {other:?}")),
            }
            .map_err(|e| e.to_string())?;
            stream.play().map_err(|e| e.to_string())?;
            Ok(stream)
        })();
        match stream {
            Ok(stream) => (
                Self {
                    sender,
                    error,
                    frames,
                    nonzero_frames,
                    _stream: Some(stream),
                },
                None,
            ),
            Err(message) => {
                sender.enabled.store(false, Ordering::Release);
                (
                    Self {
                        sender,
                        error,
                        frames,
                        nonzero_frames,
                        _stream: None,
                    },
                    Some(message),
                )
            }
        }
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sender: AudioSender,
    error: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
    nonzero_frames: Arc<AtomicU64>,
) -> Result<cpal::Stream, cpal::Error> {
    let rate = config.sample_rate as f32;
    let channels = config.channels as usize;
    let mut voices = [EMPTY; 128];
    let mut age = 0_u64;
    let enabled = sender.enabled.clone();
    device.build_output_stream(
        *config,
        move |buffer: &mut [T], _| {
            if !sender.enabled.load(Ordering::Acquire) || sender.reset.swap(false, Ordering::AcqRel)
            {
                voices.fill(EMPTY);
                while sender.queue.pop().is_some() {}
            }
            for _ in 0..512 {
                let Some(sound) = sender.queue.pop() else {
                    break;
                };
                match sound {
                    Sound::Reset => voices.fill(EMPTY),
                    Sound::ResetScheduled => {
                        for v in &mut voices {
                            if v.key < 1_000_000 {
                                *v = EMPTY;
                            }
                        }
                    }
                    Sound::Off(key) => {
                        for v in &mut voices {
                            if v.key == key {
                                v.held = false;
                            }
                        }
                    }
                    Sound::On {
                        key,
                        pitch,
                        velocity,
                        volume,
                    } => {
                        let index = voices
                            .iter()
                            .position(|v| v.active && v.key == key)
                            .or_else(|| voices.iter().position(|v| !v.active))
                            .unwrap_or_else(|| {
                                voices
                                    .iter()
                                    .enumerate()
                                    .min_by_key(|(_, v)| v.age)
                                    .map(|(i, _)| i)
                                    .unwrap_or(0)
                            });
                        age = age.wrapping_add(1);
                        voices[index] = Voice {
                            key,
                            phase: 0.0,
                            step: 440.0 * 2_f32.powf((pitch as f32 - 69.0) / 12.0) / rate,
                            gain: (velocity as f32 / 127.0) * volume.clamp(0.0, 1.0) * 0.12,
                            envelope: 0.0,
                            held: true,
                            active: true,
                            age,
                        };
                    }
                    Sound::Click => {
                        let i = voices.iter().position(|v| !v.active).unwrap_or(127);
                        voices[i] = Voice {
                            key: u32::MAX,
                            phase: 0.0,
                            step: 1200.0 / rate,
                            gain: 0.06,
                            envelope: 1.0,
                            held: false,
                            active: true,
                            age,
                        };
                    }
                }
            }
            if !voices.iter().any(|voice| voice.active) {
                buffer.fill(T::from_sample(0.0));
                frames.fetch_add((buffer.len() / channels) as u64, Ordering::Relaxed);
                return;
            }
            // Read once per callback: a preference change takes effect by the
            // next output buffer, without locks, allocation or MIDI side effects.
            let muted = sender.muted.load(Ordering::Acquire);
            let mut nonzero = 0_u64;
            for frame in buffer.chunks_mut(channels) {
                let mut sample = 0.0_f32;
                for v in &mut voices {
                    if !v.active {
                        continue;
                    }
                    if v.held {
                        v.envelope = (v.envelope + 1.0 / (rate * 0.004)).min(1.0);
                    } else {
                        v.envelope *= 1.0 - 1.0 / (rate * 0.015);
                        if v.envelope < 0.0001 {
                            v.active = false;
                            continue;
                        }
                    }
                    let phase = v.phase * std::f32::consts::TAU;
                    sample +=
                        (phase.sin() + 0.18 * (phase * 2.0).sin() + 0.06 * (phase * 3.0).sin())
                            * v.gain
                            * v.envelope;
                    v.phase = (v.phase + v.step).fract();
                }
                let limited = if muted {
                    0.0
                } else {
                    sample / (1.0 + sample.abs())
                };
                if limited.abs() > 0.000_001 {
                    nonzero += 1;
                }
                for output in frame {
                    *output = T::from_sample(limited);
                }
            }
            frames.fetch_add((buffer.len() / channels) as u64, Ordering::Relaxed);
            nonzero_frames.fetch_add(nonzero, Ordering::Relaxed);
        },
        move |_| {
            enabled.store(false, Ordering::Release);
            error.store(true, Ordering::Release);
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sender(enabled: bool) -> AudioSender {
        AudioSender {
            queue: Arc::new(ArrayQueue::new(2)),
            reset: Arc::new(AtomicBool::new(false)),
            enabled: Arc::new(AtomicBool::new(enabled)),
            muted: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }
    #[test]
    fn unavailable_audio_discards_without_accumulating_or_overflowing() {
        let audio = sender(false);
        for _ in 0..5000 {
            audio.send(Sound::Click);
            audio.send(Sound::Reset);
        }
        assert!(audio.queue.is_empty());
        assert_eq!(audio.dropped.load(Ordering::Relaxed), 0);
        assert!(!audio.reset.load(Ordering::Relaxed));
    }
    #[test]
    fn enabled_overflow_requests_one_reset_and_disabled_sender_stops_growing() {
        let audio = sender(true);
        audio.send(Sound::Click);
        audio.send(Sound::Click);
        audio.send(Sound::Click);
        assert_eq!(audio.dropped.load(Ordering::Relaxed), 1);
        assert!(audio.reset.load(Ordering::Relaxed));
        audio.enabled.store(false, Ordering::Release);
        for _ in 0..5000 {
            audio.send(Sound::Reset);
        }
        assert_eq!(audio.dropped.load(Ordering::Relaxed), 1);
        assert_eq!(audio.queue.len(), 2);
    }
}
