//! Backing tracks (PERF-04): decoded recordings played in sync with the score
//! clock, time-stretched without pitch change for the tempo control, with
//! page-turn markers on the track timeline.
//!
//! * [`decode_audio`] / [`decode_file`]: WAV, FLAC, MP3 and AAC (M4A) through
//!   `symphonia`, to f32 stereo at the engine rate (windowed-sinc resampling).
//! * [`Stretcher`]: streaming WSOLA (waveform-similarity overlap-add). At a
//!   tempo scale of exactly 1 it is a sample-exact copy.
//! * [`BackingTrack`]: a clip, its offset against the score clock and its turn
//!   markers. The [`crate::Engine`] plays it on an extra mixer channel.

use std::path::Path;
use std::sync::Arc;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Decoded stereo audio at a known sample rate.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioClip {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl AudioClip {
    /// Stereo clip from two equally long channels.
    pub fn new(sample_rate: u32, left: Vec<f32>, right: Vec<f32>) -> Self {
        assert_eq!(left.len(), right.len(), "channels differ in length");
        Self {
            sample_rate,
            left,
            right,
        }
    }

    /// Mono clip (the same signal on both sides).
    pub fn mono(sample_rate: u32, samples: Vec<f32>) -> Self {
        Self::new(sample_rate, samples.clone(), samples)
    }

    pub fn frames(&self) -> usize {
        self.left.len()
    }

    pub fn duration_sec(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 / self.sample_rate as f64
    }

    /// The clip at another sample rate (a copy when the rate is the same).
    pub fn resampled(&self, rate: u32) -> AudioClip {
        if rate == self.sample_rate || self.sample_rate == 0 {
            return self.clone();
        }
        AudioClip {
            sample_rate: rate,
            left: resample(&self.left, self.sample_rate, rate),
            right: resample(&self.right, self.sample_rate, rate),
        }
    }

    #[inline]
    fn at(&self, i: i64) -> (f32, f32) {
        if i < 0 || i as usize >= self.left.len() {
            (0.0, 0.0)
        } else {
            (self.left[i as usize], self.right[i as usize])
        }
    }

    #[inline]
    fn mono_at(&self, i: i64) -> f32 {
        let (l, r) = self.at(i);
        0.5 * (l + r)
    }
}

/// Read and decode an audio file (extension used as a format hint).
pub fn decode_file(path: &Path, sample_rate: u32) -> Result<AudioClip, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let ext = path.extension().and_then(|e| e.to_str());
    decode_audio(&bytes, ext, sample_rate).map_err(|e| format!("{}: {e}", path.display()))
}

/// Decode WAV / FLAC / MP3 / AAC-in-M4A bytes to stereo f32 at `sample_rate`.
/// Mono is duplicated to both sides; beyond two channels the first two are kept.
pub fn decode_audio(
    bytes: &[u8],
    ext_hint: Option<&str>,
    sample_rate: u32,
) -> Result<AudioClip, String> {
    let mss = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        Default::default(),
    );
    let mut hint = Hint::new();
    if let Some(ext) = ext_hint {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("unsupported audio: {e}"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("no audio track")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("no audio codec parameters")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| format!("unsupported codec: {e}"))?;

    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut rate = 0u32;
    let mut inter: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(format!("read: {e}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let ch = buf.spec().channels().count().max(1);
                rate = buf.spec().rate();
                inter.resize(buf.samples_interleaved(), 0.0);
                buf.copy_to_slice_interleaved(&mut inter);
                for f in inter.chunks_exact(ch) {
                    left.push(f[0]);
                    right.push(if ch > 1 { f[1] } else { f[0] });
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode: {e}")),
        }
    }
    if rate == 0 {
        return Err("no audio decoded".into());
    }
    Ok(AudioClip::new(rate, left, right).resampled(sample_rate))
}

/// Zero crossings each side of the windowed-sinc kernel.
const SINC_ZEROS: usize = 16;
/// Kernel table resolution per zero crossing.
const SINC_RES: usize = 512;

fn sinc_table() -> Vec<f32> {
    let n = SINC_ZEROS * SINC_RES + 2;
    (0..n)
        .map(|i| {
            let u = i as f64 / SINC_RES as f64;
            if u >= SINC_ZEROS as f64 {
                return 0.0;
            }
            let sinc = if u == 0.0 {
                1.0
            } else {
                (std::f64::consts::PI * u).sin() / (std::f64::consts::PI * u)
            };
            // Blackman window over [-Z, Z].
            let x = 0.5 + 0.5 * u / SINC_ZEROS as f64;
            let w = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * x).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * x).cos();
            (sinc * w) as f32
        })
        .collect()
}

