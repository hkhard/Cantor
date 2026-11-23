//! Stage 7 (homr `music_xml_generator`, with the token handling of
//! `staff_parsing.parse_staffs`, `vocabulary.py` and `triplet_restoration.py`):
//! MusicXML from the symbols of each staff strip.
//!
//! [`generate`] takes the systems (for the grand-staff flags and the number
//! of staffs per system) and the decoded symbols of each strip in the order
//! [`strip_order`] gives (homr's reading order: voice by voice), and is
//! self-sufficient from there: it merges the strips into one symbol list per
//! part, cleans them up as homr does and writes the score.
mod fraction;
mod generator;
mod symbols;
mod triplets;
mod voices;
mod xml;

pub use symbols::Sym;

use crate::{Symbol, System};

/// The strips in the order homr reads them (and [`generate`] expects their
/// symbols), as (system index, staff index in the system).
pub fn strip_order(systems: &[System]) -> Vec<(usize, usize)> {
    voices::Plan::new(systems).strip_order()
}

/// The symbols of each part (homr `parse_staffs`' result).
pub fn parts(systems: &[System], tokens: &[Vec<Symbol>]) -> Vec<Vec<Sym>> {
    let tokens: Vec<Vec<Sym>> = tokens
        .iter()
        .map(|t| t.iter().map(|s| Sym::parse(s)).collect())
        .collect();
    voices::Plan::new(systems).voices(systems, &tokens)
}

/// homr `_clean_up_voice`: the clean-up [`parts`] applies to each part's
/// symbols (duplicates, over-eager tuplets, lower staff without clef,
/// unmarked triplets).
pub fn clean_up_voice(symbols: &[Sym]) -> Vec<Sym> {
    triplets::restore_unmarked_triplets(&symbols::remove_duplicated_symbols(symbols))
}

/// homr `generate_xml` of the parts, as a document like homr writes it.
pub fn generate_from_parts(parts: &[Vec<Sym>]) -> String {
    xml::to_document(&generator::generate_xml(parts))
}

/// MusicXML of a page: `tokens[i]` are the symbols of strip
/// `strip_order(systems)[i]`.
pub fn generate(systems: &[System], tokens: &[Vec<Symbol>]) -> String {
    generate_from_parts(&parts(systems, tokens))
}
