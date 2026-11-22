//! Per-channel mixer state (PLAY-03) and level presets (PLAY-04).

/// Level presets for rehearsal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// My channel at 0 dB, the others at -12 dB.
    MyPart(usize),
    /// My channel muted, the others at 0 dB.
    WithoutMe(usize),
    /// Everything at 0 dB, nothing muted or soloed.
    Balanced,
}

#[derive(Debug, Clone, PartialEq)]
struct Strip {
    volume_db: f32,
    pan: f32,
    mute: bool,
    solo: bool,
}

impl Default for Strip {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

pub const MAX_DB: f32 = 6.0;
pub const OTHERS_DB: f32 = -12.0;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mixer {
    strips: Vec<Strip>,
}

impl Mixer {
    pub fn new(channels: usize) -> Self {
        Self {
            strips: vec![Strip::default(); channels],
        }
    }

    pub fn len(&self) -> usize {
        self.strips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strips.is_empty()
    }

    /// Add a channel at the end (0 dB, centre); returns its index.
    pub fn push_channel(&mut self) -> usize {
        self.strips.push(Strip::default());
        self.strips.len() - 1
    }

    /// Keep only the first `n` channels.
    pub fn truncate(&mut self, n: usize) {
        self.strips.truncate(n);
    }

    /// -inf .. +6 dB.
    pub fn set_volume_db(&mut self, i: usize, db: f32) {
        if let Some(s) = self.strips.get_mut(i) {
            s.volume_db = if db.is_nan() { 0.0 } else { db.min(MAX_DB) };
        }
    }

    pub fn volume_db(&self, i: usize) -> f32 {
        self.strips.get(i).map_or(0.0, |s| s.volume_db)
    }

    pub fn set_pan(&mut self, i: usize, pan: f32) {
        if let Some(s) = self.strips.get_mut(i) {
            s.pan = pan.clamp(-1.0, 1.0);
        }
    }

    pub fn pan(&self, i: usize) -> f32 {
        self.strips.get(i).map_or(0.0, |s| s.pan)
    }

    pub fn set_mute(&mut self, i: usize, on: bool) {
        if let Some(s) = self.strips.get_mut(i) {
            s.mute = on;
        }
    }

    pub fn muted(&self, i: usize) -> bool {
        self.strips.get(i).is_some_and(|s| s.mute)
    }

    pub fn set_solo(&mut self, i: usize, on: bool) {
        if let Some(s) = self.strips.get_mut(i) {
            s.solo = on;
        }
    }

    pub fn soloed(&self, i: usize) -> bool {
        self.strips.get(i).is_some_and(|s| s.solo)
    }

    /// Linear gain after volume, mute and solo. Mute always wins; when any
    /// channel is soloed, only soloed channels sound.
    pub fn effective_gain(&self, i: usize) -> f32 {
        let Some(s) = self.strips.get(i) else {
            return 0.0;
        };
        let any_solo = self.strips.iter().any(|s| s.solo);
        if s.mute || (any_solo && !s.solo) || s.volume_db == f32::NEG_INFINITY {
            return 0.0;
        }
        10f32.powf(s.volume_db / 20.0)
    }

    pub fn apply(&mut self, p: Preset) {
        for (i, s) in self.strips.iter_mut().enumerate() {
            s.mute = false;
            s.solo = false;
            s.volume_db = match p {
                Preset::MyPart(me) if i != me => OTHERS_DB,
                _ => 0.0,
            };
            if p == Preset::WithoutMe(i) {
                s.mute = true;
            }
        }
    }
}