/// Band-limited (Blackman-windowed sinc, 16 zero crossings) sample rate
/// conversion. The output has `len * to / from` samples.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || to == 0 || x.is_empty() {
        return x.to_vec();
    }
    let table = sinc_table();
    let step = from as f64 / to as f64; // input samples per output sample
    let fc = (1.0 / step).min(1.0); // cutoff relative to input Nyquist
    let half = SINC_ZEROS as f64 / fc; // kernel half width in input samples
    let out_len = (x.len() as f64 / step).round() as usize;
    let kernel = |d: f64| -> f32 {
        let u = (d.abs() * fc) * SINC_RES as f64;
        let i = u as usize;
        if i + 1 >= table.len() {
            return 0.0;
        }
        let f = (u - i as f64) as f32;
        table[i] + (table[i + 1] - table[i]) * f
    };
    (0..out_len)
        .map(|j| {
            let t = j as f64 * step;
            let lo = (t - half).ceil().max(0.0) as usize;
            let hi = ((t + half).floor() as usize).min(x.len() - 1);
            let (mut acc, mut wsum) = (0.0f32, 0.0f32);
            for (i, &s) in x.iter().enumerate().take(hi + 1).skip(lo) {
                let w = kernel(i as f64 - t);
                acc += s * w;
                wsum += w;
            }
            // Unity gain at DC, also at the edges where the kernel is cut.
            if wsum.abs() > 1e-6 {
                acc / wsum
            } else {
                0.0
            }
        })
        .collect()
}

/// Streaming WSOLA time-stretcher reading a clip from a start frame at a
/// playback rate (`scale`: 0.5 = half speed, same pitch). Output sample `t`
/// corresponds to clip frame `start + t * scale`, give or take the search
/// tolerance (about 10 ms). Frames outside the clip read as silence.
#[derive(Debug, Clone)]
pub struct Stretcher {
    n: usize,
    hs: usize,
    tol: i64,
    window: Vec<f32>,
    scale: f64,
    start: f64,
    /// Next grain index.
    k: i64,
    /// Clip start frame of the previous grain.
    prev: Option<i64>,
    acc_l: Vec<f32>,
    acc_r: Vec<f32>,
    ready_l: Vec<f32>,
    ready_r: Vec<f32>,
    ready_pos: usize,
    /// Next clip frame when `scale == 1` (plain copy).
    bypass: Option<i64>,
}

