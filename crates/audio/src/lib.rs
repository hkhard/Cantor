//! Playback engine (M2): sample-clock scheduler, SoundFont synth, mixer.
//!
//! The engine plays the [`Timemap`] (repeats already unfolded) through one
//! MIDI channel per mixer channel from the [`ChannelPlan`]. Notes are sent on
//! their exact sample frame regardless of the audio callback's block size.
//! Mixer state (volume, pan, mute, solo) is sent to the synth as channel
//! volume / pan controllers, so changes apply to notes already sounding.
//!
//! The engine is generic over [`Synth`] so the scheduler and mixer are tested
//! without audio; [`SoundFontSynth`] wraps `rustysynth` for real sound.

mod mixer;

pub use mixer::{Mixer, Preset};

use cantor_timeline::channels::ChannelPlan;
use cantor_timeline::Timemap;
use std::path::Path;
use std::sync::Arc;

/// A MIDI synthesizer the engine can drive.
pub trait Synth {
    /// Short MIDI message: status high nibble (0x80, 0x90, 0xB0, 0xC0), channel 0..15.
    fn message(&mut self, channel: u8, status: u8, d1: u8, d2: u8);
    /// Render stereo audio into equally long buffers.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
}

/// `rustysynth` with a SoundFont (GeneralUser GS bundled for the demo).
pub struct SoundFontSynth {
    inner: rustysynth::Synthesizer,
}

impl SoundFontSynth {
    pub fn open(path: &Path, sample_rate: u32) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_bytes(&bytes, sample_rate)
    }

    pub fn from_bytes(bytes: &[u8], sample_rate: u32) -> Result<Self, String> {
        let sf = rustysynth::SoundFont::new(&mut std::io::Cursor::new(bytes))
            .map_err(|e| format!("SoundFont: {e:?}"))?;
        let mut settings = rustysynth::SynthesizerSettings::new(sample_rate as i32);
        settings.maximum_polyphony = 128;
        let inner = rustysynth::Synthesizer::new(&Arc::new(sf), &settings)
            .map_err(|e| format!("synth: {e:?}"))?;
        Ok(Self { inner })
    }
}

impl Synth for SoundFontSynth {
    fn message(&mut self, channel: u8, status: u8, d1: u8, d2: u8) {
        self.inner
            .process_midi_message(channel as i32, status as i32, d1 as i32, d2 as i32);
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.inner.render(left, right);
    }
}

/// One scheduled MIDI event in score seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ev {
    sec: f64,
    /// 0 = note off, 1 = note on (offs sort first at the same time).
    on: u8,
    channel: usize,
    key: u8,
}

const VELOCITY: u8 = 90;
const DRUMS: u8 = 9;

/// MIDI channel for mixer channel `i`, skipping the GM drum channel.
fn midi_channel(i: usize) -> u8 {
    let c = (i % 15) as u8;
    if c >= DRUMS {
        c + 1
    } else {
        c
    }
}

pub struct Engine<S: Synth> {
    synth: S,
    events: Vec<Ev>,
    programs: Vec<u8>,
    mixer: Mixer,
    sent_mix: Vec<(u8, u8)>,
    sample_rate: u32,
    /// Score time at the last seek / tempo change.
    origin: f64,
    /// Output frames played since `origin`.
    frame: u64,
    next: usize,
    playing: bool,
    tempo_scale: f64,
    duration: f64,
    /// Notes currently held, to release on seek/pause.
    held: Vec<(u8, u8)>,
}

impl<S: Synth> Engine<S> {
    pub fn new(synth: S, tm: &Timemap, plan: &ChannelPlan, sample_rate: u32) -> Self {
        let mut events = Vec::with_capacity(tm.notes.len() * 2);
        for n in &tm.notes {
            let ch = plan.channel_of(n);
            events.push(Ev {
                sec: n.sec_on,
                on: 1,
                channel: ch,
                key: n.pitch,
            });
            events.push(Ev {
                sec: n.sec_off,
                on: 0,
                channel: ch,
                key: n.pitch,
            });
        }
        events.sort_by(|a, b| a.sec.total_cmp(&b.sec).then(a.on.cmp(&b.on)));
        let mut mixer = Mixer::new(plan.channels.len());
        for (i, c) in plan.channels.iter().enumerate() {
            mixer.set_pan(i, c.pan);
        }
        let mut e = Self {
            synth,
            events,
            programs: plan.channels.iter().map(|c| c.program).collect(),
            mixer,
            sent_mix: Vec::new(),
            sample_rate,
            origin: 0.0,
            frame: 0,
            next: 0,
            playing: false,
            tempo_scale: 1.0,
            duration: tm.duration_sec,
            held: Vec::new(),
        };
        e.init_channels();
        e
    }

