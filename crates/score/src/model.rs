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
    /// Tempo changes `(offset in measure, quarter notes per minute)`.
    pub tempos: Vec<(Tick, f64)>,
    /// Repeats, endings and jumps (PLAY-02).
    pub nav: Navigation,
    pub events: Vec<Event>,
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
