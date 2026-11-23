//! homr `transformer/vocabulary.py`: the six decoder vocabularies and
//! `EncodedSymbol` with its text form.
//!
//! The decoder's vocabularies are built in code (homr `Vocabulary()`), not
//! read from the `tokenizer_*.json` files: those are an older, smaller
//! tokenizer that homr 0.7.1 still ships but no longer uses for decoding.

/// No value for this branch (homr `nonote`).
pub const NONOTE: &str = ".";
/// A note without decoration (homr `empty`).
pub const EMPTY: &str = "_";

const VALID_TIME_SIGNATURE_DENOMINATORS: [u32; 10] = [1, 2, 3, 4, 6, 8, 12, 16, 32, 48];
const DYNAMICS: [&str; 18] = [
    "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "sfz", "sffz", "fp", "sf", "fz", "rf",
    "rfz", "sfp", "empty",
];
const DYNAMICS_MODIFIER: [&str; 9] = [
    "piu", "dolce", "poco", "espress", "subito", "atempo", "leggiero", "pocopiu", "sempre",
];

const ARTICULATIONS_LIEDER: [&str; 78] = [
    "accent",
    "accent_accentStrong",
    "accent_arpeggiate",
    "accent_arpeggiate_fermata",
    "accent_arpeggiate_staccato",
    "accent_arpeggiate_tenuto",
    "accent_breathMark",
    "accent_breathMark_fermata",
    "accent_fermata",
    "accent_fermata_tremolo",
    "accent_staccatissimo",
    "accent_staccato",
    "accent_staccato_tenuto",
    "accent_staccato_tremolo",
    "accent_staccato_trill",
    "accent_tenuto",
    "accent_tremolo",
    "accent_trill",
    "accentStrong",
    "accentStrong_arpeggiate",
    "accentStrong_arpeggiate_fermata",
    "accentStrong_arpeggiate_staccato",
    "accentStrong_arpeggiate_tenuto",
    "accentStrong_breathMark",
    "accentStrong_breathMark_fermata",
    "accentStrong_fermata",
    "accentStrong_fermata_tremolo",
    "accentStrong_staccatissimo",
    "accentStrong_staccato",
    "accentStrong_staccato_tenuto",
    "accentStrong_staccato_tremolo",
    "accentStrong_staccato_trill",
    "accentStrong_tenuto",
    "accentStrong_tremolo",
    "accentStrong_trill",
    "arpeggiate",
    "arpeggiate_breathMark",
    "arpeggiate_fermata",
    "arpeggiate_fermata_staccato",
    "arpeggiate_fermata_tenuto",
    "arpeggiate_staccatissimo",
    "arpeggiate_staccatissimo_staccato",
    "arpeggiate_staccato",
    "arpeggiate_staccato_tenuto",
    "arpeggiate_tenuto",
    "arpeggiate_tremolo",
    "arpeggiate_trill",
    "breathMark",
    "breathMark_fermata",
    "breathMark_fermata_tenuto",
    "breathMark_staccato",
    "breathMark_tenuto",
    "breathMark_tremolo",
    "breathMark_trill",
    "fermata",
    "fermata_staccato",
    "fermata_tenuto",
    "fermata_tremolo",
    "fermata_trill",
    "fermata_turn",
    "mordent",
    "staccatissimo",
    "staccatissimo_staccato",
    "staccatissimo_staccato_tenuto",
    "staccatissimo_staccato_tenuto_trill",
    "staccatissimo_tenuto",
    "staccato",
    "staccato_tenuto",
    "staccato_tremolo",
    "staccato_trill",
    "staccato_turn",
    "tenuto",
    "tenuto_tremolo",
    "tenuto_trill",
    "tremolo",
    "trill",
    "trill_turn",
    "turn",
];

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// homr `build_rhythm()`, in token order.
pub fn rhythm() -> Vec<String> {
    let mut r = owned(&["PAD", "BOS", "EOS", "chord"]);
    r.extend(owned(&["barline", "doublebarline", "bolddoublebarline"]));
    r.extend(owned(&["repeatStart", "repeatEnd", "repeatEndStart"]));
    r.extend(owned(&["voltaStart", "voltaStop", "voltaDiscontinue"]));
    r.extend((3..6).map(|c| format!("clef_F{c}")));
    r.extend((1..6).map(|c| format!("clef_C{c}")));
    r.extend((1..3).map(|c| format!("clef_G{c}")));
    r.push("clef_TAB5".into());
    r.extend((-7..8).map(|c| format!("keySignature_{c}")));
    r.extend(
        VALID_TIME_SIGNATURE_DENOMINATORS
            .iter()
            .map(|c| format!("timeSignature/{c}")),
    );
    r.extend((2..11).map(|c| format!("rest_{c}m")));
    let kern_base = [0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 32, 64, 128];
    let mut kern = vec![];
    for d in kern_base {
        for g in ["", "G"] {
            for dot in ["", ".", ".."] {
                kern.push(format!("{d}{g}{dot}"));
            }
        }
    }
    let irregular = [
        7, 11, 13, 18, 20, 21, 22, 24, 26, 28, 30, 34, 36, 40, 48, 56, 96,
    ];
    r.extend(kern.iter().map(|d| format!("note_{d}")));
    r.extend(irregular.iter().map(|d| format!("note_{d}")));
    r.extend(kern.iter().map(|d| format!("rest_{d}")));
    r.extend(irregular.iter().map(|d| format!("rest_{d}")));
    r.extend(DYNAMICS.iter().map(|d| format!("dynamic_{d}")));
    r
}