    fn init_channels(&mut self) {
        for (i, &p) in self.programs.iter().enumerate() {
            let ch = midi_channel(i);
            self.synth.message(ch, 0xC0, p, 0);
            self.synth.message(ch, 0xB0, 0x5B, 30); // a little reverb
        }
        self.sent_mix = vec![(255, 255); self.programs.len()];
        self.sync_mixer();
    }

    /// Send changed volume/pan to the synth.
    fn sync_mixer(&mut self) {
        for i in 0..self.programs.len() {
            let g = self.mixer.effective_gain(i);
            // CC7 is roughly amplitude-squared in SoundFont synths; map gain
            // 0..2 (+6 dB) to 0..127 with 1.0 at 90.
            let vol = (90.0 * g.sqrt()).round().clamp(0.0, 127.0) as u8;
            let pan = (64.0 + 63.0 * self.mixer.pan(i)).round().clamp(0.0, 127.0) as u8;
            if self.sent_mix[i] != (vol, pan) {
                let ch = midi_channel(i);
                self.synth.message(ch, 0xB0, 0x07, vol);
                self.synth.message(ch, 0xB0, 0x0A, pan);
                self.sent_mix[i] = (vol, pan);
            }
        }
    }

    pub fn synth(&self) -> &S {
        &self.synth
    }

    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }

    /// Mixer changes are sent to the synth at the next render call.
    pub fn mixer_mut(&mut self) -> &mut Mixer {
        &mut self.mixer
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.release_all();
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Score position in seconds (at the score's own tempo).
    pub fn position(&self) -> f64 {
        self.origin + self.frame as f64 * self.tempo_scale / self.sample_rate as f64
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn finished(&self) -> bool {
        self.next >= self.events.len() && self.position() >= self.duration
    }

    /// Jump to a score time; sounding notes stop.
    pub fn seek(&mut self, sec: f64) {
        self.release_all();
        self.origin = sec.clamp(0.0, self.duration);
        self.frame = 0;
        self.next = self.events.partition_point(|e| e.sec < self.origin);
    }

    /// 1.0 = as written, 0.5 = half speed (PLAY-06: 25% to 200%).
    pub fn set_tempo_scale(&mut self, s: f64) {
        self.origin = self.position();
        self.frame = 0;
        self.tempo_scale = s.clamp(0.25, 2.0);
    }

    pub fn tempo_scale(&self) -> f64 {
        self.tempo_scale
    }

    fn release_all(&mut self) {
        for i in 0..self.programs.len() {
            self.synth.message(midi_channel(i), 0xB0, 0x7B, 0);
        }
        self.held.clear();
    }

    /// Fill the buffers with the next block of audio.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.sync_mixer();
        let frames = left.len();
        let mut done = 0;
        while done < frames {
            // Frames until the next event is due (or the end of the block).
            let mut run = frames - done;
            if self.playing {
                if let Some(ev) = self.events.get(self.next) {
                    let due = self.frame_of(ev.sec);
                    run = run.min(due.saturating_sub(self.frame) as usize);
                }
            }
            if run > 0 {
                self.synth
                    .render(&mut left[done..done + run], &mut right[done..done + run]);
                if self.playing {
                    self.frame += run as u64;
                }
                done += run;
            }
            if self.playing {
                self.fire_due();
                if self.next >= self.events.len() && self.position() >= self.duration {
                    self.playing = false;
                }
            }
        }
    }

    /// Output frame (since the last seek) at which score time `sec` sounds.
    fn frame_of(&self, sec: f64) -> u64 {
        let rel = (sec - self.origin) / self.tempo_scale * self.sample_rate as f64;
        // Round to the nearest frame, tolerating float noise.
        (rel - 1e-6).ceil().max(0.0) as u64
    }

    fn fire_due(&mut self) {
        while let Some(ev) = self.events.get(self.next).copied() {
            if self.frame_of(ev.sec) > self.frame {
                break;
            }
            let ch = midi_channel(ev.channel);
            if ev.on == 1 {
                self.synth.message(ch, 0x90, ev.key, VELOCITY);
                self.held.push((ch, ev.key));
            } else if let Some(i) = self.held.iter().position(|h| *h == (ch, ev.key)) {
                self.synth.message(ch, 0x80, ev.key, 0);
                self.held.swap_remove(i);
            }
            self.next += 1;
        }
    }
}
