//! Stage 7 (homr `music_xml_generator`): MusicXML from the symbols per staff.
use crate::{Symbol, System};

pub fn generate(systems: &[System], tokens: &[Vec<Symbol>]) -> String {
    let _ = (systems, tokens);
    todo!("port homr generate_xml")
}
