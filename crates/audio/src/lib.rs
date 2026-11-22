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
    velocity: u8,
    /// 0 = note off, 1 = note on (offs sort first at the same time).
    on: u8,
    channel: usize,
    key: u8,
}

const DRUMS: u8 = 9;
/// GM percussion keys for the click: hi wood block on the downbeat, low
/// wood block on other beats.
const CLICK_ACCENT: u8 = 76;
const CLICK_BEAT: u8 = 77;

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
    /// Beats in score seconds, `true` on downbeats.
    beats: Vec<(f64, bool)>,
    next_beat: usize,
    metronome: bool,
    count_in: bool,
    /// Count-in clicks still to play before the music: (frame since origin, accent).
    pre_roll: Vec<(u64, bool)>,
    /// Frames of count-in before the music clock starts.
    pre_frames: u64,
    /// Loop range in score seconds (start, end).
    loop_range: Option<(f64, f64)>,
    /// Start second of each written measure's first play, and its end.
    measure_spans: Vec<Option<(f64, f64)>>,
}

impl<S: Synth> Engine<S> {
    pub fn new(synth: S, tm: &Timemap, plan: &ChannelPlan, sample_rate: u32) -> Self {
        let mut events = Vec::with_capacity(tm.notes.len() * 2);
        for n in &tm.notes {
            let ch = plan.channel_of(n);
            events.push(Ev {
                sec: n.sec_on,
                velocity: n.velocity,
                on: 1,
                channel: ch,
                key: n.pitch,
            });
            events.push(Ev {
                sec: n.sec_off,
                velocity: 0,
                on: 0,
                channel: ch,
                key: n.pitch,
            });
        }
        events.sort_by(|a, b| a.sec.total_cmp(&b.sec).then(a.on.cmp(&b.on)));
        // Span of each written measure on its first pass (for A-B loops).
        let n_written = tm.measure_order.iter().max().map_or(0, |m| m + 1);
        let mut measure_spans = vec![None; n_written];
        for (visit, &mi) in tm.measure_order.iter().enumerate() {
            if measure_spans[mi].is_none() {
                let a = tm.seconds_at(tm.measure_starts[visit]);
                let b = tm.seconds_at(
                    tm.measure_starts
                        .get(visit + 1)
                        .copied()
                        .unwrap_or(tm.duration_ticks),
                );
                measure_spans[mi] = Some((a, b));
            }
        }
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
            beats: tm.beat_times(),
            next_beat: 0,
            metronome: false,
            count_in: false,
            pre_roll: Vec::new(),
            pre_frames: 0,
            loop_range: None,
            measure_spans,
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
        if !self.playing && self.count_in {
            self.prepare_count_in();
        }
        self.playing = true;
    }

    /// Metronome click on every beat while playing (PLAY-06).
    pub fn set_metronome(&mut self, on: bool) {
        self.metronome = on;
    }

    pub fn metronome(&self) -> bool {
        self.metronome
    }

    /// One measure of clicks before the music when playback starts (PLAY-06).
    pub fn set_count_in(&mut self, on: bool) {
        self.count_in = on;
    }

    pub fn count_in(&self) -> bool {
        self.count_in
    }

    /// Loop written measures `a..=b` (0-based), or `None` to play through
    /// (PLAY-07). The loop uses each measure's first pass.
    pub fn set_loop(&mut self, range: Option<(usize, usize)>) {
        self.loop_range = range.and_then(|(a, b)| {
            let (a, b) = (a.min(b), a.max(b));
            let start = self.measure_spans.get(a).copied().flatten()?.0;
            let end = self.measure_spans.get(b).copied().flatten()?.1;
            (end > start).then_some((start, end))
        });
    }

    pub fn loop_range(&self) -> Option<(f64, f64)> {
        self.loop_range
    }

