//! cpal output stream driving the engine.
//!
//! The audio callback locks the engine with `try_lock`: if the UI thread holds
//! it (a mixer change), the callback outputs silence for that buffer instead
//! of blocking the audio thread.

use cantor_audio::{Engine, SoundFontSynth};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// NFR-01 probe, written by the audio callback without locking: frames in the
/// last callback, and the output delay (predicted playback instant minus
/// callback instant, in microseconds) as last value and maximum.
static LAST_FRAMES: AtomicU32 = AtomicU32::new(0);
static LAST_DELAY_US: AtomicU64 = AtomicU64::new(0);
static MAX_DELAY_US: AtomicU64 = AtomicU64::new(0);
static CALLBACKS: AtomicU64 = AtomicU64::new(0);

/// What the probe saw, for `audio_latency` and `.perf.log`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Latency {
    /// Frames per callback.
    pub frames: u32,
    pub sample_rate: u32,
    /// One callback buffer in ms.
    pub buffer_ms: f64,
    /// Output delay reported by the platform (callback to DAC), last and max.
    pub delay_ms: f64,
    pub max_delay_ms: f64,
    pub callbacks: u64,
    /// NFR-01: the buffer is within budget (10 ms Apple, 20 ms Android).
    pub within_budget: bool,
}

/// NFR-01 output-buffer budget in ms for this platform.
pub const fn budget_ms() -> f64 {
    if cfg!(target_os = "android") || cfg!(target_os = "windows") {
        20.0
    } else {
        10.0
    }
}

pub fn summarize(frames: u32, sample_rate: u32, delay_us: u64, max_us: u64, n: u64) -> Latency {
    let buffer_ms = if sample_rate == 0 {
        0.0
    } else {
        frames as f64 * 1000.0 / sample_rate as f64
    };
    Latency {
        frames,
        sample_rate,
        buffer_ms,
        delay_ms: delay_us as f64 / 1000.0,
        max_delay_ms: max_us as f64 / 1000.0,
        callbacks: n,
        within_budget: n > 0 && buffer_ms <= budget_ms(),
    }
}

pub fn latency(sample_rate: u32) -> Latency {
    summarize(
        LAST_FRAMES.load(Ordering::Relaxed),
        sample_rate,
        LAST_DELAY_US.load(Ordering::Relaxed),
        MAX_DELAY_US.load(Ordering::Relaxed),
        CALLBACKS.load(Ordering::Relaxed),
    )
}

fn record(frames: usize, info: &cpal::OutputCallbackInfo) {
    let ts = info.timestamp();
    let us = ts.playback.duration_since(ts.callback).as_micros() as u64;
    LAST_FRAMES.store(frames as u32, Ordering::Relaxed);
    LAST_DELAY_US.store(us, Ordering::Relaxed);
    // Skip the first callbacks: streams report a start-up delay.
    if CALLBACKS.fetch_add(1, Ordering::Relaxed) > 20 {
        MAX_DELAY_US.fetch_max(us, Ordering::Relaxed);
    }
}

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
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels.max(1);
                record(frames, info);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfr_01_latency_summary_checks_the_buffer_against_the_budget() {
        // 192 frames at 48 kHz is 4 ms: within every budget.
        let l = summarize(192, 48_000, 21_500, 30_000, 100);
        assert!((l.buffer_ms - 4.0).abs() < 1e-9);
        assert!((l.delay_ms - 21.5).abs() < 1e-9);
        assert!((l.max_delay_ms - 30.0).abs() < 1e-9);
        assert!(l.within_budget);
        // 1920 frames (a deep buffer) is 40 ms: over every budget.
        assert!(!summarize(1920, 48_000, 0, 0, 100).within_budget);
        // No callback yet: nothing to claim.
        assert!(!summarize(0, 48_000, 0, 0, 0).within_budget);
        assert_eq!(summarize(10, 0, 0, 0, 1).buffer_ms, 0.0);
    }
}
