//! Normalised score model.

/// Ticks per quarter note used throughout Cantor.
pub const PPQ: Tick = 960;

/// Musical time in ticks at [`PPQ`].
pub type Tick = u64;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Score {
    /// Title and credits from the file (`<work>`, `<movement-*>`,
    /// `<identification>`, `<credit>`).
    pub header: Header,
    pub parts: Vec<Part>,
}

/// The score's own header (LIB-01/LIB-03): what the notation program wrote
/// as title, composer and so on. Empty strings when absent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    pub work_title: String,
    pub work_number: String,
    pub movement_title: String,
    pub movement_number: String,
    pub composer: String,
    pub arranger: String,
    /// Lyricist or poet.
    pub lyricist: String,
    pub rights: String,
    /// Title from `<credit>` text on page 1, used when `<work>` and
    /// `<movement-title>` are missing (common in Dorico and PDF converters).
    pub credit_title: String,
}

/// Titles notation programs write when the user never set one.
const PLACEHOLDER_TITLES: &[&str] = &[
    "untitled",
    "untitled score",
    "title",
    "score",
    "titel",
    "namnlös",
];

impl Header {
    /// The title to show: work title, else movement title, else the title
    /// printed on page 1. `None` when the file has no real title.
    pub fn title(&self) -> Option<String> {
        [&self.work_title, &self.movement_title, &self.credit_title]
            .into_iter()
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .find(|t| !t.is_empty() && !PLACEHOLDER_TITLES.contains(&t.to_lowercase().as_str()))
    }
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
    /// Chord symbols (BAND-01), in file order, at written pitch.
    pub harmonies: Vec<Harmony>,
}

/// A chord symbol, MusicXML `<harmony>` (BAND-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harmony {
    /// Onset in the measure: the position of the element plus its `<offset>`.
    pub offset: Tick,
    /// `(step, alter)`; `None` for a chord without a root (N.C. or a function).
    pub root: Option<(Step, i32)>,
    /// MusicXML `<kind>` value (`major`, `dominant`, `minor-seventh`, ...).
    pub kind: String,
    /// `<kind text="...">`: how the file wants the kind printed.
    pub kind_text: Option<String>,
    /// Slash bass `(step, alter)`.
    pub bass: Option<(Step, i32)>,
    pub degrees: Vec<Degree>,
    /// 1-based staff (default 1).
    pub staff: u32,
}

/// `<degree>`: an added, altered or removed chord tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Degree {
    /// 1..13 (9 = ninth).
    pub value: u32,
    pub alter: i32,
    pub kind: DegreeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegreeKind {
    Add,
    Alter,
    Subtract,
}

impl DegreeKind {
    pub fn parse(s: &str) -> DegreeKind {
        match s.trim() {
            "alter" => DegreeKind::Alter,
            "subtract" => DegreeKind::Subtract,
            _ => DegreeKind::Add,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DegreeKind::Add => "add",
            DegreeKind::Alter => "alter",
            DegreeKind::Subtract => "subtract",
        }
    }
}

/// Semitones above the root of each chord tone of a MusicXML `<kind>`.
fn kind_intervals(kind: &str) -> (&'static [i32], &'static str) {
    match kind {
        "major" => (&[0, 4, 7], ""),
        "minor" => (&[0, 3, 7], "m"),
        "augmented" => (&[0, 4, 8], "+"),
        "diminished" => (&[0, 3, 6], "dim"),
        "dominant" => (&[0, 4, 7, 10], "7"),
        "major-seventh" => (&[0, 4, 7, 11], "maj7"),
        "minor-seventh" => (&[0, 3, 7, 10], "m7"),
        "diminished-seventh" => (&[0, 3, 6, 9], "dim7"),
        "augmented-seventh" => (&[0, 4, 8, 10], "+7"),
        "half-diminished" => (&[0, 3, 6, 10], "m7b5"),
        "major-minor" => (&[0, 3, 7, 11], "m(maj7)"),
        "major-sixth" => (&[0, 4, 7, 9], "6"),
        "minor-sixth" => (&[0, 3, 7, 9], "m6"),
        "dominant-ninth" => (&[0, 4, 7, 10, 14], "9"),
        "major-ninth" => (&[0, 4, 7, 11, 14], "maj9"),
        "minor-ninth" => (&[0, 3, 7, 10, 14], "m9"),
        "dominant-11th" => (&[0, 4, 7, 10, 14, 17], "11"),
        "major-11th" => (&[0, 4, 7, 11, 14, 17], "maj11"),
        "minor-11th" => (&[0, 3, 7, 10, 14, 17], "m11"),
        "dominant-13th" => (&[0, 4, 7, 10, 14, 21], "13"),
        "major-13th" => (&[0, 4, 7, 11, 14, 21], "maj13"),
        "minor-13th" => (&[0, 3, 7, 10, 14, 21], "m13"),
        "suspended-second" => (&[0, 2, 7], "sus2"),
        "suspended-fourth" => (&[0, 5, 7], "sus4"),
        "power" => (&[0, 7], "5"),
        "none" => (&[], "N.C."),
        // pedal, Neapolitan, Italian, ..., other: the root alone.
        _ => (&[0], ""),
    }
}

/// Semitones above the root of chord degree `value` (major scale, but the
/// 7th is the dominant seventh as in chord-symbol practice).
fn degree_semitones(value: u32) -> i32 {
    let v = value.max(1) - 1;
    let octave = (v / 7) as i32 * 12;
    octave + [0, 2, 4, 5, 7, 9, 10][(v % 7) as usize]
}

fn note_name(step: Step, alter: i32) -> String {
    let acc = match alter {
        a if a < 0 => "b".repeat((-a) as usize),
        a => "#".repeat(a as usize),
    };
    format!("{step:?}{acc}")
}

impl Harmony {
    /// Printable symbol, e.g. `Bb7`, `Am7`, `F/A`, `C7(b9)`, `N.C.`.
    pub fn symbol(&self) -> String {
        let (_, suffix) = kind_intervals(&self.kind);
        let mut s = match self.root {
            Some((st, al)) if self.kind != "none" => note_name(st, al),
            _ => String::new(),
        };
        s.push_str(suffix);
        for d in &self.degrees {
            let acc = match d.alter {
                a if a < 0 => "b",
                a if a > 0 => "#",
                _ => "",
            };
            let text = match d.kind {
                DegreeKind::Add if acc.is_empty() => format!("(add{})", d.value),
                DegreeKind::Subtract => format!("(no{})", d.value),
                _ => format!("({acc}{})", d.value),
            };
            s.push_str(&text);
        }
        if let Some((st, al)) = self.bass {
            s.push('/');
            s.push_str(&note_name(st, al));
        }
        s
    }

    /// Chord tones as semitones above the root (BAND-05), sorted, degrees
    /// applied. Empty for N.C. or a chord without a root.
    pub fn intervals(&self) -> Vec<i32> {
        if self.root.is_none() {
            return Vec::new();
        }
        let mut v: Vec<i32> = kind_intervals(&self.kind).0.to_vec();
        for d in &self.degrees {
            let base = degree_semitones(d.value);
            let same = |x: &i32| x.rem_euclid(12) == base.rem_euclid(12);
            match d.kind {
                DegreeKind::Add => v.push(base + d.alter),
                DegreeKind::Alter => {
                    v.retain(|x| !same(x));
                    v.push(base + d.alter);
                }
                DegreeKind::Subtract => v.retain(|x| !same(x)),
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    }
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
