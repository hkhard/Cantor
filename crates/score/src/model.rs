//! Normalised score model.

/// Ticks per quarter note used throughout Cantor.
pub const PPQ: Tick = 960;

/// Musical time in ticks at [`PPQ`].
pub type Tick = u64;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Score {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Part {
    pub id: String,
    pub name: String,
    pub measures: Vec<Measure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Measure {
    /// The `number` attribute, kept as text (MusicXML allows "1a", "X1", ...).
    pub number: String,
    pub events: Vec<Event>,
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
