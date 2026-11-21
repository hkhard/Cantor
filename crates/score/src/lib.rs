//! MusicXML/MXL parse, model, transpose.
//!
//! Time inside the model is in ticks at a fixed [`PPQ`] (ticks per quarter note),
//! normalised from each file's `<divisions>`, so downstream crates never see
//! file-specific divisions.

mod dom;
mod model;
mod parse;

pub use model::{Event, EventKind, Measure, Part, Pitch, Score, Step, Tick, PPQ};
pub use parse::{parse_musicxml, parse_mxl, parse_score_file, ParseError};
