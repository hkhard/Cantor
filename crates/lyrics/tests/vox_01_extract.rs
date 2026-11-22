//! VOX-01 extractor: part + verse -> phrases of sung notes, timed from the
//! timemap (repeats unfolded, tempo applied).

use cantor_lyrics::{extract, NoteKind, Phrase};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;

/// 4/4 at 60 bpm (1 beat = 1 s). Tenor in voice 1, an alto voice 2 on the
/// same staff that must be ignored when singing voice 1.
fn score(body: &str) -> cantor_score::Score {
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
        <part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
        <direction><sound tempo="60"/></direction>{body}</measure></part></score-partwise>"#
    );
    parse_musicxml(xml.as_bytes()).unwrap()
}

fn n(step: &str, dur: u32, lyric: &str) -> String {
    format!(
        r#"<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{dur}</duration><voice>1</voice>{lyric}</note>"#
    )
}
fn ly(syl: &str, text: &str) -> String {
    format!(r#"<lyric number="1"><syllabic>{syl}</syllabic><text>{text}</text></lyric>"#)
}
const REST1: &str = r#"<note><rest/><duration>1</duration><voice>1</voice></note>"#;

fn phrases(body: &str) -> Vec<Phrase> {
    let s = score(body);
    let tm = build_timemap(&s);
    extract(&s, &tm, 0, None, 1)
}

#[test]
fn vox_01_syllables_words_and_timing() {
    let body = [
        n("C", 1, &ly("begin", "Glo")),
        n("D", 1, &ly("middle", "ri")),
        n("E", 2, &ly("end", "a")),
    ]
    .concat();
    let p = phrases(&body);
    assert_eq!(p.len(), 1);
    let notes = &p[0].notes;
    let got: Vec<_> = notes
        .iter()
        .map(|x| (x.kind, x.text.as_str(), x.pitch))
        .collect();
    assert_eq!(
        got,
        [
            (NoteKind::Syllable, "Glo", 60),
            (NoteKind::Syllable, "ri", 62),
            (NoteKind::Syllable, "a", 64)
        ]
    );
    assert_eq!(p[0].words(), ["Gloria"]);
    assert!((notes[2].start_sec - 2.0).abs() < 1e-9);
    assert!((notes[2].duration_sec - 2.0).abs() < 1e-9);
    assert!((p[0].start_sec - 0.0).abs() < 1e-9 && (p[0].end_sec - 4.0).abs() < 1e-9);
}

#[test]
fn vox_01_melisma_does_not_cross_a_phrase_break() {
    // "a" then a long rest, then lyric-less notes (instrumental bars, or lyrics
    // OCR missed): those must not continue the old syllable.
    let body = [
        n("C", 1, &ly("single", "a")),
        REST1.to_string(),
        REST1.to_string(),
        n("D", 1, ""),
    ]
    .concat();
    let p = phrases(&body);
    assert_eq!(p.len(), 2);
    assert_eq!(p[1].notes[0].kind, NoteKind::NoLyric);
    assert!(p[1].notes[0].text.is_empty());
}

#[test]
fn vox_01_melisma_notes_continue_the_syllable() {
    // "a" held over three notes: explicit extender and lyric-less notes.
    let body = [
        n(
            "C",
            1,
            r#"<lyric number="1"><syllabic>single</syllabic><text>a</text><extend/></lyric>"#,
        ),
        n("D", 1, r#"<lyric number="1"><extend type="stop"/></lyric>"#),
        n("E", 1, ""),
        n("F", 1, &ly("single", "men")),
    ]
    .concat();
    let p = phrases(&body);
    let kinds: Vec<_> = p[0]
        .notes
        .iter()
        .map(|x| (x.kind, x.text.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (NoteKind::Syllable, "a"),
            (NoteKind::Melisma, "a"),
            (NoteKind::Melisma, "a"),
            (NoteKind::Syllable, "men")
        ]
    );
}

#[test]
fn vox_01_tied_notes_merge_into_one_sung_note() {
    let body = [
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="start"/><voice>1</voice><lyric number="1"><syllabic>single</syllabic><text>Ah</text></lyric></note>"#.to_string(),
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>1</voice></note>"#.to_string(),
    ]
    .concat();
    let p = phrases(&body);
    assert_eq!(p[0].notes.len(), 1);
    assert!((p[0].notes[0].duration_sec - 4.0).abs() < 1e-9);
}

#[test]
fn vox_01_phrases_split_at_rests_of_a_beat_or_more() {
    let body = [
        n("C", 1, &ly("single", "one")),
        REST1.to_string(),
        n("D", 1, &ly("single", "two")),
        n("E", 1, &ly("single", "three")),
    ]
    .concat();
    let p = phrases(&body);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].words(), ["one"]);
    assert_eq!(p[1].words(), ["two", "three"]);
    assert!((p[1].start_sec - 2.0).abs() < 1e-9);
}

#[test]
fn vox_01_never_assigns_a_syllable_to_a_rest_and_words_close() {
    let body = [
        n("C", 1, &ly("begin", "Ky")),
        REST1.to_string(),
        n("D", 2, &ly("end", "rie")),
    ]
    .concat();
    let p = phrases(&body);
    for ph in &p {
        for note in &ph.notes {
            assert!(note.pitch > 0, "rest got a syllable");
        }
    }
    // A word split by a rest still forms one word, in the phrase where it ends.
    let words: Vec<String> = p.iter().flat_map(|x| x.words()).collect();
    assert_eq!(words, ["Kyrie"]);
}