impl Stretcher {
    /// Grains of about 42 ms, 50% overlap, +-10 ms similarity search.
    pub fn new(sample_rate: u32) -> Self {
        let n = ((sample_rate as usize / 24) & !3).max(64);
        let hs = n / 2;
        let window = (0..n)
            .map(|i| {
                let x = i as f64 / n as f64;
                (0.5 - 0.5 * (2.0 * std::f64::consts::PI * x).cos()) as f32
            })
            .collect();
        Self {
            n,
            hs,
            tol: (n / 4) as i64,
            window,
            scale: 1.0,
            start: 0.0,
            k: 0,
            prev: None,
            acc_l: vec![0.0; n],
            acc_r: vec![0.0; n],
            ready_l: vec![0.0; hs],
            ready_r: vec![0.0; hs],
            ready_pos: hs,
            bypass: Some(0),
        }
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Output frames produced per internal step (one synthesis hop).
    pub fn hop(&self) -> usize {
        self.hs
    }

    /// Restart reading at clip frame `start` with playback rate `scale`.
    pub fn reset(&mut self, clip: &AudioClip, start: f64, scale: f64) {
        self.scale = scale.max(1e-3);
        self.start = start;
        if (self.scale - 1.0).abs() < 1e-9 {
            self.bypass = Some(start.round() as i64);
            return;
        }
        self.bypass = None;
        self.acc_l.fill(0.0);
        self.acc_r.fill(0.0);
        self.prev = None;
        // Grain -1 covers output [-hs, hs); its first half is discarded so
        // output 0 is fully covered by two overlapping grains.
        self.k = -1;
        self.step(clip);
        self.ready_pos = self.hs;
    }

    /// Fill `l` / `r` (overwrite) with the next output frames.
    pub fn pull(&mut self, clip: &AudioClip, l: &mut [f32], r: &mut [f32]) {
        if let Some(pos) = &mut self.bypass {
            for (j, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                (*a, *b) = clip.at(*pos + j as i64);
            }
            *pos += l.len() as i64;
            return;
        }
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            if self.ready_pos >= self.hs {
                self.step(clip);
                self.ready_pos = 0;
            }
            *a = self.ready_l[self.ready_pos];
            *b = self.ready_r[self.ready_pos];
            self.ready_pos += 1;
        }
    }

    /// Normalized similarity of the clip at `cand` with the natural
    /// continuation `target`, over the overlap region, sampled every `stride`.
    fn similarity(&self, clip: &AudioClip, target: i64, cand: i64, stride: usize) -> f32 {
        let (mut xy, mut yy) = (0.0f32, 1e-9f32);
        for i in (0..self.hs).step_by(stride) {
            let x = clip.mono_at(target + i as i64);
            let y = clip.mono_at(cand + i as i64);
            xy += x * y;
            yy += y * y;
        }
        xy / yy.sqrt()
    }

    /// Overlap-add grain `k` and move one hop of finished output to `ready`.
    fn step(&mut self, clip: &AudioClip) {
        let center =
            self.start + (self.k as f64 * self.hs as f64 + self.n as f64 / 2.0) * self.scale;
        let nominal = (center - self.n as f64 / 2.0).round() as i64;
        let start = match self.prev {
            None => nominal,
            Some(p) => {
                let target = p + self.hs as i64;
                // Coarse search, then refine around the best candidate.
                let mut best = (f32::NEG_INFINITY, nominal);
                let mut d = -self.tol;
                while d <= self.tol {
                    let s = self.similarity(clip, target, nominal + d, 4);
                    if s > best.0 {
                        best = (s, nominal + d);
                    }
                    d += 4;
                }
                let c = best.1;
                let mut fine = (f32::NEG_INFINITY, c);
                for cand in c - 3..=c + 3 {
                    if (cand - nominal).abs() > self.tol {
                        continue;
                    }
                    let s = self.similarity(clip, target, cand, 1);
                    if s > fine.0 {
                        fine = (s, cand);
                    }
                }
                fine.1
            }
        };
        for i in 0..self.n {
            let (l, r) = clip.at(start + i as i64);
            let w = self.window[i];
            self.acc_l[i] += l * w;
            self.acc_r[i] += r * w;
        }
        self.ready_l.copy_from_slice(&self.acc_l[..self.hs]);
        self.ready_r.copy_from_slice(&self.acc_r[..self.hs]);
        self.acc_l.copy_within(self.hs.., 0);
        self.acc_r.copy_within(self.hs.., 0);
        let tail = self.n - self.hs;
        self.acc_l[tail..].fill(0.0);
        self.acc_r[tail..].fill(0.0);
        self.prev = Some(start);
        self.k += 1;
    }
}

/// Offline time-stretch of a whole clip: `frames / scale` output frames,
/// same pitch.
pub fn stretch(clip: &AudioClip, scale: f64) -> AudioClip {
    let scale = scale.max(1e-3);
    let out = (clip.frames() as f64 / scale).round() as usize;
    let mut st = Stretcher::new(clip.sample_rate);
    st.reset(clip, 0.0, scale);
    let (mut l, mut r) = (vec![0.0; out], vec![0.0; out]);
    st.pull(clip, &mut l, &mut r);
    AudioClip::new(clip.sample_rate, l, r)
}

