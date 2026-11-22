//! English G2P for sung lyrics: CMUdict lookup with fallbacks for what real
//! scores (and OCR) contain: curly apostrophes, stray punctuation, compound
//! words written together, and lyric-only contractions like "ev'ry".

use cantor_vocal::english::EnglishPhonemizer;
use cantor_vocal::soulx::Phonemizer;

const DICT: &str = ";;; test subset of cmudict
christmas K R IH1 S M AH0 S
merry M EH1 R IY0
snow S N OW1
covered K AH1 V ER0 D
roof R UW1 F
tops T AA1 P S
it's IH1 T S
let's L EH1 T S
every EH1 V ER0 IY0
every(2) EH1 V R IY0
day D EY1
light L AY1 T
lights L AY1 T S
keep K IY1 P
";

fn g2p() -> EnglishPhonemizer {
    EnglishPhonemizer::from_cmudict(DICT)
}

fn p(w: &str) -> Option<String> {
    g2p().phonemes(w).map(|v| v.join(" "))
}

#[test]
fn vox_04_g2p_dictionary_words_and_punctuation() {
    assert_eq!(p("Christmas,").as_deref(), Some("K R IH1 S M AH0 S"));
    assert_eq!(p("(Merry!)").as_deref(), Some("M EH1 R IY0"));
    assert_eq!(p("“Day”").as_deref(), Some("D EY1"));
}

#[test]
fn vox_04_g2p_curly_apostrophes() {
    assert_eq!(p("It’s").as_deref(), Some("IH1 T S"));
    assert_eq!(p("Let‘s").as_deref(), Some("L EH1 T S"));
}

#[test]
fn vox_04_g2p_lyric_elision_evry_uses_short_variant() {
    // "ev'ry" is sung in two syllables: the dictionary's EV-R-IY variant.
    assert_eq!(p("ev’ry").as_deref(), Some("EH1 V R IY0"));
    assert_eq!(p("eV”ry").as_deref(), Some("EH1 V R IY0"));
}

#[test]
fn vox_04_g2p_compounds_written_together_are_split() {
    assert_eq!(p("Snowcovered").as_deref(), Some("S N OW1 K AH1 V ER0 D"));
    assert_eq!(p("rooftops").as_deref(), Some("R UW1 F T AA1 P S"));
    assert_eq!(p("lightskeep").as_deref(), Some("L AY1 T S K IY1 P"));
}

#[test]
fn vox_04_g2p_unknown_word_is_none() {
    assert_eq!(p("xqzt"), None);
    assert_eq!(p(""), None);
}

#[test]
fn vox_04_g2p_bundled_cmudict_covers_common_words() {
    let g = EnglishPhonemizer::bundled();
    for w in [
        "gloria",
        "hallelujah",
        "amen",
        "christmas",
        "beautiful",
        "shining",
    ] {
        assert!(g.phonemes(w).is_some(), "{w}");
    }
}

/// Common OCR confusions in lyric text (rn/m, in/m, ri/n, i/r, r/t) are
/// repaired when exactly one dictionary word results.
#[test]
fn vox_04_ocr_typos_repaired_to_dictionary_words() {
    use cantor_vocal::english::EnglishPhonemizer;
    use cantor_vocal::soulx::Phonemizer;
    let g = EnglishPhonemizer::bundled();
    for (typo, word) in [
        ("smg", "sing"),
        ("Chnst", "christ"),
        ("lrght", "light"),
        ("merty", "merry"),
    ] {
        assert_eq!(g.phonemes(typo), g.phonemes(word), "{typo} -> {word}");
        assert!(g.phonemes(word).is_some());
    }
    assert_eq!(g.repaired("smg").as_deref(), Some("sing"));
    assert_eq!(g.repaired("sing"), None, "known words are not touched");
    assert!(g.phonemes("doyouaecwhal").is_none(), "no wild guesses");
}
