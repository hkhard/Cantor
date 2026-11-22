//! MusicXML/MXL parse, model, transpose.
//!
//! Time inside the model is in ticks at a fixed [`PPQ`] (ticks per quarter note),
//! normalised from each file's `<divisions>`, so downstream crates never see
//! file-specific divisions.

mod dom;
mod model;
mod parse;
mod write;

pub use model::{
    Articulation, Event, EventKind, Lyric, Measure, Navigation, Part, Pitch, Score, Step, Swing,
    Syllabic, Tick, PPQ,
};
pub use parse::{musicxml_bytes, parse_musicxml, parse_mxl, parse_score_file, ParseError};
pub use write::write_musicxml;
