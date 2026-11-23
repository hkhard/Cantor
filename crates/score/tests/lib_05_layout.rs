//! LIB-05: the play cursor on the original PDF, for scores whose PDF was
//! engraved from the same MusicXML (MuseScore, Finale, Dorico, Sibelius
//! exports). The measure boxes come from the layout the file itself carries
//! (page size, margins, system and page breaks, measure widths, staff
//! distances), so no OMR is needed.

use cantor_score::{layout_measure_boxes, parse_score_file};

fn fixture(p: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../fixtures/scores/{p}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn lib_05_measure_boxes_from_musicxml_layout_anchor_song() {
    let mxl = fixture("the-anchor-song/the-anchor-song.mxl");
    let score = parse_score_file(&mxl).unwrap();
    let boxes = layout_measure_boxes(&mxl).expect("layout");
    // One box per written measure, in order.
    assert_eq!(boxes.len(), score.parts[0].measures.len());
    // Two pages (the PDF has 2), systems break where the file says.
    assert_eq!(boxes.first().unwrap().page, 1);
    assert_eq!(boxes.last().unwrap().page, 2);
    for b in &boxes {
        assert!(
            b.x >= 0.0 && b.y >= 0.0 && b.x + b.w <= 1.0 + 1e-9 && b.y + b.h <= 1.0 + 1e-9,
            "{b:?}"
        );
        assert!(b.w > 0.0 && b.h > 0.0);
    }
    // Measures 1-4 share the first system; 5 starts a new one, lower down
    // and further left (no indent, no clef/time width).
    assert_eq!(boxes[0].y, boxes[3].y);
    assert!(boxes[4].y > boxes[0].y + boxes[0].h);
    assert!(boxes[4].x < boxes[0].x);
    // Within a system, each measure starts where the previous one ends.
    assert!((boxes[1].x - (boxes[0].x + boxes[0].w)).abs() < 1e-9);
    // The box spans both staves (Tenor and Bass): taller than one staff.
    let staff = 40.0 / 1696.93;
    assert!(boxes[0].h > 2.0 * staff, "{}", boxes[0].h);
}

#[test]
fn lib_05_measure_boxes_lacrymosa_hidden_part_name_and_grand_staff() {
    let mxl = fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.mxl");
    let boxes = layout_measure_boxes(&mxl).expect("layout");
    // 30 written measures (150 counts all five parts).
    assert_eq!(boxes.len(), 30);
    // Five pages, the first measure on page 1, the last on page 5.
    assert_eq!(boxes[0].page, 1);
    assert_eq!(boxes.last().unwrap().page, 5);
    let pages: std::collections::BTreeSet<u32> = boxes.iter().map(|b| b.page).collect();
    assert_eq!(pages.len(), 5);
    // Every system's measures fill the width between the margins (MuseScore
    // justifies systems): the last box of each system ends at the right margin.
    let right = 1.0 - 69.6429 / 1517.86;
    let mut ends = 0;
    for w in boxes.windows(2) {
        if w[1].y != w[0].y || w[1].page != w[0].page {
            assert!((w[0].x + w[0].w - right).abs() < 0.01, "{:?}", w[0]);
            ends += 1;
        }
    }
    // 10 systems: 4 page breaks + 5 system breaks + the first one.
    assert_eq!(ends, 9, "{ends} system ends");
}

#[test]
fn lib_05_no_layout_means_no_boxes() {
    // Files without layout (hand-written, OMR output, our own writer) give
    // None, so the app falls back to the PDF without a cursor.
    let x = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>S</part-name></score-part></part-list>
      <part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note></measure></part></score-partwise>"#;
    assert!(layout_measure_boxes(x.as_bytes()).is_none());
    assert!(layout_measure_boxes(b"not xml").is_none());
}