    /// Count-in: the beats of the measure the music starts in, played before
    /// it at the current tempo; the music clock waits until they are done.
    fn prepare_count_in(&mut self) {
        let pos = self.position();
        // Downbeat at or before the position and the beats of that measure.
        let i = self.beats.partition_point(|b| b.0 <= pos + 1e-9);
        let start = (0..i).rev().find(|&k| self.beats[k].1).unwrap_or(0);
        let mut end = start + 1;
        while end < self.beats.len() && !self.beats[end].1 {
            end += 1;
        }
        let measure: Vec<f64> = self.beats[start..end.min(self.beats.len())]
            .iter()
            .map(|b| b.0)
            .collect();
        let beat_len = if measure.len() >= 2 {
            measure[1] - measure[0]
        } else {
            0.5
        };
        let n = measure.len().max(1);
        let fr = |sec: f64| (sec / self.tempo_scale * self.sample_rate as f64).round() as u64;
        self.pre_roll = (0..n).map(|k| (fr(beat_len * k as f64), k == 0)).collect();
        self.pre_frames = fr(beat_len * n as f64);
        self.origin = pos;
        self.frame = 0;
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.release_all();
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Score position in seconds (at the score's own tempo). During the
    /// count-in it stays at the start position.
    pub fn position(&self) -> f64 {
        let music = self.frame.saturating_sub(self.pre_frames);
        self.origin + music as f64 * self.tempo_scale / self.sample_rate as f64
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
        self.pre_frames = 0;
        self.pre_roll.clear();
        self.next = self.events.partition_point(|e| e.sec < self.origin);
        self.next_beat = self.beats.partition_point(|b| b.0 < self.origin - 1e-9);
    }

    /// 1.0 = as written, 0.5 = half speed (PLAY-06: 25% to 200%).
    pub fn set_tempo_scale(&mut self, s: f64) {
        self.origin = self.position();
        self.frame = 0;
        self.pre_frames = 0;
        self.pre_roll.clear();
        self.tempo_scale = s.clamp(0.25, 2.0);
    }

    pub fn tempo_scale(&self) -> f64 {
        self.tempo_scale
    }

    fn release_all(&mut self) {
        for (ch, key) in std::mem::take(&mut self.held) {
            self.synth.message(ch, 0x80, key, 0);
        }
        for i in 0..self.programs.len() {
            self.synth.message(midi_channel(i), 0xB0, 0x7B, 0);
        }
    }

    /// Fill the buffers with the next block of audio.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.sync_mixer();
        let frames = left.len();
        let mut done = 0;
        while done < frames {
            // Frames until the next thing is due (note, click, loop end).
            let mut run = frames - done;
            if self.playing {
                if let Some(due) = self.next_due() {
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
                if self.loop_range.is_none()
                    && self.next >= self.events.len()
                    && self.position() >= self.duration
                {
                    self.playing = false;
                }
            }
        }
    }

    /// Frame of the next event of any kind, if any.
    fn next_due(&self) -> Option<u64> {
        let mut due: Option<u64> = None;
        let mut take = |f: u64| due = Some(due.map_or(f, |d: u64| d.min(f)));
        if let Some(&(f, _)) = self.pre_roll.first() {
            take(f);
        }
        if let Some(ev) = self.events.get(self.next) {
            if self.loop_range.is_none_or(|(_, end)| ev.sec < end - 1e-9) {
                take(self.frame_of(ev.sec));
            }
        }
        if self.metronome {
            if let Some(&(sec, _)) = self.beats.get(self.next_beat) {
                take(self.frame_of(sec));
            }
        }
        if let Some((_, end)) = self.loop_range {
            take(self.frame_of(end));
        }
        due
    }

    fn click(&mut self, accent: bool) {
        let key = if accent { CLICK_ACCENT } else { CLICK_BEAT };
        let vel = if accent { 110 } else { 80 };
        self.synth.message(DRUMS, 0x80, key, 0);
        self.synth.message(DRUMS, 0x90, key, vel);
    }

    /// Output frame (since the last seek) at which score time `sec` sounds.
    fn frame_of(&self, sec: f64) -> u64 {
        let rel = (sec - self.origin) / self.tempo_scale * self.sample_rate as f64;
        // Round to the nearest frame, tolerating float noise.
        self.pre_frames + (rel - 1e-6).ceil().max(0.0) as u64
    }

    fn fire_due(&mut self) {
        // Count-in clicks.
        while let Some(&(f, accent)) = self.pre_roll.first() {
            if f > self.frame {
                break;
            }
            self.click(accent);
            self.pre_roll.remove(0);
        }
        // Loop wrap: at the end of B, release and jump to A.
        if let Some((start, end)) = self.loop_range {
            if self.frame >= self.frame_of(end) && self.frame > self.pre_frames {
                self.seek(start);
            }
        }
        // Metronome.
        while let Some(&(sec, accent)) = self.beats.get(self.next_beat) {
            if self.frame_of(sec) > self.frame {
                break;
            }
            if self.metronome && self.frame >= self.pre_frames {
                self.click(accent);
            }
            self.next_beat += 1;
        }
        while let Some(ev) = self.events.get(self.next).copied() {
            if self.loop_range.is_some_and(|(_, end)| ev.sec >= end - 1e-9) {
                break;
            }
            if self.frame_of(ev.sec) > self.frame {
                break;
            }
            let ch = midi_channel(ev.channel);
            if ev.on == 1 {
                self.synth.message(ch, 0x90, ev.key, ev.velocity.max(1));
                self.held.push((ch, ev.key));
            } else if let Some(i) = self.held.iter().position(|h| *h == (ch, ev.key)) {
                self.synth.message(ch, 0x80, ev.key, 0);
                self.held.swap_remove(i);
            }
            self.next += 1;
        }
    }
}

/// Render the whole score offline with the current mixer and tempo, as a
/// 16-bit stereo WAV (PLAY-11). Metronome, count-in and loop are left out of
/// the file (they are rehearsal aids, not the music); `tail_sec` of silence
/// after the last bar lets reverb and releases ring out. Live playback state
/// (position, playing, loop, click) is restored afterwards.
pub fn export_wav<S: Synth>(e: &mut Engine<S>, tail_sec: f64) -> Vec<u8> {
    let saved = (
        e.position(),
        e.is_playing(),
        e.loop_range,
        e.metronome,
        e.count_in,
    );
    e.pause();
    e.loop_range = None;
    e.metronome = false;
    e.count_in = false;
    e.seek(0.0);
    e.play();

    let sr = e.sample_rate;
    let music = (e.duration / e.tempo_scale * sr as f64).round() as usize;
    let total = music + (tail_sec.max(0.0) * sr as f64).round() as usize;
    let mut pcm: Vec<u8> = Vec::with_capacity(44 + total * 4);
    pcm.extend_from_slice(&wav_header(sr, total as u32));
    let block = 1024;
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        e.render(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            for x in [l[i], r[i]] {
                let v = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                pcm.extend_from_slice(&v.to_le_bytes());
            }
        }
        done += n;
    }

    e.pause();
    e.loop_range = saved.2;
    e.metronome = saved.3;
    e.count_in = saved.4;
    e.seek(saved.0);
    if saved.1 {
        e.play();
    }
    pcm
}

/// 44-byte PCM WAV header, 16-bit stereo.
fn wav_header(sample_rate: u32, frames: u32) -> [u8; 44] {
    let data = frames * 4;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    h[22..24].copy_from_slice(&2u16.to_le_bytes()); // stereo
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * 4).to_le_bytes());
    h[32..34].copy_from_slice(&4u16.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}
