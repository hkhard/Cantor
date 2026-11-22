//! VOX-03 contract: Cantor phrases serialised as SoulX-Singer target metadata
//! (score control mode). Format from SoulX-Singer `example/audio/*_target.json`
//! and `soulxsinger/utils/data_processor.py` at commit 81aeb3a.

use cantor_lyrics::extract;
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_vocal::soulx::{to_target_metadata, DictPhonemizer, Language};

fn gloria() -> (cantor_score::Score, cantor_timeline::Timemap) {
    // 60 bpm: Glo-ri-a (a held over two notes), rest of 1 beat, A-men.
    let xml = r#"<?xml version="1.0"?><score-partwise><part-list><score-part id="P1"><part-name>S</part-name></score-part></part-list>
    <part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
      <direction><sound tempo="60"/></direction>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><lyric number="1"><syllabic>begin</syllabic><text>Glo</text></lyric></note>
      <note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>middle</syllabic><text>ri</text></lyric></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>end</syllabic><text>a</text><extend/></lyric></note>
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>
      <note><rest/><duration>2</duration><voice>1</voice></note>
    </measure><measure number="2">
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><lyric number="1"><syllabic>begin</syllabic><text>A</text></lyric></note>
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>6</duration><voice>1</voice><lyric number="1"><syllabic>end</syllabic><text>men</text></lyric></note>
    </measure></part></score-partwise>"#;
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    (s, tm)
}

fn dict() -> DictPhonemizer {
    DictPhonemizer::from_pairs(&[("gloria", "G L AO1 R IY0 AH0"), ("amen", "AA2 M EH1 N")])
}

#[test]
fn vox_03_soulx_target_matches_reference_shape() {
    let (s, tm) = gloria();
    let phrases = extract(&s, &tm, 0, None, 1);
    assert_eq!(phrases.len(), 2);
    let segs = to_target_metadata(&phrases, Language::English, &dict()).unwrap();
    assert_eq!(segs.len(), 2);

    let a = &segs[0];
    assert_eq!(a.language, "English");
    assert_eq!(a.time, [0, 3000]);
    assert_eq!(a.text, "Glo ri a a");
    // Word phonemes distributed over syllables by vowel nuclei.
    assert_eq!(a.phoneme, "en_G-L-AO1 en_R-IY0 en_AH0 en_AH0");
    assert_eq!(a.note_pitch, "67 69 67 65");
    assert_eq!(a.note_type, "2 2 2 3");
    assert_eq!(a.duration, "1.00 0.50 0.50 1.00");

    let b = &segs[1];
    assert_eq!(b.time, [4000, 8000]);
    assert_eq!(b.text, "A men");
    assert_eq!(b.phoneme, "en_AA2 en_M-EH1-N");
    assert_eq!(b.note_type, "2 2");
}

#[test]
fn vox_03_soulx_json_is_a_list_of_segments_with_string_fields() {
    let (s, tm) = gloria();
    let segs =
        to_target_metadata(&extract(&s, &tm, 0, None, 1), Language::English, &dict()).unwrap();
    let json = cantor_vocal::soulx::to_json(&segs);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let first = &v.as_array().unwrap()[0];
    for key in [
        "index",
        "language",
        "duration",
        "text",
        "phoneme",
        "note_pitch",
        "note_type",
    ] {
        assert!(first[key].is_string(), "{key} must be a string");
    }
    assert!(first["time"].is_array());
    // Parallel lists have equal length.
    let len = |k: &str| first[k].as_str().unwrap().split(' ').count();
    assert_eq!(len("duration"), len("phoneme"));
    assert_eq!(len("note_pitch"), len("note_type"));
}

#[test]
fn vox_03_unknown_word_is_an_error_not_silence() {
    // SoulX's own G2P turns unknown words into <SP> silently; Cantor reports them.
    let (s, tm) = gloria();
    let empty = DictPhonemizer::from_pairs(&[]);
    let err =
        to_target_metadata(&extract(&s, &tm, 0, None, 1), Language::English, &empty).unwrap_err();
    assert!(err.to_string().contains("gloria"), "{err}");
}

#[test]
fn vox_03_swedish_is_rejected_for_soulx() {
    let (s, tm) = gloria();
    let err =
        to_target_metadata(&extract(&s, &tm, 0, None, 1), Language::Swedish, &dict()).unwrap_err();
    assert!(err.to_string().contains("Swedish"), "{err}");
}

/// Product decision 2026-11-22: 16 flow-matching steps without classifier-free
/// guidance (chosen by ear; RTF 2.5 on an M2 GPU).
#[test]
fn vox_03_soulx_default_render_settings() {
    let s = cantor_vocal::soulx::RenderSettings::default();
    assert_eq!(s.n_steps, 16);
    assert_eq!(s.cfg, 0.0);
    assert_eq!(s.control, "score");
    assert!(s.auto_shift);
}

/// Sung lyrics are English only; the app shows this note wherever singing is offered.
#[test]
fn vox_03_english_only_notice() {
    let note = cantor_vocal::SUNG_LYRICS_NOTICE;
    assert!(note.contains("English"));
    assert!(cantor_vocal::sung_language_supported("en"));
    assert!(!cantor_vocal::sung_language_supported("sv"));
}

/// For real scores (OCR typos), an unknown word is sung on "la" and reported,
/// so one bad word does not block the whole part.
#[test]
fn vox_03_lenient_mode_sings_la_and_reports_unknown_words() {
    let (s, tm) = gloria();
    let only_amen = DictPhonemizer::from_pairs(&[("amen", "AA2 M EH1 N")]);
    let (segs, unknown) = cantor_vocal::soulx::to_target_metadata_lenient(
        &extract(&s, &tm, 0, None, 1),
        Language::English,
        &only_amen,
    )
    .unwrap();
    assert_eq!(unknown, ["gloria"]);
    assert_eq!(segs[0].phoneme, "en_L-AA1 en_L-AA1 en_L-AA1 en_L-AA1");
    assert_eq!(segs[0].note_type, "2 2 2 3", "rhythm and melisma kept");
    assert_eq!(segs[1].phoneme, "en_AA2 en_M-EH1-N");
}
