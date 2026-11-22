//! cpal output stream driving the engine.
//!
//! The audio callback locks the engine with `try_lock`: if the UI thread holds
//! it (a mixer change), the callback outputs silence for that buffer instead
//! of blocking the audio thread.

use cantor_audio::{Engine, SoundFontSynth};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::sync::{Arc, Mutex};

pub struct Output {
    _stream: cpal::Stream,
}

// cpal::Stream is !Send on some platforms; the app only creates and drops it
// on the command thread while holding the player lock.
unsafe impl Send for Output {}

pub fn output_sample_rate() -> Option<u32> {
    let device = cpal::default_host().default_output_device()?;
    let cfg = device.default_output_config().ok()?;
    Some(cfg.sample_rate())
}

impl Output {
    pub fn start(engine: Arc<Mutex<Engine<SoundFontSynth>>>) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, engine),
            SampleFormat::I16 => build::<i16>(&device, config, engine),
            SampleFormat::I32 => build::<i32>(&device, config, engine),
            other => return Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream })
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    engine: Arc<Mutex<Engine<SoundFontSynth>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut left: Vec<f32> = Vec::new();
    let mut right: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let frames = data.len() / channels.max(1);
                if left.len() < frames {
                    left.resize(frames, 0.0);
                    right.resize(frames, 0.0);
                }
                let (l, r) = (&mut left[..frames], &mut right[..frames]);
                match engine.try_lock() {
                    Ok(mut e) => e.render(l, r),
                    Err(_) => {
                        l.fill(0.0);
                        r.fill(0.0);
                    }
                }
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    for (c, s) in frame.iter_mut().enumerate() {
                        let v = if c % 2 == 0 { l[i] } else { r[i] };
                        *s = T::from_sample(v.clamp(-1.0, 1.0));
                    }
                }
            },
            |err| log::warn!("audio: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
