//! Normalised score model.

/// Ticks per quarter note used throughout Cantor.
pub const PPQ: Tick = 960;

/// Musical time in ticks at [`PPQ`].
pub type Tick = u64;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Score {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Part {
    pub id: String,
    pub name: String,
    /// General MIDI program, 0-based (MusicXML `<midi-program>` is 1-based).
    pub midi_program: Option<u8>,
    /// MIDI channel, 0-based (MusicXML `<midi-channel>` is 1-based).
    pub midi_channel: Option<u8>,
    pub measures: Vec<Measure>,
}

impl Part {
    /// Verse numbers that have at least one lyric, sorted.
    pub fn verses(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .measures
            .iter()
            .flat_map(|m| &m.events)
            .flat_map(|e| &e.lyrics)
            .map(|l| l.verse)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Measure {
    /// The `number` attribute, kept as text (MusicXML allows "1a", "X1", ...).
    pub number: String,
    /// Length in ticks: the time signature length, or the content length for
    /// implicit (pickup) measures and measures whose content overruns.
    pub duration: Tick,
    /// Length actually filled by notes, rests and forwards in the longest voice.
    pub content_duration: Tick,
    /// `implicit="yes"`: a pickup or split measure that does not count.
    pub implicit: bool,
    /// Time signature in effect (carried over from earlier measures).
    pub time_signature: Option<(u32, u32)>,
    /// Key signature in effect as `<key><fifths>` (sharps positive, flats
    /// negative), carried over from earlier measures.
    pub key_fifths: Option<i32>,
    /// Semitones from written to sounding pitch for a transposing instrument
    /// (MusicXML `<transpose>`: chromatic + 12 x octave-change), carried over
    /// from earlier measures. 0 for concert-pitch parts. Playback adds it.
    pub transpose: i32,
    /// Tempo changes `(offset in measure, quarter notes per minute)`.
    pub tempos: Vec<(Tick, f64)>,
    /// Repeats, endings and jumps (PLAY-02).
    pub nav: Navigation,
    /// Swing changes `(offset in measure, swing)`; `None` = straight.
    pub swing: Vec<(Tick, Option<Swing>)>,
    pub events: Vec<Event>,
}

/// Swing feel (PLAY-10): within each pair of `unit` notes (eighths or
/// sixteenths), the first lasts `first / (first + second)` of the pair.
/// MusicXML `<sound><swing>`; a bare "Swing" means triplet feel, 2:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swing {
    pub first: u32,
    pub second: u32,
    /// Length of one swung note in ticks (PPQ/2 for eighths, PPQ/4 for 16ths).
    pub unit: Tick,
}

impl Swing {
    /// Triplet swing on eighths, used when a score says "Swing" without a ratio.
    pub const TRIPLET_EIGHTHS: Swing = Swing {
        first: 2,
        second: 1,
        unit: PPQ / 2,
    };
}

/// Navigation marks of a measure, from `<barline>` and `<sound>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Navigation {
    /// `<repeat direction="forward">` at the left barline.
    pub repeat_start: bool,
    /// `<repeat direction="backward">`: total number of times the section plays
    /// (`times` attribute, default 2).
    pub repeat_end: Option<u32>,
    /// Volta numbers this measure belongs to (`<ending number="1, 2">`),
    /// carried across the measures between ending start and stop.
    pub ending: Vec<u32>,
    pub segno: Option<String>,
    pub coda: Option<String>,
    /// Jump to the coda with this label when passing here after a D.C./D.S.
    pub tocoda: Option<String>,
    /// D.S.: jump back to the segno with this label.
    pub dalsegno: Option<String>,
    pub dacapo: bool,
    /// Stop here after a D.C./D.S.
    pub fine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Onset relative to the start of the measure.
    pub offset: Tick,
    pub duration: Tick,
    /// 1-based voice number (MusicXML default 1).
    pub voice: u32,
    /// 1-based staff number within the part (MusicXML default 1).
    pub staff: u32,
    pub kind: EventKind,
    /// `<tie type="start">`: sound continues into the next note of this pitch.
    pub tie_start: bool,
    /// `<tie type="stop">`: this note continues the previous one, no new attack.
    pub tie_stop: bool,
    /// Lyrics on this note, one per verse (VOX-01).
    pub lyrics: Vec<Lyric>,
    /// Velocity (1..127) from the dynamic in effect for this staff, if the
    /// score marks one (PLAY-10).
    pub dynamic: Option<u8>,
    /// Articulation affecting playback (PLAY-10).
    pub articulation: Articulation,
}

/// Playback-relevant articulation of a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Articulation {
    /// Fraction of the written length that sounds, in percent (staccato 50).
    pub length_pct: u8,
    /// Velocity added (accent) or 0.
    pub accent: u8,
}

impl Articulation {
    pub const NORMAL: Articulation = Articulation {
        length_pct: 100,
        accent: 0,
    };
}

/// MusicXML `<syllabic>`: where a syllable sits in its word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Syllabic {
    #[default]
    Single,
    Begin,
    Middle,
    End,
}

impl Syllabic {
    pub fn parse(s: &str) -> Syllabic {
        match s.trim() {
            "begin" => Syllabic::Begin,
            "middle" => Syllabic::Middle,
            "end" => Syllabic::End,
            _ => Syllabic::Single,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Syllabic::Single => "single",
            Syllabic::Begin => "begin",
            Syllabic::Middle => "middle",
            Syllabic::End => "end",
        }
    }

    /// True if another syllable of the same word follows.
    pub fn continues_word(self) -> bool {
        matches!(self, Syllabic::Begin | Syllabic::Middle)
    }
}

/// One lyric syllable attached to a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyric {
    /// Verse number, 1-based (`number` attribute, else the next free one).
    pub verse: u32,
    /// Syllable text; empty for an extender-only lyric (melisma continuation).
    pub text: String,
    pub syllabic: Syllabic,
    /// `<extend>` present: the syllable is held over following notes.
    pub extend: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Note(Pitch),
    Rest,
}

impl EventKind {
    /// MIDI note number, if this event is pitched.
    pub fn midi(&self) -> Option<i32> {
        match self {
            EventKind::Note(p) => Some(p.midi()),
            EventKind::Rest => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Step {
    pub fn semitone(self) -> i32 {
        match self {
            Step::C => 0,
            Step::D => 2,
            Step::E => 4,
            Step::F => 5,
            Step::G => 7,
            Step::A => 9,
            Step::B => 11,
        }
    }

    pub fn parse(s: &str) -> Option<Step> {
        Some(match s {
            "C" => Step::C,
            "D" => Step::D,
            "E" => Step::E,
            "F" => Step::F,
            "G" => Step::G,
            "A" => Step::A,
            "B" => Step::B,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pitch {
    pub step: Step,
    /// Chromatic alteration in semitones (microtones are not modelled yet).
    pub alter: i32,
    pub octave: i32,
}

impl Pitch {
    /// MIDI note number, C4 = 60.
    pub fn midi(&self) -> i32 {
        (self.octave + 1) * 12 + self.step.semitone() + self.alter
    }
}