#[test]
fn vox_01_only_the_chosen_voice_and_verse() {
    let body = [
        n("C", 4, r#"<lyric number="1"><text>one</text></lyric><lyric number="2"><text>uno</text></lyric>"#),
        r#"<backup><duration>4</duration></backup>"#.to_string(),
        r#"<note><pitch><step>A</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice><lyric number="1"><text>alto</text></lyric></note>"#.to_string(),
    ]
    .concat();
    let s = score(&body);
    let tm = build_timemap(&s);
    let v1 = extract(&s, &tm, 0, Some(1), 2);
    assert_eq!(v1[0].words(), ["uno"]);
    let v2 = extract(&s, &tm, 0, Some(2), 1);
    assert_eq!(v2[0].words(), ["alto"]);
    // No voice filter: top voice of chords/voices wins (highest pitch at an onset).
    let all = extract(&s, &tm, 0, None, 1);
    assert_eq!(all[0].notes.len(), 1);
    assert_eq!(all[0].notes[0].pitch, 60);
}

#[test]
fn vox_01_repeats_sing_each_pass_with_its_verse() {
    // |: C(1:"first" 2:"second") :|  sung on verse 1 then verse 2 when available.
    let xml = r#"<?xml version="1.0"?><score-partwise><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
      <part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes>
        <direction><sound tempo="60"/></direction>
        <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice>
          <lyric number="1"><text>first</text></lyric><lyric number="2"><text>second</text></lyric></note>
        <barline location="right"><repeat direction="backward"/></barline></measure></part></score-partwise>"#;
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let p = extract(&s, &tm, 0, None, 1);
    let words: Vec<String> = p.iter().flat_map(|x| x.words()).collect();
    assert_eq!(
        words,
        ["first", "second"],
        "pass 2 of a repeat sings verse 2"
    );
}

fn ly_n(num: u32, syl: &str, text: &str) -> String {
    format!(r#"<lyric number="{num}"><syllabic>{syl}</syllabic><text>{text}</text></lyric>"#)
}

/// OCR spreads one through-composed text over lyric lines 1 and 2 (it numbers
/// lines by height). Merged mode sings line 1 where present, else any line.
#[test]
fn vox_01_merged_verses_take_any_line_when_requested_one_is_missing() {
    let body = [
        n("C", 1, &ly_n(1, "single", "Snow")),
        n("D", 1, &ly_n(2, "single", "falls")),
        n(
            "E",
            1,
            &(ly_n(1, "single", "on") + &ly_n(2, "single", "xx")),
        ),
        n("F", 1, ""),
    ]
    .concat();
    let s = score(&body);
    let tm = build_timemap(&s);
    let p = cantor_lyrics::extract_merged(&s, &tm, 0, None);
    let got: Vec<_> = p[0]
        .notes
        .iter()
        .map(|x| (x.kind, x.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (NoteKind::Syllable, "Snow"),
            (NoteKind::Syllable, "falls"),
            (NoteKind::Syllable, "on"),
            (NoteKind::Melisma, "on"),
        ]
    );
}

/// Performer cues printed in the lyric line ("Female:", "Children:") are not sung.
#[test]
fn vox_01_role_labels_are_not_sung() {
    let body = [
        n("C", 1, &ly("single", "Female:")),
        n("D", 1, &ly("single", "You")),
        n("E", 1, &ly("single", "say:")),
    ]
    .concat();
    let p = phrases(&body);
    let got: Vec<_> = p[0]
        .notes
        .iter()
        .map(|x| (x.kind, x.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (NoteKind::NoLyric, ""),
            (NoteKind::Syllable, "You"),
            (NoteKind::Syllable, "say:"),
        ]
    );
}

/// OCR puts page text into lyric lines: dynamics, credits, URLs, stray
/// letters. Those are not sung or shown as lyrics. Real short words stay.
#[test]
fn vox_01_ocr_noise_is_not_lyric_text() {
    use cantor_lyrics::is_lyric_text;
    for junk in [
        "PP",
        "p",
        "mf",
        "Mf",
        "ff",
        "sfz",
        "f",
        "w",
        "/'",
        "ISMN",
        "(www.1i1ypond.org)",
        "www.studentersangforeningenno",
        "Studentersangforening,",
        "+T2 '",
        "-",
        "",
    ] {
        assert!(!is_lyric_text(junk), "{junk:?} should be dropped");
    }
    for word in [
        "Bre",
        "de",
        "Seil",
        "o",
        "O",
        "a",
        "I",
        "ö",
        "i",
        "y",
        "Oh,",
        "It’s",
        "ev'ry",
        "Kom",
        "Dan-mark:",
        "go-o,",
        "Tryg",
    ] {
        assert!(is_lyric_text(word), "{word:?} should be kept");
    }
}

/// Verse numbers printed before the first syllable ("1.", "1.3.") are not sung.
#[test]
fn vox_01_verse_number_prefix_is_stripped() {
    use cantor_lyrics::clean_lyric;
    assert_eq!(clean_lyric("1.3.want").as_deref(), Some("want"));
    assert_eq!(clean_lyric("2. Seks").as_deref(), Some("Seks"));
    assert_eq!(clean_lyric("Bre").as_deref(), Some("Bre"));
    assert_eq!(clean_lyric("mf"), None);
}