/// A page turn on the track timeline: at track second `at_sec` show `page`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnMarker {
    pub at_sec: f64,
    pub page: u32,
}

/// A recording played with the score. Track position = score seconds +
/// `offset_sec`.
#[derive(Debug, Clone, PartialEq)]
pub struct BackingTrack {
    pub clip: Arc<AudioClip>,
    pub offset_sec: f64,
    /// Sorted by `at_sec` when given to the engine.
    pub markers: Vec<TurnMarker>,
}

impl BackingTrack {
    pub fn new(clip: AudioClip) -> Self {
        Self {
            clip: Arc::new(clip),
            offset_sec: 0.0,
            markers: Vec::new(),
        }
    }

    pub fn with_offset(mut self, sec: f64) -> Self {
        self.offset_sec = sec;
        self
    }

    pub fn with_markers(mut self, markers: Vec<TurnMarker>) -> Self {
        self.markers = markers;
        self.sort_markers();
        self
    }

    pub fn duration_sec(&self) -> f64 {
        self.clip.duration_sec()
    }

    /// Page to show at track second `sec`: the last marker at or before it.
    pub fn page_at(&self, sec: f64) -> Option<u32> {
        self.markers
            .iter()
            .take_while(|m| m.at_sec <= sec + 1e-9)
            .last()
            .map(|m| m.page)
    }

    pub(crate) fn sort_markers(&mut self) {
        self.markers.sort_by(|a, b| a.at_sec.total_cmp(&b.at_sec));
    }
}

/// Engine-side state of the loaded backing track.
pub(crate) struct Live {
    pub track: BackingTrack,
    pub channel: usize,
    pub stretch: Stretcher,
    pub next_marker: usize,
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
}

impl Live {
    pub fn new(mut track: BackingTrack, channel: usize, sample_rate: u32) -> Self {
        if track.clip.sample_rate != sample_rate {
            track.clip = Arc::new(track.clip.resampled(sample_rate));
        }
        track.sort_markers();
        Self {
            track,
            channel,
            stretch: Stretcher::new(sample_rate),
            next_marker: 0,
            scratch_l: Vec::new(),
            scratch_r: Vec::new(),
        }
    }

    /// Restart the stretcher at score second `score_sec`.
    pub fn resync(&mut self, score_sec: f64, tempo_scale: f64) {
        let sr = self.track.clip.sample_rate as f64;
        let start = (score_sec + self.track.offset_sec) * sr;
        let clip = Arc::clone(&self.track.clip);
        self.stretch.reset(&clip, start, tempo_scale);
    }

    /// First marker at or after score second `score_sec` is the next to fire.
    pub fn rewind_markers(&mut self, score_sec: f64) {
        let t = score_sec + self.track.offset_sec - 1e-9;
        self.next_marker = self.track.markers.partition_point(|m| m.at_sec < t);
    }

    /// Score second of the next marker to fire.
    pub fn next_marker_sec(&self) -> Option<(TurnMarker, f64)> {
        let m = *self.track.markers.get(self.next_marker)?;
        Some((m, m.at_sec - self.track.offset_sec))
    }

    /// Add the next `l.len()` frames into the buffers.
    pub fn mix(&mut self, l: &mut [f32], r: &mut [f32], gain: f32, pan: f32) {
        let n = l.len();
        if self.scratch_l.len() < n {
            self.scratch_l.resize(n, 0.0);
            self.scratch_r.resize(n, 0.0);
        }
        let clip = Arc::clone(&self.track.clip);
        self.stretch
            .pull(&clip, &mut self.scratch_l[..n], &mut self.scratch_r[..n]);
        if gain == 0.0 {
            return;
        }
        // Balance: the far side fades out as the pan moves.
        let gl = gain * (1.0 - pan).min(1.0);
        let gr = gain * (1.0 + pan).min(1.0);
        for i in 0..n {
            l[i] += self.scratch_l[i] * gl;
            r[i] += self.scratch_r[i] * gr;
        }
    }
}
