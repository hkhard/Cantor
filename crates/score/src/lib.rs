//! MusicXML/MXL parse, model, transpose.
//!
//! Time inside the model is in ticks at a fixed [`PPQ`] (ticks per quarter note),
//! normalised from each file's `<divisions>`, so downstream crates never see
//! file-specific divisions.

mod dom;
mod edit;
mod layout;
mod model;
mod parse;
mod transpose;
mod write;

pub use edit::{edit_note, NoteEdit};
pub use layout::{layout_measure_boxes, LayoutBox};
pub use model::{
    Articulation, Degree, DegreeKind, Event, EventKind, Harmony, Header, Lyric, Measure,
    Navigation, Part, Pitch, Score, Step, Swing, Syllabic, Tick, PPQ,
};
pub use parse::{
    musicxml_bytes, parse_musicxml, parse_mxl, parse_score_file, read_header, ParseError,
};
pub use transpose::{musicxml_concert_pitch, musicxml_transposed, transposed_fifths};
pub use write::write_musicxml;