/// homr `build_lift()`.
pub fn lift() -> Vec<String> {
    owned(&[NONOTE, EMPTY, "#", "##", "N", "b", "bb"])
}

/// homr `build_position()`.
pub fn position() -> Vec<String> {
    owned(&[NONOTE, "upper", "upper2", "lower", "lower2"])
}

/// homr `build_articulation()`.
pub fn articulation() -> Vec<String> {
    let mut a = owned(&[NONOTE, EMPTY]);
    a.extend(owned(&ARTICULATIONS_LIEDER));
    a.extend(DYNAMICS_MODIFIER.iter().map(|d| format!("dynamic_{d}")));
    a
}

/// homr `build_slur()`.
pub fn slur() -> Vec<String> {
    owned(&[NONOTE, EMPTY, "slurStart_slurStop", "slurStart", "slurStop"])
}

/// homr `build_pitch()`: `.`, `_`, then B9 down to C0.
pub fn pitch() -> Vec<String> {
    let mut p = owned(&[NONOTE, EMPTY]);
    let mut names = vec![];
    for octave in 0..10 {
        for n in ["C", "D", "E", "F", "G", "A", "B"] {
            names.push(format!("{n}{octave}"));
        }
    }
    p.extend(names.into_iter().rev());
    p
}

/// All six vocabularies, indexed by token id (homr `Vocabulary`).
#[derive(Debug, Clone)]
pub struct Vocabulary {
    pub rhythm: Vec<String>,
    pub pitch: Vec<String>,
    pub lift: Vec<String>,
    pub articulation: Vec<String>,
    pub slur: Vec<String>,
    pub position: Vec<String>,
}

impl Default for Vocabulary {
    fn default() -> Self {
        Vocabulary {
            rhythm: rhythm(),
            pitch: pitch(),
            lift: lift(),
            articulation: articulation(),
            slur: slur(),
            position: position(),
        }
    }
}

/// A symbol split into the decoder's branches (homr `EncodedSymbol`).
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedSymbol {
    pub rhythm: String,
    pub pitch: String,
    pub lift: String,
    pub articulation: String,
    pub slur: String,
    pub position: String,
    /// From the decoder's attention, imprecise (homr `coordinates`).
    pub coordinates: Option<(f32, f32)>,
}

impl EncodedSymbol {
    /// A symbol with only a rhythm, the other branches `.`.
    pub fn new(rhythm: &str) -> Self {
        EncodedSymbol {
            rhythm: rhythm.into(),
            pitch: NONOTE.into(),
            lift: NONOTE.into(),
            articulation: NONOTE.into(),
            slur: NONOTE.into(),
            position: NONOTE.into(),
            coordinates: None,
        }
    }
}

impl std::fmt::Display for EncodedSymbol {
    /// homr `__str__`: rhythm pitch lift articulation slur position.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {}",
            self.rhythm, self.pitch, self.lift, self.articulation, self.slur, self.position
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_homr_vocabulary_sizes_match_the_decoder_outputs() {
        let v = Vocabulary::default();
        // The decoder's output widths (out_rhythms 278, out_pitchs 72, ...).
        assert_eq!(v.rhythm.len(), 278);
        assert_eq!(v.pitch.len(), 72);
        assert_eq!(v.lift.len(), 7);
        assert_eq!(v.articulation.len(), 89);
        assert_eq!(v.slur.len(), 5);
        assert_eq!(v.position.len(), 5);
        assert_eq!(v.rhythm[2], "EOS");
        assert_eq!(v.pitch[2], "B9");
        assert_eq!(v.pitch[71], "C0");
    }

    #[test]
    fn lib_06_homr_vocabulary_equals_python_vocabulary() {
        // fixtures/omr/homr_vocabulary.json: homr 0.7.1 `Vocabulary()`, each
        // dict's keys in id order (dumped with homr's Python).
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/omr/homr_vocabulary.json"
        );
        let want: std::collections::HashMap<String, Vec<String>> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let v = Vocabulary::default();
        assert_eq!(want["rhythm"], v.rhythm);
        assert_eq!(want["pitch"], v.pitch);
        assert_eq!(want["lift"], v.lift);
        assert_eq!(want["articulation"], v.articulation);
        assert_eq!(want["slur"], v.slur);
        assert_eq!(want["position"], v.position);
    }

    #[test]
    fn lib_06_homr_symbol_text_form_matches_python_str() {
        let mut s = EncodedSymbol::new("note_4");
        s.pitch = "C4".into();
        s.lift = EMPTY.into();
        s.articulation = EMPTY.into();
        s.slur = EMPTY.into();
        s.position = "upper".into();
        assert_eq!(s.to_string(), "note_4 C4 _ _ _ upper");
        assert_eq!(
            EncodedSymbol::new("barline").to_string(),
            "barline . . . . ."
        );
    }
}
